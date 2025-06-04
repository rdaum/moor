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

use std::fmt::{Display, Formatter};

use super::atom::Atom;

/// A literal in a rule body: positive or negated atom
#[derive(Clone, Debug)]
pub enum Literal {
    Pos(Atom),
    Neg(Atom),
}

/// A Rule is a Horn clause in the form: head :- body
/// where head is a single atom and body is a conjunction of atoms
#[derive(Clone, Debug)]
pub struct Rule {
    /// The head atom of the rule
    pub(crate) head: Atom,
    /// The body literals of the rule
    pub(crate) body: Vec<Literal>,
}

impl Rule {
    /// Create a new rule with the given head and body
    pub fn new(head: Atom, body: Vec<Atom>) -> Self {
        // convenience for all-positive body
        let lits = body.into_iter().map(Literal::Pos).collect();
        Self { head, body: lits }
    }
    /// Create a new rule allowing negation
    pub fn with_negation(head: Atom, body: Vec<Literal>) -> Self {
        Self { head, body }
    }
}

impl Display for Rule {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} :- ", self.head)?;
        for (i, lit) in self.body.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            match lit {
                Literal::Pos(a) => write!(f, "{}", a)?,
                Literal::Neg(a) => write!(f, "not {}", a)?,
            }
        }
        Ok(())
    }
}
