use crate::db::CommitResult;
use crate::model::objects::ObjFlag;
use crate::model::permissions::PermissionsContext;
use crate::model::props::{PropAttrs, PropFlag};
use crate::model::r#match::VerbArgsSpec;
use crate::model::verbs::{VerbFlag, VerbInfo};
use crate::model::ObjectError;
use crate::tasks::command_parse::ParsedCommand;
use crate::util::bitenum::BitEnum;
use crate::values::objid::Objid;
use crate::values::var::Var;
use crate::vm::opcode::Binary;

/// A "world state" is anything which represents the shared, mutable, state of the user's
/// environment during verb execution. This includes the location of objects, their contents,
/// their properties, their verbs, etc.
/// Each world state is expected to have a lifetime the length of a single transaction, where a
/// transaction is a single command (or top level verb execution).
/// Each world state is expected to have a consistent shapshotted view of the world, and to
/// commit any changes to the world at the end of the transaction, or be capable of rolling back
/// on failure.
pub trait WorldState: Send + Sync {
    /// Flags of an object.
    /// Note this call does not take a permission context, because it is used to *determine*
    /// permissions. It is the caller's responsibility to ensure that the program is using this
    /// call appropriately.
    fn flags_of(
        &mut self,
        obj: Objid,
    ) -> Result<BitEnum<ObjFlag>, ObjectError>;

    /// Get the location of the given object.
    fn location_of(&mut self, perms: PermissionsContext, obj: Objid) -> Result<Objid, ObjectError>;

    /// Get the contents of a given object.
    fn contents_of(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
    ) -> Result<Vec<Objid>, ObjectError>;

    /// Get the names of all the verbs on the given object.
    fn verbs(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
    ) -> Result<Vec<VerbInfo>, ObjectError>;

    /// Gets a list of the names of the properties defined directly on the given object, not
    /// inherited from its parent.
    fn properties(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
    ) -> Result<Vec<(String, PropAttrs)>, ObjectError>;

    /// Retrieve a property from the given object, walking transitively up its inheritance chain.
    fn retrieve_property(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
        pname: &str,
    ) -> Result<Var, ObjectError>;

    /// Get information about a property, without walking the inheritance tree.
    fn get_property_info(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
        pname: &str,
    ) -> Result<PropAttrs, ObjectError>;

    fn set_property_info(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
        pname: &str,
        attrs: PropAttrs,
    ) -> Result<(), ObjectError>;

    /// Update a property on the given object.
    fn update_property(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
        pname: &str,
        value: &Var,
    ) -> Result<(), ObjectError>;

    /// Add a property for the given object.
    fn add_property(
        &mut self,
        perms: PermissionsContext,
        definer: Objid,
        obj: Objid,
        pname: &str,
        owner: Objid,
        prop_flags: BitEnum<PropFlag>,
        initial_value: Option<Var>,
    ) -> Result<(), ObjectError>;

    fn add_verb(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
        names: Vec<String>,
        owner: Objid,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
        code: Binary,
    ) -> Result<(), ObjectError>;

    /// Update data about a verb on the given object.
    fn set_verb_info(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
        vname: &str,
        owner: Option<Objid>,
        names: Option<Vec<String>>,
        flags: Option<BitEnum<VerbFlag>>,
        args: Option<VerbArgsSpec>,
    ) -> Result<(), ObjectError>;

    /// Get the verb with the given name on the given object. Without doing inheritance resolution.
    fn get_verb(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
        vname: &str,
    ) -> Result<VerbInfo, ObjectError>;

    /// Retrieve a verb/method from the given object (or its parents).
    fn find_method_verb_on(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
        vname: &str,
    ) -> Result<VerbInfo, ObjectError>;

    /// Seek the verb referenced by the given command on the given object.
    fn find_command_verb_on(
        &mut self,
        perms: PermissionsContext,

        oid: Objid,
        pc: &ParsedCommand,
    ) -> Result<Option<VerbInfo>, ObjectError>;

    /// Get the object that is the parent of the given object.
    fn parent_of(&mut self, perms: PermissionsContext, obj: Objid) -> Result<Objid, ObjectError>;

    /// Get the children of the given object.
    fn children_of(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
    ) -> Result<Vec<Objid>, ObjectError>;

    /// Check the validity of an object.
    fn valid(&mut self, perms: PermissionsContext, obj: Objid) -> Result<bool, ObjectError>;

    /// Get the name & aliases of an object.
    fn names_of(
        &mut self,
        perms: PermissionsContext,
        obj: Objid,
    ) -> Result<(String, Vec<String>), ObjectError>;

    /// Get the owner of an object
    fn owner_of(&mut self, perms: PermissionsContext, obj: Objid) -> Result<Objid, ObjectError>;

    /// Commit all modifications made to the state of this world since the start of its transaction.
    fn commit(&mut self) -> Result<CommitResult, anyhow::Error>;

    /// Rollback all modifications made to the state of this world since the start of its transaction.
    fn rollback(&mut self) -> Result<(), anyhow::Error>;
}

pub trait WorldStateSource {
    /// Create a new world state for the given player.
    /// Returns the world state, and a permissions context for the player.
    fn new_world_state(
        &mut self,
        player: Objid,
    ) -> Result<(Box<dyn WorldState>, PermissionsContext), anyhow::Error>;
}
