use crate::model::r#match::PrepSpec;
use crate::model::var::{Objid, Var};

#[derive(Clone)]
pub struct ParsedCommand {
    pub(crate) verb: String,
    pub(crate) argstr: String,
    pub(crate) args: Vec<Var>,

    pub(crate) dobjstr: String,
    pub(crate) dobj: Objid,

    pub(crate) prepstr: String,
    pub(crate) prep: PrepSpec,

    pub(crate) iobjstr: String,
    pub(crate) iobj: Objid,
}