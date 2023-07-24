use crate::db::rocksdb::tx_message::Message;
use crate::db::rocksdb::tx_server::{PropHandle, VerbHandle};
use crate::db::rocksdb::RocksDbTransaction;
use crate::db::state::WorldState;
use crate::db::CommitResult;
use crate::model::objects::ObjFlag;
use crate::model::props::{PropAttrs, PropFlag};
use crate::model::r#match::{ArgSpec, VerbArgsSpec};
use crate::model::verbs::{VerbAttrs, VerbFlag, VerbInfo};
use crate::model::ObjectError;
use crate::tasks::command_parse::ParsedCommand;
use crate::util::bitenum::BitEnum;
use crate::var::{v_int, v_list, v_objid, Objid, Var, Variant, NOTHING};
use crate::vm::opcode::Binary;
use anyhow::Error;

fn verbhandle_to_verbinfo(vh: &VerbHandle, program: Option<Binary>) -> VerbInfo {
    VerbInfo {
        names: vh.names.clone(),
        attrs: VerbAttrs {
            definer: Some(vh.definer),
            owner: Some(vh.owner),
            flags: Some(vh.flags),
            args_spec: Some(vh.args),
            program,
        },
    }
}

fn prophandle_to_propattrs(ph: &PropHandle, value: Option<Var>) -> PropAttrs {
    PropAttrs {
        value,
        location: Some(ph.definer),
        owner: Some(ph.owner),
        flags: Some(ph.perms),
    }
}

