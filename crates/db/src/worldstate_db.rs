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
use crate::db_transaction::DbTransaction;
use crate::fjall_provider::FjallProvider;
use crate::tx::{SizedCache, Timestamp, TransactionalCache, Tx, WorkingSet};
use crate::{BytesHolder, ObjAndUUIDHolder, StringHolder};
use crossbeam_channel::Sender;
use fjall::{Config, PartitionCreateOptions, PartitionHandle, PersistMode};
use moor_common::model::{CommitResult, ObjFlag, ObjSet, PropDefs, PropPerms, VerbDefs};
use moor_common::util::BitEnum;
use moor_var::{Obj, Var};
use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64};
use std::time::Duration;
use tempfile::TempDir;
use tracing::warn;

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
}

pub struct WorldStateDB {
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
    commit_channel: Sender<(WorkingSets, oneshot::Sender<CommitResult>)>,
    usage_send: crossbeam_channel::Sender<oneshot::Sender<usize>>,
}

impl WorldStateDB {
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

        let default_cache_eviction_threshold = config.default_eviction_threshold;
        let object_location = Arc::new(TransactionalCache::new(
            Arc::new(object_location),
            config
                .object_location
                .cache_eviction_threshold
                .unwrap_or(default_cache_eviction_threshold),
        ));
        let object_contents = Arc::new(TransactionalCache::new(
            Arc::new(object_contents),
            config
                .object_contents
                .cache_eviction_threshold
                .unwrap_or(default_cache_eviction_threshold),
        ));
        let object_flags = Arc::new(TransactionalCache::new(
            Arc::new(object_flags),
            config
                .object_flags
                .cache_eviction_threshold
                .unwrap_or(default_cache_eviction_threshold),
        ));
        let object_parent = Arc::new(TransactionalCache::new(
            Arc::new(object_parent),
            config
                .object_parent
                .cache_eviction_threshold
                .unwrap_or(default_cache_eviction_threshold),
        ));
        let object_children = Arc::new(TransactionalCache::new(
            Arc::new(object_children),
            config
                .object_children
                .cache_eviction_threshold
                .unwrap_or(default_cache_eviction_threshold),
        ));
        let object_owner = Arc::new(TransactionalCache::new(
            Arc::new(object_owner),
            config
                .object_owner
                .cache_eviction_threshold
                .unwrap_or(default_cache_eviction_threshold),
        ));
        let object_name = Arc::new(TransactionalCache::new(
            Arc::new(object_name),
            config
                .object_name
                .cache_eviction_threshold
                .unwrap_or(default_cache_eviction_threshold),
        ));
        let object_verbdefs = Arc::new(TransactionalCache::new(
            Arc::new(object_verbdefs),
            config
                .object_verbdefs
                .cache_eviction_threshold
                .unwrap_or(default_cache_eviction_threshold),
        ));
        let object_verbs = Arc::new(TransactionalCache::new(
            Arc::new(object_verbs),
            config
                .object_verbs
                .cache_eviction_threshold
                .unwrap_or(default_cache_eviction_threshold),
        ));
        let object_propdefs = Arc::new(TransactionalCache::new(
            Arc::new(object_propdefs),
            config
                .object_propdefs
                .cache_eviction_threshold
                .unwrap_or(default_cache_eviction_threshold),
        ));
        let object_propvalues = Arc::new(TransactionalCache::new(
            Arc::new(object_propvalues),
            config
                .object_propvalues
                .cache_eviction_threshold
                .unwrap_or(default_cache_eviction_threshold),
        ));
        let object_propflags = Arc::new(TransactionalCache::new(
            Arc::new(object_propflags),
            config
                .object_propflags
                .cache_eviction_threshold
                .unwrap_or(default_cache_eviction_threshold),
        ));

