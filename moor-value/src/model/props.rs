use crate::util::bitenum::BitEnum;
use crate::var::objid::Objid;
use crate::var::Var;
use bincode::{Decode, Encode};
use enum_primitive_derive::Primitive;

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
