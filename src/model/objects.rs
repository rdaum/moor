use crate::model::var::Objid;


pub enum ObjFlag {
    User,
    Programmer,
    Wizard,
    Read,
    Write,
    Fertile
}

trait Objects {
    fn create_object() -> Result<Objid, anyhow::Error>;
    fn destroy_object(oid: Objid) -> Result<(), anyhow::Error>;
    fn object_valid(oid: Objid) -> Result<bool, anyhow::Error>;

    fn object_owner(oid: Objid) -> Result<Objid, anyhow::Error>;
    fn set_object_owner(oid: Objid, owner: Objid) -> Result<(), anyhow::Error>;

    fn object_name(oid: Objid) -> Result<String, anyhow::Error>;
    fn set_object_name(oid: Objid, name: &str) -> Result<(), anyhow::Error>;

    fn object_parent(oid:Objid) -> Result<Objid, anyhow::Error>;
    fn change_parent(oid:Objid, parent:Objid) -> Result<(), anyhow::Error>;
    fn count_object_children(oid:Objid) -> Result<usize, anyhow::Error>;
    fn object_children(oid:Objid) -> Result<Vec<Objid>, anyhow::Error>;

    fn object_location(oid:Objid) -> Result<Objid, anyhow::Error>;
    fn change_location(oid:Objid, parent:Objid) -> Result<(), anyhow::Error>;
    fn count_object_contents(oid:Objid) -> Result<usize, anyhow::Error>;
    fn object_contents(oid:Objid) -> Result<Vec<Objid>, anyhow::Error>;

    fn object_check_flags(oid:Objid, flags: Vec<ObjFlag>) -> Result<bool, anyhow::Error>;
    fn set_object_flags(oid:Objid, flags: Vec<ObjFlag>) -> Result<(), anyhow::Error>;

    // fn last_used_object() -> Result<Objid, anyhow::Error>;
    // fn reset_last_used_object() -> Result<Objid, anyhow::Error>;
}

trait Player {
    fn is_object_wizard(oid: Objid) -> Result<bool, anyhow::Error>;
    fn is_object_programmer(oid: Objid) -> Result<bool, anyhow::Error>;
    fn is_object_player(oid: Objid) -> Result<bool, anyhow::Error>;
    fn all_users() -> Result<Vec<Objid>, anyhow::Error>;
}

