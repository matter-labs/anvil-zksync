#!/usr/bin/env bash

set -xe

TEST_CONTRACT_ARTIFACTS="etc/test-contracts/zkout"
TEST_CONTRACT_TARGET="crates/core/src/deps/test-contracts"

echo "Building test contracts"

test_contracts=("Primary" "Secondary")

for test_contract in "${test_contracts[@]}"; do
  cp "$TEST_CONTRACT_ARTIFACTS/$test_contract.sol/$test_contract.json" $TEST_CONTRACT_TARGET
done

echo "Done"
