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

# Script to format Rust code using nightly toolchain with project-specific settings
#
# Why nightly? We use nightly rustfmt features for better import organization:
# - reorder_imports=true: Groups and sorts imports consistently
# - imports_indent=Block: Uses block-style indentation for imports
# - imports_layout=Mixed: Allows mixed import styles (single vs multi-line)
#
# Note: This is the only place we use nightly features - all production code runs on stable Rust.
#
# Usage: ./format-rust.sh [OPTIONS]

set -euo pipefail

# Default options
VERBOSE=false
CHECK_ONLY=false

# Parse command line options
while [[ $# -gt 0 ]]; do
  case $1 in
    -v|--verbose)
      VERBOSE=true
      shift
      ;;
    -c|--check)
      CHECK_ONLY=true
      shift
      ;;
    -h|--help)
      echo "Usage: $0 [OPTIONS]"
      echo "Options:"
      echo "  -v, --verbose    Show verbose output"
      echo "  -c, --check      Check formatting without applying changes"
      echo "  -h, --help       Show this help message"
      exit 0
      ;;
    *)
      echo "Unknown option: $1"
      echo "Use -h or --help for usage information"
      exit 1
      ;;
  esac
done

# Check if nightly toolchain is available
if ! rustup toolchain list | grep -q nightly; then
    echo "Error: Nightly toolchain not found. Install with: rustup toolchain install nightly"
    exit 1
fi

# Build the command
CMD="cargo +nightly fmt"

if [ "$CHECK_ONLY" = true ]; then
    CMD="$CMD -- --check"
fi

# Add project-specific formatting configuration
CMD="$CMD -- --config reorder_imports=true,imports_indent=Block,imports_layout=Mixed"

if [ "$VERBOSE" = true ]; then
    echo "Running: $CMD"
fi

# Execute the command
eval "$CMD"

if [ "$CHECK_ONLY" = true ]; then
    echo "✓ Format check completed"
else
    echo "✓ Formatting completed"
fi