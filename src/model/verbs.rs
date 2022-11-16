use crate::model::r#match::VerbArgsSpec;
use crate::model::var::Objid;

pub enum VerbFlag {
    Read,
    Write,
    Exec,
    Debug,
}

pub struct VerbHandle(usize);

/// Trait for the management of verbs; creating finding counting
pub trait Verbs {
    fn add_verb(
        &mut self,
        oid: Objid,
        names: &str,
        owner: Objid,
        flags: Vec<VerbFlag>,
        argspec: VerbArgsSpec,
    ) -> Result<(), anyhow::Error>;

    fn count_verbs(&self, oid: Objid) -> Result<usize, anyhow::Error>;

    fn get_all_verbs(&self, oid: Objid) -> Result<Vec<String>, anyhow::Error>;

    fn find_command_verb(
        &self,
        oid: Objid,
        verb: &str,
        argspec: VerbArgsSpec,
    ) -> Result<VerbHandle, anyhow::Error>;

    fn find_callable_verb(&self, oid: Objid, verb: &str) -> Result<VerbHandle, anyhow::Error>;

    fn find_defined_verb(
        &self,
        oid: Objid,
        verb: &str,
        allow_numbers: bool,
    ) -> Result<VerbHandle, anyhow::Error>;

    fn find_indexed_verb(&self, oid: Objid, index: usize) -> Result<VerbHandle, anyhow::Error>;
}

/// TODO: contains the actual compiled verb for execution by VM
pub struct Program {}

/// Traits for using a given verb by its handle.
pub trait Verb {
    fn delete_verb(&self, handle: VerbHandle) -> Result<(), anyhow::Error>;
    fn verb_program(&self, handle: VerbHandle) -> Result<Program, anyhow::Error>;
    fn set_verb_program(&mut self, handle: VerbHandle, prg: Program) -> Result<(), anyhow::Error>;

    fn verb_definer(&self, handle: VerbHandle) -> Result<Objid, anyhow::Error>;
    fn verb_names(&self, handle: VerbHandle) -> Result<String, anyhow::Error>;
    fn set_verb_names(&mut self, handle: VerbHandle, names: String) -> Result<(), anyhow::Error>;

    fn verb_owner(&self, handle: VerbHandle) -> Result<Objid, anyhow::Error>;
    fn set_verb_owner(&mut self, handle: VerbHandle) -> Result<Objid, anyhow::Error>;

    fn verb_flags(&self, handle: VerbHandle) -> Result<Vec<VerbFlag>, anyhow::Error>;
    fn set_verb_flags(
        &mut self,
        handle: VerbHandle,
        flags: Vec<VerbFlag>,
    ) -> Result<(), anyhow::Error>;

    fn verb_arg_spcs(&self, handle: VerbHandle) -> Result<VerbArgsSpec, anyhow::Error>;
    fn set_verb_arg_psecs(
        &mut self,
        handle: VerbHandle,
        spec: VerbArgsSpec,
    ) -> Result<(), anyhow::Error>;

    fn verb_allows(
        &self,
        handle: VerbHandle,
        oid: Objid,
        flags: VerbFlag,
    ) -> Result<bool, anyhow::Error>;
}
