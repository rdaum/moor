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

use crate::datalog::Variable;
use moor_var::{Symbol, Var, v_list};
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};

/// A Rule is a Horn clause in the form: head :- body
/// where head is a single atom and body is a conjunction of atoms
#[derive(Clone, Debug)]
pub struct Rule {
    /// The head atom of the rule
    head: Atom,
    /// The body atoms of the rule
    body: Vec<Atom>,
}

impl Rule {
    /// Create a new rule with the given head and body
    pub fn new(head: Atom, body: Vec<Atom>) -> Self {
        Self { head, body }
    }
}

impl Display for Rule {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} :- ", self.head)?;
        for (i, atom) in self.body.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", atom)?;
        }
        Ok(())
    }
}

/// An Atom is a predicate with terms
#[derive(Clone, Debug)]
pub struct Atom {
    /// The predicate name
    predicate: Symbol,
    /// The terms of the atom
    terms: Vec<Term>,
}

impl Atom {
    /// Create a new atom with the given predicate and terms
    pub fn new(predicate: impl Into<Symbol>, terms: Vec<Term>) -> Self {
        Self {
            predicate: predicate.into(),
            terms,
        }
    }

    /// Get the predicate of the atom
    pub fn predicate(&self) -> &Symbol {
        &self.predicate
    }

    /// Get the terms of the atom
    pub fn terms(&self) -> &[Term] {
        &self.terms
    }

    /// Apply a substitution to the atom, replacing variables with their values
    fn apply_substitution(&self, substitution: &Substitution) -> Self {
        let terms = self
            .terms
            .iter()
            .map(|term| term.apply_substitution(substitution))
            .collect();
        Self {
            predicate: self.predicate.clone(),
            terms,
        }
    }
}

impl Display for Atom {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}(", self.predicate.as_str())?;
        for (i, term) in self.terms.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", term)?;
        }
        write!(f, ")")
    }
}

/// A Term is either a constant value or a variable
#[derive(Clone, Debug)]
pub enum Term {
    /// A constant value
    Constant(Var),
    /// A variable
    Variable(Variable),
}

impl Term {
    /// Create a new constant term
    pub fn constant(value: Var) -> Self {
        Self::Constant(value)
    }

    /// Create a new variable term
    pub fn variable(var: Variable) -> Self {
        Self::Variable(var)
    }

    /// Check if the term is a variable
    pub fn is_variable(&self) -> bool {
        matches!(self, Self::Variable(_))
    }

    /// Get the variable if the term is a variable
    pub fn as_variable(&self) -> Option<&Variable> {
        match self {
            Self::Variable(var) => Some(var),
            _ => None,
        }
    }

    /// Get the constant if the term is a constant
    pub fn as_constant(&self) -> Option<&Var> {
        match self {
            Self::Constant(value) => Some(value),
            _ => None,
        }
    }

    /// Apply a substitution to the term, replacing variables with their values
    fn apply_substitution(&self, substitution: &Substitution) -> Self {
        match self {
            Self::Constant(_) => self.clone(),
            Self::Variable(var) => {
                if let Some(value) = substitution.get(var) {
                    Self::Constant(value.clone())
                } else {
                    self.clone()
                }
            }
        }
    }
}

impl Display for Term {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Constant(value) => write!(f, "{:?}", value),
            Self::Variable(var) => write!(f, "{}", var),
        }
    }
}

/// A Fact is a ground atom (an atom with no variables)
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Fact {
    /// The predicate name
    predicate: Symbol,
    /// The constant values of the fact
    values: Vec<Var>,
}

impl Fact {
    /// Create a new fact with the given predicate and values
    pub fn new(predicate: Symbol, values: Vec<Var>) -> Self {
        Self { predicate, values }
    }

    /// Get the predicate of the fact
    pub fn predicate(&self) -> &Symbol {
        &self.predicate
    }

    /// Get the values of the fact
    pub fn values(&self) -> &[Var] {
        &self.values
    }

    /// Convert the fact to an atom
    fn to_atom(&self) -> Atom {
        let terms = self
            .values
            .iter()
            .map(|value| Term::Constant(value.clone()))
            .collect();
        Atom::new(self.predicate, terms)
    }
}

impl Display for Fact {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}(", self.predicate.as_str())?;
        for (i, value) in self.values.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{:?}", value)?;
        }
        write!(f, ")")
    }
}

