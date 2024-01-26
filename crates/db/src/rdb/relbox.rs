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

use arc_swap::ArcSwap;
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::rdb::base_relation::BaseRelation;
use crate::rdb::paging::TupleBox;
use crate::rdb::tuples::TxTuple;
use crate::rdb::tx::WorkingSet;
use crate::rdb::tx::{CommitError, CommitSet, Transaction};
use crate::rdb::RelationId;

use super::paging::Pager;

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

/// The "RelBox" is the set of relations, referenced by their unique (usize) relation ID.
/// It exposes interfaces for starting & managing transactions on those relations.
/// It is, essentially, a micro database.
pub struct RelBox {
    /// The description of the set of base relations in our schema
    relation_info: Vec<RelationInfo>,

    /// The monotonically increasing transaction ID "timestamp" counter.
    maximum_transaction: AtomicU64,

    /// Monotonically incrementing sequence counters.
    sequences: Vec<AtomicU64>,

    /// The copy-on-write set of current canonical base relations.
    /// Held in ArcSwap so that we can swap them out atomically for their modified versions, without holding
    /// a lock for reads.
    pub(crate) canonical: Vec<ArcSwap<BaseRelation>>,

    /// Lock to hold while committing a transaction, prevents other commits from proceeding until we're done.
    commit_lock: Mutex<()>,

    /// The pager (which contains the buffer pool)
    pager: Arc<Pager>,

    /// Management of tuples happens through the tuple box (which uses said pager)
    tuple_box: Arc<TupleBox>,
}

impl Debug for RelBox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RelBox")
            .field("num_relations", &self.relation_info.len())
            .field("maximum_transaction", &self.maximum_transaction)
            .field("sequences", &self.sequences)
            .finish()
    }
}

