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

use crate::config::DatabaseConfig;
use crate::db_worldstate::db_counters;
use crate::fjall_provider::FjallProvider;
use crate::prop_cache::PropResolutionCache;
use crate::tx_management::{SizedCache, Timestamp, TransactionalCache, Tx, WorkingSet};
use crate::verb_cache::VerbResolutionCache;
use crate::ws_transaction::WorldStateTransaction;
use crate::{BytesHolder, CommitSet, ObjAndUUIDHolder, StringHolder};
use crossbeam_channel::Sender;
use fjall::{Config, PartitionCreateOptions, PartitionHandle, PersistMode};
use moor_common::model::{CommitResult, ObjFlag, ObjSet, PropDefs, PropPerms, VerbDefs};
use moor_common::util::{BitEnum, PerfTimerGuard};
use moor_var::{Obj, SYSTEM_OBJECT, Symbol, Var};
use std::ops::Deref;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tracing::{error, warn};

pub struct MoorDB {
    monotonic: AtomicU64,

    keyspace: fjall::Keyspace,

    object_location: GC<Obj, Obj>,
    object_contents: GC<Obj, ObjSet>,
    object_flags: GC<Obj, BitEnum<ObjFlag>>,
    object_parent: GC<Obj, Obj>,
    object_children: GC<Obj, ObjSet>,
    object_owner: GC<Obj, Obj>,
    object_name: GC<Obj, StringHolder>,

    object_verbdefs: GC<Obj, VerbDefs>,
    object_verbs: GC<ObjAndUUIDHolder, BytesHolder>,
    object_propdefs: GC<Obj, PropDefs>,
    object_propvalues: GC<ObjAndUUIDHolder, Var>,
    object_propflags: GC<ObjAndUUIDHolder, PropPerms>,

    sequences: [Arc<AtomicI64>; 16],
    sequences_partition: PartitionHandle,

    kill_switch: Arc<AtomicBool>,
    commit_channel: Sender<CommitSet>,
    usage_send: crossbeam_channel::Sender<oneshot::Sender<usize>>,

    verb_resolution_cache: RwLock<VerbResolutionCache>,
    prop_resolution_cache: RwLock<PropResolutionCache>,
}

type GC<Domain, Codomain> =
    Arc<TransactionalCache<Domain, Codomain, FjallProvider<Domain, Codomain>>>;

pub(crate) struct WorkingSets {
    #[allow(dead_code)]
    pub(crate) tx: Tx,
    pub(crate) object_location: WorkingSet<Obj, Obj>,
    pub(crate) object_contents: WorkingSet<Obj, ObjSet>,
    pub(crate) object_flags: WorkingSet<Obj, BitEnum<ObjFlag>>,
    pub(crate) object_parent: WorkingSet<Obj, Obj>,
    pub(crate) object_children: WorkingSet<Obj, ObjSet>,
    pub(crate) object_owner: WorkingSet<Obj, Obj>,
    pub(crate) object_name: WorkingSet<Obj, StringHolder>,
    pub(crate) object_verbdefs: WorkingSet<Obj, VerbDefs>,
    pub(crate) object_verbs: WorkingSet<ObjAndUUIDHolder, BytesHolder>,
    pub(crate) object_propdefs: WorkingSet<Obj, PropDefs>,
    pub(crate) object_propvalues: WorkingSet<ObjAndUUIDHolder, Var>,
    pub(crate) object_propflags: WorkingSet<ObjAndUUIDHolder, PropPerms>,
    pub(crate) verb_resolution_cache: VerbResolutionCache,
    pub(crate) prop_resolution_cache: PropResolutionCache,
}

impl WorkingSets {
    pub fn total_tuples(&self) -> usize {
        self.object_location.len()
            + self.object_contents.len()
            + self.object_flags.len()
            + self.object_parent.len()
            + self.object_children.len()
            + self.object_owner.len()
            + self.object_name.len()
            + self.object_verbdefs.len()
            + self.object_verbs.len()
            + self.object_propdefs.len()
            + self.object_propvalues.len()
            + self.object_propflags.len()
    }
}

