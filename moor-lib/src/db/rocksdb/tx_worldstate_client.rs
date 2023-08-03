use anyhow::Error;
use async_trait::async_trait;
use tracing::debug;

use crate::db::rocksdb::tx_message::Message;
use crate::db::rocksdb::tx_server::{PropHandle, VerbHandle};
use crate::db::rocksdb::RocksDbTransaction;
use crate::db::CommitResult;
use crate::model::objects::ObjFlag;
use crate::model::permissions::PermissionsContext;
use crate::model::props::{PropAttrs, PropFlag};
use crate::model::r#match::{ArgSpec, VerbArgsSpec};
use crate::model::verbs::{VerbAttrs, VerbFlag, VerbInfo};
use crate::model::world_state::WorldState;
use crate::model::ObjectError;
use crate::tasks::command_parse::ParsedCommand;
use crate::util::bitenum::BitEnum;
use crate::values::objid::{Objid, NOTHING};
use crate::values::var::{v_int, v_list, v_objid, Var};
use crate::values::variant::Variant;
use crate::vm::opcode::Binary;

// all of this right now is direct-talk to physical DB transaction, and should be fronted by a
// cache.
// the challenge is how to make the cache work with the transactional semantics of the DB and
// runtime.
// bare simple would be a rather inefficient cache that is flushed and re-read for each tx
// better would be one that is long lived and shared with other transactions, but this is far more
// challenging, esp if we want to support a distributed db back-end at some point. in that case,
// the invalidation process would need to be distributed as well.
// there's probably some optimistic scheme that could be done here, but here is my first thought
//    * every tx has a cache
//    * there's also a 'global' cache
//    * the tx keeps track of which entities it has modified. when it goes to commit, those
//      entities are locked.
//    * when a tx commits successfully into the db, the committed changes are merged into the
//      upstream cache, and the lock released
//    * if a tx commit fails, the (local) changes are discarded, and, again, the lock released
//    * likely something that should get run through Jepsen

fn verbhandle_to_verbinfo(vh: &VerbHandle, program: Option<Binary>) -> VerbInfo {
    VerbInfo {
        names: vh.names.clone(),
        attrs: VerbAttrs {
            definer: Some(vh.location),
            owner: Some(vh.owner),
            flags: Some(vh.flags),
            args_spec: Some(vh.args),
            program,
        },
    }
}

fn prophandle_to_propattrs(ph: &PropHandle, value: Option<Var>) -> PropAttrs {
    PropAttrs {
        name: Some(ph.name.clone()),
        value,
        location: Some(ph.location),
        owner: Some(ph.owner),
        flags: Some(ph.perms),
        is_clear: Some(ph.is_clear),
    }
}

#[async_trait]
impl WorldState for RocksDbTransaction {
    #[tracing::instrument(skip(self))]
    async fn owner_of(&mut self, obj: Objid) -> Result<Objid, ObjectError> {
        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(Message::GetObjectOwner(obj, send))
            .expect("Error sending message");
        let oid = receive.await.expect("Error receiving message")?;
        Ok(oid)
    }

    #[tracing::instrument(skip(self))]
    async fn flags_of(&mut self, obj: Objid) -> Result<BitEnum<ObjFlag>, ObjectError> {
        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(Message::GetFlagsOf(obj, send))
            .expect("Error sending message");
        let flags = receive.await.expect("Error receiving message")?;
        Ok(flags)
    }

    #[tracing::instrument(skip(self))]
    async fn location_of(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
    ) -> Result<Objid, ObjectError> {
        let (flags, owner) = (self.flags_of(obj).await?, self.owner_of(obj).await?);
        perms
            .task_perms()
            .check_object_allows(owner, flags, ObjFlag::Read)?;

        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(Message::GetLocationOf(obj, send))
            .expect("Error sending message");
        let oid = receive.await.expect("Error receiving message")?;
        Ok(oid)
    }

    #[tracing::instrument(skip(self))]
    async fn contents_of(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
    ) -> Result<Vec<Objid>, ObjectError> {
        let (flags, owner) = (self.flags_of(obj).await?, self.owner_of(obj).await?);
        perms
            .task_perms()
            .check_object_allows(owner, flags, ObjFlag::Read)?;

        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(Message::GetContentsOf(obj, send))
            .expect("Error sending message");
        let contents = receive.await.expect("Error receiving message")?;
        Ok(contents)
    }

