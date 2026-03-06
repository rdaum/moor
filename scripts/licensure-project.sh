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

set -euo pipefail

usage() {
    cat <<'EOF'
Usage: ./scripts/licensure-project.sh [LICENSURE_OPTIONS...]

Run licensure on tracked files from git ls-files, bypassing licensure --project.

Examples:
  ./scripts/licensure-project.sh -i
  ./scripts/licensure-project.sh --check
  ./scripts/licensure-project.sh -i -v
EOF
}

for arg in "$@"; do
    case "$arg" in
        --project|-p)
            echo "Do not pass --project to this wrapper; it already uses git ls-files." >&2
            exit 2
            ;;
        --help|-h)
            usage
            exit 0
            ;;
    esac
done

if ! command -v licensure >/dev/null 2>&1; then
    echo "licensure not found in PATH" >&2
    exit 127
fi

if ! git rev-parse --show-toplevel >/dev/null 2>&1; then
    echo "This script must be run inside a git working tree." >&2
    exit 2
fi

git ls-files -z | xargs -0 --no-run-if-empty licensure "$@"
