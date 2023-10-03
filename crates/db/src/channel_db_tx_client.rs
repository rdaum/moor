use async_trait::async_trait;
use crossbeam_channel::Sender;
use tokio::sync::oneshot;
use uuid::Uuid;

use moor_values::model::objects::{ObjAttrs, ObjFlag};
use moor_values::model::objset::ObjSet;
use moor_values::model::propdef::{PropDef, PropDefs};
use moor_values::model::props::PropFlag;
use moor_values::model::r#match::VerbArgsSpec;
use moor_values::model::verbdef::VerbDef;
use moor_values::model::verbs::{BinaryType, VerbAttrs, VerbFlag};
use moor_values::model::{CommitResult, WorldStateError};
use moor_values::util::bitenum::BitEnum;
use moor_values::var::objid::Objid;
use moor_values::var::Var;

use crate::db_message::DbMessage;
use crate::db_tx::DbTransaction;
use moor_values::model::verbdef::VerbDefs;

pub(crate) struct DbTxChannelClient {
    pub(crate) mailbox: Sender<DbMessage>,
}

async fn get_reply<R>(
    receive: oneshot::Receiver<Result<R, WorldStateError>>,
) -> Result<R, WorldStateError> {
    receive
        .await
        .map_err(|e| WorldStateError::DatabaseError(e.to_string()))?
}

/// An implementation of DbTransaction which communicates over a crossbeam channel to a separate
/// (per-transaction) thread. For e.g. systems which have ownership patterns that make it difficult
/// to hold transactions in an async context, etc.
impl DbTxChannelClient {
    pub fn new(mailbox: Sender<DbMessage>) -> Self {
        Self { mailbox }
    }

    fn send(&self, msg: DbMessage) -> Result<(), WorldStateError> {
        self.mailbox
            .send(msg)
            .map_err(|e| WorldStateError::DatabaseError(e.to_string()))
    }
}

