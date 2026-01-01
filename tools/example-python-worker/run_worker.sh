#!/usr/bin/env bash
# Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
# software: you can redistribute it and/or modify it under the terms of the GNU
# General Public License as published by the Free Software Foundation, version
# 3.
#
# This program is distributed in the hope that it will be useful, but WITHOUT
# ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
# FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License along with
# this program. If not, see <https://www.gnu.org/licenses/>.
#

# Change to the script's directory
cd "$(dirname "$0")"

# Activate virtual environment if it exists
if [ -d "venv" ]; then
    source venv/bin/activate
fi

# Add moor_schema to PYTHONPATH so FlatBuffers imports work
export PYTHONPATH="${PWD}:${PYTHONPATH}"

# Default to IPC sockets (no CURVE encryption needed)
REQUEST_ADDR="${WORKER_REQUEST_ADDR:-ipc:///tmp/moor_workers_request.sock}"
RESPONSE_ADDR="${WORKER_RESPONSE_ADDR:-ipc:///tmp/moor_workers_response.sock}"
ENROLLMENT_ADDR="${WORKER_ENROLLMENT_ADDR:-tcp://localhost:7900}"
DATA_DIR="${WORKER_DATA_DIR:-./.moor-worker-data}"

# Build command
CMD="python3 -u echo_worker.py"
CMD="$CMD --request-address $REQUEST_ADDR"
CMD="$CMD --response-address $RESPONSE_ADDR"
CMD="$CMD --enrollment-address $ENROLLMENT_ADDR"
CMD="$CMD --data-dir $DATA_DIR"

# Add enrollment token file if specified
if [ -n "$WORKER_ENROLLMENT_TOKEN_FILE" ]; then
    CMD="$CMD --enrollment-token-file $WORKER_ENROLLMENT_TOKEN_FILE"
fi

# Run the worker
exec $CMD
