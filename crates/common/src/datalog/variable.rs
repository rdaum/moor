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

use bincode::{Decode, Encode};
use moor_var::Symbol;
use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};

/// A Variable represents a logic variable in Datalog
#[derive(Clone, Debug, Encode, Decode)]
pub struct Variable {
    /// The name of the variable, used for debugging and pretty printing
    name: Symbol,
    /// A unique identifier for the variable
    id: usize,
}

impl Variable {
    /// Create a new variable with the given name and id
    pub fn new(name: impl Into<Symbol>, id: usize) -> Self {
        Self {
            name: name.into(),
            id,
        }
    }

    /// Get the name of the variable
    pub fn name(&self) -> &Symbol {
        &self.name
    }

    /// Get the id of the variable
    pub fn id(&self) -> usize {
        self.id
    }
}

impl PartialEq for Variable {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Variable {}

impl Hash for Variable {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Display for Variable {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "?{}_{}", self.name.as_str(), self.id)
    }
}
