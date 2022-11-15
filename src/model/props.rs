use crate::model::var::{Objid, Var};

pub struct Propdef {
    oid: Objid,
    pname: String,
    owner: Objid,
    flags: u16,
    val: Var,
}

pub trait PropDefs {
    fn add_propdef(propdef: Propdef);
    fn rename_propdef(oid: Objid, old: &str, new: &str);
    fn delete_propdef(oid: Objid, pname: &str);
    fn count_propdefs(oid: Objid);
    fn get_propdefs(oid: Objid) -> Vec<Propdef>;
}

pub enum PropFlag {
    Read,
    Write,
    Chown
}
pub struct PropHandle(usize);
pub trait Properties {
    fn find_property(oid: Objid, pname: &str) -> Option<PropHandle>;

    fn property_value(handle: PropHandle) -> Result<Var, anyhow::Error>;
    fn set_property_value(handle: PropHandle, value: Var) -> Result<(), anyhow::Error>;

    fn property_owner(handle: PropHandle) -> Result<Objid, anyhow::Error>;
    fn set_property_owner(handle: PropHandle, owner: Objid) -> Result<(), anyhow::Error>;

    fn property_flags(handle: PropHandle) -> Result<Vec<PropFlag>, anyhow::Error>;
    fn set_property_flags(handle: PropHandle, flags: Vec<PropFlag>) -> Result<(), anyhow::Error>;

    fn property_allows(handle: PropHandle, flags: u16) -> Result<bool, anyhow::Error>;
}

