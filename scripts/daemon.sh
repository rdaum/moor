#!/bin/bash
# moor daemon runner
# Usage: scripts/daemon.sh [--debug] [--traced] [--release] [--clean-slate] [--help]
#
# Environment variables:
#   MOOR_DATA_DIR              - Data directory (default: ./moor-data)
#   MOOR_DB                    - Database file (default: development.db)
#   MOOR_CORE                  - Core to import (default: cores/lambda-moor/src)
#   MOOR_EXPORT                - Export directory (default: development-export)
#   MOOR_CHECKPOINT_INTERVAL   - Checkpoint interval in seconds (default: 3600)

set -e

show_help() {
    cat <<EOF
Usage: $0 [OPTIONS]

Options:
  --debug       Build in debug mode (default: release)
  --release     Build in release mode (default)
  --traced      Build with trace_events feature (debug mode)
  --clean-slate Wipe the data directory before starting (forces re-import)
  --help        Show this help message

Environment Variables:
  MOOR_DATA_DIR              Data directory (default: ./moor-data)
  MOOR_DB                    Database file (default: development.db)
  MOOR_CORE                  Core to import (default: cores/lambda-moor/src)
  MOOR_EXPORT                Export directory (default: development-export)
  MOOR_CHECKPOINT_INTERVAL   Checkpoint interval in seconds (default: 3600)
EOF
    exit 0
}

RELEASE="--release"
TRACED=false
CLEAN_SLATE=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --help)
            show_help
            ;;
        --debug)
            RELEASE=""
            shift
            ;;
        --release)
            RELEASE="--release"
            shift
            ;;
        --traced)
            TRACED=true
            shift
            ;;
        --clean-slate)
            CLEAN_SLATE=true
            shift
            ;;
        *)
            echo "Unknown option: $1" >&2
            exit 1
            ;;
    esac
done

MOOR_DATA_DIR="${MOOR_DATA_DIR:-./moor-data}"
MOOR_DB="${MOOR_DB:-development.db}"
MOOR_CORE="${MOOR_CORE:-cores/lambda-moor/src}"
MOOR_EXPORT="${MOOR_EXPORT:-development-export}"
MOOR_CHECKPOINT_INTERVAL="${MOOR_CHECKPOINT_INTERVAL:-3600}"

if [[ "$CLEAN_SLATE" == true ]]; then
    echo "Wiping data directory: $MOOR_DATA_DIR"
    rm -rf "$MOOR_DATA_DIR"
fi

CARGO_ARGS=()
if [[ -n "$RELEASE" ]]; then
    CARGO_ARGS+=("$RELEASE")
fi
if [[ "$TRACED" == true ]]; then
    CARGO_ARGS+=(--features trace_events)
fi

COMMON_ARGS=(
    "$MOOR_DATA_DIR"
    --db "$MOOR_DB"
    --import-format objdef
    --import "$MOOR_CORE"
    --export "$MOOR_EXPORT"
    --export-format objdef
    --checkpoint-interval-seconds "$MOOR_CHECKPOINT_INTERVAL"
    --use-boolean-returns true
    --custom-errors true
    --use-uuobjids true
    --generate-keypair
)

if [[ "$TRACED" == true ]]; then
    COMMON_ARGS+=(--trace-output moor-trace.json)
fi

cargo run "${CARGO_ARGS[@]}" -p moor-daemon -- "${COMMON_ARGS[@]}"
