use crate::model::ObjectError;
use enum_primitive_derive::Primitive;
use rkyv::{Archive, Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::model::var::Objid;
use crate::util::bitenum::BitEnum;

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
#[derive(Debug, Hash, Serialize, Deserialize, Archive, Primitive)]
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
            ObjAttr::Owner => f.write_str("owner"),
            ObjAttr::Name => f.write_str("name"),
            ObjAttr::Parent => f.write_str("parent"),
            ObjAttr::Location => f.write_str("location"),
            ObjAttr::Flags => f.write_str("flags"),
        }
    }
}

impl Default for ObjAttrs {
    fn default() -> Self {
        Self::new()
    }
}

impl ObjAttrs {
    pub fn new() -> Self {
        Self {
            owner: None,
            name: None,
            parent: None,
            location: None,
            flags: None,
        }
    }
    pub fn owner(&mut self, o: Objid) -> &mut ObjAttrs {
        self.owner = Some(o);
        self
    }
    pub fn location(&mut self, o: Objid) -> &mut ObjAttrs {
        self.location = Some(o);
        self
    }
    pub fn parent(&mut self, o: Objid) -> &mut ObjAttrs {
        self.parent = Some(o);
        self
    }
    pub fn name(&mut self, s: &str) -> &mut ObjAttrs {
        self.name = Some(String::from(s));
        self
    }
    pub fn flags(&mut self, flags: BitEnum<ObjFlag>) -> &mut ObjAttrs {
        self.flags = Some(flags);
        self
    }
}

#[derive(Serialize, Deserialize, Archive, Debug, Clone)]
pub struct ObjAttrs {
    pub owner: Option<Objid>,
    pub name: Option<String>,
    pub parent: Option<Objid>,
    pub location: Option<Objid>,
    pub flags: Option<BitEnum<ObjFlag>>,
}

pub trait Objects {
    fn create_object(&mut self, oid: Option<Objid>, attrs: &ObjAttrs)
        -> Result<Objid, ObjectError>;
    fn destroy_object(&mut self, oid: Objid) -> Result<(), ObjectError>;
    fn object_valid(&mut self, oid: Objid) -> Result<bool, ObjectError>;

    fn object_get_attrs(
        &mut self,
        oid: Objid,
        attributes: BitEnum<ObjAttr>,
    ) -> Result<ObjAttrs, ObjectError>;
    fn object_set_attrs(&mut self, oid: Objid, attributes: ObjAttrs) -> Result<(), ObjectError>;

    fn object_children(&mut self, oid: Objid) -> Result<Vec<Objid>, ObjectError>;
    fn object_contents(&mut self, oid: Objid) -> Result<Vec<Objid>, ObjectError>;
}

trait Player {
    fn is_object_wizard(&self, oid: Objid) -> Result<bool, anyhow::Error>;
    fn is_object_programmer(&self, oid: Objid) -> Result<bool, anyhow::Error>;
    fn is_object_player(&self, oid: Objid) -> Result<bool, anyhow::Error>;
    fn all_users(&self) -> Result<Vec<Objid>, anyhow::Error>;
}
