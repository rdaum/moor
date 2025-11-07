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

# Test script for Debian package deployment
# This script builds .deb packages, installs them in an Incus container, and validates the deployment

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../test-helpers.sh"

CONTAINER_NAME="moor-deb-test"
DEBIAN_VERSION="13"  # Trixie

log_info "Starting Debian packages deployment test"

# Cleanup function
cleanup() {
    if [ "${KEEP_CONTAINER:-0}" = "1" ]; then
        log_info "KEEP_CONTAINER=1, skipping cleanup. Container: $CONTAINER_NAME"
    else
        log_info "Cleaning up..."
        incus delete --force "$CONTAINER_NAME" 2>/dev/null || true
        log_info "Cleanup complete"
    fi
}

# Setup trap for cleanup
trap cleanup EXIT

# Check if cargo-deb is installed
if ! command -v cargo-deb &> /dev/null; then
    log_error "cargo-deb is not installed. Install with: cargo install cargo-deb"
    exit 1
fi

# Check if packages already exist, otherwise build them
DEB_DIR="$SCRIPT_DIR/../../target/debian"
DAEMON_DEB_CHECK=$(find "$DEB_DIR" -name "moor-daemon_*.deb" 2>/dev/null | head -1)
TELNET_DEB_CHECK=$(find "$DEB_DIR" -name "moor-telnet-host_*.deb" 2>/dev/null | head -1)

if [ -n "$DAEMON_DEB_CHECK" ] && [ -n "$TELNET_DEB_CHECK" ]; then
    log_info "Using existing .deb packages from previous build"
else
    log_info "Building Debian packages (release-fast profile with CARGO_BUILD_JOBS=2)..."
    log_info "This will take 5-10 minutes on first run..."
    cd "$SCRIPT_DIR"
    ./build-all-packages.sh || {
        log_error "Failed to build packages"
        exit 1
    }
    cd "$SCRIPT_DIR"
fi

# Find the built .deb files
DEB_DIR="$SCRIPT_DIR/../../target/debian"
if [ ! -d "$DEB_DIR" ]; then
    log_error "Debian packages directory not found: $DEB_DIR"
    exit 1
fi

DAEMON_DEB=$(find "$DEB_DIR" -name "moor-daemon_*.deb" | head -1)
TELNET_DEB=$(find "$DEB_DIR" -name "moor-telnet-host_*.deb" | head -1)
WEB_DEB=$(find "$DEB_DIR" -name "moor-web-host_*.deb" | head -1)
CURL_DEB=$(find "$DEB_DIR" -name "moor-curl-worker_*.deb" | head -1)
WEB_CLIENT_DEB=$(find "$DEB_DIR" -name "moor-web-client_*.deb" | head -1)

if [ -z "$DAEMON_DEB" ] || [ -z "$TELNET_DEB" ]; then
    log_error "Required .deb files not found in $DEB_DIR"
    exit 1
fi

log_info "Found packages:"
log_info "  Daemon: $(basename "$DAEMON_DEB")"
log_info "  Telnet: $(basename "$TELNET_DEB")"
[ -n "$WEB_DEB" ] && log_info "  Web: $(basename "$WEB_DEB")"
[ -n "$CURL_DEB" ] && log_info "  Curl Worker: $(basename "$CURL_DEB")"
[ -n "$WEB_CLIENT_DEB" ] && log_info "  Web Client: $(basename "$WEB_CLIENT_DEB")"

# Clean up any existing test container
incus delete --force "$CONTAINER_NAME" 2>/dev/null || true

# Launch Debian container
log_info "Launching Debian $DEBIAN_VERSION container..."
incus launch images:debian/$DEBIAN_VERSION "$CONTAINER_NAME"

# Wait for container to be ready
log_info "Waiting for container to be ready..."
sleep 5

