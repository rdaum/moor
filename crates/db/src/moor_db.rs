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
use crate::tx_management::{Relation, Timestamp, Tx, WorkingSet};
use crate::verb_cache::{AncestryCache, VerbResolutionCache};
use crate::ws_transaction::WorldStateTransaction;
use crate::{CommitSet, ObjAndUUIDHolder, StringHolder};
use arc_swap::ArcSwap;
use fjall::{Config, PartitionCreateOptions, PartitionHandle, PersistMode};
use flume::Sender;
use gdt_cpus::{ThreadPriority, set_thread_priority};
use minstant::Instant;
use moor_common::model::{CommitResult, ObjFlag, ObjSet, PropDefs, PropPerms, VerbDefs};
use moor_common::program::ProgramType;
use moor_common::util::{BitEnum, PerfTimerGuard};
use moor_var::{Obj, Symbol, Var};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;
use tempfile::TempDir;
use tracing::{error, warn};

pub struct MoorDB {
    monotonic: CachePadded<AtomicU64>,

    keyspace: fjall::Keyspace,

    object_location: R<Obj, Obj>,
    object_contents: R<Obj, ObjSet>,
    object_flags: R<Obj, BitEnum<ObjFlag>>,
    object_parent: R<Obj, Obj>,
    object_children: R<Obj, ObjSet>,
    object_owner: R<Obj, Obj>,
    object_name: R<Obj, StringHolder>,

    object_verbdefs: R<Obj, VerbDefs>,
    object_verbs: R<ObjAndUUIDHolder, ProgramType>,
    object_propdefs: R<Obj, PropDefs>,
    object_propvalues: R<ObjAndUUIDHolder, Var>,
    object_propflags: R<ObjAndUUIDHolder, PropPerms>,

    sequences: [Arc<CachePadded<AtomicI64>>; 16],
    sequences_partition: PartitionHandle,

    kill_switch: Arc<AtomicBool>,
    commit_channel: Sender<CommitSet>,
    usage_send: Sender<oneshot::Sender<usize>>,

    verb_resolution_cache: ArcSwap<VerbResolutionCache>,
    prop_resolution_cache: ArcSwap<PropResolutionCache>,
    ancestry_cache: ArcSwap<AncestryCache>,

    jh: Mutex<Option<JoinHandle<()>>>,
}

