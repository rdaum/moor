# Pluggable Persistence and a Readable PostgreSQL Backend

Status: proposed

Date: 2026-09-29

## 1. Purpose and decisions

Make persistence a replaceable component of `moor-db`, retain Fjall as the default, and provide an
optional PostgreSQL backend whose contents can be inspected with ordinary database tools.

The organizational reason for this work is concrete: some operators prefer a conventional relational
database with readable records, established backup and restore procedures, and familiar access
control and monitoring. PostgreSQL should store recognizable world data, not just move the existing
opaque byte buffers into `bytea` columns.

The proposed decisions are:

1. Keep mooR's in-memory transaction engine, conflict rules, and publication mechanism. Replace the
   backing persistence system underneath it.
2. Split the current `Provider` responsibilities into snapshot reads, logical commit submission,
   backend encoding/application, and lifecycle/maintenance operations.
3. Submit complete cross-relation commits to a bounded background pipeline. Ordinary transactions
   must not wait for PostgreSQL network requests or WAL flushes.
4. Use one PostgreSQL table per existing relation, plus explicit bookkeeping tables and readable
   views. Keep the bounded property append representation.
5. Store general MOO values as versioned MOO literal text. Store object keys and references as
   canonical identity-preserving MOO literal text directly in the tables. Use native SQL fields for
   property/verb UUIDs, names, flags, permissions, and other simple relation components.
6. Store verb programs and lambda bodies as actual decompiled MOO source and recompile on load.
   Compiled programs, packed opcodes, and FlatBuffers are not authoritative PostgreSQL payloads.
7. Use libpq through a small Rust adapter on dedicated threads. Neither the adapter nor feature
   activation may introduce a direct or transitive Tokio dependency to the daemon.
8. Gate PostgreSQL support behind a Cargo feature and select the compiled-in backend at runtime.
9. Preserve `SnapshotInterface` and the streaming snapshot-to-objdef path.
10. Support one active mooR writer per world. Direct SQL inspection is supported; live SQL mutation
    and multiple independent mooR writers are outside this design.
11. Preserve a path to eviction and lazy loading. Fully loaded indexes are the initial operating
    mode, not a permanent requirement of the provider interface. Future cache misses must resolve
    against the transaction's logical view, including changes not yet applied to storage.

These are proposed implementation decisions, not claims about an existing PostgreSQL backend. This
document includes acceptance criteria; no PostgreSQL performance results have been measured yet.

## 2. Scope and boundaries

This work covers the world-state relations owned by `moor-db`, their sequence state, and their
snapshot/export interfaces. It does not move the daemon's suspended-task database, connection
registry, or event log into PostgreSQL. Those stores have separate implementations and recovery
semantics.

A PostgreSQL world backup can restore the complete persisted world, including its programs and
property values. It is not by itself a backup of all daemon state. Deployment documentation must
make this distinction explicit rather than imply that one `pg_dump` restores running sessions and
suspended computations too.

The initial backend does not:

- Enable eviction or lazy loading for transaction reads in the first release.
- Allow a world larger than mooR's available RAM merely by selecting PostgreSQL.
- Replace mooR's conflict detection with PostgreSQL transaction isolation.
- Make a successful ordinary MOO transaction imply durable storage completion.
- Provide active/active world engines or cache invalidation for external SQL writers.
- Depend on a PostgreSQL server extension, PL/Rust, or server-side MOO compiler.
- Promise source compatibility across arbitrary future language/compiler versions.

Retain the current Fjall on-disk format during this refactor: no new epoch, commit-counter,
timestamp-high-water, or durability-fence keys, and no `CURRENT_DB_VERSION` bump. PostgreSQL gets an
independent format version and persistent bookkeeping (§7.4). Fjall continues to recover its
transaction timestamp from surviving tuples; it does not promise timestamp uniqueness across reopens
after deleting the newest tuples. Stronger Fjall recovery would be a separate format change. Do not
build compatibility layers between unrelated physical formats; objdef import/export is the initial
backend conversion path.

Eviction and lazy loading are deferred implementation work, but their requirements constrain this
interface now. The initial fully resident mode keeps its performance guarantees. Section 5.8
describes the additional consistency, retention, and indexing contracts required for a later paged
mode; simply enabling the remaining provider fallbacks is insufficient.

## 3. Current architecture and missing abstractions

### 3.1 Reads and publication

[`MoorDB::try_open`](../crates/db/src/engine/moor_db.rs) initializes all relations and seeds
immutable in-memory indexes from their providers. `object_propvalues` has a special seed path that
reconstructs full values from bounded record chains. The indexes are marked fully loaded.

Transactions use snapshots of those indexes. Write commits prepare changed indexes, check conflicts,
and publish a new world root with a compare-and-swap operation. Read-only commits can publish cache
updates without generating backing-store writes.

`WorldStateSnapshot` already stores `Arc<dyn RelationIndex<K, V>>`, independently of the storage
backend. Preserve that root representation and its existing lookup path. `Provider` is nevertheless
more than a startup API: `RelationTransaction` fallback reads/scans and `CheckRelation` conflict
checks call it when `provider_fully_loaded` is false. Those branches require explicit treatment in
the refactor (§5.3); fully resident production reads already bypass them.

Consequently, a PostgreSQL backend does not inherently add a SQL request to each property read or
verb lookup. That separation is a requirement of this design.

### 3.2 Background persistence

[`commit_pipeline.rs`](../crates/db/src/engine/moor_db/commit_pipeline.rs) obtains a persistence
admission permit before publishing a write root. After publication, it converts the working sets
into a commit batch and submits the batch for background processing.

[`batch_writer.rs`](../crates/db/src/provider/batch_writer.rs) has parallel encoders and one ordered
writer. Encoder completion can be out of order; the writer waits for consecutive publication
versions. A Fjall batch atomically applies changes across relation keyspaces and sequence records.

The existing capacities are 1,000 encoder/admission slots and 64 writer-message slots. These are
implementation settings, not an architectural guarantee that all possible queued payloads fit a
fixed memory budget. The encoder drops the admission permit when it picks up a batch, before
encoding and application. Thus the existing admission count does not bound all unapplied commits.
Holding permits until application is a deliberate proposed change (§5.5), not current behavior.

Publication version and transaction timestamp are distinct. Publication version expresses commit
order. A transaction timestamp was allocated earlier and must not be used to reorder persistence.

### 3.3 Why implementing the existing trait is insufficient

[`Provider`](../crates/db/src/provider/mod.rs) currently exposes `get`, `put`, `del`, `scan`, and
`stop`, with `Clone` as a supertrait. It leaves these contracts implicit or absent:

- Atomicity across multiple relations.
- Consistent snapshot identity across reads and scans.
- Commit ordering and persistence admission.
- The distinction between application to storage and durable flushing.
- Streaming startup scans rather than a complete temporary `Vec`.
- Error handling after the world has already been published.
- Backend lifecycle, capabilities, and statistics.

The generated relation types explicitly use `FjallProvider`. Commit operations carry Fjall keyspaces
and byte slices. `MoorDB` directly opens Fjall, reads sequence keyspaces, requests Fjall snapshots,
and exposes Fjall maintenance data. PostgreSQL therefore requires a persistence boundary refactor
rather than just another implementation of five methods.

`CheckRelation::apply` in [`tx/apply.rs`](../crates/db/src/tx/apply.rs) calls `put` and `del`,
including an ignored write error and an unwrap, but its callers are tests and
`tx_relation_benches.rs`. Production already uses `prepare_apply_all` → `working_sets_to_batch` →
`BatchWriter`; there is no second production apply path to consolidate. Relocate/delete this helper
and adapt its tests/bench to prepared index changes and explicit test persistence, without
preserving silent error handling.

## 4. Required invariants

Both backends must satisfy the following contract:

| Invariant                               | Meaning                                                                                                               |
| --------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| In-memory reads                         | Fully loaded runtime transactions do not perform persistence I/O.                                                     |
| Residency is not existence              | Evicted or not-yet-loaded data must not be mistaken for a missing tuple.                                              |
| Version-correct fault reads             | A future cache miss retrieves the transaction-visible version or explicitly fails; it never substitutes latest state. |
| Atomic application                      | A logical commit's relation changes and sequence changes become visible together.                                     |
| Ordered application                     | Storage exposes a complete prefix of world publication order.                                                         |
| Admission before publication            | Overload can reject or delay an unpublished commit; it cannot silently discard a published one.                       |
| Eventual completion or explicit failure | Published commits are applied, retained for ordered recovery, or cause a reported fatal persistence state.            |
| Snapshot consistency                    | All relations and both export passes use one stable storage snapshot.                                                 |
| Timestamp preservation                  | Logical tuple timestamps survive recovery independently of physical record order.                                     |
| Explicit durability                     | Applied and durably flushed are different acknowledgments.                                                            |
| Bounded append work                     | A normal accepted list append encodes its suffix, not the entire preceding list.                                      |
| Readable PostgreSQL storage             | Persistent application values have a documented textual or native SQL representation.                                 |
| No Tokio introduction                   | Enabling PostgreSQL adds no Tokio path to the daemon dependency graph.                                                |

External SQL reads can lag the latest published in-memory root. This is expected and measurable.
Several SQL statements need a repeatable-read transaction if the reader wants a consistent world
view across them.

## 5. Improving the Provider interface

### 5.1 Separate reads, writes, and coordination

Use four cooperating components:

1. **Storage opener:** checks format/capabilities, claims writer ownership, and obtains a consistent
   startup snapshot.
2. **Snapshot reader:** yields typed logical relation tuples, supports point reads and bounded
   scans, and supplies a snapshot export adapter. Its read-view identity and lifetime also provide
   the basis for future transaction fault reads.
3. **Persistence coordinator:** owns admission, submitted publication versions, waiters, health,
   encoder workers, and shutdown sequencing.
4. **Backend writer:** encodes/applies backend-specific batches and establishes storage snapshots or
   durability fences at ordered boundaries.

Per-relation readers no longer own background workers or expose individual mutation methods. The
small `EncodeFor<T>` concept can remain useful inside a backend; it is not the cross-backend commit
interface.

Do not force PostgreSQL rows through `ByteView` or Fjall framing. Similarly, Fjall should retain its
existing byte representations and reusable builders rather than encode through an intermediate text
or generic JSON representation.

### 5.2 Typed, snapshot-bound relation reads

The read interface should return logical tuples, including their timestamps. A property read yields
one reconstructed `Var`, regardless of the physical number of records.

An illustrative shape is:

```rust,ignore
struct StoredTuple<K, V> {
    timestamp: Timestamp,
    key: K,
    value: V,
}

trait RelationReader<K, V>: Send {
    fn get(&mut self, key: &K) -> Result<Option<StoredTuple<K, V>>, StorageError>;

    fn scan<'a>(
        &'a mut self,
        request: ScanRequest<K>,
    ) -> Result<Box<dyn TupleCursor<K, V> + 'a>, StorageError>;
}

trait TupleCursor<K, V> {
    fn next(&mut self) -> Result<Option<StoredTuple<K, V>>, StorageError>;
}
```

These sketches specify responsibilities, not a final Rust API. Cursors own bounded buffers; `next`
does not imply one database request per tuple. A callback or batched visitor with equivalent
semantics is also acceptable if measurements favor it for startup.

Readers are constructed from a common snapshot handle. The generated relation registry can provide
typed reader methods for each relation without putting generic methods on an object-safe trait.
Share handles through `Arc` where needed instead of requiring every trait object to implement
`Clone`. A PostgreSQL snapshot is `Send` but need not be `Sync`.

Make the reader's view identity and lifetime explicit. `None` means authoritative absence within
that view; unavailable history, expired views, and I/O failures are errors. An implementation must
not interpret `get` as a latest-state read merely because its payload cache missed. The initial
readers serve startup and export; the same typed logical read boundary must remain usable by a
future version-aware fault resolver. An export snapshot that includes at least a publication is not
automatically the exact view needed by a transaction at that publication.

Replace the arbitrary Rust predicate in `scan` with explicit scan requests: initially full relation
scan, exact key lookup, object-prefix scans, and entity-metadata prefixes
`(object_ref, entity_kind, entity_uuid)`. The latter are necessary for property/verb metadata on a
particular holder. Generate relation-specific request enums rather than permit nonsensical prefixes
on every key type. Existing value-predicate scans in `ws_transaction.rs` remain in-memory in the
fully resident runtime. Startup/export filtering beyond these requests is client-side; an arbitrary
Rust closure does not become a SQL `WHERE` clause.

Ordering must be explicit. Full startup scans may be unordered. Export readers promise a consistent
object order and, for compound relations, a UUID/record order within each object. PostgreSQL need
not reproduce Fjall's byte ordering. Physical ordering is not a MOO language semantic.

