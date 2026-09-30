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
rows in batches by relation. The writer groups consecutive ready commits into one SQL transaction.
Publication order determines the result when timestamps arrive out of order.

Groups contain at most 64 logical commits, 1 MiB of estimated encoded payload, and 4,096 mutation
operations. Property payload estimates allow for JSON escaping. Group collection stops after 1 ms or
at a known durable fence. The writer never waits for more commits to fill a group. A single large
logical commit can exceed these limits and remains indivisible. After rollup encoding, the writer
checks actual payload bytes again. If a member exceeds the remaining budget, the writer retains it
for the next group.

Each member applies its mutations in order. Tentative chain metadata lets later members see earlier
appends, replacements, and deletions. Persistent commit and property-record counters still advance
per logical commit. All group permits remain held until SQL confirms the complete group.

List appends store only their suffix. The shared reconstruction checks enforce the 64-record and 4
MiB append bounds. A separate encoder handles full-value rollups. Chain metadata changes only after
SQL application is confirmed. Deletion-only property commits also advance the record counter.

Recovery retains the complete group plan. It reconnects, reacquires ownership, and compares database
identity, epoch, and progress. It replays only when the stored progress proves the group did not
commit. Recovery uses one finite deadline. An unexpected epoch or watermark causes terminal failure.

Synchronous SQL commit is the default. Asynchronous mode requires explicit configuration. A durable
fence updates persistent bookkeeping with synchronous commit enabled. The idle writer waits on both
commit and fence channels, so a fence wakes it immediately. Graceful shutdown drains published work
within its deadline. A timeout reports incomplete persistence.

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
- Group rollback and lost-COMMIT recovery, including rollups and delete/reinsert sequences within a
  group.
- A group paused inside SQL, with progress and admission permits held until commit.
- Group size limits, publication gaps, fence boundaries, and idle fence wakeups.

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

## Writer tuning measurements (2026-09-30)

A separate native PostgreSQL 17.11 cluster supplied the before/after comparison. Each run used a
fresh schema. The comparison used release builds, with `fsync` and `full_page_writes` enabled. No
builds or tests ran during the timed workloads. The baseline used the saved release binary from
before this tuning pass.

| Measurement                                   |   Before |    After |
| --------------------------------------------- | -------: | -------: |
| Synchronous history burst through durability  | 45.76 ms | 10.54 ms |
| Synchronous write mutations/s                 |  207,531 |  204,288 |
| Synchronous peak outstanding commits          |    1,000 |    1,000 |
| Synchronous admission blocking events         |    3,629 |      157 |
| Asynchronous history burst through durability | 41.87 ms | 13.15 ms |
| Asynchronous write mutations/s                |  213,931 |  199,857 |
| Asynchronous peak outstanding commits         |    1,000 |    1,000 |
| Asynchronous admission blocking events        |      601 |      157 |
| Asynchronous explicit fence wait              | 12.00 ms |  1.37 ms |

History values are medians of three 80-append runs, with three phases and zero settle delay. Burst
completion adds producer time to phase-2 application and durability waits. It excludes intervening
harness work. Write results come from one 60-second run per policy, with 256 subscribers and 20
mutations per subscriber tick. These results do not establish maximum sustained throughput or
complete the Fjall regression gate.

All 31 PostgreSQL-focused tests passed against the native server, including grouped recovery,
retained permits, and oversized rollups. Workspace Clippy passed with all targets and features.
Scoped Rust formatting checks also passed.

The [measurement data](benchmarks/postgresql-writer-grouping-results.json) includes server settings,
binary hashes, per-run results, and tick samples.

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
