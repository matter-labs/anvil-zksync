#!/usr/bin/env bash

set -e

ANVIL_URL=http://localhost:8545
WALLETS_PATH="${BASH_SOURCE%/*}/wallets.yaml"
# ~75k ETH
DEFAULT_FUND_AMOUNT=0x10000000000000000000

PROTOCOL_VERSION=${1:-v30}
case $PROTOCOL_VERSION in
  v26)
    # HEAD of anvil-zksync-0.4.x-release-v26
    ERA_CONTRACTS_GIT_COMMIT=50dc0669213366f5d3084a7a29a83541cf3c6435
    ERA_TAG=core-v26.8.1
    ;;
  v27)
    # HEAD of anvil-zksync-0.4.x-release-v27
    ERA_CONTRACTS_GIT_COMMIT=76259394f7361897d67a7d21d55b86167140b39d
    ERA_TAG=core-v27.0.0
    ;;
  v28)
    # HEAD of anvil-zksync-0.4.x-release-v28
    ERA_CONTRACTS_GIT_COMMIT=59374a3a6cd0cf323cdb5ec0caa22396432ee9db
    # HEAD of draft-v28
    ERA_TAG=core-v28.0.0
    ;;
  v29)
    # HEAD of anvil-zksync-0.6.x-draft-v29
    ERA_CONTRACTS_GIT_COMMIT=db63f0257db849aa8517101633a23cf530fe34d3
    # HEAD of draft-v29
    ERA_TAG=core-v29.0.0
    ;;
  v30)
    # TODO: update commit hash to HEAD of anvil-zksync-0.6.x-draft-v30
    # Currently https://github.com/matter-labs/era-contracts/commit/7d9275f48be1e31c26186b8149db28fbd63bb43c ?
    ERA_CONTRACTS_GIT_COMMIT=7d9275f48be1e31c26186b8149db28fbd63bb43c
    # HEAD of draft-v30
    # TODO: update to core-v30.0.0 when it is released
    ERA_TAG=kl/medium-interop-support
    ;;
  *)
    echo "Unrecognized/unsupported protocol version: $PROTOCOL_VERSION"
    exit 1
    ;;
esac

# Checkout the right revision of contracts
cd ../contracts
echo "Using era-contracts commit: $ERA_CONTRACTS_GIT_COMMIT"
git fetch && git checkout $ERA_CONTRACTS_GIT_COMMIT
cd ../l1-setup

# Kill all child processes on exit
trap 'pkill --signal SIGTERM --parent $$' EXIT

# Spawn anvil in background
anvil --port 8545  --dump-state "state/$PROTOCOL_VERSION-l1-state.json" &
ANVIL_STATE_PAYLOAD_PATH="${BASH_SOURCE%/*}/state/$PROTOCOL_VERSION-l1-state-payload.txt"
ANVIL_UPGRADE_TX_PATH="${BASH_SOURCE%/*}/state/$PROTOCOL_VERSION-l2-upgrade-tx.json"

