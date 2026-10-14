This is meant to be a "core" which is in fact a set of objdef files that can be used to benchmark
the performance of some mooR workloads.

Run `make bench` to build and run the benchmarks, which present as mooR `objdef` test_ runs.

Use `POSTGRES=1` to run the same workloads against native PostgreSQL:

```shell
make -C cores/benches bench-write-stress POSTGRES=1 DURATION=30
make -C cores/benches bench-string-history POSTGRES=1 HISTORY_PHASES=3
```

Run these commands from the repository root. PostgreSQL must already be running.
The default connection uses the cluster from `scripts/start-moor-cowbell-postgres.sh`.
Stop the Cowbell server before timing benchmarks; its PostgreSQL cluster can stay running.

Each run builds `moorc` with PostgreSQL support and creates a separate benchmark schema.
The script removes that schema after the run. It does not change the Cowbell schema.
Set `PG_KEEP_SCHEMA=1` to retain the benchmark schema for inspection.
PostgreSQL runs omit objdef export because PostgreSQL snapshot export is not implemented.
Export, perf, and simulated slow-disk targets do not support `POSTGRES=1`.

The PostgreSQL commit policy defaults to `synchronous`. Set `PG_COMMIT_POLICY=asynchronous` to change it.
Ordinary Fjall commits do not request a synchronous flush, so label the policy in comparisons.
Write-stress and combat runs report a final application and durability wait for either backend.
PostgreSQL history runs use three phases by default. Set `HISTORY_PHASES=3` for the matching Fjall run.
Compare workload time, persistence drain time, and queue occupancy together.
Mutation counts are not SQL transaction counts or durable commits per second.

For another local cluster, set `PGSERVICEFILE`, `PG_SERVICE`, and `PG_SOCKET_DIR`.
The default profile is `release`. Use `MOOR_BENCH_PROFILE=dev` only for functional checks of PostgreSQL runs.

Run `make bench-string-history` to measure appends to large string-list properties. The benchmark
starts four writer tasks by default. Each task appends 20 strings to a list that contains 1,024
distinct strings. Each string contains 2,048 bytes.

Use this configuration for one property with approximately 9.7 MB of string data and enough appends
to exercise one foreground rollup:

```shell
make bench-string-history HISTORY_PHASES=3 HISTORY_WRITERS=1 HISTORY_ENTRIES=1024 \
  HISTORY_ENTRY_BYTES=9472 HISTORY_APPENDS=70 HISTORY_SETTLE_SECONDS=2
```

The database metrics report complete and suffix bytes, ordinary encoding and commit time, rollup
count and time, and backpressure. `cargo bench -p moor-db --bench objdef_export_benches` measures
snapshot reconstruction and export of the corresponding bounded append chain.

Use `HISTORY_WRITERS`, `HISTORY_ENTRIES`, `HISTORY_ENTRY_BYTES`, `HISTORY_APPENDS`, and
`HISTORY_APPEND_DELAY` to change the workload. `HISTORY_APPEND_WIDTH` appends multiple strings in
one update. `HISTORY_MUTATION_MODE=1` replaces one element, while mode 2 rebuilds the prefix before
each append. Set `HISTORY_PHASES=3` to separate setup, producers, and reporting. The harness waits
for application and an explicit durable fence between phases. Counters exclude setup persistence
and include the producer phase's final drain. Without this option, the benchmark uses timed settling.

Use this command to put pressure on the batch-writer queue:

```shell
make bench-string-history HISTORY_PHASES=3 HISTORY_WRITERS=8 HISTORY_ENTRIES=1024 \
  HISTORY_ENTRY_BYTES=2048 HISTORY_APPENDS=150 HISTORY_SETTLE_SECONDS=0
```

This preset submits 1,200 property updates. The `HISTORY_APPEND_RESULT` line reports the time that
the producer tasks require. The batch-writer counters report queue-full events and blocked time.
`PERSISTENCE_BOUNDARY` reports application and durable-fence wait times for each phase.
`PERSISTENCE_OCCUPANCY` reports admission usage and unapplied commits sampled every millisecond.
Sampled maxima can miss shorter peaks.

See [the persistence comparison](../../docs/persistence-steps-1-2.md) for the baseline builds,
measurement script, and results. The write-stress fixture checks update-loop failures and requires
every subscriber to perform work.

Build commands use the shared rules in `cores/make/`. `make` or `make build` exports `gen.objdir/`.
`make test` validates import and export without running benchmarks. `make check` also tests the
shared rules and export/reimport stability. Benchmark workloads remain under the `bench*` targets.
`make rebuild` (also `make reformat`) checks the roundtrip before overwriting source files with
normalized objdef. Export removes comments.

This core defaults to `MOORC_PROFILE=release`. `MOORC_TYPE=direct` uses the existing release
compiler; `MOORC_TYPE=docker` uses `MOORC_IMAGE`. Use `MOORC_PROFILE=dev` for a debug compiler.
`make help` lists the common targets and compiler settings.