type R<Domain, Codomain> = Relation<Domain, Codomain, FjallProvider<Domain, Codomain>>;

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
    pub(crate) object_verbs: WorkingSet<ObjAndUUIDHolder, ProgramType>,
    pub(crate) object_propdefs: WorkingSet<Obj, PropDefs>,
    pub(crate) object_propvalues: WorkingSet<ObjAndUUIDHolder, Var>,
    pub(crate) object_propflags: WorkingSet<ObjAndUUIDHolder, PropPerms>,
    pub(crate) verb_resolution_cache: Box<VerbResolutionCache>,
    pub(crate) prop_resolution_cache: Box<PropResolutionCache>,
    pub(crate) ancestry_cache: Box<AncestryCache>,
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

        let sequences = [(); 16].map(|_| Arc::new(CachePadded::new(AtomicI64::new(-1))));

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
                config
                    .object_location
                    .clone()
                    .unwrap_or_default()
                    .partition_options(),
            )
            .unwrap();
        let object_contents = keyspace
            .open_partition(
                "object_contents",
                config
                    .object_contents
                    .clone()
                    .unwrap_or_default()
                    .partition_options(),
            )
            .unwrap();
        let object_flags = keyspace
            .open_partition(
                "object_flags",
                config
                    .object_flags
                    .clone()
                    .unwrap_or_default()
                    .partition_options(),
            )
            .unwrap();
        let object_parent = keyspace
            .open_partition(
                "object_parent",
                config
                    .object_parent
                    .clone()
                    .unwrap_or_default()
                    .partition_options(),
            )
            .unwrap();
        let object_children = keyspace
            .open_partition(
                "object_children",
                config
                    .object_children
                    .clone()
                    .unwrap_or_default()
                    .partition_options(),
            )
            .unwrap();
        let object_owner = keyspace
            .open_partition(
                "object_owner",
                config
                    .object_owner
                    .clone()
                    .unwrap_or_default()
                    .partition_options(),
            )
            .unwrap();
        let object_name = keyspace
            .open_partition(
                "object_name",
                config
                    .object_name
                    .clone()
                    .unwrap_or_default()
                    .partition_options(),
            )
            .unwrap();
        let object_verbdefs = keyspace
            .open_partition(
                "object_verbdefs",
                config
                    .object_verbdefs
                    .clone()
                    .unwrap_or_default()
                    .partition_options(),
            )
            .unwrap();
        let object_verbs = keyspace
            .open_partition(
                "object_verbs",
                config
                    .object_verbs
                    .clone()
                    .unwrap_or_default()
                    .partition_options(),
            )
            .unwrap();
        let object_propdefs = keyspace
            .open_partition(
                "object_propdefs",
                config
                    .object_propdefs
                    .clone()
                    .unwrap_or_default()
                    .partition_options(),
            )
            .unwrap();
        let object_propvalues = keyspace
            .open_partition(
                "object_propvalues",
                config
                    .object_propvalues
                    .clone()
                    .unwrap_or_default()
                    .partition_options(),
            )
            .unwrap();
        let object_propflags = keyspace
            .open_partition(
                "object_propflags",
                config
                    .object_propflags
                    .clone()
                    .unwrap_or_default()
                    .partition_options(),
            )
            .unwrap();

        let object_location = FjallProvider::new("oloc", object_location);
        let object_contents = FjallProvider::new("ocont", object_contents);
        let object_flags = FjallProvider::new("oflags", object_flags);
        let object_parent = FjallProvider::new("oparen", object_parent);
        let object_children = FjallProvider::new("ochld", object_children);
        let object_owner = FjallProvider::new("oown", object_owner);
        let object_name = FjallProvider::new("oname", object_name);
        let object_verbdefs = FjallProvider::new("ovdef", object_verbdefs);
        let object_verbs = FjallProvider::new("overb", object_verbs);
        let object_propdefs = FjallProvider::new("opdefs", object_propdefs);
        let object_propvalues = FjallProvider::new("opvals", object_propvalues);
        let object_propflags = FjallProvider::new("opflags", object_propflags);

        let object_location =
            Relation::new(Symbol::mk("object_location"), Arc::new(object_location));
        let object_contents =
            Relation::new(Symbol::mk("object_contents"), Arc::new(object_contents));
        let object_flags = Relation::new(Symbol::mk("object_flags"), Arc::new(object_flags));
        let object_parent = Relation::new(Symbol::mk("object_parent"), Arc::new(object_parent));
        let object_children =
            Relation::new(Symbol::mk("object_children"), Arc::new(object_children));
        let object_owner = Relation::new(Symbol::mk("object_owner"), Arc::new(object_owner));
        let object_name = Relation::new(Symbol::mk("object_name"), Arc::new(object_name));
        let object_verbdefs =
            Relation::new(Symbol::mk("object_verbdefs"), Arc::new(object_verbdefs));
        let object_verbs = Relation::new(Symbol::mk("object_verbs"), Arc::new(object_verbs));
        let object_propdefs =
            Relation::new(Symbol::mk("object_propdefs"), Arc::new(object_propdefs));
        let object_propvalues =
            Relation::new(Symbol::mk("object_propvalues"), Arc::new(object_propvalues));
        let object_propflags =
            Relation::new(Symbol::mk("object_propflags"), Arc::new(object_propflags));

        let (commit_channel, commit_receiver) = flume::unbounded();
        let (usage_send, usage_recv) = flume::unbounded();
        let kill_switch = Arc::new(AtomicBool::new(false));
        let verb_resolution_cache = ArcSwap::new(Arc::new(VerbResolutionCache::new()));
        let prop_resolution_cache = ArcSwap::new(Arc::new(PropResolutionCache::new()));
        let ancestry_cache = ArcSwap::new(Arc::new(AncestryCache::default()));
        let s = Arc::new(Self {
            monotonic: CachePadded::new(AtomicU64::new(start_tx_num)),
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
            ancestry_cache,
            jh: Mutex::new(None),
        });

        s.clone()
            .start_processing_thread(commit_receiver, usage_recv, kill_switch, config);

        (s, fresh)
    }

    pub(crate) fn start_transaction(&self) -> WorldStateTransaction {
        let tx = Tx {
            ts: Timestamp(
                self.monotonic
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            ),
        };

        let vc = self.verb_resolution_cache.load();
        let verb_resolution_cache = vc.fork();

        let pc = self.prop_resolution_cache.load();
        let prop_resolution_cache = pc.fork();

        let ac = self.ancestry_cache.load();
        let ancestry_cache = ac.fork();
        WorldStateTransaction {
            tx,
            commit_channel: self.commit_channel.clone(),
            usage_channel: self.usage_send.clone(),
            object_location: self.object_location.start(&tx),
            object_contents: self.object_contents.start(&tx),
            object_flags: self.object_flags.start(&tx),
            object_parent: self.object_parent.start(&tx),
            object_children: self.object_children.start(&tx),
            object_owner: self.object_owner.start(&tx),
            object_name: self.object_name.start(&tx),
            object_verbdefs: self.object_verbdefs.start(&tx),
            object_verbs: self.object_verbs.start(&tx),
            object_propdefs: self.object_propdefs.start(&tx),
            object_propvalues: self.object_propvalues.start(&tx),
            object_propflags: self.object_propflags.start(&tx),
            sequences: self.sequences.clone(),
            verb_resolution_cache,
            prop_resolution_cache,
            ancestry_cache,
            has_mutations: false,
        }
    }

    pub fn usage_bytes(&self) -> usize {
        self.keyspace.disk_space() as usize
    }

    pub fn stop(&self) {
        self.kill_switch
            .store(true, std::sync::atomic::Ordering::SeqCst);

        let mut jh_lock = self.jh.lock().unwrap();
        if let Some(jh) = jh_lock.take() {
            jh.join().unwrap();
        }

        self.object_parent.stop_provider().unwrap();
        self.object_location.stop_provider().unwrap();
        self.object_contents.stop_provider().unwrap();
        self.object_flags.stop_provider().unwrap();
        self.object_children.stop_provider().unwrap();
        self.object_owner.stop_provider().unwrap();
        self.object_name.stop_provider().unwrap();
        self.object_verbdefs.stop_provider().unwrap();
        self.object_verbs.stop_provider().unwrap();
        self.object_propdefs.stop_provider().unwrap();
        self.object_propvalues.stop_provider().unwrap();
        self.object_propflags.stop_provider().unwrap();
        if let Err(e) = self.keyspace.persist(PersistMode::SyncAll) {
            error!("Failed to persist keyspace: {}", e);
        }
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

                    if kill_switch.load(std::sync::atomic::Ordering::SeqCst) {
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
                        Ok(CommitSet::CommitReadOnly(vc, pc, ac)) => {
                            if vc.has_changed() {
                                this.verb_resolution_cache.store(Arc::new(*vc));
                            }
                            if pc.has_changed() {
                                this.prop_resolution_cache.store(Arc::new(*pc));
                            }
                            if ac.has_changed() {
                                this.ancestry_cache.store(Arc::new(*ac));
                            }
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

                    let mut object_flags = this.object_flags.begin_check();
                    let mut object_parent = this.object_parent.begin_check();
                    let mut object_children = this.object_children.begin_check();
                    let mut object_owner = this.object_owner.begin_check();
                    let mut object_location = this.object_location.begin_check();
                    let mut object_contents = this.object_contents.begin_check();
                    let mut object_name = this.object_name.begin_check();
                    let mut object_verbdefs = this.object_verbdefs.begin_check();
                    let mut object_verbs = this.object_verbs.begin_check();
                    let mut object_propdefs = this.object_propdefs.begin_check();
                    let mut object_propvalues = this.object_propvalues.begin_check();
                    let mut object_propflags = this.object_propflags.begin_check();


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
                        if object_flags.check(&ws.object_flags).is_err()
                            || object_parent.check(&ws.object_parent).is_err()
                            || object_children.check(&ws.object_children).is_err()
                            || object_owner.check(&ws.object_owner).is_err()
                            || object_location.check(&ws.object_location).is_err()
                            || object_contents.check(&ws.object_contents).is_err()
                            || object_name.check(&ws.object_name).is_err()
                            || object_verbdefs.check(&ws.object_verbdefs).is_err()
                            || object_verbs.check(&ws.object_verbs).is_err()
                            || object_propdefs.check(&ws.object_propdefs).is_err()
                            || object_propvalues.check(&ws.object_propvalues).is_err()
                            || object_propflags.check(&ws.object_propflags).is_err() {
                            reply.send(CommitResult::ConflictRetry).ok();
                            continue;
                        }
                        drop(_t);

                        // If after checking, it turns out there was nothing to do...
                        let all_clean = !object_flags.dirty()
                            && !object_parent.dirty()
                            && !object_children.dirty()
                            && !object_owner.dirty()
                            && !object_location.dirty()
                            && !object_contents.dirty()
                            && !object_name.dirty()
                            && !object_verbdefs.dirty()
                            && !object_verbs.dirty()
                            && !object_propdefs.dirty()
                            && !object_propvalues.dirty()
                            && !object_propflags.dirty();

                        if all_clean {
                            reply.send(CommitResult::Success).ok();

                            if ws.verb_resolution_cache.has_changed() {
                                this.verb_resolution_cache.store(Arc::new(*ws.verb_resolution_cache));
                            }
                            if ws.prop_resolution_cache.has_changed() {
                                this.prop_resolution_cache.store(Arc::new(*ws.prop_resolution_cache));
                            }
                            if ws.ancestry_cache.has_changed() {
                                this.ancestry_cache.store(Arc::new(*ws.ancestry_cache));
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

                        if object_flags.apply(ws.object_flags).is_err()
                            || object_parent.apply(ws.object_parent).is_err()
                            || object_children.apply(ws.object_children).is_err()
                            || object_owner.apply(ws.object_owner).is_err()
                            || object_location.apply(ws.object_location).is_err()
                            || object_contents.apply(ws.object_contents).is_err()
                            || object_name.apply(ws.object_name).is_err()
                            || object_verbdefs.apply(ws.object_verbdefs).is_err()
                            || object_verbs.apply(ws.object_verbs).is_err()
                            || object_propdefs.apply(ws.object_propdefs).is_err()
                            || object_propvalues.apply(ws.object_propvalues).is_err()
                            || object_propflags.apply(ws.object_propflags).is_err() {
                            reply.send(CommitResult::ConflictRetry).ok();
                            continue;
                        }


                        // Now take write-lock on all relations just for the very instant that we swap em out.
                        // This will hold up new transactions starting, unfortunately.
                        // TODO: this is the major source of low throughput in benchmarking
                        {
                            let object_flags_lock = object_flags.dirty().then(|| this.object_flags.write_lock());
                            object_flags.commit(object_flags_lock);

                            let object_parent_lock = object_parent.dirty().then(|| this.object_parent.write_lock());
                            object_parent.commit(object_parent_lock);

                            let object_children_lock = object_children.dirty().then(|| this.object_children.write_lock());
                            object_children.commit(object_children_lock);

                            let object_owner_lock = object_owner.dirty().then(|| this.object_owner.write_lock());
                            object_owner.commit(object_owner_lock);

                            let object_location_lock = object_location.dirty().then(|| this.object_location.write_lock());
                            object_location.commit(object_location_lock);

                            let object_contents_lock = object_contents.dirty().then(|| this.object_contents.write_lock());
                            object_contents.commit(object_contents_lock);

                            let object_name_lock = object_name.dirty().then(|| this.object_name.write_lock());
                            object_name.commit(object_name_lock);

                            let object_verbdefs_lock = object_verbdefs.dirty().then(|| this.object_verbdefs.write_lock());
                            object_verbdefs.commit(object_verbdefs_lock);

                            let object_verbs_lock = object_verbs.dirty().then(|| this.object_verbs.write_lock());
                            object_verbs.commit(object_verbs_lock);

                            let object_propdefs_lock = object_propdefs.dirty().then(|| this.object_propdefs.write_lock());
                            object_propdefs.commit(object_propdefs_lock);

                            let object_propvalues_lock = object_propvalues.dirty().then(|| this.object_propvalues.write_lock());
                            object_propvalues.commit(object_propvalues_lock);

                            let object_propflags_lock = object_propflags.dirty().then(|| this.object_propflags.write_lock());
                            object_propflags.commit(object_propflags_lock);
                        }
                        // No need to block the caller while we're doing the final write to disk.
                        reply.send(CommitResult::Success).ok();

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
                        if ws.verb_resolution_cache.has_changed() {
                            this.verb_resolution_cache.store(Arc::new(*ws.verb_resolution_cache));
                        }
                        if ws.prop_resolution_cache.has_changed() {
                            this.prop_resolution_cache.store(Arc::new(*ws.prop_resolution_cache));
                        }
                        if ws.ancestry_cache.has_changed() {
                            this.ancestry_cache.store(Arc::new(*ws.ancestry_cache));
                        }
                    }

                    let _t = PerfTimerGuard::new(&counters.commit_write_phase);

                    // Now write out the current state of the sequences to the seq partition.
                    // Start by making sure that the monotonic sequence is written out.
                    this.sequences[15].store(
                        this.monotonic.load(std::sync::atomic::Ordering::Relaxed) as i64,
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

#[cfg_attr(
    any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "powerpc64",
    ),
    repr(align(128))
)]
#[cfg_attr(
    any(
        target_arch = "arm",
        target_arch = "mips",
        target_arch = "mips64",
        target_arch = "riscv64",
    ),
    repr(align(32))
)]
#[cfg_attr(target_arch = "s390x", repr(align(256)))]
#[cfg_attr(
    not(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "powerpc64",
        target_arch = "arm",
        target_arch = "mips",
        target_arch = "mips64",
        target_arch = "riscv64",
        target_arch = "s390x",
    )),
    repr(align(64))
)]
pub struct CachePadded<T> {
    pub value: T,
}

impl<T> CachePadded<T> {
    pub fn new(value: T) -> Self {
        Self { value }
    }
}

impl<T> std::ops::Deref for CachePadded<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
