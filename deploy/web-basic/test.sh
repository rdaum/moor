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

# Test script for web-basic deployment

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEPLOY_DIR="$(dirname "$SCRIPT_DIR")"

# Source common helpers
source "$DEPLOY_DIR/test-helpers.sh"

log_info "Starting web-basic deployment test"

# Change to the deployment directory
cd "$SCRIPT_DIR"

# Setup test environment (clean up any existing containers)
setup

# Start services
log_info "Starting services with docker compose..."
docker compose up -d

# Wait for services to start
wait_for_service "moor-daemon" 30
wait_for_service "moor-web-host" 30
wait_for_service "moor-frontend" 30

# Wait for database import to complete
wait_for_import "moor-daemon" 180

# Wait for HTTP port to be available
wait_for_port "localhost" 8080 30

# Test HTTP endpoints
log_info "Testing HTTP endpoints..."

# Test frontend (should serve the web client)
test_http "http://localhost:8080/" 200

# Test web-host API health endpoint (if it exists)
# Note: This might 404 if there's no health endpoint, which is fine
if curl -s "http://localhost:8080/api/health" > /dev/null 2>&1; then
    log_info "API health endpoint found"
else
    log_info "No API health endpoint (this is fine)"
fi

# Test that we can access the frontend assets
if curl -s "http://localhost:8080/" | grep -qE "moor|<!DOCTYPE html>"; then
    log_info "Frontend is serving HTML content"
else
    log_warn "Frontend response doesn't look like HTML"
fi

# Test the welcome message endpoint - this verifies MOO core is loaded and web-host can talk to daemon
log_info "Testing MOO core via welcome message endpoint..."
WELCOME_STATUS=$(curl -s -o /dev/null -w "%{http_code}" "http://localhost:8080/fb/invoke_welcome_message")
if [ "$WELCOME_STATUS" = "200" ]; then
    log_info "✓ MOO core is loaded and responding via web API"
else
    log_error "Welcome message endpoint returned status $WELCOME_STATUS (expected 200)"
    log_error "MOO core may not be loaded or web-host cannot connect to daemon"
fi

# Test WebSocket endpoint (if accessible)
# The WebSocket is proxied through nginx to moor-web-host
log_info "Testing WebSocket endpoint availability..."
# WebSocket endpoint is typically at /ws or similar
# Just verify the nginx proxy is configured
if curl -s -I "http://localhost:8080/" | grep -qi "nginx"; then
    log_info "nginx is responding"
else
    log_info "Proxy server responding (may not be nginx)"
fi

# Check docker logs for errors
log_info "Checking docker logs for critical errors..."
for service in moor-daemon moor-web-host moor-frontend; do
    log_info "Checking $service logs..."
    ERRORS=$(docker compose logs "$service" 2>&1 | grep -iE "panic|fatal|error" | head -5)
    if [ -n "$ERRORS" ]; then
        log_warn "Found errors in $service logs:"
        echo "$ERRORS"
    else
        log_info "No critical errors in $service"
    fi
done

log_info "✓ Web-basic deployment test completed successfully"
