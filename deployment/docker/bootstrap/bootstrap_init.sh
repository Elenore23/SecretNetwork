#!/bin/bash

file=~/.secretd/config/genesis.json
if [ ! -e "$file" ]
then
  # init the node
  rm -rf ~/.secretd/*
  rm -rf ~/.sgx_secrets/*
#  secretd config chain-id enigma-pub-testnet-3
#  secretd config output json
#  secretd config indent true
#  secretd config trust-node true
#  secretd config keyring-backend test
  export SECRET_NETWORK_CHAIN_ID=secretdev-1
  export SECRET_NETWORK_KEYRING_BACKEND=test
  secretd init banana --chain-id secretdev-1

  cp ~/node_key.json ~/.secretd/config/node_key.json

  perl -i -pe 's/"stake"/ "uscrt"/g' ~/.secretd/config/genesis.json
  secretd keys add a
  secretd keys add b
  secretd keys add c
  secretd keys add d

  secretd add-genesis-account "$(secretd keys show -a a)" 1000000000000000000uscrt
#  secretd add-genesis-account "$(secretd keys show -a b)" 1000000000000000000uscrt
#  secretd add-genesis-account "$(secretd keys show -a c)" 1000000000000000000uscrt
#  secretd add-genesis-account "$(secretd keys show -a d)" 1000000000000000000uscrt


  secretd gentx a 1000000uscrt
#  secretd gentx b 1000000uscrt --keyring-backend test
#  secretd gentx c 1000000uscrt --keyring-backend test
#  secretd gentx d 1000000uscrt --keyring-backend test

  secretd collect-gentxs
  secretd validate-genesis

  secretd init-bootstrap
  secretd validate-genesis
fi

# sleep infinity
source /opt/sgxsdk/environment && RUST_BACKTRACE=1 secretd start --rpc.laddr tcp://0.0.0.0:26657 --bootstrap