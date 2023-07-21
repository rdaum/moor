use bincode::{Decode, Encode};
use int_enum::IntEnum;

use crate::var::{Var, VAR_NONE};

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, IntEnum, Ord, PartialOrd, Hash, Encode, Decode)]
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

pub struct ErrorPack {
    pub code: Error,
    pub msg: String,
    pub value: Var,
}

impl Error {
    pub fn message(&self) -> &str {
        match self {
            Error::E_NONE => "No error",
            Error::E_TYPE => "Type mismatch",
            Error::E_DIV => "Division by zero",
            Error::E_PERM => "Permission denied",
            Error::E_PROPNF => "Property not found",
            Error::E_VERBNF => "Verb not found",
            Error::E_VARNF => "Variable not found",
            Error::E_INVIND => "Invalid indirection",
            Error::E_RECMOVE => "Recursive move",
            Error::E_MAXREC => "Too many verb calls",
            Error::E_RANGE => "Range error",
            Error::E_ARGS => "Incorrect number of arguments",
            Error::E_NACC => "Move refused by destination",
            Error::E_INVARG => "Invalid argument",
            Error::E_QUOTA => "Resource limit exceeded",
            Error::E_FLOAT => "Floating-point arithmetic error",
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Error::E_NONE => "E_NONE",
            Error::E_TYPE => "E_TYPE",
            Error::E_DIV => "E_DIV",
            Error::E_PERM => "E_PERM",
            Error::E_PROPNF => "E_PROPNF",
            Error::E_VERBNF => "E_VERBNF",
            Error::E_VARNF => "E_VARNF",
            Error::E_INVIND => "E_INVIND",
            Error::E_RECMOVE => "E_RECMOVE",
            Error::E_MAXREC => "E_MAXREC",
            Error::E_RANGE => "E_RANGE",
            Error::E_ARGS => "E_ARGS",
            Error::E_NACC => "E_NACC",
            Error::E_INVARG => "E_INVARG",
            Error::E_QUOTA => "E_QUOTA",
            Error::E_FLOAT => "E_FLOAT",
        }
    }

    pub fn make_raise_pack(&self, msg: String, value: Var) -> ErrorPack {
        ErrorPack {
            code: *self,
            msg,
            value,
        }
    }

    pub fn make_error_pack(&self, msg: Option<String>) -> ErrorPack {
        ErrorPack {
            code: *self,
            msg: msg.unwrap_or(self.message().to_string()),
            value: VAR_NONE,
        }
    }
}
