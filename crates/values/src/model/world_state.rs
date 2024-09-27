// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use bincode::{Decode, Encode};
use bytes::Bytes;
use thiserror::Error;
use uuid::Uuid;

use crate::model::objects::ObjFlag;
use crate::model::objset::ObjSet;
use crate::model::propdef::{PropDef, PropDefs};
use crate::model::props::{PropAttrs, PropFlag};
use crate::model::r#match::{PrepSpec, VerbArgsSpec};
use crate::model::verbdef::{VerbDef, VerbDefs};
use crate::model::verbs::{BinaryType, VerbAttrs, VerbFlag};
use crate::model::{CommitResult, ObjectRef, PropPerms};
use crate::model::{ObjAttr, Vid};
use crate::util::BitEnum;
use crate::Symbol;
use crate::Var;
use crate::{Error, Objid};

/// Errors related to the world state and operations on it.
#[derive(Error, Debug, Eq, PartialEq, Clone, Decode, Encode)]
pub enum WorldStateError {
    #[error("Object not found: {0}")]
    ObjectNotFound(ObjectRef),
    #[error("Object already exists: {0}")]
    ObjectAlreadyExists(Objid),
    #[error("Could not set/get object attribute; {0} on {1}")]
    ObjectAttributeError(ObjAttr, Objid),
    #[error("Recursive move detected: {0} -> {1}")]
    RecursiveMove(Objid, Objid),

    #[error("Object permission denied")]
    ObjectPermissionDenied,

    #[error("Property not found: {0}.{1}")]
    PropertyNotFound(Objid, String),
    #[error("Property permission denied")]
    PropertyPermissionDenied,
    #[error("Property definition not found: {0}.{1}")]
    PropertyDefinitionNotFound(Objid, String),
    #[error("Duplicate property definition: {0}.{1}")]
    DuplicatePropertyDefinition(Objid, String),
    #[error("Property type mismatch")]
    PropertyTypeMismatch,

    #[error("Verb not found: {0}:{1}")]
    VerbNotFound(Objid, String),
    #[error("Verb definition not {0:?}")]
    InvalidVerb(Vid),

    #[error("Invalid verb, decode error: {0}:{1}")]
    VerbDecodeError(Objid, Symbol),
    #[error("Verb permission denied")]
    VerbPermissionDenied,
    #[error("Verb already exists: {0}:{1}")]
    DuplicateVerb(Objid, Symbol),

    #[error("Failed object match: {0}")]
    FailedMatch(String),
    #[error("Ambiguous object match: {0}")]
    AmbiguousMatch(String),

    // Catch-alls for system level object DB errors.
    #[error("DB communications/internal error: {0}")]
    DatabaseError(String),

    /// A rollback was requested, and the caller should retry the operation.
    #[error("Rollback requested, retry operation")]
    RollbackRetry,
}

/// Translations from WorldStateError to MOO error codes.
impl WorldStateError {
    pub fn to_error_code(&self) -> Error {
        match self {
            Self::ObjectNotFound(_) => Error::E_INVIND,
            Self::ObjectPermissionDenied => Error::E_PERM,
            Self::RecursiveMove(_, _) => Error::E_RECMOVE,
            Self::VerbNotFound(_, _) => Error::E_VERBNF,
            Self::VerbPermissionDenied => Error::E_PERM,
            Self::InvalidVerb(_) => Error::E_VERBNF,
            Self::DuplicateVerb(_, _) => Error::E_INVARG,
            Self::PropertyNotFound(_, _) => Error::E_PROPNF,
            Self::PropertyPermissionDenied => Error::E_PERM,
            Self::PropertyDefinitionNotFound(_, _) => Error::E_PROPNF,
            Self::DuplicatePropertyDefinition(_, _) => Error::E_INVARG,
            Self::PropertyTypeMismatch => Error::E_TYPE,
            _ => {
                panic!("Unhandled error code: {:?}", self);
            }
        }
    }

    pub fn database_error_msg(&self) -> Option<&str> {
        if let Self::DatabaseError(msg) = self {
            Some(msg)
        } else {
            None
        }
    }
}

impl From<WorldStateError> for Error {
    fn from(val: WorldStateError) -> Self {
        val.to_error_code()
    }
}

/// A "world state" is anything which represents the shared, mutable, state of the user's
/// environment during verb execution. This includes the location of objects, their contents,
/// their properties, their verbs, etc.
/// Each world state is expected to have a lifetime the length of a single transaction, where a
/// transaction is a single command (or top level verb execution).
/// Each world state is expected to have a consistent shapshotted view of the world, and to
/// commit any changes to the world at the end of the transaction, or be capable of rolling back
/// on failure.
pub trait WorldState: Send {
    // TODO: Combine worlstate owner & flags check into one call, to make perms check more efficient

