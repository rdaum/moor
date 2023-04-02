use enumset::EnumSet;
use enumset_derive::EnumSetType;

use crate::model::var::Objid;

#[derive(EnumSetType, Debug)]
#[enumset(serialize_repr = "u8")]
pub enum ObjFlag {
    User,
    Programmer,
    Wizard,
    Obsolete1,
    Read,
    Write,
    Obsolete2,
    Fertile,
}

// The set of built-in object attributes
#[derive(EnumSetType, Debug, Hash)]
#[enumset(serialize_repr = "u8")]
pub enum ObjAttr {
    Owner,
    Name,
    Parent,
    Location,
    Flags,
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
    pub fn flags(&mut self, flags: EnumSet<ObjFlag>) -> &mut ObjAttrs {
        self.flags = Some(flags);
        self
    }
}

#[derive(Debug, Clone)]
pub struct ObjAttrs {
    pub owner: Option<Objid>,
    pub name: Option<String>,
    pub parent: Option<Objid>,
    pub location: Option<Objid>,
    pub flags: Option<EnumSet<ObjFlag>>,
}

pub trait Objects {
    fn create_object(
        &mut self,
        oid: Option<Objid>,
        attrs: &ObjAttrs,
    ) -> Result<Objid, anyhow::Error>;
    fn destroy_object(&mut self, oid: Objid) -> Result<(), anyhow::Error>;
    fn object_valid(&self, oid: Objid) -> Result<bool, anyhow::Error>;

    fn object_get_attrs(
        &mut self,
        oid: Objid,
        attributes: EnumSet<ObjAttr>,
    ) -> Result<ObjAttrs, anyhow::Error>;
    fn object_set_attrs(&mut self, oid: Objid, attributes: ObjAttrs) -> Result<(), anyhow::Error>;

    fn object_children(&self, oid: Objid) -> Result<Vec<Objid>, anyhow::Error>;
    fn object_contents(&self, oid: Objid) -> Result<Vec<Objid>, anyhow::Error>;
}

trait Player {
    fn is_object_wizard(&self, oid: Objid) -> Result<bool, anyhow::Error>;
    fn is_object_programmer(&self, oid: Objid) -> Result<bool, anyhow::Error>;
    fn is_object_player(&self, oid: Objid) -> Result<bool, anyhow::Error>;
    fn all_users(&self) -> Result<Vec<Objid>, anyhow::Error>;
}