        let (commit_channel, commit_receiver) = crossbeam_channel::unbounded();
        let (usage_send, usage_recv) = crossbeam_channel::unbounded();
        let kill_switch = Arc::new(AtomicBool::new(false));
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
        });

        s.clone()
            .start_processing_thread(commit_receiver, usage_recv, kill_switch, config);

        (s, fresh)
    }

    pub(crate) fn start_transaction(&self) -> DbTransaction {
        let tx = Tx {
            ts: Timestamp(
                self.monotonic
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst),
            ),
        };

        DbTransaction {
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

    pub fn stop(&self) {
        self.kill_switch
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    fn start_processing_thread(
        self: Arc<Self>,
        receiver: crossbeam_channel::Receiver<(WorkingSets, oneshot::Sender<CommitResult>)>,
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
                        Ok(msg) => msg,
                        Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                            continue;
                        }
                        Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                            break;
                        }
                    };

                    let object_flags = this.object_flags.lock();
                    let object_parent = this.object_parent.lock();
                    let object_children = this.object_children.lock();
                    let object_owner = this.object_owner.lock();
                    let object_location = this.object_location.lock();
                    let object_contents = this.object_contents.lock();
                    let object_name = this.object_name.lock();
                    let object_verbdefs = this.object_verbdefs.lock();
                    let object_verbs = this.object_verbs.lock();
                    let object_propdefs = this.object_propdefs.lock();
                    let object_propvalues = this.object_propvalues.lock();
                    let object_propflags = this.object_propflags.lock();

                    let Ok(ol_lock) = this.object_flags.check(object_flags, &ws.object_flags)
                    else {
                        reply.send(CommitResult::ConflictRetry).unwrap();

                        continue;
                    };

                    let Ok(op_lock) = this.object_parent.check(object_parent, &ws.object_parent)
                    else {
                        reply.send(CommitResult::ConflictRetry).unwrap();

                        continue;
                    };

                    let Ok(oc_lock) = this
                        .object_children
                        .check(object_children, &ws.object_children)
                    else {
                        reply.send(CommitResult::ConflictRetry).unwrap();
                        continue;
                    };

                    let Ok(oo_lock) = this.object_owner.check(object_owner, &ws.object_owner)
                    else {
                        reply.send(CommitResult::ConflictRetry).unwrap();
                        continue;
                    };

                    let Ok(oloc_lock) = this
                        .object_location
                        .check(object_location, &ws.object_location)
                    else {
                        reply.send(CommitResult::ConflictRetry).unwrap();
                        continue;
                    };

                    let Ok(ocont_lock) = this
                        .object_contents
                        .check(object_contents, &ws.object_contents)
                    else {
                        reply.send(CommitResult::ConflictRetry).unwrap();
                        continue;
                    };

                    let Ok(on_lock) = this.object_name.check(object_name, &ws.object_name) else {
                        reply.send(CommitResult::ConflictRetry).unwrap();
                        continue;
                    };

                    let Ok(ovd_lock) = this
                        .object_verbdefs
                        .check(object_verbdefs, &ws.object_verbdefs)
                    else {
                        reply.send(CommitResult::ConflictRetry).unwrap();
                        continue;
                    };

                    let Ok(ov_lock) = this.object_verbs.check(object_verbs, &ws.object_verbs)
                    else {
                        reply.send(CommitResult::ConflictRetry).unwrap();
                        continue;
                    };

                    let Ok(opd_lock) = this
                        .object_propdefs
                        .check(object_propdefs, &ws.object_propdefs)
                    else {
                        reply.send(CommitResult::ConflictRetry).unwrap();
                        continue;
                    };

                    let Ok(opv_lock) = this
                        .object_propvalues
                        .check(object_propvalues, &ws.object_propvalues)
                    else {
                        reply.send(CommitResult::ConflictRetry).unwrap();
                        continue;
                    };

                    let Ok(opf_lock) = this
                        .object_propflags
                        .check(object_propflags, &ws.object_propflags)
                    else {
                        reply.send(CommitResult::ConflictRetry).unwrap();
                        continue;
                    };
                    //
                    let Ok(_unused) = this.object_flags.apply(ol_lock, ws.object_flags) else {
                        reply.send(CommitResult::ConflictRetry).unwrap();
                        continue;
                    };

                    let Ok(_unused) = this.object_parent.apply(op_lock, ws.object_parent) else {
                        reply.send(CommitResult::ConflictRetry).unwrap();
                        continue;
                    };

                    let Ok(_unused) = this.object_children.apply(oc_lock, ws.object_children)
                    else {
                        reply.send(CommitResult::ConflictRetry).unwrap();
                        continue;
                    };

                    let Ok(_unused) = this.object_owner.apply(oo_lock, ws.object_owner) else {
                        reply.send(CommitResult::ConflictRetry).unwrap();
                        continue;
                    };

                    let Ok(_unused) = this.object_location.apply(oloc_lock, ws.object_location)
                    else {
                        reply.send(CommitResult::ConflictRetry).unwrap();
                        continue;
                    };

                    let Ok(_unused) = this.object_contents.apply(ocont_lock, ws.object_contents)
                    else {
                        reply.send(CommitResult::ConflictRetry).unwrap();
                        continue;
                    };

                    let Ok(_unused) = this.object_name.apply(on_lock, ws.object_name) else {
                        reply.send(CommitResult::ConflictRetry).unwrap();
                        continue;
                    };

                    let Ok(_unused) = this.object_verbdefs.apply(ovd_lock, ws.object_verbdefs)
                    else {
                        reply.send(CommitResult::ConflictRetry).unwrap();
                        continue;
                    };

                    let Ok(_unused) = this.object_verbs.apply(ov_lock, ws.object_verbs) else {
                        reply.send(CommitResult::ConflictRetry).unwrap();
                        continue;
                    };

                    let Ok(_unused) = this.object_propdefs.apply(opd_lock, ws.object_propdefs)
                    else {
                        reply.send(CommitResult::ConflictRetry).unwrap();
                        continue;
                    };

                    let Ok(_unused) = this.object_propvalues.apply(opv_lock, ws.object_propvalues)
                    else {
                        reply.send(CommitResult::ConflictRetry).unwrap();
                        continue;
                    };

                    let Ok(_unused) = this.object_propflags.apply(opf_lock, ws.object_propflags)
                    else {
                        reply.send(CommitResult::ConflictRetry).unwrap();
                        continue;
                    };

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
                            .unwrap();
                    }

                    self.keyspace
                        .persist(PersistMode::SyncAll)
                        .expect("persist failed");

                    reply.send(CommitResult::Success).unwrap();
                }
            })
            .expect("failed to start DB processing thread");
    }
}

