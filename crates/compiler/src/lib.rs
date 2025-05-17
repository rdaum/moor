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
pub use moor_common::program::names::Names;

mod ast;
mod codegen;
mod decompile;
mod parse;
mod unparse;

mod codegen_tests;
mod objdef;
mod var_scope;

pub use crate::codegen::compile;
pub use crate::decompile::program_to_tree;
pub use crate::objdef::{
    ObjDefParseError, ObjFileContext, ObjPropDef, ObjPropOverride, ObjVerbDef, ObjectDefinition,
    compile_object_definitions,
};
pub use crate::parse::CompileOptions;
pub use crate::unparse::{to_literal, to_literal_objsub, unparse};
pub use moor_common::program::builtins::{
    ArgCount, ArgType, BUILTINS, Builtin, BuiltinId, offset_for_builtin,
};
pub use moor_common::program::labels::{JumpLabel, Label, Offset};
pub use moor_common::program::opcode::{Op, ScatterLabel};
pub use moor_common::program::program::{EMPTY_PROGRAM, Program};
pub use var_scope::{DeclType, VarScope};
