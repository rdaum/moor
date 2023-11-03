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
    /// Check the validity of the given object.
    async fn object_valid(&self, obj: Objid) -> Result<bool, WorldStateError>;

    /// Set the flags of an object.
    async fn get_object_flags(&self, obj: Objid) -> Result<BitEnum<ObjFlag>, WorldStateError>;

    /// Get the set of all objects which are 'players' in the world.
    async fn get_players(&self) -> Result<ObjSet, WorldStateError>;

    /// Get the highest "object #" in the database.
    async fn get_max_object(&self) -> Result<Objid, WorldStateError>;

    /// Get the owner of the given object.
    async fn get_object_owner(&self, obj: Objid) -> Result<Objid, WorldStateError>;

    /// Set the owner of the given object.
    async fn set_object_owner(&self, obj: Objid, owner: Objid) -> Result<(), WorldStateError>;

    /// Set the flags of an object.
    async fn set_object_flags(
        &self,
        obj: Objid,
        flags: BitEnum<ObjFlag>,
    ) -> Result<(), WorldStateError>;

    /// Get the name of the given object.
    async fn get_object_name(&self, obj: Objid) -> Result<String, WorldStateError>;

    /// Set the name of the given object.
    async fn set_object_name(&self, obj: Objid, name: String) -> Result<(), WorldStateError>;

    /// Create a new object, assigning it a new unique object id if one is not provided, and manage
    /// the property inheritance and ownership rules around the object.
    async fn create_object(
        &self,
        id: Option<Objid>,
        attrs: ObjAttrs,
    ) -> Result<Objid, WorldStateError>;

    /// Destroy the given object, and restructure the property inheritance accordingly.
    async fn recycle_object(&self, obj: Objid) -> Result<(), WorldStateError>;
    /// Get the parent of the given object.

    async fn get_object_parent(&self, obj: Objid) -> Result<Objid, WorldStateError>;

    /// Set the parent of the given object, and restructure the property inheritance accordingly.
    async fn set_object_parent(&self, obj: Objid, parent: Objid) -> Result<(), WorldStateError>;

    /// Get the children of the given object.
    async fn get_object_children(&self, obj: Objid) -> Result<ObjSet, WorldStateError>;

    /// Get the location of the given object.
    async fn get_object_location(&self, obj: Objid) -> Result<Objid, WorldStateError>;

    /// Get the contents of the given object.
    async fn get_object_contents(&self, obj: Objid) -> Result<ObjSet, WorldStateError>;

    /// Set the location of the given object.
    async fn set_object_location(&self, obj: Objid, location: Objid)
        -> Result<(), WorldStateError>;

    /// Get all the verb defined on the given object.
    async fn get_verbs(&self, obj: Objid) -> Result<VerbDefs, WorldStateError>;

    /// Get the binary of the given verb.
    // TODO: this could return SliceRef or an Arc<Vec<u8>>, to potentially avoid copying. Though
    //   for RocksDB I don't think it matters, since I don't think it will let us avoid copying
    //   anyway.
    async fn get_verb_binary(&self, obj: Objid, uuid: Uuid) -> Result<Vec<u8>, WorldStateError>;

    /// Find & get the verb with the given name on the given object.
    async fn get_verb_by_name(&self, obj: Objid, name: String) -> Result<VerbDef, WorldStateError>;

    /// Find the Nth verb on the given object. Order is set by the time of creation.
    async fn get_verb_by_index(&self, obj: Objid, index: usize)
        -> Result<VerbDef, WorldStateError>;

    /// Resolve the given verb name on the given object, following the inheritance hierarchy up the
    /// chain of parents.
    async fn resolve_verb(
        &self,
        obj: Objid,
        name: String,
        argspec: Option<VerbArgsSpec>,
    ) -> Result<VerbDef, WorldStateError>;

    /// Update the provided attributes for the given verb.
    async fn update_verb(
        &self,
        obj: Objid,
        uuid: Uuid,
        verb_attrs: VerbAttrs,
    ) -> Result<(), WorldStateError>;

    /// Define a new verb on the given object.
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

    /// Remove the given verb from the given object.
    async fn delete_verb(&self, location: Objid, uuid: Uuid) -> Result<(), WorldStateError>;

    /// Get the properties defined on the given object.
    async fn get_properties(&self, obj: Objid) -> Result<PropDefs, WorldStateError>;

    /// Set the property value on the given object.
    async fn set_property(&self, obj: Objid, uuid: Uuid, value: Var)
        -> Result<(), WorldStateError>;

    /// Define a new property on the given object, and propagate it to all children.
    async fn define_property(
        &self,
        definer: Objid,
        location: Objid,
        name: String,
        owner: Objid,
        perms: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<Uuid, WorldStateError>;

    /// Set the property info on the given object.
    async fn set_property_info(
        &self,
        obj: Objid,
        uuid: Uuid,
        new_owner: Option<Objid>,
        new_flags: Option<BitEnum<PropFlag>>,
        new_name: Option<String>,
    ) -> Result<(), WorldStateError>;

    /// "Clear" the local value of the property on the given object so that it inherits from its
    /// parent.
    async fn clear_property(&self, obj: Objid, uuid: Uuid) -> Result<(), WorldStateError>;

    /// Delete the property from the given object, and propagate the deletion to all children.
    async fn delete_property(&self, obj: Objid, uuid: Uuid) -> Result<(), WorldStateError>;

    /// Retrieve the value of the property without following inheritance.
    async fn retrieve_property(&self, obj: Objid, uuid: Uuid) -> Result<Var, WorldStateError>;

    /// Resolve the given property name on the given object, following the inheritance hierarchy up
    /// the chain of parents.
    async fn resolve_property(
        &self,
        obj: Objid,
        name: String,
    ) -> Result<(PropDef, Var), WorldStateError>;

    /// Attempt to commit the transaction, returning the result of the commit.
    async fn commit(&self) -> Result<CommitResult, WorldStateError>;

    /// Throw away all local mutations.
    async fn rollback(&self) -> Result<(), WorldStateError>;
}
