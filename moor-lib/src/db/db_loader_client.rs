use anyhow::Context;
use async_trait::async_trait;
use uuid::Uuid;

use moor_value::util::bitenum::BitEnum;
use moor_value::var::objid::Objid;
use moor_value::var::Var;

use crate::db::db_message::DbMessage;
use crate::db::{DbTxWorldState, LoaderInterface};
use moor_value::model::objects::ObjAttrs;
use moor_value::model::props::PropFlag;
use moor_value::model::r#match::VerbArgsSpec;
use moor_value::model::verbs::{BinaryType, VerbFlag};
use moor_value::model::CommitResult;

#[async_trait]
impl LoaderInterface for DbTxWorldState {
    async fn create_object(
        &self,
        objid: Option<Objid>,
        attrs: &ObjAttrs,
    ) -> Result<Objid, anyhow::Error> {
        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox.send(DbMessage::CreateObject {
            id: objid,
            attrs: attrs.clone(),
            reply: send,
        })?;
        let oid = receive.await??;
        Ok(oid)
    }
    async fn set_object_parent(&self, obj: Objid, parent: Objid) -> Result<(), anyhow::Error> {
        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox.send(DbMessage::SetParent(obj, parent, send))?;
        receive.await??;
        Ok(())
    }
    async fn set_object_location(&self, o: Objid, location: Objid) -> Result<(), anyhow::Error> {
        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(DbMessage::SetLocationOf(o, location, send))?;
        receive.await??;
        Ok(())
    }
    async fn set_object_owner(&self, obj: Objid, owner: Objid) -> Result<(), anyhow::Error> {
        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(DbMessage::SetObjectOwner(obj, owner, send))?;
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
        binary: Vec<u8>,
    ) -> Result<(), anyhow::Error> {
        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox.send(DbMessage::AddVerb {
            location: obj,
            owner,
            names: names.iter().map(|s| s.to_string()).collect(),
            binary_type: BinaryType::LambdaMoo18X,
            binary,
            flags,
            args,
            reply: send,
        })?;
        receive.await??;
        Ok(())
    }
    async fn get_property(&self, obj: Objid, pname: &str) -> Result<Option<Uuid>, anyhow::Error> {
        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox.send(DbMessage::GetProperties(obj, send))?;
        let properties = receive.await??;
        for vh in properties.iter() {
            if vh.name == pname {
                return Ok(Some(Uuid::from_bytes(vh.uuid)));
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
    ) -> Result<(), anyhow::Error> {
        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox.send(DbMessage::DefineProperty {
            definer,
            location: objid,
            name: propname.to_string(),
            owner,
            perms: flags,
            value,
            reply: send,
        })?;
        receive.await??;
        Ok(())
    }
    async fn set_update_property(
        &self,
        objid: Objid,
        propname: &str,
        owner: Objid,
        flags: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<(), anyhow::Error> {
        // First find the property.
        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox.send(DbMessage::ResolveProperty(
            objid,
            propname.to_string(),
            send,
        ))?;
        let (propdef, _) = receive.await?.with_context(|| {
            format!("Error resolving property {} on object {}", propname, objid)
        })?;
        let uuid = Uuid::from_bytes(propdef.uuid);

        // Now set the value if provided.
        if let Some(value) = value {
            let (send, receive) = tokio::sync::oneshot::channel();
            self.mailbox
                .send(DbMessage::SetProperty(objid, uuid, value, send))?;
            receive
                .await?
                .with_context(|| format!("Error setting value for {}.{}", objid, propname))?;
        }

        // And then set the flags and owner the child had.
        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox
            .send(DbMessage::SetPropertyInfo {
                obj: objid,
                uuid,
                new_owner: Some(owner),
                new_flags: Some(flags),
                new_name: None,
                reply: send,
            })
            .expect("Error sending message");
        receive
            .await?
            .with_context(|| format!("Error setting property info for {}.{}", objid, propname))?;
        Ok(())
    }

    async fn commit(self) -> Result<CommitResult, anyhow::Error> {
        let (send, receive) = tokio::sync::oneshot::channel();
        self.mailbox.send(DbMessage::Commit(send))?;
        let cr = receive.await?;
        self.join_handle
            .join()
            .expect("Error completing transaction");
        Ok(cr)
    }
}
