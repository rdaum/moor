use crate::model::var::{Objid};
use enumset::EnumSet;
use enumset_derive::EnumSetType;


#[derive(EnumSetType, Debug)]
#[enumset(serialize_repr = "u8")]
pub enum ObjFlag {
    User,
    Programmer,
    Wizard,
    Read,
    Write,
    Fertile,
}

// The set of built-in object attributes
#[derive(EnumSetType, Debug)]
#[enumset(serialize_repr = "u8")]
pub enum ObjAttr {
    Owner,
    Name,
    Parent,
    Location,
    Flags,
}

#[derive(Debug)]
pub struct ObjAttrs {
    pub owner: Option<Objid>,
    pub name: Option<String>,
    pub parent: Option<Objid>,
    pub location: Option<Objid>,
    pub flags: Option<EnumSet<ObjAttr>>,
}

pub trait Objects {
    fn create_object(&mut self) -> Result<Objid, anyhow::Error>;
    fn destroy_object(&mut self, oid: Objid) -> Result<(), anyhow::Error>;
    fn object_valid(&self, oid: Objid) -> Result<bool, anyhow::Error>;

    fn object_get_attrs(
        &mut self,
        oid: Objid,
        attributes: EnumSet<ObjAttr>,
    ) -> Result<ObjAttrs, anyhow::Error>;
    fn object_set_attrs(
        &mut self,
        oid: Objid,
        attributes: ObjAttrs,
    ) -> Result<(), anyhow::Error>;

    fn count_object_children(&self, oid: Objid) -> Result<usize, anyhow::Error>;
    fn object_children(&self, oid: Objid) -> Result<Vec<Objid>, anyhow::Error>;

    fn count_object_contents(&self, oid: Objid) -> Result<usize, anyhow::Error>;
    fn object_contents(&self, oid: Objid) -> Result<Vec<Objid>, anyhow::Error>;
}

trait Player {
    fn is_object_wizard(&self, oid: Objid) -> Result<bool, anyhow::Error>;
    fn is_object_programmer(&self, oid: Objid) -> Result<bool, anyhow::Error>;
    fn is_object_player(&self, oid: Objid) -> Result<bool, anyhow::Error>;
    fn all_users(&self) -> Result<Vec<Objid>, anyhow::Error>;
}
