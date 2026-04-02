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