    /// Get the set of all objects which are 'players' in the world.
    fn players(&self) -> Result<ObjSet, WorldStateError>;

    /// Get the owner of an object
    fn owner_of(&self, obj: Objid) -> Result<Objid, WorldStateError>;

    /// Return whether the given object is controlled by the given player.
    /// (Either who is wizard, or is owner of what).
    fn controls(&self, who: Objid, what: Objid) -> Result<bool, WorldStateError>;

    /// Flags of an object.
    /// Note this call does not take a permission context, because it is used to *determine*
    /// permissions. It is the caller's responsibility to ensure that the program is using this
    /// call appropriately.
    fn flags_of(&self, obj: Objid) -> Result<BitEnum<ObjFlag>, WorldStateError>;

    /// Set the flags of an object.
    fn set_flags_of(
        &mut self,
        perms: Objid,
        obj: Objid,
        flags: BitEnum<ObjFlag>,
    ) -> Result<(), WorldStateError>;

    /// Get the location of the given object.
    fn location_of(&self, perms: Objid, obj: Objid) -> Result<Objid, WorldStateError>;

    /// Return the number of bytes used by the given object and all its attributes.
    fn object_bytes(&self, perms: Objid, obj: Objid) -> Result<usize, WorldStateError>;

    /// Create a new object, assigning it a new unique object id.
    /// If owner is #-1, the object's is set to itself.
    /// Note it is the caller's responsibility to execute :initialize).
    fn create_object(
        &mut self,
        perms: Objid,
        parent: Objid,
        owner: Objid,
        flags: BitEnum<ObjFlag>,
    ) -> Result<Objid, WorldStateError>;

    /// Recycles (destroys) the given object, and re-parents all its children to the next parent up
    /// the chain, including removing property definitions inherited from the object.
    /// If the object is a location, the contents of that location are moved to #-1.
    /// (It is the caller's (bf_recycle) responsibility to execute :exitfunc for those objects).
    fn recycle_object(&mut self, perms: Objid, obj: Objid) -> Result<(), WorldStateError>;

    /// Return the highest used object # in the system.
    fn max_object(&self, perms: Objid) -> Result<Objid, WorldStateError>;

    /// Move an object to a new location.
    /// (Note it is the caller's responsibility to execute :accept, :enterfunc, :exitfunc, etc.)
    fn move_object(
        &mut self,
        perms: Objid,
        obj: Objid,
        new_loc: Objid,
    ) -> Result<(), WorldStateError>;

    /// Get the contents of a given object.
    fn contents_of(&self, perms: Objid, obj: Objid) -> Result<ObjSet, WorldStateError>;

    /// Get the names of all the verbs on the given object.
    fn verbs(&self, perms: Objid, obj: Objid) -> Result<VerbDefs, WorldStateError>;

    /// Gets a list of the names of the properties defined directly on the given object, not
    /// inherited from its parent.
    fn properties(&self, perms: Objid, obj: Objid) -> Result<PropDefs, WorldStateError>;

    /// Retrieve a property from the given object, walking transitively up its inheritance chain.
    fn retrieve_property(
        &self,
        perms: Objid,
        obj: Objid,
        pname: Symbol,
    ) -> Result<Var, WorldStateError>;

    /// Get information about a property, without walking the inheritance tree.
    /// Returns the PropDef as well as the owner of the property.
    fn get_property_info(
        &self,
        perms: Objid,
        obj: Objid,
        pname: Symbol,
    ) -> Result<(PropDef, PropPerms), WorldStateError>;

    fn set_property_info(
        &mut self,
        perms: Objid,
        obj: Objid,
        pname: Symbol,
        attrs: PropAttrs,
    ) -> Result<(), WorldStateError>;

    /// Update a property on the given object.
    fn update_property(
        &mut self,
        perms: Objid,
        obj: Objid,
        pname: Symbol,
        value: &Var,
    ) -> Result<(), WorldStateError>;

    /// Check if a property is 'clear' (value is purely inherited)
    fn is_property_clear(
        &self,
        perms: Objid,
        obj: Objid,
        pname: Symbol,
    ) -> Result<bool, WorldStateError>;

    /// Clear a property on the given object. That is, remove its local value, if any, and
    /// ensure that it is purely inherited.
    fn clear_property(
        &mut self,
        perms: Objid,
        obj: Objid,
        pname: Symbol,
    ) -> Result<(), WorldStateError>;

