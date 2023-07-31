use bincode::{Decode, Encode};
use std::sync::Arc;

use crate::values::error::Error;
use crate::values::objid::Objid;
use crate::values::var::Var;

#[derive(Clone, Encode, Decode)]
pub enum Variant {
    Clear,
    None,
    Str(Arc<String>),
    Obj(Objid),
    Int(i64),
    Float(f64),
    Err(Error),
    List(Arc<Vec<Var>>),
}