#[async_trait]
impl DbTransaction for DbTxChannelClient {
    async fn get_object_owner(&self, obj: Objid) -> Result<Objid, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::GetObjectOwner(obj, send))?;
        get_reply(receive).await
    }
    async fn set_object_owner(&self, obj: Objid, owner: Objid) -> Result<(), WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::SetObjectOwner(obj, owner, send))?;
        get_reply(receive).await?;
        Ok(())
    }
    async fn get_object_flags(&self, obj: Objid) -> Result<BitEnum<ObjFlag>, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::GetObjectFlagsOf(obj, send))?;
        get_reply(receive).await
    }
    async fn set_object_flags(
        &self,
        obj: Objid,
        flags: BitEnum<ObjFlag>,
    ) -> Result<(), WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::SetObjectFlagsOf(obj, flags, send))?;
        get_reply(receive).await?;
        Ok(())
    }
    async fn get_object_name(&self, obj: Objid) -> Result<String, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::GetObjectNameOf(obj, send))?;
        let name = get_reply(receive).await?;
        Ok(name)
    }
    async fn create_object(
        &self,
        id: Option<Objid>,
        attrs: ObjAttrs,
    ) -> Result<Objid, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::CreateObject {
            id,
            attrs,
            reply: send,
        })?;
        let oid = get_reply(receive).await?;
        Ok(oid)
    }
    async fn recycle_object(&self, obj: Objid) -> Result<(), WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::RecycleObject(obj, send))?;
        get_reply(receive).await?;
        Ok(())
    }
    async fn set_object_name(&self, obj: Objid, name: String) -> Result<(), WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::SetObjectNameOf(obj, name, send))?;
        get_reply(receive).await?;
        Ok(())
    }
    async fn get_object_parent(&self, obj: Objid) -> Result<Objid, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::GetParentOf(obj, send))?;
        let oid = get_reply(receive).await?;
        Ok(oid)
    }
    async fn set_object_parent(&self, obj: Objid, parent: Objid) -> Result<(), WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::SetParent(obj, parent, send))?;
        get_reply(receive).await?;
        Ok(())
    }
    async fn get_object_children(&self, obj: Objid) -> Result<ObjSet, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::GetChildrenOf(obj, send))?;
        let children = get_reply(receive).await?;
        Ok(children)
    }
    async fn get_object_location(&self, obj: Objid) -> Result<Objid, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::GetLocationOf(obj, send))?;
        let oid = get_reply(receive).await?;
        Ok(oid)
    }
    async fn set_object_location(
        &self,
        obj: Objid,
        location: Objid,
    ) -> Result<(), WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::SetLocationOf(obj, location, send))?;
        get_reply(receive).await?;
        Ok(())
    }
    async fn get_object_contents(&self, obj: Objid) -> Result<ObjSet, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::GetContentsOf(obj, send))?;
        let contents = get_reply(receive).await?;
        Ok(contents)
    }
    async fn get_max_object(&self) -> Result<Objid, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::GetMaxObject(send))?;
        let oid = get_reply(receive).await?;
        Ok(oid)
    }
    async fn get_verbs(&self, obj: Objid) -> Result<VerbDefs, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::GetVerbs(obj, send))?;
        let verbs = get_reply(receive).await?;
        Ok(verbs)
    }
    // TODO: this could return SliceRef or an Arc<Vec<u8>>, to potentially avoid copying. Though
    //   for RocksDB I don't think it matters, since I don't think it will let us avoid copying
    //   anyway.
    async fn get_verb_binary(&self, obj: Objid, uuid: Uuid) -> Result<Vec<u8>, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::GetVerbBinary(obj, uuid, send))?;
        let verb = get_reply(receive).await?;
        Ok(verb)
    }
    async fn get_verb_by_name(&self, obj: Objid, name: String) -> Result<VerbDef, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::GetVerbByName(obj, name, send))?;
        let verb = get_reply(receive).await?;
        Ok(verb)
    }
    async fn get_verb_by_index(
        &self,
        obj: Objid,
        index: usize,
    ) -> Result<VerbDef, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::GetVerbByIndex(obj, index, send))?;
        let verb = get_reply(receive).await?;
        Ok(verb)
    }
    async fn resolve_verb(
        &self,
        obj: Objid,
        name: String,
        argspec: Option<VerbArgsSpec>,
    ) -> Result<VerbDef, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::ResolveVerb {
            location: obj,
            name,
            argspec,
            reply: send,
        })?;
        let verbdef = get_reply(receive).await?;
        Ok(verbdef)
    }
    async fn update_verb(
        &self,
        obj: Objid,
        uuid: Uuid,
        verb_attrs: VerbAttrs,
    ) -> Result<(), WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::UpdateVerbDef {
            obj,
            uuid,
            owner: verb_attrs.owner,
            names: verb_attrs.names,
            flags: verb_attrs.flags,
            binary_type: verb_attrs.binary_type,
            args: verb_attrs.args_spec,
            reply: send,
        })?;
        get_reply(receive).await?;

        if let Some(binary) = verb_attrs.binary {
            let (send, receive) = oneshot::channel();
            self.send(DbMessage::SetVerbBinary {
                obj,
                uuid,
                binary,
                reply: send,
            })?;
            get_reply(receive).await?;
        }
        Ok(())
    }
    async fn add_object_verb(
        &self,
        location: Objid,
        owner: Objid,
        names: Vec<String>,
        binary: Vec<u8>,
        binary_type: BinaryType,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
    ) -> Result<(), WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::AddVerb {
            location,
            owner,
            names,
            binary_type,
            binary,
            flags,
            args,
            reply: send,
        })?;
        get_reply(receive).await?;
        Ok(())
    }
    async fn delete_verb(&self, location: Objid, uuid: Uuid) -> Result<(), WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::DeleteVerb {
            location,
            uuid,
            reply: send,
        })?;
        get_reply(receive).await?;
        Ok(())
    }
    async fn get_properties(&self, obj: Objid) -> Result<PropDefs, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::GetProperties(obj, send))?;
        let props = get_reply(receive).await?;
        Ok(props)
    }
    async fn set_property(
        &self,
        obj: Objid,
        uuid: Uuid,
        value: Var,
    ) -> Result<(), WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::SetProperty(obj, uuid, value, send))?;
        get_reply(receive).await?;
        Ok(())
    }
    async fn define_property(
        &self,
        definer: Objid,
        location: Objid,
        name: String,
        owner: Objid,
        perms: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<Uuid, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::DefineProperty {
            definer,
            location,
            name,
            owner,
            perms,
            value,
            reply: send,
        })?;
        let uuid = get_reply(receive).await?;
        Ok(uuid)
    }
    async fn set_property_info(
        &self,
        obj: Objid,
        uuid: Uuid,
        new_owner: Option<Objid>,
        new_flags: Option<BitEnum<PropFlag>>,
        new_name: Option<String>,
    ) -> Result<(), WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::SetPropertyInfo {
            obj,
            uuid,
            new_owner,
            new_flags,
            new_name,
            reply: send,
        })?;
        get_reply(receive).await?;
        Ok(())
    }
    async fn clear_property(&self, obj: Objid, uuid: Uuid) -> Result<(), WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::ClearProperty(obj, uuid, send))?;
        get_reply(receive).await?;
        Ok(())
    }
    async fn delete_property(&self, obj: Objid, uuid: Uuid) -> Result<(), WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::DeleteProperty(obj, uuid, send))?;
        get_reply(receive).await?;
        Ok(())
    }
    async fn retrieve_property(&self, obj: Objid, uuid: Uuid) -> Result<Var, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::RetrieveProperty(obj, uuid, send))?;
        let value = get_reply(receive).await?;
        Ok(value)
    }
    async fn resolve_property(
        &self,
        obj: Objid,
        name: String,
    ) -> Result<(PropDef, Var), WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::ResolveProperty(obj, name, send))?;
        let (prop, value) = get_reply(receive).await?;
        Ok((prop, value))
    }
    async fn object_valid(&self, obj: Objid) -> Result<bool, WorldStateError> {
        if obj.0 < 0 {
            return Ok(false);
        }
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::Valid(obj, send))?;
        let valid = receive
            .await
            .map_err(|e| WorldStateError::DatabaseError(e.to_string()))?;
        Ok(valid)
    }
    async fn commit(&self) -> Result<CommitResult, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::Commit(send))?;
        receive
            .await
            .map_err(|e| WorldStateError::DatabaseError(e.to_string()))
    }
    async fn rollback(&self) -> Result<(), WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::Rollback(send))?;
        receive
            .await
            .map_err(|e| WorldStateError::DatabaseError(e.to_string()))
    }
}
