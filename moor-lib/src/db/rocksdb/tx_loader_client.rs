use crate::db::rocksdb::tx_message::Message;
use crate::db::rocksdb::{LoaderInterface, RocksDbTransaction};
use crate::db::CommitResult;
use crate::model::objects::ObjAttrs;
use crate::model::props::PropFlag;
use crate::model::r#match::VerbArgsSpec;
use crate::model::verbs::VerbFlag;
use crate::vm::opcode::Binary;
use async_trait::async_trait;
use moor_value::util::bitenum::BitEnum;
use moor_value::var::objid::Objid;
use moor_value::var::Var;

#[async_trait]
impl LoaderInterface for RocksDbTransaction {
    async fn create_object(
        &self,
        objid: Option<Objid>,
        attrs: &mut ObjAttrs,
    ) -> Result<Objid, anyhow::Error> {
        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(Message::CreateObject(objid, attrs.clone(), send))?;
        let oid = receive.await??;
        Ok(oid)
    }
    async fn set_object_parent(&self, obj: Objid, parent: Objid) -> Result<(), anyhow::Error> {
        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox.send(Message::SetParent(obj, parent, send))?;
        receive.await??;
        Ok(())
    }
    async fn set_object_location(&self, o: Objid, location: Objid) -> Result<(), anyhow::Error> {
        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox.send(Message::SetLocation(o, location, send))?;
        receive.await??;
        Ok(())
    }
    async fn add_verb(
        &self,
        obj: Objid,
        names: Vec<&str>,
        owner: Objid,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
        binary: Binary,
    ) -> Result<(), anyhow::Error> {
        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox.send(Message::AddVerb {
            location: obj,
            owner,
            names: names.iter().map(|s| s.to_string()).collect(),
            program: binary,
            flags,
            args,
            reply: send,
        })?;
        receive.await??;
        Ok(())
    }
    async fn get_property(&self, obj: Objid, pname: &str) -> Result<Option<u128>, anyhow::Error> {
        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox.send(Message::GetProperties(obj, send))?;
        let properties = receive.await??;
        for vh in &properties {
            if vh.name == pname {
                return Ok(Some(vh.uuid));
            }
        }
        Ok(None)
    }
    async fn define_property(
        &self,
        definer: Objid,
        objid: Objid,
        propname: &str,
        owner: Objid,
        flags: BitEnum<PropFlag>,
        value: Option<Var>,
        is_clear: bool,
    ) -> Result<(), anyhow::Error> {
        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox.send(Message::DefineProperty {
            definer,
            obj: objid,
            name: propname.to_string(),
            owner,
            perms: flags,
            value,
            is_clear,
            reply: send,
        })?;
        receive.await??;
        Ok(())
    }

    async fn commit(self) -> Result<CommitResult, anyhow::Error> {
        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox.send(Message::Commit(send))?;
        let cr = receive.await?;
        self.join_handle
            .join()
            .expect("Error completing transaction");
        Ok(cr)
    }
}
