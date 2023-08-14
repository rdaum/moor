use bincode::{Decode, Encode};
use enum_primitive_derive::Primitive;
use uuid::Uuid;

use crate::model::{Defs, HasUuid, Named};
use crate::util::bitenum::BitEnum;
use crate::var::objid::Objid;
use crate::var::Var;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Ord, PartialOrd, Primitive, Encode, Decode)]
pub enum PropFlag {
    Read = 0,
    Write = 1,
    Chown = 2,
}

/// Property definitions are the definition of a given property by the property original owner
/// creator.
/// Property values (see below) can be overriden in children, but the definition remains.

#[derive(Debug, Clone, Copy, Primitive)]
pub enum PropAttr {
    Value = 0,
    Location = 1,
    Owner = 2,
    Flags = 3,
    Clear = 4,
}

#[derive(Clone, Debug)]
pub struct PropAttrs {
    pub name: Option<String>,
    pub value: Option<Var>,
    pub location: Option<Objid>,
    pub owner: Option<Objid>,
    pub flags: Option<BitEnum<PropFlag>>,
}

impl PropAttrs {
    pub fn new() -> Self {
        Self {
            value: None,
            name: None,
            location: None,
            owner: None,
            flags: None,
        }
    }
}

impl Default for PropAttrs {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Encode, Decode, Clone)]
pub struct PropDef {
    pub uuid: [u8; 16],
    pub definer: Objid,
    pub location: Objid,
    pub name: String,
    pub flags: BitEnum<PropFlag>,
    pub owner: Objid,
}

impl Named for PropDef {
    fn matches_name(&self, name: &str) -> bool {
        self.name.to_lowercase().as_str() == name.to_lowercase().as_str()
    }
}

impl HasUuid for PropDef {
    fn uuid(&self) -> Uuid {
        Uuid::from_bytes(self.uuid)
    }
}

pub type PropDefs = Defs<PropDef>;
