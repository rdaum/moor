use crate::datalog::{
    AggregateLiteral, AggregateOp, Atom, DatalogError, DatalogResult, Fact, HashSetRelation,
    KnowledgeBase, Literal, RelationBackend, Rule, Substitution, Term,
};
use moor_var::{Symbol, Var, v_list};
use std::collections::HashMap;

/// A Datalog database / knowledge-base with rules and facts.
pub struct QueryContext<'a> {
    knowledge_base: &'a KnowledgeBase,
    /// Evaluation state for incremental evaluation
    evaluation_state: Option<EvaluationState>,
    /// Working set of relations for facts found during unification.
    /// Searches always need to check these first before the base relations.
    working_relations: HashMap<Symbol, Box<dyn RelationBackend>>,
    /// The next fact id to use
    next_fact_id: u64,
}

/// State for incremental evaluation
#[derive(Debug)]
struct EvaluationState {
    /// Current stratum index being processed
    stratum_idx: usize,
    /// Current rule index within the current stratum
    rule_idx: usize,
    /// Current substitution index for the current rule
    substitution_idx: usize,
    /// Substitutions for the current rule
    substitutions: Vec<Substitution>,
    /// Whether new facts were added in the current iteration
    new_facts: bool,
    /// Whether the evaluation is complete
    is_complete: bool,
}

impl<'a> QueryContext<'a> {
    pub fn new(knowledge_base: &'a KnowledgeBase) -> Result<Self, DatalogError> {
        // Needed to establish unique fact IDs for the internanl working relations
        let next_fact_id = knowledge_base.next_fact_id;

        // Check if stratification is done and reject if not
        if knowledge_base.strata.is_empty() {
            return Err(DatalogError::NotStratified);
        }

        Ok(Self {
            knowledge_base,
            evaluation_state: None,
            working_relations: HashMap::new(),
            next_fact_id,
        })
    }

    /// Query the program for facts matching the given atom
    pub fn query(&mut self, query: &Atom) -> Vec<Substitution> {
        // Initialize the query and run evaluation to completion
        match self.init_query() {
            Ok(true) => {
                self.complete_evaluation();
            }
            Ok(false) => {
                // No rules to evaluate, just return facts
            }
            Err(_) => {
                // Stratification failed, return empty result
                return Vec::new();
            }
        }

        // Then, find all facts that match the query
        let facts = self.find_matching_facts(query);

        let mut results = Vec::new();
        for fact in facts {
            if let Some(substitution) = self.unify(query, &fact.to_atom()) {
                results.push(substitution);
            }
        }

        results
    }

    /// Query the program incrementally, allowing the caller to control the evaluation process.
    /// This method initializes the query engine and prepares for incremental evaluation.
    /// Returns `Ok(true)` if evaluation is needed, `Ok(false)` if no evaluation needed,
    /// or `Err(msg)` if stratification failed.
    pub fn query_incremental_init(&mut self) -> DatalogResult<bool> {
        self.init_query()
    }

    /// Get the current results for an incremental query.
    /// This doesn't advance the evaluation; it just returns the current results.
    pub fn query_incremental_results(&self, query: &Atom) -> Vec<Substitution> {
        // Find all facts that match the query
        let facts = self.find_matching_facts(query);

        let mut results = Vec::new();
        for fact in facts {
            if let Some(substitution) = self.unify(query, &fact.to_atom()) {
                results.push(substitution);
            }
        }

        results
    }

