use crossbeam_channel::Sender;
use tokio::sync::oneshot;
use uuid::Uuid;

use moor_value::model::objects::{ObjAttrs, ObjFlag};
use moor_value::model::props::PropFlag;
use moor_value::model::r#match::VerbArgsSpec;
use moor_value::model::verbs::{BinaryType, VerbFlag};
use moor_value::model::{CommitResult, WorldStateError};
use moor_value::util::bitenum::BitEnum;
use moor_value::var::objid::{ObjSet, Objid};
use moor_value::var::Var;

use crate::db::db_message::DbMessage;
use crate::db::{PropDef, PropDefs, VerbDef, VerbDefs};

pub(crate) struct DbTxClient {
    pub(crate) mailbox: Sender<DbMessage>,
}

async fn get_reply<R>(
    receive: oneshot::Receiver<Result<R, WorldStateError>>,
) -> Result<R, WorldStateError> {
    receive
        .await
        .map_err(|e| WorldStateError::CommunicationError(e.to_string()))?
}

/// Sends messages over crossbeam channel to the Db tx thread and fields replies.
impl DbTxClient {
    pub fn new(mailbox: Sender<DbMessage>) -> Self {
        Self { mailbox }
    }

    fn send(&self, msg: DbMessage) -> Result<(), WorldStateError> {
        self.mailbox
            .send(msg)
            .map_err(|e| WorldStateError::CommunicationError(e.to_string()))
    }

    pub async fn get_object_owner(&self, obj: Objid) -> Result<Objid, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::GetObjectOwner(obj, send))?;
        get_reply(receive).await
    }

    pub async fn set_object_owner(&self, obj: Objid, owner: Objid) -> Result<(), WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::SetObjectOwner(obj, owner, send))?;
        get_reply(receive).await?;
        Ok(())
    }

    pub async fn get_object_flags(&self, obj: Objid) -> Result<BitEnum<ObjFlag>, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::GetObjectFlagsOf(obj, send))?;
        get_reply(receive).await
    }

    pub async fn set_object_flags(
        &self,
        obj: Objid,
        flags: BitEnum<ObjFlag>,
    ) -> Result<(), WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::SetObjectFlagsOf(obj, flags, send))?;
        get_reply(receive).await?;
        Ok(())
    }

    pub async fn get_object_name(&self, obj: Objid) -> Result<String, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::GetObjectNameOf(obj, send))?;
        let name = get_reply(receive).await?;
        Ok(name)
    }

    pub async fn create_object(
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
    pub async fn set_object_name(&self, obj: Objid, name: String) -> Result<(), WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::SetObjectNameOf(obj, name, send))?;
        get_reply(receive).await?;
        Ok(())
    }

    pub async fn get_parent(&self, obj: Objid) -> Result<Objid, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::GetParentOf(obj, send))?;
        let oid = get_reply(receive).await?;
        Ok(oid)
    }

    pub async fn set_parent(&self, obj: Objid, parent: Objid) -> Result<(), WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::SetParent(obj, parent, send))?;
        get_reply(receive).await?;
        Ok(())
    }

    pub async fn get_children(&self, obj: Objid) -> Result<ObjSet, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::GetChildrenOf(obj, send))?;
        let children = get_reply(receive).await?;
        Ok(children)
    }

    pub async fn get_location_of(&self, obj: Objid) -> Result<Objid, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::GetLocationOf(obj, send))?;
        let oid = get_reply(receive).await?;
        Ok(oid)
    }

    pub async fn set_location_of(
        &self,
        obj: Objid,
        location: Objid,
    ) -> Result<(), WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::SetLocationOf(obj, location, send))?;
        get_reply(receive).await?;
        Ok(())
    }

    pub async fn get_contents_of(&self, obj: Objid) -> Result<ObjSet, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::GetContentsOf(obj, send))?;
        let contents = get_reply(receive).await?;
        Ok(contents)
    }

    pub async fn get_verbs(&self, obj: Objid) -> Result<VerbDefs, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::GetVerbs(obj, send))?;
        let verbs = get_reply(receive).await?;
        Ok(verbs)
    }

    pub async fn get_verb_binary(
        &self,
        obj: Objid,
        uuid: Uuid,
    ) -> Result<Vec<u8>, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::GetVerbBinary(obj, uuid, send))?;
        let verb = get_reply(receive).await?;
        Ok(verb)
    }

    pub async fn get_verb_by_name(
        &self,
        obj: Objid,
        name: String,
    ) -> Result<VerbDef, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::GetVerbByName(obj, name, send))?;
        let verb = get_reply(receive).await?;
        Ok(verb)
    }
    pub async fn get_verb_by_index(
        &self,
        obj: Objid,
        index: usize,
    ) -> Result<VerbDef, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::GetVerbByIndex(obj, index, send))?;
        let verb = get_reply(receive).await?;
        Ok(verb)
    }
    pub async fn resolve_verb(
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
    pub async fn set_verb_info(
        &self,
        obj: Objid,
        uuid: Uuid,
        owner: Option<Objid>,
        flags: Option<BitEnum<VerbFlag>>,
        names: Option<Vec<String>>,
        args: Option<VerbArgsSpec>,
    ) -> Result<(), WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::SetVerbInfo {
            obj,
            uuid,
            owner,
            names,
            flags,
            args,
            reply: send,
        })?;
        get_reply(receive).await?;
        Ok(())
    }
    pub async fn add_verb(
        &self,
        location: Objid,
        owner: Objid,
        names: Vec<String>,
        binary_type: BinaryType,
        binary: Vec<u8>,
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
    pub async fn delete_verb(&self, location: Objid, uuid: Uuid) -> Result<(), WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::DeleteVerb {
            location,
            uuid,
            reply: send,
        })?;
        get_reply(receive).await?;
        Ok(())
    }

    pub async fn get_properties(&self, obj: Objid) -> Result<PropDefs, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::GetProperties(obj, send))?;
        let props = get_reply(receive).await?;
        Ok(props)
    }
    pub async fn set_property(
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
    pub async fn define_property(
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
    pub async fn set_property_info(
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
    pub async fn clear_property(&self, obj: Objid, uuid: Uuid) -> Result<(), WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::ClearProperty(obj, uuid, send))?;
        get_reply(receive).await?;
        Ok(())
    }
    pub async fn delete_property(&self, obj: Objid, uuid: Uuid) -> Result<(), WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::DeleteProperty(obj, uuid, send))?;
        get_reply(receive).await?;
        Ok(())
    }
    pub async fn retrieve_property(&self, obj: Objid, uuid: Uuid) -> Result<Var, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::RetrieveProperty(obj, uuid, send))?;
        let value = get_reply(receive).await?;
        Ok(value)
    }
    pub async fn resolve_property(
        &self,
        obj: Objid,
        name: String,
    ) -> Result<(PropDef, Var), WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::ResolveProperty(obj, name, send))?;
        let (prop, value) = get_reply(receive).await?;
        Ok((prop, value))
    }
    pub async fn valid(&self, obj: Objid) -> Result<bool, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::Valid(obj, send))?;
        let valid = receive
            .await
            .map_err(|e| WorldStateError::CommunicationError(e.to_string()))?;
        Ok(valid)
    }
    pub async fn commit(&self) -> Result<CommitResult, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::Commit(send))?;
        receive
            .await
            .map_err(|e| WorldStateError::CommunicationError(e.to_string()))
    }

    pub async fn rollback(&self) -> Result<(), WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.send(DbMessage::Rollback(send))?;
        receive
            .await
            .map_err(|e| WorldStateError::CommunicationError(e.to_string()))
    }
}
