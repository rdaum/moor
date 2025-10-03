#!/usr/bin/env bash
# Script to run the Python echo worker with proper venv activation

# Change to the script's directory
cd "$(dirname "$0")"

# Activate virtual environment
source venv/bin/activate

# Add moor_schema to PYTHONPATH so FlatBuffers imports work
export PYTHONPATH="${PWD}/moor_schema:${PYTHONPATH}"

# Default to IPC sockets
REQUEST_ADDR="${WORKER_REQUEST_ADDR:-ipc:///tmp/moor_workers_request.sock}"
RESPONSE_ADDR="${WORKER_RESPONSE_ADDR:-ipc:///tmp/moor_workers_response.sock}"
PUBLIC_KEY="${WORKER_PUBLIC_KEY:-../../moor-verifying-key.pem}"
PRIVATE_KEY="${WORKER_PRIVATE_KEY:-../../moor-signing-key.pem}"

# Run the worker
exec python3 -u echo_worker.py \
    --public-key "$PUBLIC_KEY" \
    --private-key "$PRIVATE_KEY" \
    --request-address "$REQUEST_ADDR" \
    --response-address "$RESPONSE_ADDR"
