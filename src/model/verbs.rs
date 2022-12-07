use enumset::EnumSet;
use enumset_derive::EnumSetType;

use crate::model::r#match::VerbArgsSpec;
use crate::model::var::Objid;

#[derive(EnumSetType, Debug)]
#[enumset(serialize_repr = "u16")]
pub enum VerbFlag {
    Read,
    Write,
    Exec,
    Debug,
}

#[derive(Clone)]
pub struct Vid(pub i64);

#[derive(Clone)]
pub struct Program(pub bytes::Bytes);

#[derive(EnumSetType, Debug)]
pub enum VerbAttr {
    Definer,
    Owner,
    Flags,
    ArgsSpec,
    Program,
}

#[derive(Clone)]
pub struct VerbAttrs {
    pub definer: Option<Objid>,
    pub owner: Option<Objid>,
    pub flags: Option<EnumSet<VerbFlag>>,
    pub args_spec: Option<VerbArgsSpec>,
    pub program: Option<Program>,
}

#[derive(Clone)]
pub struct VerbInfo {
    pub vid: Vid,
    pub names: Vec<String>,
    pub attrs: VerbAttrs,
}

/// Trait for the management of verbs; creating finding counting
pub trait Verbs {
    fn add_verb(
        &mut self,
        oid: Objid,
        names: Vec<&str>,
        owner: Objid,
        flags: EnumSet<VerbFlag>,
        arg_spec: VerbArgsSpec,
        program: Program,
    ) -> Result<VerbInfo, anyhow::Error>;

    /// Get all verbs attached to the given object.
    fn get_verbs(
        &self,
        oid: Objid,
        attrs: EnumSet<VerbAttr>,
    ) -> Result<Vec<VerbInfo>, anyhow::Error>;

    fn get_verb(&self, vid: Vid, attrs: EnumSet<VerbAttr>) -> Result<VerbInfo, anyhow::Error>;

    fn update_verb(&self, vid: Vid, attrs: VerbAttrs) -> Result<(), anyhow::Error>;

    /// Match verbs using prepositional pieces.
    fn find_command_verb(
        &self,
        oid: Objid,
        verb: &str,
        arg_spec: VerbArgsSpec,
        attrs: EnumSet<VerbAttr>,
    ) -> Result<Option<VerbInfo>, anyhow::Error>;

    /// Find the verbs that match based on the provided name-stem.
    fn find_callable_verb(
        &self,
        oid: Objid,
        verb: &str,
        attrs: EnumSet<VerbAttr>,
    ) -> Result<Option<VerbInfo>, anyhow::Error>;

    /// Find the verb that is the Nth verb in insertion order for the object.
    fn find_indexed_verb(
        &self,
        oid: Objid,
        index: usize,
        attrs: EnumSet<VerbAttr>,
    ) -> Result<Option<VerbInfo>, anyhow::Error>;
}
