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

use std::collections::HashSet;

use moor_values::util::SliceRef;

use crate::rdb::tuples::TupleRef;
use crate::rdb::tx::transaction::Transaction;
use crate::rdb::{RelationError, RelationId};

/// A reference / handle / pointer to a relation, the actual operations are managed through the
/// transaction.
/// A more convenient handle tied to the lifetime of the transaction.
pub struct RelVar<'a> {
    pub(crate) tx: &'a Transaction,
    pub(crate) id: RelationId,
}

impl<'a> RelVar<'a> {
    /// Seek for a tuple by its indexed domain value.
    pub fn seek_by_domain(&self, domain: SliceRef) -> Result<HashSet<TupleRef>, RelationError> {
        self.tx.seek_by_domain(self.id, domain)
    }

    /// Seek for a tuple by its indexed domain value.
    pub fn seek_unique_by_domain(&self, domain: SliceRef) -> Result<TupleRef, RelationError> {
        self.tx.seek_unique_by_domain(self.id, domain)
    }

    /// Seek for tuples by their indexed codomain value, if there's an index. Panics if there is no
    /// secondary index.
    pub fn seek_by_codomain(&self, codomain: SliceRef) -> Result<HashSet<TupleRef>, RelationError> {
        self.tx.seek_by_codomain(self.id, codomain)
    }

    /// Insert a tuple into the relation.
    pub fn insert_tuple(&self, domain: SliceRef, codomain: SliceRef) -> Result<(), RelationError> {
        self.tx.insert_tuple(self.id, domain, codomain)
    }

    /// Update a tuple in the relation.
    pub fn update_by_domain(
        &self,
        domain: SliceRef,
        codomain: SliceRef,
    ) -> Result<(), RelationError> {
        self.tx.update_by_domain(self.id, domain, codomain)
    }

    /// Upsert a tuple into the relation.
    pub fn upsert_by_domain(
        &self,
        domain: SliceRef,
        codomain: SliceRef,
    ) -> Result<(), RelationError> {
        self.tx.upsert_by_domain(self.id, domain, codomain)
    }

    /// Remove a tuple from the relation.
    pub fn remove_by_domain(&self, domain: SliceRef) -> Result<(), RelationError> {
        self.tx.remove_by_domain(self.id, domain)
    }

    pub fn predicate_scan<F: Fn(&TupleRef) -> bool>(
        &self,
        f: &F,
    ) -> Result<Vec<TupleRef>, RelationError> {
        self.tx.predicate_scan(self.id, f)
    }
}
