use anyhow::Error;
use async_trait::async_trait;

use crate::util::bitenum::BitEnum;
use crate::var::objid::Objid;
use crate::var::Var;

use crate::model::objects::ObjFlag;
use crate::model::permissions::PermissionsContext;
use crate::model::props::{PropAttrs, PropFlag};
use crate::model::r#match::{PrepSpec, VerbArgsSpec};
use crate::model::verbs::{BinaryType, VerbFlag, VerbInfo};
use crate::model::CommitResult;
use crate::model::WorldStateError;

/// A "world state" is anything which represents the shared, mutable, state of the user's
/// environment during verb execution. This includes the location of objects, their contents,
/// their properties, their verbs, etc.
/// Each world state is expected to have a lifetime the length of a single transaction, where a
/// transaction is a single command (or top level verb execution).
/// Each world state is expected to have a consistent shapshotted view of the world, and to
/// commit any changes to the world at the end of the transaction, or be capable of rolling back
/// on failure.
#[async_trait]
pub trait WorldState: Send + Sync {
    // TODO: combine owner & flags into one call, to make perms check more efficient

    /// Get the owner of an object
    async fn owner_of(&mut self, obj: Objid) -> Result<Objid, WorldStateError>;

    /// Flags of an object.
    /// Note this call does not take a permission context, because it is used to *determine*
    /// permissions. It is the caller's responsibility to ensure that the program is using this
    /// call appropriately.
    async fn flags_of(&mut self, obj: Objid) -> Result<BitEnum<ObjFlag>, WorldStateError>;

    /// Set the flags of an object.
    async fn set_flags_of(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
        flags: BitEnum<ObjFlag>,
    ) -> Result<(), Error>;

    /// Get the location of the given object.
    async fn location_of(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
    ) -> Result<Objid, WorldStateError>;

    /// Create a new object, assigning it a new unique object id.
    /// If owner is #-1, the object's is set to itself.
    /// Note it is the caller's responsibility to execute :initialize).
    async fn create_object(
        &mut self,
        perms: PermissionsContext,
        parent: Objid,
        owner: Objid,
    ) -> Result<Objid, WorldStateError>;

    /// Move an object to a new location.
    /// (Note it is the caller's responsibility to execute :accept, :enterfunc, :exitfunc, etc.)
    async fn move_object(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
        new_loc: Objid,
    ) -> Result<(), WorldStateError>;

    /// Get the contents of a given object.
    async fn contents_of(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
    ) -> Result<Vec<Objid>, WorldStateError>;

    /// Get the names of all the verbs on the given object.
    async fn verbs(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
    ) -> Result<Vec<VerbInfo>, WorldStateError>;

    /// Gets a list of the names of the properties defined directly on the given object, not
    /// inherited from its parent.
    async fn properties(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
    ) -> Result<Vec<(String, PropAttrs)>, WorldStateError>;

    /// Retrieve a property from the given object, walking transitively up its inheritance chain.
    async fn retrieve_property(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
        pname: &str,
    ) -> Result<Var, WorldStateError>;

    /// Get information about a property, without walking the inheritance tree.
    async fn get_property_info(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
        pname: &str,
    ) -> Result<PropAttrs, WorldStateError>;

    async fn set_property_info(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
        pname: &str,
        attrs: PropAttrs,
    ) -> Result<(), WorldStateError>;

    /// Update a property on the given object.
    async fn update_property(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
        pname: &str,
        value: &Var,
    ) -> Result<(), WorldStateError>;

    /// Check if a property is 'clear' (value is purely inherited)
    async fn is_property_clear(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
        pname: &str,
    ) -> Result<bool, WorldStateError>;

    /// Clear a property on the given object. That is, remove its local value, if any, and
    /// ensure that it is purely inherited.
    async fn clear_property(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
        pname: &str,
    ) -> Result<(), WorldStateError>;

    /// Add a property for the given object.
    async fn define_property(
        &mut self,
        perms: PermissionsContext,
        definer: Objid,
        location: Objid,
        pname: &str,
        owner: Objid,
        prop_flags: BitEnum<PropFlag>,
        initial_value: Option<Var>,
    ) -> Result<(), WorldStateError>;

    /// Add a verb to the given object.
    async fn add_verb(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
        names: Vec<String>,
        owner: Objid,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
        binary: Vec<u8>,
        binary_type: BinaryType,
    ) -> Result<(), WorldStateError>;

    /// Remove a verb from the given object.
    async fn remove_verb(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
        vname: &str,
    ) -> Result<(), WorldStateError>;

    /// Update data about a verb on the given object.
    async fn set_verb_info(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
        vname: &str,
        owner: Option<Objid>,
        names: Option<Vec<String>>,
        flags: Option<BitEnum<VerbFlag>>,
        args: Option<VerbArgsSpec>,
    ) -> Result<(), WorldStateError>;

    /// Update data about a verb on the given object at a numbered offset.
    async fn set_verb_info_at_index(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
        vidx: usize,
        owner: Option<Objid>,
        names: Option<Vec<String>>,
        flags: Option<BitEnum<VerbFlag>>,
        args: Option<VerbArgsSpec>,
    ) -> Result<(), WorldStateError>;

    /// Get the verb with the given name on the given object. Without doing inheritance resolution.
    async fn get_verb(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
        vname: &str,
    ) -> Result<VerbInfo, WorldStateError>;

    /// Get the verb at numbered offset on the given object.
    async fn get_verb_at_index(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
        vidx: usize,
    ) -> Result<VerbInfo, WorldStateError>;

    /// Retrieve a verb/method from the given object (or its parents).
    async fn find_method_verb_on(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
        vname: &str,
    ) -> Result<VerbInfo, WorldStateError>;

    /// Seek the verb referenced by the given command on the given object.
    async fn find_command_verb_on(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
        command_verb: &str,
        dobj: Objid,
        prep: PrepSpec,
        iobj: Objid,
    ) -> Result<Option<VerbInfo>, WorldStateError>;

    /// Get the object that is the parent of the given object.
    async fn parent_of(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
    ) -> Result<Objid, WorldStateError>;

    /// Change the parent of the given object.
    async fn change_parent(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
        new_parent: Objid,
    ) -> Result<(), WorldStateError>;

    /// Get the children of the given object.
    async fn children_of(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
    ) -> Result<Vec<Objid>, WorldStateError>;

    /// Check the validity of an object.
    async fn valid(&mut self, obj: Objid) -> Result<bool, WorldStateError>;

    /// Get the name & aliases of an object.
    async fn names_of(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
    ) -> Result<(String, Vec<String>), WorldStateError>;

    /// Commit all modifications made to the state of this world since the start of its transaction.
    async fn commit(&mut self) -> Result<CommitResult, anyhow::Error>;

    /// Rollback all modifications made to the state of this world since the start of its transaction.
    async fn rollback(&mut self) -> Result<(), anyhow::Error>;
}

#[async_trait]
pub trait WorldStateSource {
    /// Create a new world state for the given player.
    /// Returns the world state, and a permissions context for the player.
    async fn new_world_state(&mut self) -> Result<Box<dyn WorldState>, anyhow::Error>;
}
