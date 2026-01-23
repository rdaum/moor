// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

pub use crate::model::{
    defset::{Defs, DefsIter, HasUuid, Named},
    r#match::{ArgSpec, PrepSpec, VerbArgsSpec, parse_preposition_spec, preposition_to_string},
    objects::{ObjAttr, ObjAttrs, ObjFlag, ObjectRef, obj_flags_string},
    objset::{ObjSet, ObjSetIter},
    permissions::Perms,
    propdef::{PropDef, PropDefs},
    props::{PropAttr, PropAttrs, PropFlag, PropPerms, prop_flags_string},
    verbdef::{VerbDef, VerbDefs},
    verbs::{BinaryType, VerbAttr, VerbAttrs, VerbFlag, Vid, verb_perms_string},
    world_state::{ObjectKind, WorldState, WorldStateSource},
};
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

/// Information about a transaction conflict.
/// Used to help diagnose which relation and key caused a commit conflict.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ConflictInfo {
    /// The name of the relation where the conflict occurred
    pub relation_name: Symbol,
    /// A string representation of the domain key that caused the conflict
    pub domain_key: String,
    /// Description of the conflict type
    pub conflict_type: ConflictType,
}

/// The type of conflict that occurred during commit
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ConflictType {
    /// An insert was attempted but the key already exists
    InsertDuplicate,
    /// A concurrent transaction modified this key with a newer timestamp
    ConcurrentWrite,
    /// The read timestamp was newer than the write timestamp (stale read)
    StaleRead,
    /// An update was attempted on a non-existent key
    UpdateNonExistent,
}

impl std::fmt::Display for ConflictType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConflictType::InsertDuplicate => write!(f, "insert_duplicate"),
            ConflictType::ConcurrentWrite => write!(f, "concurrent_write"),
            ConflictType::StaleRead => write!(f, "stale_read"),
            ConflictType::UpdateNonExistent => write!(f, "update_non_existent"),
        }
    }
}

impl std::fmt::Display for ConflictInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "conflict in relation '{}' on key '{}' ({})",
            self.relation_name, self.domain_key, self.conflict_type
        )
    }
}

/// The result code from a commit/complete operation on the world's state.
#[derive(Debug, Eq, PartialEq)]
pub enum CommitResult {
    Success {
        mutations_made: bool,
        timestamp: u64,
    }, // Value was committed
    ConflictRetry {
        /// Optional information about what caused the conflict.
        /// May be None if conflict was detected during apply phase without details.
        conflict_info: Option<ConflictInfo>,
    }, // Value was not committed due to conflict, caller should abort and retry tx_management
}

pub trait ValSet<V>: FromIterator<V> {
    fn empty() -> Self;
    fn from_items(items: &[V]) -> Self;
    fn iter(&self) -> impl Iterator<Item = V>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
}

#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct ParseErrorDetails {
    pub span: Option<(usize, usize)>,
    pub expected_tokens: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize)]
pub enum CompileError {
    #[error("Failure to parse string @ {0}: {1}")]
    StringLexError(CompileContext, String),
    #[error("Failure to parse program @ {error_position}:\n{message}")]
    ParseError {
        error_position: CompileContext,
        context: String,
        end_line_col: Option<(usize, usize)>,
        message: String,
        details: Box<ParseErrorDetails>,
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
    InvalidAssignmentTarget(CompileContext),
    #[error("Cannot assign to type constant literal `{0}` @ {1}")]
    InvalidTypeLiteralAssignment(String, CompileContext),
    #[error("Cannot assign to captured variable `{1}` @ {0}; lambdas capture by value")]
    AssignmentToCapturedVariable(CompileContext, Symbol),
}

impl CompileError {
    /// Convert the error to a list of error strings as expected by MOO's set_verb_code builtin.
    /// Since CompileError represents a single error, this returns a vector with one element.
    pub fn to_error_list(&self) -> Vec<String> {
        vec![self.to_string()]
    }

    /// Get the CompileContext from any CompileError variant
    pub fn context(&self) -> &CompileContext {
        match self {
            CompileError::StringLexError(context, _) => context,
            CompileError::ParseError { error_position, .. } => error_position,
            CompileError::UnknownBuiltinFunction(context, _) => context,
            CompileError::UnknownTypeConstant(context, _) => context,
            CompileError::UnknownLoopLabel(context, _) => context,
            CompileError::DuplicateVariable(context, _) => context,
            CompileError::AssignToConst(context, _) => context,
            CompileError::DisabledFeature(context, _) => context,
            CompileError::BadSlotName(context, _) => context,
            CompileError::InvalidAssignmentTarget(context) => context,
            CompileError::InvalidTypeLiteralAssignment(_, context) => context,
            CompileError::AssignmentToCapturedVariable(context, _) => context,
        }
    }
}
