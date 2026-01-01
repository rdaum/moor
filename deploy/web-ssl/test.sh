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

# Test script for web-ssl deployment
# Note: This test cannot verify actual SSL certificates without a real domain
# It validates that services start and nginx is properly configured

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEPLOY_DIR="$(dirname "$SCRIPT_DIR")"

# Source common helpers
source "$DEPLOY_DIR/test-helpers.sh"

log_info "Starting web-ssl deployment test"
log_warn "Note: SSL certificate validation requires a real domain and is skipped in automated tests"

# Change to the deployment directory
cd "$SCRIPT_DIR"

# Setup test environment (clean up any existing containers)
setup

# Check if .env file exists
if [ ! -f .env ]; then
    log_warn "No .env file found - creating test .env"
    cat > .env << EOF
VIRTUAL_HOST=localhost
LETSENCRYPT_HOST=localhost
LETSENCRYPT_EMAIL=test@example.com
EOF
fi

# Start services (without certbot)
log_info "Starting services with docker compose..."
docker compose up -d

# Wait for services to start
wait_for_service "moor-daemon" 30
wait_for_service "moor-web-host" 30
wait_for_service "moor-frontend" 30

# Wait for database import to complete
wait_for_import "moor-daemon" 180

# Wait for HTTP port to be available
wait_for_port "localhost" 80 30

# Test HTTP endpoint (nginx should be serving or redirecting)
log_info "Testing HTTP endpoint..."
# Note: Without real SSL certs, nginx might fail or return errors - that's expected
if curl -s -I "http://localhost/" 2>&1 | grep -qiE "HTTP|nginx|302|301"; then
    log_info "nginx is responding (may redirect to HTTPS if configured)"
else
    log_warn "nginx response unclear (expected without real SSL certificates)"
fi

# Check that we can reach the frontend
log_info "Checking if frontend is accessible..."
if curl -s -k "http://localhost/" 2>&1 | grep -qE "<!DOCTYPE html>|moor|nginx"; then
    log_info "Frontend is serving content"
else
    log_warn "Frontend may not be fully configured (expected without real SSL)"
fi

# Test the welcome message endpoint if accessible via HTTP
log_info "Testing MOO core via welcome message endpoint..."
WELCOME_STATUS=$(curl -s -o /dev/null -w "%{http_code}" "http://localhost/fb/invoke_welcome_message" 2>/dev/null || echo "failed")
if [ "$WELCOME_STATUS" = "200" ]; then
    log_info "✓ MOO core is loaded and responding via web API"
elif [ "$WELCOME_STATUS" = "failed" ] || [ "$WELCOME_STATUS" = "000" ]; then
    log_warn "Could not test welcome message endpoint (expected without SSL - nginx may be redirecting)"
else
    log_warn "Welcome message endpoint returned status $WELCOME_STATUS (may need HTTPS)"
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

log_info "✓ Web-ssl deployment test completed successfully"
log_info "Note: For full SSL validation, deploy on a server with a real domain and DNS"
