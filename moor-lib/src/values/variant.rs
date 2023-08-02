use bincode::{Decode, Encode};

use crate::values::error::Error;
use crate::values::list::List;
use crate::values::objid::Objid;
use crate::values::string::Str;

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
