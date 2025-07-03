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

use crate::BincodeAsByteBufferExt;
use crate::program::names::Variable;
use crate::program::program::Program;
use bincode::{Decode, Encode};

pub mod labels;
pub mod names;
pub mod opcode;

#[allow(clippy::module_inception)]
pub mod program;

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub enum ProgramType {
    MooR(Program),
}

impl BincodeAsByteBufferExt for ProgramType {}

impl ProgramType {
    pub fn is_empty(&self) -> bool {
        match self {
            ProgramType::MooR(p) => p.main_vector().is_empty(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub enum DeclType {
    Global,
    Let,
    Assign,
    For,
    Unknown,
    Register,
    Except,
    WhileLabel,
    ForkLabel,
}

#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub struct Decl {
    /// The type of declaration, how was it declared?
    pub decl_type: DeclType,
    /// The name of the variable (or register id if a register).
    pub identifier: Variable,
    /// What scope the variable was declared in.
    pub depth: usize,
    /// Is this a constant? Reject subsequent assignments.
    pub constant: bool,
    /// The scope id of the variable.
    /// This is used to determine the scope of the variable when binding (or rebinding at decompile)
    pub scope_id: u16,
}
