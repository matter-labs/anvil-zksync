#!/bin/bash

##############################################################################
# Script Name   : stop_dev_node.sh
# Description   : This script searches and terminates all processes named
#                 'era_test_node' owned by the current user. It provides feedback 
#                 on whether each process was successfully killed or not.
#
# Usage         : yarn dev:kill
##############################################################################

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