/// A Substitution is a mapping from variables to values
type Substitution = HashMap<Variable, Var>;

/// A Datalog program with rules and facts
#[derive(Debug)]
pub struct Datalog {
    /// The rules of the program
    rules: Vec<Rule>,
    /// The facts of the program, indexed by predicate
    // Primary index by predicate
    facts: HashMap<Symbol, HashSet<Fact>>,
    /// The next variable id to use
    next_var_id: usize,
}

impl Datalog {
    /// Create a new empty Datalog program
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            facts: HashMap::new(),
            next_var_id: 0,
        }
    }

    /// Create a new variable with the given name
    pub fn new_variable(&mut self, name: &str) -> Variable {
        let id = self.next_var_id;
        self.next_var_id += 1;
        Variable::new(Symbol::mk(name), id)
    }

    /// Add a rule to the program
    pub fn add_rule(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    /// Add a fact to the program
    pub fn add_fact(&mut self, fact: Fact) {
        // Add to primary index
        let facts_set = self
            .facts
            .entry(fact.predicate.clone())
            .or_insert_with(HashSet::new);

        facts_set.insert(fact);
    }

    /// Query the program for facts matching the given atom
    pub fn query(&mut self, query: &Atom) -> Vec<Substitution> {
        // First, evaluate the program to fixpoint
        self.evaluate();

        // Then, find all facts that match the query
        let facts = self
            .facts
            .get(query.predicate())
            .map(|facts| facts.iter().collect::<Vec<_>>())
            .unwrap_or_default();

        let mut results = Vec::new();
        for fact in facts {
            if let Some(substitution) = self.unify(query, &fact.to_atom()) {
                results.push(substitution);
            }
        }

        results
    }

    /// Query the program and return the results as a list of lists,
    /// where each inner list contains the values for the variables in the query
    pub fn query_as_lists(&mut self, query: &Atom) -> Vec<Vec<Var>> {
        let substitutions = self.query(query);
        let mut results = Vec::new();

        for substitution in substitutions {
            let mut row = Vec::new();
            for term in query.terms() {
                if let Some(var) = term.as_variable() {
                    if let Some(value) = substitution.get(var) {
                        row.push(value.clone());
                    }
                }
            }
            results.push(row);
        }

        results
    }

    /// Query the program and return the results as a list of Var lists
    pub fn query_as_var_lists(&mut self, query: &Atom) -> Vec<Var> {
        let lists = self.query_as_lists(query);
        lists.into_iter().map(|list| v_list(&list)).collect()
    }

    /// Evaluate the program to fixpoint using bottom-up evaluation
    fn evaluate(&mut self) {
        let mut new_facts = true;

        while new_facts {
            new_facts = false;

            // For each rule
            for rule in &self.rules {
                // Get all possible substitutions for the rule body
                let substitutions = self.evaluate_rule(rule);

                // Apply each substitution to the rule head
                for substitution in substitutions {
                    let head = rule.head.apply_substitution(&substitution);

                    // If the head has any variables, we can't add it as a fact
                    if head.terms.iter().any(|term| term.is_variable()) {
                        continue;
                    }

                    // Convert the head to a fact
                    let values = head
                        .terms
                        .iter()
                        .filter_map(|term| term.as_constant().cloned())
                        .collect();
                    let fact = Fact::new(head.predicate.clone(), values);

                    // Add the fact if it's new
                    let facts = self
                        .facts
                        .entry(fact.predicate.clone())
                        .or_insert_with(HashSet::new);
                    if facts.insert(fact) {
                        new_facts = true;
                    }
                }
            }
        }
    }

    /// Evaluate a rule and return all possible substitutions
    fn evaluate_rule(&self, rule: &Rule) -> Vec<Substitution> {
        // Start with an empty substitution
        let mut substitutions = vec![HashMap::new()];

        // For each atom in the rule body
        for atom in &rule.body {
            // Apply the current substitutions to the atom
            let atoms = substitutions
                .iter()
                .map(|subst| atom.apply_substitution(subst))
                .collect::<Vec<_>>();

            // Find matching facts for each atom
            let mut new_substitutions = Vec::new();
            for (i, atom) in atoms.iter().enumerate() {
                let subst = &substitutions[i];

                // Find facts that match the atom
                let facts = self
                    .facts
                    .get(&Symbol::from(atom.predicate.as_str()))
                    .map(|facts| facts.iter().collect::<Vec<_>>())
                    .unwrap_or_default();

                for fact in facts {
                    if let Some(mut new_subst) = self.unify(&atom, &fact.to_atom()) {
                        // Combine with the existing substitution
                        for (var, value) in subst {
                            new_subst.insert(var.clone(), value.clone());
                        }
                        new_substitutions.push(new_subst);
                    }
                }
            }

            substitutions = new_substitutions;

            // If there are no substitutions, we can stop early
            if substitutions.is_empty() {
                break;
            }
        }

        substitutions
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
}

impl Default for Datalog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moor_var::{v_int, v_string};

    #[test]
    fn test_indexed_lookup() {
        let mut dl = Datalog::new();

        // Add many facts with the same predicate but different values
        for i in 0..100 {
            dl.add_fact(Fact::new(
                Symbol::from("index_test"),
                vec![v_int(i), v_string(format!("value_{}", i))],
            ));
        }

        // This query should use the index for position 0
        let query_var = dl.new_variable("X");
        let query = Atom::new(
            Symbol::from("index_test"),
            vec![Term::Constant(v_int(42)), Term::Variable(query_var.clone())],
        );

        let results = dl.query(&query);
        assert_eq!(results.len(), 1);

        let var_results = results[0].get(&query_var).unwrap();
        assert_eq!(var_results.as_string().unwrap(), "value_42");
    }

    #[test]
    fn test_ancestor_query() {
        let mut dl = Datalog::new();

        // Add facts: parent(john, mary)
        dl.add_fact(Fact::new(
            Symbol::from("parent"),
            vec![v_string("john".to_string()), v_string("mary".to_string())],
        ));

        // Add facts: parent(mary, bob)
        dl.add_fact(Fact::new(
            Symbol::from("parent"),
            vec![v_string("mary".to_string()), v_string("bob".to_string())],
        ));

        // Add facts: parent(bob, alice)
        dl.add_fact(Fact::new(
            Symbol::from("parent"),
            vec![v_string("bob".to_string()), v_string("alice".to_string())],
        ));

        // Rule: ancestor(X, Y) :- parent(X, Y)
        let x = dl.new_variable("X");
        let y = dl.new_variable("Y");
        let parent_atom = Atom::new(
            Symbol::mk("parent"),
            vec![Term::Variable(x.clone()), Term::Variable(y.clone())],
        );
        let ancestor_atom = Atom::new(
            Symbol::from("ancestor"),
            vec![Term::Variable(x.clone()), Term::Variable(y.clone())],
        );
        dl.add_rule(Rule::new(ancestor_atom.clone(), vec![parent_atom]));

        // Rule: ancestor(X, Z) :- parent(X, Y), ancestor(Y, Z)
        let x = dl.new_variable("X");
        let y = dl.new_variable("Y");
        let z = dl.new_variable("Z");
        let parent_atom = Atom::new(
            Symbol::from("parent"),
            vec![Term::Variable(x.clone()), Term::Variable(y.clone())],
        );
        let ancestor_atom_body = Atom::new(
            Symbol::from("ancestor"),
            vec![Term::Variable(y.clone()), Term::Variable(z.clone())],
        );
        let ancestor_atom_head = Atom::new(
            Symbol::from("ancestor"),
            vec![Term::Variable(x.clone()), Term::Variable(z.clone())],
        );
        dl.add_rule(Rule::new(
            ancestor_atom_head,
            vec![parent_atom, ancestor_atom_body],
        ));

        // Query: ancestor(john, X)
        let john_x = Atom::new(
            Symbol::from("ancestor"),
            vec![
                Term::Constant(v_string("john".to_string())),
                Term::Variable(dl.new_variable("X")),
            ],
        );

        let results = dl.query_as_lists(&john_x);
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
        let mut dl = Datalog::new();

        // Add base facts: fib(0, 0) and fib(1, 1)
        dl.add_fact(Fact::new(Symbol::from("fib"), vec![v_int(0), v_int(0)]));
        dl.add_fact(Fact::new(Symbol::from("fib"), vec![v_int(1), v_int(1)]));

        // Add rules for calculating Fibonacci numbers up to a limit
        for n in 2..35 {
            // Increased range to handle fib(9) = 34
            // For each n, add a fact: next(n-2, n-1, n)
            dl.add_fact(Fact::new(
                Symbol::from("next"),
                vec![v_int(n - 2), v_int(n - 1), v_int(n)],
            ));
        }

        // Rule: fib(N, F) :- next(A, B, N), fib(A, FA), fib(B, FB), sum(FA, FB, F)
        let n = dl.new_variable("N");
        let f = dl.new_variable("F");
        let a = dl.new_variable("A");
        let b = dl.new_variable("B");
        let fa = dl.new_variable("FA");
        let fb = dl.new_variable("FB");

        // next(A, B, N)
        let next_atom = Atom::new(
            Symbol::from("next"),
            vec![
                Term::Variable(a.clone()),
                Term::Variable(b.clone()),
                Term::Variable(n.clone()),
            ],
        );

        // fib(A, FA)
        let fib_a_atom = Atom::new(
            Symbol::from("fib"),
            vec![Term::Variable(a.clone()), Term::Variable(fa.clone())],
        );

        // fib(B, FB)
        let fib_b_atom = Atom::new(
            Symbol::from("fib"),
            vec![Term::Variable(b.clone()), Term::Variable(fb.clone())],
        );

        // For Datalog, we can't compute directly, so we'll add facts for sum
        for i in 0..35 {
            // Increased range to handle larger Fibonacci numbers
            for j in 0..35 {
                // Increased range to handle larger Fibonacci numbers
                dl.add_fact(Fact::new(
                    Symbol::from("sum"),
                    vec![v_int(i), v_int(j), v_int(i + j)],
                ));
            }
        }

        // sum(FA, FB, F)
        let sum_atom = Atom::new(
            Symbol::from("sum"),
            vec![
                Term::Variable(fa.clone()),
                Term::Variable(fb.clone()),
                Term::Variable(f.clone()),
            ],
        );

        // fib(N, F)
        let fib_atom = Atom::new(
            Symbol::from("fib"),
            vec![Term::Variable(n.clone()), Term::Variable(f.clone())],
        );

        dl.add_rule(Rule::new(
            fib_atom.clone(),
            vec![next_atom, fib_a_atom, fib_b_atom, sum_atom],
        ));

        // Query: fib(5, X)
        let fib_5 = Atom::new(
            Symbol::from("fib"),
            vec![
                Term::Constant(v_int(5)),
                Term::Variable(dl.new_variable("X")),
            ],
        );

        let results = dl.query_as_lists(&fib_5);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0][0], v_int(5)); // fib(5) = 5

        // Query: fib(9, X)
        let fib_9 = Atom::new(
            Symbol::from("fib"),
            vec![
                Term::Constant(v_int(9)),
                Term::Variable(dl.new_variable("X")),
            ],
        );

        let results = dl.query_as_lists(&fib_9);
        assert_eq!(results.len(), 1);
        // According to the Fibonacci sequence: 0,1,1,2,3,5,8,13,21,34,55...
        // fib(9) is the 10th number (counting from 0), which is 34
        assert_eq!(results[0][0], v_int(34));
    }

    #[test]
    fn test_adventure_game_locations() {
        let mut dl = Datalog::new();

        // Define room connections
        // direct_path(from_room, to_room)
        dl.add_fact(Fact::new(
            Symbol::from("direct_path"),
            vec![
                v_string("entrance".to_string()),
                v_string("hall".to_string()),
            ],
        ));
        dl.add_fact(Fact::new(
            Symbol::from("direct_path"),
            vec![
                v_string("hall".to_string()),
                v_string("kitchen".to_string()),
            ],
        ));
        dl.add_fact(Fact::new(
            Symbol::from("direct_path"),
            vec![
                v_string("hall".to_string()),
                v_string("library".to_string()),
            ],
        ));
        dl.add_fact(Fact::new(
            Symbol::from("direct_path"),
            vec![
                v_string("kitchen".to_string()),
                v_string("garden".to_string()),
            ],
        ));
        dl.add_fact(Fact::new(
            Symbol::from("direct_path"),
            vec![
                v_string("library".to_string()),
                v_string("secret_room".to_string()),
            ],
        ));

        // Add rule for path transitivity - if there's a path from X to Y and from Y to Z, then there's a path from X to Z
        // path(X, Y) :- direct_path(X, Y)
        let x1 = dl.new_variable("X");
        let y1 = dl.new_variable("Y");
        let direct_path_atom = Atom::new(
            Symbol::from("direct_path"),
            vec![Term::Variable(x1.clone()), Term::Variable(y1.clone())],
        );
        let path_atom = Atom::new(
            Symbol::from("path"),
            vec![Term::Variable(x1.clone()), Term::Variable(y1.clone())],
        );
        dl.add_rule(Rule::new(path_atom, vec![direct_path_atom]));

        // path(X, Z) :- direct_path(X, Y), path(Y, Z)
        let x2 = dl.new_variable("X");
        let y2 = dl.new_variable("Y");
        let z2 = dl.new_variable("Z");
        let direct_path_atom = Atom::new(
            Symbol::from("direct_path"),
            vec![Term::Variable(x2.clone()), Term::Variable(y2.clone())],
        );
        let path_atom_body = Atom::new(
            Symbol::from("path"),
            vec![Term::Variable(y2.clone()), Term::Variable(z2.clone())],
        );
        let path_atom_head = Atom::new(
            Symbol::from("path"),
            vec![Term::Variable(x2.clone()), Term::Variable(z2.clone())],
        );
        dl.add_rule(Rule::new(
            path_atom_head,
            vec![direct_path_atom, path_atom_body],
        ));

        // Query: Can we reach the secret_room from the entrance?
        let entrance_to_secret = Atom::new(
            Symbol::from("path"),
            vec![
                Term::Constant(v_string("entrance".to_string())),
                Term::Constant(v_string("secret_room".to_string())),
            ],
        );
        let results = dl.query(&entrance_to_secret);
        assert_eq!(
            results.len(),
            1,
            "Should be able to reach secret_room from entrance"
        );

        // Query: From the entrance, what rooms can we reach?
        let reachable_from_entrance = Atom::new(
            Symbol::from("path"),
            vec![
                Term::Constant(v_string("entrance".to_string())),
                Term::Variable(dl.new_variable("Room")),
            ],
        );
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

    #[test]
    fn test_adventure_game_objects() {
        let mut dl = Datalog::new();

        // Define locations of objects
        // location(object, place)
        dl.add_fact(Fact::new(
            Symbol::from("location"),
            vec![v_string("key".to_string()), v_string("kitchen".to_string())],
        ));
        dl.add_fact(Fact::new(
            Symbol::from("location"),
            vec![
                v_string("book".to_string()),
                v_string("library".to_string()),
            ],
        ));
        dl.add_fact(Fact::new(
            Symbol::from("location"),
            vec![
                v_string("sword".to_string()),
                v_string("secret_room".to_string()),
            ],
        ));
        dl.add_fact(Fact::new(
            Symbol::from("location"),
            vec![
                v_string("flower".to_string()),
                v_string("garden".to_string()),
            ],
        ));
        dl.add_fact(Fact::new(
            Symbol::from("location"),
            vec![v_string("hat".to_string()), v_string("hall".to_string())],
        ));

        // Define containers
        // container(container_object, contained_object)
        dl.add_fact(Fact::new(
            Symbol::from("container"),
            vec![v_string("chest".to_string()), v_string("gold".to_string())],
        ));
        dl.add_fact(Fact::new(
            Symbol::from("container"),
            vec![v_string("box".to_string()), v_string("silver".to_string())],
        ));
        dl.add_fact(Fact::new(
            Symbol::from("location"),
            vec![
                v_string("chest".to_string()),
                v_string("library".to_string()),
            ],
        ));
        dl.add_fact(Fact::new(
            Symbol::from("location"),
            vec![v_string("box".to_string()), v_string("kitchen".to_string())],
        ));

        // Define rules for transitive containment
        // contained_in(Object, Container) :- container(Container, Object)
        let obj1 = dl.new_variable("Obj");
        let cont1 = dl.new_variable("Cont");
        let container_atom = Atom::new(
            Symbol::from("container"),
            vec![Term::Variable(cont1.clone()), Term::Variable(obj1.clone())],
        );
        let contained_in_atom = Atom::new(
            Symbol::from("contained_in"),
            vec![Term::Variable(obj1.clone()), Term::Variable(cont1.clone())],
        );
        dl.add_rule(Rule::new(contained_in_atom, vec![container_atom]));

        // Define rules for transitive location
        // at_location(Object, Location) :- location(Object, Location)
        let obj2 = dl.new_variable("Obj");
        let loc2 = dl.new_variable("Loc");
        let location_atom = Atom::new(
            Symbol::from("location"),
            vec![Term::Variable(obj2.clone()), Term::Variable(loc2.clone())],
        );
        let at_location_atom = Atom::new(
            Symbol::from("at_location"),
            vec![Term::Variable(obj2.clone()), Term::Variable(loc2.clone())],
        );
        dl.add_rule(Rule::new(at_location_atom, vec![location_atom]));

        // at_location(Object, Location) :- contained_in(Object, Container), at_location(Container, Location)
        let obj3 = dl.new_variable("Obj");
        let cont3 = dl.new_variable("Cont");
        let loc3 = dl.new_variable("Loc");
        let contained_in_atom = Atom::new(
            Symbol::from("contained_in"),
            vec![Term::Variable(obj3.clone()), Term::Variable(cont3.clone())],
        );
        let at_location_body_atom = Atom::new(
            Symbol::from("at_location"),
            vec![Term::Variable(cont3.clone()), Term::Variable(loc3.clone())],
        );
        let at_location_head_atom = Atom::new(
            Symbol::from("at_location"),
            vec![Term::Variable(obj3.clone()), Term::Variable(loc3.clone())],
        );
        dl.add_rule(Rule::new(
            at_location_head_atom,
            vec![contained_in_atom, at_location_body_atom],
        ));

        // Query: Where is the gold?
        let gold_location = Atom::new(
            Symbol::from("at_location"),
            vec![
                Term::Constant(v_string("gold".to_string())),
                Term::Variable(dl.new_variable("Location")),
            ],
        );
        let results = dl.query_as_lists(&gold_location);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0][0].as_string().unwrap(), "library");

        // Query: What objects are in the library?
        let library_objects = Atom::new(
            Symbol::from("at_location"),
            vec![
                Term::Variable(dl.new_variable("Object")),
                Term::Constant(v_string("library".to_string())),
            ],
        );
        let results = dl.query_as_lists(&library_objects);
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
        let mut dl = Datalog::new();

        // Define room connections
        // direct_path(from_room, to_room, is_locked)
        dl.add_fact(Fact::new(
            Symbol::from("direct_path"),
            vec![
                v_string("entrance".to_string()),
                v_string("hall".to_string()),
                v_int(0),
            ],
        ));
        dl.add_fact(Fact::new(
            Symbol::from("direct_path"),
            vec![
                v_string("hall".to_string()),
                v_string("kitchen".to_string()),
                v_int(0),
            ],
        ));
        dl.add_fact(Fact::new(
            Symbol::from("direct_path"),
            vec![
                v_string("hall".to_string()),
                v_string("library".to_string()),
                v_int(0),
            ],
        ));
        dl.add_fact(Fact::new(
            Symbol::from("direct_path"),
            vec![
                v_string("kitchen".to_string()),
                v_string("garden".to_string()),
                v_int(0),
            ],
        ));
        dl.add_fact(Fact::new(
            Symbol::from("direct_path"),
            vec![
                v_string("library".to_string()),
                v_string("vault".to_string()),
                v_int(1),
            ],
        ));

        // Define locations of items
        dl.add_fact(Fact::new(
            Symbol::from("location"),
            vec![v_string("key".to_string()), v_string("kitchen".to_string())],
        ));
        dl.add_fact(Fact::new(
            Symbol::from("location"),
            vec![
                v_string("treasure".to_string()),
                v_string("vault".to_string()),
            ],
        ));

        // Define locked door requirements
        dl.add_fact(Fact::new(
            Symbol::from("unlocks"),
            vec![
                v_string("key".to_string()),
                v_string("library".to_string()),
                v_string("vault".to_string()),
            ],
        ));

        // Define rule for accessible paths (unlocked or player has the key)
        // For unlocked paths, we need to ensure the Player variable is properly unified
        // We'll create a rule that works for any specific player by adding a fact about players
        // First, let's add facts about which players exist
        dl.add_fact(Fact::new(
            Symbol::from("player"),
            vec![v_string("alice".to_string())],
        ));
        dl.add_fact(Fact::new(
            Symbol::from("player"),
            vec![v_string("bob".to_string())],
        ));

        // can_access(Player, From, To) :- player(Player), direct_path(From, To, 0)
        let player1 = dl.new_variable("Player");
        let from1 = dl.new_variable("From");
        let to1 = dl.new_variable("To");
        let player_atom = Atom::new(
            Symbol::from("player"),
            vec![Term::Variable(player1.clone())],
        );
        let unlocked_path_atom = Atom::new(
            Symbol::from("direct_path"),
            vec![
                Term::Variable(from1.clone()),
                Term::Variable(to1.clone()),
                Term::Constant(v_int(0)),
            ],
        );
        let can_access_atom = Atom::new(
            Symbol::from("can_access"),
            vec![
                Term::Variable(player1.clone()),
                Term::Variable(from1.clone()),
                Term::Variable(to1.clone()),
            ],
        );
        dl.add_rule(Rule::new(
            can_access_atom,
            vec![player_atom, unlocked_path_atom],
        ));

        // can_access(Player, From, To) :- direct_path(From, To, 1), has_item(Player, Key), unlocks(Key, From, To)
        let player2 = dl.new_variable("Player");
        let from2 = dl.new_variable("From");
        let to2 = dl.new_variable("To");
        let key2 = dl.new_variable("Key");

        let locked_path_atom = Atom::new(
            Symbol::from("direct_path"),
            vec![
                Term::Variable(from2.clone()),
                Term::Variable(to2.clone()),
                Term::Constant(v_int(1)),
            ],
        );
        let has_item_atom = Atom::new(
            Symbol::from("has_item"),
            vec![
                Term::Variable(player2.clone()),
                Term::Variable(key2.clone()),
            ],
        );
        let unlocks_atom = Atom::new(
            Symbol::from("unlocks"),
            vec![
                Term::Variable(key2.clone()),
                Term::Variable(from2.clone()),
                Term::Variable(to2.clone()),
            ],
        );
        let can_access_locked_atom = Atom::new(
            Symbol::from("can_access"),
            vec![
                Term::Variable(player2.clone()),
                Term::Variable(from2.clone()),
                Term::Variable(to2.clone()),
            ],
        );
        dl.add_rule(Rule::new(
            can_access_locked_atom,
            vec![locked_path_atom, has_item_atom, unlocks_atom],
        ));

        // Base case for path first (important for the correct evaluation order)
        // path(Player, X, Y) :- can_access(Player, X, Y)
        let player4 = dl.new_variable("Player");
        let x4 = dl.new_variable("X");
        let y4 = dl.new_variable("Y");

        let can_access_atom = Atom::new(
            Symbol::from("can_access"),
            vec![
                Term::Variable(player4.clone()),
                Term::Variable(x4.clone()),
                Term::Variable(y4.clone()),
            ],
        );
        let path_atom = Atom::new(
            Symbol::from("path"),
            vec![
                Term::Variable(player4.clone()),
                Term::Variable(x4.clone()),
                Term::Variable(y4.clone()),
            ],
        );
        dl.add_rule(Rule::new(path_atom, vec![can_access_atom]));

        // Now add the recursive rule for transitive path access
        // path(Player, X, Z) :- can_access(Player, X, Y), path(Player, Y, Z)
        let player3 = dl.new_variable("Player");
        let x3 = dl.new_variable("X");
        let y3 = dl.new_variable("Y");
        let z3 = dl.new_variable("Z");

        let can_access_atom = Atom::new(
            Symbol::from("can_access"),
            vec![
                Term::Variable(player3.clone()),
                Term::Variable(x3.clone()),
                Term::Variable(y3.clone()),
            ],
        );
        let path_atom_body = Atom::new(
            Symbol::from("path"),
            vec![
                Term::Variable(player3.clone()),
                Term::Variable(y3.clone()),
                Term::Variable(z3.clone()),
            ],
        );
        let path_atom_head = Atom::new(
            Symbol::from("path"),
            vec![
                Term::Variable(player3.clone()),
                Term::Variable(x3.clone()),
                Term::Variable(z3.clone()),
            ],
        );
        dl.add_rule(Rule::new(
            path_atom_head,
            vec![can_access_atom, path_atom_body],
        ));

        // Test scenario 1: Player without key can't access the vault
        // Alice doesn't have the key
        let alice_to_vault = Atom::new(
            Symbol::from("path"),
            vec![
                Term::Constant(v_string("alice".to_string())),
                Term::Constant(v_string("entrance".to_string())),
                Term::Constant(v_string("vault".to_string())),
            ],
        );
        let results = dl.query(&alice_to_vault);
        assert_eq!(
            results.len(),
            0,
            "Alice shouldn't be able to access the vault without the key"
        );

        // Test scenario 2: Player with key can access the vault
        // Bob has the key
        dl.add_fact(Fact::new(
            Symbol::from("has_item"),
            vec![v_string("bob".to_string()), v_string("key".to_string())],
        ));

        // Verify the has_item fact is properly added
        let bob_has_key = Atom::new(
            Symbol::from("has_item"),
            vec![
                Term::Constant(v_string("bob".to_string())),
                Term::Constant(v_string("key".to_string())),
            ],
        );
        let results = dl.query(&bob_has_key);
        assert_eq!(results.len(), 1, "Bob should have the key in the database");

        // Verify that can_access works for unlocked doors
        let bob_to_hall = Atom::new(
            Symbol::from("can_access"),
            vec![
                Term::Constant(v_string("bob".to_string())),
                Term::Constant(v_string("entrance".to_string())),
                Term::Constant(v_string("hall".to_string())),
            ],
        );
        let results = dl.query(&bob_to_hall);
        assert_eq!(results.len(), 1, "Bob should be able to access the hall");

        // Verify that can_access works for locked doors with keys
        let bob_library_to_vault = Atom::new(
            Symbol::from("can_access"),
            vec![
                Term::Constant(v_string("bob".to_string())),
                Term::Constant(v_string("library".to_string())),
                Term::Constant(v_string("vault".to_string())),
            ],
        );
        let results = dl.query(&bob_library_to_vault);
        assert_eq!(
            results.len(),
            1,
            "Bob should be able to access the vault from the library"
        );

        // Now test the full path from entrance to vault
        let bob_to_vault = Atom::new(
            Symbol::from("path"),
            vec![
                Term::Constant(v_string("bob".to_string())),
                Term::Constant(v_string("entrance".to_string())),
                Term::Constant(v_string("vault".to_string())),
            ],
        );
        let results = dl.query(&bob_to_vault);
        assert_eq!(
            results.len(),
            1,
            "Bob should be able to access the vault with the key"
        );

        // Find which rooms bob can reach from the entrance
        let bob_reachable = Atom::new(
            Symbol::from("path"),
            vec![
                Term::Constant(v_string("bob".to_string())),
                Term::Constant(v_string("entrance".to_string())),
                Term::Variable(dl.new_variable("Room")),
            ],
        );
        let results = dl.query_as_lists(&bob_reachable);
        assert_eq!(results.len(), 5); // All 5 rooms are accessible

        // Can Bob get the treasure?
        // Define a rule: can_get(Player, Item) :- path(Player, entrance, Room), location(Item, Room)
        let player5 = dl.new_variable("Player");
        let item5 = dl.new_variable("Item");
        let room5 = dl.new_variable("Room");

        let path_atom = Atom::new(
            Symbol::from("path"),
            vec![
                Term::Variable(player5.clone()),
                Term::Constant(v_string("entrance".to_string())),
                Term::Variable(room5.clone()),
            ],
        );
        let location_atom = Atom::new(
            Symbol::from("location"),
            vec![Term::Variable(item5.clone()), Term::Variable(room5.clone())],
        );
        let can_get_atom = Atom::new(
            Symbol::from("can_get"),
            vec![
                Term::Variable(player5.clone()),
                Term::Variable(item5.clone()),
            ],
        );
        dl.add_rule(Rule::new(can_get_atom, vec![path_atom, location_atom]));

        // Query: Can Bob get the treasure?
        let bob_get_treasure = Atom::new(
            Symbol::from("can_get"),
            vec![
                Term::Constant(v_string("bob".to_string())),
                Term::Constant(v_string("treasure".to_string())),
            ],
        );
        let results = dl.query(&bob_get_treasure);
        assert_eq!(results.len(), 1, "Bob should be able to get the treasure");

        // Query: Can Alice get the treasure?
        let alice_get_treasure = Atom::new(
            Symbol::from("can_get"),
            vec![
                Term::Constant(v_string("alice".to_string())),
                Term::Constant(v_string("treasure".to_string())),
            ],
        );
        let results = dl.query(&alice_get_treasure);
        assert_eq!(
            results.len(),
            0,
            "Alice shouldn't be able to get the treasure"
        );
    }
}
