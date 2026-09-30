# Persistence step 5: libpq adapter

Step 5 adds the optional PostgreSQL client and its build requirements. The relation schema, writer,
startup loader, and recovery remain in step 6. Snapshot connection limits and export operations
remain in step 7.

## Build and selection

The `postgres` feature is optional in `moor-db`. `moor-daemon`, `moor-server`, `moorc`, and
`moor-emh` forward this feature. The kernel, shared value crates, and networking hosts have no
backend feature. Default builds use Fjall and do not build or link libpq.

All four binaries accept `--storage-backend fjall|postgres`. Without the feature, PostgreSQL
selection reports that client support is disabled. With the feature, selection reports that
PostgreSQL world storage is not yet available. Both errors occur before database opening. PostgreSQL
service, schema, and world-opening arguments belong to step 6.

`pq-sys` is pinned to 0.7.6 with only its `pkg-config` feature. Its license is MIT OR Apache-2.0,
compatible with the project's AGPL-3.0 license. Its direct runtime dependency is `libc`. Build
discovery uses `pkg-config`, with `vcpkg` used on Windows MSVC targets. The selected configuration
does not use bindgen or bundled PostgreSQL sources. The compiler and JSON dependencies are optional
and reserved for PostgreSQL row codecs.

The adapter uses Unix `poll` and supports Linux builds for amd64 and arm64. Local execution covered
arm64. CI covers amd64. Windows is not supported by this adapter. Other Unix targets need validation
before support is claimed. The standalone daemon has no Tokio dependency with or without `postgres`.
The combined server retains its existing Tokio dependencies.

