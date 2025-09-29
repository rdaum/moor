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

use crate::{model::r#match::VerbArgsSpec, util::BitEnum};
use binary_layout::LayoutAs;
use bincode::{Decode, Encode};
use enum_primitive_derive::Primitive;
use moor_var::{
    Obj, Symbol,
    encode::{DecodingError, EncodingError},
    program::ProgramType,
};
use num_traits::FromPrimitive;

#[derive(Debug, Ord, PartialOrd, Copy, Clone, Eq, PartialEq, Hash, Primitive, Encode, Decode)]
pub enum VerbFlag {
    Read = 0,
    Write = 1,
    Exec = 2,
    Debug = 3,
}

impl LayoutAs<u8> for VerbFlag {
    type ReadError = DecodingError;
    type WriteError = EncodingError;

    fn try_read(v: u8) -> Result<Self, Self::ReadError> {
        Self::from_u8(v).ok_or(DecodingError::InvalidVerbFlagValue(v))
    }

    fn try_write(v: Self) -> Result<u8, Self::WriteError> {
        Ok(v as u8)
    }
}

pub fn verb_perms_string(perms: BitEnum<VerbFlag>) -> String {
    let mut perms_string = String::new();
    if perms.contains(VerbFlag::Read) {
        perms_string.push('r');
    }
    if perms.contains(VerbFlag::Write) {
        perms_string.push('w');
    }
    if perms.contains(VerbFlag::Exec) {
        perms_string.push('x');
    }
    if perms.contains(VerbFlag::Debug) {
        perms_string.push('d');
    }

    perms_string
}

impl VerbFlag {
    pub fn parse_str(s: &str) -> Option<BitEnum<Self>> {
        let mut flags: u8 = 0;
        for c in s.chars() {
            if c == 'r' {
                flags |= 1 << VerbFlag::Read as u8;
            } else if c == 'w' {
                flags |= 1 << VerbFlag::Write as u8;
            } else if c == 'x' {
                flags |= 1 << VerbFlag::Exec as u8;
            } else if c == 'd' {
                flags |= 1 << VerbFlag::Debug as u8;
            } else {
                return None;
            }
        }

        Some(BitEnum::from_u8(flags))
    }

    #[must_use]
    pub fn rwxd() -> BitEnum<Self> {
        BitEnum::new_with(Self::Read) | Self::Write | Self::Exec | Self::Debug
    }
    #[must_use]
    pub fn rwx() -> BitEnum<Self> {
        BitEnum::new_with(Self::Read) | Self::Write | Self::Exec
    }
    #[must_use]
    pub fn rw() -> BitEnum<Self> {
        BitEnum::new_with(Self::Read) | Self::Write
    }
    #[must_use]
    pub fn rx() -> BitEnum<Self> {
        BitEnum::new_with(Self::Read) | Self::Exec
    }
    #[must_use]
    pub fn rxd() -> BitEnum<Self> {
        BitEnum::new_with(Self::Read) | Self::Exec | Self::Debug
    }
    #[must_use]
    pub fn r() -> BitEnum<Self> {
        BitEnum::new_with(Self::Read)
    }

    #[must_use]
    pub fn w() -> BitEnum<Self> {
        BitEnum::new_with(Self::Write)
    }
    #[must_use]
    pub fn x() -> BitEnum<Self> {
        BitEnum::new_with(Self::Exec)
    }
    #[must_use]
    pub fn d() -> BitEnum<Self> {
        BitEnum::new_with(Self::Debug)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Hash, Encode, Decode)]
pub struct Vid(pub i64);

#[derive(Clone, Copy, Debug, Primitive)]
pub enum VerbAttr {
    Definer = 0,
    Owner = 1,
    Flags = 2,
    ArgsSpec = 3,
    Binary = 4,
}

/// The program type encoded for a verb.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Encode, Decode, Primitive)]
#[repr(u8)]
pub enum BinaryType {
    /// For builtin functions in stack frames -- or empty code blobs.
    None = 0,
    /// Opcodes match almost 1:1 with LambdaMOO 1.8.x, but is not "binary" compatible.
    LambdaMoo18X = 1,
}

impl LayoutAs<u8> for BinaryType {
    type ReadError = DecodingError;
    type WriteError = EncodingError;

    fn try_read(v: u8) -> Result<Self, Self::ReadError> {
        Self::from_u8(v).ok_or(DecodingError::InvalidBinaryTypeValue(v))
    }

    fn try_write(v: Self) -> Result<u8, Self::WriteError> {
        Ok(v as u8)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VerbAttrs {
    pub definer: Option<Obj>,
    pub owner: Option<Obj>,
    pub names: Option<Vec<Symbol>>,
    pub flags: Option<BitEnum<VerbFlag>>,
    pub args_spec: Option<VerbArgsSpec>,
    pub program: Option<ProgramType>,
}
