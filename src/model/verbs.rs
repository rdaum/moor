use crate::model::ObjectError;
use enum_primitive_derive::Primitive;
use rkyv::{Archive, Deserialize, Serialize};

use crate::model::r#match::{ArgSpec, PrepSpec, VerbArgsSpec};
use crate::model::var::Objid;
use crate::util::bitenum::BitEnum;
use crate::vm::opcode::Binary;

#[derive(
    Debug,
    Serialize,
    Deserialize,
    Archive,
    Ord,
    PartialOrd,
    Copy,
    Clone,
    Eq,
    PartialEq,
    Hash,
    Primitive,
)]
pub enum VerbFlag {
    Read = 0,
    Write = 1,
    Exec = 2,
    Debug = 3,
}

#[derive(
    Serialize, Deserialize, Archive, Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Hash,
)]
pub struct Vid(pub i64);

#[derive(Clone, Copy, Serialize, Deserialize, Archive, Debug, Primitive)]
pub enum VerbAttr {
    Definer = 0,
    Owner = 1,
    Flags = 2,
    ArgsSpec = 3,
    Program = 4,
}

#[derive(Serialize, Deserialize, Archive, Clone, Debug)]
pub struct VerbAttrs {
    pub definer: Option<Objid>,
    pub owner: Option<Objid>,
    pub flags: Option<BitEnum<VerbFlag>>,
    pub args_spec: Option<VerbArgsSpec>,
    pub program: Option<Binary>,
}

#[derive(Serialize, Deserialize, Archive, Clone, Debug)]
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
        flags: BitEnum<VerbFlag>,
        arg_spec: VerbArgsSpec,
        program: Binary,
    ) -> Result<VerbInfo, ObjectError>;

    /// Get all verbs attached to the given object.
    fn get_verbs(
        &mut self,
        oid: Objid,
        attrs: BitEnum<VerbAttr>,
    ) -> Result<Vec<VerbInfo>, ObjectError>;

    fn get_verb(&mut self, vid: Vid, attrs: BitEnum<VerbAttr>) -> Result<VerbInfo, ObjectError>;

    fn update_verb(&mut self, vid: Vid, attrs: VerbAttrs) -> Result<(), ObjectError>;

    /// Match verbs using prepositional pieces.
    fn find_command_verb(
        &mut self,
        obj: Objid,
        verb: &str,
        dobj: ArgSpec,
        prep: PrepSpec,
        iobj: ArgSpec,
    ) -> Result<Option<VerbInfo>, ObjectError>;

    /// Find the verbs that match based on the provided name-stem.
    fn find_callable_verb(
        &mut self,
        oid: Objid,
        verb: &str,
        attrs: BitEnum<VerbAttr>,
    ) -> Result<Option<VerbInfo>, ObjectError>;

    /// Find the verb that is the Nth verb in insertion order for the object.
    fn find_indexed_verb(
        &mut self,
        oid: Objid,
        index: usize,
        attrs: BitEnum<VerbAttr>,
    ) -> Result<Option<VerbInfo>, ObjectError>;
}
