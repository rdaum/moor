use bincode::{Decode, Encode};

use crate::var::error::Error;
use crate::var::list::List;
use crate::var::objid::Objid;
use crate::var::string::Str;
use std::fmt::{Display, Formatter, Result as FmtResult};

use super::Var;

#[derive(Clone, Encode, Decode)]
pub enum Variant {
    None,
    Str(Str),
    Obj(Objid),
    Int(i64),
    Float(f64),
    Err(Error),
    List(List),
}

impl Display for Variant {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::None => write!(f, "None"),
            Self::Str(s) => write!(f, "{s}"),
            Self::Obj(o) => write!(f, "{o}"),
            Self::Int(i) => write!(f, "{i}"),
            Self::Float(fl) => write!(f, "{fl}"),
            Self::Err(e) => write!(f, "{e}"),
            Self::List(l) => write!(f, "{l}"),
        }
    }
}

impl From<Variant> for Var {
    fn from(val: Variant) -> Self {
        Self::new(val)
    }
}
