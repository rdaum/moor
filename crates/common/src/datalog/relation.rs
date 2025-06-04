// filepath: /home/ryan/moor/crates/common/src/datalog/relation.rs
// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use crate::datalog::{Atom, Fact, Term};
use hi_sparse_bitset::{BitSet, config, ops::*, reduce};
use moor_var::{Symbol, Var};
use std::collections::{HashMap, HashSet};

/// Our bitset type for fact operations
type FactSet = BitSet<config::_128bit>;

/// Trait for different relation storage backends
pub trait RelationBackend {
    /// Get the predicate name for this relation
    fn predicate(&self) -> &Symbol;

    /// Add a fact to the relation
    /// Returns true if the fact was newly added, false if it was already present
    fn add_fact(&mut self, fact_id: u64, values: Vec<Var>) -> bool;

    /// Get all facts in this relation
    fn facts(&self) -> Vec<&Fact>;

    /// Find facts that match an atom using indexes when possible
    fn find_matching_facts(&self, atom: &Atom) -> Vec<&Fact>;
}

/// A relation in a Datalog knowledge base, representing a collection of facts
/// with the same predicate. This is the default in-memory implementation with
/// hash indexes and bitset indexes for fast lookups.
#[derive(Debug)]
pub struct HashSetRelation {
    /// The name of this relation (predicate)
    predicate: Symbol,
    /// The facts in this relation
    facts: HashSet<Fact>,
    /// Secondary indexes for fact lookup by position
    /// Maps position -> value -> set of fact IDs
    fact_indexes: Vec<HashMap<Var, HashSet<u64>>>,
    /// Bitset indexes for fast joins
    /// Maps position -> value -> bitset of fact IDs
    bitset_indexes: Vec<HashMap<Var, FactSet>>,
}

impl HashSetRelation {
    /// Create a new empty relation with the given predicate name
    pub fn new(predicate: Symbol) -> Self {
        Self {
            predicate,
            facts: HashSet::new(),
            fact_indexes: Vec::new(),
            bitset_indexes: Vec::new(),
        }
    }
}

impl RelationBackend for HashSetRelation {
    /// Get the predicate name for this relation
    fn predicate(&self) -> &Symbol {
        &self.predicate
    }

    /// Add a fact to the relation
    /// Returns true if the fact was newly added, false if it was already present
    fn add_fact(&mut self, fact_id: u64, values: Vec<Var>) -> bool {
        let fact = Fact::new(fact_id, self.predicate, values.clone());

        // Add to primary index
        let is_new_fact = self.facts.insert(fact);

        // If the fact was actually added (wasn't a duplicate), update secondary indexes
        if is_new_fact {
            // Ensure we have enough indexes for each position
            if self.fact_indexes.len() < values.len() {
                self.fact_indexes.resize_with(values.len(), HashMap::new);
            }

            // Ensure we have enough bitset indexes for each position
            if self.bitset_indexes.len() < values.len() {
                self.bitset_indexes.resize_with(values.len(), HashMap::new);
            }

            // Update each position's index
            for (pos, value) in values.iter().enumerate() {
                // Update regular index
                let position_index = &mut self.fact_indexes[pos];
                let fact_ids = position_index.entry(value.clone()).or_default();
                fact_ids.insert(fact_id);

                // Update bitset index
                let position_bitset = &mut self.bitset_indexes[pos];
                let fact_bitset = position_bitset.entry(value.clone()).or_default();
                fact_bitset.insert(fact_id as usize);
            }
        }

        is_new_fact
    }

    /// Get all facts in this relation
    fn facts(&self) -> Vec<&Fact> {
        self.facts.iter().collect()
    }

    /// Find facts that match an atom using indexes when possible
    /// Uses bitset indexes for faster lookups when available
    fn find_matching_facts(&self, atom: &Atom) -> Vec<&Fact> {
        // Check if we can use the bitset indexes for faster lookups
        let mut intersection_set = vec![];

        // First try to build a bitset that represents matching facts
        for (pos, term) in atom.terms().iter().enumerate() {
            let Term::Constant(value) = term else {
                continue;
            };
            // This position has a constant, check if we have a bitset index
            if pos >= self.bitset_indexes.len() {
                continue;
            }
            let position_bitset = &self.bitset_indexes[pos];
            let Some(fact_bitset) = position_bitset.get(value) else {
                continue;
            };
            intersection_set.push(fact_bitset.clone());
        }

        let matching_set = reduce(And, intersection_set.iter());
        // If we built a matching bitset, use it to find facts
        if let Some(bitset) = matching_set {
            // If the bitset is empty, return empty
            if bitset.is_empty() {
                return Vec::new();
            }

            // Return facts matching the bitset
            return self
                .facts
                .iter()
                .filter(|fact| bitset.contains(fact.id as usize))
                .collect();
        }

        // Fall back to the regular index-based lookup if bitset indexes didn't help
        // Look for constant terms in the query that can be used for indexing
        let mut best_position: Option<usize> = None;
        let mut best_selectivity: usize = self.facts.len();

        for (pos, term) in atom.terms().iter().enumerate() {
            let Term::Constant(value) = term else {
                continue;
            };
            // This position has a constant, check if we have an index
            if pos >= self.fact_indexes.len() {
                continue;
            }
            let position_index = &self.fact_indexes[pos];
            let Some(fact_ids) = position_index.get(value) else {
                continue;
            };
            // If this index is more selective, use it
            if fact_ids.len() < best_selectivity {
                best_position = Some(pos);
                best_selectivity = fact_ids.len();
            }
        }

        // If we found a good index, use it for filtering
        if let Some(pos) = best_position {
            if let Term::Constant(value) = &atom.terms()[pos] {
                let position_index = &self.fact_indexes[pos];
                if let Some(fact_ids) = position_index.get(value) {
                    // Get the facts with these IDs
                    return self
                        .facts
                        .iter()
                        .filter(|fact| fact_ids.contains(&fact.id))
                        .collect();
                }
            }
        }

        // Fall back to scanning all facts with this predicate
        self.facts.iter().collect()
    }
}
