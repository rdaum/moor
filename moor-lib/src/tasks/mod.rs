use moor_value::var::objid::Objid;
use moor_value::var::Var;

pub mod command_parse;
mod moo_vm_host;
pub mod scheduler;
pub mod sessions;
mod task;
mod vm_host;

pub type TaskId = usize;

/// The minimum set of information needed to make a *resolution* call for a verb.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct VerbCall {
    pub verb_name: String,
    pub location: Objid,
    pub this: Objid,
    pub player: Objid,
    pub args: Vec<Var>,
    pub caller: Objid,
}
