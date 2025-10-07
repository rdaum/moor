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

use uuid::Uuid;

use crate::{
    model::{
        CommitResult, ObjAttrs, ObjFlag, ObjSet, PropDef, PropDefs, PropFlag, PropPerms,
        VerbArgsSpec, VerbDef, VerbDefs, VerbFlag, WorldStateError,
        mutations::{BatchMutationResult, MutationResult, ObjectMutation},
    },
    util::BitEnum,
};
use moor_var::{Obj, Symbol, Var, program::ProgramType};

/// Interface for read-only access to database snapshots for exporting/dumping
pub trait SnapshotInterface: Send {
    /// Get the list of all active objects in the database
    fn get_objects(&self) -> Result<ObjSet, WorldStateError>;

    /// Get the list of all players.
    fn get_players(&self) -> Result<ObjSet, WorldStateError>;

    /// Get the attributes of a given object
    fn get_object(&self, objid: &Obj) -> Result<ObjAttrs, WorldStateError>;

    /// Get the verbs living on a given object
    fn get_object_verbs(&self, objid: &Obj) -> Result<VerbDefs, WorldStateError>;

    /// Get the binary for a given verb
    fn get_verb_program(&self, objid: &Obj, uuid: Uuid) -> Result<ProgramType, WorldStateError>;

    /// Get the properties defined on a given object
    fn get_object_properties(&self, objid: &Obj) -> Result<PropDefs, WorldStateError>;

    fn get_property_value(
        &self,
        obj: &Obj,
        uuid: Uuid,
    ) -> Result<(Option<Var>, PropPerms), WorldStateError>;

    /// Returns all the property common from the root of the inheritance hierarchy down to the
    /// bottom, for the given object.
    #[allow(clippy::type_complexity)]
    fn get_all_property_values(
        &self,
        objid: &Obj,
    ) -> Result<Vec<(PropDef, (Option<Var>, PropPerms))>, WorldStateError>;

    /// Garbage collection support methods for read-only anonymous object scanning
    fn get_anonymous_object_metadata(
        &self,
        objid: &Obj,
    ) -> Result<Option<Box<dyn std::any::Any + Send>>, WorldStateError>;
    fn scan_anonymous_object_references(&self) -> Result<Vec<(Obj, Vec<Obj>)>, WorldStateError>;
}

/// Interface exposed to be used by the textdump/objdef loader for loading data into the database.
/// Overlap of functionality with what WorldState could provide, but potentially different
/// constraints/semantics (e.g. no perms checks)
pub trait LoaderInterface: Send {
    /// Create a new object with the given attributes
    fn create_object(
        &mut self,
        objid: Option<Obj>,
        attrs: &ObjAttrs,
    ) -> Result<Obj, WorldStateError>;

    /// Set the parent of an object
    fn set_object_parent(&mut self, obj: &Obj, parent: &Obj) -> Result<(), WorldStateError>;

    /// Set the location of an object
    fn set_object_location(&mut self, o: &Obj, location: &Obj) -> Result<(), WorldStateError>;

    /// Set the owner of an object
    fn set_object_owner(&mut self, obj: &Obj, owner: &Obj) -> Result<(), WorldStateError>;

    /// Add a verb to an object
    fn add_verb(
        &mut self,
        obj: &Obj,
        names: &[Symbol],
        owner: &Obj,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
        program: ProgramType,
    ) -> Result<(), WorldStateError>;

    /// Update an existing verb
    #[allow(clippy::too_many_arguments)]
    fn update_verb(
        &mut self,
        obj: &Obj,
        uuid: Uuid,
        names: &[Symbol],
        owner: &Obj,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
        program: ProgramType,
    ) -> Result<(), WorldStateError>;

