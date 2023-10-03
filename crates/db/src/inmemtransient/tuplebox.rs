// TODO: secondary (codomain) indices.
// TODO: garbage collect old versions.
// TODO: support sorted indices, too.
// TODO: 'join' and transitive closure -> datalog-style variable unification
// TODO: persistence: bare minimum is a WAL to some kind of backup state that can be re-read on
//  startup. maximal is fully paged storage.

use crate::inmemtransient::transaction::Transaction;
use moor_values::util::slice_ref::SliceRef;
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, AtomicUsize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Represents a 'canonical' base binary relation, which is a set of tuples of domain -> codomain.
/// In this representation we do not differentiate the Domain & Codomain type; they are
/// stored and managed as raw byte-arrays and it is up to layers above to interpret the the values
/// correctly.
// TODO: Add some kind of 'type' flag to the relation & tuple values, so that we can do
//   type-checking on the values, though for our purposes this may be overkill at this time.
#[derive(Clone)]
pub(crate) struct Relation {
    /// The last successful committer's tx timestamp
    pub(crate) ts: u64,
    /// The tuples in this relation, which are in this case expressed purely as bytes.
    /// It is up to the caller to interpret them.
    pub(crate) tuples: im::HashMap<Vec<u8>, TupleValue>,
}

/// Storage for individual tuple codomain values in a relation, which includes the timestamp of
/// the last successful committer and the raw byte arrays for the value.
#[derive(Clone)]
pub(crate) struct TupleValue {
    pub(crate) ts: u64,
    pub(crate) v: SliceRef,
}

/// The tuplebox is the set of relations, referenced by their unique (usize) relation ID.
/// It exposes interfaces for starting & managing transactions.
/// It is, essentially, a micro database.
// TODO: prune old versions, background thread.
// TODO: locking in general here is (probably) safe, but not optimal. optimistic locking would be
//   better for various portions here.
pub struct TupleBox {
    /// The number of relations ; cached here so that we don't have to take a lock on `canonical`
    /// every time we want to know how many relations there are.
    pub(crate) number_relations: AtomicUsize,
    /// The monotonically increasing transaction ID "timestamp" counter.
    // TODO: take a look at Adnan's thread-sharded approach described in section 3.1
    //   (https://www.vldb.org/pvldb/vol16/p1426-alhomssi.pdf) -- "Ordered Snapshot Instant Commit"
    maximum_transaction: AtomicU64,
    /// The set of currently active transactions, which will be used to prune old unreferenced
    /// versions of tuples.
    pub(crate) active_transactions: RwLock<HashSet<u64>>,
    /// Monotonically incrementing sequence counters.
    // TODO: this is a candidate for an optimistic lock.
    sequences: RwLock<Vec<u64>>,
    /// The copy-on-write set of current canonical base relations.
    // TODO: this is a candidate for an optimistic lock.
    pub(crate) canonical: RwLock<Vec<Relation>>,
}

impl TupleBox {
    pub fn new(num_relations: usize, num_sequences: usize) -> Arc<Self> {
        Arc::new(Self {
            number_relations: AtomicUsize::new(num_relations),
            maximum_transaction: AtomicU64::new(0),
            active_transactions: RwLock::new(HashSet::new()),
            canonical: RwLock::new(vec![
                Relation {
                    ts: 0,
                    tuples: im::HashMap::new()
                };
                num_relations
            ]),
            sequences: RwLock::new(vec![0; num_sequences]),
        })
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
}
