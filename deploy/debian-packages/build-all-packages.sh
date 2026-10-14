#!/bin/bash
# Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
# software: you can redistribute it and/or modify it under the terms of the GNU
# Affero General Public License as published by the Free Software Foundation,
# version 3.
#
# This program is distributed in the hope that it will be useful, but WITHOUT
# ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
# FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
# details.
#
# You should have received a copy of the GNU Affero General Public License along
# with this program. If not, see <https://www.gnu.org/licenses/>.

set -e

# PostgreSQL variants retain the default package names and add the libpq dependency.
postgres_build=()
postgres_deb=()
case "${POSTGRES:-false}" in
    true) postgres_build=(--features postgres); postgres_deb=(--variant postgres) ;;
    false) ;;
    *) echo 'POSTGRES must be true or false' >&2; exit 2 ;;
esac

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

if ! command -v npm &> /dev/null; then
    echo "Error: npm not found. Install Node.js and npm to build Meadow."
    exit 1
fi

if ! command -v dpkg-deb &> /dev/null; then
    echo "Error: dpkg-deb not found. Install the dpkg package."
    exit 1
fi

echo "Prerequisites OK"
echo ""

# Build only required packages using release-fast profile (faster builds, less memory usage)
echo "======================================"
echo "Building required packages (release-fast profile)"
echo "======================================"
echo "Using CARGO_BUILD_JOBS=2 to limit memory usage..."
CARGO_BUILD_JOBS=2 cargo build --profile release-fast -p moor-server -p moor-daemon -p moor-telnet-host -p moor-web-host -p moor-curl-worker -p moorc -p moor-emh "${postgres_build[@]}"
echo ""

# Build daemon package
echo "======================================"
echo "Building moor-daemon package"
echo "======================================"
cargo deb -p moor-daemon --profile release-fast --no-build "${postgres_deb[@]}"
echo ""

# Build single-process package
echo "======================================"
echo "Building moor package"
echo "======================================"
cargo deb -p moor-server --profile release-fast --no-build "${postgres_deb[@]}"
echo ""

# Build telnet-host package
echo "======================================"
echo "Building moor-telnet-host package"
echo "======================================"
cargo deb -p moor-telnet-host --profile release-fast --no-build
echo ""

# Build web-host package
echo "======================================"
echo "Building moor-web-host package"
echo "======================================"
cargo deb -p moor-web-host --profile release-fast --no-build
echo ""

# Build curl-worker package
echo "======================================"
echo "Building moor-curl-worker package"
echo "======================================"
cargo deb -p moor-curl-worker --profile release-fast --no-build
echo ""

# Build moorc package
echo "======================================"
echo "Building moorc package"
echo "======================================"
cargo deb -p moorc --profile release-fast --no-build "${postgres_deb[@]}"
echo ""

# Build moor-emh package
echo "======================================"
echo "Building moor-emh package"
echo "======================================"
cargo deb -p moor-emh --profile release-fast --no-build "${postgres_deb[@]}"
echo ""

# Build Meadow web client package
echo "======================================"
echo "Building moor-web-client package"
echo "======================================"
npm ci
npm run meadow:build:deb
echo ""

# Summary
echo "======================================"
echo "Build complete!"
echo "======================================"
echo ""
echo "Generated packages:"
ls -lh "$REPO_ROOT"/target/debian/*.deb 2>/dev/null || echo "No .deb files found"
echo ""
echo "Install the single-process package with:"
echo "  sudo dpkg -i $REPO_ROOT/target/debian/moor_*.deb"
echo ""
echo "Or install the split-service packages with:"
echo "  sudo dpkg -i $REPO_ROOT/target/debian/moor-daemon_*.deb"
echo "  sudo dpkg -i $REPO_ROOT/target/debian/moor-telnet-host_*.deb"
echo "  sudo dpkg -i $REPO_ROOT/target/debian/moor-web-host_*.deb"
echo "  sudo dpkg -i $REPO_ROOT/target/debian/moor-curl-worker_*.deb"
echo "  sudo dpkg -i $REPO_ROOT/target/debian/moorc_*.deb"
echo "  sudo dpkg -i $REPO_ROOT/target/debian/moor-emh_*.deb"
echo "  sudo dpkg -i $REPO_ROOT/target/debian/moor-web-client_*.deb"
echo ""
echo "Fix missing dependencies with:"
echo "  sudo apt-get install -f"
