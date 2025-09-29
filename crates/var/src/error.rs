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

use crate::{Symbol, var::Var};
use ErrorCode::*;
use bincode::{Decode, Encode};
use std::{
    fmt::{Debug, Display, Formatter},
    hash::{Hash, Hasher},
    ops::Deref,
};

#[derive(Clone, Eq, Ord, PartialOrd, Encode, Decode)]
pub struct Error {
    pub err_type: ErrorCode,
    pub msg: Option<Box<String>>,
    pub value: Option<Box<Var>>,
}

impl Hash for Error {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.err_type.hash(state);
    }
}

impl Error {
    pub fn new(err_type: ErrorCode, msg: Option<String>, value: Option<Var>) -> Self {
        Self {
            err_type,
            msg: msg.map(Box::new),
            value: value.map(Box::new),
        }
    }
}

// TODO: Debug for Error should be more informative, but we need to be careful about what it returns
//   because the `moot` tests use this to compare results via string comparison, and we don't want
//   to break that.  We need to do some work in the moot test runner to make it handle error comparisons
//   better, and then we can make this more informative again.
impl Debug for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.err_type)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.msg.is_some() {
            write!(f, "{} ({})", self.err_type, self.message())
        } else {
            write!(f, "{}", self.err_type)
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Encode, Decode)]
#[allow(non_camel_case_types)]
pub enum ErrorCode {
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
    // Our own extension
    ErrCustom(Symbol),
}

impl ErrorCode {
    pub fn parse_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "E_NONE" => Some(E_NONE),
            "E_TYPE" => Some(E_TYPE),
            "E_DIV" => Some(E_DIV),
            "E_PERM" => Some(E_PERM),
            "E_PROPNF" => Some(E_PROPNF),
            "E_VERBNF" => Some(E_VERBNF),
            "E_VARNF" => Some(E_VARNF),
            "E_INVIND" => Some(E_INVIND),
            "E_RECMOVE" => Some(E_RECMOVE),
            "E_MAXREC" => Some(E_MAXREC),
            "E_RANGE" => Some(E_RANGE),
            "E_ARGS" => Some(E_ARGS),
            "E_NACC" => Some(E_NACC),
            "E_INVARG" => Some(E_INVARG),
            "E_QUOTA" => Some(E_QUOTA),
            "E_FLOAT" => Some(E_FLOAT),
            "E_FILE" => Some(E_FILE),
            "E_EXEC" => Some(E_EXEC),
            "E_INTRPT" => Some(E_INTRPT),
            s => Some(ErrCustom(Symbol::mk(s))),
        }
    }
}

impl From<ErrorCode> for String {
    fn from(val: ErrorCode) -> Self {
        match val {
            E_NONE => "E_NONE".into(),
            E_TYPE => "E_TYPE".into(),
            E_DIV => "E_DIV".into(),
            E_PERM => "E_PERM".into(),
            E_PROPNF => "E_PROPNF".into(),
            E_VERBNF => "E_VERBNF".into(),
            E_VARNF => "E_VARNF".into(),
            E_INVIND => "E_INVIND".into(),
            E_RECMOVE => "E_RECMOVE".into(),
            E_MAXREC => "E_MAXREC".into(),
            E_RANGE => "E_RANGE".into(),
            E_ARGS => "E_ARGS".into(),
            E_NACC => "E_NACC".into(),
            E_INVARG => "E_INVARG".into(),
            E_QUOTA => "E_QUOTA".into(),
            E_FLOAT => "E_FLOAT".into(),
            E_FILE => "E_FILE".into(),
            E_EXEC => "E_EXEC".into(),
            E_INTRPT => "E_INTRPT".into(),
            ErrCustom(sym) => sym.to_string(),
        }
    }
}

impl Display for ErrorCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let s: String = (*self).into();
        write!(f, "{s}")
    }
}

impl ErrorCode {
    pub fn msg<S: ToString>(self, s: S) -> Error {
        Error::new(self, Some(s.to_string()), None)
    }

