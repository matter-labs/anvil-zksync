#!/usr/bin/env bash

ANVIL_URL=http://localhost:8545
ANVIL_STATE_PAYLOAD_PATH="${BASH_SOURCE%/*}/state/l1-state-payload.txt"
WALLETS_PATH="${BASH_SOURCE%/*}/etc/env/file_based/wallets.yaml"
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

if command -v yq >/dev/null 2>&1
then
  echo "Reading wallets from $WALLETS_PATH"
  DEPLOYER_ACCOUNT="$(yq -r '.deployer.address' "$WALLETS_PATH")"
  BLOB_OPERATOR_ACCOUNT="$(yq -r '.blob_operator.address' "$WALLETS_PATH")"
  GOVERNOR_ACCOUNT="$(yq -r '.governor.address' "$WALLETS_PATH")"
else
  echo "Command 'yq' could not be found. Falling back to user-provided environment variables"
  if [ -z "${DEPLOYER_ACCOUNT}" ] || [ -z "$BLOB_OPERATOR_ACCOUNT" ] || [ -z "$GOVERNOR_ACCOUNT" ]; then
    echo "ERR: One of environment variables was not set: \$DEPLOYER_ACCOUNT \$BLOB_OPERATOR_ACCOUNT \$GOVERNOR_ACCOUNT"
    echo "ERR: Consider installing 'yq' for this script to derive them automatically"
    exit 1
  fi
fi

fund_account "$DEPLOYER_ACCOUNT"
fund_account "$BLOB_OPERATOR_ACCOUNT"
fund_account "$GOVERNOR_ACCOUNT"

zkstack ecosystem init --dev

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

echo "Done!"
