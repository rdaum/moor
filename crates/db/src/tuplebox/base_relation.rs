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

use crate::tuplebox::tuples::TupleRef;
use crate::tuplebox::RelationId;

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
    tuples: im::HashSet<TupleRef>,

    /// The domain-indexed tuples in this relation, which are in this case expressed purely as bytes.
    /// It is up to the caller to interpret them.
    index_domain: im::HashMap<SliceRef, TupleRef>,

    /// Optional reverse index from codomain -> tuples, which is used to support (more) efficient
    /// reverse lookups.
    index_codomain: Option<im::HashMap<SliceRef, HashSet<TupleRef>>>,
}

impl BaseRelation {
    pub fn new(id: RelationId, timestamp: u64) -> Self {
        Self {
            id,
            ts: timestamp,
            tuples: im::HashSet::new(),
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
                .entry(tuple.codomain())
                .or_default()
                .insert(tuple.clone());
        }
    }

    /// Establish indexes for a tuple initial-loaded from secondary storage. Basically a, "trust us,
    /// this exists" move.
    pub fn index_tuple(&mut self, mut tuple: TupleRef) {
        self.tuples.insert(tuple.clone());

        // Reset timestamp to 0, since this is a tuple initial-loaded from secondary storage.
        tuple.update_timestamp(0);

        // Update the domain index to point to the tuple...
        self.index_domain.insert(tuple.domain(), tuple.clone());

        // ... and update the secondary index if there is one.
        if let Some(index) = &mut self.index_codomain {
            index
                .entry(tuple.codomain())
                .or_insert_with(HashSet::new)
                .insert(tuple);
        }
    }

    pub fn seek_by_domain(&self, domain: SliceRef) -> Option<TupleRef> {
        self.index_domain.get(&domain).cloned()
    }

    pub fn predicate_scan<F: Fn(&(SliceRef, SliceRef)) -> bool>(&self, f: &F) -> HashSet<TupleRef> {
        self.tuples
            .iter()
            .filter(|t| f(&(t.domain(), t.codomain())))
            .cloned()
            .collect()
    }

    pub fn seek_by_codomain(&self, codomain: SliceRef) -> HashSet<TupleRef> {
        // Attempt to seek on codomain without an index is a panic.
        // We could do full-scan, but in this case we're going to assume that the caller knows
        // what they're doing.
        let codomain_index = self.index_codomain.as_ref().expect("No codomain index");
        if let Some(tuple_refs) = codomain_index.get(&codomain) {
            tuple_refs.iter().cloned().collect()
        } else {
            HashSet::new()
        }
    }
    pub fn remove_by_domain(&mut self, domain: SliceRef) {
        // Seek the tuple id...
        if let Some(tuple_ref) = self.index_domain.remove(&domain) {
            self.tuples.remove(&tuple_ref);

            // And remove from codomain index, if it exists in there
            if let Some(index) = &mut self.index_codomain {
                index
                    .entry(domain)
                    .or_insert_with(HashSet::new)
                    .remove(&tuple_ref);
            }
        }
    }

    /// Update or insert a tuple into the relation.
    pub fn upsert_tuple(&mut self, tuple: TupleRef) {
        // First check the domain->tuple id index to see if we're inserting or updating.
        let existing_tuple_ref = self.index_domain.get(&tuple.domain()).cloned();
        match existing_tuple_ref {
            None => {
                // Insert into the tuple list and the index.
                self.index_domain.insert(tuple.domain(), tuple.clone());
                self.tuples.insert(tuple.clone());
                if let Some(codomain_index) = &mut self.index_codomain {
                    codomain_index
                        .entry(tuple.codomain())
                        .or_insert_with(HashSet::new)
                        .insert(tuple);
                }
            }
            Some(existing_tuple) => {
                // We need the old value so we can update the codomain index.
                if let Some(codomain_index) = &mut self.index_codomain {
                    codomain_index
                        .entry(existing_tuple.codomain())
                        .or_insert_with(HashSet::new)
                        .remove(&existing_tuple);
                    codomain_index
                        .entry(tuple.codomain())
                        .or_insert_with(HashSet::new)
                        .insert(tuple.clone());
                }
                self.index_domain.insert(tuple.domain(), tuple.clone());
                self.tuples.remove(&existing_tuple);
                self.tuples.insert(tuple);
            }
        }
    }

    pub fn len(&self) -> usize {
        self.tuples.len()
    }
}
