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

//! Traits for MOO world state introspection.

use moor_common::model::{PropDef, PropPerms, VerbDef};
use moor_var::program::ProgramType;
use moor_var::{Obj, Var};

/// Result type for introspection operations.
pub type IntrospectionResult<T> = Result<T, IntrospectionError>;

/// Errors from introspection operations.
#[derive(Debug, thiserror::Error)]
pub enum IntrospectionError {
    #[error("Object not found: {0}")]
    ObjectNotFound(Obj),
    #[error("Verb not found: {0}")]
    VerbNotFound(String),
    #[error("Property not found: {0}")]
    PropertyNotFound(String),
    #[error("Permission denied")]
    PermissionDenied,
    #[error("Database error: {0}")]
    DatabaseError(String),
}

/// Introspection interface for MOO world state.
///
/// Provides read-only access to object, verb, and property information.
/// Uses existing types from moor_common - no source generation.
/// LSP layer handles parse/unparse via embedded compiler.
pub trait MoorIntrospection {
    /// List verbs on object.
    /// If include_inherited is true, walks up parent chain.
    /// VerbDef.location() indicates which object defines each verb.
    fn list_verbs(
        &self,
        obj: &Obj,
        include_inherited: bool,
    ) -> IntrospectionResult<Vec<VerbDef>>;

    /// List properties with their permissions.
    /// If include_inherited is true, walks up parent chain.
    /// PropDef.definer() indicates where property was defined.
    fn list_properties(
        &self,
        obj: &Obj,
        include_inherited: bool,
    ) -> IntrospectionResult<Vec<(PropDef, PropPerms)>>;

    /// Get verb definition and compiled bytecode.
    /// LSP calls program_to_tree() + unparse() for source.
    fn get_verb(
        &self,
        obj: &Obj,
        verb_name: &str,
    ) -> IntrospectionResult<(VerbDef, ProgramType)>;

    /// Get property value (walks inheritance).
    fn get_property(&self, obj: &Obj, prop_name: &str) -> IntrospectionResult<Var>;

    /// Get property definition and permissions.
    fn get_property_info(
        &self,
        obj: &Obj,
        prop_name: &str,
    ) -> IntrospectionResult<(PropDef, PropPerms)>;

    /// List all valid objects in the database.
    fn list_objects(&self) -> IntrospectionResult<Vec<Obj>>;
}
