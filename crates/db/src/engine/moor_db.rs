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

use crate::provider::fjall_provider;
use crate::{
    AnonymousObjectMetadata, ObjAndUUIDHolder, StringHolder,
    cache::{
        ancestry_cache::AncestryCache, prop_cache::PropResolutionCache,
        verb_cache::VerbResolutionCache,
    },
    config::DatabaseConfig,
    tx::{CheckRelation, Relation, RelationTransaction, Timestamp, Tx, WorkingSet},
};
use crate::{
    engine::relation_defs::define_relations,
    provider::{
        Migrator,
        batch_writer::{BatchCollector, BatchWriter},
        fjall_migration::{self, FjallMigrator},
        fjall_provider::FjallProvider,
        fjall_snapshot_loader::FjallSnapshotLoader,
    },
};
use fjall::{Database, KeyspaceCreateOptions};
use moor_common::model::loader::SnapshotInterface;
use moor_common::util::CachePadded;
use moor_common::util::Instant;
use moor_common::{
    model::{CommitResult, ObjFlag, PropDefs, PropPerms, VerbDefs},
    util::BitEnum,
};
use moor_var::{Obj, Symbol, Var, program::ProgramType};
use parking_lot::Mutex;
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicI64, AtomicU64},
    },
};
use tempfile::TempDir;
use tracing::{error, info, warn};

mod commit_pipeline;
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
    object_last_move => Obj, Var,
    anonymous_object_metadata => Obj, AnonymousObjectMetadata,
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

/// Core storage engine for transactional world-state access.
pub struct MoorDB {
    monotonic: CachePadded<AtomicU64>,
    /// Serializes write commit processing.
    commit_apply_lock: Mutex<()>,
    keyspace: Database,
    relations: Relations,
    snapshot_planes: SnapshotPlanes,
    sequences: Arc<[CachePadded<AtomicI64>; 16]>,
    /// Background writer for sequence persistence
    sequence_writer: fjall_provider::SequenceWriter,
    /// Shared batch collector for all providers
    batch_collector: Arc<BatchCollector>,
    /// Single background writer for all fjall operations
    batch_writer: BatchWriter,
    /// Last write transaction timestamp that completed
    last_write_commit: AtomicU64,
    /// Keeps temp directory alive for the lifetime of the database when using
    /// an ephemeral path. Dropped after fjall shuts down in `Drop`.
    _tmpdir: Option<TempDir>,
}