impl Drop for WorldStateDB {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use crate::config::DatabaseConfig;
    use crate::db_transaction::DbTransaction;
    use crate::worldstate_transaction::WorldStateTransaction;
    use crate::{
        perform_reparent_props, perform_test_create_object, perform_test_create_object_fixed_id,
        perform_test_descendants, perform_test_location_contents, perform_test_max_object,
        perform_test_object_move_commits, perform_test_parent_children,
        perform_test_recycle_object, perform_test_regression_properties,
        perform_test_rename_property, perform_test_simple_property,
        perform_test_transitive_property_resolution,
        perform_test_transitive_property_resolution_clear_property, perform_test_verb_add_update,
        perform_test_verb_resolve, perform_test_verb_resolve_inherited,
        perform_test_verb_resolve_wildcard,
    };
    use moor_common::model::{HasUuid, ObjAttrs, ObjFlag, PropFlag};
    use moor_common::util::BitEnum;
    use moor_var::{NOTHING, Obj, SYSTEM_OBJECT, Symbol, v_int, v_str};
    use std::sync::Arc;

    fn test_db() -> Arc<super::WorldStateDB> {
        super::WorldStateDB::open(None, DatabaseConfig::default()).0
    }

    fn begin_tx(db: &Arc<super::WorldStateDB>) -> DbTransaction {
        db.start_transaction()
    }

