// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use bincode::{Decode, Encode};
use byteview::ByteView;
use fast_counter::ConcurrentCounter;
use std::sync::Arc;
use std::time::Instant;
use thiserror::Error;
use uuid::Uuid;

use crate::model::r#match::{PrepSpec, VerbArgsSpec};
use crate::model::objects::ObjFlag;
use crate::model::objset::ObjSet;
use crate::model::propdef::{PropDef, PropDefs};
use crate::model::props::{PropAttrs, PropFlag};
use crate::model::verbdef::{VerbDef, VerbDefs};
use crate::model::verbs::{BinaryType, VerbAttrs, VerbFlag};
use crate::model::{CommitResult, ObjectRef, PropPerms};
use crate::model::{ObjAttr, Vid};
use crate::util::BitEnum;
use moor_var::Symbol;
use moor_var::Var;
use moor_var::{Error, Obj};

/// Errors related to the world state and operations on it.
#[derive(Error, Debug, Eq, PartialEq, Clone, Decode, Encode)]
pub enum WorldStateError {
    #[error("Object not found: {0}")]
    ObjectNotFound(ObjectRef),
    #[error("Object already exists: {0}")]
    ObjectAlreadyExists(Obj),
    #[error("Could not set/get object attribute; {0} on {1}")]
    ObjectAttributeError(ObjAttr, Obj),
    #[error("Recursive move detected: {0} -> {1}")]
    RecursiveMove(Obj, Obj),

    #[error("Object permission denied")]
    ObjectPermissionDenied,

    #[error("Property not found: {0}.{1}")]
    PropertyNotFound(Obj, String),
    #[error("Property permission denied")]
    PropertyPermissionDenied,
    #[error("Property definition not found: {0}.{1}")]
    PropertyDefinitionNotFound(Obj, String),
    #[error("Duplicate property definition: {0}.{1}")]
    DuplicatePropertyDefinition(Obj, String),
    #[error("Property type mismatch")]
    PropertyTypeMismatch,

    #[error("Verb not found: {0}:{1}")]
    VerbNotFound(Obj, String),
    #[error("Verb definition not {0:?}")]
    InvalidVerb(Vid),

    #[error("Invalid verb, decode error: {0}:{1}")]
    VerbDecodeError(Obj, Symbol),
    #[error("Verb permission denied")]
    VerbPermissionDenied,
    #[error("Verb already exists: {0}:{1}")]
    DuplicateVerb(Obj, Symbol),

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
    // TODO: Combine worldstate owner & flags check into one call, to make perms check more efficient

    /// Get the set of all objects which are 'players' in the world.
    fn players(&self) -> Result<ObjSet, WorldStateError>;

    /// Get the owner of an object
    fn owner_of(&self, obj: &Obj) -> Result<Obj, WorldStateError>;

    /// Return whether the given object is controlled by the given player.
    /// (Either who is wizard, or is owner of what).
    fn controls(&self, who: &Obj, what: &Obj) -> Result<bool, WorldStateError>;

    /// Flags of an object.
    /// Note this call does not take a permission context, because it is used to *determine*
    /// permissions. It is the caller's responsibility to ensure that the program is using this
    /// call appropriately.
    fn flags_of(&self, obj: &Obj) -> Result<BitEnum<ObjFlag>, WorldStateError>;

    /// Set the flags of an object.
    fn set_flags_of(
        &mut self,
        perms: &Obj,
        obj: &Obj,
        flags: BitEnum<ObjFlag>,
    ) -> Result<(), WorldStateError>;

    /// Get the location of the given object.
    fn location_of(&self, perms: &Obj, obj: &Obj) -> Result<Obj, WorldStateError>;

    /// Return the number of bytes used by the given object and all its attributes.
    fn object_bytes(&self, perms: &Obj, obj: &Obj) -> Result<usize, WorldStateError>;

