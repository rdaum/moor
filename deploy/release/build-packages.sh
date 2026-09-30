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

# Build release artifacts for the native architecture plus architecture-independent web files
# Outputs organized packages for upload to release

set -e

# PostgreSQL variants retain the default package names and add the libpq dependency.
postgres_build=()
postgres_deb=()
case "${POSTGRES:-false}" in
    true) postgres_build=(--features postgres); postgres_deb=(--variant postgres) ;;
    false) ;;
    *) echo 'POSTGRES must be true or false' >&2; exit 2 ;;
esac

REPO_ROOT=$(cd "$(dirname "$0")/../.." && pwd)
cd "$REPO_ROOT"

# Configuration
BUILD_CORES=4
CARGO_BUILD_JOBS=4
OUTPUT_DIR="$REPO_ROOT/release-artifacts"

if ! command -v dpkg &> /dev/null; then
    echo "Error: dpkg not found. This script builds Debian packages."
    exit 1
fi

DEB_ARCH=$(dpkg --print-architecture)
case "$DEB_ARCH" in
    amd64)
        ARTIFACT_ARCH="x86_64"
        ;;
    arm64)
        ARTIFACT_ARCH="aarch64"
        ;;
    *)
        echo "Error: unsupported Debian architecture: $DEB_ARCH"
        exit 1
        ;;
esac

echo "======================================"
echo "mooR Release Build - Debian Packages"
echo "======================================"
echo ""
echo "Output directory: $OUTPUT_DIR"
echo "Build cores: $BUILD_CORES"
echo "Architecture: $ARTIFACT_ARCH ($DEB_ARCH)"
echo ""

# Create output directory
mkdir -p "$OUTPUT_DIR/$ARTIFACT_ARCH"
mkdir -p "$OUTPUT_DIR/all"

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

# ============================================================================
# Build native-architecture packages
# ============================================================================

echo "======================================"
echo "Building $ARTIFACT_ARCH packages"
echo "======================================"
echo ""

# Build native binaries
echo "Building $ARTIFACT_ARCH binaries (release profile, limited to $BUILD_CORES cores)..."
CARGO_BUILD_JOBS=$CARGO_BUILD_JOBS cargo build --release -p moor-server -p moor-daemon -p moor-telnet-host -p moor-web-host -p moor-curl-worker -p moorc -p moor-emh -j $BUILD_CORES "${postgres_build[@]}"
echo ""

# Build native Debian packages
echo "Building $ARTIFACT_ARCH Debian packages..."
for pkg in moor-server moor-daemon moor-telnet-host moor-web-host moor-curl-worker moorc moor-emh; do
    echo "  Building $pkg..."
    case "$pkg" in
        moor-server|moor-daemon|moorc|moor-emh)
            cargo deb -p "$pkg" --profile release --no-build "${postgres_deb[@]}" ;;
        *) cargo deb -p "$pkg" --profile release --no-build ;;
    esac
done
echo ""

# Copy native packages to output
echo "Copying $ARTIFACT_ARCH packages to output directory..."
cp target/debian/*_"$DEB_ARCH".deb "$OUTPUT_DIR/$ARTIFACT_ARCH/"
echo ""

# Build architecture-independent Meadow package
echo "Building Meadow web client package..."
npm ci
npm run meadow:build:deb
cp target/debian/moor-web-client_*_all.deb "$OUTPUT_DIR/all/"
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
echo "$ARTIFACT_ARCH packages:"
if [ -d "$OUTPUT_DIR/$ARTIFACT_ARCH" ] && [ "$(ls -A "$OUTPUT_DIR/$ARTIFACT_ARCH" 2>/dev/null)" ]; then
    ls -lh "$OUTPUT_DIR/$ARTIFACT_ARCH/"
else
    echo "  (none found)"
fi
echo ""
echo "Architecture-independent packages:"
if [ -d "$OUTPUT_DIR/all" ] && [ "$(ls -A "$OUTPUT_DIR/all" 2>/dev/null)" ]; then
    ls -lh "$OUTPUT_DIR/all/"
else
    echo "  (none found)"
fi
echo ""
echo "All artifacts in: $OUTPUT_DIR"
echo ""
echo "Next steps:"
echo "  1. Review packages: ls -lh $OUTPUT_DIR/*/*.deb"
echo "  2. Upload to the GitHub release"
echo ""