    /// Define a property on an object
    fn define_property(
        &mut self,
        definer: &Obj,
        objid: &Obj,
        propname: Symbol,
        owner: &Obj,
        flags: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<(), WorldStateError>;

    /// Set property attributes and value
    fn set_property(
        &mut self,
        objid: &Obj,
        propname: Symbol,
        owner: Option<Obj>,
        flags: Option<BitEnum<PropFlag>>,
        value: Option<Var>,
    ) -> Result<(), WorldStateError>;

    /// Get the highest-numbered object in the database
    fn max_object(&self) -> Result<Obj, WorldStateError>;

    /// Commit all changes made through this loader
    fn commit(self: Box<Self>) -> Result<CommitResult, WorldStateError>;

    /// Check if an object with the given ID already exists
    fn object_exists(&self, objid: &Obj) -> Result<bool, WorldStateError>;

    /// Get the existing object attributes if the object exists
    fn get_existing_object(&self, objid: &Obj) -> Result<Option<ObjAttrs>, WorldStateError>;

    /// Get the existing verbs on an object
    fn get_existing_verbs(&self, objid: &Obj) -> Result<VerbDefs, WorldStateError>;

    /// Get the existing properties defined on an object
    fn get_existing_properties(&self, objid: &Obj) -> Result<PropDefs, WorldStateError>;

    /// Get an existing property value and permissions by property name
    fn get_existing_property_value(
        &self,
        obj: &Obj,
        propname: Symbol,
    ) -> Result<Option<(Var, PropPerms)>, WorldStateError>;

    /// Find an existing verb by its name(s)
    fn get_existing_verb_by_names(
        &self,
        obj: &Obj,
        names: &[Symbol],
    ) -> Result<Option<(Uuid, VerbDef)>, WorldStateError>;

    /// Get the program for an existing verb
    fn get_verb_program(&self, obj: &Obj, uuid: Uuid) -> Result<ProgramType, WorldStateError>;

    /// Update the flags of an existing object
    fn update_object_flags(
        &mut self,
        obj: &Obj,
        flags: BitEnum<ObjFlag>,
    ) -> Result<(), WorldStateError>;

    /// Delete a property from an object
    fn delete_property(&mut self, obj: &Obj, propname: Symbol) -> Result<(), WorldStateError>;

    /// Remove a verb from an object
    fn remove_verb(&mut self, obj: &Obj, uuid: Uuid) -> Result<(), WorldStateError>;
}

/// Apply a batch of mutations to an object using the LoaderInterface.
/// This bypasses normal permission checks and verb calls - intended for VCS-style operations.
/// Returns detailed results for each mutation attempted.
pub fn batch_mutate(
    loader: &mut dyn LoaderInterface,
    target: &Obj,
    mutations: &[ObjectMutation],
) -> BatchMutationResult {
    let mut results = Vec::with_capacity(mutations.len());

    for (index, mutation) in mutations.iter().enumerate() {
        let result = apply_mutation(loader, target, mutation);
        results.push(MutationResult {
            index,
            mutation: mutation.clone(),
            result,
        });
    }

    BatchMutationResult {
        target: *target,
        results,
    }
}

/// Apply a single mutation to an object
fn apply_mutation(
    loader: &mut dyn LoaderInterface,
    target: &Obj,
    mutation: &ObjectMutation,
) -> Result<(), WorldStateError> {
    match mutation {
        ObjectMutation::DefineProperty {
            name,
            owner,
            flags,
            value,
        } => loader.define_property(target, target, *name, owner, *flags, value.clone()),

        ObjectMutation::DeleteProperty { name } => loader.delete_property(target, *name),

        ObjectMutation::SetPropertyValue { name, value } => {
            loader.set_property(target, *name, None, None, Some(value.clone()))
        }

        ObjectMutation::SetPropertyFlags { name, owner, flags } => {
            loader.set_property(target, *name, *owner, Some(*flags), None)
        }

        ObjectMutation::ClearProperty { name } => {
            // Clearing means setting to None value while keeping the property definition
            loader.set_property(target, *name, None, None, None)
        }

        ObjectMutation::DefineVerb {
            names,
            owner,
            flags,
            argspec,
            program,
        } => loader.add_verb(target, names, owner, *flags, *argspec, program.clone()),

        ObjectMutation::DeleteVerb { names } => {
            // Look up the verb UUID by names, then delete it
            match loader.get_existing_verb_by_names(target, names)? {
                Some((uuid, _)) => loader.remove_verb(target, uuid),
                None => Err(WorldStateError::VerbNotFound(*target, names[0].to_string())),
            }
        }

        ObjectMutation::UpdateVerbProgram { names, program } => {
            // Look up the verb UUID and current definition
            match loader.get_existing_verb_by_names(target, names)? {
                Some((uuid, verb_def)) => loader.update_verb(
                    target,
                    uuid,
                    names,
                    &verb_def.owner(),
                    verb_def.flags(),
                    verb_def.args(),
                    program.clone(),
                ),
                None => Err(WorldStateError::VerbNotFound(*target, names[0].to_string())),
            }
        }

        ObjectMutation::UpdateVerbMetadata {
            names,
            new_names,
            owner,
            flags,
            argspec,
        } => {
            // Look up the verb UUID and current definition
            match loader.get_existing_verb_by_names(target, names)? {
                Some((uuid, verb_def)) => {
                    // Get the current program
                    let program = loader.get_verb_program(target, uuid)?;

                    // Use provided values or fall back to existing
                    let final_names = new_names.as_ref().unwrap_or(names);
                    let verb_def_owner = verb_def.owner();
                    let final_owner = owner.as_ref().unwrap_or(&verb_def_owner);
                    let final_flags = flags.unwrap_or_else(|| verb_def.flags());
                    let final_argspec = argspec.unwrap_or_else(|| verb_def.args());

                    loader.update_verb(
                        target,
                        uuid,
                        final_names,
                        final_owner,
                        final_flags,
                        final_argspec,
                        program,
                    )
                }
                None => Err(WorldStateError::VerbNotFound(*target, names[0].to_string())),
            }
        }

        ObjectMutation::SetObjectFlags { flags } => loader.update_object_flags(target, *flags),

        ObjectMutation::SetParent { parent } => loader.set_object_parent(target, parent),

        ObjectMutation::SetLocation { location } => loader.set_object_location(target, location),
    }
}
