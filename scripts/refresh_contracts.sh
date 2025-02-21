#!/bin/bash
set -xe

SRC_DIR=contracts/system-contracts/artifacts-zk/contracts-preprocessed
DEV_CONTRACTS_SRC_DIR=contracts/l2-contracts/artifacts-zk/contracts/dev-contracts/
DST_DIR=crates/core/src/deps/contracts/

mkdir -p $DST_DIR

contracts=("AccountCodeStorage" "BootloaderUtilities" "Compressor" "ComplexUpgrader" "ContractDeployer" "DefaultAccount" "DefaultAccountNoSecurity" "EmptyContract" "ImmutableSimulator" "KnownCodesStorage" "L1Messenger" "L2BaseToken" "MsgValueSimulator" "NonceHolder" "SystemContext" "PubdataChunkPublisher" "Create2Factory")
dev_contracts=("TimestampAsserter")

for contract in "${contracts[@]}"; do
    cp $SRC_DIR/$contract.sol/$contract.json $DST_DIR
done

for contract in "${dev_contracts[@]}"; do
    cp $DEV_CONTRACTS_SRC_DIR/$contract.sol/$contract.json $DST_DIR
done

precompiles=("EcAdd" "EcMul" "Ecrecover" "Keccak256" "SHA256" "EcPairing" "CodeOracle" "P256Verify" "Modexp")

for precompile in "${precompiles[@]}"; do
    cp contracts/system-contracts/contracts-preprocessed/precompiles/artifacts/$precompile.yul.zbin $DST_DIR
done

cp contracts/system-contracts/contracts-preprocessed/artifacts/EventWriter.yul.zbin $DST_DIR


bootloaders=("fee_estimate"  "gas_test" "playground_batch" "proved_batch" "proved_batch_impersonating" "fee_estimate_impersonating")

for bootloader in "${bootloaders[@]}"; do
    cp contracts/system-contracts/bootloader/build/artifacts/$bootloader.yul.zbin $DST_DIR
done
