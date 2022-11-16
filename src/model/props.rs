use bincode::{Decode, Encode};
use crate::model::var::{Objid, Var};

#[derive(Encode, Decode)]
pub enum PropFlag {
    Read,
    Write,
    Chown,
}
pub struct Propdef {
    pub oid: Objid,
    pub pname: String,
    pub owner: Objid,
    pub flags: Vec<PropFlag>,
    pub val: Var,
}

pub trait PropDefs {
    fn add_propdef(&mut self, propdef: Propdef) -> Result<(), anyhow::Error>;
    fn rename_propdef(&mut self, oid: Objid, old: &str, new: &str) -> Result<(), anyhow::Error>;
    fn delete_propdef(&mut self, oid: Objid, pname: &str) -> Result<(), anyhow::Error>;
    fn count_propdefs(&mut self, oid: Objid) -> Result<usize, anyhow::Error>;
    fn get_propdefs(&mut self, oid: Objid) -> Result<Vec<Propdef>, anyhow::Error>;
}


pub struct PropHandle(usize);
pub trait Properties {
    fn find_property(&self, oid: Objid, pname: &str) -> Option<PropHandle>;

    fn property_value(&self, handle: PropHandle) -> Result<Var, anyhow::Error>;
    fn set_property_value(&mut self, handle: PropHandle, value: Var) -> Result<(), anyhow::Error>;

    fn property_owner(&self, handle: PropHandle) -> Result<Objid, anyhow::Error>;
    fn set_property_owner(&mut self, handle: PropHandle, owner: Objid) -> Result<(), anyhow::Error>;

    fn property_flags(&self, handle: PropHandle) -> Result<Vec<PropFlag>, anyhow::Error>;
    fn set_property_flags(&mut self, handle: PropHandle, flags: Vec<PropFlag>) -> Result<(), anyhow::Error>;

    fn property_allows(&self, handle: PropHandle, flags: u16) -> Result<bool, anyhow::Error>;
}

