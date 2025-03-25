#!/bin/bash
set -xe

BUILTIN_CONTRACTS_OUTPUT_PATH=crates/core/src/deps/contracts/builtin-contracts-v27.tar.gz

# Prepare `tar` command that will generate the output archive
cmd="tar -czvf $BUILTIN_CONTRACTS_OUTPUT_PATH"

# Forge JSON artifacts to be packed in the archive
L1_ARTIFACTS_SRC_DIR=contracts/l1-contracts/zkout
L2_ARTIFACTS_SRC_DIR=contracts/l2-contracts/zkout
SYSTEM_ARTIFACTS_SRC_DIR=contracts/system-contracts/zkout

l1_artifacts=("MessageRoot" "Bridgehub" "L2AssetRouter" "L2NativeTokenVault" "L2WrappedBaseToken")
l2_artifacts=("TimestampAsserter")
system_contracts_sol=(
  "AccountCodeStorage" "BootloaderUtilities" "Compressor" "ComplexUpgrader" "ContractDeployer" "DefaultAccount"
  "DefaultAccountNoSecurity" "EmptyContract" "ImmutableSimulator" "KnownCodesStorage" "L1Messenger" "L2BaseToken"
  "MsgValueSimulator" "NonceHolder" "SystemContext" "PubdataChunkPublisher" "Create2Factory" "L2GenesisUpgrade"
  "SloadContract"
)
system_contracts_yul=("EventWriter")
precompiles=("EcAdd" "EcMul" "Ecrecover" "Keccak256" "SHA256" "EcPairing" "CodeOracle" "P256Verify")
bootloaders=(
  "fee_estimate" "gas_test" "playground_batch" "proved_batch" "proved_batch_impersonating" "fee_estimate_impersonating"
)

for artifact in "${l1_artifacts[@]}"; do
  cmd="$cmd $L1_ARTIFACTS_SRC_DIR/$artifact.sol/$artifact.json"
done

for artifact in "${l2_artifacts[@]}"; do
  cmd="$cmd $L2_ARTIFACTS_SRC_DIR/$artifact.sol/$artifact.json"
done

for artifact in "${system_contracts_sol[@]}"; do
  cmd="$cmd $SYSTEM_ARTIFACTS_SRC_DIR/$artifact.sol/$artifact.json"
done

for artifact in "${system_contracts_yul[@]}"; do
  cmd="$cmd $SYSTEM_ARTIFACTS_SRC_DIR/$artifact.yul/$artifact.json"
done

for precompile in "${precompiles[@]}"; do
  cmd="$cmd $SYSTEM_ARTIFACTS_SRC_DIR/$precompile.yul/$precompile.json"
done

for bootloader in "${bootloaders[@]}"; do
  cmd="$cmd $SYSTEM_ARTIFACTS_SRC_DIR/$bootloader.yul/$bootloader.json"
done

eval "$cmd"
