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

use std::fmt::{Display, Formatter};

use binary_layout::LayoutAs;
use bincode::{Decode, Encode};
use strum::FromRepr;

use crate::encode::{DecodingError, EncodingError};
use crate::var::{v_none, Var};

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, FromRepr, Ord, PartialOrd, Hash, Encode, Decode)]
#[allow(non_camel_case_types)]
pub enum Error {
    E_NONE = 0,
    E_TYPE = 1,
    E_DIV = 2,
    E_PERM = 3,
    E_PROPNF = 4,
    E_VERBNF = 5,
    E_VARNF = 6,
    E_INVIND = 7,
    E_RECMOVE = 8,
    E_MAXREC = 9,
    E_RANGE = 10,
    E_ARGS = 11,
    E_NACC = 12,
    E_INVARG = 13,
    E_QUOTA = 14,
    E_FLOAT = 15,
}

impl LayoutAs<u8> for Error {
    type ReadError = DecodingError;
    type WriteError = EncodingError;

    fn try_read(v: u8) -> Result<Self, Self::ReadError> {
        Self::from_repr(v).ok_or(DecodingError::InvalidErrorValue(v))
    }

    fn try_write(v: Self) -> Result<u8, Self::WriteError> {
        Ok(v as u8)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl std::error::Error for Error {}
#[derive(Debug)]
pub struct ErrorPack {
    pub code: Error,
    pub msg: String,
    pub value: Var,
}

impl Error {
    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            Self::E_NONE => "No error",
            Self::E_TYPE => "Type mismatch",
            Self::E_DIV => "Division by zero",
            Self::E_PERM => "Permission denied",
            Self::E_PROPNF => "Property not found",
            Self::E_VERBNF => "Verb not found",
            Self::E_VARNF => "Variable not found",
            Self::E_INVIND => "Invalid indirection",
            Self::E_RECMOVE => "Recursive move",
            Self::E_MAXREC => "Too many verb calls",
            Self::E_RANGE => "Range error",
            Self::E_ARGS => "Incorrect number of arguments",
            Self::E_NACC => "Move refused by destination",
            Self::E_INVARG => "Invalid argument",
            Self::E_QUOTA => "Resource limit exceeded",
            Self::E_FLOAT => "Floating-point arithmetic error",
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::E_NONE => "E_NONE",
            Self::E_TYPE => "E_TYPE",
            Self::E_DIV => "E_DIV",
            Self::E_PERM => "E_PERM",
            Self::E_PROPNF => "E_PROPNF",
            Self::E_VERBNF => "E_VERBNF",
            Self::E_VARNF => "E_VARNF",
            Self::E_INVIND => "E_INVIND",
            Self::E_RECMOVE => "E_RECMOVE",
            Self::E_MAXREC => "E_MAXREC",
            Self::E_RANGE => "E_RANGE",
            Self::E_ARGS => "E_ARGS",
            Self::E_NACC => "E_NACC",
            Self::E_INVARG => "E_INVARG",
            Self::E_QUOTA => "E_QUOTA",
            Self::E_FLOAT => "E_FLOAT",
        }
    }

    #[must_use]
    pub fn make_raise_pack(&self, msg: String, value: Var) -> ErrorPack {
        ErrorPack {
            code: *self,
            msg,
            value,
        }
    }

    #[must_use]
    pub fn make_error_pack(&self, msg: Option<String>, value: Option<Var>) -> ErrorPack {
        ErrorPack {
            code: *self,
            msg: msg.unwrap_or(self.message().to_string()),
            value: value.unwrap_or(v_none()),
        }
    }

    pub fn parse_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "E_NONE" => Some(Self::E_NONE),
            "E_TYPE" => Some(Self::E_TYPE),
            "E_DIV" => Some(Self::E_DIV),
            "E_PERM" => Some(Self::E_PERM),
            "E_PROPNF" => Some(Self::E_PROPNF),
            "E_VERBNF" => Some(Self::E_VERBNF),
            "E_VARNF" => Some(Self::E_VARNF),
            "E_INVIND" => Some(Self::E_INVIND),
            "E_RECMOVE" => Some(Self::E_RECMOVE),
            "E_MAXREC" => Some(Self::E_MAXREC),
            "E_RANGE" => Some(Self::E_RANGE),
            "E_ARGS" => Some(Self::E_ARGS),
            "E_NACC" => Some(Self::E_NACC),
            "E_INVARG" => Some(Self::E_INVARG),
            "E_QUOTA" => Some(Self::E_QUOTA),
            "E_FLOAT" => Some(Self::E_FLOAT),
            _ => None,
        }
    }
}
