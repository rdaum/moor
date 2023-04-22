use enum_primitive_derive::Primitive;

use crate::model::ObjectError;
use crate::model::var::{Objid, Var};
use crate::util::bitenum::BitEnum;

#[derive(
    Debug,
    Clone,
    Copy,
    Eq,
    PartialEq,
    Hash,
    Ord,
    PartialOrd,
    Primitive,
)]
pub enum PropFlag {
    Read = 0,
    Write = 1,
    Chown = 2,
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Hash,
)]
pub struct Pid(pub i64);

#[derive(Debug, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
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
    fn get_propdef(&mut self, definer: Objid, pname: &str) -> Result<Propdef, ObjectError>;

    // Add a property definition.
    fn add_propdef(
        &mut self,
        definer: Objid,
        name: &str,
        owner: Objid,
        flags: BitEnum<PropFlag>,
        initial_value: Option<Var>,
    ) -> Result<Pid, ObjectError>;

    // Rename a property.
    fn rename_propdef(&mut self, definer: Objid, old: &str, new: &str) -> Result<(), ObjectError>;

    // Delete a property definition.
    fn delete_propdef(&mut self, definer: Objid, pname: &str) -> Result<(), ObjectError>;

    // Count the number of property definitions on an object.
    fn count_propdefs(&mut self, definer: Objid) -> Result<usize, ObjectError>;

    // Get all property definitions on an object.
    fn get_propdefs(&mut self, definer: Objid) -> Result<Vec<Propdef>, ObjectError>;
}

#[derive(Debug, Clone, Copy, Primitive)]
pub enum PropAttr {
    Value = 0,
    Location = 1,
    Owner = 2,
    Flags = 3,
}

#[derive(Clone, Debug)]
pub struct PropAttrs {
    pub value: Option<Var>,
    pub location: Option<Objid>,
    pub owner: Option<Objid>,
    pub flags: Option<BitEnum<PropFlag>>,
}

impl PropAttrs {
    pub fn new() -> Self {
        Self {
            value: None,
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

#[derive(Clone, Debug)]
pub struct PropertyInfo {
    pub pid: Pid,
    pub attrs: PropAttrs,
}

pub trait Properties {
    // Find a property by name, starting from the given object and going up the inheritance tree.
    fn find_property(
        &mut self,
        oid: Objid,
        name: &str,
        attrs: BitEnum<PropAttr>,
    ) -> Result<Option<PropertyInfo>, ObjectError>;

    // Get a property by its unique pid from its property definition, seeking the inheritance
    // hierarchy.
    fn get_property(
        &mut self,
        oid: Objid,
        handle: Pid,
        attrs: BitEnum<PropAttr>,
    ) -> Result<Option<PropAttrs>, ObjectError>;

    // Set a property using its unique pid from its property definition.
    fn set_property(
        &mut self,
        handle: Pid,
        location: Objid,
        value: Var,
        owner: Objid,
        flags: BitEnum<PropFlag>,
    ) -> Result<(), ObjectError>;
}
