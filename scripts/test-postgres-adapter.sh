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

# Disposable TLS/SCRAM fixture. The caller supplies libpq 16+ through its build environment.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
version=${1:-17}
case "$version" in 16|17|18) ;; *) echo "Expected PostgreSQL major 16, 17, or 18" >&2; exit 2;; esac
backend=${2:-docker}
case "$backend" in docker|native) ;; *) echo 'Expected docker or native fixture' >&2; exit 2;; esac
unset MOOR_PG_TEST_CONTAINER_ID MOOR_PG_TEST_NATIVE_DATA MOOR_PG_TEST_NATIVE_BIN
if [[ "$backend" == native ]]; then
    PG_BIN=${PG_BIN:-/usr/lib/postgresql/$version/bin}
    [[ $("$PG_BIN/postgres" --version) == *" $version."* ]]
fi
fixture=$(mktemp -d)
chmod 755 "$fixture"
mkdir "$fixture/socket"
chmod 777 "$fixture/socket"
container=""
cleanup() {
    if [[ -n "$container" ]]; then docker rm -f "$container" >/dev/null; fi
    if [[ "$backend" == native && -f "$fixture/data/postmaster.pid" ]]; then
        "$PG_BIN/pg_ctl" -D "$fixture/data" -m immediate -w stop >/dev/null || true
    fi
    rm -rf "$fixture"
}
trap cleanup EXIT
openssl req -x509 -newkey rsa:2048 -nodes -days 1 -subj /CN=localhost \
    -addext subjectAltName=DNS:localhost,IP:127.0.0.1 \
    -keyout "$fixture/server.key" -out "$fixture/server.crt" >/dev/null 2>&1
# The container init process copies this key to a private file owned by postgres.
chmod 644 "$fixture/server.key"
cat >"$fixture/init.sh" <<'INIT'
#!/usr/bin/env bash
set -euo pipefail
cp /fixture/server.key /fixture/server.crt "$PGDATA/"
chmod 600 "$PGDATA/server.key"
cat >>"$PGDATA/postgresql.conf" <<'CONFIG'
ssl = on
ssl_cert_file = 'server.crt'
ssl_key_file = 'server.key'
unix_socket_directories = '/moor-socket'
CONFIG
psql -v ON_ERROR_STOP=1 --username postgres --dbname postgres <<'SQL'
CREATE DATABASE adapter_latin1 ENCODING 'LATIN1' LC_COLLATE 'C' LC_CTYPE 'C' TEMPLATE template0;
SQL
INIT
chmod 755 "$fixture/init.sh"
password=$(openssl rand -hex 24)
# Do not put credentials in Docker arguments or test output.
printf 'POSTGRES_PASSWORD=%s\nPOSTGRES_INITDB_ARGS=--auth-host=scram-sha-256\n' "$password" >"$fixture/docker.env"
chmod 600 "$fixture/docker.env"
if [[ "$backend" == native ]]; then
    # A private cluster and socket directory never use the Cowbell data directory.
    chmod 700 "$fixture" "$fixture/socket"
    printf '%s\n' "$password" >"$fixture/password"
    chmod 600 "$fixture/password" "$fixture/server.key"
    "$PG_BIN/initdb" -D "$fixture/data" -U postgres --encoding=UTF8 --no-locale \
        --auth-local=trust --auth-host=scram-sha-256 --pwfile="$fixture/password" >/dev/null
    port=$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])')
    cat >>"$fixture/data/postgresql.conf" <<CONFIG
listen_addresses = '127.0.0.1'
port = $port
unix_socket_directories = '$fixture/socket'
ssl = on
ssl_cert_file = '$fixture/server.crt'
ssl_key_file = '$fixture/server.key'
CONFIG
    "$PG_BIN/pg_ctl" -D "$fixture/data" -l "$fixture/server.log" -w start >/dev/null
    "$PG_BIN/psql" -X -h "$fixture/socket" -p "$port" -U postgres -d postgres -v ON_ERROR_STOP=1 \
        -c "CREATE DATABASE adapter_latin1 ENCODING 'LATIN1' LC_COLLATE 'C' LC_CTYPE 'C' TEMPLATE template0" >/dev/null
    export MOOR_PG_TEST_NATIVE_DATA="$fixture/data" MOOR_PG_TEST_NATIVE_BIN="$PG_BIN"
    export MOOR_PG_TEST_SOCKET_PORT="$port"
else
    data_path=/var/lib/postgresql
    if [[ "$version" != 18 ]]; then data_path=/var/lib/postgresql/data; fi
    container=$(docker run --detach --publish 127.0.0.1::5432 \
        --env-file "$fixture/docker.env" --mount "type=bind,src=$fixture,dst=/fixture,readonly" \
        --mount "type=bind,src=$fixture/init.sh,dst=/docker-entrypoint-initdb.d/10-adapter.sh,readonly" \
        --mount "type=bind,src=$fixture/socket,dst=/moor-socket" \
        --tmpfs "$data_path" --entrypoint bash "postgres:$version-bookworm" -c \
        'while true; do /usr/local/bin/docker-entrypoint.sh postgres & wait "$!"; sleep 0.1; done')
    port=$(docker inspect --format '{{(index (index .NetworkSettings.Ports "5432/tcp") 0).HostPort}}' "$container")
    ready=false
    for ((attempt=0; attempt<60; attempt++)); do
        if docker exec "$container" pg_isready -h 127.0.0.1 -U postgres >/dev/null 2>&1; then ready=true; break; fi
        if [[ $(docker inspect --format '{{.State.Running}}' "$container") != true ]]; then break; fi
        sleep 1
    done
    if [[ "$ready" != true ]]; then docker logs "$container" >&2; exit 1; fi
    export MOOR_PG_TEST_CONTAINER_ID="$container" MOOR_PG_TEST_SOCKET_PORT=5432
fi
cat >"$fixture/service.conf" <<SERVICE
[moor_adapter]
host=localhost
port=$port
user=postgres
dbname=postgres
sslmode=verify-full
sslrootcert=$fixture/server.crt
SERVICE
printf 'localhost:%s:*:postgres:%s\n' "$port" "$password" >"$fixture/pgpass"
chmod 600 "$fixture/pgpass"
export PGSERVICEFILE="$fixture/service.conf" PGPASSFILE="$fixture/pgpass"
export MOOR_PG_TEST_CONNINFO='service=moor_adapter' MOOR_PG_TEST_TLS=1
export MOOR_PG_TEST_SOCKET="$fixture/socket"
cargo test --locked -p moor-db --features postgres --test postgres_adapter --test postgres_storage --test snapshot_contract -- --ignored
cargo test --locked -p moor-db --features postgres --lib provider::postgres:: -- --ignored
cargo test --locked -p moor-db --features postgres --test postgres_restart -- --ignored --test-threads=1
if [[ ${MOOR_PG_TEST_CLI:-0} == 1 ]]; then
    cargo build --locked -p moor-daemon -p moor-server -p moorc -p moor-emh --features postgres
    python3 scripts/test-postgres-cli.py
fi
