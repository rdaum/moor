// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

// TODO: support sorted indices, too.
// TODO: 'join' and transitive closure -> datalog-style variable unification

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::info;

use crate::tuplebox::backing::BackingStoreClient;
use crate::tuplebox::base_relation::BaseRelation;
use crate::tuplebox::rocks_backing::RocksBackingStore;
use crate::tuplebox::slots::SlotBox;
use crate::tuplebox::tuples::TxTuple;
use crate::tuplebox::tx::transaction::{CommitError, CommitSet, Transaction};
use crate::tuplebox::tx::working_set::WorkingSet;
use crate::tuplebox::RelationId;

/// Meta-data about a relation
#[derive(Clone, Debug)]
pub struct RelationInfo {
    /// Human readable name of the relation.
    pub name: String,
    /// The domain type ID, which is user defined in the client's type system, and not enforced
    pub domain_type_id: u16,
    /// The codomain type ID, which is user defined in the client's type system, and not enforced
    pub codomain_type_id: u16,
    /// Whether or not this relation has a secondary index on its codomain.
    pub secondary_indexed: bool,
}

/// The tuplebox is the set of relations, referenced by their unique (usize) relation ID.
/// It exposes interfaces for starting & managing transactions on those relations.
/// It is, essentially, a micro database.
// TODO: locking in general here is (probably) safe, but not optimal. optimistic locking would be
//   better for various portions here.
pub struct TupleBox {
    relation_info: Vec<RelationInfo>,
    /// The monotonically increasing transaction ID "timestamp" counter.
    // TODO: take a look at Adnan's thread-sharded approach described in section 3.1
    //   (https://www.vldb.org/pvldb/vol16/p1426-alhomssi.pdf) -- "Ordered Snapshot Instant Commit"
    maximum_transaction: AtomicU64,
    /// The set of currently active transactions, which will be used to prune old unreferenced
    /// versions of tuples.
    active_transactions: RwLock<HashSet<u64>>,
    /// Monotonically incrementing sequence counters.
    sequences: Vec<AtomicU64>,
    /// The copy-on-write set of current canonical base relations.
    // TODO: this is a candidate for an optimistic lock.
    pub(crate) canonical: RwLock<Vec<BaseRelation>>,

    slotbox: Arc<SlotBox>,

    backing_store: Option<BackingStoreClient>,
}

impl TupleBox {
    pub async fn new(
        memory_size: usize,
        page_size: usize,
        path: Option<PathBuf>,
        relations: &[RelationInfo],
        num_sequences: usize,
    ) -> Arc<Self> {
        let slotbox = Arc::new(SlotBox::new(page_size, memory_size));
        let mut base_relations = Vec::with_capacity(relations.len());
        for (rid, r) in relations.iter().enumerate() {
            base_relations.push(BaseRelation::new(slotbox.clone(), RelationId(rid), 0));
            if r.secondary_indexed {
                base_relations.last_mut().unwrap().add_secondary_index();
            }
        }
        let mut sequences = vec![0; num_sequences];
        let backing_store = match path {
            None => None,
            Some(path) => {
                let bs = RocksBackingStore::start(
                    path,
                    relations.to_vec(),
                    &mut base_relations,
                    &mut sequences,
                )
                .await;
                info!("Backing store loaded, and write-ahead thread started...");
                Some(bs)
            }
        };

        let sequences = sequences
            .into_iter()
            .map(|s| AtomicU64::new(s))
            .collect::<Vec<_>>();

        Arc::new(Self {
            relation_info: relations.to_vec(),
            maximum_transaction: AtomicU64::new(0),
            active_transactions: RwLock::new(HashSet::new()),
            canonical: RwLock::new(base_relations),
            sequences,
            backing_store,
            slotbox,
        })
    }

    pub fn relation_info(&self) -> Vec<RelationInfo> {
        self.relation_info.clone()
    }

    /// Begin a transaction against the current canonical relations.
    pub fn start_tx(self: Arc<Self>) -> Transaction {
        let next_ts = self
            .maximum_transaction
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Transaction::new(next_ts, self.slotbox.clone(), self.clone())
    }

    pub fn next_ts(self: Arc<Self>) -> u64 {
        let next_ts = self
            .maximum_transaction
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        next_ts
    }

    /// Get the next value for the given sequence.
    pub async fn sequence_next(self: Arc<Self>, sequence_number: usize) -> u64 {
        let sequence = &self.sequences[sequence_number];
        loop {
            let current = sequence.load(std::sync::atomic::Ordering::SeqCst);
            if let Ok(n) = sequence.compare_exchange(
                current,
                current + 1,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            ) {
                return n;
            }
        }
    }

