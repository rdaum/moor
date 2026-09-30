# Persistence step 6: PostgreSQL world storage

The library can open, write, and reopen a PostgreSQL world through `TxDB`. The daemon, combined
server, moorc, and moor-emh accept PostgreSQL settings through shared CLI arguments. The daemon and
combined server also accept YAML storage settings. Broader acceptance work remains below.

## Implemented path

`initialize_postgres_schema` creates a new schema in one durable transaction. Ordinary opening does
not create or repair tables. Each open claims a fresh writer epoch under a schema advisory lock. The
claim preserves persistent counters and completes before the engine can publish transactions.

Startup reads metadata, sequence slots, and all relations from one repeatable-read snapshot. Rows
stream directly into resident indexes. Stored verb source is compiled during loading. The loader
rejects unsupported profiles, invalid literals, noncanonical object keys, and invalid property
chains. Tuple timestamps and property record numbers cannot exceed their persistent counters.

The runtime uses bounded encoder queues and one ordered SQL writer. An admission permit remains held
until the commit is applied. Encoders render readable values and programs. Prepared statements apply
rows in batches by relation. The writer updates relations and progress in one transaction per
logical commit. Publication order determines the result when timestamps arrive out of order.

List appends store only their suffix. The shared reconstruction checks enforce the 64-record and 4
MiB append bounds. A separate encoder handles full-value rollups. Chain metadata changes only after
SQL application is confirmed. Deletion-only property commits also advance the record counter.

Recovery retains the planned batch. It reconnects, reacquires ownership, and compares database
identity, epoch, and progress. It replays only when the stored progress proves the batch did not
commit. Recovery uses one finite deadline. An unexpected epoch or watermark causes terminal failure.

Synchronous SQL commit is the default. Asynchronous mode requires explicit configuration. A durable
fence updates persistent bookkeeping with synchronous commit enabled. Graceful shutdown drains
published work within its deadline. A timeout reports incomplete persistence.

Storage size is sampled on the writer and read from a cached counter by runtime tasks. PostgreSQL
does not expose Fjall maintenance statistics or implement Fjall compaction. Snapshot export remains
an explicit error until step 7.

## Verification so far

The disposable fixture runs adapter tests, schema tests, and internal storage tests:

```sh
scripts/test-postgres-adapter.sh 17
scripts/test-postgres-adapter.sh 18
```

The live tests cover:

- Explicit initialization, exact integer domains, and exclusive ownership.
- Publication ordering, monotonic sequence slots, and restart after deletion of the newest tuples.
- Disconnects immediately before and after COMMIT, without duplicate suffix application.
- Record-bound rollups and a durable fence after asynchronous commits.
- Public `TxDB` publication, persistence, source compilation, and reopening in both commit modes.
- Two hundred commits submitted in reverse order, retained admission permits, and independent rollup
  replies.
- Ownership takeover, bounded recovery failure, and property-record counter exhaustion.

The default database unit suite also passes. These checks do not complete the step 6 acceptance
gate.

## Native launch and benchmarks

Run `scripts/start-moor-cowbell-postgres.sh` to build the monolith with Cargo and start native
PostgreSQL. The script requires PostgreSQL 17 or 18 and libpq 17+ development/runtime libraries. It
creates a private cluster under `run-cowbell-postgres/native` and initializes the Cowbell schema.
PostgreSQL stays running after the monolith exits. The script prints connection and shutdown
commands.

Use `make -C cores/benches bench-write-stress POSTGRES=1` to benchmark the running native database.
Each benchmark creates a separate schema and removes it afterward. Set
`PG_COMMIT_POLICY=asynchronous` to disable synchronous SQL commits during the workload. For history
comparisons, set `HISTORY_PHASES=3 HISTORY_SETTLE_SECONDS=0` for both backends. The benchmark README
describes the connection settings and measurement boundaries.

Native history and write-stress smoke tests passed, including durable fences and clean shutdown. All
four enabled CLI setup/path checks and both YAML override checks passed against native
PostgreSQL 17. Workspace Clippy passed with all targets and features. The smoke tests exposed a
shutdown race: encoder exit could close the writer queue during normal shutdown. The writer now
treats that closure as expected after shutdown begins.

Manual release runs showed PostgreSQL backpressure during write stress, followed by recovery in a
60-second asynchronous run. That run produced about 195,499 mutations/s, versus 215,893 for Fjall,
and drained in milliseconds. An 80-append burst took about 64.4 ms through durability with
PostgreSQL, versus 12.7 ms with Fjall. These are individual observations, not repeated performance
qualification or evidence of a passed gate. PostgreSQL encoder and SQL timing instrumentation
remains incomplete.

## Remaining acceptance work

- Extend common backend conformance coverage and malformed-row diagnostics.
- Complete the feature/default CLI matrix on both supported PostgreSQL versions.
- Measure persistence queue depth and SQL writer timing through the observed slowdown and recovery.

Live tests also cover stored value kinds, sparse properties, large full values, invalid stored
source, connection loss during statements, server restart, and shutdown with pending work. Explicit
schema initialization and rejection of Fjall paths/settings are wired into all four binaries.
Ancillary databases remain local.

Snapshot export, inspection views, validation commands, and restore drills belong to step 7.
Throughput qualification and the outstanding Fjall baseline comparison belong to step 8.
