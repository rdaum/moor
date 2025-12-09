#!/bin/bash
# Setup script for LambdaMOO sources used by lambdamoo-harness
#
# This fetches LambdaMOO from wrog/lambdamoo and applies patches needed
# for the test harness to work. The sources are placed in the workspace
# root at ./lambdamoo/
#
# LambdaMOO is licensed under the Xerox License (not GPL).
# See: https://spdx.org/licenses/Xerox.html

set -e

# Pin to a specific commit for reproducibility
LAMBDAMOO_REPO="https://github.com/wrog/lambdamoo.git"
LAMBDAMOO_COMMIT="b81bf9da88e2fd900c4ff1d0efbc8b2f964d1542"

# Find workspace root (where Cargo.toml with [workspace] lives)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
LAMBDAMOO_DIR="$WORKSPACE_ROOT/lambdamoo"
PATCHES_DIR="$SCRIPT_DIR/patches"

echo "Setting up LambdaMOO sources..."
echo "  Workspace root: $WORKSPACE_ROOT"
echo "  Target directory: $LAMBDAMOO_DIR"
echo "  Commit: $LAMBDAMOO_COMMIT"

if [ -d "$LAMBDAMOO_DIR/.git" ]; then
    echo "LambdaMOO directory exists, fetching updates..."
    cd "$LAMBDAMOO_DIR"
    git fetch origin
    git checkout "$LAMBDAMOO_COMMIT"
    git reset --hard "$LAMBDAMOO_COMMIT"
else
    echo "Cloning LambdaMOO..."
    git clone "$LAMBDAMOO_REPO" "$LAMBDAMOO_DIR"
    cd "$LAMBDAMOO_DIR"
    git checkout "$LAMBDAMOO_COMMIT"
fi

echo "Applying patches..."
for patch in "$PATCHES_DIR"/*.patch; do
    if [ -f "$patch" ]; then
        echo "  Applying $(basename "$patch")..."
        git apply "$patch" || {
            echo "    Patch may already be applied, checking..."
            git apply --check --reverse "$patch" 2>/dev/null && echo "    Already applied." || {
                echo "    ERROR: Failed to apply patch!"
                exit 1
            }
        }
    fi
done

echo "Running configure..."
if [ ! -f "$LAMBDAMOO_DIR/config.h" ]; then
    cd "$LAMBDAMOO_DIR"
    sh configure
fi

echo "Creating version_src.h..."
cat > "$LAMBDAMOO_DIR/version_src.h" << 'EOF'
/* Generated for mooR test harness */
#define VERSION_EXT "+moor_harness"
#define VERSION_SOURCE(DEF) DEF(vcs,"git")
EOF

echo ""
echo "LambdaMOO setup complete!"
echo "You can now build lambdamoo-harness with: cargo build -p lambdamoo-harness"
