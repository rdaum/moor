use bincode::{Decode, Encode};

use crate::compiler::labels::Label;
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
    // Special for exception handling
    // TODO this are profoundly inelegant, and need to go somehow.
    //   a) they're not really values, so shouldn't be here
    //   b) they tie a dependency up to `::compiler::labels` just for this
    // The core issue is just how to find labels in the program stream for certain error handling
    // opcodes, as the program stream holds exclusively `Var`s
    _Catch(usize),
    _Finally(Label),
    _Label(Label),
}