    #[tracing::instrument(skip(self))]
    async fn verbs(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
    ) -> Result<Vec<VerbInfo>, ObjectError> {
        let (flags, owner) = (self.flags_of(obj).await?, self.owner_of(obj).await?);
        perms
            .task_perms()
            .check_object_allows(owner, flags, ObjFlag::Read)?;

        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(Message::GetVerbs(obj, send))
            .expect("Error sending message");
        let verbs = receive.await.expect("Error receiving message")?;
        Ok(verbs
            .iter()
            .map(|vh| {
                // TODO: is definer correct here? I forget if MOO has a Cold-like definer-is-not-location concept
                verbhandle_to_verbinfo(vh, None)
            })
            .collect())
    }

    #[tracing::instrument(skip(self))]
    async fn properties(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
    ) -> Result<Vec<(String, PropAttrs)>, ObjectError> {
        let (flags, owner) = (self.flags_of(obj).await?, self.owner_of(obj).await?);
        perms
            .task_perms()
            .check_object_allows(owner, flags, ObjFlag::Read)?;

        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(Message::GetProperties(obj, send))
            .expect("Error sending message");
        let properties = receive.await.expect("Error receiving message")?;
        Ok(properties
            .iter()
            .filter_map(|ph| {
                // Filter out anything that isn't directly defined on us.
                if ph.location != obj {
                    return None;
                }
                Some((ph.name.clone(), prophandle_to_propattrs(ph, None)))
            })
            .collect())
    }

    #[tracing::instrument(skip(self))]
    async fn retrieve_property(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
        pname: &str,
    ) -> Result<Var, ObjectError> {
        // Special properties like name, location, and contents get treated specially.
        if pname == "name" {
            return self
                .names_of(perms, obj)
                .await
                .map(|(name, _)| Var::from(name));
        } else if pname == "location" {
            return self.location_of(perms, obj).await.map(Var::from);
        } else if pname == "contents" {
            let contents = self
                .contents_of(perms, obj)
                .await?
                .iter()
                .map(|o| v_objid(*o))
                .collect();
            return Ok(v_list(contents));
        } else if pname == "owner" {
            return self.owner_of(obj).await.map(Var::from);
        } else if pname == "programmer" {
            // TODO these can be set, too.
            let flags = self.flags_of(obj).await?;
            return if flags.contains(ObjFlag::Programmer) {
                Ok(v_int(1))
            } else {
                Ok(v_int(0))
            };
        } else if pname == "wizard" {
            let flags = self.flags_of(obj).await?;
            return if flags.contains(ObjFlag::Wizard) {
                Ok(v_int(1))
            } else {
                Ok(v_int(0))
            };
        }

        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(Message::ResolveProperty(obj, pname.into(), send))
            .expect("Error sending message");
        let (ph, value) = receive.await.expect("Error receiving message")?;

        perms
            .task_perms()
            .check_property_allows(ph.owner, ph.perms, PropFlag::Read)?;

        Ok(value)
    }

    async fn get_property_info(
        &mut self,
        perms: PermissionsContext,

        obj: Objid,
        pname: &str,
    ) -> Result<PropAttrs, ObjectError> {
        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(Message::GetProperties(obj, send))
            .expect("Error sending message");
        let properties = receive.await.expect("Error receiving message")?;
        let ph = properties
            .iter()
            .find(|ph| ph.name == pname)
            .ok_or(ObjectError::PropertyNotFound(obj, pname.into()))?;

        perms
            .task_perms()
            .check_property_allows(ph.owner, ph.perms, PropFlag::Read)?;

        let attrs = prophandle_to_propattrs(ph, None);
        Ok(attrs)
    }

