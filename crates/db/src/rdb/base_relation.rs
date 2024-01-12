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

use moor_values::util::slice_ref::SliceRef;

use crate::rdb::tuples::{TupleId, TupleRef};
use crate::rdb::RelationId;

/// Represents a 'canonical' base binary relation, which is a set of tuples of domain, codomain,
/// with a default (hash) index on the domain and an optional (hash) index on the codomain.
///
/// In this representation we do not differentiate the Domain & Codomain type; they are
/// stored and managed as raw byte-arrays and it is up to layers above to interpret the the values
/// correctly.
///
// TODO: Add some kind of 'type' flag to the relation & tuple values, so that we can do
//   type-checking on the values, though for our purposes this may be overkill at this time.
// TODO: all the 'seek' type operations should be returning a *set* of tuples that match, not
//   a single one. right now this is behaving like a key-value pair, not a proper binary relation.
//   means changing the indexes here to point to sets of tuples, not single tuples. right now
//   for moor's purposes this is irrelevant, but it will be important for proper implementation of
//   joins and other relational operations.
// TODO: the indexes should be paged.
// TODO: support ordered indexes, not just hash indexes.
//   if we're staying with in-memory, use an Adaptive Radix Tree; my implementation, but hopefully
//   modified to support CoW/shared ownership of the tree nodes, like the im::HashMap does.
//   if we're going to support on-disk indexes, use a CoW B+Tree, which I have implemented elsewhere,
//   but will need to bring in here, optimize, and provide loving care to.
// TODO: support bitmap indexes
#[derive(Clone)]
pub struct BaseRelation {
    pub(crate) id: RelationId,

    /// The last successful committer's tx timestamp
    pub(crate) ts: u64,

    /// The current tuples in this relation.
    tuples: im::HashMap<TupleId, TupleRef>,

    /// The domain-indexed tuples in this relation, which are in this case expressed purely as bytes.
    /// It is up to the caller to interpret them.
    index_domain: im::HashMap<SliceRef, TupleId>,

    /// Optional reverse index from codomain -> tuples, which is used to support (more) efficient
    /// reverse lookups.
    index_codomain: Option<im::HashMap<SliceRef, im::HashSet<TupleId>>>,
}

impl BaseRelation {
    pub fn new(id: RelationId, timestamp: u64) -> Self {
        Self {
            id,
            ts: timestamp,
            tuples: im::HashMap::new(),
            index_domain: im::HashMap::new(),
            index_codomain: None,
        }
    }
    /// Add a secondary index onto the given relation to map its codomain back to its domain.
    /// If the relation already has a secondary index, this will panic.
    /// If there is no relation with the given ID, this will panic.
    /// If the relation already has tuples, they will be indexed.
    pub fn add_secondary_index(&mut self) {
        if self.index_codomain.is_some() {
            panic!("Relation already has a secondary index");
        }
        self.index_codomain = Some(im::HashMap::new());
        for tuple in self.tuples.iter() {
            // ... update the secondary index.
            self.index_codomain
                .as_mut()
                .unwrap()
                .entry(tuple.1.codomain())
                .or_default()
                .insert(*tuple.0);
        }
    }

    /// Establish indexes for a tuple initial-loaded from secondary storage. Basically a, "trust us,
    /// this exists" move.
    pub fn index_tuple(&mut self, mut tuple: TupleRef) {
        let tref = tuple.clone();
        let id = tref.id();
        self.tuples.insert(id, tref);

        // Reset timestamp to 0, since this is a tuple initial-loaded from secondary storage.
        tuple.update_timestamp(0);

        // Update the domain index to point to the tuple...
        self.index_domain.insert(tuple.domain(), id);

        // ... and update the secondary index if there is one.
        if let Some(index) = &mut self.index_codomain {
            index.entry(tuple.codomain()).or_default().insert(id);
        }
    }

    /// Retrieve a specific tuple by its id.
    pub fn retrieve_by_id(&self, id: TupleId) -> Option<TupleRef> {
        self.tuples.get(&id).cloned()
    }

