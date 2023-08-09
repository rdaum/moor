use std::thread;

use async_trait::async_trait;
use crossbeam_channel::Sender;
use strum::{EnumString, EnumVariantNames};
use uuid::Uuid;

use moor_value::util::bitenum::BitEnum;
use moor_value::var::objid::Objid;
use moor_value::var::Var;

use crate::db::rocksdb::tx_message::Message;
use crate::db::rocksdb::tx_server::{PropDef, VerbHandle};
use moor_value::model::objects::{ObjAttrs, ObjFlag};
use moor_value::model::props::PropFlag;
use moor_value::model::r#match::VerbArgsSpec;
use moor_value::model::verbs::{BinaryType, VerbFlag};
use moor_value::model::CommitResult;

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
/// WorldState could provide, but potentially different constraints/semantics.
#[async_trait]
pub trait LoaderInterface {
    async fn create_object(
        &self,
        objid: Option<Objid>,
        attrs: &ObjAttrs,
    ) -> Result<Objid, anyhow::Error>;
    async fn set_object_parent(&self, obj: Objid, parent: Objid) -> Result<(), anyhow::Error>;

    async fn set_object_location(&self, o: Objid, location: Objid) -> Result<(), anyhow::Error>;
    async fn set_object_owner(&self, obj: Objid, owner: Objid) -> Result<(), anyhow::Error>;

    async fn add_verb(
        &self,
        obj: Objid,
        names: Vec<&str>,
        owner: Objid,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
        binary: Vec<u8>,
    ) -> Result<(), anyhow::Error>;

    async fn get_property(&self, obj: Objid, pname: &str) -> Result<Option<Uuid>, anyhow::Error>;
    async fn define_property(
        &self,
        definer: Objid,
        objid: Objid,
        propname: &str,
        owner: Objid,
        flags: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<(), anyhow::Error>;
    async fn set_update_property(
        &self,
        objid: Objid,
        propname: &str,
        owner: Objid,
        flags: BitEnum<PropFlag>,
        value: Option<Var>,
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
    ObjectPropDefs,
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
        binary: Vec<u8>,
        binary_type: BinaryType,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
    ) -> Result<(), anyhow::Error>;

    fn delete_object_verb(&self, o: Objid, v: Uuid) -> Result<(), anyhow::Error>;

    fn get_verb(&self, o: Objid, v: Uuid) -> Result<VerbHandle, anyhow::Error>;
    fn get_verb_by_name(&self, o: Objid, n: String) -> Result<VerbHandle, anyhow::Error>;
    fn get_verb_by_index(&self, o: Objid, i: usize) -> Result<VerbHandle, anyhow::Error>;
    fn get_binary(&self, o: Objid, v: Uuid) -> Result<Vec<u8>, anyhow::Error>;
    fn resolve_verb(
        &self,
        o: Objid,
        n: String,
        a: Option<VerbArgsSpec>,
    ) -> Result<VerbHandle, anyhow::Error>;
    fn retrieve_verb(&self, o: Objid, v: String) -> Result<(Vec<u8>, VerbHandle), anyhow::Error>;
    fn set_verb_info(
        &self,
        o: Objid,
        v: Uuid,
        new_owner: Option<Objid>,
        new_perms: Option<BitEnum<VerbFlag>>,
        new_names: Option<Vec<String>>,
        new_args: Option<VerbArgsSpec>,
    ) -> Result<(), anyhow::Error>;

    fn get_propdefs(&self, o: Objid) -> Result<Vec<PropDef>, anyhow::Error>;
    fn retrieve_property(&self, o: Objid, u: Uuid) -> Result<Var, anyhow::Error>;
    fn set_property_value(&self, o: Objid, u: Uuid, v: Var) -> Result<(), anyhow::Error>;
    fn set_property_info(
        &self,
        o: Objid,
        u: Uuid,
        new_owner: Option<Objid>,
        new_perms: Option<BitEnum<PropFlag>>,
        new_name: Option<String>,
    ) -> Result<(), anyhow::Error>;
    fn delete_property(&self, o: Objid, u: Uuid) -> Result<(), anyhow::Error>;
    fn define_property(
        &self,
        definer: Objid,
        location: Objid,
        name: String,
        owner: Objid,
        perms: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<Uuid, anyhow::Error>;
    fn clear_property(&self, o: Objid, u: Uuid) -> Result<(), anyhow::Error>;
    fn resolve_property(&self, o: Objid, n: String) -> Result<(PropDef, Var), anyhow::Error>;

    fn commit(self) -> Result<CommitResult, anyhow::Error>;
    fn rollback(&self) -> Result<(), anyhow::Error>;
}