    async fn set_property_info(
        &mut self,
        perms: PermissionsContext,

        obj: Objid,
        pname: &str,
        attrs: PropAttrs,
    ) -> Result<(), ObjectError> {
        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(Message::GetProperties(obj, send))
            .expect("Error sending message");
        let properties = receive.await.expect("Error receiving message")?;
        let ph = properties
            .iter()
            .find(|ph| ph.name == pname)
            .ok_or(ObjectError::PropertyNotFound(obj, pname.into()))?;

        perms
            .task_perms()
            .check_property_allows(ph.owner, ph.perms, PropFlag::Write)?;

        // Also keep a close eye on 'clear':
        //  "raises `E_INVARG' if <owner> is not valid" & If <object> is the definer of the property
        //   <prop-name>, as opposed to an inheritor of the property, then `clear_property()' raises
        //   `E_INVARG'

        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(Message::SetPropertyInfo {
                obj,
                uuid: ph.uuid,
                new_owner: attrs.owner,
                new_perms: attrs.flags,
                new_name: attrs.name,
                is_clear: None,
                reply: send,
            })
            .expect("Error sending message");
        receive.await.expect("Error receiving message")?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn update_property(
        &mut self,
        perms: PermissionsContext,

        obj: Objid,
        pname: &str,
        value: &Var,
    ) -> Result<(), ObjectError> {
        // TODO: special property updates

        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(Message::GetProperties(obj, send))
            .expect("Error sending message");
        let properties = receive.await.expect("Error receiving message")?;
        let ph = properties
            .iter()
            .find(|ph| ph.name == pname)
            .ok_or(ObjectError::PropertyNotFound(obj, pname.into()))?;

        perms
            .task_perms()
            .check_property_allows(ph.owner, ph.perms, PropFlag::Write)?;

        // If the property is marked 'clear' we need to remove that flag.
        // TODO optimization -- we could do this in parallel with the value update.
        // Alternatively, revisit putting the clear bit back in the value instead of the property
        // info.
        if ph.is_clear {
            let (send, receive) = tokio::sync::oneshot::channel();
            self.mailbox
                .send(Message::SetPropertyInfo {
                    obj,
                    uuid: ph.uuid,
                    new_owner: None,
                    new_perms: None,
                    new_name: None,
                    is_clear: Some(false),
                    reply: send,
                })
                .expect("Error sending message");
            receive.await.expect("Error receiving message")?;
        }

        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(Message::SetProperty(
                ph.location,
                ph.uuid,
                value.clone(),
                send,
            ))
            .expect("Error sending message");
        receive.await.expect("Error receiving message")?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn add_property(
        &mut self,
        perms: PermissionsContext,

        definer: Objid,
        obj: Objid,
        pname: &str,
        owner: Objid,
        prop_flags: BitEnum<PropFlag>,
        initial_value: Option<Var>,
    ) -> Result<(), ObjectError> {
        let (flags, objowner) = (self.flags_of(obj).await?, self.owner_of(obj).await?);
        perms
            .task_perms()
            .check_object_allows(objowner, flags, ObjFlag::Write)?;

        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(Message::DefineProperty {
                definer,
                obj,
                name: pname.into(),
                owner,
                perms: prop_flags,
                value: initial_value,
                // to update & query clear status, there are expected to be separate operation
                // operations?
                is_clear: false,
                reply: send,
            })
            .expect("Error sending message");
        receive.await.expect("Error receiving message")?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn add_verb(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
        names: Vec<String>,
        _owner: Objid,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
        program: Binary,
    ) -> Result<(), ObjectError> {
        let (objflags, owner) = (self.flags_of(obj).await?, self.owner_of(obj).await?);
        perms
            .task_perms()
            .check_object_allows(owner, objflags, ObjFlag::Write)?;

        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(Message::AddVerb {
                location: obj,
                owner,
                names,
                program,
                flags,
                args,
                reply: send,
            })
            .expect("Error sending message");
        receive.await.expect("Error receiving message")?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn set_verb_info(
        &mut self,
        perms: PermissionsContext,

        obj: Objid,
        vname: &str,
        owner: Option<Objid>,
        names: Option<Vec<String>>,
        flags: Option<BitEnum<VerbFlag>>,
        args: Option<VerbArgsSpec>,
    ) -> Result<(), ObjectError> {
        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(Message::GetVerbByName(obj, vname.to_string(), send))
            .expect("Error sending message");
        let vh = receive.await.expect("Error receiving message")?;

        perms
            .task_perms()
            .check_verb_allows(vh.owner, vh.flags, VerbFlag::Write)?;
        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(Message::SetVerbInfo {
                obj,
                uuid: vh.uuid,
                owner,
                names,
                flags,
                args,
                reply: send,
            })
            .expect("Error sending message");
        receive.await.expect("Error receiving message")?;
        Ok(())
    }

    async fn set_verb_info_at_index(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
        vidx: usize,
        owner: Option<Objid>,
        names: Option<Vec<String>>,
        flags: Option<BitEnum<VerbFlag>>,
        args: Option<VerbArgsSpec>,
    ) -> Result<(), ObjectError> {
        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(Message::GetVerbs(obj, send))
            .expect("Error sending message");
        let verbs = receive.await.expect("Error receiving message")?;
        if vidx >= verbs.len() {
            return Err(ObjectError::VerbNotFound(obj, format!("{}", vidx)));
        }
        let vh = verbs[vidx].clone();
        perms
            .task_perms()
            .check_verb_allows(vh.owner, vh.flags, VerbFlag::Write)?;
        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(Message::SetVerbInfo {
                obj,
                uuid: vh.uuid,
                owner,
                names,
                flags,
                args,
                reply: send,
            })
            .expect("Error sending message");
        receive.await.expect("Error receiving message")?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn get_verb(
        &mut self,
        perms: PermissionsContext,

        obj: Objid,
        vname: &str,
    ) -> Result<VerbInfo, ObjectError> {
        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(Message::GetVerbByName(obj, vname.to_string(), send))
            .expect("Error sending message");
        let vh = receive.await.expect("Error receiving message")?;

        perms
            .task_perms()
            .check_verb_allows(vh.owner, vh.flags, VerbFlag::Read)?;

        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(Message::GetProgram(vh.location, vh.uuid, send))
            .expect("Error sending message");
        let program = receive.await.expect("Error receiving message")?;
        Ok(verbhandle_to_verbinfo(&vh, Some(program)))
    }

    async fn get_verb_at_index(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
        vidx: usize,
    ) -> Result<VerbInfo, ObjectError> {
        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(Message::GetVerbByIndex(obj, vidx, send))
            .expect("Error sending message");
        let vh = receive.await.expect("Error receiving message")?;

        perms
            .task_perms()
            .check_verb_allows(vh.owner, vh.flags, VerbFlag::Read)?;

        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(Message::GetProgram(vh.location, vh.uuid, send))
            .expect("Error sending message");
        let program = receive.await.expect("Error receiving message")?;
        Ok(verbhandle_to_verbinfo(&vh, Some(program)))
    }

    #[tracing::instrument(skip(self))]
    async fn find_method_verb_on(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
        vname: &str,
    ) -> Result<VerbInfo, ObjectError> {
        let (objflags, owner) = (self.flags_of(obj).await?, self.owner_of(obj).await?);
        perms
            .task_perms()
            .check_object_allows(owner, objflags, ObjFlag::Read)?;

        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(Message::ResolveVerb(obj, vname.to_string(), None, send))
            .expect("Error sending message");
        let vh = receive.await.expect("Error receiving message")?;

        perms
            .task_perms()
            .check_verb_allows(vh.owner, vh.flags, VerbFlag::Read)?;

        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(Message::GetProgram(vh.location, vh.uuid, send))
            .expect("Error sending message");
        let program = receive.await.expect("Error receiving message")?;
        Ok(verbhandle_to_verbinfo(&vh, Some(program)))
    }

    #[tracing::instrument(skip(self))]
    async fn find_command_verb_on(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
        pc: &ParsedCommand,
    ) -> Result<Option<VerbInfo>, ObjectError> {
        if !self.valid(obj).await? {
            return Ok(None);
        }

        let (objflags, owner) = (self.flags_of(obj).await?, self.owner_of(obj).await?);
        perms
            .task_perms()
            .check_object_allows(owner, objflags, ObjFlag::Read)?;

        let spec_for_fn = |oid, pco| -> ArgSpec {
            if pco == oid {
                ArgSpec::This
            } else if pco == NOTHING {
                ArgSpec::None
            } else {
                ArgSpec::Any
            }
        };

        let dobj = spec_for_fn(obj, pc.dobj);
        let iobj = spec_for_fn(obj, pc.iobj);
        let argspec = VerbArgsSpec {
            dobj,
            prep: pc.prep,
            iobj,
        };

        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(Message::ResolveVerb(
                obj,
                pc.verb.clone(),
                Some(argspec),
                send,
            ))
            .expect("Error sending message");

        let vh = receive.await.expect("Error receiving message");
        let vh = match vh {
            Ok(vh) => vh,
            Err(ObjectError::VerbNotFound(_, _)) => {
                return Ok(None);
            }
            Err(e) => {
                return Err(e);
            }
        };

        perms
            .task_perms()
            .check_verb_allows(vh.owner, vh.flags, VerbFlag::Read)?;

        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(Message::GetProgram(vh.location, vh.uuid, send))
            .expect("Error sending message");
        let program = receive.await.expect("Error receiving message")?;
        Ok(Some(verbhandle_to_verbinfo(&vh, Some(program))))
    }

    #[tracing::instrument(skip(self))]
    async fn parent_of(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
    ) -> Result<Objid, ObjectError> {
        let (objflags, owner) = (self.flags_of(obj).await?, self.owner_of(obj).await?);
        perms
            .task_perms()
            .check_object_allows(owner, objflags, ObjFlag::Read)?;

        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(Message::GetParentOf(obj, send))
            .expect("Error sending message");
        let oid = receive.await.expect("Error receiving message")?;
        Ok(oid)
    }

    #[tracing::instrument(skip(self))]
    async fn children_of(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
    ) -> Result<Vec<Objid>, ObjectError> {
        let (objflags, owner) = (self.flags_of(obj).await?, self.owner_of(obj).await?);
        perms
            .task_perms()
            .check_object_allows(owner, objflags, ObjFlag::Read)?;

        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(Message::GetChildrenOf(obj, send))
            .expect("Error sending message");
        let children = receive.await.expect("Error receiving message")?;
        debug!("Children: {:?} {:?}", obj, children);
        Ok(children)
    }

    #[tracing::instrument(skip(self))]
    async fn valid(&mut self, obj: Objid) -> Result<bool, ObjectError> {
        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(Message::Valid(obj, send))
            .expect("Error sending message");
        let valid = receive.await.expect("Error receiving message");
        Ok(valid)
    }

    #[tracing::instrument(skip(self))]
    async fn names_of(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
    ) -> Result<(String, Vec<String>), ObjectError> {
        // Not sure if we should actually be checking perms here.
        // TODO: check to see if MOO makes names of unreadable objects available.
        let (objflags, owner) = (self.flags_of(obj).await?, self.owner_of(obj).await?);
        perms
            .task_perms()
            .check_object_allows(owner, objflags, ObjFlag::Read)?;

        let (send, receive) = tokio::sync::oneshot::channel();

        // First get name
        self.mailbox
            .send(Message::GetObjectName(obj, send))
            .expect("Error sending message");
        let name = receive.await.expect("Error receiving message")?;

        // Then grab aliases property.
        let aliases = match self.retrieve_property(perms, obj, "aliases").await {
            Ok(a) => match a.variant() {
                Variant::List(a) => a.iter().map(|v| v.to_string()).collect(),
                _ => {
                    vec![]
                }
            },
            Err(_) => {
                vec![]
            }
        };

        Ok((name, aliases))
    }

    #[tracing::instrument(skip(self))]
    async fn commit(&mut self) -> Result<CommitResult, Error> {
        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox.send(Message::Commit(send))?;
        let cr = receive.await?;
        // self.join_handle
        //     .join()
        //     .expect("Error completing transaction");
        Ok(cr)
    }

    #[tracing::instrument(skip(self))]
    async fn rollback(&mut self) -> Result<(), Error> {
        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox.send(Message::Rollback(send))?;
        receive.await?;
        // self.join_handle
        //     .join()
        //     .expect("Error rolling back transaction");
        Ok(())
    }
}
