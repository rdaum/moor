// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

use fast_telemetry::{DeriveLabel, ExportMetrics, LabeledCounter, LabeledSampledTimer};
use std::time::Duration;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    builtins::BUILTIN_ID_SPACE,
    model::{
        CommitResult, ObjectRef, PropPerms, TaskPermissions, Vid,
        r#match::{ArgSpec, PrepSpec, VerbArgsSpec},
        objects::{ObjFlag, ObjectQuery},
        objset::ObjSet,
        propdef::{PropDef, PropDefs},
        props::{PropAttrs, PropFlag},
        verbdef::{ResolvedVerb, VerbDef, VerbDefs},
        verbs::{VerbAttrs, VerbFlag},
    },
    util::{BitEnum, hot_stride, rare_stride},
};
use moor_var::{
    E_INVARG, E_INVIND, E_PERM, E_PROPNF, E_QUOTA, E_RECMOVE, E_TYPE, E_VERBNF, Error, Obj, Symbol,
    Var,
    program::{ProgramType, opcode::BuiltinId},
};

pub const BUILTIN_PROXY_CACHE_WORDS: usize = BUILTIN_ID_SPACE.div_ceil(u64::BITS as usize);
pub type BuiltinProxyCacheBits = [u64; BUILTIN_PROXY_CACHE_WORDS];

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
    /// Return flags for the `permissions` object passed into lookup.
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

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct VerbProgramKey {
    pub verb_definer: Obj,
    pub verb_uuid: Uuid,
}

#[derive(Debug, Clone)]
pub struct VerbDispatchResult {
    pub program_key: VerbProgramKey,
    pub verbdef: ResolvedVerb,
    pub permissions_flags: BitEnum<ObjFlag>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct VerbDispatch<'a> {
    pub lookup: VerbLookup<'a>,
    pub flags_source: DispatchFlagsSource,
}

impl<'a> VerbDispatch<'a> {
    #[must_use]
    pub fn new(lookup: VerbLookup<'a>, flags_source: DispatchFlagsSource) -> Self {
        Self {
            lookup,
            flags_source,
        }
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

    #[error("Database commit queue remained full for {0:?}")]
    DatabaseOverloaded(Duration),

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
            Self::DatabaseOverloaded(waited) => E_QUOTA.with_msg(|| {
                format!(
                    "Database writer remained overloaded for {waited:?}. The transaction was not committed."
                )
            }),
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
    // TODO: Combine worldstate owner & flags check into one call, to make authority checks more efficient.

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
        permissions: &TaskPermissions,
        obj: &Obj,
        flags: BitEnum<ObjFlag>,
    ) -> Result<(), WorldStateError>;

    /// Get the location of the given object.
    fn location_of(&self, permissions: &TaskPermissions, obj: &Obj)
    -> Result<Obj, WorldStateError>;

    /// Return the number of bytes used by the given object and all its attributes.
    fn object_bytes(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
    ) -> Result<usize, WorldStateError>;

    /// Create a new object with the specified object ID kind.
    /// If owner is #-1, the object's is set to itself.
    /// Note it is the caller's responsibility to execute :initialize).
    fn create_object(
        &mut self,
        permissions: &TaskPermissions,
        parent: &Obj,
        owner: &Obj,
        flags: BitEnum<ObjFlag>,
        id_kind: ObjectKind,
    ) -> Result<Obj, WorldStateError>;

    /// Recycles (destroys) the given object, and re-parents all its children to the next parent up
    /// the chain, including removing property definitions inherited from the object.
    /// If the object is a location, the contents of that location are moved to #-1.
    /// (It is the caller's (bf_recycle) responsibility to execute :exitfunc for those objects).
    fn check_recycle_object(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
    ) -> Result<(), WorldStateError>;

    fn recycle_object(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
    ) -> Result<(), WorldStateError>;

    /// Return the highest used object # in the system.
    fn max_object(&self, permissions: &TaskPermissions) -> Result<Obj, WorldStateError>;

    /// Move an object to a new location.
    /// (Note it is the caller's responsibility to execute :accept, :enterfunc, :exitfunc, etc.)
    fn move_object(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        new_loc: &Obj,
    ) -> Result<(), WorldStateError>;

    /// Get the contents of a given object.
    fn contents_of(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
    ) -> Result<ObjSet, WorldStateError>;

    /// Get the names of all the verbs on the given object.
    fn verbs(&self, permissions: &TaskPermissions, obj: &Obj) -> Result<VerbDefs, WorldStateError>;

    /// Gets a list of the names of the properties defined directly on the given object, not
    /// inherited from its parent.
    fn properties(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
    ) -> Result<PropDefs, WorldStateError>;

