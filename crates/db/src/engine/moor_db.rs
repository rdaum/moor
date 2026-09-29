// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

//! Primary database engine type and lifecycle.
//!
//! `MoorDB` owns relation snapshots, transaction seeding, serialized write
//! commit application, and background durability workers.

use crate::{
    AnonymousObjectMetadata, DatabaseOpenError, EntityMetadataKey, ObjAndUUIDHolder,
    RelationCompactionResult, StorageMaintenanceStats, StringHolder,
    cache::{
        ancestry_cache::AncestryCache, prop_cache::PropResolutionCache,
        verb_cache::VerbResolutionCache,
    },
    config::{DatabaseConfig, PersistenceConfig, StorageConfig},
    tx::{CheckRelation, Relation, RelationTransaction, Timestamp, Tx, WorkingSet},
};
use crate::{
    engine::relation_defs::define_relations,
    provider::{
        batch_writer::BatchWriter,
        coordinator::{PersistenceCoordinator, PersistenceStatus},
        fjall_format,
        fjall_provider::FjallProvider,
        fjall_snapshot_loader::FjallSnapshotLoader,
        logical::{PersistenceError, PersistenceReceipt, PublicationId, WriterEpoch},
        property_value_store::PROPERTY_VALUE_CHAIN_LIMITS,
    },
};
use fjall::{Database, KeyspaceCreateOptions};
use moor_common::model::loader::SnapshotInterface;
use moor_common::util::CachePadded;
use moor_common::util::Instant;
use moor_common::{
    model::{
        CommitResult, HasUuid, ObjFlag, PropDefs, PropPerms, ValSet, VerbDefs, WorldStateError,
    },
    util::BitEnum,
};
use moor_var::{NOTHING, Obj, Symbol, Var, program::ProgramType};
use std::{
    sync::{
        Arc,
        atomic::{AtomicI64, AtomicU16, AtomicU64, Ordering},
    },
    time::Duration,
};
use tempfile::TempDir;
use tracing::{error, info};
use uuid::Uuid;

mod property_policy;
mod snapshot_planes;

use snapshot_planes::SnapshotPlanes;
pub(crate) use snapshot_planes::TxSeed;

define_relations! {
    object_location == Obj, Obj,
    object_parent == Obj, Obj,
    object_flags => Obj, BitEnum<ObjFlag>,
    object_owner == Obj, Obj,
    object_name => Obj, StringHolder,
    object_verbdefs => Obj, VerbDefs,
    object_verbs => ObjAndUUIDHolder, ProgramType,
    object_propdefs => Obj, PropDefs,
    object_propvalues => ObjAndUUIDHolder, Var,
    object_propflags => ObjAndUUIDHolder, PropPerms,
    entity_metadata => EntityMetadataKey, Var,
    object_last_move => Obj, Var,
    anonymous_object_metadata => Obj, AnonymousObjectMetadata,
}

// The relation macro defines the private commit-method macro used by this module.
mod commit_pipeline;

impl WorldStateSnapshot {
    /// Resolve a property name from the immutable indexes already owned by this snapshot.
    pub(crate) fn property_name(&self, object: Obj, uuid: Uuid) -> Option<Symbol> {
        let mut current = object;
        for _ in 0..256 {
            if let Some(entry) = self.object_propdefs.index_lookup(&current)
                && let Some(definition) = entry.value.find_ref(&uuid)
            {
                return Some(definition.name());
            }

            let parent = self.object_parent.index_lookup(&current)?.value;
            if parent == NOTHING || parent == current {
                return None;
            }
            current = parent;
        }
        None
    }
}

/// Transaction-scoped bundle of resolution caches.
///
/// These caches are forked together at transaction start and published together
/// on commit to avoid mixed cache generations.
pub struct Caches {
    pub verb_resolution_cache: VerbResolutionCache,
    pub prop_resolution_cache: PropResolutionCache,
    pub ancestry_cache: AncestryCache,
}

impl Caches {
    /// Build empty caches for initial startup.
    pub fn new() -> Self {
        Self {
            verb_resolution_cache: VerbResolutionCache::new(),
            prop_resolution_cache: PropResolutionCache::new(),
            ancestry_cache: AncestryCache::default(),
        }
    }

    /// Fork all cache planes for use by a new transaction.
    pub fn fork(&self) -> Self {
        Self {
            verb_resolution_cache: self.verb_resolution_cache.fork(),
            prop_resolution_cache: self.prop_resolution_cache.fork(),
            ancestry_cache: self.ancestry_cache.fork(),
        }
    }

