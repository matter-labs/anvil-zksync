#!/bin/bash

##############################################################################
# Script Name   : start_dev_node.sh
# Description   : This script starts the 'era_test_node' binary in development 
#                 mode. It ensures that only one instance is running at any time, 
#                 checks if the binary exists, and confirms that the node is 
#                 accepting requests by sending an 'eth_chainId' RPC call.
#                 Output logs of the node are piped to 'era_test_node_output.log'.
#
# Usage         : yarn dev:start
# Requirements  :
#                 - The 'curl' tool must be available on the system.
#                 - The node should be accessible at "http://localhost:8011".
##############################################################################


# Check if the node is already running
EXISTING_PID=$(pgrep -f "era_test_node run")

if [[ ! -z $EXISTING_PID ]]; then
    echo "Node is already running with PID $EXISTING_PID."
    exit 0
fi

BIN=target/release/era_test_node 

# Check if built, throw if not
if [[ ! -f $BIN ]]; then
    BIN="$(cd "$(dirname "$BIN")"; pwd)/$(basename "$BIN")"
    echo "Error: Binary does not exist at $BIN"
    echo "Please build node with 'cargo build --release' first"
    exit 1
fi

# Run Node in Dev Mode in background and pipe logs to file
$BIN run > era_test_node_output.log 2>&1 &

NODE_PID=$!

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
    sleep 1
    # Send eth_chainId request
    RESPONSE=$(curl -s -o /dev/null -w "%{http_code}" -X POST -H "content-type: application/json" -d "$DATA" $URL || true)

    # Check if the request was successful
    if [ "$RESPONSE" -eq 200 ]; then
        echo "Node is running and accepting requests! "
        break
    else
        echo "Node not ready, retrying in 1 second..."
        let COUNTER=COUNTER+1
    fi
done

if [ $COUNTER -eq $MAX_RETRIES ]; then
    echo "Failed to contact node after $MAX_RETRIES attempts ❌"
    echo "Are you sure the node is running at $URL ❓️"
    exit 1
fi

echo "Node launched successfully, PID: $NODE_PID ✅"
exit 0