Startup also needs physical information that this logical reader intentionally does not return. Use
backend-private seed routines to stream tuples into common index builders while reconstructing
record counts, accumulated encoded append bytes, record-sequence recovery, and property names for
writer diagnostics. Return `SeededWorld { root, sequences, writer_bootstrap }`, where
`writer_bootstrap` is an internal backend enum consumed by the matching writer. The Fjall variant
contains its existing chain/name state; PostgreSQL contains its own chain/name state and progress
counters. Do not infer physical chain sizes from a reconstructed `Var` or expose backend records in
the transaction index. Reuse each backend's reconstruction logic for seed and export.

The opener owns a single startup snapshot and passes a borrowed handle to **every** relation seed
and bookkeeping read. Replace independently opening relation reads in `Relations::init/snapshot`
with this session-taking seeder. Current Fjall startup seeds relations independently before writes
start; PostgreSQL explicitly uses one repeatable-read transaction. Any later parallel SQL seed
workers must import that same snapshot. No root becomes available until all seeds succeed.

### 5.3 Fully resident first, with a future fault-read boundary

Make fully loaded indexes an explicit invariant of the initial operating mode. Once startup seeding
finishes, that mode's runtime reads use the indexes and issue no provider I/O. This preserves
current performance without making full residency a permanent property of the transaction engine.

Replace provider fallbacks in production `RelationTransaction` point reads/scans and `CheckRelation`
conflict checks with an explicit `IncompleteIndex` invariant error if reached in this mode. Startup
must verify completeness before publication. These current-state fallbacks cannot retrieve a
replaced older value and must not remain silently callable against PostgreSQL or Fjall.

The future fault boundary is an architectural insertion point, not an unused historical-read API to
implement in step 3. That step adds no fault resolver or placeholder history methods. A later
resolver will take `(transaction_view_lease, relation, key, expected_revision)` and return that
revision's value, authoritative absence supported by the view, or an explicit
`UnsupportedReadView`/`ExpiredReadView`/I/O error. It must merge the pinned publication overlay and
hold a retention guarantee (§5.8). An arbitrary timestamp or current backend `get` cannot substitute
for this contract. Typed storage readers stay outside resident lookups and can supply this resolver
once a retention strategy exists.

Never mask a partially seeded index as fully loaded. Retaining one startup PostgreSQL transaction
forever is also unsuitable: it would retain old row versions indefinitely. Future paged operation
needs a deliberate read-view and history-retention design, rather than either of those shortcuts.

### 5.4 Logical batches

Move backend-independent mutation preparation before physical encoding. A logical batch contains:

```rust,ignore
struct LogicalCommit {
    publication: PublicationId,
    timestamp: Timestamp,
    changes: RelationChanges,
    sequences: Vec<SequenceUpdate>,
}

struct PublicationId {
    epoch: WriterEpoch,
    version: u64,
}

enum PropertyMutation {
    Replace(Var),
    AppendList { suffix: List, final_value: Var },
    Delete,
}
```

`RelationChanges` is a generated typed aggregate with a vector/working set per relation. Do not
erase all domain values to `Any`, strings, or serialized bytes. Keep the normal relation mutation
unit the same as the existing engine: a `VerbDefs` update still changes one object's ordered
definition set/row.

Classify property append candidates while the working set still contains the base index. Preserve
the existing proof of prefix equality and its bounded work budget. A hint alone is insufficient.
Property conflict-policy handling, including clobber behavior, stays above persistence. The backend
must apply the engine's accepted mutation; it must not independently merge overlapping appends.

The final value retained on an append supports an occasional rollup. Retain it by the existing cheap
shared-value clone. Do not traverse or render it on every append.

The working-set map has one entry per relation key, so one logical commit contains at most one
accepted mutation per property. Preserve that guarantee during batch preparation and grouping.

Capture sequences at persist submission, after successful root publication, as today: claim the
global dirty-slot mask and load the selected slots into `LogicalCommit`. They are allocation
high-water observations, not transaction-local writes. Allocations from aborted transactions can be
captured by the next submitted commit; allocations with no subsequent persisted batch can be lost on
restart. Do not imply that abort rolls them back or that shutdown currently persists every
allocation independently. Concurrent submitters can capture values out of publication order, so
ordered application alone does not prevent a lower observation from arriving later. Both writers
apply each captured slot using a maximum with confirmed/tentative writer state; PostgreSQL also uses
`GREATEST` on upsert. Fjall writes the resulting maximum in its existing sequence encoding. This is
an explicit monotonicity hardening, with a reordered-submission regression test.

`PublicationId` is neutral: each open creates a fresh random writer epoch and starts runtime
publication versions at zero. Tokens name published roots within that open, not tuple timestamps or
persistent record sequences. Fjall's epoch exists only in memory; PostgreSQL records it for
ownership and recovery. Tokens from a previous open are always `StalePublication`, even if their
numeric version has already been reached again. They are not portable backup/restart bookmarks.

Backend encoders consume this batch directly. Fjall produces its current binary records. PostgreSQL
produces native parameters and literal/source text. Backend-specific encoded-batch types stay inside
the worker implementation.

### 5.5 Coordination and acknowledgment

The engine-facing handle should support these operations:

| Operation      | Contract                                                                              |
| -------------- | ------------------------------------------------------------------------------------- |
| `admit`        | Obtain a permit before publication, with the existing bounded wait/rejection policy.  |
| `submit`       | Transfer a published logical batch and its permit to the coordinator.                 |
| `wait_applied` | Wait until a complete storage prefix includes the requested publication.              |
| `wait_durable` | Additionally establish the documented durable-storage boundary.                       |
| `status`       | Return published/submitted/applied/durable progress and health where known.           |
| `shutdown`     | Stop admission, drain published work, resolve waiters, and release backend resources. |

`wait_applied` replaces the ambiguous internal notion of a persistence barrier. The existing public
`wait_for_persistence` can retain its application-level meaning while its documentation points to
the explicit durable alternative. A backend must report an unsupported durability request rather
than treating application as a flush.

`wait_applied(id, deadline)` and `wait_durable(id, deadline)` accept a `PublicationId` captured from
a published root in this open. Reject stale epochs and unpublished targets; callers cannot reserve
an arbitrary future version. Return a receipt for the proven prefix, or a typed error including
`Timeout`, `StalePublication`, and terminal writer failure. The finite deadline covers queue
submission and receipt waiting, not just the final receive. A timed-out waiter is removed; its
published commit remains owned by the pipeline and is not cancelled.

Keep the existing public `wait_for_persistence()` as an explicitly unbounded applied wait, including
terminal-failure/shutdown notification, for callers that require that behavior. Add a
deadline-taking variant and use it for administrative operations. `create_snapshot()` retains a
default 10-second acquisition deadline, now covering the whole operation, with an explicit override.
Current Fjall snapshot code bounds its reply receive by 10 seconds but its queue send can block;
fixing that gap is part of implementing deadlines. Snapshot acquisition stays in the backend factory
behind `TxDB::create_snapshot`, returning `Box<dyn SnapshotInterface>` (§12), not a neutral trait
method returning either `fjall::Snapshot` or a borrowed SQL transaction.

Shutdown closes admission and rejects new snapshot acquisitions. Previously admitted foreground
attempts must finish publishing/submitting or abandon their reservation before the coordinator seals
the final publication target; draining only the root visible at shutdown entry would race those
attempts. All phases share the finite shutdown deadline. Then drain through that final target.
Existing waiters can complete during the drain; failure or expiration of the shutdown deadline wakes
them with an error. Satisfied prefixes can still return their proven receipts while draining.
Shutdown itself has a finite configured deadline. Existing snapshot loaders own independent read
resources and can outlive the writer until dropped; shutdown does not wait indefinitely for them.

Use a typed error taxonomy: open/configuration, incompatible format, invalid stored data, encoding,
unavailable transport, ambiguous commit outcome, ownership loss, overload, and fatal writer failure.
Include relation/key and publication identity without logging complete property payloads or
connection secrets. Retry only errors with a defined recovery procedure.

Deliberately extend permit lifetime from encoder pickup to successful application for **both**
backends. The initial 1,000-permit setting now caps admitted, not-yet-applied write commits,
including prepublication reservations, encoding, reordered results, and in-flight writes. It remains
a count bound rather than a byte bound. After a grouped SQL commit succeeds, advance the prefix and
release all member credits together under coordinator accounting; no member releases capacity before
group success. Rollback/reconnect retains permits. On terminal failure close admission before
releasing credits and failing outstanding work, so failure cannot admit further publications.

Keep the existing prepublication wait/rejection policy, but measure this tighter backpressure
envelope as a distinct Fjall behavior change. It may throttle sooner during slow storage/encoding;
the performance gates in §14 apply, and a regression requires revisiting capacity or the design.
Submission after publication must not encounter an ordinary queue-full rejection. Retain a permit
through failed CAS retries or release it if the unpublished attempt is abandoned. A disconnected
worker after publication is a coordinated fatal persistence failure.

Measure queued encoded bytes and retained values before adding a byte budget. If a byte budget is
added, define an oversize single-commit path so bulk imports cannot wait forever for a permit that
can never fit. Do not recursively measure large values on the transaction thread for accounting.

### 5.6 Dispatch and generated code

Keep backend choice at database construction and worker boundaries. An internal backend enum is
adequate for two statically compiled alternatives; a dynamic-plugin ABI is unnecessary.

The relation macro should describe names, domain/codomain types, indexes, and mutation categories.
It should no longer know about Fjall keyspaces, SQL connections, or compaction functions. Backend
factories map the registry to storage resources. Generate typed batch/reader plumbing from the same
registry to avoid inconsistent lists of relations.

Use this concrete initial shape:

- Production `Relation<K, V>`, `RelationTransaction<K, V>`, and `CheckRelation<K, V>` lose their
  `Source = FjallProvider<K, V>` parameter and provider fields. They keep their typed index and
  working-set machinery. Incomplete-index branches fail as specified in §5.3; future faults add a
  transaction-view resolver at that boundary, not a backend parameter threaded through the VM.
- The macro emits `Relations`, `RelationCheckers`, `WorldStateTransaction`, `WorldStateSnapshot`,
  `RelationChanges`, relation IDs, and typed reader/seed dispatch from the same registry.
  `working_sets_to_changes` replaces `working_sets_to_batch` and returns `RelationChanges` without
  keyspaces or bytes. `LogicalCommit` wraps those changes and sequence observations.
- A feature-gated internal `StorageBackend::{Fjall, Postgres}` enum dispatches opening, seeding,
  snapshot-loader construction, and maintenance. Each variant starts its own encoder/writer behind
  the common coordinator. Fjall codec/resource bindings live in the Fjall adapter; SQL bindings live
  in the PostgreSQL adapter. Neither goes into generated transaction fields.
- Registry entries explicitly declare `Ordinary` or `PropertyValueChain` mutation category, plus
  index/policy metadata. Replace the field-name matches for `object_propvalues` in `@seed_relation`,
  `@encode_working_set`, and `@check_relation` with category dispatch. Shared conflict and
  append-candidate logic remains in the engine; physical chain seeding/encoding belongs to each
  backend. Tests/benchmarks use seeded indexes and explicit test sinks instead of requiring the
  production relation type to retain direct `put`/`del`.

There should be no new virtual call or backend branch on every resident property lookup. Cold
startup, export, and future fault reads may use trait objects. Future residency checks should be
kept cheap and measured independently of backend dispatch. Do not genericize the kernel and VM
around a database backend parameter.

### 5.7 Backend-specific maintenance

Replace a universal-looking structure filled with Fjall counters with a common progress/health
structure plus backend-specific maintenance data. Fjall reports journal/memtable/compaction state;
PostgreSQL reports applicable size and connection/write statistics.

Compacting a Fjall relation and vacuuming a PostgreSQL table are different operations. PostgreSQL
must not silently implement a compaction request as `VACUUM FULL`, which can lock and rewrite the
table. Expose unsupported operations and describe any separately supported maintenance command.

### 5.8 Preserving a path to eviction and lazy loading

#### Intended progression

Separate three increasingly demanding modes:

1. **Fully resident:** the initial implementation, with all keys and payloads in memory.
2. **Resident key directory, evictable payloads:** retain authoritative key/existence/version
   metadata and small structural relations, but fault large property values and compiled programs.
3. **Paged keys and indexes:** also load key ranges and structural/secondary-index state on demand,
   with complete range enumeration and conflict checking across unloaded data.

The second mode is a useful first target because property values and programs can dominate memory.
It still requires memory proportional to key count and does not promise arbitrary world size. The
third mode is a separate indexing project. Avoid making it a prerequisite for the first PostgreSQL
backend, but do not define APIs that equate residency with database membership.

#### Gaps in the current code

The existing abstractions retain signs of lazy loading, but not a complete paging contract:

