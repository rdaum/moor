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
        oid: Objid,
        names: &str,
        owner: Objid,
        flags: Vec<VerbFlag>,
        argspec: VerbArgsSpec,
    ) -> Result<(), anyhow::Error>;

    fn count_verbs(oid: Objid) -> Result<usize, anyhow::Error>;

    fn get_all_verbs(oid: Objid) -> Result<Vec<String>, anyhow::Error>;

    fn find_command_verb(
        oid: Objid,
        verb: &str,
        argspec: VerbArgsSpec
    ) -> Result<VerbHandle, anyhow::Error>;

    fn find_callable_verb(
        oid: Objid,
        verb: &str
    ) -> Result<VerbHandle, anyhow::Error>;

    fn find_defined_verb(oid: Objid, verb: &str,
                         allow_numbers: bool) -> Result<VerbHandle, anyhow::Error>;

    fn find_indexed_verb(oid: Objid, index: usize) -> Result<VerbHandle, anyhow::Error>;
}

/// TODO: contains the actual compiled verb for execution by VM
pub struct Program {}

/// Traits for using a given verb by its handle.
pub trait Verb {
    fn delete_verb(handle:VerbHandle) -> Result<(), anyhow::Error>;
    fn verb_program(handle:VerbHandle) -> Result<Program, anyhow::Error>;
    fn set_verb_program(handle:VerbHandle, prg: Program) -> Result<(), anyhow::Error>;

    fn verb_definer(handle : VerbHandle) -> Result<Objid, anyhow::Error>;
    fn verb_names(handle: VerbHandle) -> Result<String, anyhow::Error>;
    fn set_verb_names(handle: VerbHandle, names: String) -> Result<(), anyhow::Error>;

    fn verb_owner(handle:VerbHandle) -> Result<Objid, anyhow::Error>;
    fn set_verb_owner(handle:VerbHandle) -> Result<Objid, anyhow::Error>;

    fn verb_flags(handle:VerbHandle) -> Result<Vec<VerbFlag>, anyhow::Error>;
    fn set_verb_flags(handle:VerbHandle, flags: Vec<VerbFlag>) -> Result<(), anyhow::Error>;

    fn verb_arg_spcs(handle:VerbHandle) -> Result<VerbArgsSpec, anyhow::Error>;
    fn set_verb_arg_psecs(handle:VerbHandle, spec:VerbArgsSpec) -> Result<(), anyhow::Error>;

    fn verb_allows(handle:VerbHandle, oid: Objid, flags: VerbFlag) -> Result<bool, anyhow::Error>;
}

