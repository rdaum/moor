// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use crate::{
    AnonymousObjectMetadata, ObjAndUUIDHolder, StringHolder,
    config::DatabaseConfig,
    db_worldstate::db_counters,
    prop_cache::PropResolutionCache,
    tx_management::{CheckRelation, Relation, RelationTransaction, Timestamp, Tx, WorkingSet},
    verb_cache::{AncestryCache, VerbResolutionCache},
};
use crate::{
    provider::{
        Migrator,
        batch_writer::{BatchCollector, BatchWriter},
        fjall_migration::{self, FjallMigrator},
        fjall_provider::FjallProvider,
        fjall_snapshot_loader::FjallSnapshotLoader,
    },
    relation_defs::define_relations,
};
use arc_swap::ArcSwap;
use fjall::{Database, KeyspaceCreateOptions};
use minstant::Instant;
use moor_common::util::CachePadded;
use moor_common::{
    model::{CommitResult, ObjFlag, PropDefs, PropPerms, VerbDefs},
    util::{BitEnum, PerfTimerGuard},
};
use moor_var::{Obj, Symbol, Var, program::ProgramType};
use parking_lot::Mutex;
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicI64, AtomicU64},
    },
    time::Duration,
};
use tempfile::TempDir;
use tracing::{error, info, warn};

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

/// Combined cache structure to ensure atomic updates across all caches
pub struct Caches {
    pub verb_resolution_cache: Box<VerbResolutionCache>,
    pub prop_resolution_cache: Box<PropResolutionCache>,
    pub ancestry_cache: Box<AncestryCache>,
}

impl Caches {
    pub fn new() -> Self {
        Self {
            verb_resolution_cache: Box::new(VerbResolutionCache::new()),
            prop_resolution_cache: Box::new(PropResolutionCache::new()),
            ancestry_cache: Box::new(AncestryCache::default()),
        }
    }

    pub fn fork(&self) -> Self {
        Self {
            verb_resolution_cache: self.verb_resolution_cache.fork(),
            prop_resolution_cache: self.prop_resolution_cache.fork(),
            ancestry_cache: self.ancestry_cache.fork(),
        }
    }

    pub fn has_changed(&self) -> bool {
        self.verb_resolution_cache.has_changed()
            || self.prop_resolution_cache.has_changed()
            || self.ancestry_cache.has_changed()
    }
}

pub struct MoorDB {
    monotonic: CachePadded<AtomicU64>,
    /// Serializes write commit processing.
    commit_apply_lock: Mutex<()>,
    keyspace: Database,
    relations: Relations,
    root_state: ArcSwap<WorldStateSnapshot>,
    sequences: [Arc<CachePadded<AtomicI64>>; 16],
    /// Background writer for sequence persistence
    sequence_writer: crate::provider::fjall_provider::SequenceWriter,
    /// Shared batch collector for all providers
    batch_collector: Arc<BatchCollector>,
    /// Single background writer for all fjall operations
    batch_writer: BatchWriter,
    /// Last write transaction timestamp that completed
    last_write_commit: AtomicU64,
}

