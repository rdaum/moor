use crate::db::rocksdb::tx_message::Message;
use crate::db::rocksdb::{LoaderInterface, RocksDbTransaction};
use crate::db::CommitResult;
use crate::model::objects::ObjAttrs;
use crate::model::props::PropFlag;
use crate::model::r#match::VerbArgsSpec;
use crate::model::verbs::VerbFlag;
use crate::util::bitenum::BitEnum;
use crate::var::{Objid, Var};
use crate::vm::opcode::Binary;

impl LoaderInterface for RocksDbTransaction {
    fn create_object(
        &self,
        objid: Option<Objid>,
        attrs: &mut ObjAttrs,
    ) -> Result<Objid, anyhow::Error> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox
            .send(Message::CreateObject(objid, attrs.clone(), send))?;
        let oid = receive.recv()??;
        Ok(oid)
    }
    fn set_object_location(&self, o: Objid, location: Objid) -> Result<(), anyhow::Error> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox.send(Message::SetLocation(o, location, send))?;
        receive.recv()??;
        Ok(())
    }
    fn add_verb(
        &self,
        obj: Objid,
        names: Vec<&str>,
        owner: Objid,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
        binary: Binary,
    ) -> Result<(), anyhow::Error> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox.send(Message::AddVerb {
            location: obj,
            owner,
            names: names.iter().map(|s| s.to_string()).collect(),
            program: binary,
            flags,
            args,
            reply: send,
        })?;
        receive.recv()??;
        Ok(())
    }
    fn get_property(&self, obj: Objid, pname: &str) -> Result<Option<u128>, anyhow::Error> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox.send(Message::GetProperties(obj, send))?;
        let properties = receive.recv()??;
        for vh in &properties {
            if vh.name == pname {
                return Ok(Some(vh.uuid));
            }
        }
        Ok(None)
    }
    fn define_property(
        &self,
        objid: Objid,
        propname: &str,
        owner: Objid,
        flags: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<(), anyhow::Error> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox.send(Message::DefineProperty {
            obj: objid,
            name: propname.to_string(),
            owner,
            perms: flags,
            value,
            reply: send,
        })?;
        receive.recv()??;
        Ok(())
    }
    fn set_property(
        &self,
        objid: Objid,
        uuid: u128,
        value: Var,
        owner: Objid,
        flags: BitEnum<PropFlag>,
    ) -> Result<(), anyhow::Error> {
        let (send, receive) = crossbeam_channel::bounded(1);

        // set the value
        self.mailbox
            .send(Message::SetProperty(objid, uuid, value, send))?;
        receive.recv()??;

        // then update the property info
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox.send(Message::SetPropertyInfo {
            obj: objid,
            uuid,
            owner,
            perms: flags,
            new_name: None,
            reply: send,
        })?;

        receive.recv()??;
        Ok(())
    }

    fn commit(self) -> Result<CommitResult, anyhow::Error> {
        let (send, receive) = crossbeam_channel::bounded(1);
        self.mailbox.send(Message::Commit(send))?;
        let cr = receive.recv()?;
        self.join_handle
            .join()
            .expect("Error completing transaction");
        Ok(cr)
    }
}
