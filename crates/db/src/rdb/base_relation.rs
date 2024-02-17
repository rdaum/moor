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

use crate::rdb::tuples::{TupleId, TupleRef};
use crate::rdb::{RelationError, RelationId};

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

    /// The domain-indexed tuples in this relation, which are in this case expressed purely as bytes.
    /// It is up to the caller to interpret them.
    index_domain: im::HashMap<SliceRef, im::HashSet<TupleId>>,

    /// Optional reverse index from codomain -> tuples, which is used to support (more) efficient
    /// reverse lookups.
    index_codomain: Option<im::HashMap<SliceRef, im::HashSet<TupleId>>>,
}

impl BaseRelation {
    pub(crate) fn new(id: RelationId, unique_domain: bool, timestamp: u64) -> Self {
        Self {
            id,
            ts: timestamp,
            tuples: Default::default(),
            index_domain: im::HashMap::new(),
            index_codomain: None,
            unique_domain,
        }
    }
    /// Add a secondary index onto the given relation to map its codomain back to its domain.
    /// If the relation already has a secondary index, this will panic.
    /// If there is no relation with the given ID, this will panic.
    /// If the relation already has tuples, they will be indexed.
    pub(crate) fn add_secondary_index(&mut self) {
        if self.index_codomain.is_some() {
            panic!("Relation already has a secondary index");
        }
        self.index_codomain = Some(im::HashMap::new());
        for (_, tuple) in &self.tuples {
            // ... update the secondary index.
            self.index_codomain
                .as_mut()
                .unwrap()
                .entry(tuple.codomain())
                .or_default()
                .insert(tuple.id());
        }
    }

    /// Establish indexes for a tuple initial-loaded from secondary storage. Basically a, "trust us,
    /// this exists" move.
    pub(crate) fn index_tuple(&mut self, mut tuple: TupleRef) {
        let tref = tuple.clone();

        // Reset timestamp to 0, since this is a tuple initial-loaded from secondary storage.
        tuple.update_timestamp(0);

        // Add the tuple to the relation.
        self.tuples.insert(tref.id(), tref);

        // Update the domain index to point to the tuple...
        self.index_domain
            .entry(tuple.domain())
            .or_default()
            .insert(tuple.id());

        // ... and update the secondary index if there is one.
        if let Some(index) = &mut self.index_codomain {
            index
                .entry(tuple.codomain())
                .or_default()
                .insert(tuple.id());
        }
    }

    /// Check for a specific tuple by its id.
    pub(crate) fn has_tuple(&self, tuple: &TupleId) -> bool {
        self.tuples.contains_key(tuple)
    }

    pub fn seek_by_domain(&self, domain: SliceRef) -> HashSet<TupleRef> {
        if let Some(tuple_ids) = &self.index_domain.get(&domain) {
            return tuple_ids
                .iter()
                .map(|id| {
                    self.tuples
                        .get(id)
                        .expect("indexed tuple missing from relation")
                })
                .cloned()
                .collect();
        }

        HashSet::new()
    }

    pub fn seek_by_codomain(&self, codomain: SliceRef) -> HashSet<TupleRef> {
        // Attempt to seek on codomain without an index is a panic.
        // We could do full-scan, but in this case we're going to assume that the caller knows
        // what they're doing.
        let codomain_index = self.index_codomain.as_ref().expect("No codomain index");
        if let Some(tuple_ids) = codomain_index.get(&codomain) {
            return tuple_ids
                .iter()
                .map(|id| {
                    self.tuples
                        .get(id)
                        .expect("indexed tuple missing from relation")
                })
                .cloned()
                .collect();
        } else {
            HashSet::new()
        }
    }

    pub fn predicate_scan<F: Fn(&TupleRef) -> bool>(&self, f: &F) -> HashSet<TupleRef> {
        self.tuples.values().filter(|t| f(t)).cloned().collect()
    }

    /// Remove a specific tuple from the relation, and update indexes accordingly.
    pub(crate) fn remove_tuple(&mut self, tuple: &TupleId) -> Result<(), RelationError> {
        let Some(tuple_ref) = self.tuples.remove(tuple) else {
            return Err(RelationError::TupleNotFound);
        };

        self.index_domain
            .get_mut(&tuple_ref.domain())
            .unwrap()
            .remove(tuple)
            .unwrap();
        if let Some(index) = &mut self.index_codomain {
            index.entry(tuple_ref.codomain()).or_default().remove(tuple);
        }

        Ok(())
    }

    /// Add a net-new tuple into the relation, and update indexes accordingly.
    pub(crate) fn insert_tuple(&mut self, tuple: TupleRef) -> Result<(), RelationError> {
        // We're a set not a bag.
        let domain_entries = self.index_domain.entry(tuple.domain()).or_default();
        let mut domain_tuples = domain_entries.iter().map(|id| self.tuples.get(id).unwrap());
        if domain_tuples.any(|t| t == &tuple) {
            return Err(RelationError::UniqueConstraintViolation);
        }

        if self.unique_domain && !domain_entries.is_empty() {
            return Err(RelationError::UniqueConstraintViolation);
        }
        self.tuples.insert(tuple.id(), tuple.clone());
        domain_entries.insert(tuple.id());

        if let Some(codomain_index) = &mut self.index_codomain {
            codomain_index
                .entry(tuple.codomain())
                .or_default()
                .insert(tuple.id());
        }

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
        self.index_domain
            .get_mut(&old_tref.domain())
            .unwrap()
            .remove(old_tuple)
            .expect("tuple missing from domain index set");
        if let Some(codomain_index) = &mut self.index_codomain {
            codomain_index
                .get_mut(&old_tref.codomain())
                .unwrap()
                .remove(old_tuple)
                .expect("tuple missing from codomain index set");
        }

        self.tuples.insert(tuple.id(), tuple.clone());

        // Add the new tuple to the domain index
        self.index_domain
            .entry(tuple.domain())
            .or_default()
            .insert(tuple.id());

        // If there's a codomain index, update it.
        if let Some(codomain_index) = &mut self.index_codomain {
            // And add the new tuple to the codomain index.
            codomain_index
                .entry(tuple.codomain())
                .or_default()
                .insert(tuple.id());
        }

        Ok(())
    }
}
