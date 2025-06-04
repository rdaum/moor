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

use crate::datalog::RelationBackend;
use crate::datalog::{DatalogError, DatalogResult, HashSetRelation, Literal, Rule};
use moor_var::{Symbol, Var};
use std::collections::{HashMap, HashSet};

pub struct KnowledgeBase {
    /// The rules of the program
    pub(crate) rules: Vec<Rule>,
    /// The relations in the knowledge base, indexed by predicate name. add_rule/add_fact add to
    /// this map.
    pub(crate) base_relations: HashMap<Symbol, Box<dyn RelationBackend>>,
    /// The next fact id to use
    pub(crate) next_fact_id: u64,
    /// Rule stratification for proper negation handling
    pub(crate) strata: Vec<Vec<usize>>,
}

impl Default for KnowledgeBase {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeBase {
    /// Create a new empty knowledge base
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            base_relations: Default::default(),
            next_fact_id: 0,
            strata: Vec::new(),
        }
    }

    /// Add a rule to the program
    pub fn add_rule(&mut self, rule: Rule) {
        self.rules.push(rule);
        // Invalidate existing stratification
        self.strata.clear();
    }

    /// Add a fact to the program
    pub fn add_fact(&mut self, predicate: impl Into<Symbol>, values: Vec<Var>) {
        let predicate = predicate.into();
        let fact_id = self.next_fact_id;

        // Get or create the relation for this predicate
        let relation = self
            .base_relations
            .entry(predicate)
            .or_insert_with(|| Box::new(HashSetRelation::new(predicate)));

        // Add the fact to the relation
        let is_new_fact = relation.add_fact(fact_id, values);

        // If the fact was actually added (wasn't a duplicate), increment the fact ID and clear
        // the working relations to ensure we don't have stale unified facts
        if is_new_fact {
            self.next_fact_id += 1;
        }
    }

    /// Compute stratification of rules based on dependencies
    /// Returns an error if the program contains cycles through negation
    pub fn compute_stratification(&mut self) -> DatalogResult<()> {
        if !self.strata.is_empty() {
            return Ok(()); // Already computed
        }

        // Build dependency graph between predicates
        let mut dependencies = HashMap::<Symbol, HashSet<Symbol>>::new();
        let mut negative_dependencies = HashMap::<Symbol, HashSet<Symbol>>::new();

        // Collect all predicates used in rules
        let mut all_predicates = HashSet::new();
        for rule in &self.rules {
            all_predicates.insert(rule.head.predicate);
            for literal in &rule.body {
                let predicate = match literal {
                    Literal::Pos(atom) => atom.predicate,
                    Literal::Neg(atom) => atom.predicate,
                    Literal::Aggregate(agg) => agg.atom.predicate,
                };
                all_predicates.insert(predicate);
            }
        }

        // Initialize dependency maps
        for predicate in &all_predicates {
            dependencies.insert(*predicate, HashSet::new());
            negative_dependencies.insert(*predicate, HashSet::new());
        }

        // Build dependency relations
        for rule in &self.rules {
            let head_pred = rule.head.predicate;
            for literal in &rule.body {
                match literal {
                    Literal::Pos(atom) => {
                        dependencies
                            .get_mut(&head_pred)
                            .ok_or_else(|| {
                                DatalogError::index_inconsistency(
                                    head_pred,
                                    "Predicate not found in dependencies map",
                                )
                            })?
                            .insert(atom.predicate);
                    }
                    Literal::Neg(atom) => {
                        negative_dependencies
                            .get_mut(&head_pred)
                            .ok_or_else(|| {
                                DatalogError::index_inconsistency(
                                    head_pred,
                                    "Predicate not found in negative dependencies map",
                                )
                            })?
                            .insert(atom.predicate);
                    }
                    Literal::Aggregate(agg) => {
                        // Aggregation creates a positive dependency on the aggregated predicate
                        dependencies
                            .get_mut(&head_pred)
                            .ok_or_else(|| {
                                DatalogError::index_inconsistency(
                                    head_pred,
                                    "Predicate not found in dependencies map for aggregation",
                                )
                            })?
                            .insert(agg.atom.predicate);
                    }
                }
            }
        }

        // Compute strata using topological sorting with negative edge constraints
        let mut strata_map = HashMap::<Symbol, usize>::new();
        let mut current_stratum = 0;
        let mut remaining_predicates = all_predicates.clone();

        while !remaining_predicates.is_empty() {
            let mut stratum_predicates = HashSet::new();

            // Find predicates that can be placed in current stratum
            for &predicate in &remaining_predicates {
                let can_place = {
                    // Check negative dependencies - all must be in strictly lower strata
                    let neg_deps = negative_dependencies.get(&predicate).ok_or_else(|| {
                        DatalogError::index_inconsistency(
                            predicate,
                            "Predicate not found in negative dependencies map",
                        )
                    })?;
                    let neg_deps_ok = neg_deps
                        .iter()
                        .all(|dep| strata_map.get(dep).map_or(false, |&s| s < current_stratum));

                    // Positive dependencies can form cycles within the same stratum
                    // We only care that negative dependencies are resolved
                    neg_deps_ok
                };

                if can_place {
                    stratum_predicates.insert(predicate);
                }
            }

            if stratum_predicates.is_empty() {
                // Collect predicates that are still unprocessed (involved in cycles)
                let cycle_predicates: Vec<Symbol> = remaining_predicates.into_iter().collect();
                return Err(DatalogError::unstratifiable(cycle_predicates));
            }

            // Assign stratum to predicates
            for &predicate in &stratum_predicates {
                strata_map.insert(predicate, current_stratum);
                remaining_predicates.remove(&predicate);
            }

            current_stratum += 1;
        }

        // Group rules by stratum
        let max_stratum = strata_map.values().max().copied().unwrap_or(0);
        self.strata = vec![Vec::new(); max_stratum + 1];

        for (rule_idx, rule) in self.rules.iter().enumerate() {
            let head_stratum = strata_map[&rule.head.predicate];
            self.strata[head_stratum].push(rule_idx);
        }

        Ok(())
    }
}
