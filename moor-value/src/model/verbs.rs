use bincode::{Decode, Encode};
use enum_primitive_derive::Primitive;
use uuid::Uuid;

use crate::model::r#match::VerbArgsSpec;
use crate::model::{Defs, HasUuid, Named};
use crate::util::bitenum::BitEnum;
use crate::util::verbname_cmp;
use crate::var::objid::Objid;

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
    pub names: Option<Vec<String>>,
    pub flags: Option<BitEnum<VerbFlag>>,
    pub args_spec: Option<VerbArgsSpec>,
    pub binary_type: Option<BinaryType>,
    pub binary: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Encode, Decode)]
pub struct VerbInfo {
    pub verbdef: VerbDef,
    pub binary: Vec<u8>,
}

#[derive(Debug, Encode, Decode, Clone, Eq, PartialEq)]
pub struct VerbDef {
    pub uuid: [u8; 16],
    pub location: Objid,
    pub owner: Objid,
    pub names: Vec<String>,
    pub flags: BitEnum<VerbFlag>,
    pub binary_type: BinaryType,
    pub args: VerbArgsSpec,
}

impl Named for VerbDef {
    fn matches_name(&self, name: &str) -> bool {
        self.names
            .iter()
            .any(|verb| verbname_cmp(verb.to_lowercase().as_str(), name.to_lowercase().as_str()))
    }
}

impl HasUuid for VerbDef {
    fn uuid(&self) -> Uuid {
        Uuid::from_bytes(self.uuid)
    }
}

pub type VerbDefs = Defs<VerbDef>;
