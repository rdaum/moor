// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

use thiserror::Error;
use uuid::Uuid;

use crate::{
    model::{
        CommitResult, ObjectRef, PropPerms, Vid,
        r#match::{ArgSpec, PrepSpec, VerbArgsSpec},
        objects::ObjFlag,
        objset::ObjSet,
        propdef::{PropDef, PropDefs},
        props::{PropAttrs, PropFlag},
        verbdef::{ResolvedVerb, VerbDef, VerbDefs},
        verbs::{VerbAttrs, VerbFlag},
    },
    util::{BitEnum, PerfCounter},
};
use moor_var::{
    E_INVARG, E_INVIND, E_PERM, E_PROPNF, E_RECMOVE, E_TYPE, E_VERBNF, Error, Obj, Symbol, Var,
    program::ProgramType,
};

/// Specifies the way the object ID should be allocated when creating a new object.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ObjectKind {
    /// Create an object with a specific numeric ID (for create_at).
    Objid(Obj),
    /// Create an object with the next available numeric ID (for create() when UUID feature is off).
    NextObjid,
    /// Create an object with a random generated UUID (for create() when UUID feature is on).
    UuObjId,
    /// Create an anonymous object with a generated anonymous ID (for create() with anonymous objects).
    Anonymous,
}

/// Controls which object's flags should be returned for dispatch activation setup.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DispatchFlagsSource {
    /// Return flags for the `perms` object passed into lookup.
    Permissions,
    /// Return flags for the resolved verb owner.
    VerbOwner,
}

/// Canonical verb lookup request for both method and command dispatch.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct VerbLookup<'a> {
    pub object: &'a Obj,
    pub verb_name: Symbol,
    pub argspec: Option<VerbArgsSpec>,
    pub flagspec: Option<BitEnum<VerbFlag>>,
}

impl<'a> VerbLookup<'a> {
    #[must_use]
    pub fn method(object: &'a Obj, verb_name: Symbol) -> Self {
        Self {
            object,
            verb_name,
            argspec: None,
            flagspec: Some(BitEnum::new_with(VerbFlag::Exec)),
        }
    }

    #[must_use]
    pub fn command(object: &'a Obj, verb_name: Symbol, argspec: VerbArgsSpec) -> Self {
        Self {
            object,
            verb_name,
            argspec: Some(argspec),
            flagspec: None,
        }
    }
}

/// Command-argument matcher for lookup against a specific receiver object.
#[must_use]
pub fn command_verb_argspec(
    receiver: &Obj,
    dobj: &Obj,
    prep: PrepSpec,
    iobj: &Obj,
) -> VerbArgsSpec {
    let spec_for_target = |target: &Obj| -> ArgSpec {
        if target == receiver {
            ArgSpec::This
        } else if target.is_nothing() {
            ArgSpec::None
        } else {
            ArgSpec::Any
        }
    };
    VerbArgsSpec {
        dobj: spec_for_target(dobj),
        prep,
        iobj: spec_for_target(iobj),
    }
}