impl TransactionContext for MoorDB {
    fn commit_writes(&self, ws: Box<WorkingSets>, enqueued_at: Instant) -> CommitResult {
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
    /// Create a snapshot-based SnapshotInterface for consistent read-only access
    pub fn create_snapshot(&self) -> Result<Box<dyn SnapshotInterface>, crate::tx::Error> {
        // Wait for all write transactions up to the last completed write to finish
        // This ensures the snapshot captures all committed write data
        let last_write_timestamp = Timestamp(
            self.last_write_commit
                .load(std::sync::atomic::Ordering::Acquire),
        );
        if last_write_timestamp.0 > 0
            && let Err(e) = self
                .batch_writer
                .wait_for_barrier(last_write_timestamp, std::time::Duration::from_secs(10))
        {
            warn!(
                "Timeout waiting for write barrier {} before snapshot: {}",
                last_write_timestamp.0, e
            );
            // Continue anyway - the snapshot might be slightly inconsistent but we don't want to fail completely
        }

        // Get a database-wide snapshot
        let snapshot = self.keyspace.snapshot();

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
            anonymous_object_metadata_keyspace: self
                .relations
                .anonymous_object_metadata
                .source()
                .partition()
                .clone(),
        }))
    }

    /// Create a transaction bound to the current published snapshot.
    pub(crate) fn start_transaction(self: &Arc<Self>) -> WorldStateTransaction {
        self.relations
            .start_transaction(self.clone(), self.acquire_tx_seed())
    }

    /// Stop background workers and drain queued persistence work.
    pub fn stop(&self) {
        // Get the last write timestamp before stopping the writer
        let last_write_timestamp = Timestamp(
            self.last_write_commit
                .load(std::sync::atomic::Ordering::Acquire),
        );

        // Stop batch writer - this drains the queue and waits for completion
        info!(
            "Stopping batch writer (last write timestamp: {})",
            last_write_timestamp.0
        );
        self.batch_writer.stop();

        // Verify all writes completed
        let final_completed = self.batch_writer.completed_timestamp();
        if last_write_timestamp.0 > 0 && final_completed < last_write_timestamp.0 {
            error!(
                "Batch writer stopped before completing all writes: expected {}, got {}",
                last_write_timestamp.0, final_completed
            );
        } else if last_write_timestamp.0 > 0 {
            info!("All writes completed up to timestamp {}", final_completed);
        }

        self.sequence_writer.stop();
        self.relations.stop_all();
    }

    /// Open (or initialize) a database and return `(db, fresh)`.
    ///
    /// `fresh` indicates no existing relation keyspaces were found.
    pub fn open(path: Option<&Path>, config: DatabaseConfig) -> (Arc<Self>, bool) {
        let tmpdir = if path.is_none() {
            Some(TempDir::new().unwrap())
        } else {
            None
        };
        let path = path.unwrap_or_else(|| tmpdir.as_ref().unwrap().path());

        // Check and perform migration BEFORE opening the database
        // This ensures migration happens atomically via copy-and-swap
        fjall_migration::fjall_check_and_migrate(path)
            .unwrap_or_else(|e| panic!("Failed to migrate database: {e}"));

        let keyspace = Database::builder(path).open().unwrap();

        let sequences_partition = keyspace
            .keyspace("sequences", KeyspaceCreateOptions::default)
            .unwrap();

        let sequences = Arc::new([(); 16].map(|_| CachePadded::new(AtomicI64::new(-1))));

        let mut fresh = false;
        if !keyspace.keyspace_exists("object_location") {
            fresh = true;
        }

        let start_tx_num = sequences_partition
            .get(15_u64.to_le_bytes())
            .unwrap()
            .map(|b| u64::from_le_bytes(b[0..8].try_into().unwrap()))
            .unwrap_or(1);

        if fresh {
            // Fresh database - mark it with the current version
            let migrator = FjallMigrator::new(keyspace.clone(), sequences_partition.clone());
            migrator
                .mark_current_version()
                .unwrap_or_else(|e| error!("Failed to mark fresh database version: {}", e));
        } else {
            // Load sequences from existing database
            for (i, seq) in sequences.iter().enumerate() {
                let seq_value = sequences_partition
                    .get(i.to_le_bytes())
                    .unwrap()
                    .map(|b| i64::from_le_bytes(b[0..8].try_into().unwrap()))
                    .unwrap_or(-1);
                seq.store(seq_value, std::sync::atomic::Ordering::SeqCst);
            }
        }

        // Create shared batch collector and writer for all providers
        let batch_collector = Arc::new(BatchCollector::new());
        let batch_writer = BatchWriter::new(keyspace.clone());

        let relations = Relations::init(&keyspace, &config, batch_collector.clone());
        let initial_root = Arc::new(relations.snapshot(0, Arc::new(Caches::new())));
        let snapshot_planes = SnapshotPlanes::new(initial_root);

        // Create background sequence writer
        let sequence_writer = fjall_provider::SequenceWriter::new(sequences_partition.clone());

        let s = Arc::new(Self {
            monotonic: CachePadded::new(AtomicU64::new(start_tx_num)),
            commit_apply_lock: Mutex::new(()),
            relations,
            snapshot_planes,
            sequences,
            sequence_writer,
            batch_collector,
            batch_writer,
            keyspace,
            last_write_commit: AtomicU64::new(0),
            _tmpdir: tmpdir,
        });

        (s, fresh)
    }

    /// Return current on-disk database usage in bytes.
    pub fn usage_bytes(&self) -> usize {
        self.keyspace.disk_space().unwrap_or_default() as usize
    }

    /// Mark all relations as fully loaded from their backing providers.
    /// Call this after bulk import operations to enable optimized reads.
    pub fn mark_all_fully_loaded(&self) {
        let relations = &self.relations;
        self.snapshot_planes
            .update_root(|current_root| relations.snapshot_with_all_fully_loaded(current_root));
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
        self.stop();
        info!("MoorDB shutdown complete");
    }
}
