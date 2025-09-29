// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
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
    fjall_provider::FjallProvider,
    prop_cache::PropResolutionCache,
    tx_management::{
        Canonical, CheckRelation, Relation, RelationTransaction, Timestamp, Tx, WorkingSet,
    },
    verb_cache::{AncestryCache, VerbResolutionCache},
};
use arc_swap::ArcSwap;
use fjall::{Config, PartitionCreateOptions, PartitionHandle, PersistMode};
use flume::Sender;
use gdt_cpus::{ThreadPriority, set_thread_priority};
use minstant::Instant;
use moor_common::{
    model::{CommitResult, ObjFlag, PropDefs, PropPerms, VerbDefs},
    util::{BitEnum, PerfTimerGuard},
};
use moor_var::{Obj, Symbol, Var, program::ProgramType};
use std::{
    path::Path,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, AtomicI64},
    },
    thread::JoinHandle,
    time::Duration,
};
use tempfile::TempDir;
use tracing::{error, warn};

use crate::{relation_defs::define_relations, snapshot_loader::SnapshotLoader, utils::CachePadded};

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
    keyspace: fjall::Keyspace,
    relations: Relations,
    sequences: [Arc<CachePadded<AtomicI64>>; 16],
    sequences_partition: PartitionHandle,
    kill_switch: Arc<AtomicBool>,
    commit_channel: Sender<CommitSet>,
    usage_send: Sender<oneshot::Sender<usize>>,
    caches: ArcSwap<Caches>,
    /// Last write transaction timestamp that completed
    last_write_commit: RwLock<Timestamp>,
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
        let last_write_timestamp = *self.last_write_commit.read().unwrap();
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

        // Get a consistent instant from the keyspace
        let instant = self.keyspace.instant();

        // Create snapshots of each relation partition
        let object_location_snapshot = self
            .relations
            .object_location
            .source()
            .partition()
            .snapshot_at(instant);
        let object_flags_snapshot = self
            .relations
            .object_flags
            .source()
            .partition()
            .snapshot_at(instant);
        let object_parent_snapshot = self
            .relations
            .object_parent
            .source()
            .partition()
            .snapshot_at(instant);
        let object_owner_snapshot = self
            .relations
            .object_owner
            .source()
            .partition()
            .snapshot_at(instant);
        let object_name_snapshot = self
            .relations
            .object_name
            .source()
            .partition()
            .snapshot_at(instant);
        let object_verbdefs_snapshot = self
            .relations
            .object_verbdefs
            .source()
            .partition()
            .snapshot_at(instant);
        let object_verbs_snapshot = self
            .relations
            .object_verbs
            .source()
            .partition()
            .snapshot_at(instant);
        let object_propdefs_snapshot = self
            .relations
            .object_propdefs
            .source()
            .partition()
            .snapshot_at(instant);
        let object_propvalues_snapshot = self
            .relations
            .object_propvalues
            .source()
            .partition()
            .snapshot_at(instant);
        let object_propflags_snapshot = self
            .relations
            .object_propflags
            .source()
            .partition()
            .snapshot_at(instant);
        let anonymous_object_metadata_snapshot = self
            .relations
            .anonymous_object_metadata
            .source()
            .partition()
            .snapshot_at(instant);

        // Create snapshot of sequences partition
        let sequences_snapshot = self.sequences_partition.snapshot_at(instant);

        // Return a custom SnapshotInterface implementation that uses these snapshots
        Ok(Box::new(SnapshotLoader {
            object_location_snapshot,
            object_flags_snapshot,
            object_parent_snapshot,
            object_owner_snapshot,
            object_name_snapshot,
            object_verbdefs_snapshot,
            object_verbs_snapshot,
            object_propdefs_snapshot,
            object_propvalues_snapshot,
            object_propflags_snapshot,
            anonymous_object_metadata_snapshot,
            sequences_snapshot,
        }))
    }

    pub(crate) fn start_transaction(&self) -> WorldStateTransaction {
        let tx = Tx {
            ts: Timestamp::new(),
        };

        let caches = self.caches.load();
        let forked_caches = caches.fork();

        self.relations.start_transaction(
            tx,
            self.commit_channel.clone(),
            self.usage_send.clone(),
            self.sequences.clone(),
            forked_caches.verb_resolution_cache,
            forked_caches.prop_resolution_cache,
            forked_caches.ancestry_cache,
        )
    }

    pub fn stop(&self) {
        self.kill_switch
            .store(true, std::sync::atomic::Ordering::SeqCst);

        let mut jh_lock = self.jh.lock().unwrap();
        if let Some(jh) = jh_lock.take() {
            jh.join().unwrap();
        }

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
        let keyspace = Config::new(path).open().unwrap();

        let sequences_partition = keyspace
            .open_partition("sequences", PartitionCreateOptions::default())
            .unwrap();

        let sequences = [(); 16].map(|_| Arc::new(CachePadded::new(AtomicI64::new(-1))));

        let mut fresh = false;
        if !keyspace.partition_exists("object_location") {
            fresh = true;
        }

        if !fresh {
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
        let s = Arc::new(Self {
            relations,
            sequences,
            sequences_partition,
            commit_channel,
            usage_send,
            kill_switch: kill_switch.clone(),
            keyspace,
            caches,
            last_write_commit: RwLock::new(Timestamp(0)),
            jh: Mutex::new(None),
        });

        s.clone()
            .start_processing_thread(commit_receiver, usage_recv, kill_switch, config);

        (s, fresh)
    }

    pub fn usage_bytes(&self) -> usize {
        self.keyspace.disk_space() as usize
    }

    fn start_processing_thread(
        self: Arc<Self>,
        receiver: flume::Receiver<CommitSet>,
        usage_recv: flume::Receiver<oneshot::Sender<usize>>,
        kill_switch: Arc<AtomicBool>,
        _config: DatabaseConfig,
    ) {
        let this = self.clone();

        let thread_builder = std::thread::Builder::new().name("moor-db-process".to_string());
        let jh = thread_builder
            .spawn(move || {
                set_thread_priority(ThreadPriority::Highest).ok();
                loop {
                    let counters = db_counters();

                    if kill_switch.load(std::sync::atomic::Ordering::Relaxed) {
                        break;
                    }

                    if let Ok(msg) = usage_recv.try_recv() {
                        msg.send(this.usage_bytes())
                            .map_err(|e| warn!("{}", e))
                            .ok();
                    }

                    let msg = receiver.recv_timeout(Duration::from_millis(100));
                    let (ws, reply) = match msg {
                        Ok(CommitSet::CommitWrites(ws, reply)) => {
                            (ws, reply)
                        }
                        Ok(CommitSet::CommitReadOnly(combined_caches)) => {
                            if combined_caches.has_changed() {
                                this.caches.store(Arc::new(combined_caches));
                            }
                            // Read-only transactions don't need barrier tracking since we only
                            // wait for write transactions when creating snapshots
                            continue;
                        }
                        Err(flume::RecvTimeoutError::Timeout) => {
                            continue;
                        }
                        Err(flume::RecvTimeoutError::Disconnected) => {
                            break;
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
                    let has_mutations = ws.has_mutations;
                    let (relation_ws, verb_cache, prop_cache, ancestry_cache) = ws.extract_relation_working_sets();
                    {
                        if !checkers.check_all(&relation_ws) {
                            reply.send(CommitResult::ConflictRetry).ok();
                            continue;
                        }
                        drop(_t);

                        if checkers.all_clean() {
                            reply.send(CommitResult::Success {
                                mutations_made: false,
                                timestamp: tx_timestamp.0 as u64, // Convert u128 to u64 for external API
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
                                reply.send(CommitResult::ConflictRetry).ok();
                                continue;
                            }
                        };

                        // Now take write-lock on all relations just for the very instant that we swap em out.
                        // This will hold up new transactions starting, unfortunately.
                        // TODO: this is the major source of low throughput in benchmarking
                        checkers.commit_all(&this.relations);

                        // Track the last write transaction timestamp for snapshot consistency
                        *this.last_write_commit.write().unwrap() = tx_timestamp;

                        // Send barrier message to providers to track this write transaction completion
                        if let Err(e) = this.relations.send_barrier(tx_timestamp) {
                            warn!("Failed to send barrier for write transaction: {}", e);
                        }

                        // No need to block the caller while we're doing the final write to disk.
                        reply.send(CommitResult::Success {
                            mutations_made: has_mutations,
                            timestamp: tx_timestamp.0 as u64, // Convert u128 to u64 for external API
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


                    // All locks now dropped, now we can do the write to disk, swap in the (maybe)
                    // updated verb resolution cache update sequences, and move on.
                    // NOTE: hopefully this all happens before the next commit comes in, otherwise
                    //  we can end up backlogged here.

                    // Swap the commit set's cache with the main cache.
                    {
                        let combined_caches = Caches {
                            verb_resolution_cache: verb_cache,
                            prop_resolution_cache: prop_cache,
                            ancestry_cache,
                        };
                        if combined_caches.has_changed() {
                            this.caches.store(Arc::new(combined_caches));
                        }
                    }

                    let _t = PerfTimerGuard::new(&counters.commit_write_phase);

                    // Now write out the current state of the sequences to the seq partition.
                    for (i, seq) in this.sequences.iter().enumerate() {
                        this.sequences_partition
                            .insert(
                                i.to_le_bytes(),
                                seq.load(std::sync::atomic::Ordering::SeqCst).to_le_bytes(),
                            )
                            .unwrap_or_else(|e| {
                                error!("Failed to persist sequence {}: {}", i, e);
                            });
                    }
                }
            })
            .expect("failed to start DB processing thread");

        let mut jh_lock = self.jh.lock().unwrap();
        *jh_lock = Some(jh);
    }
}

impl Drop for MoorDB {
    fn drop(&mut self) {
        self.stop();
    }
}
