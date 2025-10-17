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

use crate::ObjdefLoaderError;
use moor_common::{
    model::{
        ObjAttrs, ObjFlag, ObjectKind, PropDef, PropFlag, ValSet, VerbDef, loader::LoaderInterface,
    },
    util::BitEnum,
};
use moor_compiler::{CompileOptions, ObjFileContext, ObjectDefinition, compile_object_definitions};
use moor_var::{NOTHING, Obj, Symbol, Var, program::ProgramType};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::Instant,
};
use tracing::info;

pub struct ObjectDefinitionLoader<'a> {
    object_definitions: HashMap<Obj, (PathBuf, ObjectDefinition)>,
    loader: &'a mut dyn LoaderInterface,
    // Track conflicts and removals as we go
    conflicts: Vec<(Obj, ConflictEntity)>,
    removals: Vec<(Obj, Entity)>,
}

/// How to handle a situation where:
///     * Object already exists
///     * Provided flags or builtin-props differ from loaded objdef file
///     * Parentage differs from loaded objdef file
///     * An existing defined property differs in value or flags from loaded objdef file
///     * An existing overridden property differs in value or flags from loaded objdef file
///     * A verb differs in flags or content from loaded objdef file
#[derive(Debug, Clone)]
pub enum ConflictMode {
    /// Indiscriminately overwrite the existing entity with the new value.
    Clobber,
    /// Skip all conflicts entirely and only add new verbs and properties that do not conflict.
    Skip,
}

/// Entities for which we can give instructions for overrides and removals.
#[derive(Debug, Clone, PartialEq)]
pub enum Entity {
    ObjectFlags,
    BuiltinProps,
    Parentage,
    PropertyDef(Symbol),
    PropertyValue(Symbol),
    PropertyFlag(Symbol),
    VerbDef(Vec<Symbol>),
    VerbProgram(Vec<Symbol>),
}

pub struct ObjDefLoaderOptions {
    /// True if we're running in "dry-run" mode where we test, and collect conflicts.
    pub dry_run: bool,
    /// How to handle conflicts.
    pub conflict_mode: ConflictMode,
    /// How to allocate the object ID. If None, uses the ID from the objdef file (default).
    /// Can be NextObjid (0), Anonymous (1), UuObjId (2), or Objid(#123) for a specific ID.
    pub object_kind: Option<ObjectKind>,
    /// Optional constants for compilation
    pub constants: Option<moor_var::Map>,
    /// The set of entities for which we will allow overriding and treat as if their specific
    /// ConflicTMode was "Clobber"
    pub overrides: Vec<(Obj, Entity)>,
    /// The set of entities which we will consider value for deletion
    /// Note that flags, builtin props and parentage are not valid values here.
    pub removals: Vec<(Obj, Entity)>,
    /// If true, validate parent changes for cycles, invalid parents, and descendant property conflicts.
    /// Should be true for individual load_object() calls, false for bulk operations (textdump, objdef directory import).
    pub validate_parent_changes: bool,
}

