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
        # TODO: Remove this, just debugging for now
        RESPONSE=$(curl -s -X POST -H "content-type: application/json" -d "$DATA" $URL || true)
        echo $RESPONSE
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
# TODO: Remove this, just debugging for now
RESPONSE=$(curl --request POST --url http://localhost:8011/ --header 'content-type: application/json' --data '{  "jsonrpc": "2.0",    "id": "2",    "method": "eth_call",    "params": [{        "to": "0x36615Cf349d7F6344891B1e7CA7C72883F5dc049",        "data": "0x0000",        "from": "0xa61464658AfeAf65CccaaFD3a512b69A83B77618",        "gas": "0x0000",        "gasPrice": "0x0000",        "value": "0x0000",        "nonce": "0x0000"    }, "latest"]}')
echo $RESPONSE

yarn test
