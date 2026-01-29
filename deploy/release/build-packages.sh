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

# Build release artifacts (Debian packages for x86_64 and aarch64)
# This script cross-compiles ARM64 packages on x86_64
# Outputs organized packages for upload to release

set -e

REPO_ROOT=$(cd "$(dirname "$0")/../.." && pwd)
cd "$REPO_ROOT"

# Configuration
BUILD_CORES=4
CARGO_BUILD_JOBS=4
OUTPUT_DIR="$REPO_ROOT/release-artifacts"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

echo "======================================"
echo "mooR Release Build - Debian Packages"
echo "======================================"
echo ""
echo "Output directory: $OUTPUT_DIR"
echo "Build cores: $BUILD_CORES"
echo ""

# Create output directory
mkdir -p "$OUTPUT_DIR/x86_64"
mkdir -p "$OUTPUT_DIR/aarch64"

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
    echo "Warning: npm not found. Skipping web client package."
    NPM_AVAILABLE=0
else
    NPM_AVAILABLE=1
fi

echo "Prerequisites OK"
echo ""

# ============================================================================
# Build x86_64 packages
# ============================================================================

echo "======================================"
echo "Building x86_64 packages"
echo "======================================"
echo ""

# Build x86_64 binaries
echo "Building x86_64 binaries (release profile, limited to $BUILD_CORES cores)..."
CARGO_BUILD_JOBS=$CARGO_BUILD_JOBS cargo build --release -p moor-daemon -p moor-telnet-host -p moor-web-host -p moor-curl-worker -p moorc -p moor-emh -j $BUILD_CORES
echo ""

# Build x86_64 Debian packages
echo "Building x86_64 Debian packages..."
for pkg in moor-daemon moor-telnet-host moor-web-host moor-curl-worker moorc moor-emh; do
    echo "  Building $pkg..."
    cargo deb -p "$pkg" --profile release --no-build
done
echo ""

# Copy x86_64 packages to output
echo "Copying x86_64 packages to output directory..."
cp target/debian/*_amd64.deb "$OUTPUT_DIR/x86_64/" 2>/dev/null || true
echo ""


# ============================================================================
# Summary
# ============================================================================

echo "======================================"
echo "Build complete!"
echo "======================================"
echo ""
echo "Release artifacts:"
echo ""
echo "x86_64 packages:"
if [ -d "$OUTPUT_DIR/x86_64" ] && [ "$(ls -A "$OUTPUT_DIR/x86_64" 2>/dev/null)" ]; then
    ls -lh "$OUTPUT_DIR/x86_64/"
else
    echo "  (none found)"
fi
echo ""
echo "All artifacts in: $OUTPUT_DIR"
echo ""
echo "Next steps:"
echo "  1. Review packages: ls -lh $OUTPUT_DIR/*/*.deb"
echo "  2. Upload to Codeberg release"
echo ""
