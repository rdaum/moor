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

use log::error;
use std::collections::HashSet;

use moor_values::util::SliceRef;

use crate::rdb::index::{pick_base_index, Index};
use crate::rdb::tuples::{TupleId, TupleRef};
use crate::rdb::{RelationError, RelationId, RelationInfo};

/// Represents a 'canonical' base binary relation, which is a set of tuples of domain, codomain,
/// with an index on the domain and an optional index on the codomain.
///
/// In this layer  we do not really differentiate the Domain & Codomain type; they are
/// stored and managed as ref-counted byte-arrays and it is up to layers above & below to interpret the the values
/// correctly.
///
// TODO: Add some kind of 'type' flag to the relation & tuple values,
//   so that we can do type-checking on the values, though for our purposes this may be overkill at this time.
// TODO: Indexes should be paged.
#[derive(Clone)]
pub struct BaseRelation {
    pub(crate) id: RelationId,

    pub(crate) info: RelationInfo,

    /// The last successful committer's tx timestamp
    pub(crate) ts: u64,

    /// All the tuples in this relation. Indexed by their TupleId, so we can map back from the values in the indexes.
    tuples: im::HashMap<TupleId, TupleRef>,

    /// Domain -> TupleIds
    domain_index: Box<dyn Index + Send + Sync>,
    /// Codomain -> TupleIds
    codomain_index: Option<Box<dyn Index + Send + Sync>>,
}

impl Clone for Box<dyn Index + Send + Sync> {
    fn clone(&self) -> Self {
        self.clone_index()
    }
}

impl BaseRelation {
    pub(crate) fn new(id: RelationId, relation_info: RelationInfo, timestamp: u64) -> Self {
        let (domain_index, codomain_index) = pick_base_index(&relation_info);
        Self {
            id,
            ts: timestamp,
            tuples: Default::default(),
            domain_index,
            codomain_index,
            info: relation_info,
        }
    }

    /// Establish indexes & storage for a tuple initial-loaded from secondary storage. Basically a, "trust us,
    /// this exists" move.
    pub(crate) fn load_tuple(&mut self, mut tuple: TupleRef) {
        // Reset timestamp to 0, since this is a tuple initial-loaded from secondary storage.
        tuple.update_timestamp(0);

        let d_result = self.domain_index.index_tuple(&tuple.domain(), tuple.id());
        if let Err(e) = d_result {
            error!(
                "Domain indexing failed on load for tuple {:?} in relation {:?}: {:?}",
                tuple, self.info.name, e
            );
            return;
        }

        if let Some(codomain_index) = &mut self.codomain_index {
            let c_result = codomain_index.index_tuple(&tuple.codomain(), tuple.id());
            if let Err(e) = c_result {
                error!(
                    "Codomain indexing failed on load for tuple {:?} in relation {:?}: {:?}",
                    tuple, self.info.name, e
                );
            }
        }

        // Add the tuple to the relation.
        self.tuples.insert(tuple.id(), tuple);
    }

    /// Check for a specific tuple by its id.
    pub(crate) fn has_tuple(&self, tuple: &TupleId) -> bool {
        self.tuples.contains_key(tuple)
    }

    pub fn check_domain_constraints(&self, domain: &SliceRef) -> Result<(), RelationError> {
        self.domain_index.check_constraints(domain)
    }

    pub fn seek_by_domain(&self, domain: SliceRef) -> Result<HashSet<TupleRef>, RelationError> {
        Ok(self
            .domain_index
            .seek(&domain)?
            .map(|id| {
                self.tuples
                    .get(&id)
                    .expect("missing tuple for indexed id")
                    .clone()
            })
            .collect())
    }

    pub fn seek_by_codomain(&self, codomain: SliceRef) -> Result<HashSet<TupleRef>, RelationError> {
        Ok(self
            .codomain_index
            .as_ref()
            .expect("no codomain index")
            .seek(&codomain)?
            .map(|id| self.tuples.get(&id).unwrap().clone())
            .collect())
    }

    pub fn predicate_scan<F: Fn(&TupleRef) -> bool>(&self, f: &F) -> HashSet<TupleRef> {
        self.tuples.values().filter(|t| f(t)).cloned().collect()
    }

    /// Remove a specific tuple from the relation, and update indexes accordingly.
    pub(crate) fn remove_tuple(&mut self, tuple: &TupleId) -> Result<(), RelationError> {
        let Some(tuple_ref) = self.tuples.remove(tuple) else {
            return Err(RelationError::TupleNotFound);
        };

        self.domain_index
            .unindex_tuple(&tuple_ref.domain(), tuple_ref.id())?;
        if let Some(codomain_index) = &mut self.codomain_index {
            codomain_index.unindex_tuple(&tuple_ref.codomain(), tuple_ref.id())?;
        }
        Ok(())
    }

    /// Add a net-new tuple into the relation, and update indexes accordingly.
    pub(crate) fn insert_tuple(&mut self, tuple_ref: TupleRef) -> Result<(), RelationError> {
        {
            // We're a set not a bag.
            // There's gotta be a more efficient way to do this.
            let domain_entries = self.domain_index.seek(&tuple_ref.domain())?;
            let mut domain_tuples = domain_entries.map(|id| self.tuples.get(&id).unwrap());
            if domain_tuples.any(|t| t == &tuple_ref) {
                return Err(RelationError::UniqueConstraintViolation);
            }
        }
        self.domain_index
            .index_tuple(&tuple_ref.domain(), tuple_ref.id())?;
        if let Some(codomain_index) = &mut self.codomain_index {
            codomain_index.index_tuple(&tuple_ref.codomain(), tuple_ref.id())?;
        }
        self.tuples.insert(tuple_ref.id(), tuple_ref.clone());

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
        self.domain_index
            .unindex_tuple(&old_tref.domain(), old_tref.id())?;
        if let Some(codomain_index) = &mut self.codomain_index {
            codomain_index.unindex_tuple(&old_tref.codomain(), old_tref.id())?;
        }

        // Add the new tuple to the domain index
        self.domain_index.index_tuple(&tuple.domain(), tuple.id())?;
        if let Some(codomain_index) = &mut self.codomain_index {
            codomain_index.index_tuple(&tuple.codomain(), tuple.id())?;
        }
        self.tuples.insert(tuple.id(), tuple.clone());

        Ok(())
    }
}
