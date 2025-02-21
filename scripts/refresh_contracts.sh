#!/bin/bash
set -xe

# Copy Solidity source files `<contract_name>.sol` from SRC to DST
L1_SOL_SRC_DIR=contracts/l1-contracts/contracts
L1_SOL_DST_DIR=crates/l1_sidecar/src/contracts/sol

mkdir -p $L1_SOL_DST_DIR

l1_contracts=("state-transition/chain-interfaces/IExecutor")

for path in "${l1_contracts[@]}"; do
  directory=$(dirname "$path")
  contract=$(basename "$path")
  cp "$L1_SOL_SRC_DIR/$directory/$contract.sol" $L1_SOL_DST_DIR
done


# Copy JSON ABI files `<contract_name>.json` from SRC to DST
SYSTEM_JSON_ABI_SRC_DIR=contracts/system-contracts/artifacts-zk/contracts-preprocessed
SYSTEM_JSON_ABI_DST_DIR=crates/core/src/deps/contracts
L2_JSON_ABI_SRC_DIR=contracts/l2-contracts/artifacts-zk/contracts
L2_JSON_ABI_DST_DIR=crates/core/src/deps/contracts

mkdir -p $SYSTEM_JSON_ABI_DST_DIR
mkdir -p $L2_JSON_ABI_DST_DIR

system_contracts=("AccountCodeStorage" "BootloaderUtilities" "Compressor" "ComplexUpgrader" "ContractDeployer" "DefaultAccount" "DefaultAccountNoSecurity" "EmptyContract" "ImmutableSimulator" "KnownCodesStorage" "L1Messenger" "L2BaseToken" "MsgValueSimulator" "NonceHolder" "SystemContext" "PubdataChunkPublisher" "Create2Factory")

for path in "${system_contracts[@]}"; do
  directory=$(dirname "$path")
  contract=$(basename "$path")
  cp "$SYSTEM_JSON_ABI_SRC_DIR/$contract.sol/$contract.json" $SYSTEM_JSON_ABI_DST_DIR
done

l2_contracts=("dev-contracts/TimestampAsserter")

for path in "${l2_contracts[@]}"; do
  directory=$(dirname "$path")
  contract=$(basename "$path")
  cp "$L2_JSON_ABI_SRC_DIR/$directory/$contract.sol/$contract.json" $L2_JSON_ABI_DST_DIR
done


# Copy Yul artifact files `<contract_name>.yul.zbin` from SRC to DST
SYSTEM_ARTIFACT_SRC_DIR=contracts/system-contracts/contracts-preprocessed/artifacts
SYSTEM_ARTIFACT_DST_DIR=crates/core/src/deps/contracts
PRECOMPILE_ARTIFACT_SRC_DIR=contracts/system-contracts/contracts-preprocessed/precompiles/artifacts
PRECOMPILE_ARTIFACT_DST_DIR=crates/core/src/deps/contracts
BOOTLOADER_ARTIFACT_SRC_DIR=contracts/system-contracts/bootloader/build/artifacts
BOOTLOADER_ARTIFACT_DST_DIR=crates/core/src/deps/contracts

mkdir -p $SYSTEM_ARTIFACT_DST_DIR
mkdir -p $PRECOMPILE_ARTIFACT_DST_DIR
mkdir -p $BOOTLOADER_ARTIFACT_DST_DIR

system_artifacts=("EventWriter")

for system_artifact in "${system_artifacts[@]}"; do
  cp "$SYSTEM_ARTIFACT_SRC_DIR/$system_artifact.yul.zbin" $SYSTEM_ARTIFACT_DST_DIR
done

precompiles=("EcAdd" "EcMul" "Ecrecover" "Keccak256" "SHA256" "EcPairing" "CodeOracle" "P256Verify" "Modexp")

for precompile in "${precompiles[@]}"; do
  cp "$PRECOMPILE_ARTIFACT_SRC_DIR/$precompile.yul.zbin" $PRECOMPILE_ARTIFACT_DST_DIR
done

bootloaders=("fee_estimate"  "gas_test" "playground_batch" "proved_batch" "proved_batch_impersonating" "fee_estimate_impersonating")

for bootloader in "${bootloaders[@]}"; do
  cp "$BOOTLOADER_ARTIFACT_SRC_DIR/$bootloader.yul.zbin" $BOOTLOADER_ARTIFACT_DST_DIR
done
