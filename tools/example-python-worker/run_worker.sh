#!/usr/bin/env bash
# Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
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
