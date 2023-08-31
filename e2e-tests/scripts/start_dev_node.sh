#!/bin/bash

## Run this script via "yarn dev:start" in the e2e-tests directory

# Check if the node is already running
EXISTING_PID=$(pgrep -f "era_test_node run")

if [[ ! -z $EXISTING_PID ]]; then
    echo "Node is already running with PID $EXISTING_PID."
    exit 0
fi

BIN=../target/release/era_test_node 

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

# Wait for node to start up, check for "Node is ready" in logs
while true; do
    if grep -q "Node is ready" era_test_node_output.log; then
        break
    fi

    sleep 1
done

echo "Node started successfully with PID $NODE_PID and is ready."
exit 0