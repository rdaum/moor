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

#[macro_use]
extern crate pest_derive;
pub use moor_var::program::names::Names;

mod ast;
mod codegen;
mod decompile;
mod diagnostics;
mod parse;
mod unparse;

mod codegen_tests;
mod objdef;
mod var_scope;

#[cfg(test)]
mod tests;

pub use crate::diagnostics::{
    DiagnosticRenderOptions, DiagnosticVerbosity, compile_error_to_map, emit_compile_error,
    format_compile_error,
};
pub use crate::{
    codegen::compile,
    decompile::program_to_tree,
    objdef::{
        ObjDefParseError, ObjFileContext, ObjPropDef, ObjPropOverride, ObjVerbDef,
        ObjectDefinition, compile_object_definitions, parse_literal_value,
    },
    parse::CompileOptions,
    unparse::{to_literal, to_literal_objsub, unparse},
};
// Re-export from var
pub use moor_common::builtins::{
    ArgCount, ArgType, BUILTINS, Builtin, BuiltinId, offset_for_builtin,
};
pub use moor_var::program::{
    labels::{JumpLabel, Label, Offset},
    opcode::{Op, ScatterLabel},
    program::{EMPTY_PROGRAM, Program},
    stored_program::StoredProgram,
};
pub use var_scope::VarScope;
