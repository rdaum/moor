use async_trait::async_trait;
use std::thread;

use crossbeam_channel::Sender;
use strum::{EnumString, EnumVariantNames};

use crate::db::rocksdb::tx_message::Message;
use crate::db::rocksdb::tx_server::{PropHandle, VerbHandle};
use crate::db::CommitResult;
use crate::model::objects::{ObjAttrs, ObjFlag};
use crate::model::props::PropFlag;
use crate::model::r#match::VerbArgsSpec;
use crate::model::verbs::VerbFlag;
use crate::util::bitenum::BitEnum;
use crate::values::objid::Objid;
use crate::values::var::Var;
use crate::vm::opcode::Binary;

pub mod server;
mod tx_db_impl;
mod tx_loader_client;
mod tx_message;
pub mod tx_server;
mod tx_worldstate_client;

pub struct RocksDbTransaction {
    pub(crate) join_handle: thread::JoinHandle<()>,
    pub(crate) mailbox: Sender<Message>,
}

/// Interface exposed to be used by the textdump loader. Overlap of functionality with what
/// WorldState could provide, but potentiall different constraints/semantics.
#[async_trait]
pub trait LoaderInterface {
    async fn create_object(
        &self,
        objid: Option<Objid>,
        attrs: &mut ObjAttrs,
    ) -> Result<Objid, anyhow::Error>;
    async fn set_object_parent(&self, obj: Objid, parent: Objid) -> Result<(), anyhow::Error>;

    async fn set_object_location(&self, o: Objid, location: Objid) -> Result<(), anyhow::Error>;

    async fn add_verb(
        &self,
        obj: Objid,
        names: Vec<&str>,
        owner: Objid,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
        binary: Binary,
    ) -> Result<(), anyhow::Error>;

    async fn get_property(&self, obj: Objid, pname: &str) -> Result<Option<u128>, anyhow::Error>;
    async fn define_property(
        &self,
        definer: Objid,
        objid: Objid,
        propname: &str,
        owner: Objid,
        flags: BitEnum<PropFlag>,
        value: Option<Var>,
        is_clear: bool,
    ) -> Result<(), anyhow::Error>;

    async fn commit(self) -> Result<CommitResult, anyhow::Error>;
}

#[derive(Debug, PartialEq, EnumString, EnumVariantNames)]
#[repr(u8)]
enum ColumnFamilies {
    // Incrementing current object id. TODO: exterminate
    ObjectIds,

    // Object->Parent
    ObjectParent,
    // Object->Children (Vec<ObjId>)
    ObjectChildren,
    // Object->Location
    ObjectLocation,
    // Object->Contents (Vec<ObjId>)
    ObjectContents,
    // Object->Flags (BitEnum<ObjFlag>)
    ObjectFlags,
    // Object->Name
    ObjectName,
    // Object->Owner
    ObjectOwner,

    // Object->Verbs (Vec<VerbHandle>)
    ObjectVerbs,
    // Verb UUID->VerbProgram (Binary)
    VerbProgram,

    // Object->Properties (Vec<PropHandle>)
    ObjectProperties,
    // Property UUID->PropertyValue (Var)
    ObjectPropertyValue,
}

// The underlying physical storage for the database goes through here. Not exposed outside of the
// module.
trait DbStorage {
    fn object_valid(&self, o: Objid) -> Result<bool, anyhow::Error>;
    fn create_object(&self, oid: Option<Objid>, attrs: ObjAttrs) -> Result<Objid, anyhow::Error>;

    fn set_object_parent(&self, o: Objid, new_parent: Objid) -> Result<(), anyhow::Error>;
    fn get_object_children(&self, o: Objid) -> Result<Vec<Objid>, anyhow::Error>;

    fn get_object_name(&self, o: Objid) -> Result<String, anyhow::Error>;
    fn set_object_name(&self, o: Objid, names: String) -> Result<(), anyhow::Error>;

    fn get_object_flags(&self, o: Objid) -> Result<BitEnum<ObjFlag>, anyhow::Error>;
    fn set_object_flags(&self, o: Objid, flags: BitEnum<ObjFlag>) -> Result<(), anyhow::Error>;

    fn get_object_owner(&self, o: Objid) -> Result<Objid, anyhow::Error>;
    fn set_object_owner(&self, o: Objid, owner: Objid) -> Result<(), anyhow::Error>;

    fn get_object_parent(&self, o: Objid) -> Result<Objid, anyhow::Error>;
    fn get_object_location(&self, o: Objid) -> Result<Objid, anyhow::Error>;

    fn get_object_contents(&self, o: Objid) -> Result<Vec<Objid>, anyhow::Error>;
    fn set_object_location(&self, o: Objid, new_location: Objid) -> Result<(), anyhow::Error>;

    fn get_object_verbs(&self, o: Objid) -> Result<Vec<VerbHandle>, anyhow::Error>;
    fn add_object_verb(
        &self,
        oid: Objid,
        owner: Objid,
        names: Vec<String>,
        program: Binary,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
    ) -> Result<(), anyhow::Error>;

    fn delete_object_verb(&self, o: Objid, v: u128) -> Result<(), anyhow::Error>;

    fn get_verb(&self, o: Objid, v: u128) -> Result<VerbHandle, anyhow::Error>;
    fn get_verb_by_name(&self, o: Objid, n: String) -> Result<VerbHandle, anyhow::Error>;
    fn get_verb_by_index(&self, o: Objid, i: usize) -> Result<VerbHandle, anyhow::Error>;
    fn get_program(&self, o: Objid, v: u128) -> Result<Binary, anyhow::Error>;
    fn resolve_verb(
        &self,
        o: Objid,
        n: String,
        a: Option<VerbArgsSpec>,
    ) -> Result<VerbHandle, anyhow::Error>;
    fn retrieve_verb(&self, o: Objid, v: String) -> Result<(Binary, VerbHandle), anyhow::Error>;
    fn set_verb_info(
        &self,
        o: Objid,
        v: u128,
        new_owner: Option<Objid>,
        new_perms: Option<BitEnum<VerbFlag>>,
        new_names: Option<Vec<String>>,
        new_args: Option<VerbArgsSpec>,
    ) -> Result<(), anyhow::Error>;

    fn get_properties(&self, o: Objid) -> Result<Vec<PropHandle>, anyhow::Error>;
    fn retrieve_property(&self, o: Objid, u: u128) -> Result<Var, anyhow::Error>;
    fn set_property_value(&self, o: Objid, u: u128, v: Var) -> Result<(), anyhow::Error>;
    fn set_property_info(
        &self,
        o: Objid,
        u: u128,
        new_owner: Option<Objid>,
        new_perms: Option<BitEnum<PropFlag>>,
        new_name: Option<String>,
        is_clear: Option<bool>,
    ) -> Result<(), anyhow::Error>;
    fn delete_property(&self, o: Objid, u: u128) -> Result<(), anyhow::Error>;
    fn add_property(
        &self,
        location: Objid,
        name: String,
        owner: Objid,
        perms: BitEnum<PropFlag>,
        value: Option<Var>,
        is_clear: bool,
    ) -> Result<PropHandle, anyhow::Error>;
    fn resolve_property(&self, o: Objid, n: String) -> Result<(PropHandle, Var), anyhow::Error>;

    fn commit(self) -> Result<CommitResult, anyhow::Error>;
    fn rollback(&self) -> Result<(), anyhow::Error>;
}