impl TransactionContext for MoorDB {
    fn commit_writes(&self, ws: Box<WorkingSets>, enqueued_at: std::time::Instant) -> CommitResult {
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
    pub fn create_snapshot(
        &self,
    ) -> Result<Box<dyn moor_common::model::loader::SnapshotInterface>, crate::tx_management::Error>
    {
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

    pub(crate) fn start_transaction(self: &Arc<Self>) -> WorldStateTransaction {
        let snapshot = self.root_state.load();
        let tx = Tx {
            ts: Timestamp(
                self.monotonic
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            ),
            snapshot_version: snapshot.version,
        };

        let forked_caches = snapshot.caches.fork();
        self.relations.start_transaction(
            tx,
            self.clone(),
            Arc::clone(&snapshot),
            self.sequences.clone(),
            forked_caches.verb_resolution_cache,
            forked_caches.prop_resolution_cache,
            forked_caches.ancestry_cache,
        )
    }

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

        let sequences = [(); 16].map(|_| Arc::new(CachePadded::new(AtomicI64::new(-1))));

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
        let root_state = ArcSwap::new(Arc::new(relations.snapshot(0, Arc::new(Caches::new()))));

        // Create background sequence writer
        let sequence_writer =
            crate::provider::fjall_provider::SequenceWriter::new(sequences_partition.clone());

        let s = Arc::new(Self {
            monotonic: CachePadded::new(AtomicU64::new(start_tx_num)),
            commit_apply_lock: Mutex::new(()),
            relations,
            root_state,
            sequences,
            sequence_writer,
            batch_collector,
            batch_writer,
            keyspace,
            last_write_commit: AtomicU64::new(0),
        });

        (s, fresh)
    }

    pub fn usage_bytes(&self) -> usize {
        self.keyspace.disk_space().unwrap_or_default() as usize
    }

    /// Mark all relations as fully loaded from their backing providers.
    /// Call this after bulk import operations to enable optimized reads.
    pub fn mark_all_fully_loaded(&self) {
        let relations = &self.relations;
        self.root_state
            .rcu(|current_root| relations.snapshot_with_all_fully_loaded(current_root));
    }

    pub(crate) fn commit_read_only(&self, snapshot_version: u64, combined_caches: Caches) {
        if !combined_caches.has_changed() {
            return;
        }

        let next_caches = Arc::new(combined_caches);
        self.root_state.rcu(|current_root| {
            if current_root.version != snapshot_version {
                return current_root.clone();
            }

            let mut next_root = (**current_root).clone();
            next_root.caches = next_caches.clone();
            Arc::new(next_root)
        });
    }

    pub(crate) fn commit_writes(
        &self,
        ws: Box<WorkingSets>,
        enqueued_at: std::time::Instant,
    ) -> CommitResult {
        let counters = db_counters();
        let _commit_guard = self.commit_apply_lock.lock();
        let dequeued_at = std::time::Instant::now();
        counters.commit_lock_wait_phase.invocations().add(1);
        counters
            .commit_lock_wait_phase
            .cumulative_duration_nanos()
            .add(dequeued_at.duration_since(enqueued_at).as_nanos() as isize);

        let result = self.process_commit_writes(ws, counters);

        let reply_sent_at = std::time::Instant::now();
        counters.commit_process_phase.invocations().add(1);
        counters
            .commit_process_phase
            .cumulative_duration_nanos()
            .add(reply_sent_at.duration_since(dequeued_at).as_nanos() as isize);
        result
    }

    fn process_commit_writes(
        &self,
        ws: Box<WorkingSets>,
        counters: &moor_common::model::WorldStatePerf,
    ) -> CommitResult {
        let _t = PerfTimerGuard::new(&counters.commit_check_phase);
        let start_time = Instant::now();

        let current_root = self.root_state.load();
        let mut checkers = self.relations.begin_check_all(&current_root);

        let num_tuples = ws.total_tuples();
        if num_tuples > 10_000 {
            warn!(
                "Potential large batch @ commit... Checking {num_tuples} total tuples from the working set..."
            );
        }

        // Get the transaction timestamp and mutations flag before extracting working sets
        let tx_timestamp = ws.tx.ts;
        let snapshot_version = ws.tx.snapshot_version;
        let has_mutations = ws.has_mutations;
        let (mut relation_ws, verb_cache, prop_cache, ancestry_cache) =
            ws.extract_relation_working_sets();

        // Optimization: If no mutation commits completed since transaction start, skip conflict checking.
        // The transaction already validated against its snapshot when creating operations.
        let skip_conflict_check = snapshot_version == current_root.version;

        {
            // Conflict validation - can skip if no concurrent commits
            if !skip_conflict_check && let Err(conflict_info) = checkers.check_all(&mut relation_ws)
            {
                warn!("Transaction conflict during commit: {}", conflict_info);
                return CommitResult::ConflictRetry {
                    conflict_info: Some(conflict_info),
                };
            }
            drop(_t);

            // Mutation detection - use has_mutations since we might have skipped check_all
            // (which normally sets the dirty flags that all_clean checks)
            if !has_mutations {
                self.commit_read_only(
                    snapshot_version,
                    Caches {
                        verb_resolution_cache: verb_cache,
                        prop_resolution_cache: prop_cache,
                        ancestry_cache,
                    },
                );
                return CommitResult::Success {
                    mutations_made: false,
                    timestamp: tx_timestamp.0,
                };
            }

            // Warn if the check phase took a really long time
            if start_time.elapsed() > Duration::from_secs(5) {
                warn!(
                    "Long running commit; check phase took {}s for {num_tuples} tuples",
                    start_time.elapsed().as_secs_f32()
                );
            }

            let _t = PerfTimerGuard::new(&counters.commit_apply_phase);

            // Start collecting operations for this commit's batch
            self.batch_collector.start_commit(tx_timestamp, num_tuples);

            let checkers = match checkers.apply_all(relation_ws) {
                Ok(checkers) => checkers,
                Err(()) => {
                    // Discard the batch on failure
                    self.batch_collector.abort_commit();
                    warn!("Transaction conflict during apply phase (no detailed info available)");
                    return CommitResult::ConflictRetry {
                        conflict_info: None,
                    };
                }
            };

            // Take the completed batch and send to background writer
            let batch = self.batch_collector.finish_commit();
            let batch_op_count = batch.operations.len();
            let batch_write_start = Instant::now();
            if !batch.is_empty() {
                self.batch_writer.write(batch);
            }
            let batch_write_elapsed = batch_write_start.elapsed();

            let next_root =
                checkers.commit_all(&current_root, verb_cache, prop_cache, ancestry_cache);
            self.root_state.store(next_root);

            // Track the last write timestamp and send barrier
            self.last_write_commit
                .store(tx_timestamp.0, std::sync::atomic::Ordering::Release);
            self.batch_writer.send_barrier(tx_timestamp);

            // Warn if batch_write blocked (backpressure)
            if batch_write_elapsed > Duration::from_secs(1) {
                warn!(
                    "Slow batch_write: {} ops blocked for {:.2}s (ts {})",
                    batch_op_count,
                    batch_write_elapsed.as_secs_f32(),
                    tx_timestamp.0
                );
            }

            drop(_t);
        }

        // Queue sequence persistence to background thread
        // (Caches and relation indexes were published atomically in the root snapshot)
        // Store monotonic counter in sequence slot 15
        self.sequences[15].store(
            self.monotonic.load(std::sync::atomic::Ordering::Relaxed) as i64,
            std::sync::atomic::Ordering::Relaxed,
        );
        // Collect current sequence values and send to background writer
        let mut seq_values = [0i64; 16];
        for (i, seq) in self.sequences.iter().enumerate() {
            seq_values[i] = seq.load(std::sync::atomic::Ordering::Relaxed);
        }
        self.sequence_writer.write(seq_values);
        CommitResult::Success {
            mutations_made: has_mutations,
            timestamp: tx_timestamp.0,
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