    pub fn seek_by_domain(&self, domain: SliceRef) -> Option<TupleRef> {
        let tid = self.index_domain.get(&domain).cloned();
        tid.and_then(|id| self.tuples.get(&id).cloned())
    }

    pub fn predicate_scan<F: Fn(&TupleRef) -> bool>(&self, f: &F) -> HashSet<TupleRef> {
        self.tuples
            .iter()
            .filter_map(|t| if f(t.1) { Some(t.1.clone()) } else { None })
            .collect()
    }

    pub fn seek_by_codomain(&self, codomain: SliceRef) -> HashSet<TupleRef> {
        // Attempt to seek on codomain without an index is a panic.
        // We could do full-scan, but in this case we're going to assume that the caller knows
        // what they're doing.
        let codomain_index = self.index_codomain.as_ref().expect("No codomain index");
        if let Some(tuple_refs) = codomain_index.get(&codomain) {
            tuple_refs
                .iter()
                .map(|t| {
                    self.tuples
                        .get(t)
                        .expect(
                            "TupleId mentioned in codomain index missing from relation tuple set",
                        )
                        .clone()
                })
                .collect()
        } else {
            HashSet::new()
        }
    }

    pub fn remove_by_domain(&mut self, domain: SliceRef) {
        // Seek the domain and get the tuple id.
        if let Some(tuple_id) = self.index_domain.remove(&domain) {
            let old_tuple = self.tuples.remove(&tuple_id).unwrap();

            // And remove it from codomain index, if it exists in there
            if let Some(index) = &mut self.index_codomain {
                index
                    .entry(old_tuple.codomain())
                    .or_default()
                    .remove(&tuple_id);
            }
        }
    }

    /// Add a net-new tuple into the relation, and update indexes accordingly.
    /// When updating the index we have to verify that a tuple with that domain doesn't already
    /// exist, and if it does, that's an error, it shouldn't happen.
    pub fn insert_tuple(&mut self, tuple: TupleRef) {
        let existing_tuple_ref = self.index_domain.get(&tuple.domain()).cloned();
        if existing_tuple_ref.is_some() {
            panic!("Attempt to insert tuple with duplicate domain");
        }
        let tref = tuple.clone();
        let tid = tref.id();
        self.index_domain.insert(tuple.domain(), tid);
        self.tuples.insert(tref.id(), tref);
        if let Some(codomain_index) = &mut self.index_codomain {
            codomain_index
                .entry(tuple.codomain())
                .or_default()
                .insert(tid);
        }
    }

    /// Update a tuple that already exists in the relation, and update indexes accordingly.
    pub fn update_tuple(&mut self, old_tuple_id: TupleId, tuple: TupleRef) {
        // First verify that the old tuple exists, and remove it from the local set.
        let old_tuple = self
            .tuples
            .remove(&old_tuple_id)
            .expect("Tuple not found")
            .clone();

        // Right now we don't support changing the domain of an existing tuple (tho we might
        // in the future)
        if old_tuple.domain() != tuple.domain() {
            panic!("Attempt to update tuple with different domain");
        }

        // Remove the old tuple from the domain index.
        self.index_domain
            .remove(&old_tuple.domain())
            .expect("Tuple not found in domain index");

        // Add the new tuple to the local set
        self.tuples.insert(tuple.id(), tuple.clone());

        // Insert the new tuple into the domain index.
        self.index_domain.insert(tuple.domain(), tuple.id());

        // If there's a codomain index, update it.
        if let Some(codomain_index) = &mut self.index_codomain {
            // If the codomain had reference to the codomain of the old tuple, remove it.
            codomain_index
                .entry(old_tuple.codomain())
                .or_default()
                .remove(&old_tuple_id);
            // And add the new tuple to the codomain index.
            codomain_index
                .entry(tuple.codomain())
                .or_default()
                .insert(tuple.id());
        }
    }

    pub fn len(&self) -> usize {
        self.tuples.len()
    }
}
