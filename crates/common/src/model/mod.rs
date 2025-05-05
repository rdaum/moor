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

pub use crate::model::defset::{Defs, DefsIter, HasUuid, Named};
pub use crate::model::r#match::{
    ArgSpec, PrepSpec, VerbArgsSpec, parse_preposition_spec, preposition_to_string,
};
pub use crate::model::objects::{ObjAttr, ObjAttrs, ObjFlag, ObjectRef};
pub use crate::model::objset::{ObjSet, ObjSetIter};
pub use crate::model::permissions::Perms;
pub use crate::model::propdef::{PropDef, PropDefs};
pub use crate::model::props::{PropAttr, PropAttrs, PropFlag, PropPerms, prop_flags_string};
pub use crate::model::verbdef::{VerbDef, VerbDefs};
pub use crate::model::verbs::{BinaryType, VerbAttr, VerbAttrs, VerbFlag, Vid, verb_perms_string};
pub use crate::model::world_state::{WorldState, WorldStateSource};
use bincode::{Decode, Encode};
use moor_var::AsByteBuffer;
use serde::Serialize;
use std::fmt::{Debug, Display};
use thiserror::Error;

mod defset;
pub mod loader;
mod r#match;
mod objects;
mod objset;
mod permissions;
mod propdef;
mod props;
mod verbdef;
mod verbs;
mod world_state;

use moor_var::Symbol;
pub use world_state::{WorldStateError, WorldStatePerf};

/// The result code from a commit/complete operation on the world's state.
#[derive(Debug, Eq, PartialEq)]
pub enum CommitResult {
    Success,       // Value was committed
    ConflictRetry, // Value was not committed due to conflict, caller should abort and retry tx_management
}

pub trait ValSet<V: AsByteBuffer>: FromIterator<V> {
    fn empty() -> Self;
    fn from_items(items: &[V]) -> Self;
    fn iter(&self) -> impl Iterator<Item = V>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
}

#[derive(Debug, Error, Clone, Decode, Encode, PartialEq, Eq, Serialize)]
pub struct CompileContext {
    pub line_col: (usize, usize),
}

impl CompileContext {
    pub fn new(line_col: (usize, usize)) -> Self {
        Self { line_col }
    }
}
impl Display for CompileContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.line_col.0, self.line_col.1)?;
        Ok(())
    }
}

#[derive(Debug, Error, Clone, Decode, Encode, PartialEq, Eq, Serialize)]
pub enum CompileError {
    #[error("Failure to parse string @ {0}: {1}")]
    StringLexError(CompileContext, String),
    #[error("Failure to parse program @ {error_position}: {message}")]
    ParseError {
        error_position: CompileContext,
        context: String,
        end_line_col: Option<(usize, usize)>,
        message: String,
    },
    #[error("Unknown built-in function @ {0}: {1}")]
    UnknownBuiltinFunction(CompileContext, String),
    #[error("Unknown type constant @ {0}: {1}")]
    UnknownTypeConstant(CompileContext, String),
    #[error("Could not find loop with id @ {0}: {1}")]
    UnknownLoopLabel(CompileContext, String),
    #[error("Duplicate variable in scope @ {0}: {1}")]
    DuplicateVariable(CompileContext, Symbol),
    #[error("Cannot assign to const @ {0}: {1}")]
    AssignToConst(CompileContext, Symbol),
    #[error("Disabled feature @ {0}: {1}")]
    DisabledFeature(CompileContext, String),
    #[error("Bad slot name on flyweight @ {0}: {1}")]
    BadSlotName(CompileContext, String),
    #[error("Invalid l-value for assignment @ {0}")]
    InvalidAssignemnt(CompileContext),
}
