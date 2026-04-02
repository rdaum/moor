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

cargo run "${CARGO_ARGS[@]}" -p moor-web-host -- \
    --listen-address "$LISTEN_ADDRESS" \
    --webrtc-enabled true \
    --webrtc-realtime-domains realtime
