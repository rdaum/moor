# Server Configuration

This section describes the options available for configuring and running the mooR server.

For most deployments, the combined `moor` binary handles everything — database, verb execution,
telnet connections, web API, and outbound HTTP — in one process. The configuration options below
apply to that binary unless noted otherwise. Options specific to split-process or clustered
deployment are called out in their own sections.

For a deeper discussion of mooR's threading model, database concurrency model, performance counters,
and tuning guidance, see [Performance and Concurrency](performance-and-concurrency.md).

## Single-Process Configuration

The `moor` binary is the default way to run the server. It bundles the daemon, telnet host, web
host, and optional HTTP and Git worker services into one process. No worker sockets or enrollment
are needed. This is the closest analogue to running a traditional LambdaMOO or ToastStunt server.

To start the server:

```bash
moor /path/to/database --db development.db --generate-keypair
```

The same configuration file and most command-line options described in this page apply to the
combined `moor` binary. The transport endpoint and enrollment options (described below) are not
needed in single-process mode — they only apply when running components as separate processes.

Enable outbound HTTP with `--enable-curl-worker` and read-only Git access with
`--enable-git-worker`. Git requests run in isolated child processes of the same `moor` executable.
They use the same protocol and resource limits as the standalone Git worker. No Git executable is
needed for worker requests. Import preparation with `--git-upstream` separately requires Git and
local repository history.

You can configure the Git service in YAML:

```yaml
services:
  git_worker:
    enabled: true
    work_dir: git-jobs
    max_concurrent_requests: 4
    limits:
      max_seconds: 30
      max_entries: 4096
      max_file_bytes: 4194304
      max_total_bytes: 16777216
      max_fetch_bytes: 268435456
      max_memory_bytes: 1073741824
```

Relative job directories resolve under the server data directory. Job directories are temporary and
removed when each request finishes. Limits apply to individual requests; excess concurrent work
receives a busy response. Invalid limits or inaccessible job storage prevent startup before database
import. Shutdown terminates active Git child processes before the worker detaches.

## PostgreSQL requirements

The optional `postgres` Cargo feature requires a thread-safe libpq 16 or newer. Supported servers
are PostgreSQL 16, 17, and 18. The client and server major versions can differ. Default builds use
Fjall and do not require libpq.

On Debian or Ubuntu, install the build dependency with:

```bash
sudo bash scripts/install-libpq.sh build
```

The installer retains compatible installed libraries. If development headers are absent, it uses
compatible distribution packages. It adds the PostgreSQL package repository only when necessary. For
runtime images, use `runtime` instead of `build`. PostgreSQL package variants depend on
`libpq5 (>= 16)`.

Exact client versions are for compatibility tests. For example, this command can downgrade an
existing client installation:

```bash
sudo bash scripts/install-libpq.sh build "$(dpkg --print-architecture)" 16
```