    pub fn with_msg<F>(self, f: F) -> Error
    where
        F: FnOnce() -> String,
    {
        Error::new(self, Some(f()), None)
    }

    pub fn with_msg_and_value<F>(self, f: F, value: Var) -> Error
    where
        F: FnOnce() -> String,
    {
        Error::new(self, Some(f()), Some(value))
    }
}

// TODO: this presents difficulties/confusions for Hash, Clippy warns about it.
impl PartialEq<ErrorCode> for Error {
    fn eq(&self, other: &ErrorCode) -> bool {
        self.err_type == *other
    }
}

impl PartialEq<Error> for Error {
    fn eq(&self, other: &Error) -> bool {
        *self == other.err_type && self.value == other.value
    }
}

impl From<ErrorCode> for Error {
    fn from(val: ErrorCode) -> Self {
        Error::new(val, None, None)
    }
}

impl Error {
    pub fn from_repr(v: u8) -> Option<Self> {
        let err_code = match v {
            0 => Some(E_NONE),
            1 => Some(E_TYPE),
            2 => Some(E_DIV),
            3 => Some(E_PERM),
            4 => Some(E_PROPNF),
            5 => Some(E_VERBNF),
            6 => Some(E_VARNF),
            7 => Some(E_INVIND),
            8 => Some(E_RECMOVE),
            9 => Some(E_MAXREC),
            10 => Some(E_RANGE),
            11 => Some(E_ARGS),
            12 => Some(E_NACC),
            13 => Some(E_INVARG),
            14 => Some(E_QUOTA),
            15 => Some(E_FLOAT),
            16 => Some(E_FILE),
            17 => Some(E_EXEC),
            18 => Some(E_INTRPT),
            _ => None,
        }?;
        Some(Error::new(err_code, None, None))
    }

    pub fn to_int(&self) -> Option<u8> {
        match self.err_type {
            E_NONE => Some(0),
            E_TYPE => Some(1),
            E_DIV => Some(2),
            E_PERM => Some(3),
            E_PROPNF => Some(4),
            E_VERBNF => Some(5),
            E_VARNF => Some(6),
            E_INVIND => Some(7),
            E_RECMOVE => Some(8),
            E_MAXREC => Some(9),
            E_RANGE => Some(10),
            E_ARGS => Some(11),
            E_NACC => Some(12),
            E_INVARG => Some(13),
            E_QUOTA => Some(14),
            E_FLOAT => Some(15),
            E_FILE => Some(16),
            E_EXEC => Some(17),
            E_INTRPT => Some(18),
            _ => None,
        }
    }
}

impl std::error::Error for Error {}

impl Error {
    #[must_use]
    pub fn message(&self) -> String {
        if let Some(msg) = &self.msg {
            return msg.deref().clone();
        }
        // Default message if one not provided.
        match self.err_type {
            E_NONE => "No error".into(),
            E_TYPE => "Type mismatch".into(),
            E_DIV => "Division by zero".into(),
            E_PERM => "Permission denied".into(),
            E_PROPNF => "Property not found".into(),
            E_VERBNF => "Verb not found".into(),
            E_VARNF => "Variable not found".into(),
            E_INVIND => "Invalid indirection".into(),
            E_RECMOVE => "Recursive move".into(),
            E_MAXREC => "Too many verb calls".into(),
            E_RANGE => "Range error".into(),
            E_ARGS => "Incorrect number of arguments".into(),
            E_NACC => "Move refused by destination".into(),
            E_INVARG => "Invalid argument".into(),
            E_QUOTA => "Resource limit exceeded".into(),
            E_FLOAT => "Floating-point arithmetic error".into(),
            E_FILE => "File error".into(),
            E_EXEC => "Execution error".into(),
            E_INTRPT => "Interruption".into(),
            ErrCustom(sym) => format!("Custom error: {sym}"),
        }
    }

    #[must_use]
    pub fn name(&self) -> Symbol {
        Symbol::mk(&format!("{}", self.err_type))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn test_error_size() {
        assert_eq!(size_of::<Error>(), 32);
    }
}
