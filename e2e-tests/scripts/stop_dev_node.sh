#!/bin/bash

# Search for processes named 'era_test_node' owned by the current user
PIDS=$(pgrep era_test_node)

# Check if any processes were found
if [[ -z $PIDS ]]; then
    echo "There are no processes to kill."
    exit 0
else
    for PID in $PIDS; do
        kill $PID
        if [[ $? -eq 0 ]]; then
            echo "Killed process with process id $PID."
        else
            echo "Failed to kill process with process id $PID."
        fi
    done
fi

exit 0
