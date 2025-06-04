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

use moor_var::Symbol;
use std::fmt::{Display, Formatter};

use super::Substitution;
use super::term::Term;

/// An Atom is a predicate with terms
#[derive(Clone, Debug)]
pub struct Atom {
    /// The predicate name
    pub(crate) predicate: Symbol,
    /// The terms of the atom
    pub(crate) terms: Vec<Term>,
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
    pub(crate) fn apply_substitution(&self, substitution: &Substitution) -> Self {
        let terms = self
            .terms
            .iter()
            .map(|term| term.apply_substitution(substitution))
            .collect();
        Self {
            predicate: self.predicate,
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
