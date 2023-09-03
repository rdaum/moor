use std::thread;

use async_trait::async_trait;

use uuid::Uuid;

use moor_value::model::objects::ObjAttrs;
use moor_value::model::props::PropFlag;
use moor_value::model::r#match::VerbArgsSpec;
use moor_value::model::verbs::VerbFlag;
use moor_value::model::CommitResult;
use moor_value::util::bitenum::BitEnum;
use moor_value::var::objid::Objid;
use moor_value::var::Var;

use crate::db::db_client::DbTxClient;

pub mod matching;

mod db_client;
mod db_loader_client;
mod db_message;
mod db_worldstate;
pub mod inmemtransient;
pub mod match_env;
pub mod mock;
pub mod rocksdb;

pub struct DbTxWorldState {
    pub join_handle: thread::JoinHandle<()>,
    client: DbTxClient,
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