fund_account () {
  local payload='{"method":"anvil_setBalance","params":['\""$1"\"', '\""$DEFAULT_FUND_AMOUNT"\"'],"id":1,"jsonrpc":"2.0"}'
  local output
  output="$(
    curl $ANVIL_URL \
      --silent --show-error \
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

echo "* Waiting for anvil to start up..."
while true
do
  EXIT_CODE=0
  curl $ANVIL_URL \
    --silent --show-error \
    -X POST \
    -H "Content-Type: application/json" \
    --data '{"method":"eth_accounts","params":[],"id":1,"jsonrpc":"2.0"}' || EXIT_CODE=$?
  if [[ $EXIT_CODE == 0 ]]; then
    break
  fi
  sleep 1
done

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

# Create a new ecosystem in `./anvil_ecosystem_<protocol_version>` with cloned `zksync-era`
zkstack ecosystem create \
  --ecosystem-name "anvil-ecosystem-$PROTOCOL_VERSION" --l1-network localhost --chain-name "anvil-$PROTOCOL_VERSION" \
  --chain-id 260 --wallet-creation in-file --wallet-path ../wallets.yaml \
  --prover-mode no-proofs --l1-batch-commit-data-generator-mode rollup --evm-emulator false --start-containers false \
  --base-token-address "0x0000000000000000000000000000000000000001"  --base-token-price-nominator 1 --base-token-price-denominator 1 \
  --link-to-code "" # Indicates that `zksync-era` needs to be cloned

ECOSYSTEM_DIR="anvil_ecosystem_$PROTOCOL_VERSION"
ECOSYSTEM_ERA_DIR="$ECOSYSTEM_DIR/zksync-era"

echo "* Patching ecosystem..."
# Checkout correct version of zksync-era
pushd "./$ECOSYSTEM_ERA_DIR" > /dev/null
echo "Using zksync-era tag: $ERA_TAG"
git fetch && git checkout --force $ERA_TAG
popd > /dev/null
# Substitute zksync-era's contracts with anvil-zksync's version
rm -rf "./$ECOSYSTEM_ERA_DIR/contracts"
ln -s ../../../contracts  "./$ECOSYSTEM_ERA_DIR/contracts"
# Patch `genesis.yaml` with anvil-zksync's parameters
patched_genesis=$(yq ". * load(\"patches/$PROTOCOL_VERSION-genesis-patch.yaml\")" "./$ECOSYSTEM_ERA_DIR/etc/env/file_based/genesis.yaml")
echo "$patched_genesis" > "./$ECOSYSTEM_ERA_DIR/etc/env/file_based/genesis.yaml"
echo "* Done"

# Initialize ecosystem
# Note: this requires a running postgres container and builds zksync-era server; neither is needed for us but there is
#       no current way to turn off this behavior in zkstack
pushd "./$ECOSYSTEM_DIR" > /dev/null
zkstack ecosystem init \
  --deploy-paymaster --deploy-erc20 --deploy-ecosystem \
  --server-db-url=postgres://postgres:notsecurepassword@localhost:5432 --server-db-name=zksync_server_localhost_era \
  --l1-rpc-url=http://localhost:8545 --observability false --update-submodules false
popd > /dev/null

GENESIS_PROTOCOL_VERSION="$(yq -r '.genesis_protocol_version' "$ECOSYSTEM_DIR/chains/anvil_$PROTOCOL_VERSION/configs/genesis.yaml")"
if [[ "v$GENESIS_PROTOCOL_VERSION" != "$PROTOCOL_VERSION" ]]; then
  echo "ERROR: Ecosystem was initialized with protocol version v$GENESIS_PROTOCOL_VERSION when we expected $PROTOCOL_VERSION"
  echo "Make sure you are using correct version of zkstack and clean ecosystem directory ($ECOSYSTEM_DIR)"
  exit 1
fi

# Persist resulting configuration files in anvil-zksync's source control
echo "* Copying resulting config files..."
cp "./$ECOSYSTEM_DIR/chains/anvil_$PROTOCOL_VERSION/configs/contracts.yaml" "./configs/$PROTOCOL_VERSION-contracts.yaml"
cp "./$ECOSYSTEM_DIR/chains/anvil_$PROTOCOL_VERSION/configs/genesis.yaml" "./configs/$PROTOCOL_VERSION-genesis.yaml"
echo "* Done"

echo "* Dumping mandatory upgrade transaction..."
psql postgres://postgres:notsecurepassword@localhost:5432/zksync_server_localhost_era -t -c \
  "select jsonb_agg(transactions) from transactions join protocol_versions on transactions.hash = protocol_versions.upgrade_tx_hash;" \
  | jq -c '.[0]' | sed -e 's/\\\\x/0x/g' > "$ANVIL_UPGRADE_TX_PATH"
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