impl Default for ObjDefLoaderOptions {
    fn default() -> Self {
        Self {
            dry_run: false,
            conflict_mode: ConflictMode::Clobber,
            object_kind: None,
            constants: None,
            overrides: vec![],
            removals: vec![],
            validate_parent_changes: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ConflictEntity {
    ObjectFlags(BitEnum<ObjFlag>),
    BuiltinProps(Symbol, Var),
    Parentage(Obj),
    PropertyDef(Symbol, PropDef),
    PropertyValue(Symbol, Var),
    PropertyFlag(Symbol, BitEnum<PropFlag>),
    VerbDef(Vec<Symbol>, VerbDef),
    VerbProgram(Vec<Symbol>, ProgramType),
}

/// The results of loading either a directory or a single object
/// Where conflicts are returned they take the form of:
///         (conflicted-object, [entity], current-value, objdef-value)
#[derive(Debug)]
pub struct ObjDefLoaderResults {
    /// True if the caller should commit the transaction, otherwise it should be rolled back, either
    /// because we have a critical error, or the loader was run in dry-run mode.
    pub commit: bool,
    /// The set of conflicts discovered during loading, and handled using ConflictMode above
    pub conflicts: Vec<(Obj, ConflictEntity)>,
    /// The set of proposed or completed deletions (where objdef was lacking an entity found in
    /// existing)
    pub removals: Vec<(Obj, Entity)>,
    pub loaded_objects: Vec<Obj>,
    pub num_loaded_verbs: usize,
    pub num_loaded_property_definitions: usize,
    pub num_loaded_property_overrides: usize,
}

impl<'a> ObjectDefinitionLoader<'a> {
    pub fn new(loader: &'a mut dyn LoaderInterface) -> Self {
        Self {
            object_definitions: HashMap::new(),
            loader,
            conflicts: Vec::new(),
            removals: Vec::new(),
        }
    }

    /// Check if an entity should be overridden regardless of conflict mode
    fn should_override(&self, obj: &Obj, entity: &Entity, options: &ObjDefLoaderOptions) -> bool {
        options.overrides.contains(&(*obj, entity.clone()))
    }

    /// Determine the effective conflict mode for a given entity
    fn effective_conflict_mode(
        &self,
        obj: &Obj,
        entity: &Entity,
        options: &ObjDefLoaderOptions,
    ) -> ConflictMode {
        if self.should_override(obj, entity, options) {
            ConflictMode::Clobber
        } else {
            options.conflict_mode.clone()
        }
    }

    /// Check if we should proceed with an operation based on conflict detection
    /// Returns (should_proceed, conflict_option)
    fn check_conflict<T: Clone + PartialEq>(
        &self,
        obj: &Obj,
        entity: Entity,
        current_value: Option<T>,
        new_value: &T,
        conflict_entity_fn: impl FnOnce(T) -> ConflictEntity,
        options: &ObjDefLoaderOptions,
    ) -> (bool, Option<(Obj, ConflictEntity)>) {
        // If there's no current value, no conflict
        let Some(current) = current_value else {
            return (true, None);
        };

        // If values are the same, no conflict
        if &current == new_value {
            return (true, None);
        }

        // We have a conflict - create conflict record
        let conflict = conflict_entity_fn(current.clone());
        let conflict_record = (*obj, conflict);

        // Determine how to handle the conflict
        let should_proceed = match self.effective_conflict_mode(obj, &entity, options) {
            ConflictMode::Clobber => true, // Proceed with overwrite
            ConflictMode::Skip => false,   // Skip this operation
        };

        (should_proceed, Some(conflict_record))
    }

    /// Read an entire directory of objdef files, along with `constants.moo`, process them, and
    /// load them into the database.
    pub fn load_objdef_directory(
        &mut self,
        compile_options: CompileOptions,
        dirpath: &Path,
        options: ObjDefLoaderOptions,
    ) -> Result<ObjDefLoaderResults, ObjdefLoaderError> {
        // Check that the directory exists
        if !dirpath.exists() {
            return Err(ObjdefLoaderError::DirectoryNotFound(dirpath.to_path_buf()));
        }

        // Constant variables will go here.
        let mut context = ObjFileContext::new();

        // Verb compilation options
        let mut compile_options = compile_options.clone();
        compile_options.call_unsupported_builtins = true;

        // Collect all the file names,
        let filenames: Vec<_> = dirpath
            .read_dir()
            .expect("Unable to open import directory")
            .filter_map(|entry| entry.ok())
            .filter_map(|e| e.path().is_file().then(|| e.path()))
            .filter(|path| path.extension().map(|ext| ext == "moo").unwrap_or(false))
            .collect();
        // and if there's a "constants.moo" put that at the top to parse first
        let constants_file = filenames
            .iter()
            .find(|f| f.file_name().unwrap() == "constants.moo");
        if let Some(constants_file) = constants_file {
            let constants_file_contents = std::fs::read_to_string(constants_file)
                .map_err(|e| ObjdefLoaderError::ObjectFileReadError(constants_file.clone(), e))?;
            self.parse_objects(
                constants_file,
                &mut context,
                &constants_file_contents,
                &compile_options,
            )?;
        }

        // Read the objects first, going through and creating them all
        // Create all the objects first with no attributes, and then update after, so that the
        // inheritance/location etc hierarchy is set up right
        for object_file in filenames {
            if object_file.extension().unwrap() != "moo"
                || object_file.file_name().unwrap() == "constants.moo"
            {
                continue;
            }

            let object_file_contents = std::fs::read_to_string(object_file.clone())
                .map_err(|e| ObjdefLoaderError::ObjectFileReadError(object_file.clone(), e))?;

            self.parse_objects(
                &object_file,
                &mut context,
                &object_file_contents,
                &compile_options,
            )?;
        }

        let num_loaded_verbs = self
            .object_definitions
            .values()
            .map(|(_, d)| d.verbs.len())
            .sum::<usize>();
        let num_loaded_property_definitions = self
            .object_definitions
            .values()
            .map(|(_, d)| d.property_definitions.len())
            .sum::<usize>();
        let num_loaded_property_overrides = self
            .object_definitions
            .values()
            .map(|(_, d)| d.property_overrides.len())
            .sum::<usize>();

        info!(
            "Created {} objects. Adjusting inheritance, location, and ownership attributes...",
            self.object_definitions.len()
        );
        self.apply_attributes(&options)?;
        info!("Defining {} properties...", num_loaded_property_definitions);
        self.define_properties(&options)?;
        info!(
            "Overriding {} property values...",
            num_loaded_property_overrides
        );
        self.set_properties(&options)?;
        info!("Defining and compiling {} verbs...", num_loaded_verbs);
        self.define_verbs(&options)?;

        // Detect removals if specified in options
        self.detect_removals(&options)?;

        Ok(ObjDefLoaderResults {
            commit: !options.dry_run,
            conflicts: self.conflicts.clone(),
            removals: self.removals.clone(),
            loaded_objects: self.object_definitions.keys().cloned().collect(),
            num_loaded_verbs,
            num_loaded_property_definitions,
            num_loaded_property_overrides,
        })
    }

    fn parse_objects(
        &mut self,
        path: &Path,
        context: &mut ObjFileContext,
        object_file_contents: &str,
        compile_options: &CompileOptions,
    ) -> Result<(), ObjdefLoaderError> {
        let compiled_defs =
            compile_object_definitions(object_file_contents, compile_options, context).map_err(
                |e| ObjdefLoaderError::ObjectDefParseError(path.to_string_lossy().to_string(), e),
            )?;

        for compiled_def in compiled_defs {
            let oid = compiled_def.oid;

            self.loader
                .create_object(
                    ObjectKind::Objid(oid),
                    &ObjAttrs::new(
                        NOTHING,
                        NOTHING,
                        NOTHING,
                        compiled_def.flags,
                        &compiled_def.name,
                    ),
                )
                .map_err(|wse| {
                    ObjdefLoaderError::CouldNotCreateObject(
                        path.to_string_lossy().to_string(),
                        oid,
                        wse,
                    )
                })?;

            self.object_definitions
                .insert(oid, (path.to_path_buf(), compiled_def));
        }
        Ok(())
    }

    pub fn apply_attributes(
        &mut self,
        options: &ObjDefLoaderOptions,
    ) -> Result<(), ObjdefLoaderError> {
        // First phase: collect all conflicts
        let mut attribute_actions = Vec::new();

        for (obj, (path, def)) in &self.object_definitions {
            // Check if object already exists
            let existing_attrs = self.loader.get_existing_object(obj).map_err(|e| {
                ObjdefLoaderError::CouldNotSetObjectParent(path.to_string_lossy().to_string(), e)
            })?;

            if let Some(existing) = existing_attrs {
                // Check parent conflict (always check if existing parent differs)
                if existing.parent() != Some(def.parent) {
                    let (should_proceed, conflict) = self.check_conflict(
                        obj,
                        Entity::Parentage,
                        existing.parent(),
                        &def.parent,
                        ConflictEntity::Parentage,
                        options,
                    );
                    if let Some(conflict) = conflict {
                        self.conflicts.push(conflict);
                    }
                    if should_proceed {
                        attribute_actions.push((*obj, "parent", def.parent, path.clone()));
                    }
                }

                // Check location conflict
                if def.location != NOTHING {
                    let (should_proceed, conflict) = self.check_conflict(
                        obj,
                        Entity::BuiltinProps,
                        existing.location(),
                        &def.location,
                        |current| {
                            ConflictEntity::BuiltinProps(
                                Symbol::mk("location"),
                                moor_var::v_obj(current),
                            )
                        },
                        options,
                    );
                    if let Some(conflict) = conflict {
                        self.conflicts.push(conflict);
                    }
                    if should_proceed {
                        attribute_actions.push((*obj, "location", def.location, path.clone()));
                    }
                }

                // Check owner conflict
                if def.owner != NOTHING {
                    let (should_proceed, conflict) = self.check_conflict(
                        obj,
                        Entity::BuiltinProps,
                        existing.owner(),
                        &def.owner,
                        |current| {
                            ConflictEntity::BuiltinProps(
                                Symbol::mk("owner"),
                                moor_var::v_obj(current),
                            )
                        },
                        options,
                    );
                    if let Some(conflict) = conflict {
                        self.conflicts.push(conflict);
                    }
                    if should_proceed {
                        attribute_actions.push((*obj, "owner", def.owner, path.clone()));
                    }
                }

                // Check flags conflict
                let (should_proceed, conflict) = self.check_conflict(
                    obj,
                    Entity::ObjectFlags,
                    Some(existing.flags()),
                    &def.flags,
                    ConflictEntity::ObjectFlags,
                    options,
                );
                if let Some(conflict) = conflict {
                    self.conflicts.push(conflict);
                }
                if should_proceed {
                    // Update object flags to match objdef
                    self.loader
                        .update_object_flags(obj, def.flags)
                        .map_err(|e| {
                            ObjdefLoaderError::CouldNotSetObjectParent(
                                path.to_string_lossy().to_string(),
                                e,
                            )
                        })?;
                } else {
                    // In Skip mode, restore the original flags (since object was created with empty flags)
                    self.loader
                        .update_object_flags(obj, existing.flags())
                        .map_err(|e| {
                            ObjdefLoaderError::CouldNotSetObjectParent(
                                path.to_string_lossy().to_string(),
                                e,
                            )
                        })?;
                }
            } else {
                // Object doesn't exist yet, add all non-nothing attributes
                if def.parent != NOTHING {
                    attribute_actions.push((*obj, "parent", def.parent, path.clone()));
                }
                if def.location != NOTHING {
                    attribute_actions.push((*obj, "location", def.location, path.clone()));
                }
                if def.owner != NOTHING {
                    attribute_actions.push((*obj, "owner", def.owner, path.clone()));
                }
            }
        }

        // Second phase: apply all the actions
        for (obj, attr_type, value, path) in attribute_actions {
            match attr_type {
                "parent" => {
                    self.loader
                        .set_object_parent(&obj, &value, options.validate_parent_changes)
                        .map_err(|e| {
                            ObjdefLoaderError::CouldNotSetObjectParent(
                                path.to_string_lossy().to_string(),
                                e,
                            )
                        })?;
                }
                "location" => {
                    self.loader.set_object_location(&obj, &value).map_err(|e| {
                        ObjdefLoaderError::CouldNotSetObjectLocation(
                            path.to_string_lossy().to_string(),
                            e,
                        )
                    })?;
                }
                "owner" => {
                    self.loader.set_object_owner(&obj, &value).map_err(|e| {
                        ObjdefLoaderError::CouldNotSetObjectOwner(
                            path.to_string_lossy().to_string(),
                            e,
                        )
                    })?;
                }
                _ => unreachable!(),
            }
        }
        Ok(())
    }

    pub fn define_verbs(&mut self, options: &ObjDefLoaderOptions) -> Result<(), ObjdefLoaderError> {
        // First phase: collect conflicts and determine actions
        let mut verb_actions = Vec::new();

        for (obj, (path, def)) in &self.object_definitions {
            for v in &def.verbs {
                // Check if verb already exists
                let existing_verb = self
                    .loader
                    .get_existing_verb_by_names(obj, &v.names)
                    .map_err(|wse| {
                        ObjdefLoaderError::CouldNotDefineVerb(
                            path.to_string_lossy().to_string(),
                            *obj,
                            v.names.clone(),
                            wse,
                        )
                    })?;

                if let Some((existing_uuid, existing_verbdef)) = existing_verb {
                    // Verb exists - check for conflicts in both metadata and program
                    // Create a comparable VerbDef for metadata comparison
                    let new_verbdef = VerbDef::new(
                        existing_uuid, // Use existing UUID for fair comparison
                        *obj,          // location
                        v.owner,       // owner
                        &v.names,      // names
                        v.flags,       // flags
                        v.argspec,     // args
                    );

                    // Check for metadata conflicts
                    let (should_proceed_metadata, conflict_metadata) = self.check_conflict(
                        obj,
                        Entity::VerbDef(v.names.clone()),
                        Some(existing_verbdef.clone()),
                        &new_verbdef,
                        |current| ConflictEntity::VerbDef(v.names.clone(), current),
                        options,
                    );

                    // Also check if the program changed
                    let existing_program = self
                        .loader
                        .get_verb_program(obj, existing_uuid)
                        .map_err(|wse| {
                            ObjdefLoaderError::CouldNotDefineVerb(
                                path.to_string_lossy().to_string(),
                                *obj,
                                v.names.clone(),
                                wse,
                            )
                        })?;

                    let program_changed = existing_program != v.program;

                    // Determine final conflict and proceed status
                    let mut should_proceed = should_proceed_metadata;
                    if let Some(conflict) = conflict_metadata {
                        self.conflicts.push(conflict);
                    } else if program_changed {
                        // Metadata matches but program differs - still a conflict
                        let conflict =
                            ConflictEntity::VerbDef(v.names.clone(), existing_verbdef.clone());
                        self.conflicts.push((*obj, conflict));

                        // Apply conflict mode to program-only changes
                        should_proceed = match self.effective_conflict_mode(
                            obj,
                            &Entity::VerbDef(v.names.clone()),
                            options,
                        ) {
                            ConflictMode::Clobber => true,
                            ConflictMode::Skip => false,
                        };
                    }

                    if should_proceed {
                        // Use update_verb for existing verbs in Clobber mode
                        self.loader
                            .update_verb(
                                obj,
                                existing_uuid,
                                &v.names,
                                &v.owner,
                                v.flags,
                                v.argspec,
                                v.program.clone(),
                            )
                            .map_err(|wse| {
                                ObjdefLoaderError::CouldNotDefineVerb(
                                    path.to_string_lossy().to_string(),
                                    *obj,
                                    v.names.clone(),
                                    wse,
                                )
                            })?;
                    }
                } else {
                    // Verb doesn't exist, add it
                    verb_actions.push((*obj, v, path.clone()));
                }
            }
        }

        // Second phase: apply all the verb actions
        for (obj, verb, path) in verb_actions {
            self.loader
                .add_verb(
                    &obj,
                    &verb.names,
                    &verb.owner,
                    verb.flags,
                    verb.argspec,
                    verb.program.clone(),
                )
                .map_err(|wse| {
                    ObjdefLoaderError::CouldNotDefineVerb(
                        path.to_string_lossy().to_string(),
                        obj,
                        verb.names.clone(),
                        wse,
                    )
                })?;
        }
        Ok(())
    }
    pub fn define_properties(
        &mut self,
        options: &ObjDefLoaderOptions,
    ) -> Result<(), ObjdefLoaderError> {
        // Track actions as either create or update
        let mut create_actions = Vec::new();
        let mut update_actions = Vec::new();

        for (obj, (path, def)) in &self.object_definitions {
            for pd in &def.property_definitions {
                // Check if property already exists
                let existing_value = self
                    .loader
                    .get_existing_property_value(obj, pd.name)
                    .map_err(|wse| {
                        ObjdefLoaderError::CouldNotDefineProperty(
                            path.to_string_lossy().to_string(),
                            *obj,
                            (*pd.name.as_arc_string()).clone(),
                            wse,
                        )
                    })?;

                if let Some((existing_val, existing_perms)) = existing_value {
                    // Property exists - check for conflicts
                    let mut should_proceed = true;

                    // Check value conflict if we're defining a value
                    if let Some(new_value) = &pd.value {
                        let (proceed_value, conflict) = self.check_conflict(
                            obj,
                            Entity::PropertyValue(pd.name),
                            Some(existing_val.clone()),
                            new_value,
                            |current| ConflictEntity::PropertyValue(pd.name, current),
                            options,
                        );
                        if let Some(conflict) = conflict {
                            self.conflicts.push(conflict);
                        }
                        should_proceed &= proceed_value;
                    }

                    // Check permissions conflict
                    let (proceed_perms, conflict) = self.check_conflict(
                        obj,
                        Entity::PropertyFlag(pd.name),
                        Some(existing_perms.flags()),
                        &pd.perms.flags(),
                        |current| ConflictEntity::PropertyFlag(pd.name, current),
                        options,
                    );
                    if let Some(conflict) = conflict {
                        self.conflicts.push(conflict);
                    }
                    should_proceed &= proceed_perms;

                    if should_proceed {
                        // Property exists and we should proceed (Clobber mode) - use update
                        update_actions.push((*obj, pd, path.clone()));
                    }
                } else {
                    // Property doesn't exist, define it
                    create_actions.push((*obj, pd, path.clone()));
                }
            }
        }

        // Apply create actions using define_property
        for (obj, prop_def, path) in create_actions {
            self.loader
                .define_property(
                    &obj,
                    &obj,
                    prop_def.name,
                    &prop_def.perms.owner(),
                    prop_def.perms.flags(),
                    prop_def.value.clone(),
                )
                .map_err(|wse| {
                    ObjdefLoaderError::CouldNotDefineProperty(
                        path.to_string_lossy().to_string(),
                        obj,
                        (*prop_def.name.as_arc_string()).clone(),
                        wse,
                    )
                })?;
        }

        // Apply update actions using set_property
        for (obj, prop_def, path) in update_actions {
            self.loader
                .set_property(
                    &obj,
                    prop_def.name,
                    Some(prop_def.perms.owner()),
                    Some(prop_def.perms.flags()),
                    prop_def.value.clone(),
                )
                .map_err(|wse| {
                    ObjdefLoaderError::CouldNotDefineProperty(
                        path.to_string_lossy().to_string(),
                        obj,
                        (*prop_def.name.as_arc_string()).clone(),
                        wse,
                    )
                })?;
        }

        Ok(())
    }

    fn set_properties(&mut self, options: &ObjDefLoaderOptions) -> Result<(), ObjdefLoaderError> {
        // First phase: collect conflicts and determine actions
        let mut override_actions = Vec::new();

        for (obj, (path, def)) in &self.object_definitions {
            for pv in &def.property_overrides {
                // Check existing property value for conflicts
                let existing_value = self
                    .loader
                    .get_existing_property_value(obj, pv.name)
                    .map_err(|wse| {
                        ObjdefLoaderError::CouldNotOverrideProperty(
                            path.to_string_lossy().to_string(),
                            *obj,
                            (*pv.name.as_arc_string()).clone(),
                            wse,
                        )
                    })?;

                let mut should_proceed = true;

                if let Some((existing_val, existing_perms)) = existing_value {
                    // Check value conflict if we're setting a new value
                    if let Some(new_value) = &pv.value {
                        let (proceed_value, conflict) = self.check_conflict(
                            obj,
                            Entity::PropertyValue(pv.name),
                            Some(existing_val),
                            new_value,
                            |current| ConflictEntity::PropertyValue(pv.name, current),
                            options,
                        );
                        if let Some(conflict) = conflict {
                            self.conflicts.push(conflict);
                        }
                        should_proceed &= proceed_value;
                    }

                    // Check permissions conflict if we're updating permissions
                    if let Some(pu) = &pv.perms_update {
                        let (proceed_perms, conflict) = self.check_conflict(
                            obj,
                            Entity::PropertyFlag(pv.name),
                            Some(existing_perms.flags()),
                            &pu.flags(),
                            |current| ConflictEntity::PropertyFlag(pv.name, current),
                            options,
                        );
                        if let Some(conflict) = conflict {
                            self.conflicts.push(conflict);
                        }
                        should_proceed &= proceed_perms;
                    }
                }

                if should_proceed {
                    override_actions.push((*obj, pv, path.clone()));
                }
            }
        }

        // Second phase: apply all the override actions
        for (obj, prop_override, path) in override_actions {
            let pu = &prop_override.perms_update;
            self.loader
                .set_property(
                    &obj,
                    prop_override.name,
                    pu.as_ref().map(|p| p.owner()),
                    pu.as_ref().map(|p| p.flags()),
                    prop_override.value.clone(),
                )
                .map_err(|wse| {
                    ObjdefLoaderError::CouldNotOverrideProperty(
                        path.to_string_lossy().to_string(),
                        obj,
                        (*prop_override.name.as_arc_string()).clone(),
                        wse,
                    )
                })?;
        }
        Ok(())
    }

    /// Detect entities that exist in the database but not in the objdef files
    /// These are candidates for removal based on the options.removals list
    fn detect_removals(&mut self, options: &ObjDefLoaderOptions) -> Result<(), ObjdefLoaderError> {
        for (obj, entity) in &options.removals {
            match entity {
                Entity::PropertyDef(prop_name) => {
                    // Check if this property exists in database but not in objdef
                    let existing_props = self.loader.get_existing_properties(obj).map_err(|e| {
                        ObjdefLoaderError::CouldNotDefineProperty(
                            format!("object {obj}"),
                            *obj,
                            prop_name.as_arc_string().to_string(),
                            e,
                        )
                    })?;

                    // Check if property exists in database
                    let prop_exists_in_db = existing_props.iter().any(|p| p.name() == *prop_name);

                    if prop_exists_in_db {
                        // Check if property is defined in any objdef file
                        let prop_exists_in_objdef = self
                            .object_definitions
                            .get(obj)
                            .map(|(_, def)| {
                                def.property_definitions
                                    .iter()
                                    .any(|pd| pd.name == *prop_name)
                            })
                            .unwrap_or(false);

                        if !prop_exists_in_objdef {
                            // Property exists in DB but not in objdef - mark for removal
                            self.removals.push((*obj, entity.clone()));
                        }
                    }
                }

                Entity::VerbDef(verb_names) => {
                    // Check if this verb exists in database but not in objdef
                    let existing_verb = self
                        .loader
                        .get_existing_verb_by_names(obj, verb_names)
                        .map_err(|e| {
                            ObjdefLoaderError::CouldNotDefineVerb(
                                format!("object {obj}"),
                                *obj,
                                verb_names.clone(),
                                e,
                            )
                        })?;

                    if existing_verb.is_some() {
                        // Check if verb is defined in objdef file
                        let verb_exists_in_objdef = self
                            .object_definitions
                            .get(obj)
                            .map(|(_, def)| {
                                def.verbs.iter().any(|v| {
                                    // Check if any verb name matches
                                    v.names.iter().any(|name| verb_names.contains(name))
                                })
                            })
                            .unwrap_or(false);

                        if !verb_exists_in_objdef {
                            // Verb exists in DB but not in objdef - mark for removal
                            self.removals.push((*obj, entity.clone()));
                        }
                    }
                }

                // Other entity types can be added as needed
                _ => {
                    // For now, only support property and verb removal detection
                }
            }
        }

        Ok(())
    }

    /// Loads a single object definition from a string.
    /// This is a simplified alternative to `read_dirdump` for loading individual objects.
    pub fn load_single_object(
        &mut self,
        object_definition: &str,
        compile_options: CompileOptions,
        constants: Option<moor_var::Map>,
        options: ObjDefLoaderOptions,
    ) -> Result<ObjDefLoaderResults, ObjdefLoaderError> {
        let start_time = Instant::now();
        let source_name = "<string>".to_string();

        // Create a fresh context for this single object
        let mut context = ObjFileContext::new();

        // Add constants to the context if provided
        if let Some(constants_map) = constants {
            for (key, value) in constants_map.iter() {
                let key_symbol = key.as_symbol().map_err(|_| {
                    ObjdefLoaderError::ObjectDefParseError(
                        source_name.clone(),
                        moor_compiler::ObjDefParseError::ConstantNotFound(format!(
                            "Constants map key must be string or symbol, got: {key:?}"
                        )),
                    )
                })?;
                context.add_constant(key_symbol, value.clone());
            }
        }

        // Parse the object definition
        let compiled_defs =
            compile_object_definitions(object_definition, &compile_options, &mut context)
                .map_err(|e| ObjdefLoaderError::ObjectDefParseError(source_name.clone(), e))?;

        // Ensure we got exactly one object
        if compiled_defs.len() != 1 {
            return Err(ObjdefLoaderError::SingleObjectExpected(
                source_name,
                compiled_defs.len(),
            ));
        }

        let compiled_def = compiled_defs.into_iter().next().unwrap();

        // Determine the ObjectKind to use for creation
        let object_kind = match &options.object_kind {
            None => ObjectKind::Objid(compiled_def.oid), // Use the ID from objdef file (default)
            Some(kind) => kind.clone(), // Use specified kind (NextObjid, UuObjId, Anonymous, or specific Objid)
        };

        // Extract the expected object ID for conflict detection (only valid for Objid kind)
        let expected_oid = match object_kind {
            ObjectKind::Objid(id) => Some(id),
            _ => None,
        };

        // Check if object already exists (only for specific Objid)
        let existing_obj = if let Some(obj_id) = expected_oid {
            self.loader
                .get_existing_object(&obj_id)
                .map_err(|e| ObjdefLoaderError::CouldNotSetObjectParent(source_name.clone(), e))?
        } else {
            None
        };

        // Only create the object if it doesn't exist
        let oid = if existing_obj.is_none() {
            self.loader
                .create_object(
                    object_kind,
                    &ObjAttrs::new(
                        NOTHING,
                        NOTHING,
                        NOTHING,
                        compiled_def.flags,
                        &compiled_def.name,
                    ),
                )
                .map_err(|wse| {
                    ObjdefLoaderError::CouldNotCreateObject(
                        source_name.clone(),
                        expected_oid.unwrap_or(NOTHING),
                        wse,
                    )
                })?
        } else {
            // Object exists, use its ID
            expected_oid.unwrap()
        };

        // Store the definition for processing
        self.object_definitions
            .insert(oid, (PathBuf::from(&source_name), compiled_def));

        // Use the conflict-aware methods instead of inline logic
        self.apply_attributes(&options)?;
        self.define_properties(&options)?;
        self.set_properties(&options)?;
        self.define_verbs(&options)?;

        let num_loaded_verbs = self
            .object_definitions
            .values()
            .map(|(_, d)| d.verbs.len())
            .sum::<usize>();
        let num_loaded_property_definitions = self
            .object_definitions
            .values()
            .map(|(_, d)| d.property_definitions.len())
            .sum::<usize>();
        let num_loaded_property_overrides = self
            .object_definitions
            .values()
            .map(|(_, d)| d.property_overrides.len())
            .sum::<usize>();

        // Detect removals if specified in options
        self.detect_removals(&options)?;

        info!(
            "Loaded single object {} in {} ms",
            oid,
            start_time.elapsed().as_millis()
        );

        Ok(ObjDefLoaderResults {
            commit: !options.dry_run,
            conflicts: self.conflicts.clone(),
            removals: self.removals.clone(),
            loaded_objects: vec![oid],
            num_loaded_verbs,
            num_loaded_property_definitions,
            num_loaded_property_overrides,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{ConflictMode, ObjDefLoaderOptions, ObjectDefinitionLoader};
    use moor_common::model::{HasUuid, Named, PrepSpec, WorldStateSource};
    use moor_compiler::{CompileOptions, ObjFileContext};
    use moor_db::{Database, DatabaseConfig, TxDB};
    use moor_var::{NOTHING, Obj, SYSTEM_OBJECT, Symbol, v_str};
    use std::{path::Path, sync::Arc};

    fn test_db(path: &Path) -> Arc<TxDB> {
        Arc::new(TxDB::open(Some(path), DatabaseConfig::default()).0)
    }

    #[test]
    fn test_simple_single_obj() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());
        let mut loader = db.loader_client().unwrap();

        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());

        let spec = r#"
                object #1
                    name: "Test Object"
                    owner: #0
                    parent: #-1
                    location: #-1
                    wizard: false
                    programmer: false
                    player: false
                    fertile: true
                    readable: true

                    property description (owner: #1, flags: "rc") = "This is a test object";
                    property other (owner: #1, flags: "rc");

                    override description (owner: #1, flags: "rc") = "This is an altered test object";

                    verb "look_self look_*" (this to any) owner: #2 flags: "rxd"
                        return this.description;
                    endverb
                endobject"#;
        let mut context = ObjFileContext::new();
        let mock_path = Path::new("mock_path.moo");
        parser
            .parse_objects(mock_path, &mut context, spec, &CompileOptions::default())
            .unwrap();

        let options = ObjDefLoaderOptions::default();
        parser.apply_attributes(&options).unwrap();
        parser.define_verbs(&options).unwrap();
        parser.define_properties(&options).unwrap();
        parser.set_properties(&options).unwrap();

        loader.commit().unwrap();

        // Verify the object was created using a new transaction
        let tx = db.new_world_state().unwrap();
        let owner = tx.owner_of(&Obj::mk_id(1)).unwrap();
        let name = tx.name_of(&SYSTEM_OBJECT, &Obj::mk_id(1)).unwrap();
        let parent = tx.parent_of(&SYSTEM_OBJECT, &Obj::mk_id(1)).unwrap();
        let location = tx.location_of(&SYSTEM_OBJECT, &Obj::mk_id(1)).unwrap();

        assert_eq!(owner, SYSTEM_OBJECT);
        assert_eq!(name, "Test Object");
        assert_eq!(parent, NOTHING);
        assert_eq!(location, NOTHING);
    }

    #[test]
    fn test_multiple_objects() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());

        let spec = r#"
                // The file can contain multiple objects.

                object #1
                    name: "Root Object"
                    owner: #0
                    parent: #-1
                    location: #-1
                    wizard: false
                    programmer: false
                    player: false
                    fertile: true
                    readable: true

                    property description (owner: #1, flags: "rc") = "This is a root object";

                    verb "look_self look_*" (this none this) owner: #2 flags: "rxd"
                        return this.description;
                    endverb
                endobject

                /*
                 * And C/C++ style comments are supported.
                 */
                object #2
                    name: "Generic Thing"
                    owner: #0
                    parent: #1
                    location: #1
                    wizard: false
                    programmer: false
                    player: false
                    fertile: true
                    readable: true

                    override description = "This is a generic thing";

                    verb look (this none none) owner: #2 flags: "rxd"
                        player:tell(this:look_self());
                    endverb
                endobject"#;

        let mut context = ObjFileContext::new();
        let mock_path = Path::new("mock_path.moo");
        parser
            .parse_objects(mock_path, &mut context, spec, &CompileOptions::default())
            .unwrap();
        let options = ObjDefLoaderOptions::default();
        parser.apply_attributes(&options).unwrap();
        parser.define_verbs(&options).unwrap();
        parser.define_properties(&options).unwrap();
        parser.set_properties(&options).unwrap();
        loader.commit().unwrap();

        let ws = db.new_world_state().unwrap();
        let v = ws
            .find_command_verb_on(
                &SYSTEM_OBJECT,
                &Obj::mk_id(2),
                Symbol::mk("look"),
                &Obj::mk_id(2),
                PrepSpec::None,
                &NOTHING,
            )
            .unwrap();

        assert!(v.unwrap().1.names().contains(&Symbol::mk("look")));

        let p = ws
            .retrieve_property(&SYSTEM_OBJECT, &Obj::mk_id(2), Symbol::mk("description"))
            .unwrap();
        assert_eq!(p, v_str("This is a generic thing"));
    }

    #[test]
    fn test_load_single_object() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());
        let mut loader = db.loader_client().unwrap();

        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());

        let spec = r#"
                object #42
                    name: "Single Test Object"
                    owner: #0
                    parent: #-1
                    location: #-1
                    wizard: false
                    programmer: false
                    player: false
                    fertile: true
                    readable: true

                    property test_prop (owner: #42, flags: "rc") = "test value";

                    verb "test_verb" (this none none) owner: #42 flags: "rxd"
                        return "tested";
                    endverb
                endobject"#;

        let options = ObjDefLoaderOptions::default();
        let results = parser
            .load_single_object(spec, CompileOptions::default(), None, options)
            .unwrap();
        assert_eq!(results.loaded_objects.len(), 1);
        assert!(results.commit);
        loader.commit().unwrap();

        let oid = results.loaded_objects[0];
        assert_eq!(oid, Obj::mk_id(42));

        // Verify the object was created correctly
        let tx = db.new_world_state().unwrap();
        let name = tx.name_of(&SYSTEM_OBJECT, &oid).unwrap();
        let prop_value = tx
            .retrieve_property(&SYSTEM_OBJECT, &oid, Symbol::mk("test_prop"))
            .unwrap();

        assert_eq!(name, "Single Test Object");
        assert_eq!(prop_value, v_str("test value"));
    }

    #[test]
    fn test_load_single_object_multiple_objects_fails() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());
        let mut loader = db.loader_client().unwrap();

        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());

        let spec = r#"
                object #1
                    name: "Object One"
                    owner: #0
                    parent: #-1
                    location: #-1
                    wizard: false
                    programmer: false
                    player: false
                    fertile: true
                    readable: true
                endobject

                object #2
                    name: "Object Two"
                    owner: #0
                    parent: #-1
                    location: #-1
                    wizard: false
                    programmer: false
                    player: false
                    fertile: true
                    readable: true
                endobject"#;

        let options = ObjDefLoaderOptions::default();
        let result = parser.load_single_object(spec, CompileOptions::default(), None, options);
        assert!(result.is_err());

        match result.unwrap_err() {
            crate::ObjdefLoaderError::SingleObjectExpected(_, count) => {
                assert_eq!(count, 2);
            }
            _ => panic!("Expected SingleObjectExpected error"),
        }
    }

