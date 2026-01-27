#!/bin/bash
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

# Start mooR with the Cowbell core
set -e

# Configuration
export RUN_DIR="run-cowbell"
export IMPORT_PATH="/db/cores/cowbell/src"
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

echo "Starting mooR with Cowbell core..."
echo "Build profile: $BUILD_PROFILE"
echo "Runtime directory: $RUN_DIR"

# Ensure runtime directories exist and IPC is clean
mkdir -p "$RUN_DIR/ipc" "$RUN_DIR/config" "$RUN_DIR/moor-data" "$RUN_DIR/export"
rm -f "$RUN_DIR/ipc"/*.sock

# Ensure cowbell is fetched
if [ ! -d "cores/cowbell/src" ]; then
    echo "Cowbell core not found, fetching..."
    ./cores/fetch-cowbell.sh
fi

export USER_ID=$(id -u)
export GROUP_ID=$(id -g)

# Core-specific features
export USE_BOOLEAN_RETURNS=true
export CUSTOM_ERRORS=true
export USE_UUOBJIDS=true
export ANONYMOUS_OBJECTS=true
export ENABLE_EVENTLOG=true

docker compose up --build