Sources: [pq-sys 0.7.6](https://docs.rs/crate/pq-sys/0.7.6),
[libpq build discovery](https://github.com/sgrif/pq-sys).

## Connection contract

`PostgresConnection` owns one `PGconn` and cannot implement `Send` or `Sync`. Create and use each
connection on its persistence or administrative worker. Ordinary transaction workers must not call
this adapter. Each query result owns its `PGresult` until all required fields are copied. Parameter
buffers stay alive throughout the operation.

Connections require libpq 17 or newer and a PostgreSQL 17 or 18 server. Both the database and client
encoding must be UTF8. Connection setup enforces the application name and `search_path=pg_catalog`.
`PostgresSchema` validates identifiers and quotes schema and relation names separately. Values use
parameters. Schema identifiers must never come from value interpolation.

`PostgresEndpoint::Tcp` takes a numeric IP address. The libpq `host` setting remains available for
TLS hostname verification and password-file matching. `PostgresEndpoint::Unix` takes an absolute
socket directory. These explicit endpoints avoid blocking DNS during connection setup. GSS
encryption and GSS/SSPI authentication are disabled because they can perform separate hostname
lookups.

Libpq handles connection strings, URIs, TLS options, and password files. Service connections require
`PGSERVICEFILE`, with the selected service defined in that file. LDAP service lookup is rejected
because it can block before socket polling starts. Keep the service file unchanged during connection
setup. Service files, password files, certificates, and keys must reside on local storage.
Filesystem access and row callbacks must return promptly: socket deadlines cannot interrupt either
operation.

Every connection, prepare, and query operation takes an absolute `Instant` deadline. Nonblocking
send, flush, consume, and result APIs use a polling loop with shutdown checks every 25 ms. The same
deadline covers all network waits within an operation. A query error, timeout, shutdown, callback
failure, or callback panic closes the connection. Closing does not prove that a submitted write
failed to commit. The adapter never retries a write or reconnects automatically. Invalid parameters
rejected before sending do not invalidate an idle connection.

Single-row mode bounds the number of rows delivered at once. Owned rows distinguish NULL from empty
data and use explicit field lengths. The default decode limits are 256 columns and 16 MiB per row.
These limits apply after libpq receives a row. They do not cap libpq's network buffers or prevent a
large row allocation. Callers must also bound SQL results and avoid retaining an entire scan in the
callback. COPY, pipeline mode, and multi-statement execution are unsupported.

Adapter errors retain SQLSTATE but discard server messages, SQL, credentials, and connection
details. Connection options have a redacted `Debug` implementation. Server notices are suppressed.

Source:
[libpq connection polling and configuration](https://www.postgresql.org/docs/17/libpq-connect.html).

## Development

Install libpq 17 development and runtime packages before an enabled build. On Debian 12 or Ubuntu
24.04, the provisioning script uses the signed PostgreSQL Apt repository:

```sh
sudo bash scripts/install-libpq.sh build
cargo build -p moor-daemon --features postgres
```

The script changes Apt configuration and installs packages. Default builds do not need this script.
The runtime mode installs `libpq5` without development headers:

```sh
sudo bash scripts/install-libpq.sh runtime
```

For an existing installation, `pq-sys` can use `pkg-config`, `pg_config`, or `PQ_LIB_DIR`. The
loader must also find `libpq.so.5` and its dependencies at runtime.

The probe connects without opening world storage:

```sh
export MOOR_PG_CONNINFO='service=moor_admin'
export MOOR_PG_ADDRESS=127.0.0.1
export PGSERVICEFILE=/path/to/pg_service.conf
export PGPASSFILE=/path/to/pgpass
cargo run -p moor-db --features postgres --example postgres_probe
```

The service file supplies the database, username, port, hostname, and TLS settings. Use
`sslmode=verify-full` and a trusted root certificate for TCP deployments. Keep the password file
private to its owner.

## Containers and Debian packages

The existing Dockerfiles accept `POSTGRES=true`:

```sh
docker build --target backend --build-arg POSTGRES=true -t moor-postgres .
docker build -f Dockerfile.arm64-cross --build-arg POSTGRES=true -t moor-postgres-arm64 .
```

Enabled builds install libpq 17 headers and libraries and enable the Cargo feature. The runtime
image includes libpq 17 and its native dependencies. The default `POSTGRES=false` path does not
install libpq.

Debian variants keep the existing package names and service paths. Their dependencies include
`libpq5 (>= 17)` even when the linker removes unused adapter code. This client-only stage does not
yet call libpq from daemon world opening.

```sh
cargo deb -p moor-daemon --variant postgres
cargo deb -p moor-server --variant postgres
cargo deb -p moorc --variant postgres
cargo deb -p moor-emh --variant postgres
```

The two package build scripts also accept `POSTGRES=true` in their environment. Their default
behavior stays unchanged. PostgreSQL-enabled packages require a repository that supplies libpq 17 or
newer on the target system.

Sources: [PostgreSQL Apt setup](https://www.postgresql.org/download/linux/debian/),
[cargo-deb variants](https://github.com/kornelski/cargo-deb#advanced-usage).

## Cross-compilation

Install libpq development libraries for the target architecture, not the build host.
`Dockerfile.arm64-cross` installs `libpq-dev:arm64` and copies the target runtime libraries into its
image. For a separate toolchain, set the target linker and discovery paths:

```sh
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
export PQ_LIB_DIR_AARCH64_UNKNOWN_LINUX_GNU=/path/to/sysroot/usr/lib/aarch64-linux-gnu
cargo build --target aarch64-unknown-linux-gnu -p moor-daemon --features postgres
```

Other native dependencies also require target libraries and the existing cross-compilation setup. A
feature-enabled binary needs target libpq runtime libraries on the deployment system.

## Validation

Run the dependency checks independently for each package to avoid workspace feature unification:

```sh
python3 scripts/check-postgres-dependencies.py
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p moor-db --features postgres --lib --tests
cargo test -p moor-db --features postgres --doc
scripts/test-postgres-adapter.sh 17
scripts/test-postgres-adapter.sh 18
```

The fixture scripts require Docker, OpenSSL, and libpq 17 or newer on the test host. They create
temporary TLS certificates, a random password, a service file, and a password file. Each script
removes its own container and temporary files on exit. The tests cover prepared parameters, NULL and
empty values, binary data, row streaming, result limits, and SQLSTATE errors. They also cover
disconnects, deadlines, shutdown, callback failures, TLS authentication, and encoding rejection.
Compile-fail documentation tests enforce thread ownership.

CI tests both server majors with the minimum client major, libpq 17. It checks the default daemon
build with an invalid libpq discovery path and inspects its linked libraries. The probe binary
verifies enabled libpq linkage. The existing workspace build, tests, documentation tests, and Clippy
job also enable the feature.

The original pre-refactor Fjall performance gate remains open. This step changes no transaction or
persistence hot path and does not claim to close that gate.

## Local validation record

Validation ran on Linux arm64 with libpq 17.11 and servers 17.11 and 18.6. Each server passed nine
live adapter tests, including TLS, Unix endpoint overrides, and a stalled upload. The database suite
passed 250 tests, with one existing stress test and nine live tests excluded from the ordinary run.
Both thread-ownership documentation tests passed. The daemon library and binary passed 137 tests.
The other CLI and combined-server tests also passed.

The broader CLI run found an outdated UUID literal expectation in `moor-emh`. Its expected string
now includes the canonical `#` prefix from the readable-codec step. This correction changes no
runtime parsing behavior.

Default and enabled builds passed for all four binaries. Default binaries had no libpq linkage. The
enabled probe linked libpq 17 and connected to both servers. Both backend-selection errors occurred
before filesystem side effects. The dependency graph checks, workspace Clippy, Rust formatting,
source license checks, and ShellCheck passed.

The libpq installer passed in disposable Debian 12 and Ubuntu 24.04 containers. A
development-profile `moorc` Debian package retained its package name and declared `libpq5 (>= 17)`.
The native Dockerfile passed the Docker build check. The ARM64 Dockerfile retained two existing
warnings about its explicit platform constants. Full release images and an x86-to-ARM64 cross-build
were not executed in this local validation. The CI jobs are configured but have not run remotely for
these uncommitted changes.