    fn subs_to_lists(substitutions: &[Substitution], query: &Atom) -> Vec<Vec<Var>> {
        substitutions
            .iter()
            .map(|subst| {
                query
                    .terms()
                    .iter()
                    .filter_map(|term| {
                        if let Some(var) = term.as_variable() {
                            subst.get(var).cloned()
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .collect::<Vec<_>>()
    }

    /// Get the current results for an incremental query as lists.
    pub fn query_incremental_results_as_lists(&self, query: &Atom) -> Vec<Vec<Var>> {
        Self::subs_to_lists(&self.query_incremental_results(query), query)
    }

    /// Query the program and return the results as a list of lists,
    /// where each inner list contains the values for the variables in the query
    pub fn query_as_lists(&mut self, query: &Atom) -> Vec<Vec<Var>> {
        Self::subs_to_lists(&self.query(query), query)
    }

    /// Query the program and return the results as a list of Var lists
    pub fn query_as_var_lists(&mut self, query: &Atom) -> Vec<Var> {
        let lists = self.query_as_lists(query);
        lists.into_iter().map(|list| v_list(&list)).collect()
    }

    /// Evaluate a rule and return all possible substitutions
    fn evaluate_rule(&self, rule: &Rule) -> Vec<Substitution> {
        // Start with an empty substitution
        let mut substitutions = vec![HashMap::new()];

        // Process each literal in the rule body
        for lit in &rule.body {
            let mut new_substitutions = Vec::new();
            match lit {
                Literal::Pos(atom) | Literal::Neg(atom) => {
                    for subst in &substitutions {
                        // Apply substitution to the atom in the literal
                        let atom_inst = atom.apply_substitution(subst);
                        let facts = self.find_matching_facts(&atom_inst);
                        match lit {
                            Literal::Pos(_) => {
                                // Positive literal: join with matching facts
                                for fact in facts {
                                    if let Some(mut new_subst) =
                                        self.unify(&atom_inst, &fact.to_atom())
                                    {
                                        // Merge existing substitution
                                        for (v, val) in subst {
                                            new_subst.insert(v.clone(), val.clone());
                                        }
                                        new_substitutions.push(new_subst);
                                    }
                                }
                            }
                            Literal::Neg(_) => {
                                // Negated literal: keep substitution only if no fact actually unifies
                                let conflict = facts
                                    .iter()
                                    .any(|fact| self.unify(&atom_inst, &fact.to_atom()).is_some());
                                if !conflict {
                                    new_substitutions.push(subst.clone());
                                }
                            }
                            _ => unreachable!(),
                        }
                    }
                }
                Literal::Aggregate(agg) => {
                    // Handle aggregation
                    new_substitutions = self.evaluate_aggregation(&substitutions, agg);
                }
            }
            substitutions = new_substitutions;
            if substitutions.is_empty() {
                break;
            }
        }

        substitutions
    }

    /// Evaluate an aggregation literal
    fn evaluate_aggregation(
        &self,
        input_substitutions: &[Substitution],
        agg: &AggregateLiteral,
    ) -> Vec<Substitution> {
        let mut result_substitutions = Vec::new();

        for input_subst in input_substitutions {
            // Apply current substitution to the aggregation atom
            let atom_inst = agg.atom.apply_substitution(input_subst);
            let facts = self.find_matching_facts(&atom_inst);

            // Group facts by the group variables
            let mut groups: HashMap<Vec<Var>, Vec<Var>> = HashMap::new();

            for fact in facts {
                if let Some(fact_subst) = self.unify(&atom_inst, &fact.to_atom()) {
                    // Extract group key values
                    let group_key: Vec<Var> = agg
                        .group_vars
                        .iter()
                        .filter_map(|gv| {
                            input_subst
                                .get(gv)
                                .cloned()
                                .or_else(|| fact_subst.get(gv).cloned())
                        })
                        .collect();

                    // Extract aggregate value
                    if let Some(aggregate_value) = fact_subst
                        .get(&agg.aggregate_var)
                        .cloned()
                        .or_else(|| input_subst.get(&agg.aggregate_var).cloned())
                    {
                        groups
                            .entry(group_key)
                            .or_insert_with(Vec::new)
                            .push(aggregate_value);
                    }
                }
            }

            // Compute aggregates for each group
            for (group_key, values) in groups {
                if let Some(aggregate_result) = self.compute_aggregate(&agg.op, &values) {
                    let mut new_subst = input_subst.clone();

                    // Bind group variables
                    for (i, group_var) in agg.group_vars.iter().enumerate() {
                        if i < group_key.len() {
                            new_subst.insert(group_var.clone(), group_key[i].clone());
                        }
                    }

                    // Bind result variable
                    new_subst.insert(agg.result_var.clone(), aggregate_result);

                    result_substitutions.push(new_subst);
                }
            }
        }

        result_substitutions
    }

    /// Compute an aggregate value from a list of values
    fn compute_aggregate(&self, op: &AggregateOp, values: &[Var]) -> Option<Var> {
        if values.is_empty() {
            return match op {
                AggregateOp::Count => Some(Var::mk_integer(0)),
                _ => None,
            };
        }

        match op {
            AggregateOp::Count => Some(Var::mk_integer(values.len() as i64)),
            AggregateOp::Min => values.iter().min().cloned(),
            AggregateOp::Max => values.iter().max().cloned(),
        }
    }

    /// Unify two atoms and return a substitution if successful
    fn unify(&self, a: &Atom, b: &Atom) -> Option<Substitution> {
        // Atoms must have the same predicate and arity
        if a.predicate != b.predicate || a.terms.len() != b.terms.len() {
            return None;
        }

        let mut substitution = HashMap::new();

        // Unify each pair of terms
        for (term_a, term_b) in a.terms.iter().zip(b.terms.iter()) {
            match (term_a, term_b) {
                // If both are constants, they must be equal
                (Term::Constant(value_a), Term::Constant(value_b)) => {
                    if value_a != value_b {
                        return None;
                    }
                }
                // If a is a variable, add to substitution
                (Term::Variable(var), Term::Constant(value)) => {
                    if let Some(existing) = substitution.get(var) {
                        if existing != value {
                            return None;
                        }
                    } else {
                        substitution.insert(var.clone(), value.clone());
                    }
                }
                // If b is a variable, we don't care (we only substitute variables in a)
                (Term::Constant(_), Term::Variable(_)) => {}
                // If both are variables, we don't care
                (Term::Variable(_), Term::Variable(_)) => {}
            }
        }

        Some(substitution)
    }

    /// Initialize a query evaluation. This starts the incremental evaluation process.
    /// Returns `Ok(true)` if initialization succeeded and evaluation is needed,
    /// `Ok(false)` if the query can be immediately answered,
    /// `Err(msg)` if stratification failed due to cycles through negation.
    pub fn init_query(&mut self) -> DatalogResult<bool> {
        // If we already have an evaluation in progress, reset it
        self.evaluation_state = None;

        // Check if we need to evaluate rules at all
        // If there are no rules, we can just return matching facts
        if self.knowledge_base.rules.is_empty() {
            return Ok(false);
        }

        // Initialize the evaluation state
        self.evaluation_state = Some(EvaluationState {
            stratum_idx: 0,
            rule_idx: 0,
            substitution_idx: 0,
            substitutions: Vec::new(),
            new_facts: false,
            is_complete: false,
        });

        Ok(true)
    }

    /// Step the evaluation process forward one step.
    /// Returns `true` if the evaluation is still in progress, `false` if it's complete.
    pub fn step_evaluation(&mut self) -> bool {
        // Check if we have an evaluation state
        if self.evaluation_state.is_none() {
            return false; // No evaluation in progress
        }

        // First check if evaluation is already complete
        if let Some(state) = &self.evaluation_state {
            if state.is_complete {
                return false;
            }
        }

        // Extract state information to avoid borrow conflicts
        let mut stratum_idx = 0;
        let mut rule_idx = 0;
        let mut need_evaluate_rule = false;

        // Extract state details to work with
        if let Some(state) = &mut self.evaluation_state {
            stratum_idx = state.stratum_idx;
            rule_idx = state.rule_idx;
            let substitution_idx = state.substitution_idx;
            let new_facts = state.new_facts;

            // If we've processed all strata, we're done
            if stratum_idx >= self.knowledge_base.strata.len() {
                state.is_complete = true;
                return false;
            }

            let current_stratum = &self.knowledge_base.strata[stratum_idx];

            // If we've processed all rules in current stratum
            if rule_idx >= current_stratum.len() {
                // If no new facts were added in this stratum iteration, move to next stratum
                if !new_facts {
                    state.stratum_idx += 1;
                    state.rule_idx = 0;
                    state.substitution_idx = 0;
                    state.new_facts = false;
                    return true;
                }

                // Otherwise, start a new iteration within the same stratum
                state.rule_idx = 0;
                state.substitution_idx = 0;
                state.new_facts = false;
                return true;
            }

            // Check if we need to evaluate the rule
            need_evaluate_rule = substitution_idx == 0;
        }

        // Get the current rule from the current stratum
        let current_stratum = &self.knowledge_base.strata[stratum_idx];
        let actual_rule_idx = current_stratum[rule_idx];
        let rule = &self.knowledge_base.rules[actual_rule_idx];

        // If we haven't evaluated this rule yet or need to start over
        if need_evaluate_rule {
            // Get all possible substitutions for the rule body
            let substitutions = self.evaluate_rule(rule);

            // Update the state with the new substitutions
            if let Some(state) = &mut self.evaluation_state {
                state.substitutions = substitutions;
            }
        }

        // Get the current substitutions and continue processing
        let Some(state) = &mut self.evaluation_state else {
            return false; // No evaluation state, can't proceed
        };

        // Process one substitution
        if state.substitution_idx < state.substitutions.len() {
            let substitution = &state.substitutions[state.substitution_idx];
            let temp_atom_head = rule.head.clone();
            let head = temp_atom_head.apply_substitution(substitution);

            // If the head has any variables, we can't add it as a fact
            if !head.terms.iter().any(|term| term.is_variable()) {
                // Convert the head to a fact
                let values: Vec<Var> = head // Ensure values is Vec<Var>
                    .terms
                    .iter()
                    .filter_map(|term| term.as_constant().cloned())
                    .collect();

                // Get or create the relation for this predicate
                let relation = self
                    .working_relations
                    .entry(head.predicate)
                    .or_insert_with(|| Box::new(HashSetRelation::new(head.predicate)));

                // Add the fact to the relation
                if relation.add_fact(self.next_fact_id, values) {
                    // If it's a new fact, increment the fact ID
                    self.next_fact_id += 1;
                    state.new_facts = true;
                }
            }

            // Move to the next substitution
            state.substitution_idx += 1;
        } else {
            // Move to the next rule within the stratum
            state.rule_idx += 1;
            state.substitution_idx = 0;
        }

        true
    }

    /// Complete the evaluation process, running until fixpoint.
    /// Returns the number of steps taken.
    pub fn complete_evaluation(&mut self) -> usize {
        let mut steps = 0;
        while self.step_evaluation() {
            steps += 1;
        }
        steps
    }

    /// Check if the evaluation is complete.
    pub fn is_evaluation_complete(&self) -> bool {
        match &self.evaluation_state {
            Some(state) => state.is_complete,
            None => true, // No evaluation means we're done
        }
    }

    /// Find facts that match an atom using indexes when possible
    fn find_matching_facts(&self, atom: &Atom) -> Vec<&Fact> {
        let predicate = atom.predicate();

        let mut matching_facts = Vec::new();
        // Check if we have any working relations first
        if let Some(relation) = self.working_relations.get(predicate) {
            // Delegate to the relation's find_matching_facts method
            matching_facts.extend(relation.find_matching_facts(atom));
        }
        // Now base relations
        if let Some(relation) = self.knowledge_base.base_relations.get(predicate) {
            // Delegate to the relation's find_matching_facts method
            matching_facts.extend(relation.find_matching_facts(atom));
        }
        matching_facts
    }
}

#[cfg(test)]
mod tests {
    use crate::datalog::Term::{Constant, Variable};
    use crate::datalog::knowledge_base::*;
    use crate::datalog::query::QueryContext;
    use crate::datalog::variable::new_variable;
    use crate::datalog::{AggregateLiteral, AggregateOp, Atom, DatalogError, Literal, Rule, Term};
    use moor_var::{Symbol, v_int, v_string, v_sym};

    #[test]
    fn test_indexed_lookup() {
        let mut dl = KnowledgeBase::new();

        // Add many facts with the same predicate but different values
        for i in 0..100 {
            dl.add_fact(
                // Changed
                "index_test",
                vec![v_int(i), v_string(format!("value_{}", i))],
            );
        }

        // This query should use the index for position 0
        let query_var = new_variable("X");
        let query = Atom::new(
            "index_test",
            vec![Constant(v_int(42)), Variable(query_var.clone())],
        );

        dl.compute_stratification().unwrap();
        let mut qc = QueryContext::new(&dl).unwrap();
        let results = qc.query(&query);
        assert_eq!(results.len(), 1);

        let var_results = results[0].get(&query_var).unwrap();
        assert_eq!(var_results.as_string().unwrap(), "value_42");
    }

    #[test]
    fn test_ancestor_query() {
        let mut dl = KnowledgeBase::new();

        // Add facts: parent(john, mary)
        dl.add_fact(
            // Changed
            "parent",
            vec![v_string("john".to_string()), v_string("mary".to_string())],
        );

        // Add facts: parent(mary, bob)
        dl.add_fact(
            // Changed
            "parent",
            vec![v_string("mary".to_string()), v_string("bob".to_string())],
        );

        // Add facts: parent(bob, alice)
        dl.add_fact(
            // Changed
            "parent",
            vec![v_string("bob".to_string()), v_string("alice".to_string())],
        );

        // Rule: ancestor(X, Y) :- parent(X, Y)
        let x = new_variable("X");
        let y = new_variable("Y");
        let parent_atom = Atom::new(
            Symbol::mk("parent"),
            vec![Variable(x.clone()), Variable(y.clone())],
        );
        let ancestor_atom = Atom::new("ancestor", vec![Variable(x.clone()), Variable(y.clone())]);
        dl.add_rule(Rule::new(ancestor_atom.clone(), vec![parent_atom]));

        // Rule: ancestor(X, Z) :- parent(X, Y), ancestor(Y, Z)
        let x = new_variable("X");
        let y = new_variable("Y");
        let z = new_variable("Z");
        let parent_atom = Atom::new("parent", vec![Variable(x.clone()), Variable(y.clone())]);
        let ancestor_atom_body =
            Atom::new("ancestor", vec![Variable(y.clone()), Variable(z.clone())]);
        let ancestor_atom_head =
            Atom::new("ancestor", vec![Variable(x.clone()), Variable(z.clone())]);
        dl.add_rule(Rule::new(
            ancestor_atom_head,
            vec![parent_atom, ancestor_atom_body],
        ));

        // Query: ancestor(john, X)
        let john_x = Atom::new(
            "ancestor",
            vec![
                Constant(v_string("john".to_string())),
                Variable(new_variable("X")),
            ],
        );

        dl.compute_stratification().unwrap();
        let mut qc = QueryContext::new(&dl).unwrap();
        let results = qc.query_as_lists(&john_x);
        assert_eq!(results.len(), 3); // john is ancestor of mary, bob, and alice

        // Check that john is ancestor of mary
        assert!(
            results
                .iter()
                .any(|row| row[0] == v_string("mary".to_string()))
        );
        // Check that john is ancestor of bob
        assert!(
            results
                .iter()
                .any(|row| row[0] == v_string("bob".to_string()))
        );
        // Check that john is ancestor of alice
        assert!(
            results
                .iter()
                .any(|row| row[0] == v_string("alice".to_string()))
        );
    }

    #[test]
    fn test_fibonacci() {
        let mut dl = KnowledgeBase::new();

        // Add base facts: fib(0, 0) and fib(1, 1)
        dl.add_fact("fib", vec![v_int(0), v_int(0)]); // Changed
        dl.add_fact("fib", vec![v_int(1), v_int(1)]); // Changed

        // Add rules for calculating Fibonacci numbers up to a limit
        for n in 2..35 {
            // Increased range to handle fib(9) = 34
            // For each n, add a fact: next(n-2, n-1, n)
            dl.add_fact(
                // Changed
                "next",
                vec![v_int(n - 2), v_int(n - 1), v_int(n)],
            );
        }

        // Rule: fib(N, F) :- next(A, B, N), fib(A, FA), fib(B, FB), sum(FA, FB, F)
        let n = new_variable("N");
        let f = new_variable("F");
        let a = new_variable("A");
        let b = new_variable("B");
        let fa = new_variable("FA");
        let fb = new_variable("FB");

        // next(A, B, N)
        let next_atom = Atom::new(
            "next",
            vec![
                Variable(a.clone()),
                Variable(b.clone()),
                Variable(n.clone()),
            ],
        );

        // fib(A, FA)
        let fib_a_atom = Atom::new("fib", vec![Variable(a.clone()), Variable(fa.clone())]);

        // fib(B, FB)
        let fib_b_atom = Atom::new("fib", vec![Variable(b.clone()), Variable(fb.clone())]);

        // For Datalog, we can't compute directly, so we'll add facts for sum
        for i in 0..35 {
            // Increased range to handle larger Fibonacci numbers
            for j in 0..35 {
                // Increased range to handle larger Fibonacci numbers
                dl.add_fact(
                    // Changed
                    "sum",
                    vec![v_int(i), v_int(j), v_int(i + j)],
                );
            }
        }

        // sum(FA, FB, F)
        let sum_atom = Atom::new(
            "sum",
            vec![
                Variable(fa.clone()),
                Variable(fb.clone()),
                Variable(f.clone()),
            ],
        );

        // fib(N, F)
        let fib_atom = Atom::new("fib", vec![Variable(n.clone()), Variable(f.clone())]);

        dl.add_rule(Rule::new(
            fib_atom.clone(),
            vec![next_atom, fib_a_atom, fib_b_atom, sum_atom],
        ));

        // Query: fib(5, X)
        let fib_5 = Atom::new("fib", vec![Constant(v_int(5)), Variable(new_variable("X"))]);

        {
            dl.compute_stratification().unwrap();
            let mut qc = QueryContext::new(&dl).unwrap();
            let results = qc.query_as_lists(&fib_5);
            assert_eq!(results.len(), 1);
            assert_eq!(results[0][0], v_int(5)); // fib(5) = 5
        }
        {
            // Query: fib(9, X)
            let fib_9 = Atom::new("fib", vec![Constant(v_int(9)), Variable(new_variable("X"))]);
            dl.compute_stratification().unwrap();
            let mut qc = QueryContext::new(&dl).unwrap();
            let results = qc.query_as_lists(&fib_9);
            assert_eq!(results.len(), 1);
            // According to the Fibonacci sequence: 0,1,1,2,3,5,8,13,21,34,55...
            // fib(9) is the 10th number (counting from 0), which is 34
            assert_eq!(results[0][0], v_int(34));
        }
    }

    #[test]
    fn test_adventure_game_locations() {
        let mut dl = KnowledgeBase::new();

        // Define room connections
        // direct_path(from_room, to_room)
        dl.add_fact(
            // Changed
            "direct_path",
            vec![
                v_string("entrance".to_string()),
                v_string("hall".to_string()),
            ],
        );
        dl.add_fact(
            // Changed
            "direct_path",
            vec![
                v_string("hall".to_string()),
                v_string("kitchen".to_string()),
            ],
        );
        dl.add_fact(
            // Changed
            "direct_path",
            vec![
                v_string("hall".to_string()),
                v_string("library".to_string()),
            ],
        );
        dl.add_fact(
            // Changed
            "direct_path",
            vec![
                v_string("kitchen".to_string()),
                v_string("garden".to_string()),
            ],
        );
        dl.add_fact(
            // Changed
            "direct_path",
            vec![
                v_string("library".to_string()),
                v_string("secret_room".to_string()),
            ],
        );

        // Add rule for path transitivity - if there's a path from X to Y and from Y to Z, then there's a path from X to Z
        // path(X, Y) :- direct_path(X, Y)
        let x1 = new_variable("X");
        let y1 = new_variable("Y");
        let direct_path_atom = Atom::new(
            "direct_path",
            vec![Variable(x1.clone()), Variable(y1.clone())],
        );
        let path_atom = Atom::new("path", vec![Variable(x1.clone()), Variable(y1.clone())]);
        dl.add_rule(Rule::new(path_atom, vec![direct_path_atom]));

        // path(X, Z) :- direct_path(X, Y), path(Y, Z)
        let x2 = new_variable("X");
        let y2 = new_variable("Y");
        let z2 = new_variable("Z");
        let direct_path_atom = Atom::new(
            "direct_path",
            vec![Variable(x2.clone()), Variable(y2.clone())],
        );
        let path_atom_body = Atom::new("path", vec![Variable(y2.clone()), Variable(z2.clone())]);
        let path_atom_head = Atom::new("path", vec![Variable(x2.clone()), Variable(z2.clone())]);
        dl.add_rule(Rule::new(
            path_atom_head,
            vec![direct_path_atom, path_atom_body],
        ));

        // Query: Can we reach the secret_room from the entrance?
        let entrance_to_secret = Atom::new(
            "path",
            vec![
                Constant(v_string("entrance".to_string())),
                Constant(v_string("secret_room".to_string())),
            ],
        );
        dl.compute_stratification().unwrap();
        {
            let mut dl = QueryContext::new(&dl).unwrap();
            let results = dl.query(&entrance_to_secret);
            assert_eq!(
                results.len(),
                1,
                "Should be able to reach secret_room from entrance"
            );
        }
        {
            // Query: From the entrance, what rooms can we reach?
            let reachable_from_entrance = Atom::new(
                "path",
                vec![
                    Constant(v_string("entrance".to_string())),
                    Variable(new_variable("Room")),
                ],
            );
            let mut dl = QueryContext::new(&dl).unwrap();
            let results = dl.query_as_lists(&reachable_from_entrance);

            // Should be able to reach all 5 other rooms from the entrance
            assert_eq!(results.len(), 5);

            // Check that each room is reachable
            let reachable_rooms: Vec<String> = results
                .iter()
                .map(|row| row[0].as_string().unwrap().to_string())
                .collect();

            assert!(reachable_rooms.contains(&"hall".to_string()));
            assert!(reachable_rooms.contains(&"kitchen".to_string()));
            assert!(reachable_rooms.contains(&"library".to_string()));
            assert!(reachable_rooms.contains(&"garden".to_string()));
            assert!(reachable_rooms.contains(&"secret_room".to_string()));
        }
    }

    #[test]
    fn test_adventure_game_objects() {
        let mut dl = KnowledgeBase::new();

        // Define locations of objects
        // location(object, place)
        dl.add_fact(
            // Changed
            "location",
            vec![v_string("key".to_string()), v_string("kitchen".to_string())],
        );
        dl.add_fact(
            // Changed
            "location",
            vec![
                v_string("book".to_string()),
                v_string("library".to_string()),
            ],
        );
        dl.add_fact(
            // Changed
            "location",
            vec![
                v_string("sword".to_string()),
                v_string("secret_room".to_string()),
            ],
        );
        dl.add_fact(
            // Changed
            "location",
            vec![
                v_string("flower".to_string()),
                v_string("garden".to_string()),
            ],
        );
        dl.add_fact(
            // Changed
            "location",
            vec![v_string("hat".to_string()), v_string("hall".to_string())],
        );

        // Define containers
        // container(container_object, contained_object)
        dl.add_fact(
            // Changed
            "container",
            vec![v_string("chest".to_string()), v_string("gold".to_string())],
        );
        dl.add_fact(
            // Changed
            "container",
            vec![v_string("box".to_string()), v_string("silver".to_string())],
        );
        dl.add_fact(
            // Changed
            "location",
            vec![
                v_string("chest".to_string()),
                v_string("library".to_string()),
            ],
        );
        dl.add_fact(
            // Changed
            "location",
            vec![v_string("box".to_string()), v_string("kitchen".to_string())],
        );

        // Define rules for transitive containment
        // contained_in(Object, Container) :- container(Container, Object)
        let obj1 = new_variable("Obj");
        let cont1 = new_variable("Cont");
        let container_atom = Atom::new(
            "container",
            vec![Variable(cont1.clone()), Variable(obj1.clone())],
        );
        let contained_in_atom = Atom::new(
            "contained_in",
            vec![Variable(obj1.clone()), Variable(cont1.clone())],
        );
        dl.add_rule(Rule::new(contained_in_atom, vec![container_atom]));

        // Define rules for transitive location
        // at_location(Object, Location) :- location(Object, Location)
        let obj2 = new_variable("Obj");
        let loc2 = new_variable("Loc");
        let location_atom = Atom::new(
            "location",
            vec![Variable(obj2.clone()), Variable(loc2.clone())],
        );
        let at_location_atom = Atom::new(
            "at_location",
            vec![Variable(obj2.clone()), Variable(loc2.clone())],
        );
        dl.add_rule(Rule::new(at_location_atom, vec![location_atom]));

        // at_location(Object, Location) :- contained_in(Object, Container), at_location(Container, Location)
        let obj3 = new_variable("Obj");
        let cont3 = new_variable("Cont");
        let loc3 = new_variable("Loc");
        let contained_in_atom = Atom::new(
            "contained_in",
            vec![Variable(obj3.clone()), Variable(cont3.clone())],
        );
        let at_location_body_atom = Atom::new(
            "at_location",
            vec![Variable(cont3.clone()), Variable(loc3.clone())],
        );
        let at_location_head_atom = Atom::new(
            "at_location",
            vec![Variable(obj3.clone()), Variable(loc3.clone())],
        );
        dl.add_rule(Rule::new(
            at_location_head_atom,
            vec![contained_in_atom, at_location_body_atom],
        ));

        // Query: Where is the gold?
        let gold_location = Atom::new(
            "at_location",
            vec![
                Constant(v_string("gold".to_string())),
                Variable(new_variable("Location")),
            ],
        );
        dl.compute_stratification().unwrap();
        {
            let mut qc = QueryContext::new(&dl).unwrap();
            let results = qc.query_as_lists(&gold_location);
            assert_eq!(results.len(), 1);
            assert_eq!(results[0][0].as_string().unwrap(), "library");
        }

        // Query: What objects are in the library?
        let library_objects = Atom::new(
            "at_location",
            vec![
                Variable(new_variable("Object")),
                Constant(v_string("library".to_string())),
            ],
        );
        let mut qc = QueryContext::new(&dl).unwrap();
        let results = qc.query_as_lists(&library_objects);
        assert_eq!(results.len(), 3); // book, chest, gold

        let objects: Vec<String> = results
            .iter()
            .map(|row| row[0].as_string().unwrap().to_string())
            .collect();

        assert!(objects.contains(&"book".to_string()));
        assert!(objects.contains(&"chest".to_string()));
        assert!(objects.contains(&"gold".to_string()));
    }

    #[test]
    fn test_adventure_game_puzzle() {
        let mut dl = KnowledgeBase::new();

        // Define room connections
        // direct_path(from_room, to_room, is_locked)
        dl.add_fact(
            "direct_path",
            vec![
                v_string("entrance".to_string()),
                v_string("hall".to_string()),
                v_int(0),
            ],
        );
        dl.add_fact(
            "direct_path",
            vec![
                v_string("hall".to_string()),
                v_string("kitchen".to_string()),
                v_int(0),
            ],
        );
        dl.add_fact(
            "direct_path",
            vec![
                v_string("hall".to_string()),
                v_string("library".to_string()),
                v_int(0),
            ],
        );
        dl.add_fact(
            "direct_path",
            vec![
                v_string("kitchen".to_string()),
                v_string("garden".to_string()),
                v_int(0),
            ],
        );
        dl.add_fact(
            "direct_path",
            vec![
                v_string("library".to_string()),
                v_string("vault".to_string()),
                v_int(1),
            ],
        );

        // Define locations of items
        dl.add_fact(
            "location",
            vec![v_string("key".to_string()), v_string("kitchen".to_string())],
        );
        dl.add_fact(
            "location",
            vec![
                v_string("treasure".to_string()),
                v_string("vault".to_string()),
            ],
        );

        // Define locked door requirements
        dl.add_fact(
            "unlocks",
            vec![
                v_string("key".to_string()),
                v_string("library".to_string()),
                v_string("vault".to_string()),
            ],
        );

        // Define rule for accessible paths (unlocked or player has the key)
        // For unlocked paths, we need to ensure the Player variable is properly unified
        // We'll create a rule that works for any specific player by adding a fact about players
        // First, let's add facts about which players exist
        dl.add_fact("player", vec![v_string("alice".to_string())]);
        dl.add_fact("player", vec![v_string("bob".to_string())]);

        // can_access(Player, From, To) :- player(Player), direct_path(From, To, 0)
        let player1 = new_variable("Player");
        let from1 = new_variable("From");
        let to1 = new_variable("To");
        let player_atom = Atom::new("player", vec![Variable(player1.clone())]);
        let unlocked_path_atom = Atom::new(
            "direct_path",
            vec![
                Variable(from1.clone()),
                Variable(to1.clone()),
                Constant(v_int(0)),
            ],
        );
        let can_access_atom = Atom::new(
            "can_access",
            vec![
                Variable(player1.clone()),
                Variable(from1.clone()),
                Variable(to1.clone()),
            ],
        );
        dl.add_rule(Rule::new(
            can_access_atom,
            vec![player_atom, unlocked_path_atom],
        ));

        // can_access(Player, From, To) :- direct_path(From, To, 1), has_item(Player, Key), unlocks(Key, From, To)
        let player2 = new_variable("Player");
        let from2 = new_variable("From");
        let to2 = new_variable("To");
        let key2 = new_variable("Key");

        let locked_path_atom = Atom::new(
            "direct_path",
            vec![
                Variable(from2.clone()),
                Variable(to2.clone()),
                Constant(v_int(1)),
            ],
        );
        let has_item_atom = Atom::new(
            "has_item",
            vec![Variable(player2.clone()), Variable(key2.clone())],
        );
        let unlocks_atom = Atom::new(
            "unlocks",
            vec![
                Variable(key2.clone()),
                Variable(from2.clone()),
                Variable(to2.clone()),
            ],
        );
        let can_access_locked_atom = Atom::new(
            "can_access",
            vec![
                Variable(player2.clone()),
                Variable(from2.clone()),
                Variable(to2.clone()),
            ],
        );
        dl.add_rule(Rule::new(
            can_access_locked_atom,
            vec![locked_path_atom, has_item_atom, unlocks_atom],
        ));

        // Base case for path first (important for the correct evaluation order)
        // path(Player, X, Y) :- can_access(Player, X, Y)
        let player4 = new_variable("Player");
        let x4 = new_variable("X");
        let y4 = new_variable("Y");

        let can_access_atom = Atom::new(
            "can_access",
            vec![
                Variable(player4.clone()),
                Variable(x4.clone()),
                Variable(y4.clone()),
            ],
        );
        let path_atom = Atom::new(
            "path",
            vec![
                Variable(player4.clone()),
                Variable(x4.clone()),
                Variable(y4.clone()),
            ],
        );
        dl.add_rule(Rule::new(path_atom, vec![can_access_atom]));

        // Now add the recursive rule for transitive path access
        // path(Player, X, Z) :- can_access(Player, X, Y), path(Player, Y, Z)
        let player3 = new_variable("Player");
        let x3 = new_variable("X");
        let y3 = new_variable("Y");
        let z3 = new_variable("Z");

        let can_access_atom = Atom::new(
            "can_access",
            vec![
                Variable(player3.clone()),
                Variable(x3.clone()),
                Variable(y3.clone()),
            ],
        );
        let path_atom_body = Atom::new(
            "path",
            vec![
                Variable(player3.clone()),
                Variable(y3.clone()),
                Variable(z3.clone()),
            ],
        );
        let path_atom_head = Atom::new(
            "path",
            vec![
                Variable(player3.clone()),
                Variable(x3.clone()),
                Variable(z3.clone()),
            ],
        );
        dl.add_rule(Rule::new(
            path_atom_head,
            vec![can_access_atom, path_atom_body],
        ));

        // Test scenario 1: Player without key can't access the vault
        // Alice doesn't have the key
        let alice_to_vault = Atom::new(
            "path",
            vec![
                Constant(v_string("alice".to_string())),
                Constant(v_string("entrance".to_string())),
                Constant(v_string("vault".to_string())),
            ],
        );
        dl.compute_stratification().unwrap();

        {
            let mut qc = QueryContext::new(&dl).unwrap();
            let results = qc.query(&alice_to_vault);
            assert_eq!(
                results.len(),
                0,
                "Alice shouldn't be able to access the vault without the key"
            );
        }

        // Test scenario 2: Player with key can access the vault
        // Bob has the key
        dl.add_fact(
            "has_item",
            vec![v_string("bob".to_string()), v_string("key".to_string())],
        );

        {
            let mut qc = QueryContext::new(&dl).unwrap();
            // Verify the has_item fact is properly added
            let bob_has_key = Atom::new(
                "has_item",
                vec![
                    Constant(v_string("bob".to_string())),
                    Constant(v_string("key".to_string())),
                ],
            );

            let results = qc.query(&bob_has_key);
            assert_eq!(results.len(), 1, "Bob should have the key in the database");

            // Verify that can_access works for unlocked doors
            let bob_to_hall = Atom::new(
                "can_access",
                vec![
                    Constant(v_string("bob".to_string())),
                    Constant(v_string("entrance".to_string())),
                    Constant(v_string("hall".to_string())),
                ],
            );
            let results = qc.query(&bob_to_hall);
            assert_eq!(results.len(), 1, "Bob should be able to access the hall");

            // Verify that can_access works for locked doors with keys
            let bob_library_to_vault = Atom::new(
                "can_access",
                vec![
                    Constant(v_string("bob".to_string())),
                    Constant(v_string("library".to_string())),
                    Constant(v_string("vault".to_string())),
                ],
            );
            let results = qc.query(&bob_library_to_vault);
            assert_eq!(
                results.len(),
                1,
                "Bob should be able to access the vault from the library"
            );

            // Now test the full path from entrance to vault
            let bob_to_vault = Atom::new(
                "path",
                vec![
                    Constant(v_string("bob".to_string())),
                    Constant(v_string("entrance".to_string())),
                    Constant(v_string("vault".to_string())),
                ],
            );
            let results = qc.query(&bob_to_vault);
            assert_eq!(
                results.len(),
                1,
                "Bob should be able to access the vault with the key"
            );
        }
        // Find which rooms bob can reach from the entrance
        let bob_reachable = Atom::new(
            "path",
            vec![
                Constant(v_string("bob".to_string())),
                Constant(v_string("entrance".to_string())),
                Variable(new_variable("Room")),
            ],
        );

        {
            let mut qc = QueryContext::new(&dl).unwrap();
            let results = qc.query_as_lists(&bob_reachable);
            assert_eq!(results.len(), 5); // All 5 rooms are accessible
        }
        // Can Bob get the treasure?
        // Define a rule: can_get(Player, Item) :- path(Player, entrance, Room), location(Item, Room)
        let player5 = new_variable("Player");
        let item5 = new_variable("Item");
        let room5 = new_variable("Room");

        let path_atom = Atom::new(
            "path",
            vec![
                Variable(player5.clone()),
                Constant(v_string("entrance".to_string())),
                Variable(room5.clone()),
            ],
        );
        let location_atom = Atom::new(
            "location",
            vec![Variable(item5.clone()), Variable(room5.clone())],
        );
        let can_get_atom = Atom::new(
            "can_get",
            vec![Variable(player5.clone()), Variable(item5.clone())],
        );
        dl.add_rule(Rule::new(can_get_atom, vec![path_atom, location_atom]));

        // Query: Can Bob get the treasure?
        let bob_get_treasure = Atom::new(
            "can_get",
            vec![
                Constant(v_string("bob".to_string())),
                Constant(v_string("treasure".to_string())),
            ],
        );
        dl.compute_stratification().unwrap();

        let mut qc = QueryContext::new(&dl).unwrap();
        let results = qc.query(&bob_get_treasure);
        assert_eq!(results.len(), 1, "Bob should be able to get the treasure");

        // Query: Can Alice get the treasure?
        let alice_get_treasure = Atom::new(
            "can_get",
            vec![
                Constant(v_string("alice".to_string())),
                Constant(v_string("treasure".to_string())),
            ],
        );
        let results = qc.query(&alice_get_treasure);
        assert_eq!(
            results.len(),
            0,
            "Alice shouldn't be able to get the treasure"
        );
    }

    #[test]
    fn test_incremental_evaluation() {
        let mut dl = KnowledgeBase::new();

        // Add facts: parent(john, mary)
        dl.add_fact(
            "parent",
            vec![v_string("john".to_string()), v_string("mary".to_string())],
        );

        // Add facts: parent(mary, bob)
        dl.add_fact(
            "parent",
            vec![v_string("mary".to_string()), v_string("bob".to_string())],
        );

        // Rule: ancestor(X, Y) :- parent(X, Y)
        let x = new_variable("X");
        let y = new_variable("Y");
        let parent_atom = Atom::new("parent", vec![Variable(x.clone()), Variable(y.clone())]);
        let ancestor_atom = Atom::new("ancestor", vec![Variable(x.clone()), Variable(y.clone())]);
        dl.add_rule(Rule::new(ancestor_atom.clone(), vec![parent_atom]));

        // Rule: ancestor(X, Z) :- parent(X, Y), ancestor(Y, Z)
        let x = new_variable("X");
        let y = new_variable("Y");
        let z = new_variable("Z");
        let parent_atom = Atom::new("parent", vec![Variable(x.clone()), Variable(y.clone())]);
        let ancestor_atom_body =
            Atom::new("ancestor", vec![Variable(y.clone()), Variable(z.clone())]);
        let ancestor_atom_head =
            Atom::new("ancestor", vec![Variable(x.clone()), Variable(z.clone())]);
        dl.add_rule(Rule::new(
            ancestor_atom_head,
            vec![parent_atom, ancestor_atom_body],
        ));

        // Query: ancestor(john, X)
        let john_x = Atom::new(
            "ancestor",
            vec![
                Constant(v_string("john".to_string())),
                Variable(new_variable("X")),
            ],
        );

        dl.compute_stratification().unwrap();
        let mut qc = QueryContext::new(&dl).unwrap();
        // Test incremental evaluation
        assert!(
            qc.query_incremental_init().unwrap(),
            "Should need evaluation"
        );

        // Initially, no ancestors should be derived yet
        let initial_results = qc.query_incremental_results_as_lists(&john_x);
        assert_eq!(
            initial_results.len(),
            0,
            "Initially no results should be available"
        );

        // Step until the first rule creates ancestor(john, mary)
        let mut steps_taken = 0;
        while steps_taken < 10 && qc.step_evaluation() {
            steps_taken += 1;
            let results = qc.query_incremental_results_as_lists(&john_x);
            if !results.is_empty() {
                // Found the first result, should be mary
                assert_eq!(results.len(), 1);
                assert_eq!(results[0][0].as_string().unwrap(), "mary");
                break;
            }
        }
        assert!(
            steps_taken < 10,
            "First result should be found within a few steps"
        );

        // Continue stepping until we get the final result (both mary and bob)
        while qc.step_evaluation() {
            steps_taken += 1;
        }

        assert!(qc.is_evaluation_complete(), "Evaluation should be complete");

        // Check final results
        let final_results = qc.query_incremental_results_as_lists(&john_x);
        assert_eq!(final_results.len(), 2); // john is ancestor of mary and bob

        // Verify specific results
        let ancestors: Vec<String> = final_results
            .iter()
            .map(|row| row[0].as_string().unwrap().to_string())
            .collect();

        assert!(
            ancestors.contains(&"mary".to_string()),
            "Mary should be an ancestor"
        );
        assert!(
            ancestors.contains(&"bob".to_string()),
            "Bob should be an ancestor"
        );
    }

    #[test]
    fn test_game_query_with_step_limit() {
        let mut dl = KnowledgeBase::new();

        // Set up a simple game world with many connected locations
        for i in 0..100 {
            dl.add_fact("connection", vec![v_int(i), v_int(i + 1)]);
        }

        // Add a direct connection between 0 and 50 to ensure we can find it more easily
        // This ensures we have a short path to test with
        dl.add_fact("connection", vec![v_int(0), v_int(50)]);

        // Add rule for path transitivity - if there's a path from X to Y and from Y to Z, then there's a path from X to Z
        // path(X, Y) :- connection(X, Y)
        let x1 = new_variable("X");
        let y1 = new_variable("Y");
        let connection_atom = Atom::new(
            "connection",
            vec![Variable(x1.clone()), Variable(y1.clone())],
        );
        let path_atom = Atom::new("path", vec![Variable(x1.clone()), Variable(y1.clone())]);
        dl.add_rule(Rule::new(path_atom, vec![connection_atom]));

        // path(X, Z) :- connection(X, Y), path(Y, Z)
        let x2 = new_variable("X");
        let y2 = new_variable("Y");
        let z2 = new_variable("Z");
        let connection_atom = Atom::new(
            "connection",
            vec![Variable(x2.clone()), Variable(y2.clone())],
        );
        let path_atom_body = Atom::new("path", vec![Variable(y2.clone()), Variable(z2.clone())]);
        let path_atom_head = Atom::new("path", vec![Variable(x2.clone()), Variable(z2.clone())]);
        dl.add_rule(Rule::new(
            path_atom_head,
            vec![connection_atom, path_atom_body],
        ));

        // Query: path(0, 50) - reachable in a complex graph
        let query = Atom::new("path", vec![Constant(v_int(0)), Constant(v_int(50))]);

        dl.compute_stratification().unwrap();
        let mut qc = QueryContext::new(&dl).unwrap();

        // Initialize incremental evaluation
        assert!(qc.query_incremental_init().unwrap_or(false));

        // Simulate a game loop with a maximum step limit per frame
        let max_steps_per_frame = 400; // Increased from 200
        let mut total_steps = 0;
        let mut frames = 0;
        let max_frames = 20; // Increased from 10

        // For debugging
        let mut found_result = false;

        while !qc.is_evaluation_complete() && frames < max_frames {
            let mut frame_steps = 0;
            while frame_steps < max_steps_per_frame && qc.step_evaluation() {
                frame_steps += 1;
                total_steps += 1;

                // Check every 100 steps if we have results to avoid unnecessary work
                if total_steps % 100 == 0 {
                    let current_results = qc.query_incremental_results(&query);
                    if !current_results.is_empty() {
                        found_result = true;
                        break;
                    }
                }
            }

            frames += 1;

            // Check if we have an answer yet
            let current_results = qc.query_incremental_results(&query);
            if !current_results.is_empty() {
                found_result = true;
                break;
            }
        }

        // Whether we completed the evaluation or aborted, we should have a result by now
        let results = qc.query_incremental_results(&query);

        assert!(
            !results.is_empty(),
            "Should have found a path result within the step limit. Steps: {}, Frames: {}",
            total_steps,
            frames
        );

        println!(
            "Evaluation completed in {} steps across {} frames. Found result: {}",
            total_steps, frames, found_result
        );
    }

    #[test]
    fn test_graph_reachability() {
        let mut dl = KnowledgeBase::new();

        // Set up a directed graph with edge(from, to) facts
        dl.add_fact("edge", vec![v_int(1), v_int(2)]);
        dl.add_fact("edge", vec![v_int(2), v_int(3)]);
        dl.add_fact("edge", vec![v_int(3), v_int(4)]);
        dl.add_fact("edge", vec![v_int(4), v_int(5)]);
        dl.add_fact("edge", vec![v_int(1), v_int(6)]);
        dl.add_fact("edge", vec![v_int(6), v_int(7)]);
        dl.add_fact("edge", vec![v_int(7), v_int(8)]);
        dl.add_fact("edge", vec![v_int(8), v_int(9)]);
        // Create a disconnected component
        dl.add_fact("edge", vec![v_int(10), v_int(11)]);
        dl.add_fact("edge", vec![v_int(11), v_int(12)]);

        // Define reachability rules
        // Base case: If there's an edge from X to Y, then Y is reachable from X
        let x1 = new_variable("X");
        let y1 = new_variable("Y");
        let edge_atom = Atom::new("edge", vec![Variable(x1.clone()), Variable(y1.clone())]);
        let reachable_atom = Atom::new(
            "reachable",
            vec![Variable(x1.clone()), Variable(y1.clone())],
        );
        dl.add_rule(Rule::new(reachable_atom, vec![edge_atom]));

        // Recursive case: If Y is reachable from X and there's an edge from Y to Z, then Z is reachable from X
        let x2 = new_variable("X");
        let y2 = new_variable("Y");
        let z2 = new_variable("Z");
        let reachable_atom_body = Atom::new(
            "reachable",
            vec![Variable(x2.clone()), Variable(y2.clone())],
        );
        let edge_atom = Atom::new("edge", vec![Variable(y2.clone()), Variable(z2.clone())]);
        let reachable_atom_head = Atom::new(
            "reachable",
            vec![Variable(x2.clone()), Variable(z2.clone())],
        );
        dl.add_rule(Rule::new(
            reachable_atom_head,
            vec![reachable_atom_body, edge_atom],
        ));

        // Query: What nodes are reachable from node 1?
        let reachable_from_1 = Atom::new(
            "reachable",
            vec![Constant(v_int(1)), Variable(new_variable("Node"))],
        );
        {
            dl.compute_stratification().unwrap();
            let mut qc = QueryContext::new(&dl).unwrap();
            let results = qc.query_as_lists(&reachable_from_1);

            // Should be able to reach nodes 2-9 from node 1
            assert_eq!(results.len(), 8);

            // Check that each reachable node is found
            let reachable_nodes: Vec<i64> = results
                .iter()
                .map(|row| row[0].as_integer().unwrap())
                .collect();

            for i in 2..=9 {
                assert!(
                    reachable_nodes.contains(&i),
                    "Node {} should be reachable from node 1",
                    i
                );
            }

            // Verify node 10 is not reachable from node 1
            assert!(
                !reachable_nodes.contains(&10),
                "Node 10 should not be reachable from node 1"
            );
        }
        // Query: What nodes are reachable from node 10?
        let reachable_from_10 = Atom::new(
            "reachable",
            vec![Constant(v_int(10)), Variable(new_variable("Node"))],
        );

        let mut qc = QueryContext::new(&dl).unwrap();
        let results = qc.query_as_lists(&reachable_from_10);

        // Should be able to reach nodes 11-12 from node 10
        assert_eq!(results.len(), 2);

        let reachable_nodes: Vec<i64> = results
            .iter()
            .map(|row| row[0].as_integer().unwrap())
            .collect();

        assert!(reachable_nodes.contains(&11));
        assert!(reachable_nodes.contains(&12));
    }

    #[test]
    fn test_negation_simple() {
        let mut dl = KnowledgeBase::new();
        dl.add_fact("foo", vec![v_int(1)]);
        dl.add_fact("foo", vec![v_int(2)]);
        dl.add_fact("bar", vec![v_int(2)]);

        // baz(X) :- foo(X), not bar(X)
        let x = new_variable("X");
        let foo_atom = Atom::new("foo", vec![Variable(x.clone())]);
        let bar_atom = Atom::new("bar", vec![Variable(x.clone())]);
        let baz_atom = Atom::new("baz", vec![Variable(x.clone())]);
        dl.add_rule(Rule::with_negation(
            baz_atom.clone(),
            vec![Literal::Pos(foo_atom), Literal::Neg(bar_atom)],
        ));

        dl.compute_stratification().unwrap();
        let mut qc = QueryContext::new(&dl).unwrap();
        let results = qc.query_as_lists(&baz_atom);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0][0], v_int(1));
    }

    #[test]
    fn test_negation_double() {
        let mut dl = KnowledgeBase::new();
        // foo: {1,2,3}, bar: {2}, baz: {3}
        for i in 1..=3 {
            dl.add_fact("foo", vec![v_int(i)]);
        }
        dl.add_fact("bar", vec![v_int(2)]);
        dl.add_fact("baz", vec![v_int(3)]);
        // qux(X) :- foo(X), not bar(X), not baz(X)
        let x = new_variable("X");
        let foo_atom = Atom::new("foo", vec![Variable(x.clone())]);
        let bar_atom = Atom::new("bar", vec![Variable(x.clone())]);
        let baz_atom = Atom::new("baz", vec![Variable(x.clone())]);
        let qux_atom = Atom::new("qux", vec![Variable(x.clone())]);
        dl.add_rule(Rule::with_negation(
            qux_atom.clone(),
            vec![
                Literal::Pos(foo_atom),
                Literal::Neg(bar_atom),
                Literal::Neg(baz_atom),
            ],
        ));
        dl.compute_stratification().unwrap();
        let mut qc = QueryContext::new(&dl).unwrap();
        let results = qc.query_as_lists(&qux_atom);
        // Only 1 is neither in bar nor baz
        assert_eq!(results.len(), 1);
        assert_eq!(results[0][0], v_int(1));
    }

    #[test]
    fn test_negation_with_constants() {
        let mut dl = KnowledgeBase::new();

        // Add facts about people and their ages
        dl.add_fact("person", vec![v_string("alice".to_string()), v_int(25)]);
        dl.add_fact("person", vec![v_string("bob".to_string()), v_int(17)]);
        dl.add_fact("person", vec![v_string("charlie".to_string()), v_int(32)]);
        dl.add_fact("person", vec![v_string("dave".to_string()), v_int(15)]);

        // Rule: minor(X) :- person(X, Age), not Age >= 18
        // In Datalog, we implement this as:
        // minor(X) :- person(X, Age), not adult_age(Age)
        // adult_age(Age) :- Age >= 18

        // Define adult_age predicate
        for i in 18..=100 {
            dl.add_fact("adult_age", vec![v_int(i)]);
        }

        // Define the minor rule
        let x = new_variable("X");
        let age = new_variable("Age");
        let person_atom = Atom::new("person", vec![Variable(x.clone()), Variable(age.clone())]);
        let adult_age_atom = Atom::new("adult_age", vec![Variable(age.clone())]);
        let minor_atom = Atom::new("minor", vec![Variable(x.clone())]);

        dl.add_rule(Rule::with_negation(
            minor_atom.clone(),
            vec![Literal::Pos(person_atom), Literal::Neg(adult_age_atom)],
        ));

        // Query: Who is a minor?
        dl.compute_stratification().unwrap();
        let mut qc = QueryContext::new(&dl).unwrap();
        let results = qc.query_as_lists(&minor_atom);
        assert_eq!(results.len(), 2, "There should be 2 minors");

        // Create a set of minors for easier verification
        let minors: Vec<String> = results
            .iter()
            .map(|row| row[0].as_string().unwrap().to_string())
            .collect();

        assert!(minors.contains(&"bob".to_string()), "Bob should be a minor");
        assert!(
            minors.contains(&"dave".to_string()),
            "Dave should be a minor"
        );
        assert!(
            !minors.contains(&"alice".to_string()),
            "Alice should not be a minor"
        );
        assert!(
            !minors.contains(&"charlie".to_string()),
            "Charlie should not be a minor"
        );
    }

    #[test]
    fn test_negation_complex_rules() {
        let mut dl = KnowledgeBase::new();

        // Set up facts about people, their skills and job requirements
        // person(Name)
        dl.add_fact("person", vec![v_sym("alice")]);
        dl.add_fact("person", vec![v_sym("bob")]);
        dl.add_fact("person", vec![v_sym("charlie")]);
        dl.add_fact("person", vec![v_sym("dave")]);

        // has_skill(Person, Skill)
        dl.add_fact("has_skill", vec![v_sym("alice"), v_sym("programming")]);
        dl.add_fact("has_skill", vec![v_sym("alice"), v_sym("design")]);
        dl.add_fact("has_skill", vec![v_sym("bob"), v_sym("programming")]);
        dl.add_fact("has_skill", vec![v_sym("charlie"), v_sym("design")]);
        dl.add_fact("has_skill", vec![v_sym("dave"), v_sym("management")]);

        // job_requires(Job, Skill)
        dl.add_fact(
            "job_requires",
            vec![v_sym("developer"), v_sym("programming")],
        );
        dl.add_fact("job_requires", vec![v_sym("designer"), v_sym("design")]);
        dl.add_fact(
            "job_requires",
            vec![v_sym("lead_dev"), v_sym("programming")],
        );
        dl.add_fact("job_requires", vec![v_sym("lead_dev"), v_sym("management")]);

        // Rule: missing_skill(Person, Job, Skill) :- person(Person), job_requires(Job, Skill), not has_skill(Person, Skill)
        let person_var = new_variable("Person");
        let job_var = new_variable("Job");
        let skill_var = new_variable("Skill");

        let person_atom = Atom::new("person", vec![Variable(person_var.clone())]);
        let job_requires_atom = Atom::new(
            "job_requires",
            vec![Variable(job_var.clone()), Variable(skill_var.clone())],
        );
        let has_skill_atom = Atom::new(
            "has_skill",
            vec![Variable(person_var.clone()), Variable(skill_var.clone())],
        );
        let missing_skill_atom = Atom::new(
            "missing_skill",
            vec![
                Variable(person_var.clone()),
                Variable(job_var.clone()),
                Variable(skill_var.clone()),
            ],
        );

        dl.add_rule(Rule::with_negation(
            missing_skill_atom.clone(),
            vec![
                Literal::Pos(person_atom),
                Literal::Pos(job_requires_atom),
                Literal::Neg(has_skill_atom),
            ],
        ));

        // Rule: qualified_for(Person, Job) :- person(Person), job_requires(Job, _), not missing_skill(Person, Job, _)
        // This is a stratified negation rule - using the previous rule in a negation
        let person_var2 = new_variable("Person");
        let job_var2 = new_variable("Job");
        let skill_var2 = new_variable("Skill");

        let person_atom2 = Atom::new("person", vec![Variable(person_var2.clone())]);
        let job_requires_atom2 = Atom::new(
            "job_requires",
            vec![Variable(job_var2.clone()), Variable(skill_var2.clone())],
        );
        let missing_skill_atom2 = Atom::new(
            "missing_skill",
            vec![
                Variable(person_var2.clone()),
                Variable(job_var2.clone()),
                Variable(new_variable("AnySkill")), // We don't care which skill specifically
            ],
        );
        let qualified_for_atom = Atom::new(
            "qualified_for",
            vec![Variable(person_var2.clone()), Variable(job_var2.clone())],
        );

        dl.add_rule(Rule::with_negation(
            qualified_for_atom.clone(),
            vec![
                Literal::Pos(person_atom2),
                Literal::Pos(job_requires_atom2),
                Literal::Neg(missing_skill_atom2),
            ],
        ));

        // Query 1: Who is missing the management skill for the lead_dev job?
        let missing_management = Atom::new(
            "missing_skill",
            vec![
                Variable(new_variable("Person")),
                Constant(v_sym("lead_dev")),
                Constant(v_sym("management")),
            ],
        );

        dl.compute_stratification().unwrap();
        let mut qc = QueryContext::new(&dl).unwrap();
        let results = qc.query_as_lists(&missing_management);
        assert_eq!(
            results.len(),
            3,
            "Three people should be missing management skills"
        );

        let missing_management_people: Vec<String> = results
            .iter()
            .map(|row| row[0].as_symbol().unwrap().to_string())
            .collect();

        assert!(missing_management_people.contains(&"alice".to_string()));
        assert!(missing_management_people.contains(&"bob".to_string()));
        assert!(missing_management_people.contains(&"charlie".to_string()));
        assert!(!missing_management_people.contains(&"dave".to_string()));

        // Query 2: Who is qualified for the developer job?
        let qualified_for_dev = Atom::new(
            "qualified_for",
            vec![
                Variable(new_variable("Person")),
                Constant(v_sym("developer")),
            ],
        );

        let results = qc.query_as_lists(&qualified_for_dev);
        assert_eq!(
            results.len(),
            2,
            "Two people should be qualified for developer"
        );

        let qualified_devs: Vec<String> = results
            .iter()
            .map(|row| row[0].as_symbol().unwrap().to_string())
            .collect();

        assert!(qualified_devs.contains(&"alice".to_string()));
        assert!(qualified_devs.contains(&"bob".to_string()));

        // Query 3: Who is qualified for the lead_dev job?
        let qualified_for_lead = Atom::new(
            "qualified_for",
            vec![
                Variable(new_variable("Person")),
                Constant(v_sym("lead_dev")),
            ],
        );

        let results = qc.query_as_lists(&qualified_for_lead);
        assert_eq!(results.len(), 0, "No one should be qualified for lead_dev");
    }

    #[test]
    fn test_stratified_negation_basic() {
        let mut dl = KnowledgeBase::new();

        // Add base facts
        dl.add_fact("a", vec![v_int(1)]);
        dl.add_fact("a", vec![v_int(2)]);
        dl.add_fact("b", vec![v_int(2)]);

        // Rule in stratum 1: c(X) :- a(X), not b(X)
        let x = new_variable("X");
        let a_atom = Atom::new("a", vec![Variable(x.clone())]);
        let b_atom = Atom::new("b", vec![Variable(x.clone())]);
        let c_atom = Atom::new("c", vec![Variable(x.clone())]);
        dl.add_rule(Rule::with_negation(
            c_atom.clone(),
            vec![Literal::Pos(a_atom), Literal::Neg(b_atom)],
        ));

        // Query: c(X)
        dl.compute_stratification().unwrap();
        let mut qc = QueryContext::new(&dl).unwrap();
        let results = qc.query_as_lists(&c_atom);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0][0], v_int(1)); // Only 1 should be in c, not 2
    }

    #[test]
    fn test_stratified_negation_two_strata() {
        let mut dl = KnowledgeBase::new();

        // Base facts
        dl.add_fact("person", vec![v_string("alice".to_string())]);
        dl.add_fact("person", vec![v_string("bob".to_string())]);
        dl.add_fact("person", vec![v_string("charlie".to_string())]);

        dl.add_fact(
            "likes",
            vec![v_string("alice".to_string()), v_string("bob".to_string())],
        );
        dl.add_fact(
            "likes",
            vec![v_string("bob".to_string()), v_string("charlie".to_string())],
        );

        // Stratum 1: friend(X,Y) :- likes(X,Y), likes(Y,X)
        let x1 = new_variable("X");
        let y1 = new_variable("Y");
        let likes_xy = Atom::new("likes", vec![Variable(x1.clone()), Variable(y1.clone())]);
        let likes_yx = Atom::new("likes", vec![Variable(y1.clone()), Variable(x1.clone())]);
        let friend_atom = Atom::new("friend", vec![Variable(x1.clone()), Variable(y1.clone())]);
        dl.add_rule(Rule::new(friend_atom.clone(), vec![likes_xy, likes_yx]));

        // Stratum 2: loner(X) :- person(X), not friend(X,_)
        let x2 = new_variable("X");
        let y2 = new_variable("Y");
        let person_atom = Atom::new("person", vec![Variable(x2.clone())]);
        let friend_check = Atom::new("friend", vec![Variable(x2.clone()), Variable(y2.clone())]);
        let loner_atom = Atom::new("loner", vec![Variable(x2.clone())]);
        dl.add_rule(Rule::with_negation(
            loner_atom.clone(),
            vec![Literal::Pos(person_atom), Literal::Neg(friend_check)],
        ));

        // Query: loner(X)
        dl.compute_stratification().unwrap();
        let mut qc = QueryContext::new(&dl).unwrap();
        let results = qc.query_as_lists(&loner_atom);

        // Since nobody has mutual likes, everyone should be a loner
        assert_eq!(results.len(), 3);
        let loners: Vec<String> = results
            .iter()
            .map(|row| row[0].as_string().unwrap().to_string())
            .collect();

        assert!(loners.contains(&"alice".to_string()));
        assert!(loners.contains(&"bob".to_string()));
        assert!(loners.contains(&"charlie".to_string()));
    }

    #[test]
    fn test_stratified_negation_cycle_detection() {
        let mut dl = KnowledgeBase::new();

        // This should panic because it contains a cycle through negation
        dl.add_fact("a", vec![v_int(1)]);

        // Rule: b(X) :- a(X), not c(X)
        let x1 = new_variable("X");
        let a_atom = Atom::new("a", vec![Variable(x1.clone())]);
        let c_atom = Atom::new("c", vec![Variable(x1.clone())]);
        let b_atom = Atom::new("b", vec![Variable(x1.clone())]);
        dl.add_rule(Rule::with_negation(
            b_atom.clone(),
            vec![Literal::Pos(a_atom), Literal::Neg(c_atom.clone())],
        ));

        // Rule: c(X) :- b(X), not b(X)  -- Creates unstratifiable dependency
        let x2 = new_variable("X");
        let b_atom2 = Atom::new("b", vec![Variable(x2.clone())]);
        let b_atom3 = Atom::new("b", vec![Variable(x2.clone())]);
        let c_atom2 = Atom::new("c", vec![Variable(x2.clone())]);
        dl.add_rule(Rule::with_negation(
            c_atom2,
            vec![Literal::Pos(b_atom2), Literal::Neg(b_atom3)],
        ));

        // This should return an error when trying to compute stratification
        match dl.compute_stratification() {
            Err(DatalogError::Unstratifiable { predicates }) => {
                assert!(
                    !predicates.is_empty(),
                    "Should identify problematic predicates"
                );
            }
            _ => panic!(
                "Expected Unstratifiable error, got {:?}",
                dl.compute_stratification()
            ),
        }
    }

    #[test]
    fn test_aggregation_count() {
        let mut dl = KnowledgeBase::new();

        // Add base facts: student grades
        dl.add_fact(
            "grade",
            vec![
                v_string("alice".to_string()),
                v_string("math".to_string()),
                v_int(85),
            ],
        );
        dl.add_fact(
            "grade",
            vec![
                v_string("alice".to_string()),
                v_string("english".to_string()),
                v_int(92),
            ],
        );
        dl.add_fact(
            "grade",
            vec![
                v_string("alice".to_string()),
                v_string("science".to_string()),
                v_int(78),
            ],
        );
        dl.add_fact(
            "grade",
            vec![
                v_string("bob".to_string()),
                v_string("math".to_string()),
                v_int(76),
            ],
        );
        dl.add_fact(
            "grade",
            vec![
                v_string("bob".to_string()),
                v_string("english".to_string()),
                v_int(84),
            ],
        );

        // Rule: course_count(Student, Count) :- Count = count(Subject) group by [Student] in grade(Student, Subject, _)
        let student_var = new_variable("Student");
        let subject_var = new_variable("Subject");
        let score_var = new_variable("Score");
        let count_var = new_variable("Count");

        let grade_atom = Atom::new(
            "grade",
            vec![
                Term::Variable(student_var.clone()),
                Term::Variable(subject_var.clone()),
                Term::Variable(score_var.clone()),
            ],
        );

        let agg_literal = AggregateLiteral::new(
            AggregateOp::Count,
            count_var.clone(),
            subject_var.clone(),
            vec![student_var.clone()],
            grade_atom,
        );

        let course_count_atom = Atom::new(
            "course_count",
            vec![
                Term::Variable(student_var.clone()),
                Term::Variable(count_var.clone()),
            ],
        );

        dl.add_rule(Rule::with_literals(
            course_count_atom.clone(),
            vec![Literal::Aggregate(agg_literal)],
        ));

        // Query: course_count(X, Y)
        dl.compute_stratification().unwrap();
        let mut qc = QueryContext::new(&dl).unwrap();
        let results = qc.query_as_lists(&course_count_atom);
        assert_eq!(results.len(), 2); // Alice and Bob

        // Check results
        let mut found_alice = false;
        let mut found_bob = false;
        for result in &results {
            let student = result[0].as_string().unwrap();
            let count = result[1].as_integer().unwrap();

            if student == "alice" {
                assert_eq!(count, 3);
                found_alice = true;
            } else if student == "bob" {
                assert_eq!(count, 2);
                found_bob = true;
            }
        }
        assert!(found_alice && found_bob);
    }

    #[test]
    fn test_aggregation_min_max() {
        let mut dl = KnowledgeBase::new();

        // Add base facts: student grades
        dl.add_fact(
            "grade",
            vec![
                v_string("alice".to_string()),
                v_string("math".to_string()),
                v_int(85),
            ],
        );
        dl.add_fact(
            "grade",
            vec![
                v_string("alice".to_string()),
                v_string("english".to_string()),
                v_int(92),
            ],
        );
        dl.add_fact(
            "grade",
            vec![
                v_string("alice".to_string()),
                v_string("science".to_string()),
                v_int(78),
            ],
        );
        dl.add_fact(
            "grade",
            vec![
                v_string("bob".to_string()),
                v_string("math".to_string()),
                v_int(76),
            ],
        );
        dl.add_fact(
            "grade",
            vec![
                v_string("bob".to_string()),
                v_string("english".to_string()),
                v_int(84),
            ],
        );

        // Rule: min_grade(Student, MinGrade) :- MinGrade = min(Score) group by [Student] in grade(Student, _, Score)
        let student_var = new_variable("Student");
        let subject_var = new_variable("Subject");
        let score_var = new_variable("Score");
        let min_var = new_variable("MinGrade");

        let grade_atom = Atom::new(
            "grade",
            vec![
                Term::Variable(student_var.clone()),
                Term::Variable(subject_var.clone()),
                Term::Variable(score_var.clone()),
            ],
        );

        let min_agg_literal = AggregateLiteral::new(
            AggregateOp::Min,
            min_var.clone(),
            score_var.clone(),
            vec![student_var.clone()],
            grade_atom.clone(),
        );

        let min_grade_atom = Atom::new(
            "min_grade",
            vec![
                Term::Variable(student_var.clone()),
                Term::Variable(min_var.clone()),
            ],
        );

        dl.add_rule(Rule::with_literals(
            min_grade_atom.clone(),
            vec![Literal::Aggregate(min_agg_literal)],
        ));

        // Rule: max_grade(Student, MaxGrade) :- MaxGrade = max(Score) group by [Student] in grade(Student, _, Score)
        let max_var = new_variable("MaxGrade");
        let max_agg_literal = AggregateLiteral::new(
            AggregateOp::Max,
            max_var.clone(),
            score_var.clone(),
            vec![student_var.clone()],
            grade_atom,
        );

        let max_grade_atom = Atom::new(
            "max_grade",
            vec![
                Term::Variable(student_var.clone()),
                Term::Variable(max_var.clone()),
            ],
        );

        dl.add_rule(Rule::with_literals(
            max_grade_atom.clone(),
            vec![Literal::Aggregate(max_agg_literal)],
        ));

        // Query min grades
        dl.compute_stratification().unwrap();
        let mut qc = QueryContext::new(&dl).unwrap();
        let min_results = qc.query_as_lists(&min_grade_atom);
        assert_eq!(min_results.len(), 2);

        for result in &min_results {
            let student = result[0].as_string().unwrap();
            let min_grade = result[1].as_integer().unwrap();

            if student == "alice" {
                assert_eq!(min_grade, 78); // Alice's minimum grade
            } else if student == "bob" {
                assert_eq!(min_grade, 76); // Bob's minimum grade
            }
        }

        // Query max grades
        let max_results = qc.query_as_lists(&max_grade_atom);
        assert_eq!(max_results.len(), 2);

        for result in &max_results {
            let student = result[0].as_string().unwrap();
            let max_grade = result[1].as_integer().unwrap();

            if student == "alice" {
                assert_eq!(max_grade, 92); // Alice's maximum grade
            } else if student == "bob" {
                assert_eq!(max_grade, 84); // Bob's maximum grade
            }
        }
    }

    #[test]
    fn test_aggregation_no_grouping() {
        let mut dl = KnowledgeBase::new();

        // Add some numbers
        for i in 1..=5 {
            dl.add_fact("number", vec![v_int(i)]);
        }

        // Rule: total_count(Count) :- Count = count(X) group by [] in number(X)
        let x_var = new_variable("X");
        let count_var = new_variable("Count");

        let number_atom = Atom::new("number", vec![Term::Variable(x_var.clone())]);

        let count_agg_literal = AggregateLiteral::new(
            AggregateOp::Count,
            count_var.clone(),
            x_var.clone(),
            vec![], // No grouping variables
            number_atom,
        );

        let total_count_atom = Atom::new("total_count", vec![Term::Variable(count_var.clone())]);

        dl.add_rule(Rule::with_literals(
            total_count_atom.clone(),
            vec![Literal::Aggregate(count_agg_literal)],
        ));

        // Query: total_count(X)
        dl.compute_stratification().unwrap();
        let mut qc = QueryContext::new(&dl).unwrap();
        let results = qc.query_as_lists(&total_count_atom);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0][0].as_integer().unwrap(), 5); // Count of all numbers
    }

    #[test]
    fn test_aggregation_with_conditions() {
        let mut dl = KnowledgeBase::new();

        // Add base facts: sales data
        dl.add_fact(
            "sale",
            vec![
                v_string("alice".to_string()),
                v_string("q1".to_string()),
                v_int(100),
            ],
        );
        dl.add_fact(
            "sale",
            vec![
                v_string("alice".to_string()),
                v_string("q2".to_string()),
                v_int(150),
            ],
        );
        dl.add_fact(
            "sale",
            vec![
                v_string("alice".to_string()),
                v_string("q3".to_string()),
                v_int(200),
            ],
        );
        dl.add_fact(
            "sale",
            vec![
                v_string("bob".to_string()),
                v_string("q1".to_string()),
                v_int(80),
            ],
        );
        dl.add_fact(
            "sale",
            vec![
                v_string("bob".to_string()),
                v_string("q2".to_string()),
                v_int(90),
            ],
        );

        // Add threshold fact
        dl.add_fact("high_performer_threshold", vec![v_int(400)]);

        // Rule: total_sales(Person, Total) :- Total = sum(Amount) group by [Person] in sale(Person, _, Amount)
        // Note: We'll use count for now since sum isn't implemented, but multiply by average
        let person_var = new_variable("Person");
        let quarter_var = new_variable("Quarter");
        let amount_var = new_variable("Amount");
        let total_var = new_variable("Total");

        let sale_atom = Atom::new(
            "sale",
            vec![
                Term::Variable(person_var.clone()),
                Term::Variable(quarter_var.clone()),
                Term::Variable(amount_var.clone()),
            ],
        );

        // For simplicity, let's count sales per person
        let count_agg_literal = AggregateLiteral::new(
            AggregateOp::Count,
            total_var.clone(),
            quarter_var.clone(),
            vec![person_var.clone()],
            sale_atom,
        );

        let total_sales_atom = Atom::new(
            "sales_count",
            vec![
                Term::Variable(person_var.clone()),
                Term::Variable(total_var.clone()),
            ],
        );

        dl.add_rule(Rule::with_literals(
            total_sales_atom.clone(),
            vec![Literal::Aggregate(count_agg_literal)],
        ));

        // Rule: high_performer(Person) :- sales_count(Person, Count), Count >= 3
        // Since we don't have comparison operators, we'll check who has exactly 3 sales
        let high_performer_atom =
            Atom::new("high_performer", vec![Term::Variable(person_var.clone())]);

        let sales_count_check = Atom::new(
            "sales_count",
            vec![Term::Variable(person_var.clone()), Term::Constant(v_int(3))],
        );

        dl.add_rule(Rule::new(
            high_performer_atom.clone(),
            vec![sales_count_check],
        ));

        // Query: who are the high performers?
        dl.compute_stratification().unwrap();
        let mut qc = QueryContext::new(&dl).unwrap();
        let high_performer_results = qc.query_as_lists(&high_performer_atom);
        assert_eq!(high_performer_results.len(), 1);
        assert_eq!(high_performer_results[0][0].as_string().unwrap(), "alice");

        // Query: what are the sales counts?
        let sales_count_results = qc.query_as_lists(&total_sales_atom);
        assert_eq!(sales_count_results.len(), 2);

        for result in &sales_count_results {
            let person = result[0].as_string().unwrap();
            let count = result[1].as_integer().unwrap();

            if person == "alice" {
                assert_eq!(count, 3); // Alice has 3 sales
            } else if person == "bob" {
                assert_eq!(count, 2); // Bob has 2 sales
            }
        }
    }

    #[test]
    fn test_fibonacci_with_aggregation() {
        let mut dl = KnowledgeBase::new();

        // Add base facts: fib(0, 0) and fib(1, 1)
        dl.add_fact("fib", vec![v_int(0), v_int(0)]);
        dl.add_fact("fib", vec![v_int(1), v_int(1)]);

        // Add some known Fibonacci values manually for testing
        dl.add_fact("fib", vec![v_int(2), v_int(1)]);
        dl.add_fact("fib", vec![v_int(3), v_int(2)]);
        dl.add_fact("fib", vec![v_int(4), v_int(3)]);

        // Add student-course like data to demonstrate aggregation with Fibonacci sequence positions
        // Let's say each position in Fibonacci sequence has "contributions" from its predecessors
        dl.add_fact("fib_contribution", vec![v_int(2), v_int(0), v_int(0)]); // fib(2) gets 0 from position 0
        dl.add_fact("fib_contribution", vec![v_int(2), v_int(1), v_int(1)]); // fib(2) gets 1 from position 1

        dl.add_fact("fib_contribution", vec![v_int(3), v_int(1), v_int(1)]); // fib(3) gets 1 from position 1  
        dl.add_fact("fib_contribution", vec![v_int(3), v_int(2), v_int(1)]); // fib(3) gets 1 from position 2

        dl.add_fact("fib_contribution", vec![v_int(4), v_int(2), v_int(1)]); // fib(4) gets 1 from position 2
        dl.add_fact("fib_contribution", vec![v_int(4), v_int(3), v_int(2)]); // fib(4) gets 2 from position 3

        // Rule: fib_sum(N, Sum) :- Sum = count(Contribution) group by [N] in fib_contribution(N, _, Contribution)
        // This will count the contributions to each Fibonacci position
        let n_var = new_variable("N");
        let from_var = new_variable("From");
        let contribution_var = new_variable("Contribution");
        let sum_var = new_variable("Sum");

        let contribution_atom = Atom::new(
            "fib_contribution",
            vec![
                Term::Variable(n_var.clone()),
                Term::Variable(from_var.clone()),
                Term::Variable(contribution_var.clone()),
            ],
        );

        let sum_agg_literal = AggregateLiteral::new(
            AggregateOp::Count,
            sum_var.clone(),
            contribution_var.clone(),
            vec![n_var.clone()],
            contribution_atom,
        );

        let fib_sum_atom = Atom::new(
            "fib_sum",
            vec![
                Term::Variable(n_var.clone()),
                Term::Variable(sum_var.clone()),
            ],
        );

        dl.add_rule(Rule::with_literals(
            fib_sum_atom.clone(),
            vec![Literal::Aggregate(sum_agg_literal)],
        ));

        // Query: fib_sum(2, X) - should count 2 contributions (0 + 1)
        let fib_sum_2 = Atom::new(
            "fib_sum",
            vec![Term::Constant(v_int(2)), Term::Variable(new_variable("X"))],
        );

        {
            dl.compute_stratification().unwrap();
            let mut qc = QueryContext::new(&dl).unwrap();
            let results = qc.query_as_lists(&fib_sum_2);
            assert!(!results.is_empty(), "Should find a result for fib_sum(2)");
            let count_2 = results[0][0].as_integer().unwrap();
            assert_eq!(count_2, 2, "fib(2) should have 2 contributions"); // 0 + 1 = 1, counted as 2 items
        }

        // Query: fib_sum(3, X) - should count 2 contributions (1 + 1)
        let fib_sum_3 = Atom::new(
            "fib_sum",
            vec![Term::Constant(v_int(3)), Term::Variable(new_variable("X"))],
        );

        {
            dl.compute_stratification().unwrap();
            let mut qc = QueryContext::new(&dl).unwrap();
            let results = qc.query_as_lists(&fib_sum_3);
            assert!(!results.is_empty(), "Should find a result for fib_sum(3)");
            let count_3 = results[0][0].as_integer().unwrap();
            assert_eq!(count_3, 2, "fib(3) should have 2 contributions"); // 1 + 1 = 2, counted as 2 items
        }

        // Query: fib_sum(4, X) - should count 2 contributions (1 + 2)
        let fib_sum_4 = Atom::new(
            "fib_sum",
            vec![Term::Constant(v_int(4)), Term::Variable(new_variable("X"))],
        );
        {
            let mut qc = QueryContext::new(&dl).unwrap();
            let results = qc.query_as_lists(&fib_sum_4);
            assert!(!results.is_empty(), "Should find a result for fib_sum(4)");
            let count_4 = results[0][0].as_integer().unwrap();
            assert_eq!(count_4, 2, "fib(4) should have 2 contributions"); // 1 + 2 = 3, counted as 2 items
        }
        // Now let's test min/max on the Fibonacci sequence values
        // Rule: min_fib_value(MinVal) :- MinVal = min(Value) group by [] in fib(_, Value)
        let value_var = new_variable("Value");
        let pos_var = new_variable("Pos");
        let min_var = new_variable("MinVal");

        let fib_atom = Atom::new(
            "fib",
            vec![
                Term::Variable(pos_var.clone()),
                Term::Variable(value_var.clone()),
            ],
        );

        let min_agg_literal = AggregateLiteral::new(
            AggregateOp::Min,
            min_var.clone(),
            value_var.clone(),
            vec![], // No grouping - global min
            fib_atom.clone(),
        );

        let min_fib_atom = Atom::new("min_fib_value", vec![Term::Variable(min_var.clone())]);

        dl.add_rule(Rule::with_literals(
            min_fib_atom.clone(),
            vec![Literal::Aggregate(min_agg_literal)],
        ));

        // Rule: max_fib_value(MaxVal) :- MaxVal = max(Value) group by [] in fib(_, Value)
        let max_var = new_variable("MaxVal");
        let max_agg_literal = AggregateLiteral::new(
            AggregateOp::Max,
            max_var.clone(),
            value_var.clone(),
            vec![], // No grouping - global max
            fib_atom.clone(),
        );

        let max_fib_atom = Atom::new("max_fib_value", vec![Term::Variable(max_var.clone())]);

        dl.add_rule(Rule::with_literals(
            max_fib_atom.clone(),
            vec![Literal::Aggregate(max_agg_literal)],
        ));

        // Query min value
        dl.compute_stratification().unwrap();
        let mut qc = QueryContext::new(&dl).unwrap();
        let min_results = qc.query_as_lists(&min_fib_atom);
        assert!(!min_results.is_empty(), "Should find min value");
        let min_value = min_results[0][0].as_integer().unwrap();
        assert_eq!(min_value, 0, "Minimum Fibonacci value should be 0");

        // Query max value
        let max_results = qc.query_as_lists(&max_fib_atom);
        assert!(!max_results.is_empty(), "Should find max value");
        let max_value = max_results[0][0].as_integer().unwrap();
        assert_eq!(
            max_value, 3,
            "Maximum Fibonacci value should be 3 (from fib(4))"
        );
    }
}
