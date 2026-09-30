#!/usr/bin/env bash
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

# Start mooR with the Cowbell core
set -euo pipefail

# Resolve host paths before changing to the Compose project directory.
project_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
core_dir="$project_dir/cores/cowbell/src"
git_upstream=${MOOR_GIT_UPSTREAM:-}
baseline_dir=${MOOR_BASELINE_OBJDEF_DIR:-}
export RUN_DIR="${RUN_DIR:-run-cowbell}"
export BUILD_PROFILE="release-fast"
COMPOSE_ARGS=()
POSTGRES=false
COMPOSE=(docker compose)

usage() {
    cat <<'USAGE'
Usage: scripts/start-moor-cowbell.sh [options] [docker compose up options]

  --core PATH                 Import these objdefs (default: cores/cowbell/src)
  --git-upstream REF          Prepare baselines from local Git ancestry
  --baseline-objdef-dir PATH   Prepare baselines from an explicit source directory
  --debug / --release         Select the server build profile
  --postgres                  Use a local PostgreSQL world in a persistent Docker volume
  --help                      Show this help

Source directories must be inside this checkout, mounted at /db in Docker.
Select at most one baseline source. Existing databases skip import and preparation.
RUN_DIR selects persistent storage (default: run-cowbell). No database is deleted.
USAGE
}

while [[ "$#" -gt 0 ]]; do
    case $1 in
        --debug) export BUILD_PROFILE="debug"; shift ;;
        --release) export BUILD_PROFILE="release-fast"; shift ;;
        --core|--git-upstream|--baseline-objdef-dir)
            if [[ $# -lt 2 || -z $2 || $2 == --* ]]; then
                echo "Error: $1 requires a value" >&2
                exit 1
            fi
            case $1 in
                --core) core_dir=$2 ;;
                --git-upstream) git_upstream=$2 ;;
                --baseline-objdef-dir) baseline_dir=$2 ;;
            esac
            shift 2 ;;
        --postgres) POSTGRES=true; shift ;;
        --help|-h) usage; exit 0 ;;
        *) COMPOSE_ARGS+=("$1"); shift ;;
    esac
done

if [[ -n $git_upstream && -n $baseline_dir ]]; then
    echo "Error: choose --git-upstream or --baseline-objdef-dir, not both" >&2
    exit 1
fi

container_source_path() {
    local source_path
    source_path=$(realpath -e -- "$1") || return 1
    if [[ ! -d $source_path || $source_path != "$project_dir/"* ]]; then
        echo "Error: source directory must be inside $project_dir: $1" >&2
        return 1
    fi
    printf '/db/%s\n' "${source_path#"$project_dir/"}"
}

export IMPORT_PATH
IMPORT_PATH=$(container_source_path "$core_dir")
unset MOOR_GIT_UPSTREAM MOOR_BASELINE_OBJDEF_DIR
if [[ -n $git_upstream ]]; then export MOOR_GIT_UPSTREAM=$git_upstream; fi
if [[ -n $baseline_dir ]]; then
    export MOOR_BASELINE_OBJDEF_DIR
    MOOR_BASELINE_OBJDEF_DIR=$(container_source_path "$baseline_dir")
fi
cd -- "$project_dir"

if [[ "$POSTGRES" == true ]]; then
    export RUN_DIR="run-cowbell-postgres"
    COMPOSE+=(-f docker-compose.yml -f docker-compose.cowbell-postgres.yml)
fi

# Check for root-owned directory
if [ -d "$RUN_DIR" ] && [ "$(stat -c '%u' "$RUN_DIR" 2>/dev/null)" == "0" ]; then
    echo "Error: $RUN_DIR is owned by root. Run: sudo chown -R $(id -u):$(id -g) $RUN_DIR"
    exit 1
fi

echo "Starting mooR with Cowbell core..."
echo "Build profile: $BUILD_PROFILE"
echo "Runtime directory: $RUN_DIR"

# Ensure runtime directories exist
mkdir -p "$RUN_DIR/config" "$RUN_DIR/moor-data" "$RUN_DIR/export" "$RUN_DIR/local-share"

# Install the root npm workspace when needed.
if [ ! -d "node_modules" ]; then
    echo "Installing web workspace dependencies..."
    npm ci
fi

export USER_ID=$(id -u)
export GROUP_ID=$(id -g)
# Rootless Docker maps container root to the host user, so bind-mounted files are
# only writable, and stay owned by the invoking user, when the container runs as root.
if docker info --format '{{.SecurityOptions}}' 2>/dev/null | grep -q 'name=rootless'; then
    export USER_ID=0
    export GROUP_ID=0
fi
if [[ "$POSTGRES" == true ]]; then
    export MOOR_CONFIG_FILE="${MOOR_CONFIG_FILE:-moor-dev-postgres.yaml}"
    "${COMPOSE[@]}" up -d --wait postgres
    schema_exists=$("${COMPOSE[@]}" exec -T postgres psql -U moor -d moor -At \
        -v ON_ERROR_STOP=1 -c "SELECT 1 FROM pg_catalog.pg_namespace WHERE nspname = 'cowbell'")
    if [[ "$schema_exists" != 1 ]]; then
        echo "Initializing PostgreSQL world schema..."
        "${COMPOSE[@]}" build moor
        "${COMPOSE[@]}" run --rm --no-deps moor /moor/moor \
            --config-file="/db/$MOOR_CONFIG_FILE" \
            --storage-backend=postgres --pg-service=cowbell --pg-schema=cowbell \
            --pg-socket-dir=/var/run/postgresql --init-storage
    fi
    echo "World storage: PostgreSQL (schema cowbell; checkpoint export disabled)"
else
    export MOOR_CONFIG_FILE="${MOOR_CONFIG_FILE:-moor-dev.yaml}"
fi

"${COMPOSE[@]}" up --build "${COMPOSE_ARGS[@]}"
