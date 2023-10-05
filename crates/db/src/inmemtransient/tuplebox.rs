// TODO: secondary (codomain) indices.
// TODO: garbage collect old versions.
// TODO: support sorted indices, too.
// TODO: 'join' and transitive closure -> datalog-style variable unification
// TODO: persistence: bare minimum is a WAL to some kind of backup state that can be re-read on
//  startup. maximal is fully paged storage.

use crate::inmemtransient::base_relation::BaseRelation;
use crate::inmemtransient::transaction::{
    CommitError, CommitSet, Transaction, TupleOperation, WorkingSet,
};
use std::collections::HashSet;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Meta-data q about a relation
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
// TODO: prune old versions, background thread.
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
    // TODO: this is a candidate for an optimistic lock.
    sequences: RwLock<Vec<u64>>,
    /// The copy-on-write set of current canonical base relations.
    // TODO: this is a candidate for an optimistic lock.
    pub(crate) canonical: RwLock<Vec<BaseRelation>>,
}

impl TupleBox {
    pub fn new(relations: &[RelationInfo], num_sequences: usize) -> Arc<Self> {
        let mut base_relations = Vec::with_capacity(relations.len());
        for r in relations {
            base_relations.push(BaseRelation::new(0));
            if r.secondary_indexed {
                base_relations.last_mut().unwrap().add_secondary_index();
            }
        }
        Arc::new(Self {
            relation_info: relations.to_vec(),
            maximum_transaction: AtomicU64::new(0),
            active_transactions: RwLock::new(HashSet::new()),
            canonical: RwLock::new(base_relations),
            sequences: RwLock::new(vec![0; num_sequences]),
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
        Transaction::new(next_ts, self.clone())
    }

    /// Get the next value for the given sequence.
    pub async fn sequence_next(self: Arc<Self>, sequence_number: usize) -> u64 {
        let mut sequences = self.sequences.write().await;
        let next = sequences[sequence_number];
        sequences[sequence_number] += 1;
        next
    }

    /// Get the current value for the given sequence.
    pub async fn sequence_current(self: Arc<Self>, sequence_number: usize) -> u64 {
        let sequences = self.sequences.read().await;
        sequences[sequence_number]
    }

    /// Update the given sequence to `value` iff `value` is greater than the current value.
    pub async fn update_sequence_max(self: Arc<Self>, sequence_number: usize, value: u64) {
        let mut sequences = self.sequences.write().await;
        if value > sequences[sequence_number] {
            sequences[sequence_number] = value;
        }
    }
    pub async fn with_relation<R, F: Fn(&BaseRelation) -> R>(&self, relation_id: usize, f: F) -> R {
        f(self
            .canonical
            .read()
            .await
            .get(relation_id)
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
        let mut commitset = CommitSet::new(tx_ts, tx_working_set.local_relations.len());

        for (relation_id, local_relation) in tx_working_set.local_relations.iter().enumerate() {
            // scan through the local working set, and for each tuple, check to see if it's safe to
            // commit. If it is, then we'll add it to the commit set.
            // note we're not actually committing yet, just producing a candidate commit set
            let canonical = &self.canonical.read().await[relation_id];
            for (k, v) in local_relation.domain_index.iter() {
                let cv = canonical.seek_by_domain(k);

                // If there's no value there, and our local is not tombstoned and we're not doing
                // an insert that's already a conflict.
                // Otherwise we have to straight-away insert into the canonical base relation.
                // TODO: it should be possible to do this without having the fork logic exist twice
                //   here.
                let Some(cv) = cv else {
                    match &v.t {
                        TupleOperation::Insert(value) => {
                            let forked_relation = commitset.fork(relation_id, &canonical);
                            forked_relation.upsert_tuple(k.clone(), tx_ts, value.clone());
                            continue;
                        }
                        TupleOperation::Tombstone => {
                            // We let this pass, as this must be a delete of something we inserted
                            // temporarily previously in our transaction.
                            continue;
                        }
                        TupleOperation::Update(_) | TupleOperation::Value(_) => {
                            return Err(CommitError::TupleVersionConflict);
                        }
                    }
                };

                // If there's no timestamp on the value in ours, but there *is* a value in
                // canonical, that's a conflict, because it means someone else has already
                // committed a change to this tuple that we thought was net-new.
                let Some(ts) = v.ts else {
                    return Err(CommitError::TupleVersionConflict);
                };

                // Check the timestamp on the value, if it's newer than the read-timestamp
                // we have for this tuple (or if we don't have one because net-new), then
                // that's conflict, because it means someone else has already committed a
                // change to this key.
                if cv.ts > ts {
                    return Err(CommitError::TupleVersionConflict);
                }

                // Otherwise apply the change into a new canonical relation, which is a CoW
                // branching of the old one.
                let forked_relation = commitset.fork(relation_id, &canonical);
                match &v.t {
                    TupleOperation::Insert(val) | TupleOperation::Update(val) => {
                        forked_relation.upsert_tuple(k.clone(), tx_ts, val.clone());
                    }
                    TupleOperation::Value(_) => {}
                    TupleOperation::Tombstone => {
                        forked_relation.remove_by_domain(k);
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
        for (relation_id, relation) in commit_set.relations.iter().enumerate() {
            if let Some(relation) = relation {
                // Did the relation get committed to by someone else in the interim? If so, return
                // back to the transaction letting it know that, and it can decide if it wants to
                // retry.
                if relation.ts != canonical[relation_id].ts {
                    return Err(CommitError::RelationContentionConflict);
                }
            }
        }

        // Everything passed, so we can commit the changes by swapping in the new canonical
        // before releasing the lock.
        for (relation_id, relation) in commit_set.relations.into_iter().enumerate() {
            if let Some(relation) = relation {
                canonical[relation_id] = relation;
                // And update the timestamp on the canonical relation.
                canonical[relation_id].ts = commit_set.ts;
            }
        }
        // Clear out the active transaction.
        self.active_transactions
            .write()
            .await
            .remove(&commit_set.ts);

        // TODO: write to WAL here.
        Ok(())
    }

    pub(crate) async fn abort_transaction(&self, ts: u64) {
        self.active_transactions.write().await.remove(&ts);
    }
}
