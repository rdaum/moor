use bincode::{Decode, Encode};
use enum_primitive_derive::Primitive;

use crate::util::bitenum::BitEnum;
use crate::var::objid::Objid;

use crate::model::r#match::VerbArgsSpec;

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
    Binary = 4,
}

/// The program type encoded for a verb.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Encode, Decode, Primitive)]
pub enum BinaryType {
    /// For builtin functions in stack frames -- or empty code blobs.
    None = 0,
    /// Opcodes match almost 1:1 with LambdaMOO 1.8.x, but is not "binary" compatible.
    LambdaMoo18X = 1,
}

#[derive(Clone, Debug, Eq, PartialEq, Encode, Decode)]
pub struct VerbAttrs {
    pub definer: Option<Objid>,
    pub owner: Option<Objid>,
    pub flags: Option<BitEnum<VerbFlag>>,
    pub args_spec: Option<VerbArgsSpec>,
    pub binary_type: BinaryType,
    pub binary: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Encode, Decode)]
pub struct VerbInfo {
    pub names: Vec<String>,
    pub attrs: VerbAttrs,
}
