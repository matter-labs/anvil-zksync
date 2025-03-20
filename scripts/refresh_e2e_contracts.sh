#!/bin/bash
set -xe

# Copy Sol-based JSON artifacts `<contract_name>.json` for L1 contracts
L1_JSON_ABI_SRC_DIR=contracts/l1-contracts/zkout
L1_JSON_ABI_DST_DIR=e2e-tests-rust/src/contracts/artifacts

mkdir -p $L1_JSON_ABI_DST_DIR

l1_contracts=("IBridgehub" "IMailbox" "IL1AssetRouter" "IL1Nullifier")

for l1_contract in "${l1_contracts[@]}"; do
  cp "$L1_JSON_ABI_SRC_DIR/$l1_contract.sol/$l1_contract.json" $L1_JSON_ABI_DST_DIR
done

SYSTEM_JSON_ABI_SRC_DIR=contracts/system-contracts/zkout
SYSTEM_JSON_ABI_DST_DIR=e2e-tests-rust/src/contracts/artifacts

mkdir -p $SYSTEM_JSON_ABI_DST_DIR

system_contracts=("IL1Messenger" "IBaseToken")

for system_contract in "${system_contracts[@]}"; do
  cp "$SYSTEM_JSON_ABI_SRC_DIR/$system_contract.sol/$system_contract.json" $SYSTEM_JSON_ABI_DST_DIR
done

TEST_CONTRACT_SRC_DIR=etc/test-contracts/zkout
TEST_CONTRACT_DST_DIR=e2e-tests-rust/src/test_contracts/artifacts

mkdir -p $TEST_CONTRACT_DST_DIR

test_contracts=("Counter")

for test_contract in "${test_contracts[@]}"; do
  cp "$TEST_CONTRACT_SRC_DIR/$test_contract.sol/$test_contract.json" $TEST_CONTRACT_DST_DIR
done