The adapter checks `PQisthreadsafe()` before connection setup. Each connection belongs to one worker
thread. The connection options include `require_auth`, which requires libpq 16. The adapter uses
nonblocking connections, parameterized queries, and single-row results. It does not require the
cancellation or chunk APIs added in later versions. See the PostgreSQL documentation for
[thread safety](https://www.postgresql.org/docs/16/libpq-threading.html) and
[connection options](https://www.postgresql.org/docs/16/libpq-connect.html).

The schema uses domains, JSONB, advisory locks, and conflict handling supported by PostgreSQL 16.
The compatibility suite exercises schema creation, startup, prepared writes, recovery, and both SQL
commit policies. It also runs the shared engine, loader, garbage collection, and permission tests
against PostgreSQL. The normal database unit suite runs those tests against Fjall. Each shared
database fixture checks a durability fence before a successful test ends.

The PostgreSQL fixture tests process and server crashes under both commit policies. The process test
kills a writer after publication while SQL is blocked. Reopening must preserve the fenced prefix and
permit new appends. Live tests are ignored in ordinary `cargo test` runs; the fixture script runs
them explicitly. Shuttle scheduler tests retain their simulated Fjall fixture.

For a disposable local fixture with TLS and SCRAM authentication, run:

```bash
scripts/test-postgres-adapter.sh 16 native
```

Set `PG_BIN` if the server tools are outside `/usr/lib/postgresql/16/bin`. The fixture creates a
private temporary cluster and removes it after the tests. It also tests server crashes. It does not
use the Cowbell cluster. Use `17` or `18` to test those server versions. Omit `native` for Docker.
Set `MOOR_PG_TEST_CLI=1` to include builds and configuration checks for all four storage-aware
tools. Set `LD_LIBRARY_PATH` to test a separately installed libpq runtime.

### PostgreSQL deployment configuration

Initialize the schema through the setup role before starting the runtime. Use the role grants in the
recovery section below. Keep credentials in a libpq service file and a separate password file. The
adapter requires an explicit `PGSERVICEFILE` for service lookup. It rejects LDAP service lookup. The
selected endpoint must be a numeric address or an absolute Unix socket directory.

For a local Unix socket, define this service in `/etc/moor/pg_service.conf`:

```ini
[world_socket]
host=/run/postgresql
port=5432
dbname=world
user=world_runtime
```

Configure the host in `/etc/moor/moor.yaml`:

```yaml
storage:
  backend: postgres
  shutdown_timeout_seconds: 60
  postgres:
    service: world_socket
    schema: moor
    socket_dir: /run/postgresql
    commit_policy: synchronous
```

Use socket permissions and PostgreSQL authentication rules appropriate for the runtime account. The
Cowbell launcher uses a separate development configuration; it is not a production authentication
example.

For TCP with TLS, use a certificate-verified service:

```ini
[world_tls]
host=db.example.net
port=5432
dbname=world
user=world_runtime
sslmode=verify-full
sslrootcert=/etc/moor/postgres-ca.pem
```

Replace `service` and `socket_dir` in the YAML with:

```yaml
service: world_tls
hostaddr: 192.0.2.10
```

The numeric address selects the server. The service hostname remains the certificate identity.
Provision the matching password entry in `/run/secrets/moor.pgpass`, with permissions `0600`. Set
`PGPASSFILE` to that file. Keep passwords out of command-line arguments and YAML.

Specify local ancillary stores independently of PostgreSQL world storage:

```bash
export PGSERVICEFILE=/etc/moor/pg_service.conf
export PGPASSFILE=/run/secrets/moor.pgpass
moor /srv/moor/local --config-file /etc/moor/moor.yaml \
  --connections-file /srv/moor/local/connections.db \
  --tasks-db /srv/moor/local/tasks.db \
  --events-db /srv/moor/local/events.db
```

The host also needs its configured keys and network listeners. The positional data directory remains
local. Do not pass `--db` with PostgreSQL; that argument selects a Fjall world directory.

One mooR writer owns one schema on one PostgreSQL database. Connections and recovery attempts use
the configured endpoint. The adapter does not select replicas, resolve changing DNS addresses, or
coordinate failover across independent clusters. PostgreSQL advisory locks do not fence writers on
different clusters.

For an endpoint change, stop the old writer and check its persistence drain. Fence access to the old
server before promoting or restoring another server. Configure the new numeric address or socket,
service identity, credentials, and TLS trust. Validate the target and rehearse recovery before
starting one writer. Keep local stores consistent with the selected world recovery point. Do not
change the service file during connection setup.

### Persistence deadlines

Set `--persistence-shutdown-timeout-seconds` or YAML `storage.shutdown_timeout_seconds` to control
the persistence shutdown budget. This applies to both backends and all four storage tools. The
default is 30 seconds. Values must be positive finite durations; explicit CLI values override YAML.
The budget covers draining admitted work and stopping persistence workers. It is not the total
process shutdown budget, which also includes scheduler and host shutdown.

Admission thresholds are runtime properties on `$server_options`:

| Property                          | Default | Effect                                        |
| --------------------------------- | ------- | --------------------------------------------- |
| `db_commit_queue_warn_seconds`    | `1`     | Log a warning for a blocked admission episode |
| `db_commit_queue_timeout_seconds` | `5`     | Reject a blocked commit with `E_QUOTA`        |

These properties accept non-negative integer or floating-point seconds. Zero timeout rejects a
commit immediately when no permit is available. A warning threshold above the timeout is clamped to
the timeout. After changing the properties, call the wizard-only `load_server_options()` builtin.
Changing admission thresholds does not change SQL query or recovery deadlines.

| PostgreSQL control              | Default | Scope                                                               |
| ------------------------------- | ------- | ------------------------------------------------------------------- |
| `--pg-connect-timeout-seconds`  | `10`    | One connection attempt                                              |
| `--pg-query-timeout-seconds`    | `30`    | A SQL operation; also the separate prepublication encoding wait     |
| `--pg-recovery-timeout-seconds` | `30`    | Reconnection, progress checks, and replay after a recoverable error |
| `--pg-retry-interval-ms`        | `50`    | Delay between recovery attempts                                     |

A transaction reserves a commit slot, completes PostgreSQL encoding, then reserves payload bytes
before publication. Each admission wait uses the current runtime policy; encoding has a separate
deadline. SQL application occurs after publication. Recovery begins after an application error; each
connection or SQL attempt also respects the remaining recovery deadline. Snapshot acquisition has
its own overall deadline, and subsequent reader requests use the query limit.

A shutdown budget can expire before recovery completes. That outcome is an error, not a successful
drain. Size the service manager's stop allowance to include scheduler shutdown and the configured
persistence budget. Inspect applied progress and shutdown errors before treating the stopped world
as a complete backup boundary. Asynchronous SQL application still does not establish WAL durability.

Writer groups currently retain their fixed defaults: 64 commits, 1 MiB, 4,096 operations, and a 1 ms
collection window. Group sizing uses measured encoded JSON bytes, with an allowance for SQL keys and
record sequences. The SQL planner checks the final size again after expanding append rollups. An
indivisible commit can exceed a normal group limit. Admission capacity is 1,000 commits. PostgreSQL
also has the payload byte target described below. More queue capacity cannot resolve sustained SQL
application lag. Use the persistence diagnostics to measure the workload before changing these
implementation limits.

### PostgreSQL write limits

Before publication, an encoder worker renders and validates each PostgreSQL write. A rejected write
leaves the published world unchanged. Accepted writes retain their encoded rows for asynchronous SQL
application. A successful transaction acknowledgment still does not establish durability.

The `--pg-max-row-bytes` option limits the complete returned JSON row, including column names,
whitespace, literal escaping, and JSON escaping. Its default is 16 MiB. An 8 MiB string of
backslashes exceeds this default after escaping. The limit also applies to verb source, definitions,
names, and metadata. Values and programs must pass the versioned persistence codecs. The literal
codec permits at most 64 nesting levels, including nested container values and captured lambda
values.

Every property append must fit as both a suffix row and a complete replacement. This check permits
later rollups under the same configuration. Property checks reserve 19 decimal digits for the future
record sequence. Changing the row limit to a smaller value can prevent an existing world from
opening.

Append validation caches size measurements for up to 4096 property versions. A matching base version
lets the encoder validate only the new suffix and calculate the complete row size. The cache retains
no property values. After restart or eviction, the next append validates the complete value before
caching its measurements. Rollups still encode the complete value.

Each logical commit has a 256 MiB encoded payload budget. The budget includes possible complete
property rollups, 128 bytes per mutation for keys and framing, and 1024 bytes for sequence updates.
The writer can exceed its normal group budget for one indivisible commit, but this commit limit
still applies.

Set `--pg-max-pending-bytes` or YAML `storage.postgres.max_pending_bytes` to bound admitted payload
estimates. The default is 67108864 bytes (64 MiB); zero is invalid. Credit covers encoded JSON,
delete keys, and retained complete append values, including preparations waiting for publication.
For append values, the estimate uses the larger of their logical size and serialized literal size.
Shared allocations can be counted repeatedly. Each commit also reserves 1024 bytes for later
sequence fields.

A valid indivisible commit larger than the target can proceed alone when no credit is in use. Credit
returns when SQL applies the commit or the transaction abandons publication. Byte waits use the
admission policy captured for that preparation; a timeout returns `E_QUOTA` before publication.
Writer failure and shutdown cancel blocked waits.

Each encoder can hold one completed payload outside the byte target while waiting for admission.
There are at most eight encoders. Resident data, transaction snapshots, queued values, scratch
space, and allocation overhead also consume memory. The byte target is not a process memory ceiling.

The PostgreSQL query timeout also bounds the wait for preparation before publication. Preparation
errors and timeouts return a transaction error. They do not publish changes or disable the writer.

### PostgreSQL snapshots and objdef export

`moorc --out-objdef-dir` and the `moor-emh` export command use the shared snapshot loader with
PostgreSQL. Acquisition captures the current publication and waits for its applied prefix. It then
reserves a reader slot and opens a dedicated read-only, repeatable-read transaction. The reader
checks the writer epoch and applied prefix in that transaction. One deadline covers all acquisition
steps; the default snapshot API allows 10 seconds. An explicit zero deadline returns `ResourceBusy`
without starting a reader.

The snapshot can include later publications. Both naming metadata and object export use the same
transaction, so later writes cannot change an export in progress. Existing sparse inherited-property
rules apply, including value-only, permission-only, and metadata-only local overrides.

Set `--pg-max-exports` or YAML `storage.postgres.max_exports` to limit reader connections. The
default is two; supported values are 1 through 64. Callers wait in arrival order for capacity,
within the acquisition deadline. A loader holds its slot until it is dropped, including idle time
before export. Each fetch has its own query deadline. An acquired snapshot can outlive writer
shutdown.

Relation scans use server-side cursors with at most eight returned rows per fetch. Each row remains
subject to the configured row limit. Export retains naming, parent, and property-definition indexes,
fetch buffers, and the current object's payload. It compiles stored verb source for the shared
objdef interface. This does not establish a constant memory bound for an entire export.

A lost connection ends the snapshot; readers never reconnect within an export. Restart a failed
export. Failed output remains in its `.in-progress` directory until you remove it or choose another
output path. Closing the reader connection releases the transaction without a rollback round trip.

The `db_counters()` values `persistence_postgres_active_exports`,
`persistence_postgres_export_limit`, and `persistence_postgres_oldest_export_micros` report reader
slots and the age of the oldest reservation. Reservations include connection setup. Long-held read
transactions retain old row versions in PostgreSQL; close unused loaders promptly.

### PostgreSQL inspection views

New schemas include nine inspection views. For an existing schema, stop its writer and install the
views through the setup service:

```bash
moorc --storage-backend postgres --pg-service world_setup \
  --pg-hostaddr 192.0.2.10 --pg-schema moor --install-storage-views
```

Installation is transactional. It replaces inspection objects without rewriting world rows or
changing the writer epoch. It requires schema ownership and fails while a writer holds the schema
lock. Ordinary startup does not install or update views. These optional projections do not change
the stored schema version.

| View                   | Content                                                          |
| ---------------------- | ---------------------------------------------------------------- |
| `objects`              | Identity, name, owner, parent, location, and flags               |
| `verb_definitions`     | Ordered definitions, UUIDs, names, owners, flags, and arguments  |
| `verb_names`           | One name per row, with definition and name ordinals              |
| `verbs`                | Definitions joined to stored source                              |
| `property_definitions` | Ordered definitions, names, UUIDs, and defining locations        |
| `property_permissions` | Stored local permission rows with available property names       |
| `property_records`     | Physical full and append rows with available property names      |
| `property_values`      | Reconstructed local values, record counts, and final timestamps  |
| `persistence_status`   | Database identity, format markers, and persisted writer progress |

Names with `name_encoding = 'json_string'` retain their JSON string representation, including NUL
escapes. Other names are UTF8 text. The `names` array in `verb_definitions` and `verbs` retains the
stored JSON name envelopes. Ordinals start at one.

The SQL `durable_fence` field counts fences. It is not the live `persistence_durable` publication
watermark.

Property views describe stored local state. They do not resolve inheritance or synthesize
permissions. An absent local value can still have an inherited value. Inactive entries left after
reparenting or property deletion remain visible; their property name can be absent.

```sql
SELECT object_ref, name, parent_ref, owner_ref FROM moor.objects WHERE object_ref = '#42';
SELECT names, source FROM moor.verbs WHERE object_ref = '#42';
SELECT property_name, value_literal, logical_timestamp
FROM moor.property_values WHERE object_ref = '#42';
```

For one large property, use the parameterized lookup to avoid definition joins:

```sql
PREPARE inspect_property(text, uuid) AS
  SELECT * FROM moor.read_property_value($1, $2);
EXECUTE inspect_property('#42', '00000000-0000-0000-0000-000000000001');
```

The lookup reads the requested key in record order. It checks the initial full record, append kinds,
list boundaries, empty suffixes, format version, and chain bounds. It joins canonical list interiors
without splitting nested values or compiling source. The final timestamp comes from the last record
by sequence, which can differ from the maximum timestamp. A missing key returns no row. Use
`--validate-storage` for complete literal parsing and semantic checks.

Inspection computes values on demand; it does not store another complete value after each append.
Cost grows with the requested value and chain size. Broad views can also scan definition arrays. The
live fixture checks targeted plans against 20,000 unrelated property keys. Set `MOOR_PG_PLAN_DIR` to
an output directory to retain its JSON execution plans. This test does not establish a production
latency budget.

Views use `security_invoker=true`, and the lookup function uses the caller's permissions. Grant the
inspection role SELECT on the views and underlying tables. The function has the default PUBLIC
EXECUTE grant; schema USAGE and table SELECT still apply. If your policy removes that grant, grant
EXECUTE on `moor.read_property_value(text, uuid)` to the inspection role. See the PostgreSQL
references for [views](https://www.postgresql.org/docs/16/sql-createview.html) and
[functions](https://www.postgresql.org/docs/16/sql-createfunction.html).

Both archive procedures below include the inspection objects. The restore fixture compares all nine
views under the inspection role before opening a writer. Direct SQL writes through tables or views
still require the offline procedure below.

### PostgreSQL validation and recovery

All four storage tools accept `--validate-storage`. This command exits before opening the runtime or
local stores. It uses a read-only, repeatable-read transaction and does not claim writer ownership
or change the writer epoch. It can run while mooR is active.

```bash
moorc --storage-backend postgres --pg-service world_inspect \
  --pg-hostaddr 192.0.2.10 --pg-schema moor --validate-storage > validation.json
```

The JSON report includes the database identity, progress counters, allocation counters, and physical
row counts for each relation. Property counts distinguish reconstructed values from physical
records. Validation checks format versions, literals, compiled verb source, property chains,
timestamps, and allocation bounds. It also checks object relationships, definition references,
canonical property permissions, and verb source pairs. Sparse inherited values and local permission
overrides are valid. Reparenting and property deletion can leave inactive local property entries.
Validation counts these separately; it does not treat them as active inherited state. Object
references inside arbitrary values and ownership fields can refer to recycled objects.

The command stops at the first error with a nonzero status. Errors identify the relation and row
key; compiler errors include a source position. Stored values and source text do not appear in
errors. Validation retains object and definition indexes, row keys, and the current property chain.
Each query has the configured query timeout; the complete validation can take longer. A successful
report describes that SQL snapshot. It does not establish a backup or inspect local stores.

#### Format compatibility and conversion

The PostgreSQL provider currently accepts one format combination:

| Format marker                                  | Supported value                                               |
| ---------------------------------------------- | ------------------------------------------------------------- |
| SQL schema                                     | `1`                                                           |
| Persistent literal format                      | `1`                                                           |
| Persistent verb-source format                  | `1`                                                           |
| Compiler profile identifier                    | `moo-v1`                                                      |
| Profile language                               | `moo`                                                         |
| Profile literal, source, and compiler versions | All `1`                                                       |
| Enabled compiler options                       | `flyweight_type`, `bool_type`, `symbol_type`, `custom_errors` |
| Disabled compiler options                      | `call_unsupported_builtins`, `legacy_type_constants`          |

Both the metadata columns and the stored profile JSON must match. Row-level literal and source
markers must also match. Startup rejects unsupported combinations before it accepts publications. It
does not rewrite format markers, convert stored rows, or create missing tables. Installing optional
inspection views does not change the authoritative schema version. Build identifiers are diagnostic;
they are not format compatibility keys.

There is no in-place SQL format upgrader. For a future incompatible schema or codec change, use
objdef export and import into a separate database or schema. The old executable must still read the
source format. A future release must qualify its old-to-new conversion before it claims support.
Current tests rehearse this procedure with version 1 on both sides; they do not prove compatibility
with an unreleased format. A compatible release can reopen the existing schema after backup and
validation. A PostgreSQL server-major upgrade is separate from mooR's stored format versions.

For an objdef conversion:

1. Stop new world writes. Export the source with its compatible executable. Keep the source
   database, executable, configuration, and local stores for rollback.
2. Initialize a separate destination with the target executable and setup role. Apply the runtime
   and inspection grants. Do not point the live service at this destination yet.
3. Import the objdef directory with the target executable. Keep the required language feature
   settings. Check import diagnostics, then run `--validate-storage` on the destination.
4. Export the destination and compare its objects, values, verb source, permissions, and metadata
   with the source export. Run application behavior probes before changing the service endpoint.
5. Change the explicit endpoint only after those checks pass. Keep the old writer stopped. If the
   new world accepts writes, restoring the old endpoint would discard those new writes.

Objdef conversion transfers world content. It does not preserve PostgreSQL database identity, writer
epochs, publication counters, physical property chains, or all physical identifiers. It also does
not transfer the local task, connection, or event stores. Use SQL backup and restore when you need
to preserve all stored SQL cells under a compatible format.

If export or import is interrupted, keep the source unchanged and leave the service endpoint there.
Discard the incomplete export directory or destination, then repeat the conversion into a fresh
location. Do not treat a successful process exit, a partly populated schema, or edited version
markers as a completed conversion. The CLI qualification suite kills a blocked destination import,
then creates a fresh destination and verifies export equivalence and behavior.

#### Database roles

Use separate setup, runtime, and inspection logins. Provision their passwords and TLS configuration
through the service and password files described above. These examples use the database `world` and
schema `moor`.

As the database administrator:

```sql
CREATE ROLE world_setup LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE;
CREATE ROLE world_runtime LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE;
CREATE ROLE world_inspect LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE;
CREATE DATABASE world OWNER world_setup TEMPLATE template0 ENCODING 'UTF8';
```

Run `--init-storage` through the setup service once. As `world_setup`, grant access to the new
schema:

```sql
GRANT USAGE ON SCHEMA moor TO world_runtime, world_inspect;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA moor TO world_runtime;
GRANT SELECT ON ALL TABLES IN SCHEMA moor TO world_inspect;
```

The runtime role does not need schema creation or table ownership. The inspection role can validate
and back up this schema. Whole-database backups require read access to any other application
schemas. These grants cover existing tables; apply grants again after an explicit schema upgrade
creates tables.

#### Logical backup and restore

A logical backup contains the applied SQL prefix visible when its snapshot starts. It can lag the
published in-memory world. In the same running mooR process, record `persistence_published` from
`db_counters()`. Wait until a consistent, healthy sample has `persistence_applied` at least that
high. Then start the dump. The dump can include later publications. If mooR restarts during this
procedure, repeat it: publication numbers belong to one writer lifetime.

For a backup that also includes local stores, stop mooR first. Check that persistence drained
without errors. Keep mooR stopped until the SQL dump and local-store copies finish. With
asynchronous SQL commits, shutdown drains application but does not itself request a WAL durability
fence.

Use PostgreSQL tools from the server's major version. Choose one archive scope:

```bash
# All schemas in this database.
pg_dump --dbname='service=world_inspect' --format=custom --file=world.dump

# Only the mooR schema, including its domains and tables.
pg_dump --dbname='service=world_inspect' --format=custom --schema=moor --file=world-schema.dump
```

`pg_dump` produces a consistent database snapshot. It does not include cluster roles or tablespaces.
Schema selection does not collect dependencies outside the selected schema. The mooR schema has no
required user-defined dependencies outside itself. Preserve separately any dependencies that you
add. See the [PostgreSQL pg_dump reference](https://www.postgresql.org/docs/16/app-pgdump.html).

Create an empty UTF8 restore database owned by the setup role. Point `world_restore` at that
isolated database. Keep its schema name unchanged, and do not run `--init-storage` there.

```bash
pg_restore --dbname='service=world_restore' --no-owner --no-acl \
  --exit-on-error --single-transaction world-schema.dump
```

The same command accepts `world.dump`. The setup login owns the restored objects. Reapply the
runtime and inspection grants in the target database. This procedure deliberately replaces archive
ownership and grants with the target roles. See the
[PostgreSQL pg_restore reference](https://www.postgresql.org/docs/16/app-pgrestore.html).

Before opening a writer, run `--validate-storage` through the target inspection service. Compare its
identity, counters, and counts with the backup record. A restore preserves the physical database
UUID, verb and property UUIDs, timestamps, allocation counters, and property records. Opening a
writer creates a new writer epoch, so compare the original epoch before that step.

Export objdef through the target runtime service with `moorc`, using an empty source directory.

```bash
mkdir restore-empty
moorc --storage-backend postgres --pg-service world_restore_runtime \
  --pg-hostaddr 192.0.2.10 --pg-schema moor \
  --src-objdef-dir restore-empty --out-objdef-dir restored.objdir
```

Compare the export with the expected world. Then run a known functional probe. Use an isolated data
directory and endpoints throughout the rehearsal. Validation and source compilation alone do not
establish that application behavior is correct.

The CLI fixture tests both archive scopes with separate roles. It compares every stored table before
writer opening, validates the restore, compares objdef files, and runs the benchmark world's append
probe. Run it with `MOOR_PG_TEST_CLI=1 scripts/test-postgres-adapter.sh 17 native`.

#### Local stores and physical recovery

PostgreSQL stores world state. Task, connection, and event databases remain separate local stores. A
world-only restore must start with fresh task and connection stores. Old suspended tasks can contain
references or assumptions from a later world. Saved connections cannot restore live network
sessions. Archive old event stores separately; they can describe actions absent from the restored
world.

For a full deployment restore, retain the stopped deployment's local stores, configuration, and keys
with the SQL backup. Document their common shutdown boundary. mooR does not provide an atomic backup
transaction across PostgreSQL and these local stores. Rehearse task resumption before reconnecting
users.

Physical recovery requires a PostgreSQL base backup and the required WAL sequence. It restores the
cluster to a PostgreSQL recovery point, not an in-memory mooR publication number. Unapplied
publications have no SQL transaction to recover. Asynchronous commits can also exceed the WAL
available after a crash. A world-state durability fence does not make the separate local stores
atomic with it. See
[PostgreSQL continuous archiving](https://www.postgresql.org/docs/16/continuous-archiving.html).

External SQL changes do not update the resident mooR world. Stop the writer before planned offline
edits. Validate the edited database, rehearse export and behavior, then restart mooR. Validation
does not repair data or authorize concurrent SQL writes.

### Persistence diagnostics

The wizard-only `db_counters()` builtin exposes live persistence status. Status values use the
existing map format: `name -> {value, 0}`. They describe the running writer, independent of the
caller's transaction snapshot.

| Name                                               | Meaning                                                           |
| -------------------------------------------------- | ----------------------------------------------------------------- |
| `persistence_published`                            | Highest observed in-memory publication                            |
| `persistence_applied`                              | Highest prefix applied by the storage writer                      |
| `persistence_durable`                              | Highest prefix with established durability                        |
| `persistence_outstanding`                          | Reserved admission slots, including unpublished transactions      |
| `persistence_healthy`                              | `1` while the writer reports healthy; otherwise `0`               |
| `persistence_sampling_consistent`                  | `1` when progress and admission passed the bounded sampling check |
| `persistence_postgres_prepared_commits`            | Validated commits waiting for publication                         |
| `persistence_postgres_unapplied_commits`           | Published commits with retained encoded payloads                  |
| `persistence_postgres_retained_encoded_bytes`      | Retained encoded JSON bytes                                       |
| `persistence_postgres_retained_append_value_bytes` | Logical size of retained complete append values                   |
| `persistence_postgres_oldest_unapplied_micros`     | Age of the oldest retained published payload, in microseconds     |
| `persistence_postgres_admission_bytes`             | Payload estimates currently holding byte credit                   |
| `persistence_postgres_admission_limit_bytes`       | Configured byte target; one indivisible commit can exceed it      |
| `persistence_postgres_admission_waiters`           | Prepared payloads waiting for byte credit                         |

Subtract applied from published to estimate application lag. Subtract durable from published to
estimate the number of publications without established durability. If
`persistence_sampling_consistent` is zero, retry before comparing lag with outstanding slots.
Progress reads follow causal order. Payload gauges use a separate locked sample and can differ from
progress during concurrent work. Neither a synchronous SQL policy nor in-memory publication makes
the foreground acknowledgment a durability fence.

Encoded byte counts exclude allocation overhead and property record sequences assigned later. Append
byte counts can count shared allocations more than once. Neither count measures resident memory.
Payload gauges fall when payloads are released, including failed or abandoned attempts.

PostgreSQL also exposes cumulative values with the `persistence_postgres_` prefix:

- `encoding_calls`, `encoding_failures`, `encoded_bytes`, and `encoding_ns` describe preparation,
  including attempts that later conflict.
- `append_validation_cache_hits` and `append_validation_cache_misses` count suffix-only and complete
  append validation. The cache starts empty each time the database opens.
- `admission_wait_ns` sums completed byte waits. `admission_timeouts` counts byte admission timeouts
  before publication.
- `sql_application_ns`, `sql_commit_ns`, and `fence_ns` separate SQL execution, COMMIT, and complete
  durability-fence time. Failed attempts contribute time. Fence time includes its SQL stages.
- `groups`, `group_commits`, `group_payload_bytes`, and `group_sql_statements` describe confirmed
  groups. Statement counts include data mutations and sequence writes, excluding transaction control
  and writer progress. Recovery retries do not count as additional confirmed groups.
- `last_group_commits`, `last_group_payload_bytes`, `last_group_sql_statements`, `last_group_first`,
  and `last_group_last` describe the last confirmed group and its publication range.
- `group_end_available`, `group_end_commit_limit`, `group_end_payload_limit`,
  `group_end_operation_limit`, `group_end_age_limit`, `group_end_fence`, and
  `group_end_rollup_expansion` count group boundaries.
- `recovery_attempts`, `recovery_ns`, `recovery_first`, and `recovery_last` describe reconnect
  attempts, cumulative recovery time including an active recovery, and the most recent affected
  publication range.

Cumulative `_ns` values are nanoseconds stored in the first element. Totals reset when the writer
restarts. The existing sampled timers also include `postgres_encode`, `postgres_apply`,
`postgres_commit`, `postgres_fence`, and `postgres_recovery`, with durations in the second element.
Diagnostics contain numeric progress, durations, and byte counts. They omit credentials and stored
values.

Phased `moorc` benchmarks emit `PERSISTENCE_SAMPLE` once per second and a final
`PERSISTENCE_OCCUPANCY` report. The final report includes peak payload gauges and the number of
inconsistent samples excluded from progress peaks.

## Daemon, Hosts, Workers, and RPC (Advanced)

For split-process or clustered deployment, the server is broken into separate binaries:

The `moor-daemon` binary provides the core server functionality — hosting the database, handling
verb executions, and scheduling tasks. It does _not_ handle network connections directly. Special
helper processes called _hosts_ manage incoming network connections and forward them to the daemon.
Likewise, outbound network connections (or future facilities like file access) are handled by
_workers_ that communicate with the daemon to perform those activities.

To run a split-process deployment, you therefore need to run not just the `moor-daemon` binary, but
also one or more "hosts" (and, optionally "workers") that will connect to the daemon.

These processes communicate over ZeroMQ sockets, with the daemon listening for RPC requests and
events, and the hosts and workers connecting to those sockets to send requests and receive
responses.

Hosts and workers can be run on the same machine as the daemon (using IPC) or distributed across
multiple machines for clustered deployments (using TCP with CURVE encryption). They are stateless
and can be restarted independently of the daemon, allowing for flexible deployment and scaling.

See [Clustered Deployment](clustered-deployment.md) for complete details on split-process and
multi-machine setups.

## Transport Modes

For single-process deployment (the default), all components run inside the `moor` binary and no
transport configuration is needed.

For split-process same-machine deployments, components run as separate processes and communicate via
**IPC (Unix domain sockets)**, which use filesystem permissions for security and require no
additional configuration.

For clustered/multi-machine deployments, components communicate via **TCP with CURVE encryption**.
See the [Clustered Deployment](clustered-deployment.md) guide for complete details on distributed
deployments, security considerations, and setup instructions.

## Authentication Keys

### PASETO Keys (Ed25519) - Client/Player Authentication

PASETO tokens authenticate **clients/players** (connecting users) using Ed25519 digital signatures.
These are used _only by the daemon_ to sign and verify player session tokens.

The daemon automatically generates these keys on first run when using the `--generate-keypair` flag:

```bash
# Keys are auto-generated on first run
moor --generate-keypair <other-args>
```

This creates `moor-signing-key.pem` (private key) and `moor-verifying-key.pem` (public key) in the
moor config directory (`${XDG_CONFIG_HOME:-$HOME/.config}/moor`).

Alternatively, you can pre-generate them using `openssl`:

```bash
openssl genpkey -algorithm ed25519 -out moor-signing-key.pem
openssl pkey -in moor-signing-key.pem -pubout -out moor-verifying-key.pem
```

**Note**: Hosts and workers do **not** need these PEM files - they are only used by the daemon for
client authentication.

## How to set server options

In general, all options can be set either by command line arguments or by configuration file. The
same option cannot be set by both methods at the same time, and if it is set by both, the command
line argument takes precedence over the configuration.

## Configuration File Format

The configuration file uses YAML format. You can specify the path to your configuration file using
the `--config-file` command-line argument. Configuration file values can be overridden by
command-line arguments.

## General Server Options

These options control the basic server behavior:

- `--config-file <PATH>`: Path to configuration (YAML) file to use. If not specified, defaults are
  used.
- `--connections-file <PATH>` (default: `connections.db`): Path to connections database
- `--tasks-db <PATH>` (default: `tasks.db`): Path to persistent tasks database
- `--public-key <PATH>` (default: `${XDG_CONFIG_HOME:-$HOME/.config}/moor/moor-verifying-key.pem`):
  PEM encoded PASETO public key for token verification
- `--private-key <PATH>` (default: `${XDG_CONFIG_HOME:-$HOME/.config}/moor/moor-signing-key.pem`):
  PEM encoded PASETO private key for token signing
- `--num-io-threads <NUM>` (default: `8`): Number of ZeroMQ IO threads
- `--debug` (default: `false`): Enable debug logging

### Transport Endpoint Configuration (Split-Process Only)

These options configure how the daemon communicates with hosts and workers when running as separate
processes. They do not apply to the combined `moor` binary, which uses in-process endpoints. The
defaults use IPC (Unix domain sockets) for same-machine split-process deployments. Change these to
TCP addresses (e.g., `tcp://0.0.0.0:7899`) only for clustered deployments - see
[Clustered Deployment](clustered-deployment.md) for details.

| Option                      | Default                                 | Description                     |
| --------------------------- | --------------------------------------- | ------------------------------- |
| `--rpc-listen`              | `ipc:///tmp/moor_rpc.sock`              | RPC server address              |
| `--events-listen`           | `ipc:///tmp/moor_events.sock`           | Events publisher address        |
| `--workers-request-listen`  | `ipc:///tmp/moor_workers_request.sock`  | Workers request pub-sub address |
| `--workers-response-listen` | `ipc:///tmp/moor_workers_response.sock` | Workers response RPC address    |

### Enrollment Configuration (Clustered Deployments Only)

These options are only needed for clustered deployments with TCP transport. See
[Clustered Deployment](clustered-deployment.md) for complete setup instructions.

| Option                    | Default                                                   | Description                                      |
| ------------------------- | --------------------------------------------------------- | ------------------------------------------------ |
| `--enrollment-listen`     | `tcp://0.0.0.0:7900`                                      | Enrollment endpoint for host/worker registration |
| `--enrollment-token-file` | `${XDG_CONFIG_HOME:-$HOME/.config}/moor/enrollment-token` | Path to enrollment token file                    |

## Database Configuration

- `<PATH>` (positional argument): Path to the database directory
- `--db <NAME>` (default: `world.db`): Name of the main database within the directory
- `--connections-file <PATH>` (default: `connections.db`): Path to connections database (relative to
  data directory if not absolute)
- `--tasks-db <PATH>` (default: `tasks.db`): Path to persistent tasks database (relative to data
  directory if not absolute)
- `--events-db <PATH>` (default: `events.db`): Path to persistent events database (relative to data
  directory if not absolute)

The first positional argument specifies the database directory (typically `moor-data` or similar).
The daemon stores several databases within this directory by default:

- `world.db/` (or name specified by `--db`) - The main MOO database
- `connections.db` - Connection state database
- `tasks.db` - Persistent tasks database
- `events.db` - Event logging database (if event logging is enabled)

All database paths can be customized and are relative to the data directory unless specified as
absolute paths.

## Language Features Configuration

These options enable or disable runtime and language features. Lexical scopes and list/range
comprehensions are always enabled; older config files may still contain those keys, but they are
ignored.

| Feature             | Command Line                | Default | Description                                                     |
| ------------------- | --------------------------- | ------- | --------------------------------------------------------------- |
| Rich notify         | `--rich-notify`             | `true`  | Allow notify() to send arbitrary MOO values to players          |
| Type dispatch       | `--type-dispatch`           | `true`  | Enable primitive-type verb dispatching (e.g., "test":reverse()) |
| Flyweight type      | `--flyweight-type`          | `true`  | Enable flyweight types (lightweight object delegates)           |
| Boolean type        | `--bool-type`               | `true`  | Enable boolean true/false literals                              |
| Boolean returns     | `--use-boolean-returns`     | `false` | Make builtins return boolean types instead of integers 0/1      |
| Symbol type         | `--symbol-type`             | `true`  | Enable symbol literals                                          |
| Custom errors       | `--custom-errors`           | `false` | Enable error symbols beyond standard builtin set                |
| Symbols in builtins | `--use-symbols-in-builtins` | `false` | Use symbols instead of strings in builtins                      |
| Persistent tasks    | `--persistent-tasks`        | `true`  | Enable persistent tasks between server restarts                 |
| Event logging       | `--enable-eventlog`         | `true`  | Enable persistent event logging and history features            |
| Anonymous objects   | `--anonymous-objects`       | `false` | Enable anonymous objects with automatic garbage collection      |
| UUID objects        | `--use-uuobjids`            | `false` | Enable UUID object identifiers like #048D05-1234567890          |

## Import/Export Configuration

These options control database import and checkpoint export functionality:

- `--import <PATH>`: Path to a textdump or objdef directory to import
- `--export <PATH>`: Path to export checkpoints into (always uses objdef format)
- `--import-format <FORMAT>` (default: `Textdump`): Format to import from (Textdump or Objdef)
- `--checkpoint-interval-seconds <SECONDS>`: Interval between database checkpoints

`--import` is used only when creating a new database. If the requested database already exists, mooR
opens it and skips the import. The import directory is not watched for later source changes. See
[Starting a MOO from Objdef Source](bootstrapping-from-source.md) for the full lifecycle.

## Runtime Timing Configuration

These options control latency duration sampling for internal performance counters. Invocation counts
remain exact; these settings only affect duration collection.

These counters are used to observe hot runtime paths such as scheduler wakeups, lock waits, database
commit stages, builtin execution, and other VM execution activity. In the normal server
configuration, the system does not record a full timestamp pair for every hot-path event. Instead,
it samples durations and scales the totals back up. This keeps the counters cheap enough to leave
enabled in regular use.

In practice, these settings are mostly useful in three situations:

- You are benchmarking and want exact latency measurements rather than sampled estimates.
- You are chasing a performance regression and want denser timing data from hot paths.
- You want to reduce timing overhead further and are willing to trade away duration fidelity.

| Setting               | Command Line                         | Default | Description                                                   |
| --------------------- | ------------------------------------ | ------- | ------------------------------------------------------------- |
| Perf timing enabled   | `--perf-timing-enabled <BOOL>`       | `true`  | Enable or disable latency duration collection globally        |
| Hot-path sample shift | `--perf-timing-hot-path-shift <NUM>` | `6`     | Sampling shift for hot paths. `0` means exact, `6` means 1/64 |

In YAML, set these under `runtime:`:

```yaml
runtime:
  gc_interval: "30s"
  scheduler_tick_duration: "10ms"
  perf_timing_enabled: true
  perf_timing_hot_path_shift: 6
```

For exact timing during benchmarking or profiling runs:

```yaml
runtime:
  perf_timing_enabled: true
  perf_timing_hot_path_shift: 0
```

To disable duration timing entirely while keeping invocation counters:

```yaml
runtime:
  perf_timing_enabled: false
```

Guidance:

- Leave the defaults alone for normal deployments. They are intended to keep timing overhead low
  while still producing useful long-run aggregates.
- Use `0` for the sample shift during focused benchmarking or profiling runs where exact timing is
  more important than hot-path overhead.
- Set `perf_timing_enabled: false` if you only care about invocation counts and do not want duration
  timing at all.

## Task Pool Affinity Configuration

Task worker affinity is configured under `runtime:` and can also be overridden on the command line.

The daemon has two broad thread classes:

- service or control-plane threads, such as the scheduler, RPC/event handling, and coordination work
- task worker threads, which execute verbs and other task bodies in the task pool

This is an important architectural difference from LambdaMOO-style servers. In LambdaMOO, task
execution is effectively serialized through one main execution path. In moor, runnable tasks are
dispatched onto a worker pool so independent task execution can proceed concurrently across multiple
cores. The scheduler remains responsible for orchestration, wakeups, and queue management, while the
task pool provides the actual parallel execution capacity.

That means thread placement matters more here than in a single-threaded MOO. A poor affinity choice
can leave the scheduler contending with task execution on the same high-performance cores, while an
appropriate split can preserve both throughput and responsiveness.

On systems with heterogeneous CPUs, especially recent x86 and ARM systems, not all cores are equal.
Some cores are tuned for throughput and sustained performance, while others are tuned for efficiency
or background work. The affinity settings let the daemon reserve stronger cores for task execution
while leaving some capacity for the scheduler and other control-plane work.

If the runtime can identify a distinct performance-core tier, the default `auto` mode tries to use
that tier for task workers. If it cannot identify a meaningful split, the task pool is left
unpinned.

| Setting                     | Command Line                 | Default        | Description                                                                                       |
| --------------------------- | ---------------------------- | -------------- | ------------------------------------------------------------------------------------------------- |
| Task pool pinning           | `--task-pool-pinning <MODE>` | `auto`         | Controls whether task worker threads are pinned to detected performance cores                     |
| Reserved service perf cores | `--service-perf-cores <NUM>` | topology-based | Reserves detected performance cores for non-task service threads before assigning worker affinity |

`task_pool_pinning` accepts:

- `auto`: Use the runtime's default policy.
- `performance`: Pin task workers to detected performance cores when available.
- `none`: Do not pin task worker threads.

`service_perf_cores` must be a non-negative integer. It reserves that many detected performance
cores for service threads. The value is clamped so that, when possible, at least one performance
core remains available for task workers.

When `service_perf_cores` is not set, the reservation defaults are:

- `0` for systems with `0..=2` detected performance cores
- `1` for systems with `3..=7` detected performance cores
- `2` for systems with `8+` detected performance cores

Examples:

```yaml
runtime:
  task_pool_pinning: performance
  service_perf_cores: 2
```

```bash
# Force performance-core pinning for task workers
moor-daemon --task-pool-pinning performance ...

# Reserve two detected performance cores for scheduler / control-plane work
moor-daemon --service-perf-cores 2 ...

# Disable task-worker pinning
moor-daemon --task-pool-pinning none ...
```

Guidance:

- Leave this on `auto` unless you have measured a reason to override it.
- `performance` is useful when you know the machine has a meaningful fast-core tier and you want
  task execution to stay there even if automatic detection would otherwise fall back.
- `none` is useful inside containers, VMs, or unusual schedulers where explicit pinning hurts more
  than it helps.
- Increase `service_perf_cores` if the scheduler, RPC handling, or other daemon-side coordination
  work becomes a bottleneck while worker threads are saturating the faster cores.
- Decrease `service_perf_cores` if the machine has only a few performance cores and you want to
  maximize task execution throughput.

## Captured Verb Call Configuration

Some clients ask the daemon to run a verb and hand back its return value together with the narrative
output the call produced, instead of streaming that output to a connection. The web host's verb
endpoint and the welcome-message endpoint both work this way. Those calls occupy an RPC worker while
they wait, so the daemon puts an upper bound on how long a caller may ask it to wait.

| Setting              | Command Line                            | Default | Description                                                    |
| -------------------- | --------------------------------------- | ------- | -------------------------------------------------------------- |
| Max capture deadline | `--max-capture-deadline-seconds <SECS>` | `60`    | Longest deadline a client may request for a captured verb call |

A client may request a shorter deadline, but not a longer one: a request above the maximum is
refused rather than quietly shortened, so the client's own receive timeout still matches what it
asked for. A deadline of zero is not valid. When the deadline passes, the task is cancelled and the
caller gets a time-limit task error.

```yaml
runtime:
  max_capture_deadline: "30s"
```

```bash
moor-daemon --max-capture-deadline-seconds 30 ...
```

## Example Configuration

Here's an example configuration file:

```yaml
# Database configuration
database:
  object_verbs:
    max_memtable_size: 536870912

# Language features configuration
features:
  persistent_tasks: true
  rich_notify: true
  bool_type: true
  symbol_type: true
  type_dispatch: true
  flyweight_type: true
  use_boolean_returns: false
  use_symbols_in_builtins: false
  custom_errors: false
  enable_eventlog: true
  use_uuobjids: true
  anonymous_objects: true

# Import/export configuration
import_export:
  checkpoint_interval: "60s"

# Runtime timing configuration
runtime:
  perf_timing_enabled: true
  perf_timing_hot_path_shift: 6
  task_pool_pinning: auto
  service_perf_cores: 1
```

## LambdaMOO Compatibility Mode

If you need to import LambdaMOO 1.8 content, use the
[Lambda-moor core](https://github.com/timbran-project/moor/tree/main/cores/lambda-moor) or disable
the remaining compatibility-sensitive features. Lexical scopes and list/range comprehensions are
part of the language now and cannot be disabled.

```yaml
# LambdaMOO 1.8 compatible features
features:
  persistent_tasks: true
  rich_notify: false
  bool_type: false
  symbol_type: false
  type_dispatch: false
  flyweight_type: false
  use_boolean_returns: false
  use_symbols_in_builtins: false
  custom_errors: false
  enable_eventlog: true
  use_uuobjids: false
  anonymous_objects: false

# LambdaMOO textdump import is supported
# Checkpoints always export in objdef format
```

## Anonymous Objects Configuration

The `anonymous_objects` feature flag enables a new type of object that is automatically garbage
collected when no longer referenced. This feature is disabled by default due to performance
considerations.

### Enabling Anonymous Objects

To enable anonymous objects, set the flag in your configuration file:

```yaml
features:
  anonymous_objects: true
```

Or use the command line flag: `--anonymous-objects`

### When to Enable Anonymous Objects

**Consider enabling if:**

- Your MOO creates many temporary objects (game pieces, UI elements, etc.)
- You have developers who struggle with manual object cleanup
- You want to reduce the burden of object lifecycle management
- Your server has sufficient CPU resources for garbage collection overhead

**Keep disabled if:**

- Your MOO has strict performance requirements with minimal latency tolerance
- Your builders are experienced with manual object lifecycle management
- Your server runs on resource-constrained hardware
- You need maximum predictable performance without GC pauses

### Performance Implications

Anonymous objects use a mark-and-sweep garbage collector with the following characteristics:

- **CPU Overhead**: The GC thread runs continuously, consuming CPU cycles even when not collecting
- **Memory Usage**: Same storage costs as regular objects until collection occurs
- **Concurrency**: Mark phase runs concurrently with normal server operations to minimize blocking
  but can put load on the system as it scans the entire database.
- **Collection Pauses**: Sweep phase can cause brief server pauses during collection cycles

The garbage collector is optimized but will impact overall server performance. Monitor your server's
CPU usage and response times when enabling this feature.

### Migration Considerations

When enabling anonymous objects on an existing MOO:

- Existing code using `create(parent, owner, 1)` will begin creating anonymous objects
- No changes needed to existing numbered or UUID object code
- Consider updating builder documentation to explain the new object type option
- Test performance impact during peak usage periods before enabling permanently

## Git worker

`moor-git-worker` provides read-only Git repository access through `worker_request('git, ...)`. It
runs as a separate process and uses the existing worker sockets and CURVE enrollment. The daemon
does not need Git installed. The worker uses the Rust `gix` library.

Build and start the worker with the same socket addresses as the daemon:

```bash
cargo build -p moor-git-worker
./target/debug/moor-git-worker --work-dir=/tmp/moor-git-jobs
```

The default addresses match the daemon's default IPC addresses. Use `--help` for connection options.
For TCP connections, use the standard host enrollment options and a separate worker identity
directory. The `deploy/clustered/web-basic` deployment includes an optional `git-worker` profile:

```bash
COMPOSE_PROFILES=git-worker ./start.sh
```

The worker accepts public HTTP and HTTPS repository URLs. It rejects embedded credentials, URL query
strings, fragments, local paths, and other transports. It does not follow HTTP redirects. Use the
repository's final URL. Private repository credentials and persistent repository caches are not
supported in this version.

Each request uses a disposable bare repository. The worker does not check out files, run repository
hooks, follow symlinks, fetch submodules, or apply content filters. Job processes have an empty
inherited environment and isolated Git configuration. A request cannot select a host filesystem
path. The worker uses Linux resource limits for each job. An operator can also limit the worker's
network access and temporary storage through its container or service configuration.

### Git request protocol

Requests contain an operation and a schema-1 map. Field names and operation names accept strings or
symbols. Unknown fields and duplicate normalized field names are errors.

```moo
response = worker_request('git, {'snapshot, [
    "schema" -> 1,
    "repository" -> "https://github.com/timbran-project/moor.git",
    "revision" -> ["ref" -> "refs/heads/main"],
    "path" -> "cores/cowbell/src"
]}, ["timeout_seconds" -> 30.0]);
```

The caller needs wizard authority or an explicit grant for `worker_request()`. The worker does not
interpret the returned files or write to the world database.

| Operation      | Request fields, in addition to `schema`                          | Result fields                                                                                                      |
| -------------- | ---------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| `capabilities` | None                                                             | `operations`, `transports`, `object_formats`, `limits`, `max_concurrent_requests`, `content_type`, `path_encoding` |
| `refs`         | `repository`, optional `limits`                                  | `refs`                                                                                                             |
| `tree`         | `repository`, `revision`, optional `path`, `recursive`, `limits` | `commit`, `tree`, `path`, `entries`                                                                                |
| `read`         | `repository`, `revision`, `path`, optional `limits`              | `commit`, `entry`                                                                                                  |
| `snapshot`     | `repository`, `revision`, optional `path`, `limits`              | `commit`, `tree`, `path`, `entries`                                                                                |

A revision contains exactly one full ref name or full commit ID:

```moo
["ref" -> "refs/tags/v1.2.3"]
["commit" -> "sha1:0123456789abcdef0123456789abcdef01234567"]
```

Schema 1 initially supports SHA-1 repositories. Annotated tags resolve to commits. Branch and tag
short names, abbreviated IDs, and revision expressions are rejected. A requested commit must be
available from the remote. Some servers restrict fetching commits that no advertised ref names. A
failed commit fetch never falls back to the current branch head.

The worker resolves the revision once for each request. All returned entries come from that commit.
Use the returned commit ID in later requests to keep the same revision when a branch moves. `refs`
lists advertised branches and tags without fetching their object contents. Each ref has a `name` and
`oid`. Annotated tags also have a `peeled` ID when the server provides it.

Paths are UTF-8 strings, relative to the repository root. An empty path selects the root directory.
Absolute paths, empty components, dot segments, backslashes, and NUL bytes are rejected. A
repository entry with an unsupported name fails the listing instead of receiving a lossy replacement
name.

`tree` lists immediate children by default. Set `recursive` to `true` to include descendants.
`snapshot` always includes descendants. Returned listing paths are relative to the selected
directory and sorted by their UTF-8 bytes. A `read` entry retains the requested repository-relative
path.

Every entry has `path`, `kind`, and an algorithm-prefixed `oid`:

| Kind        | Additional fields                        | Behavior                                                                |
| ----------- | ---------------------------------------- | ----------------------------------------------------------------------- |
| `file`      | `size`, `executable`, optional `content` | Exact blob bytes and executable mode.                                   |
| `symlink`   | `size`, optional `content`               | Link-target bytes. The target is never followed.                        |
| `directory` | None                                     | The `oid` identifies the directory tree.                                |
| `submodule` | None                                     | The `oid` identifies the recorded submodule commit. No recursive fetch. |

`read` and `snapshot` include `content` for files and symlinks. Content is a MOO binary value.
`tree` returns metadata only. The worker does not decode text, change line endings, or expand Git
LFS pointers. Git LFS pointer files remain ordinary file contents.

### Results, errors, and limits

Successful responses have this envelope:

```moo
["schema" -> 1, "ok" -> true, "result" -> result_map]
```

Application failures have this envelope:

```moo
["schema" -> 1, "ok" -> false,
 "error" -> ["code" -> "path_not_found", "message" -> "Requested path does not exist."]]
```

Callers should branch on `code`, not the message. Codes include `invalid_request`,
`unsupported_schema`, `unsupported_operation`, `unsupported_transport`, `revision_not_found`,
`path_not_found`, `wrong_object_type`, `unsupported_path`, `authentication_required`,
`fetch_failed`, `invalid_repository`, `limit_exceeded`, `busy`, and `job_failed`. `job_failed`
includes child process failures such as memory exhaustion. Unavailable workers, disconnection,
request deadlines, and worker infrastructure failures use the existing raised `worker_request()`
errors.

| Worker option               | Default    | Meaning                                                                                   |
| --------------------------- | ---------- | ----------------------------------------------------------------------------------------- |
| `--max-concurrent-requests` | 4          | Active Git jobs. Further requests return `busy`.                                          |
| `--max-entries`             | 4096       | Entries or refs in one result, including directories.                                     |
| `--max-file-bytes`          | 4194304    | Bytes in one returned file or symlink.                                                    |
| `--max-total-bytes`         | 16777216   | Combined returned file and symlink contents.                                              |
| `--max-fetch-bytes`         | 268435456  | Git HTTP response headers and bodies across the request. Also limits each temporary file. |
| `--max-memory-bytes`        | 1073741824 | Virtual address space for each job process.                                               |
| `--max-seconds`             | 30         | Maximum job duration, including connection and download.                                  |

A request can lower `max_entries`, `max_file_bytes`, and `max_total_bytes` through its `limits` map.
It cannot raise the operator's limits. Metadata has a separate bounded response allowance. The
effective deadline is the smaller of `timeout_seconds` and `--max-seconds`.

A successful snapshot is complete for the selected directory. Exceeding a limit fails the whole
request, without a partial result. Deadlines terminate and reap the child before releasing its slot.
Normal completion, failure, and graceful shutdown remove temporary repositories. After an abrupt
worker or machine termination, an operator can remove leftover `job-*` directories while the worker
is stopped. These directories contain no review state and are not required by later requests.
