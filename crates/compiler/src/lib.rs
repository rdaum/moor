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
pub use names::Names;

use strum::{Display, EnumCount, EnumIter, FromRepr};

mod ast;
mod builtins;
mod codegen;
mod decompile;
mod labels;
mod parse;
mod unparse;

mod codegen_tests;
mod names;
mod objdef;
mod opcode;
mod program;

pub use crate::builtins::{ArgCount, ArgType, BUILTINS, Builtin, BuiltinId, offset_for_builtin};
pub use crate::codegen::compile;
pub use crate::decompile::program_to_tree;
pub use crate::labels::{JumpLabel, Label, Offset};
pub use crate::names::{Name, UnboundNames};
pub use crate::objdef::{
    ObjDefParseError, ObjFileContext, ObjPropDef, ObjPropOverride, ObjVerbDef, ObjectDefinition,
    compile_object_definitions,
};
pub use crate::opcode::{Op, ScatterLabel};
pub use crate::parse::CompileOptions;
pub use crate::program::{EMPTY_PROGRAM, Program};
pub use crate::unparse::{to_literal, to_literal_objsub, unparse};

/// The set of known variable names that are always set for every verb invocation.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, FromRepr, EnumCount, Display, EnumIter)]
#[repr(usize)]
#[allow(non_camel_case_types, non_snake_case)]
pub enum GlobalName {
    player,
    this,
    caller,
    verb,
    args,
    argstr,
    dobj,
    dobjstr,
    prepstr,
    iobj,
    iobjstr,
}
