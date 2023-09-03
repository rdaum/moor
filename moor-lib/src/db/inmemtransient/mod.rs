/// "local" in-memory transient "db" implementation. For testing.
/// no persistence, no transactions, just a write lock and some hash tables.
use crate::db::db_client::DbTxClient;
use crate::db::db_message::DbMessage;
use crate::db::DbTxWorldState;
use async_trait::async_trait;
use crossbeam_channel::Receiver;
use moor_value::model::defset::HasUuid;
use moor_value::model::objects::ObjFlag;
use moor_value::model::objset::ObjSet;
use moor_value::model::propdef::{PropDef, PropDefs};
use moor_value::model::verbdef::{VerbDef, VerbDefs};
use moor_value::model::world_state::{WorldState, WorldStateSource};
use moor_value::model::WorldStateError::{ObjectNotFound, PropertyNotFound, VerbNotFound};
use moor_value::model::{CommitResult, WorldStateError};
use moor_value::util::bitenum::BitEnum;
use moor_value::var::objid::Objid;
use moor_value::var::Var;
use moor_value::NOTHING;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::thread;
use tracing::warn;
use uuid::Uuid;

struct Object {
    name: String,
    flags: BitEnum<ObjFlag>,
    owner: Objid,
    location: Objid,
    contents: ObjSet,
    parent: Objid,
    children: ObjSet,
}

struct TransientStore {
    verbdefs: HashMap<Objid, VerbDefs>,
    verb_programs: HashMap<(Objid, Uuid), Vec<u8>>,
    objects: HashMap<Objid, Object>,
    propdefs: HashMap<Objid, PropDefs>,
    properties: HashMap<(Objid, Uuid), Var>,
}

impl TransientStore {
    pub(crate) fn descendants(&self, obj: Objid) -> ObjSet {
        let mut search_queue = vec![obj];

        let all_children = std::iter::from_fn(move || {
            while let Some(search_obj) = search_queue.pop() {
                let Some(o) = self.objects.get(&search_obj) else {
                    continue;
                };
                let new_children = o.children.clone();
                search_queue.extend(new_children.iter());
                // Extend the iterator with new children
                return Some(new_children.iter());
            }
            None
        })
        .flatten();

        ObjSet::from_oid_iter(all_children)
    }
}
pub struct InMemTransientDatabase {
    db: Arc<RwLock<TransientStore>>,
}

impl InMemTransientDatabase {
    pub fn new() -> Self {
        InMemTransientDatabase {
            db: Arc::new(RwLock::new(TransientStore {
                verbdefs: HashMap::new(),
                verb_programs: HashMap::new(),
                objects: HashMap::new(),
                propdefs: HashMap::new(),
                properties: HashMap::new(),
            })),
        }
    }
}

