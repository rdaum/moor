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

// TODO: 'join' and transitive closure on db relations
//   -> datalog-style variable unification
//   can be used for some of the inheritance graph / verb & property resolution activity done manually now

use crate::base_relation::BaseRelation;
use crate::index::{AttrType, IndexType};
use crate::paging::TupleBox;
use crate::tx::WorkingSet;
use crate::tx::{CommitError, CommitSet, Transaction};
use crate::RelationId;
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, RwLock};

use super::paging::Pager;

/// Meta-data about a relation
#[derive(Clone, Debug)]
pub struct RelationInfo {
    /// Human readable name of the relation.
    pub name: String,
    /// The domain type ID, which is user defined in the client's type system, and not enforced
    pub domain_type: AttrType,
    /// The codomain type ID, which is user defined in the client's type system, and not enforced
    pub codomain_type: AttrType,
    /// Whether or not this relation has a secondary index on its codomain.
    pub secondary_indexed: bool,
    /// Whether the domain is assumed to be uniquely constrained.
    pub unique_domain: bool,
    /// Type of index to use for this relation.
    pub index_type: IndexType,
    /// Type of the codomain index (only used if `secondary_indexed` is true)
    pub codomain_index_type: Option<IndexType>,
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
    canonical: RwLock<Vec<BaseRelation>>,

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
        let pager = Arc::new(Pager::new(memory_size).expect(
            "Unable to create pager. You may need to set /proc/sys/vm/overcommit_memory to '1'",
        ));
        let tuple_box = Arc::new(TupleBox::new(pager.clone()));
        let mut base_relations = Vec::with_capacity(relations.len());
        for (rid, r) in relations.iter().enumerate() {
            base_relations.push(BaseRelation::new(RelationId(rid), r.clone(), 0));
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
            canonical: RwLock::new(base_relations),
            sequences,
            tuple_box,
            pager,
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
        let rl = self.canonical.read().unwrap();
        f(rl.get(relation_id.0).unwrap())
    }

    /// Prepare a commit set for the given transaction. This will scan through the transaction's
    /// working set, and for each tuple, check to see if it's safe to commit. If it is, then we'll
    /// add it to the commit set.
    pub(crate) fn prepare_commit_set(
        &self,
        commit_ts: u64,
        tx_working_set: &mut WorkingSet,
    ) -> Result<CommitSet, CommitError> {
        // The lock belongs to the transaction now now.
        let canonical_lock = self.canonical.write().unwrap();
        let mut commitset = CommitSet::new(commit_ts, canonical_lock);
        commitset.prepare(tx_working_set)?;
        Ok(commitset)
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

    /// Get a copy of the set of the database's canonical relations (generally used for
    /// testing purposes only.)
    pub fn copy_canonical(&self) -> Vec<BaseRelation> {
        self.canonical.read().unwrap().clone()
    }
}
