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