- [`RelationIndex`](../crates/db/src/tx/indexes.rs) returns either a resident `Entry<V>` or no
  entry. It has one `provider_fully_loaded` bit, not independent keyspace-completeness and
  payload-residency information. Its hash implementation removes the key for both a tombstone and
  entry removal.
- [`RelationTransaction`](../crates/db/src/tx/transaction.rs) has fallbacks that call a provider's
  current `get`/`scan` and then compare tuple timestamps with `visible_ts`. If storage now contains
  a newer replacement, rejecting that replacement does not retrieve the older visible value.
- The exact rebase path in [`relation_key_unchanged`](../crates/db/src/engine/relation_defs.rs)
  deliberately fails safe when indexes are not fully loaded. Eviction must not turn into perpetual
  conflict retries or make unknown state compare equal to absence.
- Root snapshots and working sets hold actual shared values. Removing a value from a separate cache
  does not free it while those structures retain it. Real payload eviction requires changing value
  ownership or indirection, not just adding an LRU beside the existing indexes.
- The property append classifier currently obtains its base value from the working set's base index.
  A paged implementation must provide that version or safely choose replacement encoding.

These observations motivate explicit contracts. They do not establish that every existing fallback
caller is incorrect in its current fully resident use.

#### Distinguish presence, residency, and coverage

An index needs to answer separate questions: does the key exist in this logical view, which revision
is visible, and is its payload resident? A conceptual result is:

```rust,ignore
enum IndexLookup<V> {
    Resident { revision: TupleRevision, value: V },
    NonResident { revision: TupleRevision, locator: ValueLocator },
    Absent { proof: AbsenceProof },
    Unknown,
}
```

These are design concepts, not proposed immediate allocations on every lookup. A complete immutable
key directory can prove absence by missing membership; it need not allocate an absence record per
possible key. A partial directory needs coverage information and/or versioned negative entries.
Committed deletes not yet reflected in storage require an overlay tombstone to suppress stale rows.

Split the meaning of `provider_fully_loaded` into keyspace/range completeness and payload residency
when paging is implemented. Removing only a payload must preserve metadata needed for existence,
conflict checking, reverse indexes, and transaction visibility. Loading a payload changes cache
state, not logical world state: it must not generate a commit or alter its tuple timestamp.

Cache keys must include relation, logical key, and tuple revision/read-view identity. A negative
result is also view-specific. Concurrent misses can share a load for the same version; a load
started for one root must not overwrite a newer value in another root when it completes.

#### The asynchronous persistence gap

Suppose storage is applied through publication 100 while the current in-memory root is 105. A
transaction on root 105 may need a value changed at 103. A storage-only miss would return the wrong
value even if no newer transaction existed.

Published but unapplied payloads and deletion markers must therefore remain recoverable in memory,
or in another explicitly durable/versioned layer, until the backing store can supply the required
version. The existing encoder/writer queue is not automatically such a read layer: it can contain
consumed or backend-encoded operations and has no versioned lookup contract.

The lowest-complexity first policy is to pin unapplied versions and use a shared versioned overlay
for them. Fault resolution consults transaction-local changes, the correct published overlay, and
then a compatible storage view. Advancing the applied watermark permits some overlay reclamation; it
does not prove that every older active transaction's version can be discarded.

During an outage, memory pressure must reduce admission or report overload rather than evict the
only copy of an accepted value. The dirty overlay, encoder buffers, transaction working sets, and
old root references all count toward memory use. Memory limits and persistence backpressure must be
designed together. Ordinary writes should still avoid waiting for SQL; exceptional pressure is an
explicit backpressure condition, not a silent data-loss policy.

#### Old roots after storage advances

The reverse problem is equally important. Transaction A holds root 100, transaction B publishes 101,
and storage applies 101. If A's payload from 100 is evicted, the current database row may no longer
be the value A is entitled to read.

Neither `wait_applied(100)` nor a newly acquired export snapshot that includes 100 solves this.
Filtering on `logical_timestamp <= visible_ts` cannot recover overwritten values, and transaction
timestamps are not a total publication order. Deletion and delete/reinsert need the same care.

A paged implementation must choose an actual retention strategy:

| Strategy                                                     | Benefits and costs                                                                                                                                                                                         |
| ------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Retain overwritten versions needed by active roots in memory | Avoids persistent historical rows but can consume substantial memory under long transactions/write churn. The previous payload must already be retained or be fetched safely before storage overwrites it. |
| Pin backend snapshots by read generation                     | Lets the backend retain visible row versions. Fjall has native snapshots; PostgreSQL needs live snapshot transactions, connection/lease management, and vacuum pressure controls.                          |
| Persist explicitly versioned readable rows                   | Supports version-selective reads through short SQL sessions, but adds schema/index/write amplification and an explicit history-GC protocol.                                                                |

A backend snapshot can be shared across a generation with a versioned overlay for later
publications. The generation must record which storage prefix it represents. Reclaim it only when no
active root, fault, export, or overlay reference needs it. This is a possible design, not a reason
to keep one startup SQL snapshot open indefinitely or allocate a SQL connection for every MOO
transaction.

For PostgreSQL, MVCC row history is not a general historical lookup API. A repeatable-read
transaction opened after an overwrite cannot ask for an earlier row merely by its application
timestamp. An exported PostgreSQL snapshot likewise requires a live transaction and cannot be
reconstructed later from the token. The initial current-state tables do not promise historical
reads.

Whichever strategy is selected, make revision lifetime and retention explicit. A `ValueLocator`
identifying an old row is useless if the provider has already reclaimed that row. The caller must
hold a read-view lease or a version pin, or receive an explicit unsupported/expired-view error.
Never downgrade such an error to `None` or silently refresh the transaction to the latest root.

#### Provider capabilities to preserve now

Keep a distinction between these operations:

- Acquiring a stable startup/export storage snapshot at or beyond a requested applied prefix.
- Obtaining a transaction-compatible read view, with its storage prefix and overlay requirements.
- Loading a specified logical revision under a retention guarantee.
- Enumerating a complete key range under that same view.

The first is required immediately. The others are contracts for the future paging design, not
placeholder methods added by this refactor; no API may imply that an arbitrary timestamp is
sufficient to request history. A future read-view handle should expose opaque backend state,
snapshot identity, lifetime/lease, and capabilities. Keep this separate from the current
at-least-version export contract.

Do not impose a history schema or new Fjall record format just to reserve these interfaces. Choose
the retention strategy in a dedicated eviction design and then add the necessary version metadata.
Logical transaction timestamps, publication IDs, persistent record sequences, and payload revisions
have different meanings and must not be casually substituted for one another.

#### Conflict checking, scans, and other consumers

Payload eviction should leave the resident key directory's revision comparisons sufficient for
common conflict/rebase checks. A comparison requiring actual values can fault them outside the final
publication step and then revalidate against the candidate root. Never hold a global publication
lock across disk or SQL I/O. A cache fill must not be counted as a conflicting write.

Scans in a paged mode must cover unloaded keys, merge the captured overlay and transaction-local
changes, suppress tombstoned/replaced backing rows, and avoid duplicates. A predicate applied only
to resident values is not a complete scan. Partial key indexes also need coverage/existence proofs
for duplicate checks and whatever read/range validation the engine requires; paging must preserve
the engine's isolation rules without assuming the backing database validates them.

Keep location/parent reverse indexes, ancestry, and resolution metadata resident in the first
payload-only phase. Evicting those structures later requires complete reverse/range lookups, not
just point `get`. An empty resident reverse-index bucket must not falsely mean no children or
contents exist.

Audit all value consumers, not only property lookup: anonymous-object reachability/collection,
memory accounting, property-policy checking, list-append preparation, program dispatch, and
export/validation must handle nonresident payloads. A reachability pass that visits only cached
values could lose references held in an evicted property or captured lambda.

Pin payloads used by active VM frames or working sets for the required lifetime. Evicting a compiled
PostgreSQL program means loading its source and compiling on the fault path, preferably on a bounded
worker pool. Neither fault coalescing nor compiler work needs Tokio. A thread may wait on a true
fault initially; parking/resuming MOO tasks without occupying a worker is a separate scheduler
integration. These cold-read latency and throughput costs must be stated explicitly.

Literal-backed values initially fault at whole-value granularity. One huge property can still
require substantial memory to parse and reconstruct, and append-chain reads can require a full base
plus suffixes. Paging the contents of a single list/map is another representation change, not an
automatic consequence of evicting relation payloads.

#### Rollout and measurements

Start an eviction prototype with property payloads and compiled verb programs while retaining
complete key/version metadata. Select and prove a version-retention strategy before enabling any
actual eviction. Then add bounded caching, version-specific fault coalescing, and dirty/history pin
accounting. Only after those work should lazy startup or partial key/index loading be enabled.

Measure actual reclaimed memory, including references held by immutable roots, the dirty overlay,
compiler caches, and active tasks. Add cache-hit rate, fault/coalescing counts, cold-read p95/p99,
fault compilation time, version-retention size, oldest live read view, and backend snapshot age.
Benchmark long readers overlapping writes, persistence lag/outage, and repeated eviction/refault.
Memory pressure must not cause endless conflict retry, unbounded history growth, or loss of accepted
state. Full-residency performance remains a separate regression baseline.

## 6. Build, dependencies, and client architecture

### 6.1 Cargo feature

Add an opt-in `postgres` feature to `moor-db` and forward it from the daemon, combined server, and
database tools that expose backend selection. Fjall remains available by default. Illustratively:

```toml
[features]
postgres = ["dep:pq-sys", "dep:moor-compiler", "dep:serde_json"]
```

Concrete forwarding is `moor-daemon/postgres = ["moor-db/postgres"]`,
`moor-server/postgres = ["moor-daemon/postgres"]`, and `postgres = ["moor-db/postgres"]` in each of
`tools/moorc` and `tools/moor-emh`. `moor-db` currently has neither `moor-compiler` nor `serde_json`
as a dependency; add them optionally for the PostgreSQL codecs. `moor-kernel`, `moor-common`,
`moor-var`, `moor-objdef`, and networking hosts stay backend-unaware and gain no PostgreSQL feature.
The compiler exports shared literal/source APIs without depending on a persistence backend.

This is a proposed manifest change, not an implemented feature. External dependency versions belong
in the root workspace manifest, with `.workspace = true` in members. The compiler dependency is
needed only for the PostgreSQL source codec; it should not impose source rendering on Fjall.

A default build must not build/link libpq or require PostgreSQL development headers. Selecting
PostgreSQL at runtime in a build without the feature is a clear configuration error. Keep config
parsing capable of identifying that error rather than silently falling back to Fjall.

A PostgreSQL-only world-state build may be useful later, but eliminating all Fjall linkage from the
daemon is a separate task because other daemon stores still use Fjall.

### 6.2 No Tokio