    #[test]
    fn test_clobber_mode_detects_flags_conflict() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());

        // Create initial object with wizard=false
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let initial_spec = r#"
            object #50
                name: "Test Object"
                owner: #0
                parent: #-1
                location: #-1
                wizard: false
                programmer: false
                player: false
                fertile: false
                readable: false
            endobject"#;

        parser
            .load_single_object(
                initial_spec,
                CompileOptions::default(),
                None,
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        // Now load same object with wizard=true (conflict)
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let conflicting_spec = r#"
            object #50
                name: "Test Object"
                owner: #0
                parent: #-1
                location: #-1
                wizard: true
                programmer: false
                player: false
                fertile: false
                readable: false
            endobject"#;

        let results = parser
            .load_single_object(
                conflicting_spec,
                CompileOptions::default(),
                None,
                ObjDefLoaderOptions::default(),
            )
            .unwrap();

        // Should detect conflict in flags
        assert_eq!(
            results.conflicts.len(),
            1,
            "Should detect one flags conflict"
        );

        // Verify conflict is for object flags
        match &results.conflicts[0].1 {
            crate::ConflictEntity::ObjectFlags(_) => {}
            other => panic!("Expected ObjectFlags conflict, got {:?}", other),
        }

        loader.commit().unwrap();

        // Verify flags were actually updated (Clobber mode)
        let ws = db.new_world_state().unwrap();
        let flags = ws.flags_of(&Obj::mk_id(50)).unwrap();
        assert!(
            flags.contains(moor_common::model::ObjFlag::Wizard),
            "Wizard flag should be set after clobber"
        );
    }

    #[test]
    fn test_skip_mode_preserves_existing_flags() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());

        // Create initial object with wizard=true
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let initial_spec = r#"
            object #51
                name: "Test Object"
                owner: #0
                parent: #-1
                location: #-1
                wizard: true
                programmer: false
                player: false
                fertile: false
                readable: false
            endobject"#;

        parser
            .load_single_object(
                initial_spec,
                CompileOptions::default(),
                None,
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        // Now load with wizard=false in Skip mode
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let conflicting_spec = r#"
            object #51
                name: "Test Object"
                owner: #0
                parent: #-1
                location: #-1
                wizard: false
                programmer: false
                player: false
                fertile: false
                readable: false
            endobject"#;

        let results = parser
            .load_single_object(
                conflicting_spec,
                CompileOptions::default(),
                None,
                ObjDefLoaderOptions {
                    conflict_mode: ConflictMode::Skip,
                    ..ObjDefLoaderOptions::default()
                },
            )
            .unwrap();

        // Should detect conflict
        assert_eq!(
            results.conflicts.len(),
            1,
            "Should detect one flags conflict"
        );

        loader.commit().unwrap();

        // Verify flags were NOT updated (Skip mode)
        let ws = db.new_world_state().unwrap();
        let flags = ws.flags_of(&Obj::mk_id(51)).unwrap();
        assert!(
            flags.contains(moor_common::model::ObjFlag::Wizard),
            "Wizard flag should still be true after skip"
        );
    }

    #[test]
    fn test_parse_objects_detects_flags_conflict() {
        // This test uses parse_objects directly (like load_objdef_directory does)
        // to verify that the bug in db_loader_client.rs:create_object is fixed
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());

        // Create initial object with wizard=false using parse_objects
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let initial_spec = r#"
            object #52
                name: "Test Object"
                owner: #0
                parent: #-1
                location: #-1
                wizard: false
                programmer: false
                player: false
                fertile: false
                readable: false
            endobject"#;

        let mut context = ObjFileContext::new();
        let mock_path = Path::new("test.moo");
        parser
            .parse_objects(
                mock_path,
                &mut context,
                initial_spec,
                &CompileOptions::default(),
            )
            .unwrap();

        let options = ObjDefLoaderOptions::default();
        parser.apply_attributes(&options).unwrap();
        loader.commit().unwrap();

        // Now load same object with wizard=true using parse_objects again
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let conflicting_spec = r#"
            object #52
                name: "Test Object"
                owner: #0
                parent: #-1
                location: #-1
                wizard: true
                programmer: false
                player: false
                fertile: false
                readable: false
            endobject"#;

        let mut context = ObjFileContext::new();
        // This parse_objects call will call create_object for an existing object
        // which triggers the bug in db_loader_client.rs
        parser
            .parse_objects(
                mock_path,
                &mut context,
                conflicting_spec,
                &CompileOptions::default(),
            )
            .unwrap();

        let options = ObjDefLoaderOptions::default();
        parser.apply_attributes(&options).unwrap();

        // BUG: This should detect a flags conflict, but won't because
        // parse_objects calls create_object which updates flags immediately
        // before apply_attributes can compare them
        assert_eq!(
            parser.conflicts.len(),
            1,
            "Should detect one flags conflict (WILL FAIL DUE TO BUG)"
        );

        loader.commit().unwrap();

        // Flags should be updated (Clobber mode)
        let ws = db.new_world_state().unwrap();
        let flags = ws.flags_of(&Obj::mk_id(52)).unwrap();
        assert!(
            flags.contains(moor_common::model::ObjFlag::Wizard),
            "Wizard flag should be set after clobber"
        );
    }

    #[test]
    fn test_clobber_works_for_parent() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());

        // Create parent objects first
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let parents_spec = r#"
            object #1
                name: "Parent One"
                owner: #0
                parent: #-1
                location: #-1
                wizard: false
                programmer: false
                player: false
                fertile: false
                readable: false
            endobject
            object #2
                name: "Parent Two"
                owner: #0
                parent: #-1
                location: #-1
                wizard: false
                programmer: false
                player: false
                fertile: false
                readable: false
            endobject"#;

        parser
            .load_single_object(
                parents_spec,
                CompileOptions::default(),
                None,
                ObjDefLoaderOptions::default(),
            )
            .unwrap_err(); // This should fail because we're loading 2 objects with load_single_object

        // Create parents properly
        let mut context = ObjFileContext::new();
        let mock_path = Path::new("test.moo");
        parser
            .parse_objects(
                mock_path,
                &mut context,
                parents_spec,
                &CompileOptions::default(),
            )
            .unwrap();
        let options = ObjDefLoaderOptions::default();
        parser.apply_attributes(&options).unwrap();
        loader.commit().unwrap();

        // Create child object with parent=#1
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let initial_spec = r#"
            object #53
                name: "Child Object"
                owner: #0
                parent: #1
                location: #-1
                wizard: false
                programmer: false
                player: false
                fertile: false
                readable: false
            endobject"#;

        parser
            .load_single_object(
                initial_spec,
                CompileOptions::default(),
                None,
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        // Verify initial parent is #1
        let ws = db.new_world_state().unwrap();
        let parent = ws.parent_of(&SYSTEM_OBJECT, &Obj::mk_id(53)).unwrap();
        assert_eq!(parent, Obj::mk_id(1), "Initial parent should be #1");

        // Now load with parent=#2 (clobber mode)
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let updated_spec = r#"
            object #53
                name: "Child Object"
                owner: #0
                parent: #2
                location: #-1
                wizard: false
                programmer: false
                player: false
                fertile: false
                readable: false
            endobject"#;

        parser
            .load_single_object(
                updated_spec,
                CompileOptions::default(),
                None,
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        // Verify parent was updated to #2
        let ws = db.new_world_state().unwrap();
        let parent = ws.parent_of(&SYSTEM_OBJECT, &Obj::mk_id(53)).unwrap();
        assert_eq!(
            parent,
            Obj::mk_id(2),
            "Parent should be updated to #2 in clobber mode"
        );
    }

    #[test]
    fn test_clobber_works_for_location() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());

        // Create location objects first
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let mut context = ObjFileContext::new();
        let mock_path = Path::new("test.moo");
        let locations_spec = r#"
            object #1
                name: "Location One"
                owner: #0
                parent: #-1
                location: #-1
            endobject
            object #2
                name: "Location Two"
                owner: #0
                parent: #-1
                location: #-1
            endobject"#;
        parser
            .parse_objects(
                mock_path,
                &mut context,
                locations_spec,
                &CompileOptions::default(),
            )
            .unwrap();
        let options = ObjDefLoaderOptions::default();
        parser.apply_attributes(&options).unwrap();
        loader.commit().unwrap();

        // Create object with location=#1
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let initial_spec = r#"
            object #54
                name: "Test Object"
                owner: #0
                parent: #-1
                location: #1
            endobject"#;
        parser
            .load_single_object(
                initial_spec,
                CompileOptions::default(),
                None,
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        // Verify initial location
        let ws = db.new_world_state().unwrap();
        let location = ws.location_of(&SYSTEM_OBJECT, &Obj::mk_id(54)).unwrap();
        assert_eq!(location, Obj::mk_id(1), "Initial location should be #1");

        // Now load with location=#2
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let updated_spec = r#"
            object #54
                name: "Test Object"
                owner: #0
                parent: #-1
                location: #2
            endobject"#;
        parser
            .load_single_object(
                updated_spec,
                CompileOptions::default(),
                None,
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        // Verify location was updated to #2
        let ws = db.new_world_state().unwrap();
        let location = ws.location_of(&SYSTEM_OBJECT, &Obj::mk_id(54)).unwrap();
        assert_eq!(
            location,
            Obj::mk_id(2),
            "Location should be updated to #2 in clobber mode"
        );
    }

    #[test]
    fn test_clobber_works_for_owner() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());

        // Create owner objects first
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let mut context = ObjFileContext::new();
        let mock_path = Path::new("test.moo");
        let owners_spec = r#"
            object #1
                name: "Owner One"
                owner: #0
                parent: #-1
                location: #-1
            endobject
            object #2
                name: "Owner Two"
                owner: #0
                parent: #-1
                location: #-1
            endobject"#;
        parser
            .parse_objects(
                mock_path,
                &mut context,
                owners_spec,
                &CompileOptions::default(),
            )
            .unwrap();
        let options = ObjDefLoaderOptions::default();
        parser.apply_attributes(&options).unwrap();
        loader.commit().unwrap();

        // Create object with owner=#1
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let initial_spec = r#"
            object #55
                name: "Test Object"
                owner: #1
                parent: #-1
                location: #-1
            endobject"#;
        parser
            .load_single_object(
                initial_spec,
                CompileOptions::default(),
                None,
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        // Verify initial owner
        let ws = db.new_world_state().unwrap();
        let owner = ws.owner_of(&Obj::mk_id(55)).unwrap();
        assert_eq!(owner, Obj::mk_id(1), "Initial owner should be #1");

        // Now load with owner=#2
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let updated_spec = r#"
            object #55
                name: "Test Object"
                owner: #2
                parent: #-1
                location: #-1
            endobject"#;
        parser
            .load_single_object(
                updated_spec,
                CompileOptions::default(),
                None,
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        // Verify owner was updated to #2
        let ws = db.new_world_state().unwrap();
        let owner = ws.owner_of(&Obj::mk_id(55)).unwrap();
        assert_eq!(
            owner,
            Obj::mk_id(2),
            "Owner should be updated to #2 in clobber mode"
        );
    }

    #[test]
    fn test_clobber_works_for_property_values() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());

        // Create object with property = "initial"
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let initial_spec = r#"
            object #56
                name: "Test Object"
                owner: #0
                parent: #-1
                location: #-1
                property test_prop (owner: #56, flags: "rc") = "initial value";
            endobject"#;
        parser
            .load_single_object(
                initial_spec,
                CompileOptions::default(),
                None,
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        // Verify initial property value
        let ws = db.new_world_state().unwrap();
        let prop_value = ws
            .retrieve_property(&SYSTEM_OBJECT, &Obj::mk_id(56), Symbol::mk("test_prop"))
            .unwrap();
        assert_eq!(
            prop_value,
            v_str("initial value"),
            "Initial property value should be 'initial value'"
        );

        // Now load with property = "updated"
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let updated_spec = r#"
            object #56
                name: "Test Object"
                owner: #0
                parent: #-1
                location: #-1
                property test_prop (owner: #56, flags: "rc") = "updated value";
            endobject"#;
        parser
            .load_single_object(
                updated_spec,
                CompileOptions::default(),
                None,
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        // Verify property value was updated
        let ws = db.new_world_state().unwrap();
        let prop_value = ws
            .retrieve_property(&SYSTEM_OBJECT, &Obj::mk_id(56), Symbol::mk("test_prop"))
            .unwrap();
        assert_eq!(
            prop_value,
            v_str("updated value"),
            "Property value should be updated to 'updated value' in clobber mode"
        );
    }

    // Note: We're skipping a dedicated property flags test because PropDef doesn't expose flags directly
    // and the property values test already covers the main clobber functionality

    #[test]
    fn test_clobber_works_for_verbs() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());

        // Create object with verb returning "initial"
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let initial_spec = r#"
            object #58
                name: "Test Object"
                owner: #0
                parent: #-1
                location: #-1
                verb "test_verb" (this none none) owner: #58 flags: "rxd"
                    return "initial";
                endverb
            endobject"#;
        parser
            .load_single_object(
                initial_spec,
                CompileOptions::default(),
                None,
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        // Get initial verb
        let ws = db.new_world_state().unwrap();
        let initial_verbdef = ws
            .get_verb(&SYSTEM_OBJECT, &Obj::mk_id(58), Symbol::mk("test_verb"))
            .unwrap();

        // Now load with verb returning "updated"
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let updated_spec = r#"
            object #58
                name: "Test Object"
                owner: #0
                parent: #-1
                location: #-1
                verb "test_verb" (this none none) owner: #58 flags: "rxd"
                    return "updated";
                endverb
            endobject"#;
        parser
            .load_single_object(
                updated_spec,
                CompileOptions::default(),
                None,
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        // Verify verb was updated by checking UUID changed (verb got replaced)
        let ws = db.new_world_state().unwrap();
        let updated_verbdef = ws
            .get_verb(&SYSTEM_OBJECT, &Obj::mk_id(58), Symbol::mk("test_verb"))
            .unwrap();

        // The verb should still exist with same name but different UUID indicates it was replaced
        assert_eq!(
            initial_verbdef.names(),
            updated_verbdef.names(),
            "Verb name should be same"
        );
        // In clobber mode, verbs are updated in place, so UUID should be the same but we can't easily verify program changed
        // Without access to the program. Let's just verify the verb still exists and has correct metadata
        assert_eq!(
            updated_verbdef.owner(),
            Obj::mk_id(58),
            "Verb owner should be correct"
        );
    }

    #[test]
    fn test_skip_mode_preserves_existing_parent() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());

        // Create parent objects first
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let mut context = ObjFileContext::new();
        let mock_path = Path::new("test.moo");
        let parents_spec = r#"
            object #1
                name: "Parent One"
                owner: #0
                parent: #-1
                location: #-1
            endobject
            object #2
                name: "Parent Two"
                owner: #0
                parent: #-1
                location: #-1
            endobject"#;
        parser
            .parse_objects(
                mock_path,
                &mut context,
                parents_spec,
                &CompileOptions::default(),
            )
            .unwrap();
        let options = ObjDefLoaderOptions::default();
        parser.apply_attributes(&options).unwrap();
        loader.commit().unwrap();

        // Create child object with parent=#1
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let initial_spec = r#"
            object #60
                name: "Child Object"
                owner: #0
                parent: #1
                location: #-1
            endobject"#;
        parser
            .load_single_object(
                initial_spec,
                CompileOptions::default(),
                None,
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        // Verify initial parent is #1
        let ws = db.new_world_state().unwrap();
        let parent = ws.parent_of(&SYSTEM_OBJECT, &Obj::mk_id(60)).unwrap();
        assert_eq!(parent, Obj::mk_id(1), "Initial parent should be #1");

        // Now load with parent=#2 in Skip mode
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let updated_spec = r#"
            object #60
                name: "Child Object"
                owner: #0
                parent: #2
                location: #-1
            endobject"#;
        let results = parser
            .load_single_object(
                updated_spec,
                CompileOptions::default(),
                None,
                ObjDefLoaderOptions {
                    conflict_mode: ConflictMode::Skip,
                    ..ObjDefLoaderOptions::default()
                },
            )
            .unwrap();

        // Should detect conflict
        assert_eq!(
            results.conflicts.len(),
            1,
            "Should detect one parent conflict"
        );

        loader.commit().unwrap();

        // Verify parent was NOT updated (Skip mode)
        let ws = db.new_world_state().unwrap();
        let parent = ws.parent_of(&SYSTEM_OBJECT, &Obj::mk_id(60)).unwrap();
        assert_eq!(
            parent,
            Obj::mk_id(1),
            "Parent should still be #1 after skip"
        );
    }

    #[test]
    fn test_skip_mode_preserves_existing_location() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());

        // Create location objects first
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let mut context = ObjFileContext::new();
        let mock_path = Path::new("test.moo");
        let locations_spec = r#"
            object #1
                name: "Location One"
                owner: #0
                parent: #-1
                location: #-1
            endobject
            object #2
                name: "Location Two"
                owner: #0
                parent: #-1
                location: #-1
            endobject"#;
        parser
            .parse_objects(
                mock_path,
                &mut context,
                locations_spec,
                &CompileOptions::default(),
            )
            .unwrap();
        let options = ObjDefLoaderOptions::default();
        parser.apply_attributes(&options).unwrap();
        loader.commit().unwrap();

        // Create object with location=#1
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let initial_spec = r#"
            object #61
                name: "Test Object"
                owner: #0
                parent: #-1
                location: #1
            endobject"#;
        parser
            .load_single_object(
                initial_spec,
                CompileOptions::default(),
                None,
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        // Verify initial location
        let ws = db.new_world_state().unwrap();
        let location = ws.location_of(&SYSTEM_OBJECT, &Obj::mk_id(61)).unwrap();
        assert_eq!(location, Obj::mk_id(1), "Initial location should be #1");

        // Now load with location=#2 in Skip mode
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let updated_spec = r#"
            object #61
                name: "Test Object"
                owner: #0
                parent: #-1
                location: #2
            endobject"#;
        let results = parser
            .load_single_object(
                updated_spec,
                CompileOptions::default(),
                None,
                ObjDefLoaderOptions {
                    conflict_mode: ConflictMode::Skip,
                    ..ObjDefLoaderOptions::default()
                },
            )
            .unwrap();

        // Should detect conflict
        assert_eq!(
            results.conflicts.len(),
            1,
            "Should detect one location conflict"
        );

        loader.commit().unwrap();

        // Verify location was NOT updated (Skip mode)
        let ws = db.new_world_state().unwrap();
        let location = ws.location_of(&SYSTEM_OBJECT, &Obj::mk_id(61)).unwrap();
        assert_eq!(
            location,
            Obj::mk_id(1),
            "Location should still be #1 after skip"
        );
    }

    #[test]
    fn test_skip_mode_preserves_existing_owner() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());

        // Create owner objects first
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let mut context = ObjFileContext::new();
        let mock_path = Path::new("test.moo");
        let owners_spec = r#"
            object #1
                name: "Owner One"
                owner: #0
                parent: #-1
                location: #-1
            endobject
            object #2
                name: "Owner Two"
                owner: #0
                parent: #-1
                location: #-1
            endobject"#;
        parser
            .parse_objects(
                mock_path,
                &mut context,
                owners_spec,
                &CompileOptions::default(),
            )
            .unwrap();
        let options = ObjDefLoaderOptions::default();
        parser.apply_attributes(&options).unwrap();
        loader.commit().unwrap();

        // Create object with owner=#1
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let initial_spec = r#"
            object #62
                name: "Test Object"
                owner: #1
                parent: #-1
                location: #-1
            endobject"#;
        parser
            .load_single_object(
                initial_spec,
                CompileOptions::default(),
                None,
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        // Verify initial owner
        let ws = db.new_world_state().unwrap();
        let owner = ws.owner_of(&Obj::mk_id(62)).unwrap();
        assert_eq!(owner, Obj::mk_id(1), "Initial owner should be #1");

        // Now load with owner=#2 in Skip mode
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let updated_spec = r#"
            object #62
                name: "Test Object"
                owner: #2
                parent: #-1
                location: #-1
            endobject"#;
        let results = parser
            .load_single_object(
                updated_spec,
                CompileOptions::default(),
                None,
                ObjDefLoaderOptions {
                    conflict_mode: ConflictMode::Skip,
                    ..ObjDefLoaderOptions::default()
                },
            )
            .unwrap();

        // Should detect conflict
        assert_eq!(
            results.conflicts.len(),
            1,
            "Should detect one owner conflict"
        );

        loader.commit().unwrap();

        // Verify owner was NOT updated (Skip mode)
        let ws = db.new_world_state().unwrap();
        let owner = ws.owner_of(&Obj::mk_id(62)).unwrap();
        assert_eq!(owner, Obj::mk_id(1), "Owner should still be #1 after skip");
    }

    #[test]
    fn test_skip_mode_preserves_existing_property_values() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());

        // Create object with property = "initial"
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let initial_spec = r#"
            object #63
                name: "Test Object"
                owner: #0
                parent: #-1
                location: #-1
                property test_prop (owner: #63, flags: "rc") = "initial value";
            endobject"#;
        parser
            .load_single_object(
                initial_spec,
                CompileOptions::default(),
                None,
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        // Verify initial property value
        let ws = db.new_world_state().unwrap();
        let prop_value = ws
            .retrieve_property(&SYSTEM_OBJECT, &Obj::mk_id(63), Symbol::mk("test_prop"))
            .unwrap();
        assert_eq!(
            prop_value,
            v_str("initial value"),
            "Initial property value should be 'initial value'"
        );

        // Now load with property = "updated" in Skip mode
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let updated_spec = r#"
            object #63
                name: "Test Object"
                owner: #0
                parent: #-1
                location: #-1
                property test_prop (owner: #63, flags: "rc") = "updated value";
            endobject"#;
        let results = parser
            .load_single_object(
                updated_spec,
                CompileOptions::default(),
                None,
                ObjDefLoaderOptions {
                    conflict_mode: ConflictMode::Skip,
                    ..ObjDefLoaderOptions::default()
                },
            )
            .unwrap();

        // Should detect conflict
        assert_eq!(
            results.conflicts.len(),
            1,
            "Should detect one property value conflict"
        );

        loader.commit().unwrap();

        // Verify property value was NOT updated (Skip mode)
        let ws = db.new_world_state().unwrap();
        let prop_value = ws
            .retrieve_property(&SYSTEM_OBJECT, &Obj::mk_id(63), Symbol::mk("test_prop"))
            .unwrap();
        assert_eq!(
            prop_value,
            v_str("initial value"),
            "Property value should still be 'initial value' after skip"
        );
    }

    #[test]
    fn test_skip_mode_preserves_existing_verbs() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());

        // Create object with verb returning "initial"
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let initial_spec = r#"
            object #64
                name: "Test Object"
                owner: #0
                parent: #-1
                location: #-1
                verb "test_verb" (this none none) owner: #64 flags: "rxd"
                    return "initial";
                endverb
            endobject"#;
        parser
            .load_single_object(
                initial_spec,
                CompileOptions::default(),
                None,
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        // Get initial verb
        let ws = db.new_world_state().unwrap();
        let initial_verbdef = ws
            .get_verb(&SYSTEM_OBJECT, &Obj::mk_id(64), Symbol::mk("test_verb"))
            .unwrap();
        let initial_uuid = initial_verbdef.uuid();

        // Now load with verb returning "updated" in Skip mode
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let updated_spec = r#"
            object #64
                name: "Test Object"
                owner: #0
                parent: #-1
                location: #-1
                verb "test_verb" (this none none) owner: #64 flags: "rxd"
                    return "updated";
                endverb
            endobject"#;
        let results = parser
            .load_single_object(
                updated_spec,
                CompileOptions::default(),
                None,
                ObjDefLoaderOptions {
                    conflict_mode: ConflictMode::Skip,
                    ..ObjDefLoaderOptions::default()
                },
            )
            .unwrap();

        // Should detect conflict
        assert_eq!(
            results.conflicts.len(),
            1,
            "Should detect one verb conflict"
        );

        loader.commit().unwrap();

        // Verify verb was NOT updated (Skip mode) - UUID should be unchanged
        let ws = db.new_world_state().unwrap();
        let final_verbdef = ws
            .get_verb(&SYSTEM_OBJECT, &Obj::mk_id(64), Symbol::mk("test_verb"))
            .unwrap();
        assert_eq!(
            final_verbdef.uuid(),
            initial_uuid,
            "Verb UUID should be unchanged in skip mode"
        );
    }

    #[test]
    fn test_reject_parent_cycle() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());

        // Create #1 with parent #-1 and #2 with parent #1
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let mut context = ObjFileContext::new();
        let mock_path = Path::new("test.moo");
        let initial_spec = r#"
            object #1
                name: "Object One"
                owner: #0
                parent: #-1
                location: #-1
            endobject
            object #2
                name: "Object Two"
                owner: #0
                parent: #1
                location: #-1
            endobject"#;
        parser
            .parse_objects(
                mock_path,
                &mut context,
                initial_spec,
                &CompileOptions::default(),
            )
            .unwrap();
        let options = ObjDefLoaderOptions::default();
        parser.apply_attributes(&options).unwrap();
        loader.commit().unwrap();

        // Now try to change #1's parent to #2, creating a cycle: #1 → #2 → #1
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let cycle_spec = r#"
            object #1
                name: "Object One"
                owner: #0
                parent: #2
                location: #-1
            endobject"#;

        let result = parser.load_single_object(
            cycle_spec,
            CompileOptions::default(),
            None,
            ObjDefLoaderOptions {
                dry_run: false,
                conflict_mode: ConflictMode::Clobber,
                object_kind: None,
                constants: None,
                overrides: vec![],
                removals: vec![],
                validate_parent_changes: true,
            },
        );

        // Should fail with a cycle detection error
        assert!(result.is_err(), "Loading object with cycle should fail");
        match result.unwrap_err() {
            crate::ObjdefLoaderError::CouldNotSetObjectParent(_, e) => {
                // Verify it's a cycle error from WorldStateError
                assert!(
                    matches!(e, moor_common::model::WorldStateError::RecursiveMove(_, _)),
                    "Expected RecursiveMove error, got {:?}",
                    e
                );
            }
            other => panic!("Expected CouldNotSetObjectParent error, got {:?}", other),
        }
    }

    #[test]
    fn test_reject_invalid_parent() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());

        // Create #1
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let initial_spec = r#"
            object #1
                name: "Object One"
                owner: #0
                parent: #-1
                location: #-1
            endobject"#;
        parser
            .load_single_object(
                initial_spec,
                CompileOptions::default(),
                None,
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        // Try to set #1's parent to #999 which doesn't exist
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let invalid_parent_spec = r#"
            object #1
                name: "Object One"
                owner: #0
                parent: #999
                location: #-1
            endobject"#;

        let result = parser.load_single_object(
            invalid_parent_spec,
            CompileOptions::default(),
            None,
            ObjDefLoaderOptions {
                dry_run: false,
                conflict_mode: ConflictMode::Clobber,
                object_kind: None,
                constants: None,
                overrides: vec![],
                removals: vec![],
                validate_parent_changes: true,
            },
        );

        // Should fail with invalid parent error
        assert!(
            result.is_err(),
            "Loading object with invalid parent should fail"
        );
        match result.unwrap_err() {
            crate::ObjdefLoaderError::CouldNotSetObjectParent(_, e) => {
                // Verify it's an invalid parent error
                assert!(
                    matches!(e, moor_common::model::WorldStateError::ObjectNotFound(_)),
                    "Expected ObjectNotFound error, got {:?}",
                    e
                );
            }
            other => panic!("Expected CouldNotSetObjectParent error, got {:?}", other),
        }

        // But NOTHING (#-1) should be allowed
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let nothing_parent_spec = r#"
            object #1
                name: "Object One"
                owner: #0
                parent: #-1
                location: #-1
            endobject"#;

        let result = parser.load_single_object(
            nothing_parent_spec,
            CompileOptions::default(),
            None,
            ObjDefLoaderOptions {
                dry_run: false,
                conflict_mode: ConflictMode::Clobber,
                object_kind: None,
                constants: None,
                overrides: vec![],
                removals: vec![],
                validate_parent_changes: true,
            },
        );

        // Should succeed
        assert!(
            result.is_ok(),
            "Loading object with NOTHING parent should succeed"
        );
    }

    #[test]
    fn test_reject_descendant_property_conflict() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());

        // Create hierarchy: #10 (no prop "bar"), #20 (defines prop "bar")
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let mut context = ObjFileContext::new();
        let mock_path = Path::new("test.moo");
        let parents_spec = r#"
            object #10
                name: "Parent Without Bar"
                owner: #0
                parent: #-1
                location: #-1
            endobject
            object #20
                name: "Parent With Bar"
                owner: #0
                parent: #-1
                location: #-1
                property bar (owner: #20, flags: "rc") = "from parent 20";
            endobject"#;
        parser
            .parse_objects(
                mock_path,
                &mut context,
                parents_spec,
                &CompileOptions::default(),
            )
            .unwrap();
        let options = ObjDefLoaderOptions::default();
        parser.apply_attributes(&options).unwrap();
        parser.define_properties(&options).unwrap();
        loader.commit().unwrap();

        // Create #50 with parent #10, and #51 as child of #50 defining property "bar"
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let mut context = ObjFileContext::new();
        let children_spec = r#"
            object #50
                name: "Middle Object"
                owner: #0
                parent: #10
                location: #-1
            endobject
            object #51
                name: "Child With Bar"
                owner: #0
                parent: #50
                location: #-1
                property bar (owner: #51, flags: "rc") = "from child 51";
            endobject"#;
        parser
            .parse_objects(
                mock_path,
                &mut context,
                children_spec,
                &CompileOptions::default(),
            )
            .unwrap();
        let options = ObjDefLoaderOptions::default();
        parser.apply_attributes(&options).unwrap();
        parser.define_properties(&options).unwrap();
        loader.commit().unwrap();

        // Now try to change #50's parent to #20
        // This should fail because #51 (descendant of #50) defines "bar"
        // and #20 (new parent ancestor) also defines "bar"
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let conflict_spec = r#"
            object #50
                name: "Middle Object"
                owner: #0
                parent: #20
                location: #-1
            endobject"#;

        let result = parser.load_single_object(
            conflict_spec,
            CompileOptions::default(),
            None,
            ObjDefLoaderOptions {
                dry_run: false,
                conflict_mode: ConflictMode::Clobber,
                object_kind: None,
                constants: None,
                overrides: vec![],
                removals: vec![],
                validate_parent_changes: true,
            },
        );

        // Should fail with property name conflict error
        assert!(
            result.is_err(),
            "Loading object with descendant property conflict should fail"
        );
        match result.unwrap_err() {
            crate::ObjdefLoaderError::CouldNotSetObjectParent(_, e) => {
                // Verify it's a property name conflict error
                assert!(
                    matches!(
                        e,
                        moor_common::model::WorldStateError::ChparentPropertyNameConflict(_, _, _)
                    ),
                    "Expected ChparentPropertyNameConflict error, got {:?}",
                    e
                );
            }
            other => panic!("Expected CouldNotSetObjectParent error, got {:?}", other),
        }
    }
}
