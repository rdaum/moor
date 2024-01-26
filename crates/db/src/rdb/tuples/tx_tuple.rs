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

use moor_values::util::SliceRef;

use crate::rdb::tuples::TupleId;
use crate::rdb::tuples::TupleRef;

/// Possible operations on tuples, in the context local to a transaction.
#[derive(Clone)]
pub enum TxTuple {
    /// Insert tuple into the relation.
    Insert(TupleRef),
    /// Update an existing tuple in the relation whose domain matches.
    Update(TupleId, TupleRef),
    /// Clone/fork a tuple from the base relation into our local working set.
    Value(TupleRef),
    /// Delete the tuple.
    Tombstone {
        ts: u64,
        tuple_id: TupleId,
        domain: SliceRef,
    },
}

impl TxTuple {
    pub fn domain(&self) -> SliceRef {
        match self {
            TxTuple::Insert(tref) | TxTuple::Update(_, tref) | TxTuple::Value(tref) => {
                tref.domain()
            }
            TxTuple::Tombstone {
                ts: _,
                tuple_id: _,
                domain: d,
            } => d.clone(),
        }
    }

    /// Return the "origin" tuple id for this tuple, which is the tuple id of the tuple that
    /// this tuple was forked from, if any.  Inserts do not have an origin tuple id, as they
    /// are not forked from any existing tuple.
    pub fn origin_tuple_id(&self) -> TupleId {
        match self {
            TxTuple::Update(id, _) => *id,
            TxTuple::Value(tref) => tref.id(),
            TxTuple::Tombstone {
                ts: _,
                tuple_id: id,
                domain: _,
            } => *id,
            TxTuple::Insert(_) => panic!("Inserts do not have an origin tuple id"),
        }
    }

    pub fn ts(&self) -> u64 {
        match self {
            TxTuple::Insert(tref) | TxTuple::Update(_, tref) | TxTuple::Value(tref) => tref.ts(),
            TxTuple::Tombstone {
                ts,
                tuple_id: _,
                domain: _,
            } => *ts,
        }
    }
}
