This is meant to be a "core" which is in fact a set of objdef files that can be used to benchmark
the performance of some mooR workloads.

Run `make bench` to build and run the benchmarks, which present as mooR `objdef` test_ runs.

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
