use bincode::{Decode, Encode};

use crate::var::error::Error;
use crate::var::list::List;
use crate::var::objid::Objid;
use crate::var::string::Str;

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
