use enumset::EnumSet;
use enumset_derive::EnumSetType;

use crate::model::var::{Objid, Var};

#[derive(EnumSetType, Debug)]
#[enumset(serialize_repr = "u8")]
pub enum PropFlag {
    Read,
    Write,
    Chown,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct Pid(pub i64);

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Propdef {
    pub pid: Pid,
    pub definer: Objid,
    pub pname: String,
}

/// Property definitions are the definition of a given property by the property original owner
/// creator.
/// Property values (see below) can be overriden in children, but the definition remains.

pub trait PropDefs {
    // Get a property definition by its name.
    fn get_propdef(&mut self, definer: Objid, pname: &str) -> Result<Propdef, anyhow::Error>;

    // Add a property definition.
    fn add_propdef(
        &mut self,
        definer: Objid,
        name: &str,
        owner: Objid,
        flags: EnumSet<PropFlag>,
        initial_value: Option<Var>,
    ) -> Result<Pid, anyhow::Error>;

    // Rename a property.
    fn rename_propdef(&mut self, definer: Objid, old: &str, new: &str)
        -> Result<(), anyhow::Error>;

    // Delete a property definition.
    fn delete_propdef(&mut self, definer: Objid, pname: &str) -> Result<(), anyhow::Error>;

    // Count the number of property definitions on an object.
    fn count_propdefs(&mut self, definer: Objid) -> Result<usize, anyhow::Error>;

    // Get all property definitions on an object.
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

impl PropAttrs {
    pub fn new() -> Self {
        Self { value: None, location: None, owner: None, flags: None }
    }
}

impl Default for PropAttrs {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct PropertyInfo {
    pub pid: Pid,
    pub attrs: PropAttrs,
}

pub trait Properties {

    // Find a property by name, starting from the given object and going up the inheritance tree.
    fn find_property(
        &self,
        oid: Objid,
        name: &str,
        attrs: EnumSet<PropAttr>,
    ) -> Result<Option<PropertyInfo>, anyhow::Error>;

    // Get a property by its unique pid from its property definition, seeking the inheritance
    // hierarchy.
    fn get_property(
        &self,
        oid: Objid,
        handle: Pid,
        attrs: EnumSet<PropAttr>,
    ) -> Result<Option<PropAttrs>, anyhow::Error>;

    // Set a property using its unique pid from its property definition.
    fn set_property(
        &mut self,
        handle: Pid,
        location: Objid,
        value: Var,
        owner: Objid,
        flags: EnumSet<PropFlag>,
    ) -> Result<(), anyhow::Error>;
}