    /// Create a new object, assigning it a new unique object id.
    /// If owner is #-1, the object's is set to itself.
    /// Note it is the caller's responsibility to execute :initialize).
    fn create_object(
        &mut self,
        perms: &Obj,
        parent: &Obj,
        owner: &Obj,
        flags: BitEnum<ObjFlag>,
    ) -> Result<Obj, WorldStateError>;

    /// Recycles (destroys) the given object, and re-parents all its children to the next parent up
    /// the chain, including removing property definitions inherited from the object.
    /// If the object is a location, the contents of that location are moved to #-1.
    /// (It is the caller's (bf_recycle) responsibility to execute :exitfunc for those objects).
    fn recycle_object(&mut self, perms: &Obj, obj: &Obj) -> Result<(), WorldStateError>;

    /// Return the highest used object # in the system.
    fn max_object(&self, perms: &Obj) -> Result<Obj, WorldStateError>;

    /// Move an object to a new location.
    /// (Note it is the caller's responsibility to execute :accept, :enterfunc, :exitfunc, etc.)
    fn move_object(&mut self, perms: &Obj, obj: &Obj, new_loc: &Obj)
    -> Result<(), WorldStateError>;

    /// Get the contents of a given object.
    fn contents_of(&self, perms: &Obj, obj: &Obj) -> Result<ObjSet, WorldStateError>;

    /// Get the names of all the verbs on the given object.
    fn verbs(&self, perms: &Obj, obj: &Obj) -> Result<VerbDefs, WorldStateError>;

    /// Gets a list of the names of the properties defined directly on the given object, not
    /// inherited from its parent.
    fn properties(&self, perms: &Obj, obj: &Obj) -> Result<PropDefs, WorldStateError>;

    /// Retrieve a property from the given object, walking transitively up its inheritance chain.
    fn retrieve_property(
        &self,
        perms: &Obj,
        obj: &Obj,
        pname: Symbol,
    ) -> Result<Var, WorldStateError>;

    /// Get information about a property, walking the inheritance tree to find the definition.
    /// Returns the PropDef as well as the owner of the property.
    fn get_property_info(
        &self,
        perms: &Obj,
        obj: &Obj,
        pname: Symbol,
    ) -> Result<(PropDef, PropPerms), WorldStateError>;

    /// Change the property info for the given property.
    fn set_property_info(
        &mut self,
        perms: &Obj,
        obj: &Obj,
        pname: Symbol,
        attrs: PropAttrs,
    ) -> Result<(), WorldStateError>;

    /// Update a property on the given object.
    fn update_property(
        &mut self,
        perms: &Obj,
        obj: &Obj,
        pname: Symbol,
        value: &Var,
    ) -> Result<(), WorldStateError>;

    /// Check if a property is 'clear' (value is purely inherited)
    fn is_property_clear(
        &self,
        perms: &Obj,
        obj: &Obj,
        pname: Symbol,
    ) -> Result<bool, WorldStateError>;

    /// Clear a property on the given object. That is, remove its local value, if any, and
    /// ensure that it is purely inherited.
    fn clear_property(
        &mut self,
        perms: &Obj,
        obj: &Obj,
        pname: Symbol,
    ) -> Result<(), WorldStateError>;

    /// Add a property for the given object.
    // Yes yes I know it's a lot of arguments, but wrapper object here is redundant.
    #[allow(clippy::too_many_arguments)]
    fn define_property(
        &mut self,
        perms: &Obj,
        definer: &Obj,
        location: &Obj,
        pname: Symbol,
        owner: &Obj,
        prop_flags: BitEnum<PropFlag>,
        initial_value: Option<Var>,
    ) -> Result<(), WorldStateError>;

    fn delete_property(
        &mut self,
        perms: &Obj,
        obj: &Obj,
        pname: Symbol,
    ) -> Result<(), WorldStateError>;

