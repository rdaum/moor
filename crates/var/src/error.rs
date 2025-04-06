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

use bincode::{Decode, Encode};

use crate::Symbol;
use crate::var::{Var, v_none};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Encode, Decode)]
#[allow(non_camel_case_types)]
pub enum Error {
    E_NONE,
    E_TYPE,
    E_DIV,
    E_PERM,
    E_PROPNF,
    E_VERBNF,
    E_VARNF,
    E_INVIND,
    E_RECMOVE,
    E_MAXREC,
    E_RANGE,
    E_ARGS,
    E_NACC,
    E_INVARG,
    E_QUOTA,
    E_FLOAT,
    // Toast extensions:
    E_FILE,
    E_EXEC,
    E_INTRPT,
    Custom(Symbol),
}

impl Error {
    pub fn from_repr(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::E_NONE),
            1 => Some(Self::E_TYPE),
            2 => Some(Self::E_DIV),
            3 => Some(Self::E_PERM),
            4 => Some(Self::E_PROPNF),
            5 => Some(Self::E_VERBNF),
            6 => Some(Self::E_VARNF),
            7 => Some(Self::E_INVIND),
            8 => Some(Self::E_RECMOVE),
            9 => Some(Self::E_MAXREC),
            10 => Some(Self::E_RANGE),
            11 => Some(Self::E_ARGS),
            12 => Some(Self::E_NACC),
            13 => Some(Self::E_INVARG),
            14 => Some(Self::E_QUOTA),
            15 => Some(Self::E_FLOAT),
            16 => Some(Self::E_FILE),
            17 => Some(Self::E_EXEC),
            18 => Some(Self::E_INTRPT),
            _ => None,
        }
    }

    pub fn to_int(&self) -> Option<u8> {
        match self {
            Self::E_NONE => Some(0),
            Self::E_TYPE => Some(1),
            Self::E_DIV => Some(2),
            Self::E_PERM => Some(3),
            Self::E_PROPNF => Some(4),
            Self::E_VERBNF => Some(5),
            Self::E_VARNF => Some(6),
            Self::E_INVIND => Some(7),
            Self::E_RECMOVE => Some(8),
            Self::E_MAXREC => Some(9),
            Self::E_RANGE => Some(10),
            Self::E_ARGS => Some(11),
            Self::E_NACC => Some(12),
            Self::E_INVARG => Some(13),
            Self::E_QUOTA => Some(14),
            Self::E_FLOAT => Some(15),
            Self::E_FILE => Some(16),
            Self::E_EXEC => Some(17),
            Self::E_INTRPT => Some(18),
            _ => None,
        }
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

impl ErrorPack {
    pub fn new(code: Error, msg: String, value: Var) -> Self {
        Self { code, msg, value }
    }
}

impl From<Error> for ErrorPack {
    fn from(value: Error) -> Self {
        ErrorPack::new(value, value.message().to_string(), v_none())
    }
}

impl Error {
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::E_NONE => "No error".into(),
            Self::E_TYPE => "Type mismatch".into(),
            Self::E_DIV => "Division by zero".into(),
            Self::E_PERM => "Permission denied".into(),
            Self::E_PROPNF => "Property not found".into(),
            Self::E_VERBNF => "Verb not found".into(),
            Self::E_VARNF => "Variable not found".into(),
            Self::E_INVIND => "Invalid indirection".into(),
            Self::E_RECMOVE => "Recursive move".into(),
            Self::E_MAXREC => "Too many verb calls".into(),
            Self::E_RANGE => "Range error".into(),
            Self::E_ARGS => "Incorrect number of arguments".into(),
            Self::E_NACC => "Move refused by destination".into(),
            Self::E_INVARG => "Invalid argument".into(),
            Self::E_QUOTA => "Resource limit exceeded".into(),
            Self::E_FLOAT => "Floating-point arithmetic error".into(),
            Self::E_FILE => "File error".into(),
            Self::E_EXEC => "Execution error".into(),
            Self::E_INTRPT => "Interruption".into(),
            Self::Custom(sym) => format!("Error: {}", sym.as_str().to_uppercase()),
        }
    }

    #[must_use]
    pub fn name(&self) -> Symbol {
        match self {
            Self::E_NONE => "E_NONE".into(),
            Self::E_TYPE => "E_TYPE".into(),
            Self::E_DIV => "E_DIV".into(),
            Self::E_PERM => "E_PERM".into(),
            Self::E_PROPNF => "E_PROPNF".into(),
            Self::E_VERBNF => "E_VERBNF".into(),
            Self::E_VARNF => "E_VARNF".into(),
            Self::E_INVIND => "E_INVIND".into(),
            Self::E_RECMOVE => "E_RECMOVE".into(),
            Self::E_MAXREC => "E_MAXREC".into(),
            Self::E_RANGE => "E_RANGE".into(),
            Self::E_ARGS => "E_ARGS".into(),
            Self::E_NACC => "E_NACC".into(),
            Self::E_INVARG => "E_INVARG".into(),
            Self::E_QUOTA => "E_QUOTA".into(),
            Self::E_FLOAT => "E_FLOAT".into(),
            Self::E_FILE => "E_FILE".into(),
            Self::E_EXEC => "E_EXEC".into(),
            Self::E_INTRPT => "E_INTRPT".into(),
            Self::Custom(sym) => *sym,
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
            s => Some(Self::Custom(Symbol::mk_case_insensitive(s))),
        }
    }
}
