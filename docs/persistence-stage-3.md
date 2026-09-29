# Persistence stage 3: snapshot reads and resident indexes

Stage 3 separates storage reads from transaction indexes and shared export assembly. Fjall remains
the only backend.

## Boundaries

`crates/db/src/relation_registry.rs` declares each relation once. Entries specify key/value types,
primary or secondary indexes, mutation category, and conflict policy.

The registry generates resident relations, transactions, checkers, snapshots, logical changes,
relation IDs, typed readers, and Fjall bindings. The engine macro has no Fjall resources or encoding
operations. `Relation<K, V>`, `RelationTransaction<K, V>`, and `CheckRelation<K, V>` have no
provider parameter or field.

`StorageBackend::Fjall` owns storage opening, seeding, snapshot-loader construction, and maintenance
dispatch. Encoder workers own `FjallRelations`, which binds accepted logical changes to keyspaces.
The common coordinator returns a backend snapshot receipt.

Resident lookups use their existing typed indexes. They do not call a storage reader or dispatch on
a backend. Transaction indexes now hold the immutable index directly; reads cannot populate it from
storage.

## One snapshot for startup

The opener creates all keyspaces, then captures one snapshot. Sequence recovery, ordinary relation
scans, and property-chain reconstruction use that snapshot.

Ordinary scans stream tuples into the selected index builder. Property scans use the same
reconstruction code as export. They also collect record versions, append byte counts, and chain
state for the writer. Property names come from the seeded root.

An index becomes fully resident only after its seed cursor ends successfully. Startup checks every
index before returning the root. A failed scan returns an opening error without publishing a partial
root.

The import-time `mark_all_fully_loaded` shortcut is removed. Imports begin with a fully resident
empty root and preserve completeness through ordinary commits.

## Typed reads and export

Snapshot readers provide exact-key reads and streaming scans. Scan request types permit full scans,
object prefixes for UUID relations, and object/entity prefixes for metadata. They accept no
arbitrary Rust predicate. Runtime predicates operate on resident indexes.

Cursors yield one logical tuple per advance. Property reconstruction retains its existing record and
append-byte limits. A cursor can report a later decoding error after earlier rows have been
consumed.

The scan order is object-reference byte order, then UUID bytes and metadata name. It is not numeric
object order. All readers use the same object order for streaming export joins.

The shared `snapshot_loader` assembles object attributes, ancestry, verbs, properties, and metadata
from typed readers. Fjall decoding and property reconstruction remain in the adapter. Readers and
cursors retain snapshot leases. For temporary databases, those leases also retain the storage
directory after database shutdown.

## Invariant failures

A missing entry in an incomplete index returns `IncompleteIndex`. Full scans reject incomplete
indexes. Commit checks report the same invariant error; the engine does not convert it into a
retryable write conflict.

The incomplete-index boundary is the future fault insertion point. A pager must supply
transaction-view and expected-revision information there. This stage adds no historical-read
interface or runtime paging.

The resident scan refactor also fixes predicate shadowing. If a local update stops matching a
predicate, the old matching value no longer appears in the scan.

## Validation

The database tests cover snapshot-consistent seeding, sequence recovery, property-chain bootstrap,
typed prefixes, lazy decoding, and cursor lifetime. They also cover incomplete-index failures,
failed seeding, predicate shadowing, and export after later writes and database shutdown.

The broader database, compiler, kernel, daemon, and objdef test run passed 1,393 tests, with 14
ignored. The final database run passed 243 tests, with one ignored, after adding the shutdown/export
regression.

Workspace Clippy passed with all targets and features. The existing dependency notice for
`proc-macro-error2` remains.

## Performance

The comparison uses the steps 1–2 held-permit binaries recorded in
`docs/benchmarks/fjall-persistence-results.json`. Their hashes were checked before the comparison.
It measures the incremental stage 3 change; it does not replace the original pre-refactor
performance gate.

The harness completed 48 runs: two builds, eight workloads, and three repeats. Build order
alternated between repeats. No compilation ran during measurement. The foreground workers used CPUs
5 and 5–8, as in the previous qualification.

| Median measurement                       | Steps 1–2 |   Stage 3 | Change |
| ---------------------------------------- | --------: | --------: | -----: |
| One worker, commits/s                    |   171,580 |   183,546 |  +7.0% |
| One worker, p95 us                       |     7.184 |     6.736 |  -6.2% |
| Four workers, commits/s                  |   213,589 |   216,703 |  +1.5% |
| Four workers, p95 us                     |    33.904 |    33.424 |  -1.4% |
| History rollup, producer ms              |    20.713 |    20.749 |  +0.2% |
| History pressure, producer ms            |    23.246 |    22.891 |  -1.5% |
| History replace, producer ms             |    20.018 |    19.097 |  -4.6% |
| History rebuild, producer ms             |   124.519 |   126.711 |  +1.8% |
| Write overwrite, mutations in 10 seconds | 1,966,080 | 1,966,080 |   0.0% |
| Write append, mutations in 10 seconds    | 2,488,320 | 2,488,320 |   0.0% |

The small four-worker difference is within the spread of individual runs. These measurements do not
isolate the cause of the single-worker improvement.

The incremental comparison shows no foreground regression beyond the proposed 5% threshold. The
original pre-refactor gate remains open; this run does not repeat that comparison. Applied and
durable waits remain separate in the raw results.

A separate import/export comparison produced nine byte-identical files from the benchmark core with
tests disabled. This checks shared export assembly against the steps 1–2 output.

The [results artifact](benchmarks/fjall-persistence-stage-3-results.json) contains commands, binary
and fixture hashes, changed-source hashes, per-run counters, export timings, and export equivalence
hashes.

To repeat the qualification with saved binaries:

```sh
python3 scripts/qualify-fjall-persistence.py \
  --variant steps-1-2 /tmp/moor-persistence-candidate-moorc /tmp/moor-persistence-candidate-micro \
  --variant stage-3 /tmp/moor-stage3-moorc /tmp/moor-stage3-micro \
  --output /tmp/moor-stage3-qualification
```