impl WorldState for RocksDbTransaction {
    fn location_of(&mut self, obj: Objid) -> Result<Objid, ObjectError> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::GetLocationOf(obj, send))
            .expect("Error sending message");
        let oid = receive.recv().expect("Error receiving message")?;
        Ok(oid)
    }

    fn contents_of(&mut self, obj: Objid) -> Result<Vec<Objid>, ObjectError> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::GetContentsOf(obj, send))
            .expect("Error sending message");
        let contents = receive.recv().expect("Error receiving message")?;
        Ok(contents)
    }

    fn flags_of(&mut self, obj: Objid) -> Result<BitEnum<ObjFlag>, ObjectError> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::GetFlagsOf(obj, send))
            .expect("Error sending message");
        let flags = receive.recv().expect("Error receiving message")?;
        Ok(flags)
    }

    fn verbs(&mut self, obj: Objid) -> Result<Vec<VerbInfo>, ObjectError> {
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

    fn properties(&mut self, obj: Objid) -> Result<Vec<(String, PropAttrs)>, ObjectError> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::GetProperties(obj, send))
            .expect("Error sending message");
        let properties = receive.recv().expect("Error receiving message")?;
        Ok(properties
            .iter()
            .map(|ph| (ph.name.clone(), prophandle_to_propattrs(ph, None)))
            .collect())
    }

    fn retrieve_property(
        &mut self,
        obj: Objid,
        pname: &str,
        _player_flags: BitEnum<ObjFlag>,
    ) -> Result<Var, ObjectError> {
        // TODO: use player_flags to check permissions

        // Special properties like name, location, and contents get treated specially.
        if pname == "name" {
            return self.names_of(obj).map(|(name, _)| Var::from(name));
        } else if pname == "location" {
            return self.location_of(obj).map(Var::from);
        } else if pname == "contents" {
            let contents = self.contents_of(obj)?.iter().map(|o| v_objid(*o)).collect();
            return Ok(v_list(contents));
        } else if pname == "owner" {
            return self.owner_of(obj).map(Var::from);
        } else if pname == "programmer" {
            // TODO these can be set, too.
            let flags = self.flags_of(obj)?;
            return if flags.contains(ObjFlag::Programmer) {
                Ok(v_int(1))
            } else {
                Ok(v_int(0))
            };
        } else if pname == "wizard" {
            let flags = self.flags_of(obj)?;
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
        let ph = receive.recv().expect("Error receiving message")?;
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::RetrieveProperty(ph.definer, ph.uuid, send))
            .expect("Error sending message");
        let value = receive.recv().expect("Error receiving message")?;
        Ok(value)
    }

    fn update_property(
        &mut self,
        obj: Objid,
        pname: &str,
        _player_flags: BitEnum<ObjFlag>,
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
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::SetProperty(
                ph.definer,
                ph.uuid,
                value.clone(),
                send,
            ))
            .expect("Error sending message");
        receive.recv().expect("Error receiving message")?;
        Ok(())
    }

    fn add_property(
        &mut self,
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
                obj,
                name: pname.into(),
                owner,
                perms: prop_flags,
                value: initial_value,
                reply: send,
            })
            .expect("Error sending message");
        receive.recv().expect("Error receiving message")?;
        Ok(())
    }

    fn add_verb(
        &mut self,
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

    fn get_verb(&mut self, obj: Objid, vname: &str) -> Result<VerbInfo, ObjectError> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::GetVerbByName(obj, vname.to_string(), send))
            .expect("Error sending message");
        let vh = receive.recv().expect("Error receiving message")?;

        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::GetProgram(vh.definer, vh.uuid, send))
            .expect("Error sending message");
        let program = receive.recv().expect("Error receiving message")?;
        Ok(verbhandle_to_verbinfo(&vh, Some(program)))
    }

    fn find_method_verb_on(&mut self, obj: Objid, vname: &str) -> Result<VerbInfo, ObjectError> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::ResolveVerb(obj, vname.to_string(), None, send))
            .expect("Error sending message");
        let vh = receive.recv().expect("Error receiving message")?;

        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::GetProgram(vh.definer, vh.uuid, send))
            .expect("Error sending message");
        let program = receive.recv().expect("Error receiving message")?;
        Ok(verbhandle_to_verbinfo(&vh, Some(program)))
    }

    #[tracing::instrument(skip(self, obj, pc))]
    fn find_command_verb_on(
        &mut self,
        obj: Objid,
        pc: &ParsedCommand,
    ) -> Result<Option<VerbInfo>, ObjectError> {
        if !self.valid(obj)? {
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
            .send(Message::GetProgram(vh.definer, vh.uuid, send))
            .expect("Error sending message");
        let program = receive.recv().expect("Error receiving message")?;
        Ok(Some(verbhandle_to_verbinfo(&vh, Some(program))))
    }

    fn parent_of(&mut self, obj: Objid) -> Result<Objid, ObjectError> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::GetParentOf(obj, send))
            .expect("Error sending message");
        let oid = receive.recv().expect("Error receiving message")?;
        Ok(oid)
    }

    fn children_of(&mut self, obj: Objid) -> Result<Vec<Objid>, ObjectError> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::GetChildrenOf(obj, send))
            .expect("Error sending message");
        let children = receive.recv().expect("Error receiving message")?;
        Ok(children)
    }

    fn valid(&mut self, obj: Objid) -> Result<bool, ObjectError> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::Valid(obj, send))
            .expect("Error sending message");
        let valid = receive.recv().expect("Error receiving message");
        Ok(valid)
    }

    fn names_of(&mut self, obj: Objid) -> Result<(String, Vec<String>), ObjectError> {
        let (send, receive) = crossbeam_channel::bounded(1);

        // First get name
        self.mailbox
            .send(Message::GetObjectName(obj, send))
            .expect("Error sending message");
        let name = receive.recv().expect("Error receiving message")?;

        // Then grab aliases property.
        let aliases = match self.retrieve_property(obj, "aliases", BitEnum::new_with(ObjFlag::Read))
        {
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

    fn owner_of(&mut self, obj: Objid) -> Result<Objid, ObjectError> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::GetObjectOwner(obj, send))
            .expect("Error sending message");
        let oid = receive.recv().expect("Error receiving message")?;
        Ok(oid)
    }

    fn commit(&mut self) -> Result<CommitResult, Error> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox.send(Message::Commit(send))?;
        let cr = receive.recv()?;
        // self.join_handle
        //     .join()
        //     .expect("Error completing transaction");
        Ok(cr)
    }

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
