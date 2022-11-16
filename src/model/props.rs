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
    fn find_propdef(
        &mut self,
        definer: Objid,
        pname: &str,
    ) -> Result<Option<Propdef>, anyhow::Error>;
    fn add_propdef(
        &mut self,
        definer: Objid,
        name: &str,
        owner: Objid,
        flags: EnumSet<PropFlag>,
        initial_value: Var,
    ) -> Result<Pid, anyhow::Error>;
    fn rename_propdef(&mut self, definer: Objid, old: &str, new: &str) -> Result<(), anyhow::Error>;
    fn delete_propdef(&mut self, definer: Objid, pname: &str) -> Result<(), anyhow::Error>;
    fn count_propdefs(&mut self, definer: Objid) -> Result<usize, anyhow::Error>;
    fn get_propdefs(&mut self, definer: Objid) -> Result<Vec<Propdef>, anyhow::Error>;
}

#[derive(EnumSetType, Debug)]
#[enumset(serialize_repr = "u8")]
pub enum PropAttr {
    Value,
    Owner,
    Flags,
}

#[derive(Debug)]
pub struct PropAttrs {
    pub value: Option<Var>,
    pub owner: Option<Objid>,
    pub flags: Option<EnumSet<PropFlag>>,
}

pub trait Properties {
    fn get_property(
        &self,
        handle: Pid,
        attrs: EnumSet<PropAttr>,
    ) -> Result<PropAttrs, anyhow::Error>;
    fn set_property(
        &self,
        handle: Pid,
        value: Var,
        owner: Objid,
        flags: EnumSet<PropFlag>,
    ) -> Result<(), anyhow::Error>;
}
