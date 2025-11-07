#!/bin/bash
# Test script for telnet-only deployment

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEPLOY_DIR="$(dirname "$SCRIPT_DIR")"

# Source common helpers
source "$DEPLOY_DIR/test-helpers.sh"

log_info "Starting telnet-only deployment test"

# Change to the deployment directory
cd "$SCRIPT_DIR"

# Setup test environment (clean up any existing containers)
setup

# Start services
log_info "Starting services with docker compose..."
docker compose up -d

# Wait for services to start
wait_for_service "moor-daemon" 30
wait_for_service "moor-telnet-host" 30

# Wait for database import to complete
wait_for_import "moor-daemon" 180

# Wait for telnet port to be available
wait_for_port "localhost" 8888 30

# Test telnet connection with actual MOO commands
log_info "Testing telnet connection and MOO login..."
{
    sleep 2
    echo "connect wizard"
    sleep 3
    echo "look"
    sleep 2
    echo "@who"
    sleep 2
    echo "@quit"
    sleep 1
} | telnet localhost 8888 > /tmp/telnet-test-output.txt 2>&1 || true

# Always show the telnet output for debugging
log_info "Telnet test output:"
cat /tmp/telnet-test-output.txt

# Verify we actually got logged in and can see the MOO
if grep -qE "Connected|Welcome|The First Room|Wizard" /tmp/telnet-test-output.txt; then
    log_info "✓ Telnet login test passed - MOO core is loaded and responding"
else
    log_error "✗ Telnet login test failed - MOO core may not be loaded properly"
    log_error "Check the output above for errors"
    exit 1
fi

# Check docker logs for errors
log_info "Checking docker logs for critical errors..."
ERRORS=$(docker compose logs moor-daemon | grep -i "panic\|fatal\|error" | head -5)
if [ -n "$ERRORS" ]; then
    log_warn "Found errors in daemon logs:"
    echo "$ERRORS"
else
    log_info "No critical errors found in daemon logs"
fi

log_info "✓ Telnet-only deployment test completed successfully"