    /// Add a verb to the given object.
    // Yes yes I know it's a lot of arguments, but wrapper object here is redundant.
    #[allow(clippy::too_many_arguments)]
    fn add_verb(
        &mut self,
        perms: &Obj,
        obj: &Obj,
        names: Vec<Symbol>,
        owner: &Obj,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
        binary: Vec<u8>,
        binary_type: BinaryType,
    ) -> Result<(), WorldStateError>;

    /// Remove a verb from the given object.
    fn remove_verb(&mut self, perms: &Obj, obj: &Obj, verb: Uuid) -> Result<(), WorldStateError>;

    /// Update data about a verb on the given object.
    fn update_verb(
        &mut self,
        perms: &Obj,
        obj: &Obj,
        vname: Symbol,
        verb_attrs: VerbAttrs,
    ) -> Result<(), WorldStateError>;

    /// Update data about a verb on the given object at a numbered offset.
    fn update_verb_at_index(
        &mut self,
        perms: &Obj,
        obj: &Obj,
        vidx: usize,
        verb_attrs: VerbAttrs,
    ) -> Result<(), WorldStateError>;

    fn update_verb_with_id(
        &mut self,
        perms: &Obj,
        obj: &Obj,
        uuid: Uuid,
        verb_attrs: VerbAttrs,
    ) -> Result<(), WorldStateError>;

    /// Get the verbdef with the given name on the given object. Without doing inheritance resolution.
    fn get_verb(&self, perms: &Obj, obj: &Obj, vname: Symbol) -> Result<VerbDef, WorldStateError>;

    /// Get the verbdef at numbered offset on the given object.
    fn get_verb_at_index(
        &self,
        perms: &Obj,
        obj: &Obj,
        vidx: usize,
    ) -> Result<VerbDef, WorldStateError>;

    /// Get the verb binary for the given verbdef.
    fn retrieve_verb(
        &self,
        perms: &Obj,
        obj: &Obj,
        uuid: Uuid,
    ) -> Result<(ByteView, VerbDef), WorldStateError>;

    /// Retrieve a verb/method from the given object (or its parents).
    fn find_method_verb_on(
        &self,
        perms: &Obj,
        obj: &Obj,
        vname: Symbol,
    ) -> Result<(ByteView, VerbDef), WorldStateError>;

    /// Seek the verb referenced by the given command on the given object.
    fn find_command_verb_on(
        &self,
        perms: &Obj,
        obj: &Obj,
        command_verb: Symbol,
        dobj: &Obj,
        prep: PrepSpec,
        iobj: &Obj,
    ) -> Result<Option<(ByteView, VerbDef)>, WorldStateError>;

    /// Get the object that is the parent of the given object.
    fn parent_of(&self, perms: &Obj, obj: &Obj) -> Result<Obj, WorldStateError>;

    /// Change the parent of the given object.
    /// This manages the movement of property definitions between the old and new parents.
    fn change_parent(
        &mut self,
        perms: &Obj,
        obj: &Obj,
        new_parent: &Obj,
    ) -> Result<(), WorldStateError>;

    /// Get the children of the given object.
    fn children_of(&self, perms: &Obj, obj: &Obj) -> Result<ObjSet, WorldStateError>;

    /// Get the full descendant tree of the given object.
    fn descendants_of(&self, perms: &Obj, obj: &Obj) -> Result<ObjSet, WorldStateError>;

    /// Get the list of ancestors of the given object (parent + parent-parents)
    fn ancestors_of(&self, perms: &Obj, obj: &Obj) -> Result<ObjSet, WorldStateError>;

    /// Check the validity of an object.
    fn valid(&self, obj: &Obj) -> Result<bool, WorldStateError>;

    /// Get the name & aliases of an object.
    fn names_of(&self, perms: &Obj, obj: &Obj) -> Result<(String, Vec<String>), WorldStateError>;

    /// Returns the (rough) total number of bytes used by database storage subsystem.
    fn db_usage(&self) -> Result<usize, WorldStateError>;

