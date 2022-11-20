use crate::model::var::{Objid, Var};
use enumset::EnumSet;
use enumset_derive::EnumSetType;

#[derive(EnumSetType, Debug)]
#[enumset(serialize_repr = "u8")]
pub enum PropFlag {
    Read,
    Write,
    Chown,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub struct Pid(pub i64);

pub struct Propdef {
    pub pid: Pid,
    pub definer: Objid,
    pub pname: String,
}

/// Property definitions are the definition of a given property by the property original owner
/// creator.
/// Property values (see below) can be overriden in children, but the definition remains.

pub trait PropDefs {
    fn get_propdef(&mut self, definer: Objid, pname: &str) -> Result<Propdef, anyhow::Error>;
    fn add_propdef(
        &mut self,
        definer: Objid,
        name: &str,
        owner: Objid,
        flags: EnumSet<PropFlag>,
        initial_value: Option<Var>,
    ) -> Result<Pid, anyhow::Error>;
    fn rename_propdef(&mut self, definer: Objid, old: &str, new: &str)
        -> Result<(), anyhow::Error>;
    fn delete_propdef(&mut self, definer: Objid, pname: &str) -> Result<(), anyhow::Error>;
    fn count_propdefs(&mut self, definer: Objid) -> Result<usize, anyhow::Error>;
    fn get_propdefs(&mut self, definer: Objid) -> Result<Vec<Propdef>, anyhow::Error>;
}

#[derive(EnumSetType, Debug)]
#[enumset(serialize_repr = "u8")]
pub enum PropAttr {
    Value,
    Location,
    Owner,
    Flags,
}

#[derive(Debug, Clone)]
pub struct PropAttrs {
    pub value: Option<Var>,
    pub location: Option<Objid>,
    pub owner: Option<Objid>,
    pub flags: Option<EnumSet<PropFlag>>,
}

#[derive(Clone)]
pub struct PropertyInfo {
    pub pid: Pid,
    pub attrs: PropAttrs,
}

pub trait Properties {
    fn find_property(
        &self,
        oid: Objid,
        name: &str,
        attrs: EnumSet<PropAttr>,
    ) -> Result<Option<PropertyInfo>, anyhow::Error>;
    fn get_property(
        &self,
        oid: Objid,
        handle: Pid,
        attrs: EnumSet<PropAttr>,
    ) -> Result<Option<PropAttrs>, anyhow::Error>;
    fn set_property(
        &self,
        handle: Pid,
        location: Objid,
        value: Var,
        owner: Objid,
        flags: EnumSet<PropFlag>,
    ) -> Result<(), anyhow::Error>;
}