    /// Returns `true` if any cache in this bundle has staged modifications.
    pub fn has_changed(&self) -> bool {
        self.verb_resolution_cache.has_changed()
            || self.prop_resolution_cache.has_changed()
            || self.ancestry_cache.has_changed()
    }
}

const SEQUENCE_COUNT: usize = 15;
pub(crate) const DEFAULT_SNAPSHOT_ACQUISITION_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) struct SequenceState {
    values: [CachePadded<AtomicI64>; SEQUENCE_COUNT],
    dirty: AtomicU16,
}

impl SequenceState {
    fn new() -> Self {
        Self {
            values: [(); SEQUENCE_COUNT].map(|_| CachePadded::new(AtomicI64::new(-1))),
            dirty: AtomicU16::new(0),
        }
    }

    fn set_initial(&self, sequence: usize, value: i64) {
        self.values[sequence].store(value, Ordering::Relaxed);
    }

    pub(crate) fn load(&self, sequence: usize) -> i64 {
        self.values[sequence].load(Ordering::Relaxed)
    }

    pub(crate) fn increment(&self, sequence: usize) -> i64 {
        let value = self.values[sequence].fetch_add(1, Ordering::Relaxed) + 1;
        self.mark_dirty(sequence);
        value
    }

    pub(crate) fn update_max(&self, sequence: usize, value: i64) -> i64 {
        loop {
            let current = self.values[sequence].load(Ordering::Relaxed);
            if value <= current {
                return current;
            }
            if self.values[sequence]
                .compare_exchange(current, value, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                self.mark_dirty(sequence);
                return current;
            }
        }
    }

    fn mark_dirty(&self, sequence: usize) {
        self.dirty.fetch_or(1 << sequence, Ordering::Release);
    }

    fn claim_dirty(&self) -> u16 {
        self.dirty.swap(0, Ordering::AcqRel)
    }
}

/// Core storage engine for transactional world-state access.
pub struct MoorDB {
    monotonic: CachePadded<AtomicU64>,
    keyspace: Database,
    relations: Arc<Relations>,
    snapshot_planes: SnapshotPlanes,
    sequences: Arc<SequenceState>,
    /// Admission, publication tracking, and the background storage writer.
    coordinator: PersistenceCoordinator,
    shutdown_timeout: Duration,
    /// Keeps temp directory alive for the lifetime of the database when using
    /// an ephemeral path. Dropped after fjall shuts down in `Drop`.
    _tmpdir: Option<TempDir>,
}

impl TransactionContext for MoorDB {
    fn commit_writes(
        &self,
        ws: Box<WorkingSets>,
        enqueued_at: Instant,
    ) -> Result<CommitResult, WorldStateError> {
        self.commit_writes(ws, enqueued_at)
    }

    fn commit_read_only(&self, snapshot_version: u64, caches: Caches) {
        self.commit_read_only(snapshot_version, caches);
    }

    fn usage_bytes(&self) -> usize {
        self.usage_bytes()
    }
}

impl MoorDB {
    pub(crate) fn set_commit_queue_policy(&self, warn_after: Duration, timeout: Duration) {
        self.coordinator
            .set_commit_queue_policy(warn_after, timeout);
    }

    /// Create a snapshot-based SnapshotInterface for consistent read-only access
    pub fn create_snapshot(&self) -> Result<Box<dyn SnapshotInterface>, crate::tx::Error> {
        self.create_snapshot_with_timeout(DEFAULT_SNAPSHOT_ACQUISITION_TIMEOUT)
    }

    /// Create a snapshot with an explicit acquisition deadline.
    ///
    /// The deadline covers the applied barrier, queue submission, and the snapshot receipt.
    pub fn create_snapshot_with_timeout(
        &self,
        timeout: std::time::Duration,
    ) -> Result<Box<dyn SnapshotInterface>, crate::tx::Error> {
        let published_version = self.snapshot_planes.load_root().version;
        let snapshot = self
            .coordinator
            .snapshot(published_version, timeout)
            .map_err(|error| crate::tx::Error::StorageFailure(error.to_string()))?;

        // Return a custom SnapshotInterface implementation that uses this snapshot
        Ok(Box::new(FjallSnapshotLoader {
            snapshot,
            object_location_keyspace: self.relations.object_location.source().partition().clone(),
            object_flags_keyspace: self.relations.object_flags.source().partition().clone(),
            object_parent_keyspace: self.relations.object_parent.source().partition().clone(),
            object_owner_keyspace: self.relations.object_owner.source().partition().clone(),
            object_name_keyspace: self.relations.object_name.source().partition().clone(),
            object_verbdefs_keyspace: self.relations.object_verbdefs.source().partition().clone(),
            object_verbs_keyspace: self.relations.object_verbs.source().partition().clone(),
            object_propdefs_keyspace: self.relations.object_propdefs.source().partition().clone(),
            object_propvalues_keyspace: self
                .relations
                .object_propvalues
                .source()
                .partition()
                .clone(),
            object_propflags_keyspace: self.relations.object_propflags.source().partition().clone(),
            entity_metadata_keyspace: self.relations.entity_metadata.source().partition().clone(),
        }))
    }