    #[test]
    fn test_create_object() {
        let db = test_db();
        perform_test_create_object(|| begin_tx(&db));
    }

    #[test]
    fn test_create_object_fixed_id() {
        let db = test_db();
        perform_test_create_object_fixed_id(|| begin_tx(&db));
    }

    #[test]
    fn test_parent_children() {
        let db = test_db();
        perform_test_parent_children(|| begin_tx(&db));
    }

    #[test]
    fn test_descendants() {
        let db = test_db();
        perform_test_descendants(|| begin_tx(&db));
    }

    #[test]
    fn test_location_contents() {
        let db = test_db();
        perform_test_location_contents(|| begin_tx(&db));
    }

    /// Test data integrity of object moves between commits.
    #[test]
    fn test_object_move_commits() {
        let db = test_db();
        perform_test_object_move_commits(|| begin_tx(&db));
    }

    #[test]
    fn test_simple_property() {
        let db = test_db();
        perform_test_simple_property(|| begin_tx(&db));
    }

    /// Regression test for updating-verbs failing.
    #[test]
    fn test_verb_add_update() {
        let db = test_db();
        perform_test_verb_add_update(|| begin_tx(&db));
    }

    #[test]
    fn test_transitive_property_resolution() {
        let db = test_db();
        perform_test_transitive_property_resolution(|| begin_tx(&db));
    }

    #[test]
    fn test_transitive_property_resolution_clear_property() {
        let db = test_db();
        perform_test_transitive_property_resolution_clear_property(|| begin_tx(&db));
    }

    #[test]
    fn test_rename_property() {
        let db = test_db();
        perform_test_rename_property(|| begin_tx(&db));
    }

    #[test]
    fn test_regression_properties() {
        let db = test_db();
        perform_test_regression_properties(|| begin_tx(&db));
    }

    #[test]
    fn test_verb_resolve() {
        let db = test_db();
        perform_test_verb_resolve(|| begin_tx(&db));
    }

    #[test]
    fn test_verb_resolve_inherited() {
        let db = test_db();
        perform_test_verb_resolve_inherited(|| begin_tx(&db));
    }

    #[test]
    fn test_verb_resolve_wildcard() {
        let db = test_db();
        perform_test_verb_resolve_wildcard(|| begin_tx(&db));
    }

    #[test]
    fn test_reparent() {
        let db = test_db();
        perform_reparent_props(|| begin_tx(&db));
    }

    #[test]
    fn test_recycle_object() {
        let db = test_db();
        perform_test_recycle_object(|| begin_tx(&db));
    }

    #[test]
    fn test_max_object() {
        let db = test_db();
        perform_test_max_object(|| begin_tx(&db));
    }

    #[test]
    fn test_chown_property() {
        let db = test_db();
        let mut tx = begin_tx(&db);
        let obj = tx.create_object(None, Default::default()).unwrap();
        let obj2 = tx.create_object(None, Default::default()).unwrap();
        let obj3 = tx.create_object(None, Default::default()).unwrap();
        let obj4 = tx.create_object(None, Default::default()).unwrap();
        tx.set_object_parent(&obj2, &obj).unwrap();
        tx.set_object_owner(&obj2, &obj4).unwrap();

        let uuid = tx
            .define_property(
                &obj,
                &obj,
                Symbol::mk("test"),
                &obj3,
                BitEnum::new_with(PropFlag::Chown),
                None,
            )
            .unwrap();

        // Property owner on obj should be obj3, but on obj2 it should be obj4, since that's
        // obj2's owner
        let o1perms = tx.retrieve_property_permissions(&obj, uuid).unwrap();
        assert_eq!(o1perms.owner(), obj3);
        let o2perms = tx.retrieve_property_permissions(&obj2, uuid).unwrap();
        assert_eq!(o2perms.owner(), obj4);

        // Now create a new object, descendant of obj, and the same logic should apply
        let obj5 = tx
            .create_object(
                None,
                ObjAttrs::new(
                    Obj::mk_id(obj4.id().0 + 1),
                    obj.clone(),
                    obj,
                    BitEnum::new_with(ObjFlag::Read),
                    "zoinks",
                ),
            )
            .unwrap();

        assert_eq!(tx.get_object_owner(&obj5).unwrap(), obj5);
        let o5perms = tx.retrieve_property_permissions(&obj5, uuid).unwrap();
        assert_eq!(o5perms.owner(), obj5);
    }