impl MoorDB {
    pub fn open(path: Option<&Path>, config: DatabaseConfig) -> (Arc<Self>, bool) {
        let tmpdir = if path.is_none() {
            Some(TempDir::new().unwrap())
        } else {
            None
        };
        // Open the fjall db and then get all the partition handles.
        let path = path.unwrap_or_else(|| tmpdir.as_ref().unwrap().path());
        let keyspace = Config::new(path).open().unwrap();

        let sequences_partition = keyspace
            .open_partition("sequences", PartitionCreateOptions::default())
            .unwrap();

        let sequences = [(); 16].map(|_| Arc::new(AtomicI64::new(-1)));

        let mut fresh = false;
        if !keyspace.partition_exists("object_location") {
            fresh = true;
        }

        if !fresh {
            // Fill sequences from the sequences partition.
            for (i, seq) in sequences.iter().enumerate() {
                let seq_value = sequences_partition
                    .get(i.to_le_bytes())
                    .unwrap()
                    .map(|b| i64::from_le_bytes(b[0..8].try_into().unwrap()))
                    .unwrap_or(-1);
                seq.store(seq_value, std::sync::atomic::Ordering::SeqCst);
            }
        }

        // 16th sequence is the monotonic transaction number.
        let start_tx_num = sequences_partition
            .get(15_u64.to_le_bytes())
            .unwrap()
            .map(|b| u64::from_le_bytes(b[0..8].try_into().unwrap()))
            .unwrap_or(1);

        let object_location = keyspace
            .open_partition(
                "object_location",
                config.object_location.partition_options(),
            )
            .unwrap();
        let object_contents = keyspace
            .open_partition(
                "object_contents",
                config.object_contents.partition_options(),
            )
            .unwrap();
        let object_flags = keyspace
            .open_partition("object_flags", config.object_flags.partition_options())
            .unwrap();
        let object_parent = keyspace
            .open_partition("object_parent", config.object_parent.partition_options())
            .unwrap();
        let object_children = keyspace
            .open_partition(
                "object_children",
                config.object_children.partition_options(),
            )
            .unwrap();
        let object_owner = keyspace
            .open_partition("object_owner", config.object_owner.partition_options())
            .unwrap();
        let object_name = keyspace
            .open_partition("object_name", config.object_name.partition_options())
            .unwrap();
        let object_verbdefs = keyspace
            .open_partition(
                "object_verbdefs",
                config.object_verbdefs.partition_options(),
            )
            .unwrap();
        let object_verbs = keyspace
            .open_partition("object_verbs", config.object_verbs.partition_options())
            .unwrap();
        let object_propdefs = keyspace
            .open_partition(
                "object_propdefs",
                config.object_propdefs.partition_options(),
            )
            .unwrap();
        let object_propvalues = keyspace
            .open_partition(
                "object_propvalues",
                config.object_propvalues.partition_options(),
            )
            .unwrap();
        let object_propflags = keyspace
            .open_partition(
                "object_propflags",
                config.object_propflags.partition_options(),
            )
            .unwrap();

        let object_location = FjallProvider::new(object_location);
        let object_contents = FjallProvider::new(object_contents);
        let object_flags = FjallProvider::new(object_flags);
        let object_parent = FjallProvider::new(object_parent);
        let object_children = FjallProvider::new(object_children);
        let object_owner = FjallProvider::new(object_owner);
        let object_name = FjallProvider::new(object_name);
        let object_verbdefs = FjallProvider::new(object_verbdefs);
        let object_verbs = FjallProvider::new(object_verbs);
        let object_propdefs = FjallProvider::new(object_propdefs);
        let object_propvalues = FjallProvider::new(object_propvalues);
        let object_propflags = FjallProvider::new(object_propflags);

        let preseed_objects = vec![SYSTEM_OBJECT, Obj::mk_id(1)];

        let default_cache_eviction_threshold = config.default_eviction_threshold;
        let object_location = Arc::new(TransactionalCache::new(
            Symbol::mk("object_location"),
            Arc::new(object_location),
            config
                .object_location
                .cache_eviction_threshold
                .unwrap_or(default_cache_eviction_threshold),
            &preseed_objects,
        ));
        let object_contents = Arc::new(TransactionalCache::new(
            Symbol::mk("object_contents"),
            Arc::new(object_contents),
            config
                .object_contents
                .cache_eviction_threshold
                .unwrap_or(default_cache_eviction_threshold),
            &preseed_objects,
        ));
        let object_flags = Arc::new(TransactionalCache::new(
            Symbol::mk("object_flags"),
            Arc::new(object_flags),
            config
                .object_flags
                .cache_eviction_threshold
                .unwrap_or(default_cache_eviction_threshold),
            &preseed_objects,
        ));
        let object_parent = Arc::new(TransactionalCache::new(
            Symbol::mk("object_parent"),
            Arc::new(object_parent),
            config
                .object_parent
                .cache_eviction_threshold
                .unwrap_or(default_cache_eviction_threshold),
            &preseed_objects,
        ));
        let object_children = Arc::new(TransactionalCache::new(
            Symbol::mk("object_children"),
            Arc::new(object_children),
            config
                .object_children
                .cache_eviction_threshold
                .unwrap_or(default_cache_eviction_threshold),
            &preseed_objects,
        ));
        let object_owner = Arc::new(TransactionalCache::new(
            Symbol::mk("object_owner"),
            Arc::new(object_owner),
            config
                .object_owner
                .cache_eviction_threshold
                .unwrap_or(default_cache_eviction_threshold),
            &preseed_objects,
        ));
        let object_name = Arc::new(TransactionalCache::new(
            Symbol::mk("object_name"),
            Arc::new(object_name),
            config
                .object_name
                .cache_eviction_threshold
                .unwrap_or(default_cache_eviction_threshold),
            &preseed_objects,
        ));
        let object_verbdefs = Arc::new(TransactionalCache::new(
            Symbol::mk("object_verbdefs"),
            Arc::new(object_verbdefs),
            config
                .object_verbdefs
                .cache_eviction_threshold
                .unwrap_or(default_cache_eviction_threshold),
            &preseed_objects,
        ));
        let object_verbs = Arc::new(TransactionalCache::new(
            Symbol::mk("object_verbs"),
            Arc::new(object_verbs),
            config
                .object_verbs
                .cache_eviction_threshold
                .unwrap_or(default_cache_eviction_threshold),
            &[],
        ));
        let object_propdefs = Arc::new(TransactionalCache::new(
            Symbol::mk("object_propdefs"),
            Arc::new(object_propdefs),
            config
                .object_propdefs
                .cache_eviction_threshold
                .unwrap_or(default_cache_eviction_threshold),
            &preseed_objects,
        ));
        let object_propvalues = Arc::new(TransactionalCache::new(
            Symbol::mk("object_propvalues"),
            Arc::new(object_propvalues),
            config
                .object_propvalues
                .cache_eviction_threshold
                .unwrap_or(default_cache_eviction_threshold),
            &[],
        ));
        let object_propflags = Arc::new(TransactionalCache::new(
            Symbol::mk("object_propflags"),
            Arc::new(object_propflags),
            config
                .object_propflags
                .cache_eviction_threshold
                .unwrap_or(default_cache_eviction_threshold),
            &[],
        ));

        let (commit_channel, commit_receiver) = crossbeam_channel::unbounded();
        let (usage_send, usage_recv) = crossbeam_channel::unbounded();
        let kill_switch = Arc::new(AtomicBool::new(false));
        let verb_resolution_cache = RwLock::new(VerbResolutionCache::new());
        let prop_resolution_cache = RwLock::new(PropResolutionCache::new());

        let s = Arc::new(Self {
            monotonic: AtomicU64::new(start_tx_num),
            object_location,
            object_contents,
            object_flags,
            object_parent,
            object_children,
            object_owner,
            object_name,
            object_verbdefs,
            object_verbs,
            object_propdefs,
            object_propvalues,
            object_propflags,
            sequences,
            sequences_partition,
            commit_channel,
            usage_send,
            kill_switch: kill_switch.clone(),
            keyspace,
            verb_resolution_cache,
            prop_resolution_cache,
        });

        s.clone()
            .start_processing_thread(commit_receiver, usage_recv, kill_switch, config);

        (s, fresh)
    }