    pub(crate) fn publication(&self) -> PublicationId {
        self.coordinator.published(self.published_version())
    }

    pub(crate) fn persistence_status(&self) -> PersistenceStatus {
        self.publication();
        self.coordinator.status()
    }

    pub(crate) fn wait_applied(
        &self,
        publication: PublicationId,
        timeout: Duration,
    ) -> Result<PersistenceReceipt, PersistenceError> {
        self.coordinator.wait_applied(publication, timeout)
    }

    pub(crate) fn wait_durable(
        &self,
        publication: PublicationId,
        timeout: Duration,
    ) -> Result<PersistenceReceipt, PersistenceError> {
        self.coordinator.wait_durable(publication, timeout)
    }

    /// Wait for the current published world state to be applied to storage.
    ///
    /// This is an application-level handoff boundary, not a durable-storage fence. It is
    /// explicitly unbounded and reports terminal writer failure.
    pub fn wait_for_persistence(&self) -> Result<(), String> {
        let published_version = self.snapshot_planes.load_root().version;
        let publication = self.coordinator.published(published_version);
        self.coordinator
            .wait_applied_unbounded(publication)
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    /// Deadline-bounded variant of [`Self::wait_for_persistence`] for administrative callers.
    pub fn wait_for_persistence_with_deadline(&self, deadline: Duration) -> Result<(), String> {
        let published_version = self.snapshot_planes.load_root().version;
        let publication = self.coordinator.published(published_version);
        self.coordinator
            .wait_applied(publication, deadline)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// The currently published root version.
    pub fn published_version(&self) -> u64 {
        self.snapshot_planes.load_root().version
    }

    /// Wait until the current published state has crossed the durable-storage fence.
    pub fn wait_for_durability(&self, deadline: Duration) -> Result<(), String> {
        let publication = self.coordinator.published(self.published_version());
        self.coordinator
            .wait_durable(publication, deadline)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// Create a transaction bound to the current published snapshot.
    pub(crate) fn start_transaction(self: &Arc<Self>) -> WorldStateTransaction {
        self.relations
            .start_transaction(self.clone(), self.acquire_tx_seed())
    }

    /// Stop background workers and drain queued persistence work.
    pub fn stop(&self) -> Result<(), String> {
        let published_version = self.snapshot_planes.load_root().version;

        info!(
            "Stopping persistence coordinator (published version: {}, deadline: {:?})",
            published_version, self.shutdown_timeout
        );
        let stop_result = self
            .coordinator
            .shutdown()
            .map_err(|error| error.to_string());

        let status = self.coordinator.status();
        info!(
            writer_epoch = status.epoch.as_u64(),
            published = status.published,
            last_submitted = status.last_submitted,
            outstanding = status.outstanding,
            healthy = status.healthy,
            applied = status.applied,
            durable = status.durable,
            shutdown = status.shutdown,
            "Persistence status at shutdown"
        );
        let final_completed = status.applied;
        if published_version > 0 && final_completed < published_version {
            let detail = format!(
                "persistence stopped before completing all writes: expected {published_version}, got {final_completed}"
            );
            error!("{detail}");
            self.relations.stop_all();
            return Err(stop_result.err().unwrap_or(detail));
        }
        if published_version > 0 {
            info!("All writes completed up to version {}", final_completed);
        }

        self.relations.stop_all();
        stop_result
    }

    /// Open (or initialize) a database and return `(db, fresh)`.
    ///
    /// `fresh` indicates no existing relation keyspaces were found.
    pub fn try_open(
        storage: StorageConfig,
        config: DatabaseConfig,
        persistence: PersistenceConfig,
    ) -> Result<(Arc<Self>, bool), DatabaseOpenError> {
        let StorageConfig::Fjall(fjall) = storage;
        let tmpdir = if fjall.path.is_none() {
            Some(TempDir::new().map_err(|source| DatabaseOpenError::TempDir { source })?)
        } else {
            None
        };
        let path_buf = fjall
            .path
            .clone()
            .unwrap_or_else(|| tmpdir.as_ref().unwrap().path().to_path_buf());
        let path = path_buf.as_path();

        fjall_format::fjall_check_format(path).map_err(|e| DatabaseOpenError::Format {
            path: path_buf.clone(),
            detail: e.to_string(),
        })?;

        let keyspace = Database::builder(path)
            .open()
            .map_err(|e| DatabaseOpenError::Open {
                path: path_buf.clone(),
                detail: e.to_string(),
            })?;

        let sequences_partition = keyspace
            .keyspace("sequences", KeyspaceCreateOptions::default)
            .map_err(|e| DatabaseOpenError::Keyspace {
                path: path_buf.clone(),
                keyspace: "sequences",
                detail: e.to_string(),
            })?;

        let sequences = Arc::new(SequenceState::new());
        let mut initial_sequences = vec![-1_i64; SEQUENCE_COUNT];

        let mut fresh = false;
        if !keyspace.keyspace_exists("object_location") {
            fresh = true;
        }

        if !fresh {
            // Load sequences from existing database
            for (i, slot) in initial_sequences.iter_mut().enumerate() {
                let seq_value = sequences_partition
                    .get(i.to_le_bytes())
                    .map_err(|e| DatabaseOpenError::ReadSequence {
                        path: path_buf.clone(),
                        index: i,
                        detail: e.to_string(),
                    })?
                    .map(|b| {
                        b.get(..8)
                            .and_then(|bytes| bytes.try_into().ok())
                            .map(i64::from_le_bytes)
                            .ok_or_else(|| DatabaseOpenError::DecodeSequence {
                                path: path_buf.clone(),
                                index: i,
                                len: b.len(),
                            })
                    })
                    .transpose()?
                    .unwrap_or(-1);
                sequences.set_initial(i, seq_value);
                *slot = seq_value;
            }
        }

        let relations = Arc::new(Relations::init(&keyspace, &config, &path_buf)?);
        let (initial_root, property_value_chains) =
            relations.snapshot(0, Timestamp(0), Arc::new(Caches::new()), &path_buf)?;
        let start_tx_num = initial_root
            .committed_ts
            .0
            .checked_add(1)
            .ok_or_else(|| DatabaseOpenError::TransactionTimestampExhausted {
                path: path_buf.clone(),
            })?
            .max(1);
        let property_names = initial_root
            .object_propdefs
            .iter()
            .flat_map(|(_, entry)| entry.value.iter())
            .map(|definition| (definition.uuid(), definition.name()))
            .collect();
        let snapshot_planes = SnapshotPlanes::new(Arc::new(initial_root));
        let writer = BatchWriter::with_property_value_state(
            keyspace.clone(),
            sequences_partition,
            initial_sequences,
            Some(relations.clone()),
            property_names,
            property_value_chains,
            PROPERTY_VALUE_CHAIN_LIMITS,
        );
        let shutdown_timeout = persistence.shutdown_timeout;
        let coordinator = PersistenceCoordinator::new(WriterEpoch::random(), persistence, writer);
        info!(
            writer_epoch = coordinator.epoch().as_u64(),
            "Persistence coordinator started"
        );

        let s = Arc::new(Self {
            monotonic: CachePadded::new(AtomicU64::new(start_tx_num)),
            relations,
            snapshot_planes,
            sequences,
            coordinator,
            shutdown_timeout,
            keyspace,
            _tmpdir: tmpdir,
        });

        Ok((s, fresh))
    }

    /// Return current on-disk database usage in bytes.
    pub fn usage_bytes(&self) -> usize {
        self.keyspace.disk_space().unwrap_or_default() as usize
    }

    /// Return a point-in-time view of Fjall's background maintenance state.
    pub fn storage_maintenance_stats(&self) -> StorageMaintenanceStats {
        StorageMaintenanceStats {
            write_buffer_bytes: self.keyspace.write_buffer_size(),
            outstanding_flushes: self.keyspace.outstanding_flushes(),
            active_compactions: self.keyspace.active_compactions(),
            compactions_completed: self.keyspace.compactions_completed(),
            compaction_time: self.keyspace.time_compacting(),
            journal_count: self.keyspace.journal_count(),
            journal_bytes: self.keyspace.journal_disk_space().unwrap_or_default(),
            disk_bytes: self.keyspace.disk_space().unwrap_or_default(),
        }
    }

    /// Flush and major-compact only the selected relation keyspaces.
    pub fn compact_relations(
        &self,
        relations: &[DatabaseRelation],
    ) -> Result<Vec<RelationCompactionResult>, String> {
        self.wait_for_persistence()?;
        Ok(self.relations.compact_relations(relations))
    }

    /// Mark all relations as fully loaded from their backing providers.
    /// Call this after bulk import operations to enable optimized reads.
    pub fn mark_all_fully_loaded(&self) {
        self.snapshot_planes.mark_all_fully_loaded(&self.relations);
    }
}

impl MoorDB {
    /// Capture timestamp, snapshot, sequence handles, and forked caches for startup.
    fn acquire_tx_seed(&self) -> TxSeed {
        let (snapshot, caches) = self.snapshot_planes.acquire_seed_caches();
        TxSeed {
            tx: Tx {
                ts: Timestamp(
                    self.monotonic
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                ),
                visible_ts: snapshot.committed_ts,
                snapshot_version: snapshot.version,
            },
            snapshot,
            sequences: self.sequences.clone(),
            caches,
        }
    }
}

impl Drop for MoorDB {
    fn drop(&mut self) {
        info!("MoorDB::drop() called - initiating shutdown");
        if let Err(error) = self.stop() {
            error!("MoorDB shutdown failed: {error}");
        }
        info!("MoorDB shutdown complete");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::property_value_store::{
        PropertyValueRecordKind, decode_property_value_record, decode_property_value_record_key,
    };
    use fjall::PersistMode;
    use moor_common::{
        model::{CommitResult, ObjAttrs, ObjectKind, PropFlag},
        util::BitEnum,
    };
    use moor_var::{NOTHING, Obj, Symbol, v_int, v_list};

    fn property_value_records(
        db: &MoorDB,
    ) -> Vec<(ObjAndUUIDHolder, u64, PropertyValueRecordKind, Timestamp)> {
        db.relations
            .object_propvalues
            .provider()
            .partition()
            .iter()
            .map(|entry| {
                let (key, value) = entry.into_inner().unwrap();
                let key = decode_property_value_record_key(&key).unwrap();
                let value = decode_property_value_record(&value).unwrap();
                (
                    key.property,
                    key.record_version,
                    value.kind,
                    value.logical_timestamp,
                )
            })
            .collect()
    }

    #[test]
    fn sequence_increment_returns_each_reserved_value_once() {
        let sequences = Arc::new(SequenceState::new());
        let mut threads = Vec::new();
        for _ in 0..8 {
            let sequences = sequences.clone();
            threads.push(std::thread::spawn(move || {
                (0..1_000)
                    .map(|_| sequences.increment(SEQUENCE_MAX_OBJECT))
                    .collect::<Vec<_>>()
            }));
        }

        let mut values = threads
            .into_iter()
            .flat_map(|thread| thread.join().unwrap())
            .collect::<Vec<_>>();
        values.sort_unstable();
        assert_eq!(values, (0..8_000).collect::<Vec<_>>());

        let dirty = sequences.claim_dirty();
        assert_ne!(dirty & (1 << SEQUENCE_MAX_OBJECT), 0);
        assert_eq!(sequences.claim_dirty(), 0);
    }

    #[test]
    fn reopen_recovers_tuple_timestamps_before_starting_new_transactions() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path();
        let db = MoorDB::try_open(
            StorageConfig::fjall(path),
            DatabaseConfig::default(),
            PersistenceConfig::default(),
        )
        .unwrap()
        .0;

        for _ in 0..40 {
            drop(db.start_transaction());
        }
        let mut tx = db.start_transaction();
        tx.set_object_name(&Obj::mk_id(1), "persisted".to_string())
            .unwrap();
        let CommitResult::Success {
            timestamp: committed_timestamp,
            ..
        } = tx.commit().unwrap()
        else {
            panic!("write transaction did not commit");
        };
        db.stop().unwrap();
        drop(db);

        let reopened = MoorDB::try_open(
            StorageConfig::fjall(path),
            DatabaseConfig::default(),
            PersistenceConfig::default(),
        )
        .unwrap()
        .0;
        let root = reopened.snapshot_planes.load_root();
        let entry = root.object_name.index_lookup(&Obj::mk_id(1)).unwrap();
        assert_eq!(entry.ts, Timestamp(committed_timestamp));

        let tx = reopened.start_transaction();
        assert!(tx.tx.ts.0 > committed_timestamp);
    }

    #[test]
    fn shutdown_drains_an_attempt_admitted_before_shutdown() {
        let db = MoorDB::try_open(
            StorageConfig::temporary_fjall(),
            DatabaseConfig::default(),
            PersistenceConfig::default(),
        )
        .unwrap()
        .0;
        let admission = db.coordinator.admit(Timestamp(1)).unwrap();
        let shutdown_db = db.clone();
        let shutdown = std::thread::spawn(move || shutdown_db.stop());
        let started = std::time::Instant::now();
        while !db.coordinator.status().shutdown {
            assert!(started.elapsed() < Duration::from_secs(2));
            std::thread::yield_now();
        }
        assert!(!shutdown.is_finished());
        let tx = db.start_transaction();
        let (changes, _, _, _) = tx
            .into_working_sets()
            .unwrap()
            .extract_relation_working_sets();
        db.coordinator
            .submit(
                crate::provider::logical::LogicalCommit {
                    publication: db.coordinator.published(1),
                    timestamp: Timestamp(1),
                    changes: changes.into_changes(),
                    sequences: Vec::new(),
                    property_definition_changes: Vec::new(),
                },
                admission,
            )
            .unwrap();
        shutdown.join().unwrap().unwrap();
        assert_eq!(db.coordinator.status().applied, 1);
    }

    #[test]
    fn durability_fence_advances_after_a_write() {
        let db = MoorDB::try_open(
            StorageConfig::temporary_fjall(),
            DatabaseConfig::default(),
            PersistenceConfig::default(),
        )
        .unwrap()
        .0;

        let mut tx = db.start_transaction();
        tx.set_object_name(&Obj::mk_id(1), "durable".to_string())
            .unwrap();
        assert!(matches!(tx.commit().unwrap(), CommitResult::Success { .. }));

        db.wait_for_durability(Duration::from_secs(5)).unwrap();
        assert!(db.coordinator.status().durable >= 1);
    }

    #[test]
    fn sequence_high_water_survives_reopen() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path();
        let db = MoorDB::try_open(
            StorageConfig::fjall(path),
            DatabaseConfig::default(),
            PersistenceConfig::default(),
        )
        .unwrap()
        .0;

        let mut tx = db.start_transaction();
        tx.create_object(ObjectKind::NextObjid, ObjAttrs::default())
            .unwrap();
        assert!(matches!(tx.commit().unwrap(), CommitResult::Success { .. }));
        db.wait_for_persistence().unwrap();
        let allocated = db.sequences.load(SEQUENCE_MAX_OBJECT);
        assert!(allocated >= 0);

        db.stop().unwrap();
        drop(db);

        let reopened = MoorDB::try_open(
            StorageConfig::fjall(path),
            DatabaseConfig::default(),
            PersistenceConfig::default(),
        )
        .unwrap()
        .0;
        assert!(reopened.sequences.load(SEQUENCE_MAX_OBJECT) >= allocated);
    }

    #[test]
    fn property_append_chain_survives_snapshot_export_and_reopen() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path();
        let db = MoorDB::try_open(
            StorageConfig::fjall(path),
            DatabaseConfig::default(),
            PersistenceConfig::default(),
        )
        .unwrap()
        .0;

        let mut tx = db.start_transaction();
        let object = tx
            .create_object(
                ObjectKind::NextObjid,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "append test"),
            )
            .unwrap();
        let property_uuid = tx
            .define_property(
                &object,
                &object,
                Symbol::mk("values"),
                &object,
                BitEnum::new(),
                Some(v_list(&[v_int(1), v_int(2)])),
            )
            .unwrap();
        assert!(matches!(tx.commit().unwrap(), CommitResult::Success { .. }));

        let mut tx = db.start_transaction();
        let current = tx
            .retrieve_property(&object, property_uuid)
            .unwrap()
            .0
            .unwrap();
        let appended = current.push(&v_int(3)).unwrap();
        tx.set_property(&object, property_uuid, appended.clone())
            .unwrap();
        let CommitResult::Success {
            timestamp: append_timestamp,
            ..
        } = tx.commit().unwrap()
        else {
            panic!("append transaction did not commit");
        };
        db.wait_for_persistence().unwrap();

        let property = ObjAndUUIDHolder::new(&object, property_uuid);
        let records = property_value_records(&db);
        assert_eq!(records.len(), 2);
        assert!(records.iter().all(|(stored, ..)| stored == &property));
        assert_eq!(records[0].2, PropertyValueRecordKind::Full);
        assert_eq!(records[1].2, PropertyValueRecordKind::ListAppend);
        assert_eq!(records[1].3, Timestamp(append_timestamp));

        let snapshot = db.create_snapshot().unwrap();
        assert_eq!(
            snapshot
                .get_property_value(&object, property_uuid)
                .unwrap()
                .0,
            Some(appended.clone())
        );
        let mut export = snapshot.begin_export(&[]).unwrap();
        let exported = export.next_object().unwrap().unwrap();
        let exported_property = exported
            .properties
            .iter()
            .find(|property| property.definition.uuid() == property_uuid)
            .unwrap();
        assert_eq!(exported_property.value, Some(appended.clone()));
        assert!(export.next_object().unwrap().is_none());
        drop(export);
        drop(snapshot);

        let old_max_version = records.iter().map(|record| record.1).max().unwrap();
        db.stop().unwrap();
        drop(db);

        let reopened = MoorDB::try_open(
            StorageConfig::fjall(path),
            DatabaseConfig::default(),
            PersistenceConfig::default(),
        )
        .unwrap()
        .0;
        let root = reopened.snapshot_planes.load_root();
        let recovered = root.object_propvalues.index_lookup(&property).unwrap();
        assert_eq!(recovered.value, appended);
        assert_eq!(recovered.ts, Timestamp(append_timestamp));

        let mut tx = reopened.start_transaction();
        let current = tx
            .retrieve_property(&object, property_uuid)
            .unwrap()
            .0
            .unwrap();
        let appended_again = current.push(&v_int(4)).unwrap();
        tx.set_property(&object, property_uuid, appended_again.clone())
            .unwrap();
        assert!(matches!(tx.commit().unwrap(), CommitResult::Success { .. }));
        reopened.wait_for_persistence().unwrap();

        let records = property_value_records(&reopened);
        assert_eq!(records.len(), 3);
        assert_eq!(records[2].2, PropertyValueRecordKind::ListAppend);
        assert!(records[2].1 > old_max_version);
        assert_eq!(
            reopened
                .create_snapshot()
                .unwrap()
                .get_property_value(&object, property_uuid)
                .unwrap()
                .0,
            Some(appended_again)
        );

        let mut tx = reopened.start_transaction();
        tx.set_property(&object, property_uuid, v_list(&[v_int(99)]))
            .unwrap();
        assert!(matches!(tx.commit().unwrap(), CommitResult::Success { .. }));
        reopened.wait_for_persistence().unwrap();
        let records = property_value_records(&reopened);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].2, PropertyValueRecordKind::Full);

        let mut tx = reopened.start_transaction();
        tx.clear_property(&object, property_uuid).unwrap();
        assert!(matches!(tx.commit().unwrap(), CommitResult::Success { .. }));
        reopened.wait_for_persistence().unwrap();
        assert!(property_value_records(&reopened).is_empty());
    }

    #[test]
    fn reopen_rejects_a_corrupt_property_value_chain() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path();
        let db = MoorDB::try_open(
            StorageConfig::fjall(path),
            DatabaseConfig::default(),
            PersistenceConfig::default(),
        )
        .unwrap()
        .0;

        let mut tx = db.start_transaction();
        let object = tx
            .create_object(ObjectKind::NextObjid, ObjAttrs::default())
            .unwrap();
        tx.define_property(
            &object,
            &object,
            Symbol::mk("value"),
            &object,
            BitEnum::new(),
            Some(v_int(1)),
        )
        .unwrap();
        assert!(matches!(tx.commit().unwrap(), CommitResult::Success { .. }));
        db.stop().unwrap();
        drop(db);

        let database = Database::builder(path).open().unwrap();
        let values = database
            .keyspace("object_propvalues", KeyspaceCreateOptions::default)
            .unwrap();
        let (key, _) = values.iter().next().unwrap().into_inner().unwrap();
        values.insert(key, b"corrupt").unwrap();
        database.persist(PersistMode::SyncAll).unwrap();
        drop(values);
        drop(database);

        let Err(error) = MoorDB::try_open(
            StorageConfig::fjall(path),
            DatabaseConfig::default(),
            PersistenceConfig::default(),
        ) else {
            panic!("database with a corrupt property-value chain was accepted");
        };
        assert!(matches!(
            error,
            DatabaseOpenError::SeedRelation {
                relation: "object_propvalues",
                ..
            }
        ));
    }

    #[test]
    fn reopen_rejects_a_missing_database_format_marker() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path();
        let db = MoorDB::try_open(
            StorageConfig::fjall(path),
            DatabaseConfig::default(),
            PersistenceConfig::default(),
        )
        .unwrap()
        .0;
        db.stop().unwrap();
        drop(db);

        let database = Database::builder(path).open().unwrap();
        let sequences = database
            .keyspace("sequences", KeyspaceCreateOptions::default)
            .unwrap();
        sequences.remove(b"__db_version__").unwrap();
        database.persist(PersistMode::SyncAll).unwrap();
        drop(sequences);
        drop(database);

        let Err(error) = MoorDB::try_open(
            StorageConfig::fjall(path),
            DatabaseConfig::default(),
            PersistenceConfig::default(),
        ) else {
            panic!("database without a format marker was accepted");
        };
        assert!(matches!(error, DatabaseOpenError::Format { .. }));
    }

    #[test]
    fn relation_compaction_only_rotates_selected_relation() {
        let db = MoorDB::try_open(
            StorageConfig::temporary_fjall(),
            DatabaseConfig::default(),
            PersistenceConfig::default(),
        )
        .unwrap()
        .0;
        let mut tx = db.start_transaction();
        let object = tx
            .create_object(
                ObjectKind::NextObjid,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();
        tx.define_property(
            &object,
            &object,
            Symbol::mk("value"),
            &object,
            PropFlag::all_flags(),
            Some(v_int(1)),
        )
        .unwrap();
        assert!(matches!(tx.commit().unwrap(), CommitResult::Success { .. }));
        db.wait_for_persistence().unwrap();

        let selected = db.relations.object_propvalues.provider().partition();
        let unrelated = db.relations.object_name.provider().partition();
        let selected_tables_before = selected.table_count();
        let unrelated_tables_before = unrelated.table_count();

        let results = db
            .compact_relations(&[DatabaseRelation::ObjectPropvalues])
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].relation, DatabaseRelation::ObjectPropvalues);
        assert_eq!(results[0].error, None);
        assert!(selected.table_count() > selected_tables_before);
        assert_eq!(unrelated.table_count(), unrelated_tables_before);
    }

    #[test]
    fn relation_compaction_returns_one_ordered_result_per_selection() {
        let db = MoorDB::try_open(
            StorageConfig::temporary_fjall(),
            DatabaseConfig::default(),
            PersistenceConfig::default(),
        )
        .unwrap()
        .0;
        let selected = [
            DatabaseRelation::ObjectPropvalues,
            DatabaseRelation::ObjectPropflags,
        ];

        let results = db.compact_relations(&selected).unwrap();

        assert_eq!(results.len(), selected.len());
        assert_eq!(results[0].relation, selected[0]);
        assert_eq!(results[1].relation, selected[1]);
    }

    #[test]
    fn commit_admission_timeout_does_not_publish_transaction() {
        let db = MoorDB::try_open(
            StorageConfig::temporary_fjall(),
            DatabaseConfig::default(),
            PersistenceConfig::default(),
        )
        .unwrap()
        .0;
        db.set_commit_queue_policy(Duration::ZERO, Duration::from_millis(10));
        let admission = db.coordinator.hold_all_admission();
        let root_version = db.snapshot_planes.load_root().version;
        let object = Obj::mk_id(1);
        let mut tx = db.start_transaction();
        tx.set_object_name(&object, "rejected".to_string()).unwrap();

        let error = tx.commit().unwrap_err();

        assert!(matches!(error, WorldStateError::DatabaseOverloaded(_)));
        assert_eq!(db.snapshot_planes.load_root().version, root_version);
        assert!(matches!(
            db.start_transaction().get_object_name(&object),
            Err(WorldStateError::ObjectNotFound(_))
        ));

        drop(admission);
        let mut tx = db.start_transaction();
        tx.set_object_name(&object, "accepted".to_string()).unwrap();
        assert!(matches!(tx.commit().unwrap(), CommitResult::Success { .. }));
        assert_eq!(
            db.start_transaction().get_object_name(&object).unwrap(),
            "accepted"
        );
    }
}
