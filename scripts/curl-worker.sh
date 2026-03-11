#!/bin/bash
# moor curl worker runner
# Usage: scripts/curl-worker.sh [--debug] [--release] [--help]

set -e

show_help() {
    cat <<EOF
Usage: $0 [OPTIONS]

Options:
  --debug     Build in debug mode (default: release)
  --release   Build in release mode (default)
  --help      Show this help message
EOF
    exit 0
}

RELEASE="--release"

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
        *)
            echo "Unknown option: $1" >&2
            exit 1
            ;;
    esac
done

CARGO_ARGS=()
if [[ -n "$RELEASE" ]]; then
    CARGO_ARGS+=("$RELEASE")
fi

cargo run "${CARGO_ARGS[@]}" -p moor-curl-worker --
