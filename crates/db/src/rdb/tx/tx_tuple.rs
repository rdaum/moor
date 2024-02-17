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

use crate::rdb::paging::TupleBox;
use crate::rdb::tuples::TupleRef;
use crate::rdb::{RelationError, RelationId};
use moor_values::util::SliceRef;
use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct TxTupleEvent {
    pub op: TxTupleOp,
    pub op_source: OpSource,
    pub data_source: DataSource,
}

/// Possible operations on tuples, in the context local to a working set in a transaction.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum TxTupleOp {
    /// Insert tuple into the relation.
    Insert(TupleRef),
    /// Update an existing tuple in the relation whose domain matches.
    Update {
        from_tuple: TupleRef,
        to_tuple: TupleRef,
    },
    /// Clone/fork a tuple from the base relation into our local working set.
    Value(TupleRef),
    /// Delete the referenced tuple from the relation.
    Tombstone(TupleRef, u64),
}

impl TxTupleOp {
    #[allow(dead_code)]
    pub fn domain(&self) -> SliceRef {
        match self {
            TxTupleOp::Insert(t) => t.domain().clone(),
            TxTupleOp::Update { from_tuple, .. } => from_tuple.domain().clone(),
            TxTupleOp::Value(t) => t.domain().clone(),
            TxTupleOp::Tombstone(t, _) => t.domain().clone(),
        }
    }
}
/// What kind of operation triggered the tuple event or apply.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum OpSource {
    Seek,
    Insert,
    Update,
    Upsert,
    Remove,
}

/// The source of the data we derived the operation from.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum DataSource {
    Base,
    WorkingSet,
}

/// Represents the needed information for instructions to apply a tuple operation to the local working set relation
#[derive(Clone, Debug)]
pub struct TupleApply {
    /// The original type of the operation that started the chain.
    pub op_source: OpSource,
    /// The source of the origin tuple.
    pub data_source: DataSource,
    /// What action to take, if any.
    pub replacement_op: Option<TxTupleOp>,
    /// What the old tuple to remove/replace is, if any.
    pub del_tuple: Option<TupleRef>,
    /// What the new tuple to insert is, if any.
    pub add_tuple: Option<TupleRef>,
}

impl TxTupleOp {
    /// Returns the timestamp of the operation, which should be the transaction's own timestamp.
    /// (Not the original tuple's timestamp.)
    pub fn ts(&self) -> u64 {
        match self {
            TxTupleOp::Insert(tref)
            | TxTupleOp::Update { to_tuple: tref, .. }
            | TxTupleOp::Value(tref) => tref.ts(),
            TxTupleOp::Tombstone(_, ts) => *ts,
        }
    }
}

impl TxTupleEvent {
    /// Given a new domain & codomain, return the replacement
    /// operation, the new tupe ref to insert, and the old tuple ref
    /// to remove.
    pub fn transform_to_update(
        &self,
        domain: &SliceRef,
        codomain: &SliceRef,
        tuple_box: Arc<TupleBox>,
        relation_id: RelationId,
        unique_domain: bool,
    ) -> Result<Option<TupleApply>, RelationError> {
        let apply = match &self.op {
            TxTupleOp::Tombstone(_, _) => {
                if unique_domain {
                    return Err(RelationError::TupleNotFound);
                }
                return Ok(None);
            }
            TxTupleOp::Insert(t) => {
                let new_t = TupleRef::allocate(
                    relation_id,
                    tuple_box.clone(),
                    t.ts(),
                    domain.as_slice(),
                    codomain.as_slice(),
                )
                .unwrap();

                self.fork_to(
                    Some(TxTupleOp::Insert(new_t.clone())),
                    Some(new_t),
                    Some(t.clone()),
                )
            }
            TxTupleOp::Update {
                from_tuple,
                to_tuple,
            } => {
                let new_t = TupleRef::allocate(
                    relation_id,
                    tuple_box.clone(),
                    to_tuple.ts(),
                    domain.as_slice(),
                    codomain.as_slice(),
                )
                .unwrap();
                self.fork_to(
                    Some(TxTupleOp::Update {
                        from_tuple: from_tuple.clone(),
                        to_tuple: new_t.clone(),
                    }),
                    Some(new_t),
                    Some(to_tuple.clone()),
                )
            }
            TxTupleOp::Value(t) => {
                let new_t = TupleRef::allocate(
                    relation_id,
                    tuple_box.clone(),
                    t.ts(),
                    domain.as_slice(),
                    codomain.as_slice(),
                )
                .unwrap();
                let tuple_ref = new_t.clone();
                self.fork_to(
                    Some(TxTupleOp::Update {
                        from_tuple: t.clone(),
                        to_tuple: tuple_ref,
                    }),
                    Some(new_t),
                    Some(t.clone()),
                )
            }
        };
        Ok(Some(apply))
    }

