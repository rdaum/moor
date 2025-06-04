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

mod atom;
mod error;
mod fact;
mod knowledge_base;
mod relation;
mod rule;
mod term;
mod variable;

pub use atom::Atom;
pub use error::{DatalogError, DatalogResult};
pub use fact::Fact;
pub use knowledge_base::KnowledgeBase;
use moor_var::Var;
pub use relation::{HashSetRelation, RelationBackend};
pub use rule::{AggregateLiteral, AggregateOp, Literal, Rule};
use std::collections::HashMap;
pub use term::Term;
pub use variable::Variable;

/// A Substitution is a mapping from variables to values
pub type Substitution = HashMap<Variable, Var>;
