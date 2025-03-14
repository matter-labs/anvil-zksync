#!/usr/bin/env bash

set -e

ANVIL_URL=http://localhost:8545
ANVIL_STATE_PAYLOAD_PATH="${BASH_SOURCE%/*}/state/l1-state-payload.txt"
WALLETS_PATH="${BASH_SOURCE%/*}/wallets.yaml"
# ~75k ETH
DEFAULT_FUND_AMOUNT=0x10000000000000000000

fund_account () {
  local payload='{"method":"anvil_setBalance","params":['\""$1"\"', '\""$DEFAULT_FUND_AMOUNT"\"'],"id":1,"jsonrpc":"2.0"}'
  local output
  output="$(
    curl $ANVIL_URL \
      -s \
      -X POST \
      -H "Content-Type: application/json" \
      --data "$payload"
  )"
  local error
  error="$(echo "$output" | jq '.error')"
  if [[ $error != null ]]; then
    echo "Failed to fund $1 with $DEFAULT_FUND_AMOUNT ETH (error=$error)"
    exit 1
  fi

  echo "Funded $1 with $DEFAULT_FUND_AMOUNT ETH (output=$output)"
}

if ! command -v yq >/dev/null 2>&1
then
  echo "Command 'yq' could not be found. Please follow https://github.com/mikefarah/yq to install."
  exit 1
fi

echo "* Reading chain accounts from $WALLETS_PATH..."
DEPLOYER_ACCOUNT="$(yq -r '.deployer.address' "$WALLETS_PATH")"
BLOB_OPERATOR_ACCOUNT="$(yq -r '.blob_operator.address' "$WALLETS_PATH")"
GOVERNOR_ACCOUNT="$(yq -r '.governor.address' "$WALLETS_PATH")"
echo "* Done (deployer='$DEPLOYER_ACCOUNT', blob_operator='$BLOB_OPERATOR_ACCOUNT', governor='$GOVERNOR_ACCOUNT')"

echo "* Funding accounts..."
fund_account "$DEPLOYER_ACCOUNT"
fund_account "$BLOB_OPERATOR_ACCOUNT"
fund_account "$GOVERNOR_ACCOUNT"
echo "* Done"

# Create a new ecosystem in `./anvil_ecosystem` with cloned `zksync-era`
zkstack ecosystem create \
  --ecosystem-name anvil-ecosystem --l1-network localhost --chain-name anvil --chain-id 260 \
  --wallet-creation in-file --wallet-path ../wallets.yaml \
  --prover-mode no-proofs --l1-batch-commit-data-generator-mode rollup --evm-emulator false --start-containers false \
  --base-token-address "0x0000000000000000000000000000000000000001"  --base-token-price-nominator 1 --base-token-price-denominator 1 \
  --link-to-code "" # Indicates that `zksync-era` needs to be cloned

echo "* Patching ecosystem..."
# Substitute zksync-era's contracts with anvil-zksync's version
rm -rf ./anvil_ecosystem/zksync-era/contracts
ln -s ../../../contracts  ./anvil_ecosystem/zksync-era/contracts
# Patch `genesis.yaml` with anvil-zksync's parameters
patched_genesis=$(yq '. * load("patches/genesis-patch.yaml")' ./anvil_ecosystem/zksync-era/etc/env/file_based/genesis.yaml)
echo "$patched_genesis" > ./anvil_ecosystem/zksync-era/etc/env/file_based/genesis.yaml
echo "* Done"

# Initialize ecosystem
# Note: this requires a running postgres container and builds zksync-era server; neither is needed for us but there is
#       no current way to turn off this behavior in zkstack
pushd ./anvil_ecosystem > /dev/null
zkstack ecosystem init \
  --deploy-paymaster --deploy-erc20 --deploy-ecosystem \
  --server-db-url=postgres://postgres:notsecurepassword@localhost:5432 --server-db-name=zksync_server_localhost_era \
  --l1-rpc-url=http://localhost:8545 --observability false --update-submodules false
popd > /dev/null

# Persist resulting configuration files in anvil-zksync's source control
echo "* Copying resulting config files..."
cp ./anvil_ecosystem/chains/anvil/configs/{contracts,genesis}.yaml ./configs/
echo "* Done"

echo "* Dumping mandatory upgrade transaction..."
psql postgres://postgres:notsecurepassword@localhost:5432/zksync_server_localhost_era -t -c \
  "select jsonb_agg(transactions) from transactions join protocol_versions on transactions.hash = protocol_versions.upgrade_tx_hash;" \
  | jq -c '.[0]' | sed -e 's/\\\\x/0x/g' > ./state/upgrade_tx.json
echo "* Done"

echo "* Dumping anvil's state as payload..."
anvil_state="$(
  curl $ANVIL_URL \
    -s \
    -X POST \
    -H "Content-Type: application/json" \
    --data '{"method":"anvil_dumpState","params":[],"id":1,"jsonrpc":"2.0"}'
)"
anvil_state_error="$(echo "$anvil_state" | jq '.error')"
if [[ $anvil_state_error != null ]]; then
  echo "Failed to dump anvil's state (error=$anvil_state_error)"
  exit 1
fi
echo "$anvil_state" | jq -r '.result' > "$ANVIL_STATE_PAYLOAD_PATH"
echo "* Done"

echo "Done!"
