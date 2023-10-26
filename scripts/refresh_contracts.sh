#!/bin/bash
set -xe

SRC_DIR=etc/system-contracts/artifacts-zk/cache-zk/solpp-generated-contracts
DST_DIR=src/deps/contracts/

mkdir -p $DST_DIR

contracts=("AccountCodeStorage" "BootloaderUtilities" "Compressor" "ComplexUpgrader" "ContractDeployer" "DefaultAccount" "EmptyContract" "ImmutableSimulator" "KnownCodesStorage" "L1Messenger" "L2EthToken" "MsgValueSimulator" "NonceHolder" "SystemContext" )

for contract in "${contracts[@]}"; do
    cp $SRC_DIR/$contract.sol/$contract.json $DST_DIR
done

precompiles=("Ecrecover" "Keccak256" "SHA256")

for precompile in "${precompiles[@]}"; do
    cp etc/system-contracts/contracts/precompiles/artifacts/$precompile.yul/$precompile.yul.zbin $DST_DIR
done

cp etc/system-contracts/contracts/artifacts/EventWriter.yul/EventWriter.yul.zbin $DST_DIR


bootloaders=("fee_estimate"  "gas_test" "playground_batch" "proved_batch" "proved_batch_impersonating" "fee_estimate_impersonating" )

for bootloader in "${bootloaders[@]}"; do
    cp etc/system-contracts/bootloader/build/artifacts/$bootloader.yul/$bootloader.yul.zbin $DST_DIR
done