    /// Get the current value for the given sequence.
    pub async fn sequence_current(self: Arc<Self>, sequence_number: usize) -> u64 {
        self.sequences[sequence_number].load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Update the given sequence to `value` iff `value` is greater than the current value.
    pub async fn update_sequence_max(self: Arc<Self>, sequence_number: usize, value: u64) {
        let sequence = &self.sequences[sequence_number];
        loop {
            let current = sequence.load(std::sync::atomic::Ordering::SeqCst);
            if let Ok(_) = sequence.compare_exchange(
                current,
                std::cmp::max(current, value),
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            ) {
                return;
            }
        }
    }

    pub async fn with_relation<R, F: Fn(&BaseRelation) -> R>(
        &self,
        relation_id: RelationId,
        f: F,
    ) -> R {
        f(self
            .canonical
            .read()
            .await
            .get(relation_id.0)
            .expect("No such relation"))
    }

    /// Prepare a commit set for the given transaction. This will scan through the transaction's
    /// working set, and for each tuple, check to see if it's safe to commit. If it is, then we'll
    /// add it to the commit set.
    pub(crate) async fn prepare_commit_set<'a>(
        &self,
        tx_ts: u64,
        tx_working_set: &WorkingSet,
    ) -> Result<CommitSet, CommitError> {
        let mut commitset = CommitSet::new(tx_ts);

        for (relation_id, local_relation) in tx_working_set.relations.iter().enumerate() {
            let relation_id = RelationId(relation_id);
            // scan through the local working set, and for each tuple, check to see if it's safe to
            // commit. If it is, then we'll add it to the commit set.
            // note we're not actually committing yet, just producing a candidate commit set
            let canonical = &self.canonical.read().await[relation_id.0];
            for tuple in local_relation.tuples() {
                let canon_tuple = canonical.seek_by_domain(tuple.domain().clone());

                // If there's no value there, and our local is not tombstoned and we're not doing
                // an insert -- that's already a conflict.
                // Otherwise we can straight-away insert into the canonical base relation.
                // TODO: it should be possible to do this without having the fork logic exist twice
                //   here.
                let Some(cv) = canon_tuple else {
                    match &tuple {
                        TxTuple::Insert(tref) => {
                            let t = tref.get();
                            t.update_timestamp(self.slotbox.clone(), tx_ts);
                            let forked_relation = commitset.fork(relation_id, &canonical);
                            forked_relation.upsert_tuple(tref.clone());
                            continue;
                        }
                        TxTuple::Tombstone { .. } => {
                            // We let this pass, as this must be a delete of something we inserted
                            // temporarily previously in our transaction.
                            continue;
                        }
                        TxTuple::Update(..) | TxTuple::Value(..) => {
                            return Err(CommitError::TupleVersionConflict);
                        }
                    }
                };

                // If the timestamp in our working tuple is our own ts, that's a conflict, because
                // it means someone else has already committed a change to this tuple that we
                // thought was net-new (and so used our own TS)
                if tuple.ts() == tx_ts {
                    return Err(CommitError::TupleVersionConflict);
                };

                // Check the timestamp on the value, if it's newer than the read-timestamp,
                // we have for this tuple then that's a conflict, because it means someone else has
                // already committed a change to this tuple.
                let cv = cv.get();
                if cv.ts() > tuple.ts() {
                    return Err(CommitError::TupleVersionConflict);
                }

                // Otherwise apply the change into a new canonical relation, which is a CoW
                // branching of the old one.
                let forked_relation = commitset.fork(relation_id, &canonical);
                match &tuple {
                    TxTuple::Insert(tref) | TxTuple::Update(tref) => {
                        let t = tref.get();
                        t.update_timestamp(self.slotbox.clone(), tx_ts);
                        let forked_relation = commitset.fork(relation_id, &canonical);
                        forked_relation.upsert_tuple(tref.clone());
                    }
                    TxTuple::Value(..) => {}
                    TxTuple::Tombstone { ts: _, domain: k } => {
                        forked_relation.remove_by_domain(k.clone());
                    }
                }
            }
        }
        Ok(commitset)
    }

    /// Actually commit a transaction's (approved) CommitSet. If the underlying base relations have
    /// changed since the transaction started, this will return `Err(RelationContentionConflict)`
    /// and the transaction can choose to try to produce a new CommitSet, or just abort.
    pub(crate) async fn try_commit(&self, commit_set: CommitSet) -> Result<(), CommitError> {
        // swap the active canonical state with the new one. but only if the timestamps have not
        // changed in the interim; we have to hold a lock while this is done. If any relations have
        // had their ts change, we need to retry.
        // We have to hold a lock during the duration of this. If we fail, we will loop back
        // and retry.
        let mut canonical = self.canonical.write().await;
        for relation in commit_set.iter() {
            // Did the relation get committed to by someone else in the interim? If so, return
            // back to the transaction letting it know that, and it can decide if it wants to
            // retry.
            if relation.ts != canonical[relation.id.0].ts {
                return Err(CommitError::RelationContentionConflict);
            }
        }

        // Everything passed, so we can commit the changes by swapping in the new canonical
        // before releasing the lock.
        let commit_ts = commit_set.ts;
        for relation in commit_set.into_iter() {
            let idx = relation.id.0;
            canonical[idx] = relation;
            // And update the timestamp on the canonical relation.
            canonical[idx].ts = commit_ts;
        }
        // Clear out the active transaction.
        self.active_transactions.write().await.remove(&commit_ts);

        Ok(())
    }

    pub(crate) async fn abort_transaction(&self, ts: u64) {
        self.active_transactions.write().await.remove(&ts);
    }

    pub async fn sync(&self, ts: u64, world_state: WorkingSet) {
        if let Some(bs) = &self.backing_store {
            let seqs = self
                .sequences
                .iter()
                .map(|s| s.load(std::sync::atomic::Ordering::SeqCst))
                .collect();
            bs.sync(ts, world_state, seqs).await;
        }
    }

    pub async fn shutdown(&self) {
        if let Some(bs) = &self.backing_store {
            bs.shutdown().await;
        }
    }
}
