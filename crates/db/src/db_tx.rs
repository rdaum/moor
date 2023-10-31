use async_trait::async_trait;
use uuid::Uuid;

use moor_values::model::objects::{ObjAttrs, ObjFlag};
use moor_values::model::objset::ObjSet;
use moor_values::model::propdef::{PropDef, PropDefs};
use moor_values::model::props::PropFlag;
use moor_values::model::r#match::VerbArgsSpec;
use moor_values::model::verbdef::{VerbDef, VerbDefs};
use moor_values::model::verbs::{BinaryType, VerbAttrs, VerbFlag};
use moor_values::model::{CommitResult, WorldStateError};
use moor_values::util::bitenum::BitEnum;
use moor_values::var::objid::Objid;
use moor_values::var::Var;

/// A trait defining a generic interface to a database for storing the the per-attribute values
/// of our objects and their properties and verbs.  Used by DbTxWorldState.
/// One instance per transaction.
#[async_trait]
pub trait DbTransaction {
    async fn get_object_owner(&self, obj: Objid) -> Result<Objid, WorldStateError>;
    async fn set_object_owner(&self, obj: Objid, owner: Objid) -> Result<(), WorldStateError>;
    async fn get_object_flags(&self, obj: Objid) -> Result<BitEnum<ObjFlag>, WorldStateError>;
    async fn set_object_flags(
        &self,
        obj: Objid,
        flags: BitEnum<ObjFlag>,
    ) -> Result<(), WorldStateError>;
    async fn get_object_name(&self, obj: Objid) -> Result<String, WorldStateError>;
    async fn create_object(
        &self,
        id: Option<Objid>,
        attrs: ObjAttrs,
    ) -> Result<Objid, WorldStateError>;
    async fn recycle_object(&self, obj: Objid) -> Result<(), WorldStateError>;
    async fn set_object_name(&self, obj: Objid, name: String) -> Result<(), WorldStateError>;
    async fn get_object_parent(&self, obj: Objid) -> Result<Objid, WorldStateError>;
    async fn set_object_parent(&self, obj: Objid, parent: Objid) -> Result<(), WorldStateError>;
    async fn get_object_children(&self, obj: Objid) -> Result<ObjSet, WorldStateError>;
    async fn get_object_location(&self, obj: Objid) -> Result<Objid, WorldStateError>;
    async fn set_object_location(&self, obj: Objid, location: Objid)
        -> Result<(), WorldStateError>;
    async fn get_object_contents(&self, obj: Objid) -> Result<ObjSet, WorldStateError>;
    async fn get_max_object(&self) -> Result<Objid, WorldStateError>;
    async fn get_verbs(&self, obj: Objid) -> Result<VerbDefs, WorldStateError>;
    // TODO: this could return SliceRef or an Arc<Vec<u8>>, to potentially avoid copying. Though
    //   for RocksDB I don't think it matters, since I don't think it will let us avoid copying
    //   anyway.
    async fn get_verb_binary(&self, obj: Objid, uuid: Uuid) -> Result<Vec<u8>, WorldStateError>;
    async fn get_verb_by_name(&self, obj: Objid, name: String) -> Result<VerbDef, WorldStateError>;
    async fn get_verb_by_index(&self, obj: Objid, index: usize)
        -> Result<VerbDef, WorldStateError>;
    async fn resolve_verb(
        &self,
        obj: Objid,
        name: String,
        argspec: Option<VerbArgsSpec>,
    ) -> Result<VerbDef, WorldStateError>;
    async fn update_verb(
        &self,
        obj: Objid,
        uuid: Uuid,
        verb_attrs: VerbAttrs,
    ) -> Result<(), WorldStateError>;
    async fn add_object_verb(
        &self,
        location: Objid,
        owner: Objid,
        names: Vec<String>,
        binary: Vec<u8>,
        binary_type: BinaryType,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
    ) -> Result<(), WorldStateError>;
    async fn delete_verb(&self, location: Objid, uuid: Uuid) -> Result<(), WorldStateError>;
    async fn get_properties(&self, obj: Objid) -> Result<PropDefs, WorldStateError>;
    async fn set_property(&self, obj: Objid, uuid: Uuid, value: Var)
        -> Result<(), WorldStateError>;
    async fn define_property(
        &self,
        definer: Objid,
        location: Objid,
        name: String,
        owner: Objid,
        perms: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<Uuid, WorldStateError>;
    async fn set_property_info(
        &self,
        obj: Objid,
        uuid: Uuid,
        new_owner: Option<Objid>,
        new_flags: Option<BitEnum<PropFlag>>,
        new_name: Option<String>,
    ) -> Result<(), WorldStateError>;
    async fn clear_property(&self, obj: Objid, uuid: Uuid) -> Result<(), WorldStateError>;
    async fn delete_property(&self, obj: Objid, uuid: Uuid) -> Result<(), WorldStateError>;
    async fn retrieve_property(&self, obj: Objid, uuid: Uuid) -> Result<Var, WorldStateError>;
    async fn resolve_property(
        &self,
        obj: Objid,
        name: String,
    ) -> Result<(PropDef, Var), WorldStateError>;
    async fn object_valid(&self, obj: Objid) -> Result<bool, WorldStateError>;
    async fn commit(&self) -> Result<CommitResult, WorldStateError>;
    async fn rollback(&self) -> Result<(), WorldStateError>;
}
