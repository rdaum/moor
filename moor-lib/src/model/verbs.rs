use bincode::{Decode, Encode};
use enum_primitive_derive::Primitive;

use crate::model::r#match::VerbArgsSpec;
use crate::util::bitenum::BitEnum;
use crate::var::Objid;
use crate::vm::opcode::Binary;

#[derive(Debug, Ord, PartialOrd, Copy, Clone, Eq, PartialEq, Hash, Primitive, Encode, Decode)]
pub enum VerbFlag {
    Read = 0,
    Write = 1,
    Exec = 2,
    Debug = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Hash, Encode, Decode)]
pub struct Vid(pub i64);

#[derive(Clone, Copy, Debug, Primitive)]
pub enum VerbAttr {
    Definer = 0,
    Owner = 1,
    Flags = 2,
    ArgsSpec = 3,
    Program = 4,
}

#[derive(Clone, Debug, Encode, Decode)]
pub struct VerbAttrs {
    pub definer: Option<Objid>,
    pub owner: Option<Objid>,
    pub flags: Option<BitEnum<VerbFlag>>,
    pub args_spec: Option<VerbArgsSpec>,
    pub program: Option<Binary>,
}

#[derive(Clone, Debug, Encode, Decode)]
pub struct VerbInfo {
    pub names: Vec<String>,
    pub attrs: VerbAttrs,
}
