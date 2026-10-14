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

# Run one benchmark in a fresh schema on an existing native PostgreSQL server.
set -euo pipefail
repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
export PGSERVICEFILE=${PGSERVICEFILE:-"$repo/run-cowbell-postgres/native/pg_service.conf"}
service=${PG_SERVICE:-cowbell}
socket=${PG_SOCKET_DIR:-"$repo/run-cowbell-postgres/native/socket"}
policy=${PG_COMMIT_POLICY:-synchronous}
profile=${MOOR_BENCH_PROFILE:-release}
if [[ ! -f "$PGSERVICEFILE" ]]; then
    echo "Missing $PGSERVICEFILE. Start native PostgreSQL with start-moor-cowbell-postgres.sh first." >&2
    exit 1
fi
command -v psql >/dev/null
# Resolve the connection before compiling. The Cowbell world uses a different schema.
PGSERVICE="$service" psql -X -h "$socket" -v ON_ERROR_STOP=1 -Atc 'SELECT 1' >/dev/null
if [[ -n ${PQ_LIB_DIR:-} ]]; then
    export LD_LIBRARY_PATH="$PQ_LIB_DIR${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
fi
schema="bench_$(date +%s%N)_$$"
created=false
cleanup() {
    if [[ "$created" != true ]]; then return; fi
    if [[ ${PG_KEEP_SCHEMA:-0} == 1 ]]; then
        echo "Retained benchmark schema: $schema"
        return
    fi
    PGSERVICE="$service" PGOPTIONS='-c client_min_messages=warning' \
        psql -X -h "$socket" -v ON_ERROR_STOP=1 -c "DROP SCHEMA \"$schema\" CASCADE" >/dev/null
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
moor=(cargo run --locked --manifest-path "$repo/Cargo.toml" --profile "$profile" -p moorc --features postgres --)
storage=(--storage-backend=postgres --pg-service="$service" --pg-socket-dir="$socket"
    --pg-schema="$schema" --pg-commit-policy="$policy")
"${moor[@]}" "${storage[@]}" --init-storage
created=true
echo "Benchmark storage: PostgreSQL; commit policy: $policy; schema: $schema"
"${moor[@]}" "${storage[@]}" "$@"
