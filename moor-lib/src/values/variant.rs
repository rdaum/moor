use bincode::{Decode, Encode};

use crate::values::error::Error;
use crate::values::objid::Objid;
use crate::values::var::Var;

#[derive(Clone, Encode, Decode)]
pub enum Variant {
    Clear,
    None,
    Str(String),
    Obj(Objid),
    Int(i64),
    Float(f64),
    Err(Error),
    List(Vec<Var>),
}
