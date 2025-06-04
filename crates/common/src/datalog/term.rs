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
use moor_var::Var;
use std::fmt::{Display, Formatter};

use super::Substitution;

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
    pub(crate) fn apply_substitution(&self, substitution: &Substitution) -> Self {
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
