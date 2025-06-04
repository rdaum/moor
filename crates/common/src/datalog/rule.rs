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

/// A Rule is a Horn clause in the form: head :- body
/// where head is a single atom and body is a conjunction of atoms
#[derive(Clone, Debug)]
pub struct Rule {
    /// The head atom of the rule
    pub(crate) head: Atom,
    /// The body atoms of the rule
    pub(crate) body: Vec<Atom>,
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
