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
    AnonymousObjectMetadata, CommitSet, ObjAndUUIDHolder, StringHolder,
    config::DatabaseConfig,
    db_worldstate::db_counters,
    prop_cache::PropResolutionCache,
    tx_management::{
        Canonical, CheckRelation, Relation, RelationTransaction, Timestamp, Tx, WorkingSet,
    },
    verb_cache::{AncestryCache, VerbResolutionCache},
};
use crate::{
    provider::{
        Migrator,
        fjall_migration::{self, FjallMigrator},
        fjall_provider::FjallProvider,
        fjall_snapshot_loader::FjallSnapshotLoader,
    },
    relation_defs::define_relations,
};
use arc_swap::ArcSwap;
use fjall::{Database, KeyspaceCreateOptions, PersistMode};
use flume::Sender;
use gdt_cpus::{ThreadPriority, set_thread_priority};
use minstant::Instant;
use moor_common::util::CachePadded;
use moor_common::{
    model::{CommitResult, ObjFlag, PropDefs, PropPerms, VerbDefs},
    util::{BitEnum, PerfTimerGuard},
};
use moor_var::{Obj, Symbol, Var, program::ProgramType};
use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicI64, AtomicU64},
    },
    thread::JoinHandle,
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
    /// Seqlock version counter: even = stable, odd = commit in progress
    commit_version: CachePadded<AtomicU64>,
    keyspace: Database,
    relations: Relations,
    sequences: [Arc<CachePadded<AtomicI64>>; 16],
    /// Background writer for sequence persistence
    sequence_writer: crate::provider::fjall_provider::SequenceWriter,
    kill_switch: Arc<AtomicBool>,
    commit_channel: Sender<CommitSet>,
    usage_send: Sender<oneshot::Sender<usize>>,
    caches: ArcSwap<Caches>,
    /// Last write transaction timestamp that completed
    last_write_commit: AtomicU64,
    jh: Mutex<Option<JoinHandle<()>>>,
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
                .relations
                .wait_for_write_barrier(last_write_timestamp, std::time::Duration::from_secs(10))
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

    pub(crate) fn start_transaction(&self) -> WorldStateTransaction {
        let mut backoff = 0u32;
        loop {
            // Check if commit is in progress (odd version)
            let v1 = self
                .commit_version
                .load(std::sync::atomic::Ordering::Acquire);
            if v1 & 1 == 1 {
                // Commit in progress - use exponential backoff to reduce CPU waste
                // Phase 1: Spin (backoff 0-5) - tight loop for very brief waits
                // Phase 2: Yield (backoff 6-9) - give up timeslice
                // Phase 3: Sleep (backoff 10+) - sleep 10µs for longer waits
                if backoff < 6 {
                    for _ in 0..(1 << backoff) {
                        std::hint::spin_loop();
                    }
                } else if backoff < 10 {
                    std::thread::yield_now();
                } else {
                    std::thread::sleep(Duration::from_micros(10));
                }
                backoff = backoff.saturating_add(1);
                continue;
            }

            // Acquire fence ensures all relation loads see committed data
            std::sync::atomic::fence(std::sync::atomic::Ordering::Acquire);

            let tx = Tx {
                ts: Timestamp(
                    self.monotonic
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                ),
                snapshot_version: v1,
            };

            let caches = self.caches.load();
            let forked_caches = caches.fork();

            let ws_tx = self.relations.start_transaction(
                tx,
                self.commit_channel.clone(),
                self.usage_send.clone(),
                self.sequences.clone(),
                forked_caches.verb_resolution_cache,
                forked_caches.prop_resolution_cache,
                forked_caches.ancestry_cache,
            );

            // Verify version didn't change during snapshot (detect commit race)
            if self
                .commit_version
                .load(std::sync::atomic::Ordering::Acquire)
                == v1
            {
                return ws_tx;
            }
            // Retry if commit happened during snapshot
        }
    }

    pub fn stop(&self) {
        self.kill_switch
            .store(true, std::sync::atomic::Ordering::SeqCst);

        let mut jh_lock = self.jh.lock().unwrap();
        if let Some(jh) = jh_lock.take() {
            jh.join().unwrap();
        }

        // Wait for all pending write transactions to flush before stopping
        let last_write_timestamp = Timestamp(
            self.last_write_commit
                .load(std::sync::atomic::Ordering::Acquire),
        );
        if last_write_timestamp.0 > 0 {
            info!(
                "Waiting for write barrier {} before shutdown",
                last_write_timestamp.0
            );
            if let Err(e) = self
                .relations
                .wait_for_write_barrier(last_write_timestamp, std::time::Duration::from_secs(30))
            {
                error!(
                    "Timeout waiting for write barrier {} during shutdown: {}",
                    last_write_timestamp.0, e
                );
            } else {
                info!("Write barrier {} completed", last_write_timestamp.0);
            }
        }

        // Stop sequence writer first to ensure all sequences are flushed
        self.sequence_writer.stop();
        self.relations.stop_all();
        if let Err(e) = self.keyspace.persist(PersistMode::SyncAll) {
            error!("Failed to persist keyspace: {}", e);
        }
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

        let relations = Relations::init(&keyspace, &config);

        let (commit_channel, commit_receiver) = flume::unbounded();
        let (usage_send, usage_recv) = flume::unbounded();
        let kill_switch = Arc::new(AtomicBool::new(false));
        let caches = ArcSwap::new(Arc::new(Caches::new()));

        // Create background sequence writer
        let sequence_writer =
            crate::provider::fjall_provider::SequenceWriter::new(sequences_partition.clone());

        let s = Arc::new(Self {
            monotonic: CachePadded::new(AtomicU64::new(start_tx_num)),
            commit_version: CachePadded::new(AtomicU64::new(0)),
            relations,
            sequences,
            sequence_writer,
            commit_channel,
            usage_send,
            kill_switch: kill_switch.clone(),
            keyspace,
            caches,
            last_write_commit: AtomicU64::new(0),
            jh: Mutex::new(None),
        });

        s.clone()
            .start_processing_thread(commit_receiver, usage_recv, kill_switch, config);

        (s, fresh)
    }

    pub fn usage_bytes(&self) -> usize {
        self.keyspace.disk_space().unwrap_or_default() as usize
    }

    /// Mark all relations as fully loaded from their backing providers.
    /// Call this after bulk import operations to enable optimized reads.
    pub fn mark_all_fully_loaded(&self) {
        self.relations.mark_all_fully_loaded();
    }

    fn start_processing_thread(
        self: Arc<Self>,
        receiver: flume::Receiver<CommitSet>,
        usage_recv: flume::Receiver<oneshot::Sender<usize>>,
        kill_switch: Arc<AtomicBool>,
        _config: DatabaseConfig,
    ) {
        let this_weak = Arc::downgrade(&self);

        let thread_builder = std::thread::Builder::new().name("moor-db-process".to_string());
        let jh = thread_builder
            .spawn(move || {
                set_thread_priority(ThreadPriority::Highest).ok();
                loop {
                    let counters = db_counters();

                    if kill_switch.load(std::sync::atomic::Ordering::Relaxed) {
                        break;
                    }

                    // Use selector to block on both channels simultaneously
                    enum DbMessage {
                        Commit(CommitSet),
                        Usage(oneshot::Sender<usize>),
                    }

                    let selector = flume::Selector::new()
                        .recv(&receiver, |result| {
                            result.ok().map(DbMessage::Commit)
                        })
                        .recv(&usage_recv, |result| {
                            result.ok().map(DbMessage::Usage)
                        });

                    // Wait for message without holding Arc reference
                    let message = match selector.wait_timeout(Duration::from_millis(100)) {
                        Ok(Some(msg)) => msg,
                        Ok(None) | Err(_) => continue, // Timeout or disconnected - loop to check kill_switch
                    };

                    // Now upgrade to process the message - if upgrade fails, MoorDB was dropped
                    let Some(this) = this_weak.upgrade() else {
                        break;
                    };

                    let (ws, reply) = match message {
                        DbMessage::Usage(reply) => {
                            reply.send(this.usage_bytes())
                                .map_err(|e| warn!("{}", e))
                                .ok();
                            continue;
                        }
                        DbMessage::Commit(CommitSet::CommitReadOnly(combined_caches)) => {
                            if combined_caches.has_changed() {
                                this.caches.store(Arc::new(combined_caches));
                            }
                            // Read-only transactions don't need barrier tracking since we only
                            // wait for write transactions when creating snapshots
                            continue;
                        }
                        DbMessage::Commit(CommitSet::CommitWrites(ws, reply)) => {
                            // Process commit below
                            (ws, reply)
                        }
                    };

                    let _t = PerfTimerGuard::new(&counters.commit_check_phase);

                    let start_time = Instant::now();

                    let mut checkers = this.relations.begin_check_all();

                    let num_tuples = ws.total_tuples();
                    if num_tuples > 10_000 {
                        warn!("Potential large batch @ commit... Checking {num_tuples} total tuples from the working set...");
                    }

                    // Get the transaction timestamp and mutations flag before extracting working sets
                    let tx_timestamp = ws.tx.ts;
                    let snapshot_version = ws.tx.snapshot_version;
                    let has_mutations = ws.has_mutations;
                    let (relation_ws, verb_cache, prop_cache, ancestry_cache) = ws.extract_relation_working_sets();

                    // Optimization: If no commits completed since transaction start, skip conflict checking.
                    // The transaction already validated against its snapshot when creating operations.
                    // If the snapshot is still current (commit_version unchanged), those validations remain valid.
                    let current_version = this.commit_version.load(std::sync::atomic::Ordering::Acquire);
                    let skip_conflict_check = snapshot_version == current_version;

                    {
                        // Conflict validation - can skip if no concurrent commits
                        if !skip_conflict_check
                            && let Err(conflict_info) = checkers.check_all(&relation_ws)
                        {
                            warn!("Transaction conflict during commit: {}", conflict_info);
                            reply
                                .send(CommitResult::ConflictRetry {
                                    conflict_info: Some(conflict_info),
                                })
                                .ok();
                            continue;
                        }
                        drop(_t);

                        // Mutation detection - use has_mutations since we might have skipped check_all
                        // (which normally sets the dirty flags that all_clean checks)
                        if !has_mutations {
                            reply.send(CommitResult::Success {
                                mutations_made: false,
                                timestamp: tx_timestamp.0,
                            }).ok();

                            let combined_caches = Caches {
                                verb_resolution_cache: verb_cache,
                                prop_resolution_cache: prop_cache,
                                ancestry_cache,
                            };
                            if combined_caches.has_changed() {
                                this.caches.store(Arc::new(combined_caches));
                            }
                            continue;
                        }

                        // Warn if the duration of the check phase took a really long time...
                        let apply_start = Instant::now();
                        if start_time.elapsed() > Duration::from_secs(5) {
                            warn!(
                                "Long running commit; check phase took {}s for {num_tuples} tuples",
                                start_time.elapsed().as_secs_f32()
                            );
                        }

                        let _t = PerfTimerGuard::new(&counters.commit_apply_phase);

                        let checkers = match checkers.apply_all(relation_ws) {
                            Ok(checkers) => checkers,
                            Err(()) => {
                                warn!("Transaction conflict during apply phase (no detailed info available)");
                                reply
                                    .send(CommitResult::ConflictRetry { conflict_info: None })
                                    .ok();
                                continue;
                            }
                        };

                        // Use seqlock coordination to atomically swap relation indexes AND update caches.
                        // Caches must be updated INSIDE seqlock protection to prevent transactions from
                        // seeing stable seqlock with stale caches (which would cause old verb code to execute).
                        checkers.commit_all(
                            &this.relations,
                            &this.commit_version,
                            &this.caches,
                            verb_cache,
                            prop_cache,
                            ancestry_cache,
                        );

                        // Track the last write transaction timestamp for snapshot consistency
                        this.last_write_commit.store(tx_timestamp.0, std::sync::atomic::Ordering::Release);

                        // Send barrier message to providers to track this write transaction completion
                        if let Err(e) = this.relations.send_barrier(tx_timestamp) {
                            warn!("Failed to send barrier for write transaction: {}", e);
                        }

                        // No need to block the caller while we're doing the final write to disk.
                        reply.send(CommitResult::Success {
                            mutations_made: has_mutations,
                            timestamp: tx_timestamp.0,
                        }).ok();

                        // And if the commit took a long time, warn before the write to disk is begun.
                        if start_time.elapsed() > Duration::from_secs(5) {
                            warn!(
                                "Long running commit, apply phase took {}s for {num_tuples} tuples",
                                apply_start.elapsed().as_secs_f32()
                            );
                        }

                        drop(_t);
                    }

                    // Queue sequence persistence to background thread
                    // (Caches were already updated inside commit_all before seqlock was marked stable)
                    // Store monotonic counter in sequence slot 15
                    this.sequences[15].store(
                        this.monotonic.load(std::sync::atomic::Ordering::Relaxed) as i64,
                        std::sync::atomic::Ordering::Relaxed,
                    );
                    // Collect current sequence values and send to background writer
                    let mut seq_values = [0i64; 16];
                    for (i, seq) in this.sequences.iter().enumerate() {
                        seq_values[i] = seq.load(std::sync::atomic::Ordering::Relaxed);
                    }
                    this.sequence_writer.write(seq_values);
                }
            })
            .expect("failed to start DB processing thread");

        let mut jh_lock = self.jh.lock().unwrap();
        *jh_lock = Some(jh);
    }
}

impl Drop for MoorDB {
    fn drop(&mut self) {
        info!("MoorDB::drop() called - initiating shutdown");
        self.stop();
        info!("MoorDB shutdown complete");
    }
}
