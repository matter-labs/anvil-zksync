#!/usr/bin/env bash

set -xe

TEST_CONTRACT_ARTIFACTS="etc/test-contracts/artifacts-zk/cache-zk/solpp-generated-contracts"
TEST_CONTRACT_TARGET="src/deps/test-contracts"

echo "Building test contracts"
(cd etc/test-contracts && yarn && yarn build)

echo "Copying test contracts"
mkdir -p $TEST_CONTRACT_TARGET
find $TEST_CONTRACT_ARTIFACTS -name "*.json" ! -iname "*.dbg.json" -exec cp {} $TEST_CONTRACT_TARGET \;

echo "Done"
