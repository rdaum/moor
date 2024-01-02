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
use std::sync::Arc;

use moor_values::util::slice_ref::SliceRef;

use crate::tuplebox::tuples::TupleRef;
use crate::tuplebox::tuples::{SlotBox, TupleId};
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
#[derive(Clone)]
pub struct BaseRelation {
    pub(crate) id: RelationId,

    /// The last successful committer's tx timestamp
    pub(crate) ts: u64,

    slotbox: Arc<SlotBox>,

    /// The current tuples in this relation.
    tuples: im::HashSet<TupleRef>,

    /// The domain-indexed tuples in this relation, which are in this case expressed purely as bytes.
    /// It is up to the caller to interpret them.
    index_domain: im::HashMap<Vec<u8>, TupleRef>,

    /// Optional reverse index from codomain -> tuples, which is used to support (more) efficient
    /// reverse lookups.
    index_codomain: Option<im::HashMap<Vec<u8>, HashSet<TupleRef>>>,
}

impl BaseRelation {
    pub fn new(slotbox: Arc<SlotBox>, id: RelationId, timestamp: u64) -> Self {
        Self {
            id,
            ts: timestamp,
            slotbox,
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
        for tuple_ref in self.tuples.iter() {
            let tuple = tuple_ref.get();
            // ... update the secondary index.
            self.index_codomain
                .as_mut()
                .unwrap()
                .entry(tuple.codomain().as_slice().to_vec())
                .or_default()
                .insert(tuple_ref.clone());
        }
    }

    /// Establish indexes on a tuple initial-loaded from secondary storage. Basically a, "trust us,
    /// this exists" move.
    pub fn index_tuple(&mut self, tuple_id: TupleId) {
        let tuple_ref = TupleRef::new(self.slotbox.clone(), tuple_id);
        self.tuples.insert(tuple_ref.clone());
        let tuple = tuple_ref.get();

        // Reset timestamp to 0, since this is a tuple initial-loaded from secondary storage.
        tuple.update_timestamp(self.id, self.slotbox.clone(), 0);

        // Update the domain index to point to the tuple...
        self.index_domain
            .insert(tuple.domain().as_slice().to_vec(), tuple_ref.clone());

        // ... and update the secondary index if there is one.
        if let Some(index) = &mut self.index_codomain {
            index
                .entry(tuple.codomain().as_slice().to_vec())
                .or_insert_with(HashSet::new)
                .insert(tuple_ref);
        }
    }

    pub fn seek_by_domain(&self, domain: SliceRef) -> Option<TupleRef> {
        self.index_domain.get(domain.as_slice()).cloned()
    }

    pub fn predicate_scan<F: Fn(&(SliceRef, SliceRef)) -> bool>(&self, f: &F) -> HashSet<TupleRef> {
        self.tuples
            .iter()
            .filter(|t| {
                let t = t.get();
                f(&(t.domain(), t.codomain()))
            })
            .cloned()
            .collect()
    }

    pub fn seek_by_codomain(&self, codomain: SliceRef) -> HashSet<TupleRef> {
        // Attempt to seek on codomain without an index is a panic.
        // We could do full-scan, but in this case we're going to assume that the caller knows
        // what they're doing.
        let codomain_index = self.index_codomain.as_ref().expect("No codomain index");
        if let Some(tuple_refs) = codomain_index.get(codomain.as_slice()) {
            tuple_refs.iter().cloned().collect()
        } else {
            HashSet::new()
        }
    }
    pub fn remove_by_domain(&mut self, domain: SliceRef) {
        // Seek the tuple id...
        if let Some(tuple_ref) = self.index_domain.remove(domain.as_slice()) {
            self.tuples.remove(&tuple_ref);

            // And remove from codomain index, if it exists in there
            if let Some(index) = &mut self.index_codomain {
                index
                    .entry(domain.as_slice().to_vec())
                    .or_insert_with(HashSet::new)
                    .remove(&tuple_ref);
            }
        }
    }

    /// Update or insert a tuple into the relation.
    pub fn upsert_tuple(&mut self, new_tuple_ref: TupleRef) {
        let tuple = new_tuple_ref.get();
        // First check the domain->tuple id index to see if we're inserting or updating.
        let existing_tuple_ref = self.index_domain.get(tuple.domain().as_slice()).cloned();
        match existing_tuple_ref {
            None => {
                // Insert into the tuple list and the index.
                self.index_domain
                    .insert(tuple.domain().as_slice().to_vec(), new_tuple_ref.clone());
                self.tuples.insert(new_tuple_ref.clone());
                if let Some(codomain_index) = &mut self.index_codomain {
                    codomain_index
                        .entry(tuple.codomain().as_slice().to_vec())
                        .or_insert_with(HashSet::new)
                        .insert(new_tuple_ref);
                }
            }
            Some(existing_tuple) => {
                // We need the old value so we can update the codomain index.
                let old_value = existing_tuple.get();

                if let Some(codomain_index) = &mut self.index_codomain {
                    codomain_index
                        .entry(old_value.codomain().as_slice().to_vec())
                        .or_insert_with(HashSet::new)
                        .remove(&existing_tuple);
                    codomain_index
                        .entry(tuple.codomain().as_slice().to_vec())
                        .or_insert_with(HashSet::new)
                        .insert(new_tuple_ref.clone());
                }
                self.index_domain
                    .insert(tuple.domain().as_slice().to_vec(), new_tuple_ref.clone());
                self.tuples.remove(&existing_tuple);
                self.tuples.insert(new_tuple_ref);
            }
        }
    }

    pub fn len(&self) -> usize {
        self.tuples.len()
    }
}
