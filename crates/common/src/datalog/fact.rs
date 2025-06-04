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

use moor_var::{Symbol, Var};
use std::fmt::{Display, Formatter};

use super::atom::Atom;
use super::term::Term;

/// A Fact is a ground atom (an atom with no variables)
#[derive(Clone, Debug)]
pub struct Fact {
    /// Unique identifier for this fact instance
    pub id: u64,
    /// The predicate name
    predicate: Symbol,
    /// The constant values of the fact
    values: Vec<Var>,
}

impl Fact {
    /// Create a new fact with the given ID, predicate and values
    /// This is pub(crate) because ID assignment is managed by the Datalog engine.
    pub(crate) fn new(id: u64, predicate: Symbol, values: Vec<Var>) -> Self {
        Self {
            id,
            predicate,
            values,
        }
    }

    /// Get the unique ID of the fact
    pub fn id(&self) -> u64 {
        self.id
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
    pub(crate) fn to_atom(&self) -> Atom {
        let terms = self
            .values
            .iter()
            .map(|value| Term::Constant(value.clone()))
            .collect();
        Atom::new(self.predicate, terms)
    }
}

impl PartialEq for Fact {
    fn eq(&self, other: &Self) -> bool {
        self.predicate == other.predicate && self.values == other.values
    }
}

impl Eq for Fact {}

impl std::hash::Hash for Fact {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.predicate.hash(state);
        self.values.hash(state);
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
