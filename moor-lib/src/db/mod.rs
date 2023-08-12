use std::ops::Index;
use crate::db::db_message::DbMessage;
use async_trait::async_trait;
use bincode::{Decode, Encode};
use crossbeam_channel::Sender;
use moor_value::model::objects::ObjAttrs;
use moor_value::model::props::PropFlag;
use moor_value::model::r#match::VerbArgsSpec;
use moor_value::model::verbs::{BinaryType, VerbFlag};
use moor_value::model::CommitResult;
use moor_value::util::bitenum::BitEnum;
use moor_value::var::objid::Objid;
use moor_value::var::Var;
use std::thread;
use uuid::Uuid;

pub mod matching;

mod db_loader_client;
mod db_message;
mod db_worldstate;
pub mod match_env;
pub mod mock;
pub mod rocksdb;

// TODO: not sure this is the most appropriate place; used to be in tasks/command_parse.rs, but
// is needed elsewhere (by verb_args, etc)
// Putting here in DB because it's kinda version/DB specific, but not sure it's the best place.
pub const PREP_LIST: [&str; 15] = [
    "with/using",
    "at/to",
    "in front of",
    "in/inside/into",
    "on top of/on/onto/upon",
    "out of/from inside/from",
    "over",
    "through",
    "under/underneath/beneath",
    "behind",
    "beside",
    "for/about",
    "is",
    "as",
    "off/off of",
];

pub struct DbTxWorldState {
    pub(crate) join_handle: thread::JoinHandle<()>,
    pub(crate) mailbox: Sender<DbMessage>,
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

// DB-level representations of objects and verbs. Translate to/from VerbInfo and PropAttrs.
// TODO: We may be able to collapse those into each other, long run. Bit of legacy of how things
// used to be.

#[derive(Debug, Encode, Decode, Clone)]
pub(crate) struct VerbDef {
    pub(crate) uuid: [u8; 16],
    pub(crate) location: Objid,
    pub(crate) owner: Objid,
    pub(crate) names: Vec<String>,
    pub(crate) flags: BitEnum<VerbFlag>,
    pub(crate) binary_type: BinaryType,
    pub(crate) args: VerbArgsSpec,
}

pub trait HasUuid {
    fn uuid(&self) -> Uuid;
}

impl HasUuid for VerbDef {
    fn uuid(&self) -> Uuid {
        Uuid::from_bytes(self.uuid)
    }
}
impl HasUuid for PropDef {
    fn uuid(&self) -> Uuid {
        Uuid::from_bytes(self.uuid)
    }
}

type VerbDefs = Container<VerbDef>;

#[derive(Debug, Encode, Decode, Clone)]
pub(crate) struct PropDef {
    pub(crate) uuid: [u8; 16],
    pub(crate) definer: Objid,
    pub(crate) location: Objid,
    pub(crate) name: String,
    pub(crate) perms: BitEnum<PropFlag>,
    pub(crate) owner: Objid,
}

type PropDefs = Container<PropDef>;


#[derive(Debug, Encode, Decode, Clone)]
pub(crate) struct Container<T: Encode + Decode + Clone + Sized + HasUuid + 'static>(Vec<T>);
impl<T: Encode + Decode + Clone + HasUuid> Container<T> {
    pub fn empty() -> Self {
        Self(vec![])
    }
    pub(crate) fn iter(&self) -> impl Iterator<Item = &T> {
        self.0.iter()
    }
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }
    pub(crate) fn push(&mut self, v: T) {
        self.0.push(v)
    }
    pub(crate) fn contains(&self, uuid: Uuid) -> bool {
        self.0.iter().any(|p| p.uuid() == uuid)
    }
    pub(crate) fn with_removed(&self, uuid: Uuid) -> Option<Self> {
        // Return None if the uuid isn't found, otherwise return a copy with the verb removed.
        if !self.contains(uuid) {
            return None;
        }
        Some(Self(self.0.iter().filter(|v| v.uuid() != uuid).cloned().collect()))
    }
    pub(crate) fn with_updated<F: Fn(&T) -> T>(&mut self, uuid: Uuid, f: F) -> Option<Self> {
        // Return None if the uuid isn't found, otherwise return a copy with the updated verb.
        let mut found = false;
        let mut new = vec![];
        for v in &self.0 {
            if v.uuid() == uuid {
                found = true;
                new.push(f(v));
            } else {
                new.push(v.clone());
            }
        }
        found.then(|| Self(new))
    }
}

impl<T: Encode + Decode + Clone + HasUuid> Index<usize> for Container<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl<T: Encode + Decode + Clone + HasUuid> From<Vec<T>> for Container<T> {
    fn from(v: Vec<T>) -> Self {
        Self(v)
    }
}