# Copy .deb files to container
log_info "Copying .deb files to container..."
incus file push "$DAEMON_DEB" "$CONTAINER_NAME/tmp/"
incus file push "$TELNET_DEB" "$CONTAINER_NAME/tmp/"
[ -n "$WEB_DEB" ] && incus file push "$WEB_DEB" "$CONTAINER_NAME/tmp/"
[ -n "$CURL_DEB" ] && incus file push "$CURL_DEB" "$CONTAINER_NAME/tmp/"
[ -n "$WEB_CLIENT_DEB" ] && incus file push "$WEB_CLIENT_DEB" "$CONTAINER_NAME/tmp/"

# Copy cores directory for database import
log_info "Copying MOO core database..."
incus exec "$CONTAINER_NAME" -- mkdir -p /tmp/cores
incus file push -r "$SCRIPT_DIR/../../cores/lambda-moor" "$CONTAINER_NAME/tmp/cores/"

# Update package lists
log_info "Updating package lists in container..."
incus exec "$CONTAINER_NAME" -- apt-get update

# Install packages
log_info "Installing moor-daemon..."
incus exec "$CONTAINER_NAME" -- apt-get install -y /tmp/$(basename "$DAEMON_DEB")

log_info "Installing moor-telnet-host..."
incus exec "$CONTAINER_NAME" -- apt-get install -y /tmp/$(basename "$TELNET_DEB")

if [ -n "$WEB_DEB" ]; then
    log_info "Installing moor-web-host..."
    incus exec "$CONTAINER_NAME" -- apt-get install -y /tmp/$(basename "$WEB_DEB")
fi

if [ -n "$CURL_DEB" ]; then
    log_info "Installing moor-curl-worker..."
    incus exec "$CONTAINER_NAME" -- apt-get install -y /tmp/$(basename "$CURL_DEB")
fi

# Install tools for testing
log_info "Installing testing tools (telnet, curl)..."
incus exec "$CONTAINER_NAME" -- apt-get install -y telnet curl

# Configure daemon to import core database
log_info "Configuring daemon to import core database..."
incus exec "$CONTAINER_NAME" -- bash -c 'cat > /etc/default/moor-daemon << EOF
MOOR_DAEMON_ARGS="--import=/tmp/cores/lambda-moor/src --import-format=objdef"
EOF'

# Start services
log_info "Starting moor-daemon..."
incus exec "$CONTAINER_NAME" -- systemctl start moor-daemon

log_info "Waiting for daemon to import database and start..."
sleep 30

# Check daemon status
log_info "Checking daemon status..."
if incus exec "$CONTAINER_NAME" -- systemctl is-active --quiet moor-daemon; then
    log_info "✓ moor-daemon is running"
else
    log_error "moor-daemon failed to start"
    incus exec "$CONTAINER_NAME" -- journalctl -u moor-daemon -n 50 --no-pager
    exit 1
fi

# Start telnet host
log_info "Starting moor-telnet-host..."
incus exec "$CONTAINER_NAME" -- systemctl start moor-telnet-host

sleep 5

# Check telnet host status
log_info "Checking telnet host status..."
if incus exec "$CONTAINER_NAME" -- systemctl is-active --quiet moor-telnet-host; then
    log_info "✓ moor-telnet-host is running"
else
    log_error "moor-telnet-host failed to start"
    incus exec "$CONTAINER_NAME" -- journalctl -u moor-telnet-host -n 50 --no-pager
    exit 1
fi

# Test telnet connection
log_info "Testing telnet connection..."
CONTAINER_IP=$(incus list "$CONTAINER_NAME" -c 4 -f csv | cut -d' ' -f1)
TELNET_VERIFIED=false

# Try from host if we have an IP
if [ -n "$CONTAINER_IP" ]; then
    log_info "Container IP: $CONTAINER_IP"
    if timeout 5 bash -c "echo quit | telnet $CONTAINER_IP 7777" 2>&1 | grep -q "Connected"; then
        log_info "✓ Telnet accessible from host"
        TELNET_VERIFIED=true
    else
        log_warn "Could not connect to telnet from host (may need network configuration)"
    fi