    /// Regression where property values of an ancestor shared with a new parent got lost.
    /// E.g. password lost when reparenting from "player" to "programmer", which also inherited
    /// from "player"
    #[test]
    fn test_property_subgraph_reparent() {
        let db = test_db();
        let mut tx = begin_tx(&db);
        let player = tx.create_object(None, Default::default()).unwrap();
        let uuid = tx
            .define_property(
                &player,
                &player,
                Symbol::mk("password"),
                &player,
                BitEnum::new(),
                Some(v_int(666)),
            )
            .unwrap();
        let programmer = tx.create_object(None, Default::default()).unwrap();
        let builder = tx.create_object(None, Default::default()).unwrap();
        let user = tx.create_object(None, Default::default()).unwrap();
        tx.set_object_parent(&builder, &player).unwrap();
        tx.set_object_parent(&programmer, &builder).unwrap();
        tx.set_object_parent(&user, &player).unwrap();

        tx.set_property(&user, uuid, v_int(1234567890)).unwrap();
        assert_eq!(
            tx.retrieve_property(&user, uuid).unwrap().0.unwrap(),
            v_int(1234567890)
        );

        tx.set_object_parent(&user, &programmer).unwrap();
        assert_eq!(
            tx.retrieve_property(&user, uuid).unwrap().0.unwrap(),
            v_int(1234567890)
        );
    }

    #[test]
    fn test_regression_recycle_parent_lose_prop() {
        let db = test_db();
        let mut tx = begin_tx(&db);
        let a = tx
            .create_object(
                None,
                ObjAttrs::new(Obj::mk_id(1), NOTHING, NOTHING, BitEnum::all(), "a"),
            )
            .unwrap();
        let b = tx
            .create_object(
                None,
                ObjAttrs::new(Obj::mk_id(1), a.clone(), NOTHING, BitEnum::all(), "b"),
            )
            .unwrap();
        tx.define_property(
            &b,
            &b,
            Symbol::mk("b"),
            &SYSTEM_OBJECT,
            BitEnum::new(),
            Some(v_str("b")),
        )
        .unwrap();

        assert_eq!(
            tx.resolve_property(&b, Symbol::mk("b")).unwrap().1,
            v_str("b")
        );

        tx.recycle_object(&a).unwrap();

        assert_eq!(
            tx.resolve_property(&b, Symbol::mk("b")).unwrap().1,
            v_str("b")
        );
    }

    #[test]
    fn test_regression_missing_child_propdef() {
        let db = test_db();
        let mut tx = begin_tx(&db);
        let object_e = tx.create_object(None, Default::default()).unwrap();
        let object_c = tx.create_object(None, Default::default()).unwrap();
        tx.define_property(
            &object_c,
            &object_c,
            Symbol::mk("c"),
            &SYSTEM_OBJECT,
            BitEnum::new(),
            Some(v_str("c")),
        )
        .unwrap();
        tx.set_object_parent(&object_e, &object_c).unwrap();

        let c_uuid = tx
            .resolve_property(&object_c, Symbol::mk("c"))
            .unwrap()
            .0
            .uuid();
        tx.delete_property(&object_c, c_uuid).unwrap();
    }
}