/// Monomorphic property-lookup hint for call-site hint fast paths.
///
/// This hint is valid only when all guard fields match, including the
/// property-resolution cache version captured at fill time.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct PropertyLookupHint {
    pub receiver: Obj,
    pub property_name: Symbol,
    pub property_uuid: Uuid,
    pub prop_cache_version: u64,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PropertyLookupPicOutcome {
    NotApplicable,
    Hit,
    MissNoHint,
    MissGuardMismatch,
    MissVersionMismatch,
    MissResolveFailed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PropertyLookupResult {
    pub value: Var,
    pub next_hint: Option<PropertyLookupHint>,
    pub pic_outcome: PropertyLookupPicOutcome,
}

/// Monomorphic verb-lookup hint for call-site hint fast paths.
///
/// This hint is valid only when all guard fields match, including the
/// verb-resolution cache version captured at fill time.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct VerbLookupHint {
    pub receiver: Obj,
    pub verb_name: Symbol,
    pub verb_definer: Obj,
    pub verb_uuid: Uuid,
    pub verb_cache_version: u64,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct VerbProgramKey {
    pub verb_definer: Obj,
    pub verb_uuid: Uuid,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum VerbLookupPicOutcome {
    NotApplicable,
    Hit,
    MissNoHint,
    MissGuardMismatch,
    MissVersionMismatch,
    MissResolveFailed,
}

#[derive(Debug, Clone)]
pub struct VerbDispatchResult {
    pub program_key: VerbProgramKey,
    pub verbdef: ResolvedVerb,
    pub permissions_flags: BitEnum<ObjFlag>,
    pub next_hint: Option<VerbLookupHint>,
    pub pic_outcome: VerbLookupPicOutcome,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct VerbDispatch<'a> {
    pub lookup: VerbLookup<'a>,
    pub flags_source: DispatchFlagsSource,
    pub hint: Option<VerbLookupHint>,
}

impl<'a> VerbDispatch<'a> {
    #[must_use]
    pub fn new(lookup: VerbLookup<'a>, flags_source: DispatchFlagsSource) -> Self {
        Self {
            lookup,
            flags_source,
            hint: None,
        }
    }

    #[must_use]
    pub fn with_hint(mut self, hint: Option<VerbLookupHint>) -> Self {
        self.hint = hint;
        self
    }
}

/// Errors related to the world state and operations on it.
#[derive(Error, Debug, Eq, PartialEq, Clone)]
pub enum WorldStateError {
    #[error("Object not found: {0}")]
    ObjectNotFound(ObjectRef),
    #[error("Object already exists: {0}")]
    ObjectAlreadyExists(Obj),
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
    #[error("Property name conflict: {0}-or-descendants and {1}-or-ancestors both define {2}")]
    ChparentPropertyNameConflict(Obj, Obj, String),
    #[error("Property type mismatch")]
    PropertyTypeMismatch,
    #[error("Cannot clear property on defining object: {0}.{1}")]
    CannotClearPropertyOnDefiner(Obj, String),

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

    #[error("Invalid renumber: {0}")]
    InvalidRenumber(String),

    // Catch-alls for system level object DB errors.
    #[error("DB communications/internal error: {0}")]
    DatabaseError(String),

    /// A rollback was requested, and the caller should retry the operation.
    #[error("Rollback requested, retry operation")]
    RollbackRetry,
}

/// Translations from WorldStateError to MOO error codes.
impl WorldStateError {
    pub fn to_error(&self) -> Error {
        match self {
            Self::ObjectNotFound(x) => E_INVIND.with_msg(|| format!("Object {x} not found")),
            Self::ObjectAlreadyExists(obj) => E_PERM.with_msg(|| format!("Object {obj} already exists")),
            Self::ObjectPermissionDenied => E_PERM.with_msg(|| "Object permission denied".to_string()),
            Self::VerbPermissionDenied => E_PERM.with_msg(|| "Verb permission denied".to_string()),
            Self::PropertyPermissionDenied => E_PERM.with_msg(|| "Property permission denied".to_string()),
            Self::RecursiveMove(from, to) => E_RECMOVE.with_msg(|| format!("Recursive move detected: {from} -> {to}")),
            Self::VerbNotFound(obj, verb) => E_VERBNF.with_msg(|| format!("Verb not found: {obj}:{verb}")),
            Self::InvalidVerb(vid) => E_VERBNF.with_msg(|| format!("Invalid verb: {vid:?}")),
            Self::VerbDecodeError(obj, verb) => E_VERBNF.with_msg(|| format!("Invalid verb, decode error: {obj}:{verb}")),
            Self::DuplicateVerb(obj, verb) => E_INVARG.with_msg(|| format!("Verb already exists: {obj}:{verb}")),
            Self::DuplicatePropertyDefinition(obj, prop) => E_INVARG.with_msg(|| format!("Duplicate property definition: {obj}.{prop}")),
            Self::ChparentPropertyNameConflict(obj1, obj2, prop) => E_INVARG.with_msg(|| format!("Property name conflict: {obj1}-or-descendants and {obj2}-or-ancestors both define {prop}")),
            Self::PropertyNotFound(obj, prop) => E_PROPNF.with_msg(|| format!("Property not found: {obj}.{prop}")),
            Self::PropertyDefinitionNotFound(obj, prop) => E_PROPNF.with_msg(|| format!("Property definition not found: {obj}.{prop}")),
            Self::PropertyTypeMismatch => E_TYPE.with_msg(|| "Property type mismatch".to_string()),
            Self::CannotClearPropertyOnDefiner(obj, prop) => E_INVARG.with_msg(|| format!("Cannot clear property on defining object: {obj}.{prop}")),
            Self::FailedMatch(msg) => E_INVARG.with_msg(|| format!("Failed object match: {msg}")),
            Self::AmbiguousMatch(msg) => E_INVARG.with_msg(|| format!("Ambiguous object match: {msg}")),
            Self::InvalidRenumber(msg) => E_INVARG.with_msg(|| msg.clone()),
            _ => panic!("Unhandled error code: {self:?}"),
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
        val.to_error()
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

    /// Get the set of all valid objects in the world.
    fn all_objects(&self) -> Result<ObjSet, WorldStateError>;

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

    /// Create a new object with the specified object ID kind.
    /// If owner is #-1, the object's is set to itself.
    /// Note it is the caller's responsibility to execute :initialize).
    fn create_object(
        &mut self,
        perms: &Obj,
        parent: &Obj,
        owner: &Obj,
        flags: BitEnum<ObjFlag>,
        id_kind: ObjectKind,
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

    /// Retrieve a property value with an optional call-site hint.
    ///
    /// Implementations may use the hint to avoid full name resolution when
    /// guards match. On success, returns the property value plus an optional
    /// refreshed hint for future calls.
    #[inline]
    fn retrieve_property_with_hint(
        &self,
        perms: &Obj,
        obj: &Obj,
        pname: Symbol,
        _hint: Option<PropertyLookupHint>,
    ) -> Result<PropertyLookupResult, WorldStateError> {
        let value = self.retrieve_property(perms, obj, pname)?;
        Ok(PropertyLookupResult {
            value,
            next_hint: None,
            pic_outcome: PropertyLookupPicOutcome::NotApplicable,
        })
    }

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

    /// Update a property value with an optional call-site hint.
    ///
    /// Implementations may use the hint to avoid full name resolution when
    /// guards match. On success, returns an optional refreshed hint for
    /// future calls.
    #[inline]
    fn update_property_with_hint(
        &mut self,
        perms: &Obj,
        obj: &Obj,
        pname: Symbol,
        value: &Var,
        _hint: Option<PropertyLookupHint>,
    ) -> Result<Option<PropertyLookupHint>, WorldStateError> {
        self.update_property(perms, obj, pname, value)?;
        Ok(None)
    }

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
        program: ProgramType,
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
    ) -> Result<(ProgramType, VerbDef), WorldStateError>;

    /// Resolve verb metadata (with inheritance) for a canonical lookup request.
    ///
    /// Returns `Ok(None)` when the verb is not found or the receiver object is invalid.
    fn lookup_verb(
        &self,
        perms: &Obj,
        lookup: VerbLookup<'_>,
    ) -> Result<Option<VerbDef>, WorldStateError>;

    /// Resolve a dispatch-ready verb (program + resolved metadata + activation flags).
    ///
    /// Returns `Ok(None)` when the verb is not found or the receiver object is invalid.
    /// Implementations may honor `dispatch.hint` for fast-path lookups and return an
    /// updated hint/pic outcome.
    fn dispatch_verb(
        &self,
        perms: &Obj,
        dispatch: VerbDispatch<'_>,
    ) -> Result<Option<VerbDispatchResult>, WorldStateError>;

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

    /// Get all objects owned by the given object.
    fn owned_objects(&self, perms: &Obj, owner: &Obj) -> Result<ObjSet, WorldStateError>;

    /// Get the full descendant tree of the given object.
    fn descendants_of(
        &self,
        perms: &Obj,
        obj: &Obj,
        include_self: bool,
    ) -> Result<ObjSet, WorldStateError>;

    /// Get the list of ancestors of the given object (parent + parent-parents)
    fn ancestors_of(
        &self,
        perms: &Obj,
        obj: &Obj,
        include_self: bool,
    ) -> Result<ObjSet, WorldStateError>;

    /// Check the validity of an object.
    fn valid(&self, obj: &Obj) -> Result<bool, WorldStateError>;

    /// Get just the name of a given object.
    fn name_of(&self, perms: &Obj, obj: &Obj) -> Result<String, WorldStateError>;

    /// Get the name & aliases of an object.
    fn names_of(&self, perms: &Obj, obj: &Obj) -> Result<(String, Vec<String>), WorldStateError>;

    /// Returns the (rough) total number of bytes used by database storage subsystem.
    fn db_usage(&self) -> Result<usize, WorldStateError>;

    /// Increment the given sequence, return the new value.
    fn increment_sequence(&self, seq: usize) -> i64;

    /// Renumber an object to a new object ID. Supports numbered and UUID objects
    /// as both source and target; anonymous objects are not supported.
    ///
    /// If target is None:
    /// - For numbered objects: finds lowest available object number below current
    /// - For UUID objects: finds lowest available numbered object ID
    ///
    /// If target is Some(kind):
    /// - ObjectKind::Objid(num): renumber to a specific numeric object ID
    /// - ObjectKind::NextObjid: renumber to next available numeric ID (max + 1)
    /// - ObjectKind::UuObjId: renumber to a newly generated UUID
    ///
    /// Updates structural database relationships (parent/child, location/contents, ownership)
    /// but does not rewrite object references in verb code or property values.
    /// Returns the new object ID.
    fn renumber_object(
        &mut self,
        perms: &Obj,
        obj: &Obj,
        target: Option<ObjectKind>,
    ) -> Result<Obj, WorldStateError>;

    /// Flush all internal caches (verb resolution, property resolution, ancestry).
    /// This is useful when you want to ensure that subsequent queries see fresh data.
    fn flush_caches(&mut self);

    /// Commit all modifications made to the state of this world since the start of its transaction.
    fn commit(self: Box<Self>) -> Result<CommitResult, WorldStateError>;

    /// Rollback all modifications made to the state of this world since the start of its transaction.
    fn rollback(self: Box<Self>) -> Result<(), WorldStateError>;

    /// Convert this WorldState to a LoaderInterface using the same underlying transaction.
    /// This allows using loader operations (which bypass permissions) on the same transaction.
    /// Returns an error if the implementation doesn't support this conversion.
    fn as_loader_interface(
        self: Box<Self>,
    ) -> Result<Box<dyn crate::model::loader::LoaderInterface>, WorldStateError> {
        Err(WorldStateError::DatabaseError(
            "This WorldState implementation does not support loader interface conversion"
                .to_string(),
        ))
    }
}

pub trait WorldStateSource: Send {
    /// Create a new world state for the given player.
    /// Returns the world state, and a permissions context for the player.
    fn new_world_state(&self) -> Result<Box<dyn WorldState>, WorldStateError>;

    /// Synchronize any in-memory state with the backing store.
    /// e.g. sequences
    fn checkpoint(&self) -> Result<(), WorldStateError>;
}

pub struct WorldStatePerf {
    pub players: PerfCounter,
    pub set_flags_of: PerfCounter,
    pub object_bytes: PerfCounter,
    pub create_object: PerfCounter,
    pub recycle_object: PerfCounter,
    pub move_object: PerfCounter,
    pub verbs: PerfCounter,
    pub retrieve_property: PerfCounter,
    pub get_property_info: PerfCounter,
    pub set_property_info: PerfCounter,
    pub update_property: PerfCounter,
    pub clear_property: PerfCounter,
    pub define_property: PerfCounter,
    pub delete_property: PerfCounter,
    pub add_verb: PerfCounter,
    pub remove_verb: PerfCounter,
    pub update_verb: PerfCounter,
    pub update_verb_at_index: PerfCounter,
    pub update_verb_with_id: PerfCounter,
    pub get_verb: PerfCounter,
    pub get_verb_at_index: PerfCounter,
    pub retrieve_verb: PerfCounter,
    pub lookup_verb: PerfCounter,
    pub dispatch_verb: PerfCounter,
    pub change_parent: PerfCounter,
    pub children_of: PerfCounter,
    pub owned_objects: PerfCounter,
    pub descendants_of: PerfCounter,
    pub ancestors_of: PerfCounter,
    pub db_usage: PerfCounter,
    pub commit: PerfCounter,
    pub rollback: PerfCounter,
    pub commit_success: PerfCounter,
    pub commit_success_readonly: PerfCounter,
    pub commit_success_write: PerfCounter,
    pub commit_conflict: PerfCounter,

    pub commit_check_phase: PerfCounter,
    pub commit_apply_phase: PerfCounter,

    pub apply_index_insert: PerfCounter,

    pub commit_prepare_working_set_phase: PerfCounter,
    pub commit_wait_phase: PerfCounter,
    pub commit_lock_wait_phase: PerfCounter,
    pub commit_process_phase: PerfCounter,

    pub provider_tuple_load: PerfCounter,
    pub provider_tuple_check: PerfCounter,
    pub crdt_resolve_success: PerfCounter,
    pub crdt_resolve_fail: PerfCounter,
}

impl Default for WorldStatePerf {
    fn default() -> Self {
        Self::new()
    }
}

impl WorldStatePerf {
    pub fn new() -> Self {
        Self {
            players: PerfCounter::new("players"),
            set_flags_of: PerfCounter::new("set_flags_of"),
            object_bytes: PerfCounter::new("object_bytes"),
            create_object: PerfCounter::new("create_object"),
            recycle_object: PerfCounter::new("recycle_object"),
            move_object: PerfCounter::new("move_object"),
            verbs: PerfCounter::new("verbs"),
            retrieve_property: PerfCounter::new("retrieve_property"),
            get_property_info: PerfCounter::new("get_property_info"),
            set_property_info: PerfCounter::new("set_property_info"),
            update_property: PerfCounter::new("update_property"),
            clear_property: PerfCounter::new("clear_property"),
            define_property: PerfCounter::new("define_property"),
            delete_property: PerfCounter::new("delete_property"),
            add_verb: PerfCounter::new("add_verb"),
            remove_verb: PerfCounter::new("remove_verb"),
            update_verb: PerfCounter::new("update_verb"),
            update_verb_at_index: PerfCounter::new("update_verb_at_index"),
            update_verb_with_id: PerfCounter::new("update_verb_with_id"),
            get_verb: PerfCounter::new("get_verb"),
            get_verb_at_index: PerfCounter::new("get_verb_at_index"),
            retrieve_verb: PerfCounter::new("retrieve_verb"),
            lookup_verb: PerfCounter::new("lookup_verb"),
            dispatch_verb: PerfCounter::new("dispatch_verb"),
            change_parent: PerfCounter::new("change_parent"),
            children_of: PerfCounter::new("children_of"),
            owned_objects: PerfCounter::new("owned_objects"),
            descendants_of: PerfCounter::new("descendants_of"),
            ancestors_of: PerfCounter::new("ancestors_of"),
            db_usage: PerfCounter::new("db_usage"),
            commit: PerfCounter::new("commit"),
            rollback: PerfCounter::new("rollback"),
            commit_success: PerfCounter::new("commit_success"),
            commit_success_readonly: PerfCounter::new("commit_success_readonly"),
            commit_success_write: PerfCounter::new("commit_success_write"),
            commit_conflict: PerfCounter::new("commit_conflict"),
            commit_check_phase: PerfCounter::new("commit_check_phase"),
            commit_apply_phase: PerfCounter::new("commit_apply_phase"),
            apply_index_insert: PerfCounter::new("apply_index_insert"),
            commit_prepare_working_set_phase: PerfCounter::new("commit_prepare_working_set_phase"),
            commit_wait_phase: PerfCounter::new("commit_wait_phase"),
            commit_lock_wait_phase: PerfCounter::new("commit_lock_wait_phase"),
            commit_process_phase: PerfCounter::new("commit_process_phase"),

            provider_tuple_load: PerfCounter::new("provider_tuple_load"),
            provider_tuple_check: PerfCounter::new("provider_tuple_check"),
            crdt_resolve_success: PerfCounter::new("crdt_resolve_success"),
            crdt_resolve_fail: PerfCounter::new("crdt_resolve_fail"),
        }
    }

    pub fn all_counters(&self) -> Vec<&PerfCounter> {
        vec![
            &self.players,
            &self.set_flags_of,
            &self.object_bytes,
            &self.create_object,
            &self.recycle_object,
            &self.move_object,
            &self.verbs,
            &self.retrieve_property,
            &self.get_property_info,
            &self.set_property_info,
            &self.update_property,
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
            &self.lookup_verb,
            &self.dispatch_verb,
            &self.change_parent,
            &self.children_of,
            &self.owned_objects,
            &self.descendants_of,
            &self.ancestors_of,
            &self.db_usage,
            &self.commit,
            &self.rollback,
            &self.commit_success,
            &self.commit_success_readonly,
            &self.commit_success_write,
            &self.commit_conflict,
            &self.commit_check_phase,
            &self.commit_apply_phase,
            &self.apply_index_insert,
            &self.commit_prepare_working_set_phase,
            &self.commit_wait_phase,
            &self.commit_lock_wait_phase,
            &self.commit_process_phase,
            &self.provider_tuple_load,
            &self.provider_tuple_check,
            &self.crdt_resolve_success,
            &self.crdt_resolve_fail,
        ]
    }
}
