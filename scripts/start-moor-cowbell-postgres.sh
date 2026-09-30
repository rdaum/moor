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

# Run Cowbell in the native monolith with a private PostgreSQL cluster.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

profile=dev
while [[ $# -gt 0 ]]; do
    case "$1" in
        --debug) profile=dev; shift ;;
        --release) profile=release-fast; shift ;;
        --help|-h)
            cat <<'HELP'
Usage: scripts/start-moor-cowbell-postgres.sh [--debug|--release] [-- moor arguments]

Builds moor with Cargo and runs Cowbell using native PostgreSQL. No containers.
Requires PostgreSQL 17 or 18 and libpq 17+ development/runtime libraries.
Defaults to a debug build. PostgreSQL stays running when moor exits.

Environment:
  PG_BIN          PostgreSQL bin directory (otherwise auto-detected)
  PQ_LIB_DIR      Optional directory containing libpq.so and libpq.so.5
  MOOR_RUN_DIR    Runtime directory (default: run-cowbell-postgres/native)
  MOOR_CONFIG_FILE  Server config (default: moor-dev-postgres.yaml)

On Ubuntu/Debian, install prerequisites explicitly if needed:
  sudo bash scripts/install-libpq.sh build
  sudo apt-get install postgresql-17
HELP
            exit 0 ;;
        --) shift; break ;;
        *) break ;;
    esac
done

if [[ -z ${PG_BIN:-} ]]; then
    for candidate in /usr/lib/postgresql/18/bin /usr/lib/postgresql/17/bin; do
        if [[ -x "$candidate/postgres" ]]; then PG_BIN=$candidate; break; fi
    done
fi
if [[ -z ${PG_BIN:-} ]] && command -v pg_config >/dev/null; then
    PG_BIN=$(pg_config --bindir)
fi
for command in postgres initdb pg_ctl psql; do
    if [[ ! -x "${PG_BIN:-}/$command" ]]; then
        echo "Missing native PostgreSQL tools. Install PostgreSQL 17 or 18, or set PG_BIN. See --help." >&2
        exit 1
    fi
done
case "$("$PG_BIN/postgres" --version)" in
    *" 17."*|*" 18."*) ;;
    *) echo "PostgreSQL 17 or 18 is required; found $("$PG_BIN/postgres" --version). See --help." >&2; exit 1 ;;
esac
if [[ -n ${PQ_LIB_DIR:-} ]]; then
    export LD_LIBRARY_PATH="$PQ_LIB_DIR${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
elif ! pkg-config --atleast-version=17 libpq; then
    echo "libpq 17+ development/runtime libraries are required. Install them or set PQ_LIB_DIR. See --help." >&2
    exit 1
fi

run_dir=${MOOR_RUN_DIR:-"$PWD/run-cowbell-postgres/native"}
mkdir -p "$run_dir"
run_dir=$(cd "$run_dir" && pwd)
pg_data="$run_dir/postgres"
pg_socket="$run_dir/socket"
if (( ${#pg_socket} > 90 )); then
    echo "Unix socket path is too long; set MOOR_RUN_DIR to a shorter absolute path." >&2
    exit 1
fi
mkdir -p "$pg_socket" "$run_dir/config" "$run_dir/local-share"
chmod 700 "$pg_socket"
export XDG_CONFIG_HOME="$run_dir/config"
export XDG_DATA_HOME="$run_dir/local-share"
export PGSERVICEFILE="$run_dir/pg_service.conf"
cat >"$PGSERVICEFILE" <<SERVICE
[cowbell]
host=$pg_socket
port=5432
user=moor
dbname=postgres
sslmode=disable
SERVICE

cargo_args=(--locked -p moor-server --bin moor --features postgres --profile "$profile")
cargo build "${cargo_args[@]}"

if [[ ! -f "$pg_data/PG_VERSION" ]]; then
    "$PG_BIN/initdb" -D "$pg_data" -U moor --encoding=UTF8 --no-locale \
        --auth-local=trust --auth-host=reject
fi
if ! "$PG_BIN/pg_ctl" -D "$pg_data" status >/dev/null 2>&1; then
    "$PG_BIN/pg_ctl" -D "$pg_data" -l "$run_dir/postgres.log" \
        -o "-c listen_addresses='' -k '$pg_socket'" -w start
fi

storage_args=(--config-file="${MOOR_CONFIG_FILE:-moor-dev-postgres.yaml}"
    --storage-backend=postgres --pg-service=cowbell --pg-schema=cowbell
    --pg-socket-dir="$pg_socket")
schema_exists=$("$PG_BIN/psql" -X -h "$pg_socket" -U moor -d postgres -At \
    -v ON_ERROR_STOP=1 -c "SELECT 1 FROM pg_catalog.pg_namespace WHERE nspname = 'cowbell'")
if [[ "$schema_exists" != 1 ]]; then
    cargo run "${cargo_args[@]}" -- "${storage_args[@]}" --init-storage
fi

printf '\nConnect with: %q -h %q -U moor -d postgres\n' "$PG_BIN/psql" "$pg_socket"
printf 'Stop PostgreSQL with: %q -D %q -m fast -w stop\n\n' "$PG_BIN/pg_ctl" "$pg_data"
exec cargo run "${cargo_args[@]}" -- "$run_dir/moor-data" "${storage_args[@]}" \
    --import="$PWD/cores/cowbell/src" --import-format=objdef \
    --generate-keypair --enable-curl-worker --web-listen-address=0.0.0.0:8081 "$@"
