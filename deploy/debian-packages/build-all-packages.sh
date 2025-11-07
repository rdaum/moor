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

set -e

# Build all mooR debian packages
# This script builds the rust binary packages and the web client package

REPO_ROOT=$(cd "$(dirname "$0")/../.." && pwd)
cd "$REPO_ROOT"

echo "======================================"
echo "Building all mooR Debian packages"
echo "======================================"
echo ""

# Check prerequisites
echo "Checking prerequisites..."
if ! command -v cargo &> /dev/null; then
    echo "Error: cargo not found. Please install Rust toolchain."
    exit 1
fi

if ! command -v cargo-deb &> /dev/null; then
    echo "Error: cargo-deb not found. Install with: cargo install cargo-deb"
    exit 1
fi

echo "Prerequisites OK"
echo ""

# Build Rust binaries in release mode
echo "======================================"
echo "Building Rust workspace (release)"
echo "======================================"
cargo build --release --workspace
echo ""

# Build daemon package
echo "======================================"
echo "Building moor-daemon package"
echo "======================================"
cargo deb -p moor-daemon --no-build
echo ""

# Build telnet-host package
echo "======================================"
echo "Building moor-telnet-host package"
echo "======================================"
cargo deb -p moor-telnet-host --no-build
echo ""

# Build web-host package
echo "======================================"
echo "Building moor-web-host package"
echo "======================================"
cargo deb -p moor-web-host --no-build
echo ""

# Build web client package (if Node.js is available)
if command -v npm &> /dev/null; then
    echo "======================================"
    echo "Building web client"
    echo "======================================"

    # Build web client if dist doesn't exist or is older than source
    if [ ! -d "dist" ] || [ "dist" -ot "web-client" ]; then
        echo "Building web client with npm..."
        npm install
        npm run build
    else
        echo "Using existing dist/ directory"
    fi
    echo ""

    echo "======================================"
    echo "Building moor-web-client package"
    echo "======================================"
    ./deploy/debian-packages/build-web-client-deb.sh
    echo ""
else
    echo "======================================"
    echo "Skipping web client (npm not found)"
    echo "======================================"
    echo "Install Node.js to build web client package"
    echo ""
fi

# Summary
echo "======================================"
echo "Build complete!"
echo "======================================"
echo ""
echo "Generated packages:"
ls -lh *.deb 2>/dev/null || echo "No .deb files found"
echo ""
echo "Install packages with:"
echo "  sudo dpkg -i moor-daemon_*.deb"
echo "  sudo dpkg -i moor-telnet-host_*.deb"
echo "  sudo dpkg -i moor-web-host_*.deb"
echo "  sudo dpkg -i moor-web-client_*.deb  # if built"
echo ""
echo "Fix missing dependencies with:"
echo "  sudo apt-get install -f"