    /// Commit all modifications made to the state of this world since the start of its transaction.
    fn commit(self: Box<Self>) -> Result<CommitResult, WorldStateError>;

    /// Rollback all modifications made to the state of this world since the start of its transaction.
    fn rollback(self: Box<Self>) -> Result<(), WorldStateError>;

    fn perf_counters(&self) -> Arc<WorldStatePerf>;
}

pub trait WorldStateSource: Send {
    /// Create a new world state for the given player.
    /// Returns the world state, and a permissions context for the player.
    fn new_world_state(&self) -> Result<Box<dyn WorldState>, WorldStateError>;

    /// Synchronize any in-memory state with the backing store.
    /// e.g. sequences
    fn checkpoint(&self) -> Result<(), WorldStateError>;
}

pub struct WSPerfCounter {
    pub operation: Symbol,
    pub invocations: ConcurrentCounter,
    pub cumulative_duration_us: ConcurrentCounter,
}

impl WSPerfCounter {
    fn new(name: &str) -> Self {
        Self {
            operation: Symbol::mk(name),
            invocations: ConcurrentCounter::new(0),
            cumulative_duration_us: ConcurrentCounter::new(0),
        }
    }
}
pub struct WorldStatePerf {
    pub players: WSPerfCounter,
    pub owner_of: WSPerfCounter,
    pub controls: WSPerfCounter,
    pub flags_of: WSPerfCounter,
    pub set_flags_of: WSPerfCounter,
    pub location_of: WSPerfCounter,
    pub object_bytes: WSPerfCounter,
    pub create_object: WSPerfCounter,
    pub recycle_object: WSPerfCounter,
    pub max_object: WSPerfCounter,
    pub move_object: WSPerfCounter,
    pub contents_of: WSPerfCounter,
    pub verbs: WSPerfCounter,
    pub properties: WSPerfCounter,
    pub retrieve_property: WSPerfCounter,
    pub get_property_info: WSPerfCounter,
    pub set_property_info: WSPerfCounter,
    pub update_property: WSPerfCounter,
    pub is_property_clear: WSPerfCounter,
    pub clear_property: WSPerfCounter,
    pub define_property: WSPerfCounter,
    pub delete_property: WSPerfCounter,
    pub add_verb: WSPerfCounter,
    pub remove_verb: WSPerfCounter,
    pub update_verb: WSPerfCounter,
    pub update_verb_at_index: WSPerfCounter,
    pub update_verb_with_id: WSPerfCounter,
    pub get_verb: WSPerfCounter,
    pub get_verb_at_index: WSPerfCounter,
    pub retrieve_verb: WSPerfCounter,
    pub find_method_verb_on: WSPerfCounter,
    pub find_command_verb_on: WSPerfCounter,
    pub parent_of: WSPerfCounter,
    pub change_parent: WSPerfCounter,
    pub children_of: WSPerfCounter,
    pub descendants_of: WSPerfCounter,
    pub ancestors_of: WSPerfCounter,
    pub valid: WSPerfCounter,
    pub names_of: WSPerfCounter,
    pub db_usage: WSPerfCounter,
    pub commit: WSPerfCounter,
    pub rollback: WSPerfCounter,
}

impl Default for WorldStatePerf {
    fn default() -> Self {
        Self::new()
    }
}