fn inmem_db_server(
    db: Arc<RwLock<TransientStore>>,
    rx: Receiver<DbMessage>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        'outer: loop {
            let Ok(msg) = rx.recv() else {
                warn!("Transaction closed without commit");
                return;
            };
            match msg {
                DbMessage::CreateObject { id, attrs, reply } => {
                    let mut db = db.write().unwrap();
                    let obj = Object {
                        name: "".to_string(),
                        flags: attrs.flags.unwrap_or(BitEnum::new()),
                        owner: attrs.owner.unwrap_or(NOTHING),
                        location: attrs.location.unwrap_or(NOTHING),
                        contents: ObjSet::new(),
                        parent: attrs.parent.unwrap_or(NOTHING),
                        children: ObjSet::new(),
                    };
                    let id = id.unwrap_or_else(|| Objid(db.objects.len() as i64));
                    db.objects.insert(id, obj);

                    reply.send(Ok(id)).unwrap();
                }
                DbMessage::GetLocationOf(o, reply) => {
                    let db = db.read().unwrap();
                    let Some(obj) = db.objects.get(&o) else {
                        let _ = reply.send(Err(ObjectNotFound(o)));
                        continue;
                    };
                    reply.send(Ok(obj.location)).unwrap();
                }
                DbMessage::GetContentsOf(o, reply) => {
                    let db = db.read().unwrap();
                    let Some(obj) = db.objects.get(&o) else {
                        let _ = reply.send(Err(ObjectNotFound(o)));
                        continue;
                    };
                    reply.send(Ok(obj.contents.clone())).unwrap();
                }
                DbMessage::SetLocationOf(o, loc, r) => {
                    let mut db = db.write().unwrap();
                    let Some(obj) = db.objects.get_mut(&o) else {
                        let _ = r.send(Err(ObjectNotFound(o)));
                        continue;
                    };
                    let old_location = obj.location;
                    obj.location = loc;
                    if old_location != NOTHING {
                        let old_loc_obj = db.objects.get_mut(&old_location).unwrap();
                        let updated_old_contents = old_loc_obj.contents.with_removed(o);
                        old_loc_obj.contents = updated_old_contents;
                    }
                    if loc != NOTHING {
                        let new_loc_obj = db.objects.get_mut(&loc).unwrap();
                        let updated_new_contents = new_loc_obj.contents.with_inserted(o);
                        new_loc_obj.contents = updated_new_contents;
                    }
                    r.send(Ok(())).unwrap();
                }
                DbMessage::GetObjectFlagsOf(o, r) => {
                    let db = db.read().unwrap();
                    let Some(obj) = db.objects.get(&o) else {
                        let _ = r.send(Err(ObjectNotFound(o)));
                        continue;
                    };
                    r.send(Ok(obj.flags)).unwrap();
                }
                DbMessage::SetObjectFlagsOf(o, f, r) => {
                    let mut db = db.write().unwrap();
                    let Some(obj) = db.objects.get_mut(&o) else {
                        let _ = r.send(Err(ObjectNotFound(o)));
                        continue;
                    };
                    obj.flags = f;
                    r.send(Ok(())).unwrap();
                }
                DbMessage::GetObjectNameOf(o, r) => {
                    let db = db.read().unwrap();
                    let Some(obj) = db.objects.get(&o) else {
                        let _ = r.send(Err(ObjectNotFound(o)));
                        continue;
                    };
                    r.send(Ok(obj.name.clone())).unwrap();
                }
                DbMessage::SetObjectNameOf(o, n, r) => {
                    let mut db = db.write().unwrap();
                    let Some(obj) = db.objects.get_mut(&o) else {
                        let _ = r.send(Err(ObjectNotFound(o)));
                        continue;
                    };
                    obj.name = n;
                    r.send(Ok(())).unwrap();
                }
                DbMessage::GetParentOf(o, r) => {
                    let db = db.read().unwrap();
                    let Some(obj) = db.objects.get(&o) else {
                        let _ = r.send(Err(ObjectNotFound(o)));
                        continue;
                    };
                    r.send(Ok(obj.parent)).unwrap();
                }
                DbMessage::SetParent(o, p, r) => {
                    // TODO This does not implement the full fancy reparenting logic yet, which
                    //   involves moving propdefs around. Just update parent, and remove from
                    //   old parent's children and add to new parent's children for now.
                    let mut db = db.write().unwrap();
                    let Some(obj) = db.objects.get_mut(&o) else {
                        let _ = r.send(Err(ObjectNotFound(o)));
                        continue;
                    };
                    let old_parent = obj.parent;
                    obj.parent = p;
                    if old_parent != NOTHING {
                        let old_parent_obj = db.objects.get_mut(&old_parent).unwrap();
                        let updated_old_children = old_parent_obj.children.with_removed(o);
                        old_parent_obj.children = updated_old_children;
                    }
                    if p != NOTHING {
                        let new_parent_obj = db.objects.get_mut(&p).unwrap();
                        let updated_new_children = new_parent_obj.children.with_inserted(o);
                        new_parent_obj.children = updated_new_children;
                    }
                    r.send(Ok(())).unwrap();
                }
                DbMessage::GetChildrenOf(o, r) => {
                    let db = db.read().unwrap();
                    let Some(obj) = db.objects.get(&o) else {
                        let _ = r.send(Err(ObjectNotFound(o)));
                        continue;
                    };
                    r.send(Ok(obj.children.clone())).unwrap();
                }
                DbMessage::GetObjectOwner(o, r) => {
                    let db = db.read().unwrap();
                    let Some(obj) = db.objects.get(&o) else {
                        let _ = r.send(Err(ObjectNotFound(o)));
                        continue;
                    };
                    r.send(Ok(obj.owner)).unwrap();
                }
                DbMessage::SetObjectOwner(o, no, r) => {
                    let mut db = db.write().unwrap();
                    let Some(obj) = db.objects.get_mut(&o) else {
                        let _ = r.send(Err(ObjectNotFound(o)));
                        continue;
                    };
                    obj.owner = no;
                    r.send(Ok(())).unwrap();
                }
                DbMessage::GetVerbs(o, r) => {
                    let db = db.read().unwrap();
                    let Some(verbdefs) = db.verbdefs.get(&o) else {
                        let _ = r.send(Err(ObjectNotFound(o)));
                        continue;
                    };
                    r.send(Ok(verbdefs.clone())).unwrap();
                }
                DbMessage::GetVerbByName(o, n, r) => {
                    let db = db.read().unwrap();
                    let Some(verbdefs) = db.verbdefs.get(&o) else {
                        let _ = r.send(Err(ObjectNotFound(o)));
                        continue;
                    };
                    let verbdef = verbdefs.find_named(n.as_str());
                    let Some(verbdef) = verbdef else {
                        let _ = r.send(Err(VerbNotFound(o, n)));
                        continue;
                    };
                    r.send(Ok(verbdef)).unwrap();
                }
                DbMessage::GetVerbByIndex(o, idx, r) => {
                    let db = db.read().unwrap();
                    let Some(verbdefs) = db.verbdefs.get(&o) else {
                        let _ = r.send(Err(ObjectNotFound(o)));
                        continue;
                    };
                    if verbdefs.len() <= idx {
                        let _ = r.send(Err(VerbNotFound(o, idx.to_string())));
                        continue;
                    }
                    let verbdef = verbdefs.iter().nth(idx).unwrap();
                    r.send(Ok(verbdef)).unwrap();
                }
                DbMessage::GetVerbBinary(o, uuid, r) => {
                    let db = db.read().unwrap();
                    let Some(binary) = db.verb_programs.get(&(o, uuid)) else {
                        let _ = r.send(Err(VerbNotFound(o, uuid.to_string())));
                        continue;
                    };
                    r.send(Ok(binary.clone())).unwrap();
                }
                DbMessage::ResolveVerb {
                    location,
                    name,
                    argspec: _,
                    reply,
                } => {
                    let db = db.read().unwrap();
                    let Some(_) = db.objects.get(&location) else {
                        let _ = reply.send(Err(ObjectNotFound(location)));
                        continue;
                    };
                    let mut search_o = location;
                    loop {
                        if let Some(verbs) = db.verbdefs.get(&search_o) {
                            // If we found the verb, return it.
                            if let Some(verb) = verbs.find_named(name.as_str()) {
                                reply.send(Ok(verb.clone())).unwrap();
                                continue 'outer;
                            };
                        };
                        // Otherwise, find our parent.  If it's, then set o to it and continue unless we've
                        // hit the end of the chain.
                        let Some(o) = db.objects.get(&search_o) else {
                            break;
                        };
                        let parent = o.parent;
                        if parent == NOTHING {
                            break;
                        }
                        search_o = parent;
                    }
                    reply.send(Err(VerbNotFound(location, name))).unwrap();
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
                    let Some(verbdefs) = db.verbdefs.get_mut(&obj) else {
                        let _ = reply.send(Err(ObjectNotFound(obj)));
                        continue;
                    };
                    let Some(new_verbs) = verbdefs.with_updated(uuid, |ov| {
                        let names = match &names {
                            None => ov.names(),
                            Some(new_names) => {
                                new_names.iter().map(|n| n.as_str()).collect::<Vec<&str>>()
                            }
                        };
                        VerbDef::new(
                            ov.uuid(),
                            ov.location(),
                            owner.unwrap_or(ov.owner()),
                            &names,
                            flags.unwrap_or(ov.flags()),
                            binary_type.unwrap_or(ov.binary_type()),
                            args.unwrap_or(ov.args()),
                        )
                    }) else {
                        let v_uuid_str = uuid.to_string();
                        reply.send(Err(VerbNotFound(obj, v_uuid_str))).unwrap();
                        continue;
                    };
                    db.verbdefs.insert(obj, new_verbs);

                    reply.send(Ok(())).unwrap();
                }
                DbMessage::SetVerbBinary {
                    obj,
                    uuid,
                    binary,
                    reply,
                } => {
                    let mut db = db.write().unwrap();
                    db.verb_programs.insert((obj, uuid), binary);
                    reply.send(Ok(())).unwrap();
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
                    let verbdefs = db.verbdefs.entry(location).or_insert_with(VerbDefs::empty);
                    let uuid = Uuid::new_v4();
                    let new_verbs = verbdefs.with_added(VerbDef::new(
                        uuid,
                        location,
                        owner,
                        &names.iter().map(|n| n.as_str()).collect::<Vec<&str>>(),
                        flags,
                        binary_type,
                        args,
                    ));
                    db.verbdefs.insert(location, new_verbs);
                    db.verb_programs.insert((location, uuid), binary);
                    reply.send(Ok(())).unwrap();
                }
                DbMessage::DeleteVerb {
                    location,
                    uuid,
                    reply,
                } => {
                    let mut db = db.write().unwrap();
                    let verbdefs = db.verbdefs.entry(location).or_insert_with(VerbDefs::empty);
                    let new_verbs = verbdefs.with_removed(uuid).unwrap();
                    db.verbdefs.insert(location, new_verbs);
                    db.verb_programs.remove(&(location, uuid));
                    reply.send(Ok(())).unwrap();
                }
                DbMessage::GetProperties(o, r) => {
                    let db = db.read().unwrap();
                    let Some(propdefs) = db.propdefs.get(&o) else {
                        let _ = r.send(Err(ObjectNotFound(o)));
                        continue;
                    };
                    r.send(Ok(propdefs.clone())).unwrap();
                }
                DbMessage::RetrieveProperty(o, u, r) => {
                    let db = db.read().unwrap();
                    let prop_v = db.properties.get(&(o, u)).unwrap();
                    r.send(Ok(prop_v.clone())).unwrap();
                }
                DbMessage::SetProperty(o, u, v, r) => {
                    let mut db = db.write().unwrap();
                    db.properties.insert((o, u), v);
                    r.send(Ok(())).unwrap();
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

                    let descendants = db.descendants(location);
                    let locations = ObjSet::from(&[location]).with_concatenated(descendants);
                    let uuid = Uuid::new_v4();
                    for location in locations.iter() {
                        let propdefs = db.propdefs.entry(definer).or_insert_with(PropDefs::empty);
                        let new_propdefs = propdefs.with_added(PropDef::new(
                            uuid,
                            location,
                            owner,
                            name.as_str(),
                            perms,
                            owner,
                        ));
                        db.propdefs.insert(definer, new_propdefs);
                    }

                    if let Some(value) = value {
                        db.properties.insert((definer, uuid), value);
                    }

                    reply.send(Ok(uuid)).unwrap();
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
                    let propdefs = db.propdefs.get_mut(&obj).unwrap();
                    let new_propdefs = propdefs
                        .with_updated(uuid, |op| {
                            PropDef::new(
                                op.uuid(),
                                op.location(),
                                new_owner.unwrap_or(op.owner()),
                                new_name.clone().unwrap_or(op.name().to_string()).as_str(),
                                new_flags.unwrap_or(op.flags()),
                                op.owner(),
                            )
                        })
                        .unwrap();
                    db.propdefs.insert(obj, new_propdefs);
                    reply.send(Ok(())).unwrap();
                }
                DbMessage::ClearProperty(o, u, r) => {
                    let mut db = db.write().unwrap();
                    db.properties.remove(&(o, u));
                    r.send(Ok(())).unwrap();
                }
                DbMessage::DeleteProperty(o, u, r) => {
                    let mut db = db.write().unwrap();
                    let propdefs = db.propdefs.get_mut(&o).unwrap();
                    let Some(new_propdefs) = propdefs.with_removed(u) else {
                        let _ = r.send(Err(PropertyNotFound(o, u.to_string())));
                        continue;
                    };
                    db.propdefs.insert(o, new_propdefs);
                    db.properties.remove(&(o, u));
                    r.send(Ok(())).unwrap();
                }
                DbMessage::ResolveProperty(o, n, r) => {
                    let db = db.read().unwrap();
                    // First find the propdef, then seek up the parent tree for the first value.
                    let Some(propdefs) = db.propdefs.get(&o) else {
                        let _ = r.send(Err(PropertyNotFound(o, n)));
                        continue;
                    };
                    let Some(propdef) = propdefs.find_named(n.as_str()) else {
                        let _ = r.send(Err(PropertyNotFound(o, n)));
                        continue;
                    };
                    let mut search_o = o;
                    loop {
                        let Some(prop_v) = db.properties.get(&(search_o, propdef.uuid())) else {
                            let Some(o) = db.objects.get(&search_o) else {
                                let _ = r.send(Err(ObjectNotFound(search_o)));
                                continue 'outer;
                            };
                            let parent = o.parent;
                            if parent == NOTHING {
                                let _ = r.send(Err(PropertyNotFound(search_o, n.clone())));
                                continue 'outer;
                            }
                            search_o = parent;
                            continue;
                        };
                        r.send(Ok((propdef, prop_v.clone()))).unwrap();
                        continue 'outer;
                    }
                }
                DbMessage::Valid(o, r) => {
                    let db = db.read().unwrap();
                    let valid = db.objects.contains_key(&o);
                    r.send(valid).unwrap();
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
