#!/bin/bash
set -xe

# Copy Sol-based JSON artifacts `<contract_name>.json` for L1 contracts
L1_JSON_ABI_SRC_DIR=contracts/l1-contracts/zkout
L1_JSON_ABI_DST_DIR=e2e-tests-rust/src/contracts/artifacts

mkdir -p $L1_JSON_ABI_DST_DIR

l1_contracts=("IL1Messenger" "IBridgehub")

for l1_contract in "${l1_contracts[@]}"; do
  cp "$L1_JSON_ABI_SRC_DIR/$l1_contract.sol/$l1_contract.json" $L1_JSON_ABI_DST_DIR
done
