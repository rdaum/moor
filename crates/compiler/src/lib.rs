// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use bincode::{Decode, Encode};
use strum::{Display, EnumCount, EnumIter, FromRepr};
use thiserror::Error;

pub mod ast;
pub mod builtins;
pub mod codegen;
pub mod decompile;
pub mod labels;
pub mod parse;
pub mod unparse;

mod codegen_tests;
pub mod opcode;

#[macro_use]
extern crate pest_derive;

/// The set of known variable names that are always set for every verb invocation.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, FromRepr, EnumCount, Display, EnumIter)]
#[repr(usize)]
#[allow(non_camel_case_types, non_snake_case)]
pub enum GlobalName {
    NUM = 0,
    OBJ,
    STR,
    LIST,
    ERR,
    INT,
    FLOAT,
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

#[derive(Debug, Error, Clone, Decode, Encode)]
pub enum CompileError {
    #[error("Failure to parse string: {0}")]
    StringLexError(String),
    #[error("Failure to parse program: {0}")]
    ParseError(String),
    #[error("Unknown built-in function: {0}")]
    UnknownBuiltinFunction(String),
    #[error("Could not find loop with id: {0}")]
    UnknownLoopLabel(String),
}
