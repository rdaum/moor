use thiserror::Error;

use crate::tuplebox::tuples::TupleRef;
use moor_values::util::slice_ref::SliceRef;

#[derive(Debug, Clone, Eq, PartialEq, Error)]
pub enum TupleError {
    #[error("Tuple not found")]
    NotFound,
    #[error("Tuple already exists")]
    Duplicate,
}

/// Possible operations on tuples, in the context of a transaction .
#[derive(Clone)]
pub enum TxTuple {
    /// Insert tuple into the relation.
    Insert(TupleRef),
    /// Update an existing tuple in the relation whose domain matches.
    Update(TupleRef),
    /// Clone/fork a tuple from the base relation into our local working set.
    Value(TupleRef),
    /// Delete the tuple.
    Tombstone { ts: u64, domain: SliceRef },
}

impl TxTuple {
    pub fn domain(&self) -> SliceRef {
        match self {
            TxTuple::Insert(tref) | TxTuple::Update(tref) | TxTuple::Value(tref) => {
                tref.get().domain()
            }
            TxTuple::Tombstone { ts: _, domain: d } => d.clone(),
        }
    }
    pub fn ts(&self) -> u64 {
        match self {
            TxTuple::Insert(tref) | TxTuple::Update(tref) | TxTuple::Value(tref) => tref.get().ts(),
            TxTuple::Tombstone { ts, domain: _ } => *ts,
        }
    }
}
