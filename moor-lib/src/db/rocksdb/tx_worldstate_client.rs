use anyhow::Error;
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

impl WorldState for RocksDbTransaction {
    #[tracing::instrument(skip(self))]
    fn location_of(&mut self, perms: PermissionsContext, obj: Objid) -> Result<Objid, ObjectError> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::GetLocationOf(obj, send))
            .expect("Error sending message");
        let oid = receive.recv().expect("Error receiving message")?;
        Ok(oid)
    }

    #[tracing::instrument(skip(self))]
    fn contents_of(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
    ) -> Result<Vec<Objid>, ObjectError> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::GetContentsOf(obj, send))
            .expect("Error sending message");
        let contents = receive.recv().expect("Error receiving message")?;
        Ok(contents)
    }

    #[tracing::instrument(skip(self))]
    fn flags_of(
        &mut self,
        obj: Objid,
    ) -> Result<BitEnum<ObjFlag>, ObjectError> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::GetFlagsOf(obj, send))
            .expect("Error sending message");
        let flags = receive.recv().expect("Error receiving message")?;
        Ok(flags)
    }

    #[tracing::instrument(skip(self))]
    fn verbs(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
    ) -> Result<Vec<VerbInfo>, ObjectError> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::GetVerbs(obj, send))
            .expect("Error sending message");
        let verbs = receive.recv().expect("Error receiving message")?;
        Ok(verbs
            .iter()
            .map(|vh| {
                // TODO: is definer correct here? I forget if MOO has a Cold-like definer-is-not-location concept
                verbhandle_to_verbinfo(vh, None)
            })
            .collect())
    }

    #[tracing::instrument(skip(self))]
    fn properties(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
    ) -> Result<Vec<(String, PropAttrs)>, ObjectError> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::GetProperties(obj, send))
            .expect("Error sending message");
        let properties = receive.recv().expect("Error receiving message")?;
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
    fn retrieve_property(
        &mut self,
        perms: PermissionsContext,

        obj: Objid,
        pname: &str,
    ) -> Result<Var, ObjectError> {
        // Special properties like name, location, and contents get treated specially.
        if pname == "name" {
            return self.names_of(perms, obj).map(|(name, _)| Var::from(name));
        } else if pname == "location" {
            return self.location_of(perms, obj).map(Var::from);
        } else if pname == "contents" {
            let contents = self
                .contents_of(perms, obj)?
                .iter()
                .map(|o| v_objid(*o))
                .collect();
            return Ok(v_list(contents));
        } else if pname == "owner" {
            return self.owner_of(perms, obj).map(Var::from);
        } else if pname == "programmer" {
            // TODO these can be set, too.
            let flags = self.flags_of(obj)?;
            return if flags.contains(ObjFlag::Programmer) {
                Ok(v_int(1))
            } else {
                Ok(v_int(0))
            };
        } else if pname == "wizard" {
            let flags = self.flags_of( obj)?;
            return if flags.contains(ObjFlag::Wizard) {
                Ok(v_int(1))
            } else {
                Ok(v_int(0))
            };
        }

        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::ResolveProperty(obj, pname.into(), send))
            .expect("Error sending message");
        let (_ph, value) = receive.recv().expect("Error receiving message")?;

        // TODO: use player_flags to check permissions against handle.

        Ok(value)
    }

    fn get_property_info(
        &mut self,
        _perms: PermissionsContext,

        obj: Objid,
        pname: &str,
    ) -> Result<PropAttrs, ObjectError> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::GetProperties(obj, send))
            .expect("Error sending message");
        let properties = receive.recv().expect("Error receiving message")?;
        let ph = properties
            .iter()
            .find(|ph| ph.name == pname)
            .ok_or(ObjectError::PropertyNotFound(obj, pname.into()))?;
        let attrs = prophandle_to_propattrs(ph, None);
        Ok(attrs)
    }

    fn set_property_info(
        &mut self,
        _perms: PermissionsContext,

        obj: Objid,
        pname: &str,
        attrs: PropAttrs,
    ) -> Result<(), ObjectError> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::GetProperties(obj, send))
            .expect("Error sending message");
        let properties = receive.recv().expect("Error receiving message")?;
        let ph = properties
            .iter()
            .find(|ph| ph.name == pname)
            .ok_or(ObjectError::PropertyNotFound(obj, pname.into()))?;

        // TODO perms check
        // Also keep a close eye on 'clear':
        //  "raises `E_INVARG' if <owner> is not valid" & If <object> is the definer of the property
        //   <prop-name>, as opposed to an inheritor of the property, then `clear_property()' raises
        //   `E_INVARG'

        let (send, receive) = crossbeam_channel::bounded(1);
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
        receive.recv().expect("Error receiving message")?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    fn update_property(
        &mut self,
        perms: PermissionsContext,

        obj: Objid,
        pname: &str,
        value: &Var,
    ) -> Result<(), ObjectError> {
        // TODO: use player_flags to check permissions
        // TODO: special property updates

        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::GetProperties(obj, send))
            .expect("Error sending message");
        let properties = receive.recv().expect("Error receiving message")?;
        let ph = properties
            .iter()
            .find(|ph| ph.name == pname)
            .ok_or(ObjectError::PropertyNotFound(obj, pname.into()))?;

        // If the property is marked 'clear' we need to remove that flag.
        // TODO optimization -- we could do this in parallel with the value update.
        // Alternatively, revisit putting the clear bit back in the value instead of the property
        // info.
        if ph.is_clear {
            let (send, receive) = crossbeam_channel::bounded(1);
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
            receive.recv().expect("Error receiving message")?;
        }

        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::SetProperty(
                ph.location,
                ph.uuid,
                value.clone(),
                send,
            ))
            .expect("Error sending message");
        receive.recv().expect("Error receiving message")?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    fn add_property(
        &mut self,
        perms: PermissionsContext,

        definer: Objid,
        obj: Objid,
        pname: &str,
        owner: Objid,
        prop_flags: BitEnum<PropFlag>,
        initial_value: Option<Var>,
    ) -> Result<(), ObjectError> {
        // TODO: prevent special property adds

        let (send, receive) = crossbeam_channel::bounded(1);
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
        receive.recv().expect("Error receiving message")?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    fn add_verb(
        &mut self,
        perms: PermissionsContext,

        obj: Objid,
        names: Vec<String>,
        owner: Objid,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
        program: Binary,
    ) -> Result<(), ObjectError> {
        let (send, receive) = crossbeam_channel::bounded(1);
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
        receive.recv().expect("Error receiving message")?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    fn set_verb_info(
        &mut self,
        perms: PermissionsContext,

        obj: Objid,
        vname: &str,
        owner: Option<Objid>,
        names: Option<Vec<String>>,
        flags: Option<BitEnum<VerbFlag>>,
        args: Option<VerbArgsSpec>,
    ) -> Result<(), ObjectError> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::GetVerbByName(obj, vname.to_string(), send))
            .expect("Error sending message");
        let vh = receive.recv().expect("Error receiving message")?;

        let (send, receive) = crossbeam_channel::bounded(1);
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
        receive.recv().expect("Error receiving message")?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    fn get_verb(
        &mut self,
        perms: PermissionsContext,

        obj: Objid,
        vname: &str,
    ) -> Result<VerbInfo, ObjectError> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::GetVerbByName(obj, vname.to_string(), send))
            .expect("Error sending message");
        let vh = receive.recv().expect("Error receiving message")?;

        // TODO apply permissions check

        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::GetProgram(vh.location, vh.uuid, send))
            .expect("Error sending message");
        let program = receive.recv().expect("Error receiving message")?;
        Ok(verbhandle_to_verbinfo(&vh, Some(program)))
    }

    #[tracing::instrument(skip(self))]
    fn find_method_verb_on(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
        vname: &str,
    ) -> Result<VerbInfo, ObjectError> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::ResolveVerb(obj, vname.to_string(), None, send))
            .expect("Error sending message");
        let vh = receive.recv().expect("Error receiving message")?;

        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::GetProgram(vh.location, vh.uuid, send))
            .expect("Error sending message");
        let program = receive.recv().expect("Error receiving message")?;
        Ok(verbhandle_to_verbinfo(&vh, Some(program)))
    }

    #[tracing::instrument(skip(self))]
    fn find_command_verb_on(
        &mut self,
        perms: PermissionsContext,

        obj: Objid,
        pc: &ParsedCommand,
    ) -> Result<Option<VerbInfo>, ObjectError> {
        if !self.valid(perms, obj)? {
            return Ok(None);
        }

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

        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::ResolveVerb(
                obj,
                pc.verb.clone(),
                Some(argspec),
                send,
            ))
            .expect("Error sending message");

        let vh = receive.recv().expect("Error receiving message");
        let vh = match vh {
            Ok(vh) => vh,
            Err(ObjectError::VerbNotFound(_, _)) => {
                return Ok(None);
            }
            Err(e) => {
                return Err(e);
            }
        };

        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::GetProgram(vh.location, vh.uuid, send))
            .expect("Error sending message");
        let program = receive.recv().expect("Error receiving message")?;
        Ok(Some(verbhandle_to_verbinfo(&vh, Some(program))))
    }

    #[tracing::instrument(skip(self))]
    fn parent_of(&mut self, perms: PermissionsContext, obj: Objid) -> Result<Objid, ObjectError> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::GetParentOf(obj, send))
            .expect("Error sending message");
        let oid = receive.recv().expect("Error receiving message")?;
        Ok(oid)
    }

    #[tracing::instrument(skip(self))]
    fn children_of(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
    ) -> Result<Vec<Objid>, ObjectError> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::GetChildrenOf(obj, send))
            .expect("Error sending message");
        let children = receive.recv().expect("Error receiving message")?;
        debug!("Children: {:?} {:?}", obj, children);
        Ok(children)
    }

    #[tracing::instrument(skip(self))]
    fn valid(&mut self, perms: PermissionsContext, obj: Objid) -> Result<bool, ObjectError> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::Valid(obj, send))
            .expect("Error sending message");
        let valid = receive.recv().expect("Error receiving message");
        Ok(valid)
    }

    #[tracing::instrument(skip(self))]
    fn names_of(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
    ) -> Result<(String, Vec<String>), ObjectError> {
        let (send, receive) = crossbeam_channel::bounded(1);

        // First get name
        self.mailbox
            .send(Message::GetObjectName(obj, send))
            .expect("Error sending message");
        let name = receive.recv().expect("Error receiving message")?;

        // Then grab aliases property.
        let aliases = match self.retrieve_property(perms, obj, "aliases") {
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
    fn owner_of(&mut self, perms: PermissionsContext, obj: Objid) -> Result<Objid, ObjectError> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::GetObjectOwner(obj, send))
            .expect("Error sending message");
        let oid = receive.recv().expect("Error receiving message")?;
        Ok(oid)
    }

    #[tracing::instrument(skip(self))]
    fn commit(&mut self) -> Result<CommitResult, Error> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox.send(Message::Commit(send))?;
        let cr = receive.recv()?;
        // self.join_handle
        //     .join()
        //     .expect("Error completing transaction");
        Ok(cr)
    }

    #[tracing::instrument(skip(self))]
    fn rollback(&mut self) -> Result<(), Error> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox.send(Message::Rollback(send))?;
        receive.recv()?;
        // self.join_handle
        //     .join()
        //     .expect("Error rolling back transaction");
        Ok(())
    }
}
