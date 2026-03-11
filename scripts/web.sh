#!/bin/bash
# moor web host runner
# Usage: scripts/web.sh [--debug] [--release] [--listen-address ADDRESS] [--help]

set -e

show_help() {
    cat <<EOF
Usage: $0 [OPTIONS]

Options:
  --debug              Build in debug mode (default: release)
  --release            Build in release mode (default)
  --listen-address     Address to listen on (default: 0.0.0.0:8080)
  --help               Show this help message
EOF
    exit 0
}

RELEASE="--release"
LISTEN_ADDRESS="0.0.0.0:8080"

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
        --listen-address)
            LISTEN_ADDRESS="$2"
            shift 2
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

cargo run "${CARGO_ARGS[@]}" -p moor-web-host -- --listen-address "$LISTEN_ADDRESS"
