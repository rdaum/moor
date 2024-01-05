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

use crate::tuplebox::tuples::TupleId;
use crate::tuplebox::tuples::TupleRef;
use moor_values::util::slice_ref::SliceRef;

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
    Tombstone {
        ts: u64,
        tuple_id: TupleId,
        domain: SliceRef,
    },
}

impl TxTuple {
    pub fn domain(&self) -> SliceRef {
        match self {
            TxTuple::Insert(tref) | TxTuple::Update(tref) | TxTuple::Value(tref) => tref.domain(),
            TxTuple::Tombstone {
                ts: _,
                tuple_id: _,
                domain: d,
            } => d.clone(),
        }
    }
    pub fn tuple_id(&self) -> TupleId {
        match self {
            TxTuple::Insert(tref) | TxTuple::Update(tref) | TxTuple::Value(tref) => tref.id(),
            TxTuple::Tombstone {
                ts: _,
                tuple_id: id,
                domain: _,
            } => *id,
        }
    }

    pub fn ts(&self) -> u64 {
        match self {
            TxTuple::Insert(tref) | TxTuple::Update(tref) | TxTuple::Value(tref) => tref.ts(),
            TxTuple::Tombstone {
                ts,
                tuple_id: _,
                domain: _,
            } => *ts,
        }
    }
}
