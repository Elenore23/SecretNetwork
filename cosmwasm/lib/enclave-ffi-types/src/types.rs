use core::ffi::c_void;
use derive_more::Display;
use rand;
use secp256k1;

/// This type represents an opaque pointer to a memory address in normal user space.
#[repr(C)]
pub struct UserSpaceBuffer {
    pub ptr: *mut c_void,
}

/// This type represents an opaque pointer to a memory address inside the enclave.
#[repr(C)]
pub struct EnclaveBuffer {
    pub ptr: *mut c_void,
}

impl EnclaveBuffer {
    pub unsafe fn clone(&self) -> EnclaveBuffer {
        EnclaveBuffer { ptr: self.ptr }
    }
}

/// This struct holds a pointer to memory in userspace, that contains the storage
#[repr(C)]
pub struct Ctx {
    pub data: *mut c_void,
}

impl Ctx {
    pub unsafe fn clone(&self) -> Ctx {
        Ctx { data: self.data }
    }
}

/// This type represents the possible error conditions that can be encountered in the enclave
/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Debug, Display)]
pub enum EnclaveError {
    /// This indicated failed ocalls, but ocalls during callbacks from wasm code will not currently
    /// be represented this way. This is doable by returning a `TrapKind::Host` from these callbacks,
    /// but that's a TODO at the moment.
    FailedOcall,
    /// The WASM code was invalid and could not be loaded.
    InvalidWasm,
    /// The WASM module contained a start section, which is not allowed.
    WasmModuleWithStart,
    /// The WASM module contained floating point operations, which is not allowed.
    WasmModuleWithFP,
    /// Calling a function in the contract failed.
    FailedFunctionCall,
    /// Fail to inject gas metering
    FailedGasMeteringInjection,
    /// Ran out of gas
    OutOfGas,
    /// Unexpected Error happened, no more details available
    Unknown,
}

pub enum CryptoError {
    /// The `DerivingKeyError` error.
    ///
    /// This error means that the ECDH process failed.
    DerivingKeyError {
        self_key: [u8; 64],
        other_key: [u8; 64],
    },
    /// The `MissingKeyError` error.
    ///
    /// This error means that a key was missing.
    MissingKeyError {
        key_type: &'static str,
    },
    /// The `DecryptionError` error.
    ///
    /// This error means that the symmetric decryption has failed for some reason.
    DecryptionError,
    /// The `ImproperEncryption` error.
    ///
    /// This error means that the ciphertext provided was imporper.
    /// e.g. MAC wasn't valid, missing IV etc.
    ImproperEncryption,
    /// The `EncryptionError` error.
    ///
    /// This error means that the symmetric encryption has failed for some reason.
    EncryptionError,
    /// The `SigningError` error.
    ///
    /// This error means that the signing process has failed for some reason.
    SigningError {
        hashed_msg: [u8; 32],
    },
    /// The `ParsingError` error.
    ///
    /// This error means that the signature couldn't be parsed correctly.
    ParsingError {
        sig: [u8; 65],
    },
    /// The `RecoveryError` error.
    ///
    /// This error means that the public key can't be recovered from that message & signature.
    RecoveryError {
        sig: [u8; 65],
    },
    /// The `KeyError` error.
    ///
    /// This error means that a key wasn't vaild.
    /// e.g. PrivateKey, PubliKey, SharedSecret.
    // #[cfg(feature = "asymmetric")]
    KeyError {
        key_type: &'static str,
        err: Option<secp256k1::Error>,
    },
    // #[cfg(not(feature = "asymmetric"))]
    // KeyError { key_type: &'static str, err: Option<()> },
    // /// The `RandomError` error.
    // ///
    // /// This error means that the random function had failed generating randomness.
    // #[cfg(feature = "std")]
    RandomError {
        err: rand::Error,
    },
    // #[cfg(feature = "sgx")]
    // RandomError {
    //     err: sgx_types::sgx_status_t,
    // },
}

/// This struct is returned from ecall_init.
/// cbindgen:prefix-with-name
#[repr(C)]
pub enum InitResult {
    Success {
        /// A pointer to the output of the calculation
        output: UserSpaceBuffer,
        /// The gas used by the execution.
        used_gas: u64,
        /// A signature by the enclave on all of the results.
        signature: [u8; 65],
    },
    Failure {
        err: EnclaveError,
    },
}

/// This struct is returned from ecall_handle.
/// cbindgen:prefix-with-name
#[repr(C)]
pub enum HandleResult {
    Success {
        /// A pointer to the output of the calculation
        output: UserSpaceBuffer,
        /// The gas used by the execution.
        used_gas: u64,
        /// A signature by the enclave on all of the results.
        signature: [u8; 65],
    },
    Failure {
        err: EnclaveError,
    },
}

/// This struct is returned from ecall_query.
/// cbindgen:prefix-with-name
#[repr(C)]
pub enum QueryResult {
    Success {
        /// A pointer to the output of the calculation
        output: UserSpaceBuffer,
        /// The gas used by the execution.
        used_gas: u64,
        /// A signature by the enclave on all of the results.
        signature: [u8; 65],
    },
    Failure {
        err: EnclaveError,
    },
}

/// This struct is returned from ecall_key_gen.
/// cbindgen:prefix-with-name
#[repr(C)]
pub enum KeyGenResult {
    Success {
        /// A pointer to the output of the calculation
        output: UserSpaceBuffer,
        /// A signature by the enclave on all of the results.
        signature: [u8; 65],
    },
    Failure {
        err: CryptoError,
    },
}