impl RelBox {
    pub fn new(
        memory_size: usize,
        path: Option<PathBuf>,
        relations: &[RelationInfo],
        num_sequences: usize,
    ) -> Arc<Self> {
        let pager = Arc::new(Pager::new(memory_size).expect("Unable to create pager"));
        let tuple_box = Arc::new(TupleBox::new(pager.clone()));
        let mut base_relations = Vec::with_capacity(relations.len());
        for (rid, r) in relations.iter().enumerate() {
            base_relations.push(BaseRelation::new(RelationId(rid), 0));
            if r.secondary_indexed {
                base_relations.last_mut().unwrap().add_secondary_index();
            }
        }
        let mut sequences = vec![0; num_sequences];

        // Open the pager to the provided path, and restore the relations and sequences from it.
        // (If there's no path, this is a no-op and the database will be transient and empty).
        if let Some(path) = path {
            pager
                .open(path, &mut base_relations, &mut sequences, tuple_box.clone())
                .expect("Unable to open database at path");
        }
        let sequences = sequences
            .into_iter()
            .map(AtomicU64::new)
            .collect::<Vec<_>>();

        Arc::new(Self {
            relation_info: relations.to_vec(),
            maximum_transaction: AtomicU64::new(0),
            canonical: base_relations
                .into_iter()
                .map(|b| ArcSwap::new(Arc::new(b)))
                .collect(),
            sequences,
            tuple_box,
            pager,
            commit_lock: Mutex::new(()),
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
        Transaction::new(next_ts, self.tuple_box.clone(), self.clone())
    }

    pub fn next_ts(self: Arc<Self>) -> u64 {
        self.maximum_transaction
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    /// Increment this sequence and return its previous value.
    pub fn increment_sequence(self: Arc<Self>, sequence_number: usize) -> u64 {
        let sequence = &self.sequences[sequence_number];
        sequence.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    /// Get the current value for the given sequence.
    pub fn sequence_current(self: Arc<Self>, sequence_number: usize) -> u64 {
        self.sequences[sequence_number].load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Update the given sequence to `value` iff `value` is greater than the current value.
    pub fn update_sequence_max(self: Arc<Self>, sequence_number: usize, value: u64) {
        let sequence = &self.sequences[sequence_number];
        loop {
            let current = sequence.load(std::sync::atomic::Ordering::SeqCst);
            if sequence
                .compare_exchange(
                    current,
                    std::cmp::max(current, value),
                    std::sync::atomic::Ordering::SeqCst,
                    std::sync::atomic::Ordering::SeqCst,
                )
                .is_ok()
            {
                return;
            }
        }
    }

    pub fn with_relation<R, F: Fn(&BaseRelation) -> R>(&self, relation_id: RelationId, f: F) -> R {
        f(self
            .canonical
            .get(relation_id.0)
            .expect("No such relation")
            .load()
            .as_ref())
    }

    /// Prepare a commit set for the given transaction. This will scan through the transaction's
    /// working set, and for each tuple, check to see if it's safe to commit. If it is, then we'll
    /// add it to the commit set.
    pub(crate) fn prepare_commit_set(
        &self,
        commit_ts: u64,
        tx_working_set: &mut WorkingSet,
    ) -> Result<(CommitSet, MutexGuard<()>), CommitError> {
        let mut commitset = CommitSet::new(commit_ts);
        let commit_guard = self.commit_lock.lock().unwrap();

        for (_, local_relation) in tx_working_set.relations.iter_mut() {
            let relation_id = local_relation.id;
            // scan through the local working set, and for each tuple, check to see if it's safe to
            // commit. If it is, then we'll add it to the commit set.
            // note we're not actually committing yet, just producing a candidate commit set
            let canonical = &self.canonical[relation_id.0].load();
            for mut tuple in local_relation.tuples_mut() {
                // Look for the most recent version for this domain in the canonical relation.
                let canon_tuple = canonical.seek_by_domain(tuple.domain().clone());

                // If there's no value there, and our local is not tombstoned and we're not doing
                // an insert -- that's already a conflict.
                // Otherwise we can straight-away insert into the canonical base relation.
                let Some(cv) = canon_tuple else {
                    match &mut tuple {
                        TxTuple::Insert(t) => {
                            t.update_timestamp(commit_ts);
                            let forked_relation = commitset.fork(relation_id, canonical);
                            forked_relation.insert_tuple(t.clone());
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
                if tuple.ts() == commit_ts {
                    return Err(CommitError::TupleVersionConflict);
                };

                // Check the timestamp on the value, if it's newer than the read-timestamp,
                // we have for this tuple then that's a conflict, because it means someone else has
                // already committed a change to this tuple.
                if cv.ts() > tuple.ts() {
                    return Err(CommitError::TupleVersionConflict);
                }

                // Otherwise apply the change into a new canonical relation, which is a CoW
                // branching of the old one.
                let forked_relation = commitset.fork(relation_id, canonical);
                match &mut tuple {
                    TxTuple::Update(_, t) => {
                        t.update_timestamp(commit_ts);
                        let forked_relation = commitset.fork(relation_id, canonical);
                        forked_relation.update_tuple(cv.id(), t.clone());
                    }
                    TxTuple::Insert(_) => {
                        panic!("Unexpected insert");
                    }
                    TxTuple::Value(..) => {}
                    TxTuple::Tombstone {
                        ts: _,
                        domain: k,
                        tuple_id: _,
                    } => {
                        forked_relation.remove_by_domain(k.clone());
                    }
                }
            }
        }
        Ok((commitset, commit_guard))
    }

    /// Actually commit a transaction's (approved) CommitSet. If the underlying base relations have
    /// changed since the commit set began, this will return `Err(RelationContentionConflict)`
    /// and the transaction can choose to try to produce a new CommitSet, or just abort.
    pub(crate) fn try_commit(
        &self,
        commit_set: CommitSet,
        _commit_guard: MutexGuard<()>,
    ) -> Result<(), CommitError> {
        // swap the active canonical state with the new one
        // but only if the timestamps have not changed in the interim;
        // we have to hold a lock while this is checked. If any relations have
        // had their ts change, we probably need to retry because someone else got there first
        // This shouldn't happen if we have a commit guard.
        for (_, relation) in commit_set.iter() {
            // Did the relation get committed to by someone else in the interim? If so, return
            // back to the transaction letting it know that, and it can decide if it wants to
            // retry.
            // This shouldn't happen if we have a commit guard.
            if relation.ts != self.canonical[relation.id.0].load().ts {
                return Err(CommitError::RelationContentionConflict);
            }
        }

        // Everything passed, so we can commit the changes by swapping in the new canonical
        // before releasing the lock.
        let commit_ts = commit_set.ts;
        for (_, mut relation) in commit_set.into_iter() {
            let idx = relation.id.0;

            relation.ts = commit_ts;
            self.canonical[idx].store(Arc::new(relation));
        }

        Ok(())
    }

    pub fn sync(&self, ts: u64, working_set: WorkingSet) {
        let seqs = self
            .sequences
            .iter()
            .map(|s| s.load(std::sync::atomic::Ordering::SeqCst))
            .collect();
        self.pager.sync(ts, working_set, seqs);
    }

    pub fn db_usage_bytes(&self) -> usize {
        self.tuple_box.used_bytes()
    }

    pub fn shutdown(&self) {
        self.pager.shutdown();
    }
}
