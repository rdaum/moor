#!/bin/bash
# moor telnet host runner
# Usage: scripts/telnet.sh [--debug] [--release] [--tls] [--help]

set -e

show_help() {
    cat <<EOF
Usage: $0 [OPTIONS]

Options:
  --debug     Build in debug mode (default: release)
  --release   Build in release mode (default)
  --tls       Enable TLS on port 8889
  --help      Show this help message
EOF
    exit 0
}

RELEASE="--release"
TLS=false

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
        --tls)
            TLS=true
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

if [[ "$TLS" == true ]]; then
    cargo run "${CARGO_ARGS[@]}" -p moor-telnet-host -- \
        --tls-port 8889 \
        --tls-cert certs/test-cert.pem \
        --tls-key certs/test-key.pem
else
    cargo run "${CARGO_ARGS[@]}" -p moor-telnet-host --
fi