    /// Retrieve a property from the given object, walking transitively up its inheritance chain.
    fn retrieve_property(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
        pname: Symbol,
    ) -> Result<Var, WorldStateError>;

    /// Get information about a property, walking the inheritance tree to find the definition.
    /// Returns the PropDef as well as the owner of the property.
    fn get_property_info(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
        pname: Symbol,
    ) -> Result<(PropDef, PropPerms), WorldStateError>;

    /// Read local value, permission-row presence, and metadata without resolving inherited values.
    /// Resolves the UUID in this object's ancestry and requires property read permission using
    /// its effective permissions. Missing bindings and denied access are errors, never clear state.
    fn snapshot_property(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
        uuid: Uuid,
    ) -> Result<crate::model::PropertySnapshot, WorldStateError>;

    /// Change the property info for the given property.
    fn set_property_info(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        pname: Symbol,
        attrs: PropAttrs,
    ) -> Result<(), WorldStateError>;

    fn object_metadata(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
    ) -> Result<Vec<(Symbol, Var)>, WorldStateError>;

    fn get_object_metadata(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
        key: Symbol,
    ) -> Result<Option<Var>, WorldStateError>;

    fn set_object_metadata(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        key: Symbol,
        value: Var,
    ) -> Result<(), WorldStateError>;

    fn clear_object_metadata(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        key: Symbol,
    ) -> Result<(), WorldStateError>;

    fn property_metadata(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
        pname: Symbol,
    ) -> Result<Vec<(Symbol, Var)>, WorldStateError>;

    fn get_property_metadata(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
        pname: Symbol,
        key: Symbol,
    ) -> Result<Option<Var>, WorldStateError>;

    fn set_property_metadata(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        pname: Symbol,
        key: Symbol,
        value: Var,
    ) -> Result<(), WorldStateError>;

    fn clear_property_metadata(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        pname: Symbol,
        key: Symbol,
    ) -> Result<(), WorldStateError>;

    /// Update a property on the given object.
    fn update_property(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        pname: Symbol,
        value: &Var,
    ) -> Result<(), WorldStateError>;

    /// Check if a property is 'clear' (value is purely inherited)
    fn is_property_clear(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
        pname: Symbol,
    ) -> Result<bool, WorldStateError>;

    /// Clear a property on the given object. That is, remove its local value, if any, and
    /// ensure that it is purely inherited.
    fn clear_property(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        pname: Symbol,
    ) -> Result<(), WorldStateError>;

    /// Add a property for the given object.
    // Yes yes I know it's a lot of arguments, but wrapper object here is redundant.
    #[allow(clippy::too_many_arguments)]
    fn define_property(
        &mut self,
        permissions: &TaskPermissions,
        definer: &Obj,
        location: &Obj,
        pname: Symbol,
        owner: &Obj,
        prop_flags: BitEnum<PropFlag>,
        initial_value: Option<Var>,
    ) -> Result<(), WorldStateError>;

    fn delete_property(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        pname: Symbol,
    ) -> Result<(), WorldStateError>;

    /// Add a verb to the given object.
    // Yes yes I know it's a lot of arguments, but wrapper object here is redundant.
    #[allow(clippy::too_many_arguments)]
    fn add_verb(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        names: Vec<Symbol>,
        owner: &Obj,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
        program: ProgramType,
    ) -> Result<(), WorldStateError>;

    /// Remove a verb from the given object.
    fn remove_verb(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        verb: Var,
    ) -> Result<(), WorldStateError>;

    /// Update data about a verb on the given object.
    fn update_verb(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        vname: Symbol,
        verb_attrs: VerbAttrs,
    ) -> Result<(), WorldStateError>;

    /// Update data about a verb on the given object at a numbered offset.
    fn update_verb_at_index(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        vidx: usize,
        verb_attrs: VerbAttrs,
    ) -> Result<(), WorldStateError>;

    fn update_verb_with_id(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        uuid: Uuid,
        verb_attrs: VerbAttrs,
    ) -> Result<(), WorldStateError>;

    fn verb_metadata(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
        uuid: Uuid,
    ) -> Result<Vec<(Symbol, Var)>, WorldStateError>;

    fn get_verb_metadata(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
        uuid: Uuid,
        key: Symbol,
    ) -> Result<Option<Var>, WorldStateError>;

    fn set_verb_metadata(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        uuid: Uuid,
        key: Symbol,
        value: Var,
    ) -> Result<(), WorldStateError>;

    fn clear_verb_metadata(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        uuid: Uuid,
        key: Symbol,
    ) -> Result<(), WorldStateError>;

