/// "local" in-memory transient "db" implementation. For testing.
/// no persistence, no transactions, just a write lock and some hash tables.
use crate::db::db_client::DbTxClient;
use crate::db::db_message::DbMessage;
use crate::db::inmemtransient::transient_store::TransientStore;
use crate::db::DbTxWorldState;
use async_trait::async_trait;
use crossbeam_channel::Receiver;
use moor_value::model::world_state::{WorldState, WorldStateSource};
use moor_value::model::{CommitResult, WorldStateError};
use std::sync::{Arc, RwLock};
use std::thread;
use tracing::warn;

mod transient_store;

pub struct InMemTransientDatabase {
    db: Arc<RwLock<TransientStore>>,
}

impl InMemTransientDatabase {
    pub fn new() -> Self {
        InMemTransientDatabase {
            db: Arc::new(RwLock::new(TransientStore::new())),
        }
    }
}

fn inmem_db_server(
    db: Arc<RwLock<TransientStore>>,
    rx: Receiver<DbMessage>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        loop {
            let Ok(msg) = rx.recv() else {
                warn!("Transaction closed without commit");
                return;
            };
            match msg {
                DbMessage::CreateObject { id, attrs, reply } => {
                    let mut db = db.write().unwrap();
                    reply.send(db.create_object(id, attrs)).unwrap();
                }
                DbMessage::GetLocationOf(o, reply) => {
                    let db = db.read().unwrap();
                    reply.send(db.get_object_location(o)).unwrap();
                }
                DbMessage::GetContentsOf(o, reply) => {
                    let db = db.read().unwrap();
                    reply.send(db.get_object_contents(o)).unwrap();
                }
                DbMessage::SetLocationOf(o, loc, r) => {
                    let mut db = db.write().unwrap();
                    r.send(db.set_object_location(o, loc)).unwrap();
                }
                DbMessage::GetObjectFlagsOf(o, r) => {
                    let db = db.read().unwrap();
                    r.send(db.get_flags_of(o)).unwrap();
                }
                DbMessage::SetObjectFlagsOf(o, f, r) => {
                    let mut db = db.write().unwrap();
                    r.send(db.set_flags_of(o, f)).unwrap();
                }
                DbMessage::GetObjectNameOf(o, r) => {
                    let db = db.read().unwrap();
                    r.send(db.get_object_name(o)).unwrap();
                }
                DbMessage::SetObjectNameOf(o, n, r) => {
                    let mut db = db.write().unwrap();
                    r.send(db.set_object_name(o, n)).unwrap();
                }
                DbMessage::GetParentOf(o, r) => {
                    let db = db.read().unwrap();
                    r.send(db.get_object_parent(o)).unwrap();
                }
                DbMessage::SetParent(o, p, r) => {
                    let mut db = db.write().unwrap();
                    r.send(db.set_object_parent(o, p)).unwrap();
                }
                DbMessage::GetChildrenOf(o, r) => {
                    let db = db.read().unwrap();
                    r.send(db.get_object_children(o)).unwrap();
                }
                DbMessage::GetObjectOwner(o, r) => {
                    let db = db.read().unwrap();
                    r.send(db.get_object_owner(o)).unwrap();
                }
                DbMessage::SetObjectOwner(o, no, r) => {
                    let mut db = db.write().unwrap();
                    r.send(db.set_object_owner(o, no)).unwrap();
                }
                DbMessage::GetVerbs(o, r) => {
                    let db = db.read().unwrap();
                    r.send(db.get_verbdefs(o)).unwrap();
                }
                DbMessage::GetVerbByName(o, n, r) => {
                    let db = db.read().unwrap();
                    r.send(db.get_verb_by_name(o, n)).unwrap();
                }
                DbMessage::GetVerbByIndex(o, idx, r) => {
                    let db = db.read().unwrap();
                    r.send(db.get_verb_by_index(o, idx)).unwrap();
                }
                DbMessage::GetVerbBinary(o, uuid, r) => {
                    let db = db.read().unwrap();
                    r.send(db.get_binary(o, uuid)).unwrap();
                }
                DbMessage::ResolveVerb {
                    location,
                    name,
                    argspec,
                    reply,
                } => {
                    let db = db.read().unwrap();
                    reply
                        .send(db.resolve_verb(location, name, argspec))
                        .unwrap();
                }
                DbMessage::UpdateVerbDef {
                    obj,
                    uuid,
                    owner,
                    names,
                    flags,
                    binary_type,
                    args,
                    reply,
                } => {
                    let mut db = db.write().unwrap();
                    reply
                        .send(db.update_verbdef(obj, uuid, owner, names, flags, binary_type, args))
                        .unwrap();
                }
                DbMessage::SetVerbBinary {
                    obj,
                    uuid,
                    binary,
                    reply,
                } => {
                    let mut db = db.write().unwrap();
                    reply.send(db.set_verb_binary(obj, uuid, binary)).unwrap();
                }
                DbMessage::AddVerb {
                    location,
                    owner,
                    names,
                    binary_type,
                    binary,
                    flags,
                    args,
                    reply,
                } => {
                    let mut db = db.write().unwrap();
                    reply
                        .send(db.add_object_verb(
                            location,
                            owner,
                            names,
                            binary,
                            binary_type,
                            flags,
                            args,
                        ))
                        .unwrap();
                }
                DbMessage::DeleteVerb {
                    location,
                    uuid,
                    reply,
                } => {
                    let mut db = db.write().unwrap();
                    reply.send(db.delete_verb(location, uuid)).unwrap();
                }
                DbMessage::GetProperties(o, r) => {
                    let db = db.read().unwrap();
                    r.send(db.get_propdefs(o)).unwrap();
                }
                DbMessage::RetrieveProperty(o, u, r) => {
                    let db = db.read().unwrap();
                    r.send(db.retrieve_property(o, u)).unwrap();
                }
                DbMessage::SetProperty(o, u, v, r) => {
                    let mut db = db.write().unwrap();
                    r.send(db.set_property(o, u, v)).unwrap();
                }
                DbMessage::DefineProperty {
                    definer,
                    location,
                    name,
                    owner,
                    perms,
                    value,
                    reply,
                } => {
                    let mut db = db.write().unwrap();
                    reply
                        .send(db.define_property(definer, location, name, owner, perms, value))
                        .unwrap();
                }
                DbMessage::SetPropertyInfo {
                    obj,
                    uuid,
                    new_owner,
                    new_flags,
                    new_name,
                    reply,
                } => {
                    let mut db = db.write().unwrap();
                    reply
                        .send(db.set_property_info(obj, uuid, new_owner, new_flags, new_name))
                        .unwrap();
                }
                DbMessage::ClearProperty(o, u, r) => {
                    let mut db = db.write().unwrap();
                    r.send(db.clear_property(o, u)).unwrap();
                }
                DbMessage::DeleteProperty(o, u, r) => {
                    let mut db = db.write().unwrap();
                    r.send(db.delete_property(o, u)).unwrap();
                }
                DbMessage::ResolveProperty(o, n, r) => {
                    let db = db.read().unwrap();
                    r.send(db.resolve_property(o, n)).unwrap();
                }
                DbMessage::Valid(o, r) => {
                    let db = db.read().unwrap();
                    r.send(db.object_valid(o).unwrap()).unwrap();
                }
                DbMessage::Commit(r) => {
                    r.send(CommitResult::Success).unwrap();
                    return;
                }
                DbMessage::Rollback(r) => {
                    // unimpl
                    r.send(()).unwrap();
                    return;
                }
            }
        }
    })
}

impl InMemTransientDatabase {
    pub fn tx(&mut self) -> Result<Box<DbTxWorldState>, WorldStateError> {
        let (tx, rx) = crossbeam_channel::unbounded();
        let tx_client = DbTxClient { mailbox: tx };
        let join_handle = inmem_db_server(self.db.clone(), rx);
        let tx_world_state = DbTxWorldState {
            join_handle,
            client: tx_client,
        };
        Ok(Box::new(tx_world_state))
    }
}

#[async_trait]
impl WorldStateSource for InMemTransientDatabase {
    #[tracing::instrument(skip(self))]
    async fn new_world_state(&mut self) -> Result<Box<dyn WorldState>, WorldStateError> {
        let tx = self.tx()?;
        Ok(tx)
    }
}