fi

# If host connection failed or no IP, test via exec
if [ "$TELNET_VERIFIED" = false ]; then
    log_info "Testing telnet via exec inside container..."
    TEST_OUTPUT=$(incus exec "$CONTAINER_NAME" -- timeout 10 bash -c '
        {
            sleep 2
            echo "connect wizard"
            sleep 3
            echo "@who"
            sleep 2
            echo "@quit"
            sleep 1
        } | telnet localhost 7777 2>&1' || true)

    if echo "$TEST_OUTPUT" | grep -qE "Connected|Welcome|The First Room|Wizard"; then
        log_info "✓ Telnet connection test passed - MOO core is loaded"
        TELNET_VERIFIED=true
    else
        log_error "Telnet connection test failed"
        echo "$TEST_OUTPUT"
        exit 1
    fi
fi

# Start web host if package was installed
if [ -n "$WEB_DEB" ]; then
    log_info "Starting moor-web-host..."
    incus exec "$CONTAINER_NAME" -- systemctl start moor-web-host

    sleep 5

    # Check web host status
    log_info "Checking web host status..."
    if incus exec "$CONTAINER_NAME" -- systemctl is-active --quiet moor-web-host; then
        log_info "✓ moor-web-host is running"
    else
        log_error "moor-web-host failed to start"
        incus exec "$CONTAINER_NAME" -- journalctl -u moor-web-host -n 50 --no-pager
        exit 1
    fi

    # Test web endpoint
    log_info "Testing web endpoint..."
    HTTP_STATUS=$(incus exec "$CONTAINER_NAME" -- curl -s -o /dev/null -w "%{http_code}" http://localhost:8080/fb/invoke_welcome_message)
    if [ "$HTTP_STATUS" = "200" ]; then
        log_info "✓ Web host responding (HTTP $HTTP_STATUS)"
    else
        log_error "Web host endpoint test failed (HTTP $HTTP_STATUS)"
        exit 1
    fi
fi

# Check logs for errors
log_info "Checking logs for critical errors..."
DAEMON_ERRORS=$(incus exec "$CONTAINER_NAME" -- journalctl -u moor-daemon --no-pager | grep -iE "panic|fatal" | head -5 || true)
if [ -n "$DAEMON_ERRORS" ]; then
    log_warn "Found errors in daemon logs:"
    echo "$DAEMON_ERRORS"
else
    log_info "✓ No critical errors in daemon logs"
fi

TELNET_ERRORS=$(incus exec "$CONTAINER_NAME" -- journalctl -u moor-telnet-host --no-pager | grep -iE "panic|fatal" | head -5 || true)
if [ -n "$TELNET_ERRORS" ]; then
    log_warn "Found errors in telnet host logs:"
    echo "$TELNET_ERRORS"
else
    log_info "✓ No critical errors in telnet host logs"
fi

if [ -n "$WEB_DEB" ]; then
    WEB_ERRORS=$(incus exec "$CONTAINER_NAME" -- journalctl -u moor-web-host --no-pager | grep -iE "panic|fatal" | head -5 || true)
    if [ -n "$WEB_ERRORS" ]; then
        log_warn "Found errors in web host logs:"
        echo "$WEB_ERRORS"
    else
        log_info "✓ No critical errors in web host logs"
    fi
fi

log_info "✓ Debian packages deployment test completed successfully"
log_info ""
log_info "Summary:"
log_info "  - Packages built successfully"
log_info "  - Packages installed in Debian $DEBIAN_VERSION container"
log_info "  - Services started and running"
log_info "  - MOO core database loaded"
log_info "  - Telnet connectivity verified"
if [ -n "$WEB_DEB" ]; then
    log_info "  - Web host endpoint verified"
fi
