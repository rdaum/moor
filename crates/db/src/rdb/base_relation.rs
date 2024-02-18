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

use crate::rdb::index::{ImHashIndex, Index};
use crate::rdb::tuples::{TupleId, TupleRef};
use crate::rdb::{RelationError, RelationId, RelationInfo};

/// Represents a 'canonical' base binary relation, which is a set of tuples of domain, codomain,
/// with a default (hash) index on the domain and an optional (hash) index on the codomain.
///
/// In this representation we do not differentiate the Domain & Codomain type; they are
/// stored and managed as raw byte-arrays and it is up to layers above to interpret the the values
/// correctly.
///
// TODO: Add some kind of 'type' flag to the relation & tuple values,
//   so that we can do type-checking on the values, though for our purposes this may be overkill at this time.
// TODO: Indexes should be paged.
// TODO: support ordered indexes, not just hash indexes.
//   if we're staying with in-memory, use an Adaptive Radix Tree; my implementation, but hopefully
//   modified to support CoW/shared ownership of the tree nodes, like the im::HashMap does.
//   if we're going to support on-disk indexes, use a CoW B+Tree, which I have implemented elsewhere,
//   but will need to bring in here, optimize, and provide loving care to.
#[derive(Clone)]
pub struct BaseRelation {
    pub(crate) id: RelationId,

    /// The last successful committer's tx timestamp
    pub(crate) ts: u64,

    pub(crate) unique_domain: bool,

    /// All the tuples in this relation. Indexed by their TupleId, so we can map back from the values in the indexes.
    tuples: im::HashMap<TupleId, TupleRef>,

    index: Box<dyn Index + Send + Sync>,
}

impl Clone for Box<dyn Index + Send + Sync> {
    fn clone(&self) -> Self {
        self.clone_index()
    }
}

impl BaseRelation {
    pub(crate) fn new(id: RelationId, relation_info: RelationInfo, timestamp: u64) -> Self {
        let unique_domain = relation_info.unique_domain;
        Self {
            id,
            ts: timestamp,
            tuples: Default::default(),
            index: Box::new(ImHashIndex::new(relation_info)),
            unique_domain,
        }
    }

    /// Establish indexes for a tuple initial-loaded from secondary storage. Basically a, "trust us,
    /// this exists" move.
    pub(crate) fn index_tuple(&mut self, mut tuple: TupleRef) {
        // Reset timestamp to 0, since this is a tuple initial-loaded from secondary storage.
        tuple.update_timestamp(0);

        self.index.index_tuple(&tuple).expect("Indexing failed");

        // Add the tuple to the relation.
        self.tuples.insert(tuple.id(), tuple);
    }

    /// Check for a specific tuple by its id.
    pub(crate) fn has_tuple(&self, tuple: &TupleId) -> bool {
        self.tuples.contains_key(tuple)
    }

    pub fn check_domain_constraints(&self, domain: &SliceRef) -> Result<(), RelationError> {
        self.index.check_domain_constraints(domain)
    }

    pub fn seek_by_domain(&self, domain: SliceRef) -> HashSet<TupleRef> {
        self.index
            .seek_domain(&domain)
            .map(|id| self.tuples.get(&id).unwrap().clone())
            .collect()
    }

    pub fn seek_by_codomain(&self, codomain: SliceRef) -> HashSet<TupleRef> {
        self.index
            .seek_codomain(&codomain)
            .map(|id| self.tuples.get(&id).unwrap().clone())
            .collect()
    }

    pub fn predicate_scan<F: Fn(&TupleRef) -> bool>(&self, f: &F) -> HashSet<TupleRef> {
        self.tuples.values().filter(|t| f(t)).cloned().collect()
    }

    /// Remove a specific tuple from the relation, and update indexes accordingly.
    pub(crate) fn remove_tuple(&mut self, tuple: &TupleId) -> Result<(), RelationError> {
        let Some(tuple_ref) = self.tuples.remove(tuple) else {
            return Err(RelationError::TupleNotFound);
        };

        self.index.unindex_tuple(&tuple_ref);

        Ok(())
    }

    /// Add a net-new tuple into the relation, and update indexes accordingly.
    pub(crate) fn insert_tuple(&mut self, tuple: TupleRef) -> Result<(), RelationError> {
        {
            // We're a set not a bag.
            // There's gotta be a more efficient way to do this.
            let domain_entries = self.index.seek_domain(&tuple.domain());
            let mut domain_tuples = domain_entries.map(|id| self.tuples.get(&id).unwrap());
            if domain_tuples.any(|t| t == &tuple) {
                return Err(RelationError::UniqueConstraintViolation);
            }
        }
        self.index.index_tuple(&tuple)?;
        self.tuples.insert(tuple.id(), tuple.clone());

        Ok(())
    }

    /// Update a tuple that already exists in the relation, and update indexes accordingly.
    pub(crate) fn update_tuple(
        &mut self,
        old_tuple: &TupleId,
        tuple: TupleRef,
    ) -> Result<(), RelationError> {
        let Some(old_tref) = self.tuples.remove(old_tuple) else {
            return Err(RelationError::TupleNotFound);
        };
        // Remove the old tuple from the domain index (and codomain index if it exists)
        self.index.unindex_tuple(&old_tref);

        // Add the new tuple to the domain index
        self.index.index_tuple(&tuple)?;

        self.tuples.insert(tuple.id(), tuple.clone());

        Ok(())
    }
}
