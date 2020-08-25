use log::*;

use crate::cosmwasm::encoding::Binary;
use crate::cosmwasm::types::{
    CanonicalAddr, CosmosSignature, Env, HumanAddr, SigInfo, SignDoc, SignDocWasmMsg,
};
use crate::crypto::traits::PubKey;
use crate::crypto::{sha_256, AESKey, Hmac, Kdf, HASH_SIZE, KEY_MANAGER};
use crate::wasm::io;
use crate::wasm::types::SecretMessage;
use enclave_ffi_types::EnclaveError;

pub type ContractKey = [u8; CONTRACT_KEY_LENGTH];

pub const CONTRACT_KEY_LENGTH: usize = HASH_SIZE + HASH_SIZE;

const HEX_ENCODED_HASH_SIZE: usize = HASH_SIZE * 2;

pub fn generate_encryption_key(
    env: &Env,
    contract: &[u8],
    contract_address: &[u8],
) -> Result<[u8; CONTRACT_KEY_LENGTH], EnclaveError> {
    let consensus_state_ikm = KEY_MANAGER.get_consensus_state_ikm().unwrap();

    let contract_hash = calc_contract_hash(contract);

    let sender_id = generate_sender_id(env.message.sender.as_slice(), env.block.height as u64);

    let mut encryption_key = [0u8; 64];

    let authenticated_contract_id = generate_contract_id(
        &consensus_state_ikm,
        &sender_id,
        &contract_hash,
        contract_address,
    );

    encryption_key[0..32].copy_from_slice(&sender_id);
    encryption_key[32..].copy_from_slice(&authenticated_contract_id);

    Ok(encryption_key)
}

pub fn extract_contract_key(env: &Env) -> Result<[u8; CONTRACT_KEY_LENGTH], EnclaveError> {
    if env.contract_key.is_none() {
        error!("Contract execute with empty contract key");
        return Err(EnclaveError::FailedContractAuthentication);
    }

    let contract_key =
        base64::decode(env.contract_key.as_ref().unwrap().as_bytes()).map_err(|err| {
            error!(
                "got an error while trying to deserialize output bytes into json {:?}: {}",
                env, err
            );
            EnclaveError::FailedContractAuthentication
        })?;

    if contract_key.len() != CONTRACT_KEY_LENGTH {
        error!("Contract execute with empty contract key");
        return Err(EnclaveError::FailedContractAuthentication);
    }

    let mut key_as_bytes = [0u8; CONTRACT_KEY_LENGTH];

    key_as_bytes.copy_from_slice(&contract_key);

    Ok(key_as_bytes)
}

pub fn generate_sender_id(msg_sender: &[u8], block_height: u64) -> [u8; HASH_SIZE] {
    let mut input_data = msg_sender.to_vec();
    input_data.extend_from_slice(&block_height.to_be_bytes());
    sha_256(&input_data)
}

pub fn generate_contract_id(
    consensus_state_ikm: &AESKey,
    sender_id: &[u8; HASH_SIZE],
    code_hash: &[u8; HASH_SIZE],
    contract_address: &[u8],
) -> [u8; HASH_SIZE] {
    let authentication_key = consensus_state_ikm.derive_key_from_this(sender_id.as_ref());

    let mut input_data = sender_id.to_vec();
    input_data.extend_from_slice(code_hash);
    input_data.extend_from_slice(contract_address);
    authentication_key.sign_sha_256(&input_data)
}

pub fn calc_contract_hash(contract_bytes: &[u8]) -> [u8; HASH_SIZE] {
    sha_256(&contract_bytes)
}

pub fn validate_contract_key(
    contract_key: &[u8; CONTRACT_KEY_LENGTH],
    contract_address: &[u8],
    contract_code: &[u8],
) -> bool {
    // parse contract key -> < signer_id || authentication_code >
    let mut signer_id: [u8; HASH_SIZE] = [0u8; HASH_SIZE];
    signer_id.copy_from_slice(&contract_key[0..HASH_SIZE]);

    let mut expected_authentication_id: [u8; HASH_SIZE] = [0u8; HASH_SIZE];
    expected_authentication_id.copy_from_slice(&contract_key[HASH_SIZE..]);

    // calculate contract hash
    let contract_hash = calc_contract_hash(contract_code);

    // get the enclave key
    let enclave_key = KEY_MANAGER
        .get_consensus_state_ikm()
        .map_err(|_err| {
            error!("Error extracting consensus_state_key");
            false
        })
        .unwrap();

    // calculate the authentication_id
    let calculated_authentication_id =
        generate_contract_id(&enclave_key, &signer_id, &contract_hash, contract_address);

    calculated_authentication_id == expected_authentication_id
}

pub fn validate_msg(msg: &[u8], contract_code: &[u8]) -> Result<Vec<u8>, EnclaveError> {
    if msg.len() < HEX_ENCODED_HASH_SIZE {
        error!("Malformed message - expected contract code hash to be prepended to the msg");
        return Err(EnclaveError::ValidationFailure);
    }

    let contract_hash = calc_contract_hash(contract_code);

    let mut encrypted_contract_hash: [u8; HEX_ENCODED_HASH_SIZE] = [0u8; HEX_ENCODED_HASH_SIZE];
    encrypted_contract_hash.copy_from_slice(&msg[0..HEX_ENCODED_HASH_SIZE]);

    let decoded_hash: Vec<u8> = hex::decode(&encrypted_contract_hash[..]).map_err(|_| {
        error!("Got exec message with malformed contract hash");
        EnclaveError::ValidationFailure
    })?;

    if decoded_hash != contract_hash {
        error!("Message contains mismatched contract hash");
        return Err(EnclaveError::ValidationFailure);
    }

    Ok(msg[HEX_ENCODED_HASH_SIZE..].to_vec())
}

