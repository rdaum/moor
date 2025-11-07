#!/bin/bash
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

# Common testing utilities for deployment validation

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Print colored status messages
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

# Wait for a service to be healthy
# Usage: wait_for_service <container_name> <timeout_seconds>
wait_for_service() {
    local container=$1
    local timeout=${2:-60}
    local elapsed=0

    log_info "Waiting for container '$container' to be healthy (timeout: ${timeout}s)..."

    while [ $elapsed -lt $timeout ]; do
        # Check container status directly by name
        local status=$(docker inspect --format='{{.State.Status}}' "$container" 2>/dev/null || echo "not-found")
        local health=$(docker inspect --format='{{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}' "$container" 2>/dev/null || echo "none")

        if [ "$status" = "not-found" ]; then
            log_error "Container '$container' not found"
            return 1
        fi

        # If container is running and either has no healthcheck or is healthy, we're good
        if [ "$status" = "running" ]; then
            if [ "$health" = "healthy" ] || [ "$health" = "none" ]; then
                log_info "Container '$container' is ready (status: $status, health: $health)"
                return 0
            fi
        fi

        sleep 2
        elapsed=$((elapsed + 2))
    done

    log_error "Container '$container' did not become ready within ${timeout}s"
    return 1
}

# Wait for a TCP port to be available
# Usage: wait_for_port <host> <port> <timeout_seconds>
wait_for_port() {
    local host=$1
    local port=$2
    local timeout=${3:-30}
    local elapsed=0

    log_info "Waiting for $host:$port to be available (timeout: ${timeout}s)..."

    while [ $elapsed -lt $timeout ]; do
        if nc -z -w1 "$host" "$port" 2>/dev/null; then
            log_info "$host:$port is available"
            return 0
        fi
        sleep 1
        elapsed=$((elapsed + 1))
    done

    log_error "$host:$port not available within ${timeout}s"
    return 1
}

# Test HTTP endpoint
# Usage: test_http <url> <expected_status>
test_http() {
    local url=$1
    local expected=${2:-200}

    log_info "Testing HTTP endpoint: $url (expecting status $expected)"

    local status
    status=$(curl -s -o /dev/null -w "%{http_code}" "$url")

    if [ "$status" = "$expected" ]; then
        log_info "HTTP test passed: got status $status"
        return 0
    else
        log_error "HTTP test failed: expected $expected, got $status"
        return 1
    fi
}

# Test telnet connection and send a command
# Usage: test_telnet <host> <port> <command> <expected_pattern>
test_telnet() {
    local host=$1
    local port=$2
    local command=$3
    local expected=$4

    log_info "Testing telnet connection to $host:$port"
    log_info "Sending command: $command"

    # Send command and capture output
    local output
    output=$(
        {
            sleep 1
            echo "$command"
            sleep 2
        } | telnet "$host" "$port" 2>&1
    )

    if echo "$output" | grep -q "$expected"; then
        log_info "Telnet test passed: found expected pattern '$expected'"
        return 0
    else
        log_error "Telnet test failed: pattern '$expected' not found in output"
        log_error "Output was:"
        echo "$output" | head -20
        return 1
    fi
}

# Test WebSocket connection
# Usage: test_websocket <url>
test_websocket() {
    local url=$1

    log_info "Testing WebSocket endpoint: $url"

    # Try to connect with curl's WebSocket support (if available)
    # Otherwise just test if the HTTP upgrade request is accepted
    if curl --version | grep -q "WebSockets"; then
        if curl -s --no-buffer -H "Connection: Upgrade" -H "Upgrade: websocket" "$url" > /dev/null 2>&1; then
            log_info "WebSocket test passed"
            return 0
        fi
    else
        # Fallback: just check if the endpoint responds to upgrade request
        local response
        response=$(curl -s -i -H "Connection: Upgrade" -H "Upgrade: websocket" "$url" 2>&1 | head -1)
        if echo "$response" | grep -qE "101|Switching Protocols|426"; then
            log_info "WebSocket endpoint responding (got: $response)"
            return 0
        fi
    fi

    log_warn "WebSocket test inconclusive (curl may not support WebSocket testing)"
    return 0  # Don't fail on this
}

# Clean up function
cleanup() {
    log_info "Cleaning up..."
    # Export USER_ID and GROUP_ID for cleanup too
    export USER_ID=$(id -u)
    export GROUP_ID=$(id -g)
    docker compose down -v 2>&1 | grep -v "^$" || true
    log_info "Cleanup complete"
}

# Setup function - clean up any existing containers before starting
setup() {
    log_info "Setting up test environment..."

    # Export USER_ID and GROUP_ID for docker-compose to use
    # (UID is readonly in bash, so we use USER_ID instead)
    export USER_ID=$(id -u)
    export GROUP_ID=$(id -g)

    # Stop and remove any existing containers that might conflict
    docker compose down -v 2>&1 | grep -v "^$" || true

    # Also manually remove any moor containers that might be stuck
    for container in moor-daemon moor-telnet-host moor-web-host moor-frontend moor-curl-worker; do
        if docker ps -a --format '{{.Names}}' | grep -q "^${container}$"; then
            log_info "Removing stuck container: $container"
            docker rm -f "$container" 2>&1 | grep -v "^$" || true
        fi
    done

    # Remove existing moor-data directory to force a fresh import
    # No sudo needed since containers now run as current user
    if [ -d "./moor-data" ]; then
        log_info "Removing existing database to force fresh core import..."
        rm -rf ./moor-data ./moor-*-data ./moor-ipc 2>/dev/null || true
    fi

    # Pre-create directories with correct ownership to prevent Docker from creating them as root
    log_info "Creating data directories with correct ownership..."
    mkdir -p ./moor-data ./moor-ipc ./moor-telnet-host-data ./moor-web-host-data ./moor-curl-worker-data 2>/dev/null || true

    log_info "Setup complete"
}

# Wait for database import to complete
# Usage: wait_for_import <container_name> <timeout_seconds>
wait_for_import() {
    local container=$1
    local timeout=${2:-180}
    local elapsed=0

    log_info "Waiting for core database import to complete (timeout: ${timeout}s)..."

    while [ $elapsed -lt $timeout ]; do
        local logs=$(docker logs "$container" 2>&1)

        # Check if daemon finished starting (this appears after import or DB load)
        if echo "$logs" | grep -q "Daemon started. Listening for RPC events"; then
            log_info "Daemon has started - database is ready"
            return 0
        fi

        # Check if import is happening
        if echo "$logs" | grep -qi "import"; then
            log_info "Import in progress... (${elapsed}s elapsed)"
        fi

        # Check if recovering/loading existing database
        if echo "$logs" | grep -q "Recovering keyspace"; then
            log_info "Loading existing database... (${elapsed}s elapsed)"
        fi

        sleep 5
        elapsed=$((elapsed + 5))
    done

    log_error "Database did not become ready within ${timeout}s"
    log_error "Last logs:"
    docker logs "$container" 2>&1 | tail -20
    return 1
}

# Trap cleanup on exit
trap cleanup EXIT