    /// Get the verbdef with the given name on the given object. Without doing inheritance resolution.
    fn get_verb(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
        vname: Symbol,
    ) -> Result<VerbDef, WorldStateError>;

    /// Get the verbdef at numbered offset on the given object.
    fn get_verb_at_index(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
        vidx: usize,
    ) -> Result<VerbDef, WorldStateError>;

    /// Get verb metadata by stable verb UUID.
    fn get_verb_by_id(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
        uuid: Uuid,
    ) -> Result<VerbDef, WorldStateError>;

    /// Get the verb binary for the given verbdef.
    fn retrieve_verb(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
        uuid: Uuid,
    ) -> Result<(ProgramType, VerbDef), WorldStateError>;

    /// Retrieve executable verb program data after dispatch has selected a verb to run.
    ///
    /// This is separate from `retrieve_verb()`, which is source/read access. Execution
    /// materialization is authorized by verb execute permission or a matching call grant.
    fn retrieve_verb_for_execution(
        &self,
        permissions: &TaskPermissions,
        authorization_obj: &Obj,
        obj: &Obj,
        uuid: Uuid,
    ) -> Result<(ProgramType, VerbDef), WorldStateError>;

    /// Resolve verb metadata (with inheritance) for a canonical lookup request.
    ///
    /// Returns `Ok(None)` when the verb is not found or the receiver object is invalid.
    fn lookup_verb(
        &self,
        permissions: &TaskPermissions,
        lookup: VerbLookup<'_>,
    ) -> Result<Option<VerbDef>, WorldStateError>;

    /// Resolve a dispatch-ready verb (program + resolved metadata + activation flags).
    ///
    /// Returns `Ok(None)` when the verb is not found or the receiver object is invalid.
    /// Implementations may honor `dispatch.hint` for fast-path lookups and return an
    /// updated hint/pic outcome.
    fn dispatch_verb(
        &self,
        permissions: &TaskPermissions,
        dispatch: VerbDispatch<'_>,
    ) -> Result<Option<VerbDispatchResult>, WorldStateError>;

    /// Snapshot of builtins known to have no #0 bf_* proxy in this transaction.
    fn builtin_proxy_cache_snapshot(&self) -> BuiltinProxyCacheBits {
        [0; BUILTIN_PROXY_CACHE_WORDS]
    }

    /// Invalidation guard for the builtin proxy cache snapshot.
    fn builtin_proxy_cache_guard_version(&self) -> i64 {
        0
    }

    /// Remember that this builtin has no #0 bf_* proxy in this transaction.
    fn mark_builtin_proxy_absent(&mut self, _builtin: BuiltinId) {}

    /// Get the object that is the parent of the given object.
    fn parent_of(&self, permissions: &TaskPermissions, obj: &Obj) -> Result<Obj, WorldStateError>;

    /// Change the parent of the given object.
    /// This manages the movement of property definitions between the old and new parents.
    fn change_parent(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        new_parent: &Obj,
    ) -> Result<(), WorldStateError>;

    /// Get the children of the given object.
    fn children_of(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
    ) -> Result<ObjSet, WorldStateError>;

    /// Get all objects owned by the given object.
    fn owned_objects(
        &self,
        permissions: &TaskPermissions,
        owner: &Obj,
    ) -> Result<ObjSet, WorldStateError>;

    /// Get the full descendant tree of the given object.
    fn descendants_of(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
        include_self: bool,
    ) -> Result<ObjSet, WorldStateError>;

    /// Return whether `obj` is valid and descends from `possible_ancestor`, including itself.
    fn isa(&self, obj: &Obj, possible_ancestor: &Obj) -> Result<bool, WorldStateError>;

    /// Get the list of ancestors of the given object (parent + parent-parents)
    fn ancestors_of(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
        include_self: bool,
    ) -> Result<ObjSet, WorldStateError>;

    /// Query objects matching the given filter criteria.
    /// Returns all objects satisfying all specified filters (ANDed).
    fn query_objects(&self, query: &ObjectQuery) -> Result<ObjSet, WorldStateError>;

    /// Check the validity of an object.
    fn valid(&self, obj: &Obj) -> Result<bool, WorldStateError>;

    /// Get just the name of a given object.
    fn name_of(&self, permissions: &TaskPermissions, obj: &Obj) -> Result<String, WorldStateError>;

    /// Get the name & aliases of an object.
    fn names_of(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
    ) -> Result<(String, Vec<String>), WorldStateError>;

    /// Returns the (rough) total number of bytes used by database storage subsystem.
    fn db_usage(&self) -> Result<usize, WorldStateError>;