    pub(crate) fn start_transaction(&self) -> WorldStateTransaction {
        let tx = Tx {
            ts: Timestamp(
                self.monotonic
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst),
            ),
        };

        let vc_lock = self.verb_resolution_cache.read().unwrap();
        let verb_resolution_cache = vc_lock.fork();

        let prop_lock = self.prop_resolution_cache.read().unwrap();
        let prop_resolution_cache = prop_lock.fork();
        WorldStateTransaction {
            tx,
            commit_channel: self.commit_channel.clone(),
            usage_channel: self.usage_send.clone(),
            object_location: self.object_location.clone().start(&tx),
            object_contents: self.object_contents.clone().start(&tx),
            object_flags: self.object_flags.clone().start(&tx),
            object_parent: self.object_parent.clone().start(&tx),
            object_children: self.object_children.clone().start(&tx),
            object_owner: self.object_owner.clone().start(&tx),
            object_name: self.object_name.clone().start(&tx),
            object_verbdefs: self.object_verbdefs.clone().start(&tx),
            object_verbs: self.object_verbs.clone().start(&tx),
            object_propdefs: self.object_propdefs.clone().start(&tx),
            object_propvalues: self.object_propvalues.clone().start(&tx),
            object_propflags: self.object_propflags.clone().start(&tx),
            sequences: self.sequences.clone(),
            verb_resolution_cache,
            prop_resolution_cache,
            ancestry_cache: Default::default(),
            has_mutations: false,
        }
    }

    fn caches(&self) -> Vec<&dyn SizedCache> {
        vec![
            self.object_location.deref(),
            self.object_contents.deref(),
            self.object_flags.deref(),
            self.object_parent.deref(),
            self.object_children.deref(),
            self.object_owner.deref(),
            self.object_name.deref(),
            self.object_verbdefs.deref(),
            self.object_verbs.deref(),
            self.object_propdefs.deref(),
            self.object_propvalues.deref(),
            self.object_propflags.deref(),
        ]
    }

    pub fn usage_bytes(&self) -> usize {
        self.keyspace.disk_space() as usize
    }

    /// Provide a rough estimate of memory usage in bytes.
    #[allow(dead_code)]
    pub fn cache_usage_bytes(&self) -> usize {
        self.caches()
            .iter()
            .map(|c| c.cache_usage_bytes())
            .sum::<usize>()
    }

    #[allow(dead_code)]
    pub fn process_cache_evictions(&self) -> (usize, usize) {
        let (mut total_before, mut total_after) = (0, 0);
        for c in self.caches().iter() {
            let (before, after) = c.process_cache_evictions();
            total_before += before;
            total_after += after;
        }
        (total_before, total_after)
    }

    pub fn stop(&self) {
        self.kill_switch
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    fn start_processing_thread(
        self: Arc<Self>,
        receiver: crossbeam_channel::Receiver<CommitSet>,
        usage_recv: crossbeam_channel::Receiver<oneshot::Sender<usize>>,
        kill_switch: Arc<AtomicBool>,
        config: DatabaseConfig,
    ) {
        let this = self.clone();

        let thread_builder = std::thread::Builder::new().name("moor-db-process".to_string());
        thread_builder
            .spawn(move || {
                let mut last_eviction_check = std::time::Instant::now();
                loop {
                    let counters = db_counters();

                    if kill_switch.load(std::sync::atomic::Ordering::SeqCst) {
                        break;
                    }

                    if let Ok(msg) = usage_recv.try_recv() {
                        msg.send(this.usage_bytes())
                            .map_err(|e| warn!("{}", e))
                            .ok();
                    }

                    // If eviction processing interval has passed, check for evictions.
                    if last_eviction_check.elapsed() > config.cache_eviction_interval {
                        let mut total_evicted_entries = 0;
                        let mut total_evicted_bytes = 0;
                        for cache in this.caches() {
                            cache.select_victims();
                            let (evicted_entries, evicted_bytes) = cache.process_cache_evictions();
                            total_evicted_entries += evicted_entries;
                            total_evicted_bytes += evicted_bytes;
                        }

                        if total_evicted_entries > 0 {
                            warn!(
                                "Evicted {} entries, freeing {} bytes",
                                total_evicted_entries, total_evicted_bytes
                            );
                        }

                        last_eviction_check = std::time::Instant::now();
                    }

                    let msg = receiver.recv_timeout(Duration::from_millis(100));
                    let (ws, reply) = match msg {
                        Ok(CommitSet::CommitWrites(ws, reply)) => {
                            (ws, reply)
                        }
                        Ok(CommitSet::CommitReadOnly(vc, pc)) => {
                            let mut vc_lock = this.verb_resolution_cache.write().unwrap();
                            *vc_lock = vc;
                            let mut pc_lock = this.prop_resolution_cache.write().unwrap();
                            *pc_lock = pc;
                            continue;
                        }
                        Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                            continue;
                        }
                        Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                            break;
                        }
                    };

                    let _t = PerfTimerGuard::new(&counters.commit_check_phase);

                    let start_time = Instant::now();

                    let object_flags = this.object_flags.write_lock();
                    let object_parent = this.object_parent.write_lock();
                    let object_children = this.object_children.write_lock();
                    let object_owner = this.object_owner.write_lock();
                    let object_location = this.object_location.write_lock();
                    let object_contents = this.object_contents.write_lock();
                    let object_name = this.object_name.write_lock();
                    let object_verbdefs = this.object_verbdefs.write_lock();
                    let object_verbs = this.object_verbs.write_lock();
                    let object_propdefs = this.object_propdefs.write_lock();
                    let object_propvalues = this.object_propvalues.write_lock();
                    let object_propflags = this.object_propflags.write_lock();

                    let num_tuples = ws.object_flags.len()
                        + ws.object_parent.len()
                        + ws.object_children.len()
                        + ws.object_owner.len()
                        + ws.object_location.len()
                        + ws.object_contents.len()
                        + ws.object_name.len()
                        + ws.object_verbdefs.len()
                        + ws.object_verbs.len()
                        + ws.object_propdefs.len()
                        + ws.object_propvalues.len()
                        + ws.object_propflags.len();

                    if num_tuples > 10_000 {
                        warn!("Potential large batch @ commit... Checking {num_tuples} total tuples from the working set...");
                    }

                    {
                        let Ok(ol_lock) = this.object_flags.check(object_flags, &ws.object_flags)
                        else {
                            reply.send(CommitResult::ConflictRetry).ok();

                            continue;
                        };

                        let Ok(op_lock) = this.object_parent.check(object_parent, &ws.object_parent)
                        else {
                            reply.send(CommitResult::ConflictRetry).ok();

                            continue;
                        };

                        let Ok(oc_lock) = this
                            .object_children
                            .check(object_children, &ws.object_children)
                        else {
                            reply.send(CommitResult::ConflictRetry).ok();
                            continue;
                        };

                        let Ok(oo_lock) = this.object_owner.check(object_owner, &ws.object_owner)
                        else {
                            reply.send(CommitResult::ConflictRetry).ok();
                            continue;
                        };

                        let Ok(oloc_lock) = this
                            .object_location
                            .check(object_location, &ws.object_location)
                        else {
                            reply.send(CommitResult::ConflictRetry).ok();
                            continue;
                        };

                        let Ok(ocont_lock) = this
                            .object_contents
                            .check(object_contents, &ws.object_contents)
                        else {
                            reply.send(CommitResult::ConflictRetry).ok();
                            continue;
                        };

                        let Ok(on_lock) = this.object_name.check(object_name, &ws.object_name) else {
                            reply.send(CommitResult::ConflictRetry).ok();
                            continue;
                        };

                        let Ok(ovd_lock) = this
                            .object_verbdefs
                            .check(object_verbdefs, &ws.object_verbdefs)
                        else {
                            reply.send(CommitResult::ConflictRetry).ok();
                            continue;
                        };

                        let Ok(ov_lock) = this.object_verbs.check(object_verbs, &ws.object_verbs)
                        else {
                            reply.send(CommitResult::ConflictRetry).ok();
                            continue;
                        };

                        let Ok(opd_lock) = this
                            .object_propdefs
                            .check(object_propdefs, &ws.object_propdefs)
                        else {
                            reply.send(CommitResult::ConflictRetry).ok();
                            continue;
                        };

                        let Ok(opv_lock) = this
                            .object_propvalues
                            .check(object_propvalues, &ws.object_propvalues)
                        else {
                            reply.send(CommitResult::ConflictRetry).ok();
                            continue;
                        };

                        let Ok(opf_lock) = this
                            .object_propflags
                            .check(object_propflags, &ws.object_propflags)
                        else {
                            reply.send(CommitResult::ConflictRetry).ok();
                            continue;
                        };

                        drop(_t);

                        let _t = PerfTimerGuard::new(&counters.commit_apply_phase);

                        // Warn if the duration of the check phase took a really long time...
                        let apply_start = Instant::now();
                        if start_time.elapsed() > Duration::from_secs(5) {
                            warn!(
                                "Long running commit; check phase took {}s for {num_tuples} tuples",
                                start_time.elapsed().as_secs_f32()
                            );
                        }

                        // Apply phase
                        let Ok(_unused) = this.object_flags.apply(ol_lock, ws.object_flags) else {
                            reply.send(CommitResult::ConflictRetry).ok();
                            continue;
                        };

                        let Ok(_unused) = this.object_parent.apply(op_lock, ws.object_parent) else {
                            reply.send(CommitResult::ConflictRetry).ok();
                            continue;
                        };

                        let Ok(_unused) = this.object_children.apply(oc_lock, ws.object_children)
                        else {
                            reply.send(CommitResult::ConflictRetry).ok();
                            continue;
                        };

                        let Ok(_unused) = this.object_owner.apply(oo_lock, ws.object_owner) else {
                            reply.send(CommitResult::ConflictRetry).ok();
                            continue;
                        };

                        let Ok(_unused) = this.object_location.apply(oloc_lock, ws.object_location)
                        else {
                            reply.send(CommitResult::ConflictRetry).ok();
                            continue;
                        };

                        let Ok(_unused) = this.object_contents.apply(ocont_lock, ws.object_contents)
                        else {
                            reply.send(CommitResult::ConflictRetry).ok();
                            continue;
                        };

                        let Ok(_unused) = this.object_name.apply(on_lock, ws.object_name) else {
                            reply.send(CommitResult::ConflictRetry).ok();
                            continue;
                        };

                        let Ok(_unused) = this.object_verbdefs.apply(ovd_lock, ws.object_verbdefs)
                        else {
                            reply.send(CommitResult::ConflictRetry).ok();
                            continue;
                        };

                        let Ok(_unused) = this.object_verbs.apply(ov_lock, ws.object_verbs) else {
                            reply.send(CommitResult::ConflictRetry).ok();
                            continue;
                        };

                        let Ok(_unused) = this.object_propdefs.apply(opd_lock, ws.object_propdefs)
                        else {
                            reply.send(CommitResult::ConflictRetry).ok();
                            continue;
                        };

                        let Ok(_unused) = this.object_propvalues.apply(opv_lock, ws.object_propvalues)
                        else {
                            reply.send(CommitResult::ConflictRetry).ok();
                            continue;
                        };

                        let Ok(_unused) = this.object_propflags.apply(opf_lock, ws.object_propflags)
                        else {
                            reply.send(CommitResult::ConflictRetry).ok();
                            continue;
                        };

                        // And if the commit took a long time, warn before the write to disk is begun.
                        if start_time.elapsed() > Duration::from_secs(5) {
                            warn!(
                                "Long running commit, apply phase took {}s for {num_tuples} tuples",
                                apply_start.elapsed().as_secs_f32()
                            );
                        }

                        drop(_t);
                    }
                    // No need to block the caller while we're doing the final write to disk.
                    reply.send(CommitResult::Success).ok();

                    // All locks now dropped, now we can do the write to disk, swap in the (maybe)
                    // updated verb resolution cache update sequences, and move on.
                    // NOTE: hopefully this all happens before the next commit comes in, otherwise
                    //  we can end up backlogged here.

                    // Swap the commit set's cache with the main cache.
                    {
                        let mut vc_lock = this.verb_resolution_cache.write().unwrap();
                        *vc_lock = ws.verb_resolution_cache;
                        let mut pc_lock = this.prop_resolution_cache.write().unwrap();
                        *pc_lock = ws.prop_resolution_cache;
                    }

                    let _t = PerfTimerGuard::new(&counters.commit_write_phase);
                    // Now write out the current state of the sequences to the seq partition.
                    // Start by making sure that the monotonic sequence is written out.
                    self.sequences[15].store(
                        self.monotonic.load(std::sync::atomic::Ordering::SeqCst) as i64,
                        std::sync::atomic::Ordering::Relaxed,
                    );
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

                    let write_start = Instant::now();
                    self.keyspace
                        .persist(PersistMode::SyncAll)
                        .unwrap_or_else(|e| {
                            error!("Failed to persist DB state to disk: {}", e);
                        });

                    if start_time.elapsed() > Duration::from_secs(5) {
                        warn!(
                            "Long running commit, write phase took {}s; total commit time {}s for {num_tuples} tuples",
                            write_start.elapsed().as_secs_f32(),
                            start_time.elapsed().as_secs_f32()
                        );
                    }
                }
            })
            .expect("failed to start DB processing thread");
    }
}

impl Drop for MoorDB {
    fn drop(&mut self) {
        self.stop();
    }
}
