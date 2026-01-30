#!/bin/bash
# Start mooR with the LambdaCore core
set -e

# Configuration
export RUN_DIR="run-lambda-moor"
export IMPORT_PATH="/db/cores/lambda-moor/src"
export BUILD_PROFILE="release-fast"

# Parse arguments
while [[ "$#" -gt 0 ]]; do
    case $1 in
        --debug) export BUILD_PROFILE="debug"; shift ;;
        --release) export BUILD_PROFILE="release-fast"; shift ;;
        *) echo "Unknown parameter: $1"; exit 1 ;;
    esac
done

# Check for root-owned directory
if [ -d "$RUN_DIR" ] && [ "$(stat -c '%u' "$RUN_DIR" 2>/dev/null)" == "0" ]; then
    echo "Error: $RUN_DIR is owned by root. Run: sudo chown -R $(id -u):$(id -g) $RUN_DIR"
    exit 1
fi

echo "Starting mooR with LambdaCore..."
echo "Build profile: $BUILD_PROFILE"
echo "Runtime directory: $RUN_DIR"

# Ensure runtime directories exist and IPC is clean
mkdir -p "$RUN_DIR/ipc" "$RUN_DIR/config" "$RUN_DIR/moor-data" "$RUN_DIR/export"
rm -f "$RUN_DIR/ipc"/*.sock

# Ensure meadow is fetched and dependencies installed
MEADOW_DIR="${MEADOW_PATH:-clients/meadow}"
if [ ! -d "$MEADOW_DIR" ]; then
    echo "Meadow web client not found, fetching..."
    ./clients/fetch-meadow.sh
fi
if [ ! -d "$MEADOW_DIR/node_modules" ]; then
    echo "Installing meadow dependencies..."
    (cd "$MEADOW_DIR" && npm install)
fi

export USER_ID=$(id -u)
export GROUP_ID=$(id -g)

# Core-specific features (Classic compatibility)
export USE_BOOLEAN_RETURNS=false
export CUSTOM_ERRORS=false
export USE_UUOBJIDS=false
export ANONYMOUS_OBJECTS=false
export ENABLE_EVENTLOG=true

docker compose up --build
