use std::fmt::{Display, Formatter};

use bincode::{Decode, Encode};
use enum_primitive_derive::Primitive;

use crate::util::bitenum::BitEnum;
use crate::var::objid::Objid;

#[derive(Debug, Ord, PartialOrd, Copy, Clone, Eq, PartialEq, Hash, Primitive, Encode, Decode)]
pub enum ObjFlag {
    User = 0,
    Programmer = 1,
    Wizard = 2,
    Obsolete1 = 3,
    Read = 4,
    Write = 5,
    Obsolete2 = 6,
    Fertile = 8,
}

// The set of built-in object attributes
#[derive(Clone, Copy, Eq, PartialEq, Debug, Hash, Primitive, Decode, Encode)]
pub enum ObjAttr {
    Owner = 0,
    Name = 1,
    Parent = 2,
    Location = 3,
    Flags = 4,
}
impl Display for ObjAttr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Owner => f.write_str("owner"),
            Self::Name => f.write_str("name"),
            Self::Parent => f.write_str("parent"),
            Self::Location => f.write_str("location"),
            Self::Flags => f.write_str("flags"),
        }
    }
}

impl Default for ObjAttrs {
    fn default() -> Self {
        Self::new()
    }
}

impl ObjAttrs {
    #[must_use]
    pub fn new() -> Self {
        Self {
            owner: None,
            name: None,
            parent: None,
            location: None,
            flags: None,
        }
    }
    pub fn owner(&mut self, o: Objid) -> &mut Self {
        self.owner = Some(o);
        self
    }
    pub fn location(&mut self, o: Objid) -> &mut Self {
        self.location = Some(o);
        self
    }
    pub fn parent(&mut self, o: Objid) -> &mut Self {
        self.parent = Some(o);
        self
    }
    pub fn name(&mut self, s: &str) -> &mut Self {
        self.name = Some(String::from(s));
        self
    }
    pub fn flags(&mut self, flags: BitEnum<ObjFlag>) -> &mut Self {
        self.flags = Some(flags);
        self
    }
}

#[derive(Debug, Clone, Encode, Decode)]
pub struct ObjAttrs {
    pub owner: Option<Objid>,
    pub name: Option<String>,
    pub parent: Option<Objid>,
    pub location: Option<Objid>,
    pub flags: Option<BitEnum<ObjFlag>>,
}