    /// Add a property for the given object.
    // Yes yes I know it's a lot of arguments, but wrapper object here is redundant.
    #[allow(clippy::too_many_arguments)]
    fn define_property(
        &mut self,
        perms: Objid,
        definer: Objid,
        location: Objid,
        pname: Symbol,
        owner: Objid,
        prop_flags: BitEnum<PropFlag>,
        initial_value: Option<Var>,
    ) -> Result<(), WorldStateError>;

    fn delete_property(
        &mut self,
        perms: Objid,
        obj: Objid,
        pname: Symbol,
    ) -> Result<(), WorldStateError>;

    /// Add a verb to the given object.
    // Yes yes I know it's a lot of arguments, but wrapper object here is redundant.
    #[allow(clippy::too_many_arguments)]
    fn add_verb(
        &mut self,
        perms: Objid,
        obj: Objid,
        names: Vec<Symbol>,
        owner: Objid,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
        binary: Vec<u8>,
        binary_type: BinaryType,
    ) -> Result<(), WorldStateError>;

    /// Remove a verb from the given object.
    fn remove_verb(&mut self, perms: Objid, obj: Objid, verb: Uuid) -> Result<(), WorldStateError>;

    /// Update data about a verb on the given object.
    fn update_verb(
        &mut self,
        perms: Objid,
        obj: Objid,
        vname: Symbol,
        verb_attrs: VerbAttrs,
    ) -> Result<(), WorldStateError>;

    /// Update data about a verb on the given object at a numbered offset.
    fn update_verb_at_index(
        &mut self,
        perms: Objid,
        obj: Objid,
        vidx: usize,
        verb_attrs: VerbAttrs,
    ) -> Result<(), WorldStateError>;

    fn update_verb_with_id(
        &mut self,
        perms: Objid,
        obj: Objid,
        uuid: Uuid,
        verb_attrs: VerbAttrs,
    ) -> Result<(), WorldStateError>;

    /// Get the verbdef with the given name on the given object. Without doing inheritance resolution.
    fn get_verb(&self, perms: Objid, obj: Objid, vname: Symbol)
        -> Result<VerbDef, WorldStateError>;

    /// Get the verbdef at numbered offset on the given object.
    fn get_verb_at_index(
        &self,
        perms: Objid,
        obj: Objid,
        vidx: usize,
    ) -> Result<VerbDef, WorldStateError>;

    /// Get the verb binary for the given verbdef.
    fn retrieve_verb(
        &self,
        perms: Objid,
        obj: Objid,
        uuid: Uuid,
    ) -> Result<(Bytes, VerbDef), WorldStateError>;

    /// Retrieve a verb/method from the given object (or its parents).
    fn find_method_verb_on(
        &self,
        perms: Objid,
        obj: Objid,
        vname: Symbol,
    ) -> Result<(Bytes, VerbDef), WorldStateError>;

    /// Seek the verb referenced by the given command on the given object.
    fn find_command_verb_on(
        &self,
        perms: Objid,
        obj: Objid,
        command_verb: Symbol,
        dobj: Objid,
        prep: PrepSpec,
        iobj: Objid,
    ) -> Result<Option<(Bytes, VerbDef)>, WorldStateError>;

    /// Get the object that is the parent of the given object.
    fn parent_of(&self, perms: Objid, obj: Objid) -> Result<Objid, WorldStateError>;

    /// Change the parent of the given object.
    /// This manages the movement of property definitions between the old and new parents.
    fn change_parent(
        &mut self,
        perms: Objid,
        obj: Objid,
        new_parent: Objid,
    ) -> Result<(), WorldStateError>;

    /// Get the children of the given object.
    fn children_of(&self, perms: Objid, obj: Objid) -> Result<ObjSet, WorldStateError>;

    /// Check the validity of an object.
    fn valid(&self, obj: Objid) -> Result<bool, WorldStateError>;

    /// Get the name & aliases of an object.
    fn names_of(&self, perms: Objid, obj: Objid) -> Result<(String, Vec<String>), WorldStateError>;

    /// Returns the (rough) total number of bytes used by database storage subsystem.
    fn db_usage(&self) -> Result<usize, WorldStateError>;

    /// Commit all modifications made to the state of this world since the start of its transaction.
    fn commit(&mut self) -> Result<CommitResult, WorldStateError>;

    /// Rollback all modifications made to the state of this world since the start of its transaction.
    fn rollback(&mut self) -> Result<(), WorldStateError>;
}

pub trait WorldStateSource: Send {
    /// Create a new world state for the given player.
    /// Returns the world state, and a permissions context for the player.
    fn new_world_state(&self) -> Result<Box<dyn WorldState>, WorldStateError>;

    /// Synchronize any in-memory state with the backing store.
    /// e.g. sequences
    fn checkpoint(&self) -> Result<(), WorldStateError>;
}