impl WorldStatePerf {
    pub fn new() -> Self {
        Self {
            players: WSPerfCounter::new("players"),
            owner_of: WSPerfCounter::new("owner_of"),
            controls: WSPerfCounter::new("controls"),
            flags_of: WSPerfCounter::new("flags_of"),
            set_flags_of: WSPerfCounter::new("set_flags_of"),
            location_of: WSPerfCounter::new("location_of"),
            object_bytes: WSPerfCounter::new("object_bytes"),
            create_object: WSPerfCounter::new("create_object"),
            recycle_object: WSPerfCounter::new("recycle_object"),
            max_object: WSPerfCounter::new("max_object"),
            move_object: WSPerfCounter::new("move_object"),
            contents_of: WSPerfCounter::new("contents_of"),
            verbs: WSPerfCounter::new("verbs"),
            properties: WSPerfCounter::new("properties"),
            retrieve_property: WSPerfCounter::new("retrieve_property"),
            get_property_info: WSPerfCounter::new("get_property_info"),
            set_property_info: WSPerfCounter::new("set_property_info"),
            update_property: WSPerfCounter::new("update_property"),
            is_property_clear: WSPerfCounter::new("is_property_clear"),
            clear_property: WSPerfCounter::new("clear_property"),
            define_property: WSPerfCounter::new("define_property"),
            delete_property: WSPerfCounter::new("delete_property"),
            add_verb: WSPerfCounter::new("add_verb"),
            remove_verb: WSPerfCounter::new("remove_verb"),
            update_verb: WSPerfCounter::new("update_verb"),
            update_verb_at_index: WSPerfCounter::new("update_verb_at_index"),
            update_verb_with_id: WSPerfCounter::new("update_verb_with_id"),
            get_verb: WSPerfCounter::new("get_verb"),
            get_verb_at_index: WSPerfCounter::new("get_verb_at_index"),
            retrieve_verb: WSPerfCounter::new("retrieve_verb"),
            find_method_verb_on: WSPerfCounter::new("find_method_verb_on"),
            find_command_verb_on: WSPerfCounter::new("find_command_verb_on"),
            parent_of: WSPerfCounter::new("parent_of"),
            change_parent: WSPerfCounter::new("change_parent"),
            children_of: WSPerfCounter::new("children_of"),
            descendants_of: WSPerfCounter::new("descendants_of"),
            ancestors_of: WSPerfCounter::new("ancestors_of"),
            valid: WSPerfCounter::new("valid"),
            names_of: WSPerfCounter::new("names_of"),
            db_usage: WSPerfCounter::new("db_usage"),
            commit: WSPerfCounter::new("commit"),
            rollback: WSPerfCounter::new("rollback"),
        }
    }

    pub fn all_counters(&self) -> Vec<&WSPerfCounter> {
        vec![
            &self.players,
            &self.owner_of,
            &self.controls,
            &self.flags_of,
            &self.set_flags_of,
            &self.location_of,
            &self.object_bytes,
            &self.create_object,
            &self.recycle_object,
            &self.max_object,
            &self.move_object,
            &self.contents_of,
            &self.verbs,
            &self.properties,
            &self.retrieve_property,
            &self.get_property_info,
            &self.set_property_info,
            &self.update_property,
            &self.is_property_clear,
            &self.clear_property,
            &self.define_property,
            &self.delete_property,
            &self.add_verb,
            &self.remove_verb,
            &self.update_verb,
            &self.update_verb_at_index,
            &self.update_verb_with_id,
            &self.get_verb,
            &self.get_verb_at_index,
            &self.retrieve_verb,
            &self.find_method_verb_on,
            &self.find_command_verb_on,
            &self.parent_of,
            &self.change_parent,
            &self.children_of,
            &self.descendants_of,
            &self.ancestors_of,
            &self.valid,
            &self.names_of,
            &self.db_usage,
            &self.commit,
            &self.rollback,
        ]
    }
}

pub struct WsOpTimer<'a>(&'a WSPerfCounter, Instant);
impl<'a> WsOpTimer<'a> {
    pub fn new(perf: &'a WSPerfCounter) -> Self {
        Self(perf, Instant::now())
    }
}

impl Drop for WsOpTimer<'_> {
    fn drop(&mut self) {
        let elapsed = self.1.elapsed().as_micros();
        self.0.invocations.add(1);
        self.0.cumulative_duration_us.add(elapsed as isize);
    }
}
