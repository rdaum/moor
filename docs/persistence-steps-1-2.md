# Persistence steps 1 and 2

This change implements the functional scope of the first two steps of
[the persistence proposal](../pluggable-persistence-and-postgresql.md#16-implementation-sequence).
Fjall remains the only storage backend. Snapshot/read separation and readable codecs belong to later
steps. The proposed 5% foreground performance gate remains unmet; see the measurements below.

## Behavior

The transaction engine submits typed logical commits after publication. Fjall workers prepare and
encode their physical batches. Logical changes release transaction indexes before enqueueing. The
writer drains queued encoder results before applying the ready prefix. Each commit retains its own
atomic Fjall batch. The admission pool holds each permit until application. Its 1,000 permits cover
unpublished reservations, encoding, reordered results, and storage application. This limit counts
commits, not bytes.

Shutdown closes admission, waits for admitted attempts to submit or abandon their reservations, and
drains the writer. One deadline covers these phases, queue sends, and worker joins. A timeout
reports incomplete shutdown and wakes waiters. A blocked filesystem operation can continue after the
caller's deadline.

Worker failures close admission before they return capacity. A waiter timeout does not cancel a
published commit. Expired waiters release their reply handles. Independent snapshots retain their
captured view after writer shutdown.

`TxDB::publication()` captures an epoch-scoped token. `wait_applied()` and `wait_durable()` return
receipts for that token. Tokens from another open fail, including version zero. A published root can
precede its batch submission. `persistence_status()` reports publication, submission, application,
durability, admission usage, and writer health.

Ordinary commits and default shutdown do not request an explicit Fjall sync. An explicit durability
wait uses `PersistMode::SyncAll` after application. Sequence observations use per-slot maxima even
when submission order differs from publication order. The Fjall format, append records, and format
version remain unchanged.

## Conformance

The database tests cover ordered application, retained admission credits, abandoned reservations,
shutdown races, queue deadlines, waiter timeouts, durability, and sequence recovery. The existing
snapshot, append-chain, transaction-history, and objdef tests remain part of validation.

The literal corpus compares nested values exactly. It checks string and symbol spelling, float bits,
map keys, error payloads, and flyweight contents. Unsupported literal cases remain explicitly
ignored until step 4. Source fixed points supplement the runtime lambda tests. These tests establish
the codec target. They do not claim that the current formatter is a persistence codec.

## Measurement protocol

The comparison uses three release builds:

1. The pre-refactor engine at `891d2cb80fd86b02a088bdc7931bfaed0e467e07`.
2. The refactor with admission permits released at encoder pickup.
3. The refactor with admission permits held through application.

The second build exists only for measurement. It isolates the effect of the permit lifetime. It is
not a supported runtime configuration.

The [baseline patch](benchmarks/fjall-persistence-baseline.patch) adds the measurement harness and
an explicit durability fence to the pre-refactor source. It does not change ordinary commit
acknowledgments or storage encoding. Its applied wait retains the original unbounded behavior. The
outer measurement process has a timeout.

The [foreground example](../crates/db/examples/persistence_baseline.rs) warms each worker with 1,000
transactions and drains persistence before measurement. Foreground threads use the existing
`spawn_worker_perf` placement helper. This keeps the variants on the same CPU tier on hybrid
machines. Each measured transaction reads and updates one worker-specific property. Workers share
the publication root. The output includes throughput, p50/p95/p99 latency, retries, admission waits,
and application drain time. The final snapshot and resident view must contain each worker's last
value.

`moorc --test-phases N` appends the phase number and previous result to each test invocation. It
waits for application and an explicit durable fence between phases. Persistence failure fails the
test. The history workload uses three phases: seed, producers, and reporting with cleanup. It
captures final counters before deleting the history objects. A probe samples admission usage and
unapplied commits every millisecond through the final drain. Sampled maxima can miss shorter peaks.

For the phased history workload, run:

```sh
make -C cores/benches bench-string-history HISTORY_PHASES=3 \
  HISTORY_WRITERS=1 HISTORY_ENTRIES=1024 HISTORY_ENTRY_BYTES=9472 \
  HISTORY_APPENDS=70 HISTORY_SETTLE_SECONDS=2
```

The benchmark core restores the missing `sysobj` and `game_update` fixtures from the parent of
`40f40e362e902b60de5bc505910870c6f3fbf5e4`. It updates their objdef identifiers and numeric
conversions. The write-stress test rejects update-loop faults and subscribers that perform no work.
All three builds run the same fixture files; `results.json` records their hashes.

The zero-settle pressure preset retains eight writers and 1,200 updates. The write-stress workloads
use one phase and keep their existing internal counters. Their final drain includes workload
cleanup, so those counters do not provide the history workload's exact measurement boundary.

The [comparison runner](../scripts/qualify-fjall-persistence.py) records commands, binary hashes,
logs, and measurements in `results.json`. It alternates variant order between repetitions. It runs
foreground workloads and the history, replacement, rebuilt-prefix, and write-stress cases. It does
not build binaries during measurement.

```sh
python3 scripts/qualify-fjall-persistence.py \
  --variant baseline /tmp/moor-persistence-baseline-moorc /tmp/moor-persistence-baseline-micro \
  --variant early /tmp/moor-persistence-early-moorc /tmp/moor-persistence-early-micro \
  --variant held /tmp/moor-persistence-candidate-moorc /tmp/moor-persistence-candidate-micro \
  --output /tmp/moor-persistence-qualification
```

To reproduce the baseline, extract the named commit into a separate directory and apply the baseline
patch. Build `moorc` and `persistence_baseline` in release mode. Use separate target directories for
variant builds. Shared target directories can reuse incompatible workspace artifacts when switching
source trees.

The early-release build uses the candidate source with two lines inserted in
`BatchWriter::encoder_loop`, immediately before `let mut encoded = match request`:

```rust
drop(admission);
let admission = None;
```

Build with `cargo build --release -p moorc -p moor-db --bins --example persistence_baseline`. Copy
each variant's binaries before measuring. The baseline patch includes the same harness and probe.

## Validation and core workload results

Validation on 2026-09-29 passed:

- `cargo test -p moor-db -p moor-compiler -p moor-kernel -p moor-objdef -p moor-daemon --lib --tests`:
  1,387 passed, 14 ignored.
- `cargo clippy --workspace --all-targets --all-features`.
- All 54 core workload runs completed, including their application and durability fences.

Eight ignored literal cases and one lambda-default test belong to the step 4 codec work. The
remaining ignored tests were already excluded by their suites. Clippy reports a dependency
future-compatibility notice for `proc-macro-error2`.

The core results below are medians of three runs. The machine has 20 logical CPUs, with Cortex-X925
and Cortex-A725 cores. Runtime workers use the existing performance-core placement policy. Each
write-stress run lasts 10 seconds. All builds use ordinary asynchronous commit acknowledgment and
explicit fences at the phase boundaries.

| Workload and measurement            |  Baseline | Early release | Held through application |
| ----------------------------------- | --------: | ------------: | -----------------------: |
| History rollup, producer ms         |    20.712 |        20.747 |                   20.865 |
| History pressure, producer ms       |    23.191 |        22.947 |                   23.469 |
| History replacement, producer ms    |    19.767 |        18.475 |                   15.482 |
| History rebuilt prefix, producer ms |   126.404 |       128.565 |                  128.698 |
| Write overwrite, counter increments | 1,971,200 |     1,976,320 |                1,966,080 |
| Write append, counter increments    | 2,493,440 |     2,503,680 |                2,498,560 |

The history task polls producer completion every 10 ms. These timings cannot establish
sub-millisecond parity. Producer completion does not imply storage completion. These are the
additional applied-drain waits:

| Workload, applied-drain ms         | Baseline | Early release | Held through application |
| ---------------------------------- | -------: | ------------: | -----------------------: |
| History rollup                     |    9.184 |        11.308 |                   11.065 |
| History pressure                   |   11.841 |        10.571 |                   11.308 |
| History replacement                |   52.946 |        54.697 |                   58.784 |
| History rebuilt prefix             |   41.644 |        29.944 |                   38.507 |
| Write overwrite, including cleanup |    0.829 |         0.936 |                    0.968 |
| Write append, including cleanup    |    0.831 |         1.118 |                    1.030 |

For the pressure preset, median sampled peak admission usage was 588, 570, and 699 commits. The
corresponding unapplied peaks were 696, 695, and 699. The held permits count work after encoder
pickup. These runs stayed below the 1,000-permit limit. They do not measure performance at admission
saturation. Controlled lifecycle tests verify permit retention while encoding and while an earlier
publication blocks application.

Durable-fence waits varied independently of producer time. The per-run artifact retains these
timings and the encoding, rollup, and admission counters.

All nine rollup runs encoded 669,611 suffix bytes and performed one full-value rollup.

## Pinned foreground results and remaining gate

These runs use 250,000 measured transactions per worker after warmup. The one-worker case uses CPU
5; the four-worker case uses CPUs 5 through 8. All are in the detected performance tier. The initial
unpinned exploratory microbenchmarks are excluded from this comparison. No compilation ran during
measurement.

| Foreground measurement, median of three runs      | Baseline | Early release | Held through application |
| ------------------------------------------------- | -------: | ------------: | -----------------------: |
| One worker, commits/s                             |  183,022 |       165,963 |                  164,321 |
| One worker, p50 us                                |    5.392 |         5.856 |                    5.904 |
| One worker, p95 us                                |    6.768 |         7.488 |                    7.552 |
| One worker, p99 us                                |    7.488 |         8.736 |                    8.560 |
| Four workers, commits/s                           |  174,123 |       285,696 |                  216,059 |
| Four workers, p50 us                              |   22.448 |         9.296 |                   16.768 |
| Four workers, p95 us                              |   28.864 |        30.176 |                   33.409 |
| Four workers, p99 us                              |   33.792 |        84.864 |                   49.776 |
| Four workers, admission waits per million commits |  985,892 |        35,899 |                  959,951 |
| Four workers, applied-drain ms                    |    4.510 |         0.001 |                    2.678 |

The held-permit refactor improves four-worker throughput by 24.1% over baseline, but p95 latency
rises by 15.7%. Single-worker throughput falls by 10.2%, and p95 latency rises by 11.6%. The
provisional 5% gate therefore remains open. The single-worker regression also appears in the
early-release build; permit lifetime does not explain all of it.

Holding permits through application reduces four-worker throughput by 24.4% relative to early
release. It also changes admission frequency and tail latency. The early-release build cannot
substitute for the bounded pipeline. Ordinary acknowledgments and durability policy are the same in
all three variants.

The next performance work should target logical-batch preparation and coordinator overhead on the
single-worker path, then publication tails under sustained admission pressure. These measurements do
not justify advancing the performance gate.

The [per-run results](benchmarks/fjall-persistence-results.json) contain all 72 comparison runs,
binary hashes, fixture hashes, commands, CPU topology, and counters. They retain application and
durability waits separately.