    /// Live numeric persistence diagnostics, independent of this transaction's snapshot.
    /// Callers must restrict these operator metrics to authorized users.
    fn persistence_metrics(&self) -> Vec<(&'static str, u64)> {
        Vec::new()
    }

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
        permissions: &TaskPermissions,
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

pub trait WorldStateSource: Send + Sync {
    /// Create a new world state for the given player.
    /// Returns the world state, and a permissions context for the player.
    fn new_world_state(&self) -> Result<Box<dyn WorldState>, WorldStateError>;

    /// Synchronize any in-memory state with the backing store.
    /// e.g. sequences
    fn checkpoint(&self) -> Result<(), WorldStateError>;
}

#[derive(Copy, Clone, Debug, DeriveLabel)]
#[label_name = "op"]
pub enum WorldStateTimerOp {
    Players,
    SetFlagsOf,
    ObjectBytes,
    CreateObject,
    RecycleObject,
    MoveObject,
    Verbs,
    RetrieveProperty,
    GetPropertyInfo,
    SetPropertyInfo,
    UpdateProperty,
    ClearProperty,
    DefineProperty,
    DeleteProperty,
    AddVerb,
    RemoveVerb,
    UpdateVerb,
    UpdateVerbAtIndex,
    UpdateVerbWithId,
    GetVerb,
    GetVerbAtIndex,
    RetrieveVerb,
    LookupVerb,
    DispatchVerb,
    ChangeParent,
    ChildrenOf,
    OwnedObjects,
    DescendantsOf,
    Isa,
    AncestorsOf,
    DbUsage,
    Commit,
    Rollback,
    CommitSuccess,
    CommitSuccessReadonly,
    CommitSuccessWrite,
    CommitConflict,
    CommitCheckPhase,
    CommitApplyPhase,
    ApplyIndexInsert,
    CommitPrepareWorkingSetPhase,
    CommitWaitPhase,
    CommitProcessPhase,
    ProviderTupleLoad,
    ProviderTupleCheck,
    ProviderPendingOpsReadLockWait,
    ProviderPendingOpsWriteLockWait,
    PostgresEncode,
    PostgresApply,
    PostgresCommit,
    PostgresFence,
    PostgresRecovery,
    BatchWriterEncode,
    BatchWriterCommit,
    BatchWriterBackpressureBlock,
    BatchWriterDurableFlush,
    PropertyListAppendClassify,
    PropertyValueRollupEncode,
    PropertyValueReconstruct,
}

#[derive(Copy, Clone, Debug, DeriveLabel)]
#[label_name = "op"]
pub enum WorldStateCountOp {
    CrdtResolveSuccess,
    CrdtResolveFail,
    BatchWriterBackpressure,
    PropertyListAppendCandidate,
    PropertyListAppendAccepted,
    PropertyListAppendSuffixElements,
    PropertyListAppendSuffixBytes,
    PropertyListAppendMissingBase,
    PropertyListAppendNonList,
    PropertyListAppendNotLonger,
    PropertyListAppendPrefixMismatch,
    PropertyListAppendComparisonBudget,
    PropertyValueCompleteReplacement,
    PropertyValueFullEncodedBytes,
    PropertyValueAppendEncodedBytes,
    PropertyValueForegroundRollup,
    PropertyValueReconstructedRecords,
}

const WS_SHARD_COUNT: usize = 16;

#[derive(ExportMetrics)]
#[metric_prefix = "db"]
pub struct WorldStatePerf {
    #[help = "Hot-path world-state operation latency"]
    pub timers_hot: LabeledSampledTimer<WorldStateTimerOp>,
    #[help = "Rare-path world-state operation latency"]
    pub timers_rare: LabeledSampledTimer<WorldStateTimerOp>,
    #[help = "World-state operation counters"]
    pub counters: LabeledCounter<WorldStateCountOp>,
}

impl Default for WorldStatePerf {
    fn default() -> Self {
        Self::new()
    }
}

impl WorldStatePerf {
    pub fn new() -> Self {
        Self {
            timers_hot: LabeledSampledTimer::with_latency_buckets(WS_SHARD_COUNT, hot_stride()),
            timers_rare: LabeledSampledTimer::with_latency_buckets(WS_SHARD_COUNT, rare_stride()),
            counters: LabeledCounter::new(WS_SHARD_COUNT),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_queue_overload_maps_to_quota_error() {
        let error = WorldStateError::DatabaseOverloaded(Duration::from_secs(5)).to_error();

        assert_eq!(error.err_type(), E_QUOTA);
        assert!(error.to_string().contains("transaction was not committed"));
    }
}