    /// Given a new domain & codomain, return the replacement operation, the new tuple to insert, and the old tuple to
    /// remove, if any.
    pub fn transform_to_upsert(
        &self,
        domain: &SliceRef,
        codomain: &SliceRef,
        tuple_box: Arc<TupleBox>,
        relation_id: RelationId,
    ) -> Result<TupleApply, RelationError> {
        let apply = match &self.op {
            TxTupleOp::Insert(t) => {
                let new_t = TupleRef::allocate(
                    relation_id,
                    tuple_box.clone(),
                    t.ts(),
                    domain.as_slice(),
                    codomain.as_slice(),
                )
                .unwrap();
                self.fork_to(
                    Some(TxTupleOp::Insert(new_t.clone())),
                    Some(new_t),
                    Some(t.clone()),
                )
            }
            TxTupleOp::Tombstone(tombstoned_tuple, ts) => {
                // We need to allocate a new tuple to replace the tombstone...
                let new_t = TupleRef::allocate(
                    relation_id,
                    tuple_box.clone(),
                    *ts,
                    domain.as_slice(),
                    codomain.as_slice(),
                )
                .unwrap();
                // TODO: do I need to pick Insert if the tombstone is Base-derived?
                self.fork_to(
                    Some(TxTupleOp::Update {
                        from_tuple: tombstoned_tuple.clone(),
                        to_tuple: new_t.clone(),
                    }),
                    Some(new_t),
                    Some(tombstoned_tuple.clone()),
                )
            }
            TxTupleOp::Update {
                from_tuple,
                to_tuple,
            } => {
                let new_t = TupleRef::allocate(
                    relation_id,
                    tuple_box.clone(),
                    to_tuple.ts(),
                    domain.as_slice(),
                    codomain.as_slice(),
                )
                .unwrap();
                self.fork_to(
                    Some(TxTupleOp::Update {
                        from_tuple: from_tuple.clone(),
                        to_tuple: new_t.clone(),
                    }),
                    Some(new_t),
                    Some(from_tuple.clone()),
                )
            }
            TxTupleOp::Value(tuple) => {
                let new_t = TupleRef::allocate(
                    relation_id,
                    tuple_box.clone(),
                    tuple.ts(),
                    domain.as_slice(),
                    codomain.as_slice(),
                )
                .unwrap();
                self.fork_to(
                    Some(TxTupleOp::Update {
                        from_tuple: tuple.clone(),
                        to_tuple: new_t.clone(),
                    }),
                    Some(new_t),
                    Some(tuple.clone()),
                )
            }
        };
        Ok(apply)
    }

    pub fn transform_to_remove(&self) -> Result<Option<TupleApply>, RelationError> {
        let apply = match &self.op {
            // Inserts are removed.
            TxTupleOp::Insert(t) => Some(self.fork_to(None, None, Some(t.clone()))),
            // If it was an update or echo'd value, we can just replace it with a tombstone.
            TxTupleOp::Update { to_tuple: t, .. } | TxTupleOp::Value(t) => Some(self.fork_to(
                Some(TxTupleOp::Tombstone(t.clone(), t.ts())),
                Some(t.clone()),
                Some(t.clone()),
            )),
            // If it was a tombstone, error NotFound.
            TxTupleOp::Tombstone(_, _) => {
                return Err(RelationError::TupleNotFound);
            }
        };
        Ok(apply)
    }

    fn fork_to(
        &self,
        replacement_op: Option<TxTupleOp>,
        add_tuple: Option<TupleRef>,
        del_tuple: Option<TupleRef>,
    ) -> TupleApply {
        let apply = TupleApply {
            data_source: DataSource::WorkingSet,
            op_source: self.op_source.clone(),
            replacement_op,
            add_tuple,
            del_tuple,
        };

        // verify the domains all match, if present
        if let (Some(add_tuple), Some(del_tuple)) = (&apply.add_tuple, &apply.del_tuple) {
            assert_eq!(
                add_tuple.domain(),
                del_tuple.domain(),
                "domain mismatch for {:?} => {:?}",
                add_tuple,
                del_tuple
            );
        }

        apply
    }
}