The commonly named synchronous Rust `postgres` client is unsuitable for this constraint: it wraps
`tokio-postgres` and owns a Tokio runtime. A synchronous-looking API does not satisfy the dependency
requirement. See the
[client implementation documentation](https://docs.rs/postgres/latest/postgres/).

Use the official libpq client via optional `pq-sys` bindings and a narrowly scoped safe adapter.
Review the selected bindings' dependency tree, licensing, and platform support before pinning the
version. The [pq-sys project](https://github.com/sgrif/pq-sys) documents linkage and build
discovery.

The adapter owns `PGconn` and `PGresult` through RAII, checks all result statuses and lengths,
retains parameter buffers for their required lifetime, and copies/decodes data before dropping its
result. Keep unsafe code localized and document each ownership/lifetime invariant.

Connections are owned by worker threads. No connection is manipulated concurrently. This matches
[libpq's threading contract](https://www.postgresql.org/docs/current/libpq-threading.html).

SQL execution may block its persistence worker, never an ordinary transaction worker. For bounded
deadlines and shutdown, the adapter can use libpq's nonblocking send/consume APIs with an OS polling
loop on that dedicated thread. This requires neither Rust futures nor an async runtime. Avoid an
uninterruptible `PQexec` as the final design for network operations with deadlines. See
[libpq command processing](https://www.postgresql.org/docs/current/libpq-async.html).

Use prepared parameters and bounded result fetching. libpq's pipeline mode is an optional later
optimization, not a reason to introduce Tokio. Its result/error state machine and restrictions on
COPY must be tested before adoption; see
[pipeline mode](https://www.postgresql.org/docs/current/libpq-pipeline-mode.html).

### 6.3 Connection and deployment policy

Use one writer connection, one startup reader initially, and a small configured limit on concurrent
snapshot/export connections. A snapshot connection is reserved for the snapshot lifetime and is not
shared with the writer. Future parallel startup readers must share a single database snapshot.

Acquire an export slot with a FIFO bounded wait using the caller's acquisition deadline. At the
limit, callers wait until a slot is returned or receive `Timeout`; an explicit zero-wait request
returns `ResourceBusy`. The same deadline covers the applied barrier, slot wait, connection setup,
and snapshot-establishing query. Check shutdown/failure during acquisition. This prevents indefinite
starvation without introducing an unbounded queue of connected PostgreSQL sessions.

The slot is held from `create_snapshot` until the loader and any borrowed export session are
finished, including time spent making point queries or waiting before `begin_export`. Expose the
limit and active snapshot ages; a long-lived loader intentionally consumes capacity. Queries/fetches
have their own finite I/O deadlines; acquisition timeout does not silently expire a successfully
returned snapshot halfway through an export. There is no automatic reconnect inside a snapshot.

The proposed initial compatibility target is PostgreSQL 17 and 18 with libpq 17 or newer; CI must
verify the chosen minimum. This is a support policy to validate, not a claim that earlier releases
lack the basic required SQL facilities.

Support libpq service configuration/password files and TLS settings. Redact connection details in
logs. Set application names for writer and export sessions. Require UTF-8 database encoding. Qualify
SQL objects by the configured, validated schema and exclude writable schemas from `search_path`.
Values use parameters; identifiers require explicit validation/quoting. See
[libpq connection configuration](https://www.postgresql.org/docs/current/libpq-connect.html).

Package PostgreSQL-enabled binaries with their libpq runtime requirement. Document development,
container, Debian, and cross-compilation requirements. Do not add a large ORM to avoid writing a
small number of fixed queries.

CI must examine the daemon's resolved dependency graph with and without `postgres`. Enabling the
feature must add no Tokio dependency path. The combined server already uses Tokio for other work;
that does not waive the standalone daemon constraint.

### 6.4 Configuration and opening a database

Separate backend selection from the existing Fjall table settings. A proposed configuration shape
is:

```rust,ignore
enum StorageConfig {
    Fjall(FjallStorageConfig),
    Postgres(PostgresStorageConfig),
}

struct PersistenceConfig {
    admission: AdmissionPolicy,
    shutdown_timeout: Duration,
}
```

Fjall configuration owns its path/temporary-directory choice and keyspace options. PostgreSQL
configuration owns a libpq service or connection specification, schema, connection/export limits,
connect/query/recovery deadlines, SQL commit policy, and grouping limits. Common configuration owns
publication admission and common lifecycle policy. Unsupported backend settings are errors.

Replace `TxDB::try_open(Option<&Path>, DatabaseConfig)` with
`TxDB::try_open(StorageConfig, DatabaseConfig, PersistenceConfig)` and make the internal `MoorDB`
opener accept the same separation. Add explicit `TxDB::try_open_fjall(path, config)` and
`TxDB::try_open_temporary(config)` convenience constructors using default persistence policy. Update
callers in tools/tests; no URL guessing or legacy path-dispatch shim. Backend selection must reach
daemon startup, the combined server, import/export, and maintenance through this constructor.

Add `--storage-backend=fjall|postgres` plus PostgreSQL service/schema settings. `--db` continues to
mean the Fjall **world** path; reject an explicitly supplied `--db` with PostgreSQL. Its implicit
default is simply unused in that mode. Argument parsing must retain whether it was explicitly set.
Resolve/log `paths.db_path` only for Fjall world opening. `--data-dir` still controls defaults for
the separate `tasks.db`, `connections.db`, and `events.db` stores, with their existing overrides. Do
not repurpose `--db` as an ancillary-store directory or a PostgreSQL connection string.

Make schema initialization explicit: a new empty target can be initialized transactionally by a
setup command; ordinary startup checks an existing schema and its format marker. A failed network
connection, missing required table, or incompatible format must not be treated as a fresh empty
world. Routine startup requires no DDL privileges. Schema evolution uses versioned, reviewable SQL
or an explicit objdef conversion; no automatic destructive upgrade is implied.

Place the common coordinator and logical batch definitions under `provider/`, with backend modules
for Fjall and PostgreSQL. Keep libpq ownership, PostgreSQL row codecs, and schema/query definitions
inside the feature-gated PostgreSQL module. Shared MOO source/literal functionality belongs in the
compiler crate; shared sparse export assembly belongs in the database export layer. Export the
supported configuration and administrative APIs from `moor-db`'s `lib.rs`.

## 7. Readable relational schema

### 7.1 Object keys and timestamps

Represent object keys and object-valued relation fields as canonical MOO object-literal text. Use
`object_ref` for the object key, and `parent_ref`, `owner_ref`, and `location_ref` for corresponding
relation values. The literal is the authoritative key, readable directly in base tables and joins.

| Object kind           | Example stored spelling   |
| --------------------- | ------------------------- |
| Traditional object ID | `#42`                     |
| Sentinel object ID    | `#-1`                     |
| UUID-style object     | `#048D05-1234567890`      |
| Anonymous object      | `#anon_048D05-1234567890` |

Use the identity-preserving object syntax already supported by objdef as the basis of one shared
formatter/parser. Ordinary `Obj::Display` prints anonymous objects as `*anonymous*` and must not
serve as the storage codec. Anonymous objects therefore need the explicit literal spelling, not a
separate numeric key representation. Actual property and verb UUIDs use PostgreSQL `uuid`.

Define one canonical spelling per identity: decimal IDs without redundant leading zeros or a plus
sign, fixed-width uppercase hexadecimal groups for UUID-style and anonymous IDs, and the exact
prefixes shown above. The parser validates each kind's representable range. The encoder always
writes canonical text, and startup/validation rejects noncanonical stored keys rather than allowing
two SQL keys to become the same `Obj`. Aliases such as `$name` are not stored identities.

Use a shared text domain with deterministic `C` collation for every object-reference column.
Snapshot scans and joins use the same literal ordering across relations; it need not be numeric
object order. Object references embedded in definition JSON use the same literal strings, and the
general value codec uses the same spelling inside MOO literals.

Readable object literals are the baseline design. Any proposed departure to save key bytes or
comparison time requires a demonstrated workload cost and a separately reviewed tradeoff; assumed
index savings do not override readability.

Use a checked SQL domain based on `numeric(20, 0)` for full-range mooR `u64` timestamps and
persistent commit/record counters. These are logical counters, not SQL wall-clock timestamps or
PostgreSQL transaction IDs. Preserve the existing `u128` anonymous-object microsecond counters with
checked `numeric(39, 0)` values. Range checks must include the actual Rust maximum, not just the
decimal column width.

Runtime sequence slots use signed `bigint` values. Persist their reserved high-water state as data;
do not replace the engine allocator with a network `nextval` call per object allocation. Gaps are
already possible. Concurrently collected monotonic high-water updates must never lower the stored
value, even if their collection and publication interleave.

### 7.2 Relation mapping

All tables below use `object_ref` as the object portion of their key, and ordinary tuples carry
`logical_timestamp`. Collection-valued relations initially retain one row per object. Definition
arrays preserve order and represent an empty collection explicitly.

| Table                       | Key after `object_ref`                                     | Value fields                                                     |
| --------------------------- | ---------------------------------------------------------- | ---------------------------------------------------------------- |
| `object_location`           | None                                                       | `location_ref`                                                   |
| `object_parent`             | None                                                       | `parent_ref`                                                     |
| `object_flags`              | None                                                       | `flag_names`, `flag_bits`                                        |
| `object_owner`              | None                                                       | `owner_ref`                                                      |
| `object_name`               | None                                                       | `name`, `name_encoding`                                          |
| `object_verbdefs`           | None                                                       | Ordered `definitions jsonb`; complete fields below               |
| `object_verbs`              | `verb_uuid`                                                | `source text`, `source_format`, `compiler_profile`               |
| `object_propdefs`           | None                                                       | Ordered `definitions jsonb`; complete fields below               |
| `object_propvalues`         | `property_uuid`, `record_sequence`                         | Record kind, timestamp, value/suffix literal, literal format     |
| `object_propflags`          | `property_uuid`                                            | `owner_ref`, `flag_names`, `flag_bits`                           |
| `entity_metadata`           | `entity_kind`, `entity_uuid`, `key_encoding`, `key_folded` | Original key spelling/encoding, value literal and literal format |
| `object_last_move`          | None                                                       | Value literal and literal format                                 |
| `anonymous_object_metadata` | None                                                       | Creation and last-access microsecond values                      |

Definition elements have these exact fields (all object references use §7.1 and symbol strings use
the escaped-string rule in §7.3 when necessary):

- `PropDef`: `uuid`, `definer_ref`, `location_ref`, `name`. Both definer and location are required.
- `VerbDef`: `uuid`, `location_ref`, `owner_ref`, `names` (ordered), `flags: { names, bits }`,
  `args: { dobj, prep, iobj }`. There is no verb `definer` field. `dobj`/`iobj` use `none`, `any`,
  or `this`; `prep` uses `none`, `any`, or the canonical spelling of its `Preposition` variant. The
  schema/source version identifies that spelling table.

For object/verb/property flag sets, preserve the full current `BitEnum` u16 mask in a checked SQL
integer `0..65535` (JSON number for definition flags) and include readable names for all recognized
set bits, including named obsolete bits. Bits are authoritative; names must agree with recognized
bits under the stored format version. Unknown bits survive load/write and appear numerically;
unknown names or a names/bits mismatch are format errors. A name-only encoding is not sufficient.

`object_contents` and `object_children` are reverse indexes over location and parent, not additional
persisted relations in the current registry. Add reverse SQL indexes only for demonstrated SQL
inspection workloads; the runtime builds its existing reverse indexes in memory.

The full metadata primary key is `(object_ref, entity_kind, entity_uuid, key_encoding, key_folded)`:
**the holder object is required**, including for inherited definitions sharing a UUID. Entity kinds
correspond to the current tags object/property/verb. Object metadata uses a fixed nil entity UUID;
property/verb metadata uses the actual definition UUID. Prefix reads on
`(object_ref, entity_kind, entity_uuid)` and deletion/rehome by holder must remain possible.

Compute `key_folded` with `Symbol::to_folded_case`, using deterministic `C` SQL comparison, and
retain `key_spelling` plus `key_spelling_encoding` as payload for the exact stored symbol spelling.
`key_encoding` encodes the folded key using §7.3; original spelling has its own encoding because the
fields serve different purposes. This gives engine-consistent case-insensitive key identity without
relying on PostgreSQL locale folding, while lookup/enumeration returns the stored spelling. Updates
take spelling from the accepted engine key, not from SQL lookup parameters. General symbol values
and definition names also preserve their original spelling. Current Fjall metadata-key encoding
folds spelling on disk; preserving it in PostgreSQL is an explicit stronger round-trip contract, not
a claim about the Fjall baseline.

Do not place broad foreign keys on arbitrary object-valued fields: sentinel values and references to
missing objects are valid parts of the language. Add checks for physical encoding invariants and
known relation constraints only. Persistence writes must not unexpectedly acquire application
semantics through database triggers.

### 7.3 Text edge cases

Normal object names and definition names remain ordinary text. PostgreSQL text cannot store NUL. For
an affected string, use an explicitly marked escaped JSON-string-literal spelling, including quotes,
in the text column; decode it using the recorded encoding. Use a canonical policy: raw UTF-8 when
representable, escaped spelling only when necessary. Include the encoding discriminator in a key
when required to avoid collisions between raw and escaped text.

This small string-escaping mechanism is separate from the value format: general property values are
MOO literal text, with language escapes that ensure the stored source contains no actual NUL bytes.
Definition JSON also uses explicit escaped-string envelopes where necessary. PostgreSQL JSONB does
not solve the NUL issue and cannot represent non-finite JSON numbers. See
[PostgreSQL JSON types](https://www.postgresql.org/docs/current/datatype-json.html).

### 7.4 Bookkeeping

For PostgreSQL, add explicit, dumpable tables for:

- Database identity, schema version, literal/source versions, and compiler profile policy.
- Active writer epoch and last applied runtime publication version in that epoch.
- Persistent commit sequence and maximum committed transaction timestamp.
- A checked signed-64-bit property-record sequence counter (§10.3).
- Sequence-slot high-water values.
- A durable-fence counter if needed to implement explicit durable barriers.

Keep writer epochs distinct from stable database identity. A restored database retains its data and
format identity but acquires a new writer epoch when opened. Recover transaction allocation above
the stored maximum timestamp, including commits whose newest tuples were later deleted.

The progress record is updated in the same SQL transaction as its relation changes. It describes
applied state, not a durable acknowledgment when asynchronous SQL commit is selected.

Advance persistent commit sequence once per logical commit, including each member of a SQL group;
update maximum transaction timestamp with `GREATEST`, not with the last member's timestamp. On a
fresh open, the new epoch's applied runtime version starts at zero over the recovered world;
claiming it preserves all persistent counters and sequence maxima. Counter overflow fails
explicitly.

These PostgreSQL counters do not imply new Fjall keys. Shared conformance requires recovery above
all surviving tuple timestamps and persisted sequence-slot maxima. Recovery above timestamps of
deleted tuples is a PostgreSQL-specific additional guarantee in this design (§2).

Initially select one immutable canonical source profile per database for all program and literal
decoding, including lambdas nested in otherwise ordinary values. Record its complete settings in
database metadata. Program rows can carry the profile identifier explicitly; property records use
the database profile and their literal-format version. Do not change the profile in place while
older source remains stored under it. If multiple profiles become necessary, extend every affected
source-bearing record with a profile reference, not just the verb table.

### 7.5 Representative physical table

The following DDL illustrates the property-record mapping. It is not a complete schema migration:

```sql
CREATE DOMAIN moor.object_ref AS text COLLATE "C";

CREATE DOMAIN moor.u64_counter AS numeric(20, 0)
    CHECK (VALUE BETWEEN 0 AND 18446744073709551615);

CREATE TABLE moor.object_propvalues (
    object_ref moor.object_ref NOT NULL,
    property_uuid uuid NOT NULL,
    record_sequence bigint NOT NULL CHECK (record_sequence > 0),
    logical_timestamp moor.u64_counter NOT NULL,
    record_kind text NOT NULL
        CHECK (record_kind IN ('full', 'list_append')),
    literal_format integer NOT NULL,
    value_kind text NOT NULL,
    value_literal text NOT NULL,
    PRIMARY KEY (object_ref, property_uuid, record_sequence),
    CHECK (record_kind <> 'list_append' OR value_kind = 'list')
);
```

The application validates canonical object references, the full value-literal grammar, supported
format, and chain invariants. The illustrative text domain specifies storage and collation;
canonicality and object-kind range validation use the shared object-literal codec on write and load.
SQL checks enforce cheap physical invariants without embedding a MOO runtime in the database. Keep
`value_kind` consistent with the literal on write and verify it on read. A SQL NULL is not a stored
MOO `None`; clearing a property removes its value chain while separately preserving any local
permission or metadata state.

### 7.6 Inspection views

Ship documented views with the schema:

- `objects`: object identity, name, parent, owner, location, and flags.
- `verb_definitions`: expand ordered definition arrays with their ordinals.
- `verbs`: join definitions to decompiled program source.
- `property_definitions`: expand definition arrays and expose names/UUIDs.
- `property_records`: physical full/suffix rows joined to property names where available.
- `property_values`: reconstruct current top-level list chains as readable MOO literal text.
- `persistence_status`: writer identity and applied progress.

`property_values` represents local stored state. It must not imply that an absent row means a
property has no inherited value. An effective-inheritance query is a distinct operation.

Reconstructing text in a view must not require compiling MOO. Canonical top-level lists use braces;
suffix rows also contain canonical lists. A checked helper can remove the outer braces and join
nonempty interiors in record order. Do not split on commas or parse nested values with regular
expressions. Check that the chain starts with one full row and that append rows follow a list.
Expose the last record's timestamp by sequence order, not `MAX(logical_timestamp)`.

This view is for inspection, not the engine read path. Its cost is proportional to the requested
value size. It must not maintain another complete stored value on every append. Filter by object and
property for large values; provide a parameterized helper if the view plan does not push those
filters into the chain scan.

Example queries, against these proposed views:

```sql
SELECT object_ref, name, parent_ref, owner_ref
FROM moor.objects
WHERE object_ref = '#42';

SELECT property_name, value_literal, logical_timestamp
FROM moor.property_values
WHERE object_ref = '#42';

SELECT names, source
FROM moor.verbs
WHERE object_ref = '#42';
```

Backup completeness does not depend on the views. All authoritative information lives in the
relation and bookkeeping tables.

## 8. A lossless MOO literal codec

### 8.1 Why literal text is the default

The primary requirement is that an operator can recognize and inspect values, and back them up with
ordinary PostgreSQL tools. It is not initially a requirement to execute SQL JSON-path queries inside
every nested MOO value.

MOO literals are a better match for that requirement than a tagged JSON tree:

```text
42
"Welcome"
{#42, #43, #44}
[1 -> "one", 'enabled -> true]
{x} => x + base with captured [{base: 42}]
```

Lists, typed map keys, symbols, object references, flyweights, and lambdas remain recognizable to
world authors. The same language-oriented representation can serve properties, metadata, and
captured lambda values.

JSONB remains appropriate for relational definition records and compiler-profile metadata. It is not
the canonical representation of arbitrary `Var` values. Optional SQL projections for frequently
queried scalar properties can be added later after measuring a real need; avoid mandatory dual
storage of each value as both text and JSON.

### 8.2 Existing code is a starting point, not a persistence specification

The starting implementations are [`compiler::unparse`](../crates/compiler/src/unparse/mod.rs) and
[`objdef_literal.rs`](../crates/compiler/src/objdef_literal.rs).

Several observed details require attention:

- Ordinary object display hides anonymous identity. Separately, `Obj::to_literal()` omits the `#` on
  UUID-style objects; `Obj::Display` does include it. The generic flyweight formatter calls
  `delegate().to_literal()`, so its delegate loses identity or literal syntax. The objdef-aware
  `write_literal_objsub` path handles these objects; use it with no naming substitutions as the
  starting point and share the fix with generic `to_literal`. Auditing only direct property
  serialization misses generic rendering used by lambda constant substitution.
- The ordinary rich-error formatter emits the error name and optional message, but does not emit
  every stored error payload.
- Lambda formatting skips empty capture frames and `None` values and resolves captures through
  names. Its parser also uses `Var::is_none()` to identify unassigned slots. Track capture presence
  separately from value with a bitmap/set or `Option<Var>` during parsing; reject duplicate slots
  even when the first assigned value was `None`. Preserve frame identity and shadowing.
- Lambda formatting always emits `self 1` when any self binding exists; parsing likewise hardcodes
  `Name(1, 0, 0)`. Rebind the actual recursive lexical identity in the newly compiled program. Do
  not copy an old numeric offset or assume that every recursive lambda uses the same slot.
- `ScatterLabel::Optional(name, _)` discards the default-expression information in formatting. This
  can change execution after reload, not merely source appearance. Defaults and recursive self
  binding join rich errors and exact floats as explicit release-blocking codec cases.
- Lambda decompilation paths contain unwraps. Persistence encoding requires fallible APIs.
- The current literal lambda parser compiles using default compile options. Persistence must use an
  explicit stored profile.
- Float output is Rust `Debug` (`{f:?}`), not a specified persistence grammar. Non-finite values and
  signed zero need the explicit rules below.
- `parse_literal_value` resolves bare identifiers through `ObjFileContext.constants()`, and lambda
  compilation substitutes matching identifiers using generic `to_literal`. Persistence parsing must
  use an empty constant context and disable include macros, with no object-name substitutions.
- Generic formatting emits `None`, but the current objdef literal parser has no corresponding
  keyword case. Recognize it as a built-in literal in persistence/source grammar rather than relying
  on a context constant, including inside captures and containers.

Add a persistence-oriented, fallible interface rather than treating `to_literal(value)` followed by
`parse_literal_value(text)` as already sufficient:

```rust,ignore
fn write_persistent_literal(
    value: &Var,
    profile: &SourceProfile,
    out: &mut impl std::fmt::Write,
) -> Result<(), LiteralEncodeError>;

fn read_persistent_literal(
    text: &str,
    profile: &SourceProfile,
) -> Result<Var, LiteralDecodeError>;
```

The compiler crate is the appropriate initial owner because it already owns the grammar, decompiler,
and closure reconstruction. Share fixes with objdef formatting where the semantics match. Do not
implement a second independent MOO parser in the PostgreSQL provider.

### 8.3 Required round-trip contract

The codec must cover every value supported by the current database encoder:

| Value     | Requirement                                                                            |
| --------- | -------------------------------------------------------------------------------------- |
| Integer   | Exact signed 64-bit value, including extrema.                                          |
| Boolean   | Distinct from integer zero/one.                                                        |
| Float     | Exact finite value and signed zero; defined, reversible non-finite representation.     |
| String    | Exact UTF-8 contents, including NUL, controls, quotes, backslashes, and newlines.      |
| Symbol    | Identity under current symbol rules, with spelling preserved where observable.         |
| Object    | Kind and complete identity, including anonymous references and sentinels.              |
| List      | Element order, empty lists, and recursively exact values.                              |
| Map       | Typed keys and values; no stringification of keys or case-insensitive comparison loss. |
| Binary    | Documented MOO binary-literal encoding; arbitrary bytes preserved.                     |
| Error     | Builtin/custom code, optional message, optional attached value.                        |
| Flyweight | Delegate, slots, and contents.                                                         |
| None      | Distinct from missing row, SQL NULL, and a string containing `None`.                   |
| Lambda    | Executable behavior, parameters/defaults, lexical captures, and recursion semantics.   |

Specify canonical grammar and escaping with a literal-format version. Finite floats use shortest
round-tripping decimal with a decimal point or exponent distinguishing them from integers; write
negative zero as `-0.0`. Non-finite values require an explicit scalar literal grammar extension
preserving infinity sign and NaN sign/payload bits, not an evaluated function call. Its spelling is
an outstanding codec-prototype decision, to be settled and recorded **before** freezing version 1.
Do not copy Rust's `NaN`/`inf` spellings without parser support or reserve identifiers in a way that
changes existing program variable bindings. The chosen form must work both in the literal parser and
as a constant inside decompiled program/lambda source. Exact float-bit and executable-source tests
are mandatory; silently coercing non-finite floats to null or string values is forbidden.

Canonical string escaping writes NUL as the four ASCII characters `\x00`, and uses `\n`, `\r`, `\t`,
`\"`, and `\\` for the corresponding characters. Other C0 controls and DEL use `\xNN` with uppercase
hexadecimal digits; ordinary Unicode remains UTF-8. `parse_string_value` calls `unquote_str`, which
already decodes `\x00` to NUL and has a test for it in
[`common::util`](../crates/common/src/util/mod.rs). Lexer acceptance alone is insufficient: for
example, the lexer recognizes `\U` escapes while this unquoter has no matching eight-digit decode
branch. Do not emit `\U`; add end-to-end persistence tests and reject unsupported escapes instead of
silently passing them through. Preserve literal backslashes distinctly from escape sequences.

Closure binding syntax still needs a compiler prototype (§9.2); its round-trip contract is fixed
even though lexical descriptor spelling is not. It must be settled before freezing the format.

MOO equality can ignore distinctions that storage must preserve, such as string case in some
comparisons. Tests need an exact structural comparator and float bit checks where applicable, not
just ordinary `Var` equality.

The parser constructs values and compiles lambda source. It must not evaluate arbitrary MOO code,
resolve runtime `$` properties, or execute user initialization functions while loading a value. Use
an empty `ObjFileContext`, explicitly disable `include!`/`include_bin!`, and reject unknown bare
identifiers instead of looking them up in external constants. Lambda compilation retains ordinary
local/captured identifiers without applying the objdef constant-substitution pass. Use stable
explicit object identities; objdef naming aliases are not database-global constants.

A value that cannot be rendered losslessly is an encoding error. There is no fallback to storing a
binary blob or a diagnostic display string. Expensive validation/rendering happens on encoder
threads. An error after publication follows the fatal persistence contract; codec coverage is
therefore a release gate, not a best-effort presentation feature.

## 9. Program and lambda source persistence

### 9.1 Verb programs

The authoritative `object_verbs.source` is canonical decompiled MOO text. The writer decompiles a
changed `ProgramType`; startup and snapshot reads compile its source back into `ProgramType`. The
representation is executable language source, not named JSON opcodes or base64 bytecode.

Record the language, source-format version, and compile profile needed to interpret the source. Keep
an originating compiler/build identifier for diagnosis, but use an explicit compatibility rule
instead of assuming that matching a build string alone proves correctness. The backend must reject
unsupported profiles before exposing a partially loaded world.

Recompilation may produce different instruction offsets, register assignments, and line mappings.
The persistence contract is equivalent program behavior under the supported compiler profile; exact
compiled-byte equality is not a requirement. Decompiled source does not recover comments or original
formatting that the runtime never retained. Diagnostic line numbers after reload refer to the stored
canonical source.

There is no durable compiled-code cache in PostgreSQL. A bounded process-local cache keyed by source
and complete profile can avoid repeated compilation during one startup/export. Its correctness must
not depend on an unchecked hash collision. Cold-start measurements must include compilation.

### 9.2 Lambda values and captures

Persist a lambda as an actual lambda literal, using the objdef-style `with captured` representation
as the starting point. That puts the body, parameter declarations, and captured values together in
readable MOO text, including when the lambda is nested in a list or map.

The source codec must reconstruct the closure after recompilation. Old compiled `Name` offsets are
not stable bindings into a newly compiled program. Introduce compiler-owned capture descriptors or
literal syntax extensions where needed to identify lexical bindings unambiguously and map them to
the freshly compiled environment.

The required cases include shadowed names, several capture frames, empty intervening frames,
captured `None`, nested lambdas, anonymous references, optional-parameter defaults, rest parameters,
and self-recursion. A formatter must not omit a frame or value merely because its current display
form appears empty. If captures can be canonicalized safely, that rule needs a semantic test and a
documented binding algorithm.

The current `{x} => x + base with captured [{base: 42}]` path demonstrates feasibility. It does not
establish that every runtime lambda already round-trips. Completing this codec is a prerequisite for
enabling general PostgreSQL persistence.

### 9.3 Write and load costs

Program writes pay decompilation cost on an encoder thread; reload pays compilation cost. General
value rendering pays lambda decompilation only for lambda values actually visited. Suffix-only list
encoding must not walk lambdas in the unchanged prefix.

During initial implementation, validate generated program/lambda source by compiling it in the
encoder before accepting the encoded batch. This catches syntactically invalid or unsupported output
before it becomes stored state. It is not a proof of semantic equivalence; behavioral round-trip
tests supply that evidence. Measure this extra CPU cost separately and permit bounded in-process
memoization for immutable programs. Do not move validation onto MOO task threads.

On load, use a bounded compiler work queue. A malformed literal or compilation error reports the
relation, object/property/verb identity, and source location, then fails startup/export. Do not
substitute an empty program, skip a row, or partially start the world.

### 9.4 Suspended tasks and upgrades

Suspended VM activations can contain instruction positions and their own program state. Never attach
an old instruction position to a newly recompiled world verb merely because its UUID matches. Audit
the separate task serialization/resume path and test this interaction before declaring the backend
supported. Where suspended tasks preserve their own compiled program, retain that recovery contract
independently of source-backed world verbs.

World-only restore should start with no unrelated suspended-task state. An explicit full-server
restore must use a compatible server build and matched ancillary backups. Supporting arbitrary
compiler upgrades for saved activations is outside this provider change.

## 10. PostgreSQL write pipeline

### 10.1 Normal commit

1. The engine prepares and validates its transaction as today.
2. It acquires a coordinator admission permit before publishing.
3. A successful root publication receives its publication version.
4. The engine submits typed changes and returns ordinary commit success without SQL I/O.
5. Encoders render changed values/programs and build backend parameters in parallel.
6. The writer receives encoded results and waits for consecutive publication versions.
7. It applies each commit's changes and progress metadata atomically in PostgreSQL.
8. On confirmed application, it advances progress, releases permits, and satisfies eligible waiters.

Do not compare timestamps in `ON CONFLICT` to decide which accepted value wins. The writer already
has the authoritative publication order, including the engine's special property policies.

Use fixed prepared SQL and batch rows by relation within a logical commit. Avoid a separate network
round trip for each tuple. For large imports, COPY into staging followed by set-based application is
an implementation option; COPY does not itself provide upsert semantics. All relation changes still
belong to the same logical atomic boundary. See
[PostgreSQL COPY](https://www.postgresql.org/docs/current/sql-copy.html).

### 10.2 Bounded grouping

One SQL transaction per mooR commit is the initial correctness baseline. To amortize commit flush
and protocol overhead, support bounded grouping of adjacent ready logical commits if the baseline
cannot meet the workload target.

A group must contain a consecutive publication range and apply it in order. The simplest first
grouping implementation executes each logical commit's mutations in sequence inside one SQL
transaction; it does not coalesce multiple writes to the same key. More aggressive coalescing must
preserve deletes/reinserts, append bases, sequence state, and metadata changes.

Use maximum age, logical-commit count, encoded bytes, and operation count. Never wait indefinitely
to fill a group. A barrier, snapshot request, shutdown, or latency bound seals the group. One large
logical commit may exceed grouping thresholds, but cannot be split into independently visible SQL
transactions.

On group success, progress advances through its last publication and every included permit is
released. Intermediate storage snapshots are unavailable, which is compatible with the proposed
at-least-version snapshot API. On failure the entire group rolls back. Report the failing relation
and operation; do not drop only the offending already-published commit and continue with later ones.

The writer needs a transaction-local overlay of property-chain and other writer bookkeeping while
building a group. Later members must see earlier members' tentative changes for rollup decisions,
deletes/reinserts, and append bases. Publish that overlay into confirmed writer state only after SQL
commit succeeds. Rollback discards the overlay; ambiguous-outcome recovery either installs the
confirmed result or rebuilds it before replay. Reading only the pre-group chain map for each member
would make grouping incorrect.

The Fjall adapter keeps its existing one-batch-per-logical-commit behavior. PostgreSQL grouping must
not force a change in Fjall's batching or durability defaults.

### 10.3 Property records

Retain the algorithm in [Bounded Property Append Log](incremental-property-value-persistence.md),
but use readable payloads.

An indicative physical row is:

```text
object_ref, property_uuid, record_sequence
logical_timestamp
record_kind = 'full' | 'list_append'
literal_format
value_kind
value_literal
```

The primary key starts with the complete logical property key and ends with record sequence. A full
row contains the complete value literal. An append row contains only a canonical list literal for
the suffix. For a full list, `value_kind` identifies it without SQL having to parse arbitrary MOO.

Ordinary append:

- Prove append eligibility before losing the base working-set information.
- Encode only the suffix on an encoder thread.
- Insert one append row, update commit bookkeeping, and retain the final value for possible rollup.

Replacement or rollup:

- Encode the final value on a separate rollup path when needed.
- Delete the preceding active chain and insert one full row in the same SQL transaction.
- Update the writer's chain metadata only after successful application.

Deletion removes the entire active chain. Startup, point reads, and export use one reconstruction
algorithm per backend with shared logical validation rules.

Initially use the same 64-record and 4 MiB accumulated-append-byte bounds as Fjall, with bytes
measured in this backend's encoded format. The full value is not subject to the append-byte limit.
Text size differs from FlatBuffers, so rollup frequency can differ. A separate rollup encoder or
equivalent independent response path must preserve the existing avoidance of writer/encoder queue
deadlock.

Choose a separate PostgreSQL `property_record_sequence` counter, matching Fjall's allocation unit:
advance once per logical commit with property-value mutations, including deletion-only commits.
Every inserted full/suffix row in that commit uses that number; the working-set map guarantees one
mutation per property. Rollup substitutes a full row under the same allocated number, not another
increment. A grouped SQL transaction assigns one number to **each** qualifying member in order,
using its tentative writer overlay; persist the final counter with the group. Replay after a
confirmed rollback reuses the planned numbers, and ambiguous outcomes resolve progress first.

Use a checked positive `bigint` for property record sequences, not `numeric(20,0)` in this hot
composite primary key. This is a new backend-private counter bounded by `i64::MAX`, independent of
mooR's full-u64 timestamps and persistent commit counter, which keep the numeric domain. Recover the
counter from bookkeeping even when every chain was deleted. Exhaustion is a controlled fatal error
before applying a batch, never wraparound. Fjall retains its existing record format/allocation.

Do not maintain an eagerly rewritten complete literal alongside the chain. That would restore the
full-value serialization and storage amplification the append representation avoids.

## 11. Application, durability, and recovery

### 11.1 Three completion boundaries

| Boundary  | What it guarantees                                                           |
| --------- | ---------------------------------------------------------------------------- |
| Published | The in-memory root contains the accepted commit.                             |
| Applied   | The backing database exposes an atomic prefix through that publication.      |
| Durable   | That prefix has passed the explicitly requested local durable-storage fence. |

Ordinary task success remains the published boundary. A process crash can lose published work still
in the in-memory persistence queue even if PostgreSQL uses synchronous SQL commits. This design does
not introduce a second local durable journal to change that behavior.

Default PostgreSQL persistence to durable SQL commits, with normal server `synchronous_commit=on`
behavior. Grouping is the preferred first tool for amortizing that cost. An explicitly configured
asynchronous-commit mode can be evaluated separately; it trades recent applied work for lower flush
latency on server failure. Never disable PostgreSQL `fsync` as an optimization. See
[asynchronous commit](https://www.postgresql.org/docs/current/wal-async-commit.html).

`wait_durable` is issued through the ordered writer after the requested prefix has applied. For
Fjall this is **new behavior**: the world `BatchWriter` currently does not call `persist`, and
`wait_for_persistence` is explicitly non-durable. Implement the new request with the database's
`persist(PersistMode::SyncAll)` operation at an ordered boundary and advance the durable watermark
only on success. Normal Fjall commits retain their existing application/flush behavior; they gain no
per-commit sync. Connection-store sync calls do not establish world-store durability. PostgreSQL can
perform a real bookkeeping update in a transaction with synchronous local flush enabled; a no-op
read-only commit is not a durability fence. Test that the fence covers preceding asynchronous
commits. Local durability does not claim survival of every failover policy or asynchronous replica
loss.

Do not benchmark synchronous PostgreSQL commits against unflushed Fjall application and describe the
result as equal-durability throughput. Report both acknowledgment and storage-flush policies.

### 11.2 Ownership and ambiguous outcomes

The writer connection holds a session advisory lock for the world/schema. On initial open it also
claims a fresh writer epoch in persistent metadata. The lock prevents accidental cooperating
writers; the epoch identifies which engine lifetime owns pending work. See
[PostgreSQL advisory locks](https://www.postgresql.org/docs/current/explicit-locking.html).

Commit the initial epoch claim durably before accepting publications, even when data transactions
will use asynchronous SQL commit. Otherwise a server restart could lose the ownership identity
needed to classify retained work during recovery. Advisory-lock scope must identify the selected
database/schema consistently for every cooperating opener; a fresh random epoch is not a lock key.

Every applying SQL transaction verifies the expected epoch and previous progress before advancing
the progress row. That row and all relation changes commit together. The persistent commit sequence
continues across engine restarts while runtime publication versions start within a new epoch.

If the connection fails during COMMIT, its outcome is ambiguous. Recovery must:

1. Stop applying later batches and retain the affected batch/group and admission permits.
2. Reconnect within the configured recovery deadline.
3. Reacquire the writer lock; inability to do so is ownership loss.
4. Verify that persistent writer epoch still matches this engine lifetime.
5. Read the applied publication watermark and determine whether the group committed.
6. If committed, update in-memory writer metadata from the confirmed result or reload it; if not,
   replay the unchanged consecutive group. An impossible partial group or unexpected watermark is a
   consistency error.

Do not blindly replay appends, and do not generate a new epoch merely to bypass a mismatch. Another
engine may have taken ownership and loaded a different world. Losing a database session also loses
the advisory lock. Multi-primary database split-brain and failover that loses acknowledged database
state require external fencing/recovery policy; this interface does not solve them.

Use bounded retries for known transient failures before terminal failure. During an outage,
admission eventually applies backpressure. Persistent encoding/schema/constraint failures are not
infinite-retry conditions. Exceeding the recovery deadline closes admission, fails waiters, and
signals the existing fatal database path. Published in-memory changes cannot be safely rolled back
one batch at a time.

### 11.3 Startup and shutdown

Startup claims ownership, validates versions, opens one consistent storage snapshot, loads sequence
and progress data, and streams every relation. Source compilation can use a bounded worker pool
without publishing a partial world. Reconstruct property chains and writer chain metadata from the
same snapshot. Finish the seed and close the SQL read transaction before accepting world tasks.

On graceful shutdown, stop new admission, drain all already published work in order, perform any
explicitly configured final durable fence, resolve pending requests, and release writer resources.
Default Fjall shutdown keeps its existing non-explicit-sync policy; adding `wait_durable` does not
silently change that default. PostgreSQL retains its selected SQL commit policy. Independent
snapshot loaders release their resources on drop (§5.5). A drain timeout must be reported as
incomplete persistence, not successful shutdown. Keep finite network deadlines. A synchronous Fjall
filesystem flush may outlast a caller's deadline: timeout stops waiting, not the OS operation, and
cannot be reported as a successful flush or justify an unbounded thread join on the shutdown caller.

## 12. Snapshots and objdef

### 12.1 Existing public interface

[`SnapshotInterface` and `SnapshotExportSession`](../crates/common/src/model/loader.rs) already form
the appropriate storage-independent boundary. The [`objdef writer`](../crates/objdef/src/dump.rs)
consumes naming metadata and an object stream.

Implement `PostgresSnapshotLoader`; keep the objdef format and public writer behavior. Extract the
storage-independent ancestry/property assembly from
[`fjall_snapshot_loader.rs`](../crates/db/src/provider/fjall_snapshot_loader.rs) so the two adapters
share its sparse inherited-property semantics.

### 12.2 Acquisition

1. Capture `PublicationId { epoch, version: V }` from the current published root.
2. Wait for the PostgreSQL writer to apply through that token under the acquisition deadline.
3. Acquire an export slot under the same deadline (§6.3), then open a dedicated connection in
   `REPEATABLE READ READ ONLY`.
4. Execute a progress query to establish the snapshot and verify both the epoch and that its applied
   version includes the requested prefix. An epoch mismatch is an error.
5. Keep that transaction alive for all point reads and both export passes.

The factory returns an owned `Box<dyn SnapshotInterface>`. `FjallSnapshotLoader` owns its existing
snapshot/keyspace handles. `PostgresSnapshotLoader` owns a handle to a dedicated read-session
worker, which exclusively owns the libpq connection/transaction and export-capacity lease. This
avoids a self-referential borrowed SQL transaction or a requirement for concurrent `PGconn` access.
Synchronous loader methods send bounded requests to that worker; cursors/fetch buffers live in the
session. A borrowed `SnapshotExportSession` cannot outlive its loader. No global writer/coordinator
lock is held while a loader query waits.

Dropping the loader, including error/unwind paths, closes its session and releases the slot; an open
read transaction ends when its connection closes. Cleanup must not depend on an unbounded rollback
round trip. Cancelling acquisition before a loader is returned likewise releases every acquired
resource. The owned reader has no Rust borrow from `MoorDB` and may outlive writer shutdown. Startup
seeding instead borrows the opener-owned session until `SeededWorld` is complete (§5.2).

BEGIN alone does not establish the repeatable-read snapshot; the first relevant statement does. Once
established, the snapshot is stable across queries. Use the same primary database, not a potentially
lagging read replica, for this initial protocol. See
[transaction isolation](https://www.postgresql.org/docs/current/transaction-iso.html).

The returned snapshot can include later publications, matching the current at-least-version Fjall
behavior. Exact historical publication selection is outside the initial API. It cannot be emulated
by filtering rows on mooR timestamps, since rows are replaced and timestamps are not commit order.

One connection does not need `pg_export_snapshot`. Parallel readers must import the same exported
snapshot before their first query, while the exporting transaction remains available for import. An
exported snapshot ID is a temporary coordination token, not a durable backup identifier. See
[snapshot synchronization](https://www.postgresql.org/docs/current/functions-admin.html#FUNCTIONS-SNAPSHOT-SYNCHRONIZATION).

### 12.3 Streaming and compilation

Use ordered server-side cursors with bounded fetches for large relation scans. All streams agree on
the SQL object-key order and their per-object UUID/record order. Avoid offset pagination, per-object
N+1 queries, and joins that multiply verb/property/metadata rows.

Retain naming metadata, parent/ancestry indexes, and property-definition indexes. Release program
and property payloads after each exported object. Memory is proportional to those indexes, fetch
buffers, bounded compilation work, and the largest object being assembled; it is not constant in
world size.

The current snapshot interface returns compiled programs and `Var` values. PostgreSQL will therefore
compile source while building exported objects, then the objdef writer will render those programs
again. This extra cost is acceptable for the first compatible implementation and must be measured. A
later source-aware export payload could remove that compile/decompile cycle, but must preserve the
same validation and property semantics. It is not required to make the existing path work.

Preserve local definition rows, value-only overrides, permission-only overrides, metadata-only
overrides, and absent inherited state. Do not resolve all inherited values into explicit rows during
export. Filter stale property rows against the captured ancestry, as the Fjall export does.

The SQL snapshot ends on drop, including error paths. Disconnecting invalidates it; do not reconnect
mid-export and mix two snapshots. Restart the export using the existing incomplete-output handling.
Long-running snapshots retain old row versions; expose age and duration, bound concurrent exports,
and monitor table/TOAST growth. See
[vacuuming](https://www.postgresql.org/docs/current/routine-vacuuming.html).

## 13. Backup, restore, and operation

### 13.1 Ordinary PostgreSQL backups

All authoritative world state must be included in ordinary PostgreSQL logical dumps. No data needed
for recovery may live exclusively in a client-side cache, external program binary, extension-private
file, or non-dumpable temporary table.

Support documented whole-database `pg_dump`/`pg_restore` procedures and an audited schema-scoped
procedure. Include tables, domains, helper functions, views, and format metadata. Roles/grants and
other cluster-level configuration need their corresponding provisioning or backup procedure.
PostgreSQL documents consistent logical dumping and its restore formats in
[pg_dump](https://www.postgresql.org/docs/current/app-pgdump.html).

Illustrative commands for the future operator guide:

```sh
pg_dump --format=custom --file=world.dump --dbname=service=moor
pg_restore --exit-on-error --dbname=service=moor_restore world.dump
```

These are standard-tool examples, not commands executed as part of this design. Restore into a
separate empty database or a deliberately stopped/recreated target. Readable plain-SQL dumps are
also available; the base tables remain inspectable regardless of archive choice.

A dump without coordination captures a consistent applied prefix, possibly behind mooR memory. For a
checkpoint guaranteed to include a captured publication, wait for that publication's applied and
requested durable boundaries before starting the dump against the same database. Later commits may
also be included. An exact coordinated dump can use a held exported PostgreSQL snapshot and
`pg_dump --snapshot`; that is an optional operational integration, not a requirement for routine
backups.

### 13.2 Physical backup and point-in-time recovery

The schema should also work with ordinary PostgreSQL physical backup and WAL/PITR practices. These
recover database state, not unsubmitted or unapplied in-memory work. Document the selected recovery
point and any mismatch with ancillary daemon stores. See
[PostgreSQL backup and restore](https://www.postgresql.org/docs/current/backup.html).

Do not present SQL readability as permission to edit restored data without validation. A restored
world must pass the same schema, literal, source, and semantic checks as any startup.

### 13.3 Restore acceptance

The operator-facing restore procedure should:

1. Restore into an isolated target and provision the application/inspection roles.
2. Run a backend validation command that reads every relation, reconstructs chains, parses all
   values, and compiles every verb/lambda under the stored profiles.
3. Report counts, format compatibility, unresolved references where relevant, and precise failures.
4. Start one engine with a new writer epoch only after validation succeeds.
5. Start without unrelated suspended tasks/connections for a world-only restore.
6. Export objdef and run a small functional probe as part of periodic restore drills.

`pg_restore` succeeding is necessary but insufficient: it verifies SQL restoration, not whether the
installed mooR compiler can rebuild every saved program.

### 13.4 Permissions and supported access

Use an owner/migration role for schema changes, an application role for runtime persistence, and a
read-only inspection/backup role as appropriate. Avoid requiring superuser privileges. Defaults must
not grant public access to properties or verb source; those tables may contain world secrets.

Live external UPDATE/INSERT/DELETE operations are unsupported because runtime indexes are
authoritative between startups. Offline edits may be possible through an explicit validation and
restart workflow, but general SQL authoring is not an initial feature. No notification/listen scheme
is proposed as an implicit cache-coherence solution.

## 14. Performance model and acceptance criteria

### 14.1 Expected cost changes

This table describes the initial fully resident mode. Future paged mode retains the same resident
read/write fast paths but adds explicit fault I/O and, for source-backed programs/lambdas,
compilation costs as described in section 5.8.

| Area                    | Fjall                                                    | Proposed PostgreSQL                                                     |
| ----------------------- | -------------------------------------------------------- | ----------------------------------------------------------------------- |
| Transaction read        | In-memory index                                          | Same in-memory index                                                    |
| Transaction publication | Index preparation, conflict checks, CAS, queue admission | Same shape; no SQL/decompilation on task thread                         |
| Background encoding     | Binary encoding; suffix-only append                      | Literal rendering/decompilation; suffix-only append                     |
| Storage application     | Embedded batch                                           | SQL protocol, execution, WAL, heap/index work                           |
| Large append            | Bounded suffix chain                                     | Bounded readable suffix chain                                           |
| Startup                 | Decode values and programs                               | Parse literals and compile program/lambda source                        |
| Export                  | Snapshot scans and decompilation                         | SQL snapshot scans, parsing/compilation, then existing objdef rendering |
| Storage maintenance     | LSM flush/compaction                                     | WAL/checkpoints/vacuum/TOAST                                            |
| Operational memory      | World indexes plus embedded store                        | World indexes plus PostgreSQL server/cache resources                    |

Text storage necessarily gives up some binary codec efficiencies. Source recompilation shifts
additional cost to startup and export. The goal is approximately preserved foreground behavior and
acceptable sustained persistence throughput, not identical CPU usage, storage size, or startup time.

### 14.2 Avoidable regressions

- No SQL on property lookup, cache miss in a fully loaded index, or read-only commit in fully
  resident mode. In future paged mode, real version-correct faults can perform I/O; a resident hit
  must retain its fast path.
- No connection creation per MOO transaction.
- No literal rendering or source compilation on the foreground publication path.
- No full-list rendering on an ordinary accepted append.
- No fsync wait added to ordinary task completion.
- No global transaction lock around SQL activity or snapshot export.
- No unbounded queue or unbounded full-table `PGresult`.
- No mandatory index on every changing value/timestamp.
- No blanket deep JSON representation or per-value heap allocation added to Fjall's encoder.

PostgreSQL full-value updates create new row versions and may rewrite changed TOAST values. Keep
small hot relation rows separate from larger payload rows as the relation mapping already does.
Primary-key-only updates can benefit from HOT when its conditions hold; secondary indexes on
modified columns can prevent that optimization. Tune fillfactor only from measurements. See
[HOT](https://www.postgresql.org/docs/current/storage-hot.html) and
[TOAST](https://www.postgresql.org/docs/current/storage-toast.html).

### 14.3 Measurements

Measure publication p50/p95/p99, sustained applied commits/second, applied lag in commits and time,
admission waits/timeouts, encoder CPU/allocation, source compile/decompile time, and rollup latency.
Include encoded/wire bytes, WAL volume, table/index/TOAST size, vacuum/checkpoint activity, startup
time, export time, and peak memory in both processes.

Run tests long enough to reach maintenance activity and a stable queue. Report the time required to
drain after publication stops. A short burst that leaves thousands of unapplied writes is not a
sustained-throughput result.

Test local Unix socket, local TCP, and a specified remote RTT separately. Compare explicit
durability policies. On a serial connection, sequential protocol round trips impose an approximate
lower bound of `round_trips * RTT` before SQL/flush work. Batching and grouping address that cost;
more encoder threads alone do not.

### 14.4 Proposed review gates

These are provisional engineering gates to confirm with workload measurements:

1. Fjall refactor: no reproducible regression above 5% in representative warmed foreground
   transaction throughput/p95 latency, with no change in durability settings; investigate smaller
   changes outside benchmark noise as well.
2. PostgreSQL under a declared sustainable load: warmed foreground publication p95 within roughly
   10% of the refactored Fjall baseline and no SQL waits on the foreground path. This excludes
   intentionally overloaded cases, which must instead demonstrate bounded backpressure.
3. Persistence capacity: measure a stable maximum, then demonstrate at least 20% headroom over the
   agreed deployment workload while exporting and performing normal database maintenance.
4. Append workload: ordinary encoded work scales with suffix size, with bounded, measured rollup
   spikes. No per-append complete-value rendering hidden in metrics or inspection projections.
5. Startup/export: publish absolute time and memory results on an agreed real-world corpus before
   selecting operational limits; no invented equivalence target to binary program loading.
6. Restore: a standard dump restored into a clean database reproduces logical state and executable
   behavior with a compatible mooR build.

If PostgreSQL cannot meet foreground behavior because its sustained writer capacity is too low for
the target workload, narrow the supported workload or improve batching/encoding. Do not conceal the
shortfall by weakening admission or silently changing durability.

These initial gates measure fully resident mode. A later eviction feature needs separate cold-fault
and memory-reduction gates; it must not reinterpret a fully resident benchmark as evidence about
lazy reads. Warming a payload can also be required to prepare a write in paged mode, but background
storage application should remain independent of ordinary publication acknowledgment.

### 14.5 Existing core write-amplification workloads

Use [`cores/benches`](../cores/benches/README.md) as a primary end-to-end comparison workload,
alongside the Rust microbenchmarks. Its MOO tasks exercise the scheduler, commits, property mutation
classification, encoding, and background writer together. The existing
[`Makefile`](../cores/benches/Makefile) runs these workloads through release `moorc` and objdef
tests. Extend that construction path to select either backend while keeping the MOO workload
identical.

Keep three separately identified results: current Fjall, refactored Fjall, and PostgreSQL. Isolate
the permit-lifetime change in the Fjall comparison, especially queue-pressure presets: report
admission waits/timeouts and outstanding commits during encoding/application, not just producer
throughput. Use the same declared overload policy and record the changed capacity envelope.

The relevant cases are:

| Workload                                          | Comparison purpose                                                                                                                                       |
| ------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `bench-string-history`, mutation mode 0           | Suffix-only persistence as large list properties grow; vary writer count, append width, and delay.                                                       |
| The single-writer 9.7 MB history preset           | Ordinary append costs plus a foreground full-value rollup within one run.                                                                                |
| `HISTORY_MUTATION_MODE=1`                         | Replace one element without growing the list; measure complete-value serialization/application costs.                                                    |
| `HISTORY_MUTATION_MODE=2`                         | Rebuild the prefix before appending; measure append classification and its bounded-proof fallback rather than assuming every logical append is eligible. |
| The eight-writer queue-pressure preset            | Admission/backpressure, persistence lag, and eventual drain under a burst of 1,200 property updates.                                                     |
| `bench-write-stress`, both `APPEND_MODE` settings | Repeated overwrite and growing/truncated-value pressure across many tasks.                                                                               |
| `bench-combat-stress`                             | Mixed state writes, peer access, and scheduler load as a check beyond isolated history properties.                                                       |

The existing history presets, run from the repository root, are:

```sh
make -C cores/benches bench-string-history HISTORY_WRITERS=1 HISTORY_ENTRIES=1024 \
  HISTORY_ENTRY_BYTES=9472 HISTORY_APPENDS=70 HISTORY_SETTLE_SECONDS=2

make -C cores/benches bench-string-history HISTORY_WRITERS=8 HISTORY_ENTRIES=1024 \
  HISTORY_ENTRY_BYTES=2048 HISTORY_APPENDS=150 HISTORY_SETTLE_SECONDS=0
```

These commands describe existing workloads, not completed PostgreSQL comparisons. The initial
Makefile has no PostgreSQL selector. Add backend/feature configuration to the runner when the
provider exists; do not maintain separate MOO benchmark implementations for each backend.

The controller currently calls `commit()` while seeding, sleeps for `HISTORY_SETTLE_SECONDS` before
and after the measured producers, and logs `HISTORY_APPEND_RESULT` with producer elapsed time. Those
sleeps do not prove that persistence has drained. For backend qualification, add a harness boundary
that waits for the captured applied prefix before the initial counters and after the measured
producers. Measure a durable fence separately under the chosen durability policy. Capture end
counters before the benchmark recycles its writer objects, so deletion/cleanup does not obscure the
measured history work. Preserve the zero-settle burst preset as a distinct pressure test.

Report three times: producer completion, completion of application through the captured final
publication, and completion of the requested durable fence. Include append acceptance/rejection
reasons, complete/suffix encoded bytes, rollup counts and time, queue waits, and persistence lag.
The benchmark already logs useful database counters; retain common metric meanings for both backends
and add backend-specific physical-write measurements.

Separate these amplification measures:

- Encoded property bytes per byte logically appended or replaced, with full and suffix bytes shown
  separately. Use actual generated data sizes; numbered history entries add identifying text.
- Fjall journal/table/compaction writes or PostgreSQL WAL/heap/index/TOAST writes over a defined
  observation interval, using comparable instrumentation.
- Total device writes during the measured run and a stated maintenance/drain interval, on isolated
  storage where attribution is possible. Report net database-size growth separately; it does not
  measure total bytes rewritten.

The controller also updates bookkeeping properties, so distinguish property-payload counters from
whole-workload bytes and commits. Compare clean equivalent seeds, identical language options,
release builds, task counts, workload parameters, and explicit durability policies. Repeat runs long
enough to include several rollups and steady maintenance activity; report variability. Pair the
history cases with snapshot/export and restart verification of final contents, rather than using
producer timing alone as the success criterion.

## 15. Validation plan

### 15.1 Common backend conformance

Run the same tests for both backends where the logical contract is shared:

- Multi-relation insert/update/delete atomicity and sequence recovery.
- Out-of-order encoder completion and consecutive publication application.
- Snapshot acquisition after a barrier and during later writes.
- Sparse inherited property values, permission-only and metadata-only state.
- Rename, reparent, recycle, delete/reinsert, and stale metadata/property filtering.
- Sequence-slot maxima under reordered persist submissions, including observations from aborted
  transactions; recovery above surviving tuple timestamps. Deleted-tuple timestamp high-water
  recovery is PostgreSQL-specific, not a shared Fjall assertion.
- Admission lifetime through encoding/application, group credit release only after commit,
  timeout/recovery, stale-epoch wait rejection, and bounded shutdown. Include an unpublished CAS
  attempt giving back its permit and a timed-out waiter whose commit still applies.
- Snapshot-capacity exhaustion, FIFO/deadline behavior, point queries before export, loader drop,
  writer shutdown with a live reader, and one shared startup snapshot across relations.
- Append replacement, chain rollover, deletion, clobber policy, and restart reconstruction.

Reuse the relevant existing batch writer, transaction, and objdef tests. Parameterize meaningful
behavior rather than duplicating the same implementation-specific assertions.

### 15.2 Literal and source conformance

Build an exact round-trip corpus and property-based tests for every supported `Var` type. Include
deep nesting, integer limits, string case, embedded controls/NUL, non-finite floats, binary data,
typed map keys, anonymous references, rich errors, and flyweights.

Include anonymous/UUID flyweight delegates nested in lambda captures, duplicate capture assignments
where the first value is `None`, and recursive bindings whose old offset is not one. Check canonical
finite/non-finite float text and exact bits, NUL versus literal `\\x00`, and rejection of include
macros and nonempty constant contexts. Schema tests cover metadata on different holders sharing the
same definition UUID, compound prefix scans, original symbol spelling with folded identity, all
definition fields, and preservation of unnamed flag bits.

For source, test representative programs and lambdas by compile, decompile, persist, reload,
recompile, and execute. Include optional defaults, named/rest parameters, recursion, nested scopes,
shadowed captures, sparse frames, nested lambda values, exceptions, forks, and feature-dependent
language syntax. Compare relevant values/behavior and canonical source where appropriate, not
bytecode addresses. Test diagnostics and hard failure for unsupported source profiles.

Existing objdef round-trip tests are useful evidence but are not a substitute for the exact-value
corpus, given the observed formatting gaps.

### 15.3 PostgreSQL failure tests

Inject connection loss before application, during statements, during COMMIT, and after COMMIT before
acknowledgment. Verify progress-based recovery never duplicates an append. Test complete-group
rollback, ownership takeover, stale epoch rejection, backend restart, retry deadlines, and a failure
while an export holds a snapshot.

Delete every property chain and the newest timestamped tuples, then reopen: the PostgreSQL progress
and property-record counters must still advance. Test multiple property-changing members and rollups
within one grouped transaction, including rollback and ambiguous-outcome replay.

Test synchronous and asynchronous SQL commit policies, the explicit durable fence, and recovery
after process/server failure. Verify that diagnostics distinguish unapplied queued work from
database-applied work and never claim a durability guarantee not established by the selected mode.

### 15.4 Import/export and backup tests

Exercise Fjall → objdef → PostgreSQL → objdef → Fjall. Compare logical content and program behavior;
objdef can regenerate definition UUIDs and omit engine bookkeeping, so do not compare raw backend
rows as an identity-preserving physical copy.

Separately test pg_dump → clean PostgreSQL restore. That path must preserve physical UUIDs,
timestamps, sequence state, literal/source formats, and complete property chains. Test standard
readable views on the restored database before starting mooR.

Use the existing [`objdef_export_benches.rs`](../crates/db/benches/objdef_export_benches.rs), the
[`cores/benches`](../cores/benches/README.md) write-amplification workloads described in section
14.5, property-update load tools, and transaction benchmarks. Add a source-heavy and lambda-heavy
corpus because binary decoding benchmarks do not capture compilation cost.

### 15.5 Build matrix

Validate default Fjall builds, `moor-db` with PostgreSQL enabled, the standalone daemon with
PostgreSQL enabled, the combined server, and the relevant tools. Check formatting, targeted tests,
workspace checks, and required clippy coverage before implementation review. Verify the default
build needs no libpq and feature-enabled daemon dependency resolution introduces no Tokio.

### 15.6 Future eviction conformance

Before enabling paged operation, extend the common backend tests with a deterministic fault/eviction
schedule. At minimum cover:

- An evicted key is still present; a known-absent key and an unknown key remain distinct.
- A published value newer than storage is read from its pinned overlay after eviction attempts.
- A published delete suppresses a stale provider row until storage catches up.
- An old transaction can fault its old value after a later update/delete/reinsert has applied.
- A load completing late cannot replace another root's newer resident revision.
- Concurrent faults share only identical revision requests and preserve independent error paths.
- Eviction/refill does not change transaction timestamps, logical publication versions, or conflict
  results. Partial coverage causes safe checking rather than false absence or endless retries.
- Scans and reverse lookups return identical results across different residency patterns.
- Append preparation/rollup and anonymous-object reachability remain correct with nonresident data.
- Dropping the last root/read-view pin releases historical versions; long-lived readers and backend
  outages trigger explicit memory/backpressure policies instead of unbounded retention.
- PostgreSQL refault recompilation reconstructs correct lambda captures and program behavior.
- An expired or unavailable read view produces a defined transaction failure/retry path, never a
  latest-state substitution. Any retry must respect the scheduler's existing side-effect rules.

The provider refactor should preserve test seams for these cases now. Implementing an actual paging
policy and running this expanded suite are gates for the later eviction feature, not a prerequisite
for shipping the fully resident PostgreSQL backend.

## 16. Implementation sequence

1. **Conformance and baselines.** Capture current Fjall behavior and representative performance;
   include the existing core history/write-stress presets with explicit persistence boundaries.
   Establish the literal/source edge-case corpus and relocate/adapt the test-only direct-apply
   helper and its benchmark caller. Production already has a single batch path.
2. **Common persistence boundary.** Introduce logical batches, explicit applied/durable receipts,
   and backend-neutral coordinator/configuration boundaries. Adapt Fjall without changing its
   physical encoding, append strategy, ordinary acknowledgment point, or default flush policy.
   Isolate and measure the permit-lifetime change; add explicit durable flushes and sequence-max
   hardening with their own behavioral tests. Do not add Fjall bookkeeping keys.
3. **Snapshot/read separation.** Add bounded typed scans, extract shared export assembly, and make
   the initial fully resident mode explicit. Apply the concrete generated-type/registry changes in
   §5.6, disable unversioned runtime fallbacks with an invariant error, and pass the opener's
   snapshot into seeding. Reserve the documented fault insertion point without adding unused
   historical-read methods; enabling faults remains separate work.
4. **Readable codecs.** Implement the fallible versioned MOO literal/source codec, fix lambda/error
   edge cases, and establish compiler-profile compatibility. This is a correctness gate.
5. **Feature and libpq adapter.** Add optional dependencies, packaging, safe connection/result
   ownership, prepared parameters, deadlines, and dependency-graph checks.
6. **PostgreSQL baseline.** Create the relation schema, ownership/progress tables, ordered writer,
   suffix records, startup seeding, source compilation, and error recovery.
7. **Snapshot and operator path.** Implement PostgreSQL snapshot export, readable views, validation
   command, and backup/restore runbook; test full round trips and restore drills.
8. **Performance qualification.** Measure stable throughput/lag, then add bounded SQL grouping or
   pipelining only where evidence justifies it. Confirm Fjall gates again after shared changes.

Each step should be independently reviewable. No step requires migrating every daemon store or
making the PostgreSQL backend the default.

A follow-on eviction design selects history retention and payload ownership, then prototypes
property/program eviction against a resident key directory. Partial key/index paging and lazy
startup follow only after the fault-read and scan consistency contracts are established.

## 17. Decisions still requiring prototype evidence

- Choose and validate non-finite scalar literal spellings (§8.3), and settle exact closure
  binding-identity syntax before freezing the source/literal format.
- The exact compiler profile/version compatibility policy and whether the initial single-profile
  restriction is sufficient for the real-world corpus.
- SQL grouping thresholds and whether set-based statements alone meet the target workload.
- Startup compiler parallelism and bounded source-cache size.
- Whether the proposed chain reconstruction view gets adequate predicate pushdown or needs an
  explicit property lookup function.
- Supported maximum value/program sizes and nesting limits without silently narrowing valid existing
  data. PostgreSQL field-size limits and text expansion must be checked on the corpus.
- libpq packaging choices on the supported platforms and the final minimum supported server/client
  versions.
- The representative deployment workload and absolute startup/export/restore time budgets.
- The future paging strategy: resident historical values, leased backend snapshots, explicit
  versioned records, or a measured combination; and its memory/history reclamation policy.
- The boundary between resident key/structural indexes and evictable payloads, followed later by
  partial keyspace coverage and complete paged scans.

The architectural commitments are already explicit: no Tokio introduction, readable native fields
and MOO literals/source, a fully resident initial fast path with a future version-correct paging
boundary, ordered background persistence, bounded property append work, and support for the existing
snapshot-to-objdef interface.