pub fn verify_params(
    sig_info: &SigInfo,
    env: &Env,
    msg: &SecretMessage,
) -> Result<(), EnclaveError> {
    trace!("Verifying message signatures..");

    // If there's no callback signature - it's not a callback and there has to be a tx signer + signature
    if let Some(callback_sig) = &sig_info.callback_sig {
        if verify_callback_sig(callback_sig.as_slice(), &env.message.sender, msg) {
            trace!("Message verified! msg.sender is the calling contract");
            return Ok(());
        }

        error!("Callback signature verification failed");
    } else {
        trace!(
            "Sign bytes are: {:?}",
            String::from_utf8_lossy(sig_info.sign_bytes.as_slice())
        );

        let sign_doc: SignDoc =
            serde_json::from_slice(sig_info.sign_bytes.as_slice()).map_err(|err| {
                error!(
                    "got an error while trying to deserialize sign doc bytes into json {:?}: {}",
                    sig_info.sign_bytes.as_slice(),
                    err
                );
                EnclaveError::FailedToDeserialize
            })?;

        trace!("sign doc: {:?}", sign_doc);

        // This verifies that signatures and sign bytes are self consistent
        sig_info
            .signature
            .get_public_key()
            .verify_bytes(
                &sig_info.sign_bytes.as_slice(),
                &sig_info.signature.get_signature().as_slice(),
            )
            .map_err(|err| {
                error!("Signature verification failed: {:?}", err);
                EnclaveError::FailedTxVerification
            })?;

        if verify_signature_params(&sign_doc, sig_info, env, msg) {
            trace!("Parameters verified successfully");
            return Ok(());
        }

        error!("Parameter verification failed");
    }

    Err(EnclaveError::FailedTxVerification)
}

fn verify_callback_sig(
    callback_signature: &[u8],
    sender: &CanonicalAddr,
    msg: &SecretMessage,
) -> bool {
    if callback_signature.is_empty() {
        return false;
    }

    let callback_sig = io::create_callback_signature(sender, msg);

    if !callback_signature.eq(callback_sig.as_slice()) {
        info!(
            "Contract signature does not match with the one sent: {:?}",
            callback_signature
        );
        return false;
    }

    true
}

fn verify_sender(signature: &CosmosSignature, msg_sender: &CanonicalAddr) -> bool {
    let sender_pubkey = signature.get_public_key();
    let address = sender_pubkey.get_address();

    if address.eq(&msg_sender) {
        return true;
    }

    false
}

fn get_verified_msg<'a>(
    sign_doc: &'a SignDoc,
    sent_msg: &'a SecretMessage,
) -> Option<&'a SignDocWasmMsg> {
    sign_doc.msgs.iter().find(|&m| match m {
        SignDocWasmMsg::Execute { msg, .. } | SignDocWasmMsg::Instantiate { init_msg: msg, .. } => {
            let binary_msg_result = Binary::from_base64(msg);
            if let Ok(binary_msg) = binary_msg_result {
                return Binary(sent_msg.to_vec()) == binary_msg;
            }

            false
        }
    })
}

fn verify_contract(msg: &SignDocWasmMsg, env: &Env) -> bool {
    // Contract address is relevant only to execute, since during sending an instantiate message the contract address is not yet known
    if let SignDocWasmMsg::Execute { contract, .. } = msg {
        trace!("Verifying contract address..");
        if let Ok(human_addr) = HumanAddr::from_canonical(env.contract.address.clone()) {
            if human_addr != *contract {
                trace!(
                    "Contract address sent to enclave {:?} is not the same as the signed one {:?}",
                    human_addr,
                    *contract
                );
                return false;
            }
        } else {
            trace!("Address sent to the enclave is malformed!");
            return false;
        }
    }

    true
}

fn verify_funds(msg: &SignDocWasmMsg, env: &Env) -> bool {
    match msg {
        SignDocWasmMsg::Execute { sent_funds, .. }
        | SignDocWasmMsg::Instantiate {
            init_funds: sent_funds,
            ..
        } => match &env.message.sent_funds {
            Some(env_sent_funds) if env_sent_funds == sent_funds => true,
            None if sent_funds.is_empty() => true,
            _ => {
                debug!(
                    "Funds sent to enclave {:?} are not the same as the signed one {:?}",
                    env.message.sent_funds, *sent_funds,
                );
                false
            }
        },
    }
}

fn verify_signature_params(
    sign_doc: &SignDoc,
    sig_info: &SigInfo,
    env: &Env,
    sent_msg: &SecretMessage,
) -> bool {
    trace!("Verifying sender..");
    if !verify_sender(&sig_info.signature, &env.message.sender) {
        error!("Sender verification failed!");
        debug!(
            "Message sender {:?} does not match with the message signer {:?}",
            &env.message.sender,
            &sig_info.signature.get_public_key().get_address()
        );
        return false;
    }

    trace!("Verifying message..");
    // If msg is not found (is None) then it means message verification failed,
    // since it didn't find a matching signed message
    let msg = get_verified_msg(sign_doc, sent_msg);
    if msg.is_none() {
        error!("Message verification failed!");
        debug!(
            "Message sent to contract {:?} is not equal to any signed messages {:?}",
            sent_msg.to_vec(),
            sign_doc.msgs
        );
        return false;
    }
    let msg = msg.unwrap();

    if !verify_contract(msg, env) {
        error!("Contract address verification failed!");
        return false;
    }

    trace!("Verifying funds..");
    if !verify_funds(msg, env) {
        error!("Funds verification failed!");
        return false;
    }

    true
}
