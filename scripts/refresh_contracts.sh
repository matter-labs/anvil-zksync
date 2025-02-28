#!/bin/bash
set -xe

# Copy Solidity source files `<contract_name>.sol` from SRC to DST
L1_SOL_SRC_DIR=contracts/l1-contracts/contracts
L1_SOL_DST_DIR=crates/l1_sidecar/src/contracts/sol

mkdir -p $L1_SOL_DST_DIR

l1_sol_contracts=("state-transition/chain-interfaces/IExecutor")

for path in "${l1_sol_contracts[@]}"; do
  directory=$(dirname "$path")
  contract=$(basename "$path")
  cp "$L1_SOL_SRC_DIR/$directory/$contract.sol" $L1_SOL_DST_DIR
done

# Copy Sol-based JSON artifacts `<contract_name>.json` for L1 contracts
L1_JSON_ABI_SRC_DIR=contracts/l1-contracts/zkout
L1_JSON_ABI_DST_DIR=crates/l1_sidecar/src/contracts/artifacts

mkdir -p $L1_JSON_ABI_DST_DIR

l1_json_contracts=("IZKChain")

for path in "${l1_json_contracts[@]}"; do
  directory=$(dirname "$path")
  contract=$(basename "$path")
  cp "$L1_JSON_ABI_SRC_DIR/$directory/$contract.sol/$contract.json" $L1_JSON_ABI_DST_DIR
done

# Copy Sol-based JSON artifacts `<contract_name>.json` for L2 contracts that are identical to their L1 counterparts from SRC to DST
L1_L2_JSON_ABI_SRC_DIR=contracts/l1-contracts/zkout
L1_L2_JSON_ABI_DST_DIR=crates/core/src/deps/contracts

mkdir -p $L1_L2_JSON_ABI_DST_DIR

l1_l2_contracts=("MessageRoot" "Bridgehub" "L2AssetRouter" "L2NativeTokenVault" "L2WrappedBaseToken")

for path in "${l1_l2_contracts[@]}"; do
  directory=$(dirname "$path")
  contract=$(basename "$path")
  cp "$L1_L2_JSON_ABI_SRC_DIR/$directory/$contract.sol/$contract.json" $L1_L2_JSON_ABI_DST_DIR
done


# Copy Sol-based JSON artifacts `<contract_name>.json` from SRC to DST
SYSTEM_JSON_ABI_SRC_DIR=contracts/system-contracts/zkout
SYSTEM_JSON_ABI_DST_DIR=crates/core/src/deps/contracts
L2_JSON_ABI_SRC_DIR=contracts/l2-contracts/zkout
L2_JSON_ABI_DST_DIR=crates/core/src/deps/contracts

mkdir -p $SYSTEM_JSON_ABI_DST_DIR
mkdir -p $L2_JSON_ABI_DST_DIR

system_contracts=("AccountCodeStorage" "BootloaderUtilities" "Compressor" "ComplexUpgrader" "ContractDeployer" "DefaultAccount" "DefaultAccountNoSecurity" "EmptyContract" "ImmutableSimulator" "KnownCodesStorage" "L1Messenger" "L2BaseToken" "MsgValueSimulator" "NonceHolder" "SystemContext" "PubdataChunkPublisher" "Create2Factory" "L2GenesisUpgrade" "SloadContract")

for path in "${system_contracts[@]}"; do
  directory=$(dirname "$path")
  contract=$(basename "$path")
  cp "$SYSTEM_JSON_ABI_SRC_DIR/$directory/$contract.sol/$contract.json" $SYSTEM_JSON_ABI_DST_DIR
done

l2_contracts=("TimestampAsserter")

for path in "${l2_contracts[@]}"; do
  directory=$(dirname "$path")
  contract=$(basename "$path")
  cp "$L2_JSON_ABI_SRC_DIR/$directory/$contract.sol/$contract.json" $L2_JSON_ABI_DST_DIR
done


# Copy Yul-based JSON artifacts `<contract_name>.json` from SRC to DST
SYSTEM_ARTIFACT_SRC_DIR=contracts/system-contracts/zkout
SYSTEM_ARTIFACT_DST_DIR=crates/core/src/deps/contracts
PRECOMPILE_ARTIFACT_SRC_DIR=contracts/system-contracts/zkout
PRECOMPILE_ARTIFACT_DST_DIR=crates/core/src/deps/contracts
BOOTLOADER_ARTIFACT_SRC_DIR=contracts/system-contracts/zkout
BOOTLOADER_ARTIFACT_DST_DIR=crates/core/src/deps/contracts

mkdir -p $SYSTEM_ARTIFACT_DST_DIR
mkdir -p $PRECOMPILE_ARTIFACT_DST_DIR
mkdir -p $BOOTLOADER_ARTIFACT_DST_DIR

system_artifacts=("EventWriter")

for system_artifact in "${system_artifacts[@]}"; do
  cp "$SYSTEM_ARTIFACT_SRC_DIR/$system_artifact.yul/$system_artifact.json" $SYSTEM_ARTIFACT_DST_DIR
done

precompiles=("EcAdd" "EcMul" "Ecrecover" "Keccak256" "SHA256" "EcPairing" "CodeOracle" "P256Verify")

for precompile in "${precompiles[@]}"; do
  cp "$PRECOMPILE_ARTIFACT_SRC_DIR/$precompile.yul/$precompile.json" $PRECOMPILE_ARTIFACT_DST_DIR
done

bootloaders=("fee_estimate"  "gas_test" "playground_batch" "proved_batch" "proved_batch_impersonating" "fee_estimate_impersonating")

for bootloader in "${bootloaders[@]}"; do
  cp "$BOOTLOADER_ARTIFACT_SRC_DIR/$bootloader.yul/$bootloader.json" $BOOTLOADER_ARTIFACT_DST_DIR
done
