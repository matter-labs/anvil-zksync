#!/bin/bash
set -e

# Check if the node is running
MAX_RETRIES=10
COUNTER=0
URL="http://localhost:8011"

# Payload
DATA='{
    "jsonrpc": "2.0",
    "id": "1",
    "method": "eth_chainId",
    "params": []
}'

while [ $COUNTER -lt $MAX_RETRIES ]; do
    # Send eth_chainId request
    RESPONSE=$(curl -s -o /dev/null -w "%{http_code}" -X POST -H "content-type: application/json" -d "$DATA" $URL || true)

    # Check if the request was successful
    if [ "$RESPONSE" -eq 200 ]; then
        echo "Node is running! Starting tests..."
        break
    else
        echo "Node not ready, retrying in 1 second..."
        let COUNTER=COUNTER+1
        sleep 1
    fi
done

if [ $COUNTER -eq $MAX_RETRIES ]; then
    echo "Failed to contact node after $MAX_RETRIES attempts. Are you sure the node is running at $URL ?"
    exit 1
fi

cd e2e-tests

# Install dependencies
echo ""
echo "============"
echo "Yarn install"
echo "============"
yarn install --frozen-lockfile

# Compile contracts
echo ""
echo "==================="
echo "Compiling contracts"
echo "==================="
yarn hardhat compile

# Run tests
echo ""
echo "================="
echo "Running e2e tests"
echo "================="
yarn test
