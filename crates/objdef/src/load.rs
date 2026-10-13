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

//! Database apply path for parsed objdef definitions.
//!
//! This module owns the mutating side of objdef import: creating placeholder objects, applying
//! attributes and metadata, defining properties and verbs, and resolving conflicts according to
//! loader options. It intentionally consumes parsed sets from `set.rs` for directory import so that
//! parsing, constants resolution, duplicate detection, and proposed graph construction have one
//! shared implementation.
//!
//! Use `ObjDefSet` when code needs to inspect an incoming objdef set without changing the database.
//! Use `ObjectDefinitionLoader` when code is ready to apply an objdef set or single definition to a
//! `LoaderInterface`.

#[cfg(test)]
use crate::set::compile_normalized_object_definitions;
use crate::{ObjDefSet, ObjDefSource, ObjdefLoaderError};
use moor_common::model::{
    HasUuid, Named, ObjAttrs, ObjFlag, ObjectKind, ValSet, WorldStateError, loader::LoaderInterface,
};
#[cfg(test)]
use moor_compiler::ObjFileContext;
use moor_compiler::{CompileOptions, ObjectDefinition};
use moor_var::{NOTHING, Obj, Symbol, Var};
use std::{collections::HashMap, path::Path, time::Instant};
use tracing::info;

/// Constants supplied to objdef parsing.
///
/// Directory imports usually read constants from `constants.moo`. Builtins and future changelist
/// code may instead provide a pre-parsed map. In both cases constants are loaded into the same
/// `ObjFileContext` used for object definitions.
#[derive(Clone)]
pub enum Constants {
    /// Pre-parsed constants map
    Map(moor_var::Map),
    /// MOO file content containing constant definitions to parse
    FileContent(String),
}

/// Applies parsed objdef definitions to a database loader.
///
/// The loader is stateful for one import operation. It stores the parsed object definitions,
/// creates placeholder objects first, and then applies the remaining object state in phases so
/// parent/location/owner references can resolve across the incoming set. Direct imports apply
/// supplied values; reviewed program updates use the separate comparison and review APIs.
pub struct ObjectDefinitionLoader<'a> {
    object_definitions: HashMap<Obj, (String, ObjectDefinition)>,
    parsed_constants: HashMap<Symbol, Var>,
    loader: &'a mut dyn LoaderInterface,
    restore_tracking: bool,
    mutation_started: bool,
}

/// Options controlling objdef apply behavior.
#[derive(Clone, Default)]
pub struct ObjDefLoaderOptions {
    /// How to allocate the object ID. If None, uses the ID from the objdef file (default).
    /// Can be NextObjid (0), Anonymous (1), UuObjId (2), or Objid(#123) for a specific ID.
    pub object_kind: Option<ObjectKind>,
    /// Optional constants for compilation (either as a map or as file content to parse)
    pub constants: Option<Constants>,
    /// If true, validate parent changes for cycles, invalid parents, and descendant property conflicts.
    /// Should be true for individual load_object() calls, false for bulk operations (textdump, objdef directory import).
    pub validate_parent_changes: bool,
}

/// Result summary from directory, single-object, or reload apply.
///
/// These counts describe applied source declarations. The caller still owns the transaction
/// and must commit it before reporting the import as durable.
#[derive(Debug)]
pub struct ObjDefLoaderResults {
    /// Whether the operation applied changes that require committing the transaction.
    pub commit: bool,
    /// Object IDs affected by the import, including any newly allocated IDs.
    pub loaded_objects: Vec<Obj>,
    /// Number of verb declarations applied.
    pub num_loaded_verbs: usize,
    /// Number of local property definitions applied.
    pub num_loaded_property_definitions: usize,
    /// Number of property overrides applied.
    pub num_loaded_property_overrides: usize,
}
/// A target override changes the declaration address, not embedded object references.
/// Reject self-references that would require a clone/remapping protocol.
fn validate_relocation(
    def: &ObjectDefinition,
    target: Option<Obj>,
) -> Result<(), ObjdefLoaderError> {
    if target == Some(def.oid) {
        return Ok(());
    }
    fn contains(value: &Var, object: Obj) -> bool {
        match value.variant() {
            moor_var::Variant::Obj(o) => o == object,
            moor_var::Variant::List(l) => l.iter().any(|v| contains(&v, object)),
            moor_var::Variant::Map(m) => m
                .iter()
                .any(|(k, v)| contains(&k, object) || contains(&v, object)),
            moor_var::Variant::Flyweight(f) => {
                *f.delegate() == object
                    || f.slots().iter().any(|(_, v)| contains(v, object))
                    || f.contents().iter().any(|v| contains(&v, object))
            }
            moor_var::Variant::Err(e) => e.value().is_some_and(|v| contains(v, object)),
            _ => false,
        }
    }
    let metadata = |entries: &[(Symbol, Var)]| {
        entries
            .iter()
            .any(|(k, v)| *k != Symbol::mk(crate::review::BASE_KEY) && contains(v, def.oid))
    };
    let mut references =
        [def.parent, def.owner, def.location].contains(&def.oid) || metadata(&def.metadata);
    for property in &def.property_definitions {
        references |= property.perms.owner() == def.oid
            || property
                .value
                .as_ref()
                .is_some_and(|v| contains(v, def.oid))
            || metadata(&property.metadata);
    }
    for property in &def.property_overrides {
        references |= property
            .perms_update
            .as_ref()
            .is_some_and(|p| p.owner() == def.oid)
            || property
                .value
                .as_ref()
                .is_some_and(|v| contains(v, def.oid))
            || metadata(&property.metadata);
    }
    for verb in &def.verbs {
        let moor_var::program::ProgramType::MooR(program) = &verb.program;
        let tree = moor_compiler::program_to_tree(program)
            .map_err(|e| ObjdefLoaderError::InvalidRelocation(e.to_string()))?;
        let (_, literals) = moor_compiler::unparse_for_comparison(&tree)
            .map_err(|e| ObjdefLoaderError::InvalidRelocation(e.to_string()))?;
        references |= verb.owner == def.oid
            || metadata(&verb.metadata)
            || literals.iter().any(|v| contains(v, def.oid));
    }
    if references {
        return Err(ObjdefLoaderError::InvalidRelocation(
            "source contains self-references; bind them explicitly before selecting another target"
                .into(),
        ));
    }
    Ok(())
}

impl<'a> ObjectDefinitionLoader<'a> {
    /// Create a loader that applies objdefs through the supplied database loader interface.
    pub fn new(loader: &'a mut dyn LoaderInterface) -> Self {
        Self {
            object_definitions: HashMap::new(),
            parsed_constants: HashMap::new(),
            loader,
            restore_tracking: false,
            mutation_started: false,
        }
    }

    /// Whether a failed operation requires discarding its enclosing transaction.
    pub fn mutation_started(&self) -> bool {
        self.mutation_started
    }

    fn definition_counts(&self) -> (usize, usize, usize) {
        let verbs = self
            .object_definitions
            .values()
            .map(|(_, d)| d.verbs.len())
            .sum();
        let property_defs = self
            .object_definitions
            .values()
            .map(|(_, d)| d.property_definitions.len())
            .sum();
        let property_overrides = self
            .object_definitions
            .values()
            .map(|(_, d)| d.property_overrides.len())
            .sum();
        (verbs, property_defs, property_overrides)
    }

    /// Import a directory, preserving supplied baselines without inventing missing history.
    pub fn load_objdef_directory(
        &mut self,
        compile_options: CompileOptions,
        dirpath: &Path,
        options: ObjDefLoaderOptions,
    ) -> Result<ObjDefLoaderResults, ObjdefLoaderError> {
        let set = ObjDefSet::read_directory(&compile_options, dirpath)?;
        self.load_objdef_set(set, options)
    }

    /// Bulk import an already parsed source set, optionally prepared with upstream baselines.
    /// The caller owns the transaction and must commit or roll back the entire import.
    pub fn load_objdef_set(
        &mut self,
        objdef_set: ObjDefSet,
        options: ObjDefLoaderOptions,
    ) -> Result<ObjDefLoaderResults, ObjdefLoaderError> {
        let staging_started_at = Instant::now();
        for (label, definition) in objdef_set.graph().object_definitions().values() {
            for verb in &definition.verbs {
                if let Some((_, baseline)) = verb
                    .metadata
                    .iter()
                    .find(|(key, _)| *key == Symbol::mk(crate::review::BASE_KEY))
                {
                    crate::review::read_baseline(Some(baseline.clone())).map_err(|e| {
                        ObjdefLoaderError::CouldNotDefineVerb(
                            label.clone(),
                            definition.oid,
                            verb.names.clone(),
                            WorldStateError::DatabaseError(e.to_string()),
                        )
                    })?;
                }
            }
        }
        self.restore_tracking = true;
        let constant_count = objdef_set.constants().len();
        self.stage_objdef_set(objdef_set, &options)?;

        info!(
            object_count = self.object_definitions.len(),
            constant_count,
            elapsed_ms = staging_started_at.elapsed().as_secs_f64() * 1000.0,
            "Staged object definitions"
        );

        let (num_loaded_verbs, num_loaded_property_definitions, num_loaded_property_overrides) =
            self.definition_counts();

        info!(
            "Created {} objects. Adjusting inheritance, location, and ownership attributes...",
            self.object_definitions.len()
        );
        self.apply_attributes(&options)?;
        self.apply_object_metadata(&options)?;
        info!("Defining {} properties...", num_loaded_property_definitions);
        self.define_properties(&options)?;
        info!(
            "Overriding {} property values...",
            num_loaded_property_overrides
        );
        self.set_properties(&options)?;
        info!("Defining and compiling {} verbs...", num_loaded_verbs);
        self.define_verbs(&options)?;

        // Create import_export_id metadata from constants when the input has no explicit IDs.
        self.create_import_export_ids_if_needed()?;

        Ok(ObjDefLoaderResults {
            commit: true,
            loaded_objects: self.object_definitions.keys().cloned().collect(),
            num_loaded_verbs,
            num_loaded_property_definitions,
            num_loaded_property_overrides,
        })
    }

    #[cfg(test)]
    fn parse_objects(
        &mut self,
        path: &Path,
        context: &mut ObjFileContext,
        object_file_contents: &str,
        compile_options: &CompileOptions,
    ) -> Result<(), ObjdefLoaderError> {
        context.set_base_path(path);
        let path_str = path.to_string_lossy().into_owned();
        let compiled_defs = compile_normalized_object_definitions(
            object_file_contents,
            compile_options,
            context,
        )
        .map_err(|e| ObjdefLoaderError::ObjectDefParseError(path_str.clone(), Box::new(e)))?;

        for compiled_def in compiled_defs {
            let oid = compiled_def.oid;

            self.object_definitions
                .insert(oid, (path_str.clone(), compiled_def));
        }
        self.parsed_constants = context.constants().clone();
        self.create_placeholder_objects()?;
        Ok(())
    }

    /// Attach a parsed objdef set to this loader and create placeholder objects.
    ///
    /// Placeholder creation keeps the existing import algorithm intact: all incoming objects exist
    /// before attributes, properties, and verbs are applied, so references inside the set can resolve
    /// during later phases.
    fn stage_objdef_set(
        &mut self,
        objdef_set: ObjDefSet,
        _options: &ObjDefLoaderOptions,
    ) -> Result<(), ObjdefLoaderError> {
        let (object_definitions, constants) = objdef_set.into_parts();
        self.object_definitions = object_definitions;
        self.create_placeholder_objects()?;
        self.parsed_constants = constants;
        Ok(())
    }

    fn create_placeholder_objects(&mut self) -> Result<(), ObjdefLoaderError> {
        for (oid, (path, compiled_def)) in &self.object_definitions {
            self.loader
                .create_object(
                    ObjectKind::Objid(*oid),
                    &ObjAttrs::new(
                        NOTHING,
                        NOTHING,
                        NOTHING,
                        compiled_def.flags,
                        &compiled_def.name,
                    ),
                )
                .map_err(|wse| ObjdefLoaderError::CouldNotCreateObject(path.clone(), *oid, wse))?;
        }
        Ok(())
    }

    /// Apply object attributes after all referenced objects have been allocated.
    pub fn apply_attributes(
        &mut self,
        options: &ObjDefLoaderOptions,
    ) -> Result<(), ObjdefLoaderError> {
        for (obj, (path, def)) in &self.object_definitions {
            let error = |e| ObjdefLoaderError::CouldNotSetObjectParent(path.clone(), e);
            self.loader
                .set_object_name(obj, def.name.clone())
                .map_err(error)?;
            // Establish ownership before acquiring properties from a new parent.
            // Existing inherited properties retain their owners across this change.
            self.loader
                .set_object_owner(obj, &def.owner)
                .map_err(|e| ObjdefLoaderError::CouldNotSetObjectOwner(path.clone(), e))?;
            self.loader
                .set_object_parent(obj, &def.parent, options.validate_parent_changes)
                .map_err(error)?;
            self.loader
                .set_object_location(obj, &def.location)
                .map_err(|e| ObjdefLoaderError::CouldNotSetObjectLocation(path.clone(), e))?;
            self.loader
                .update_object_flags(obj, def.flags)
                .map_err(error)?;
        }
        Ok(())
    }

    fn apply_object_metadata(
        &mut self,
        _options: &ObjDefLoaderOptions,
    ) -> Result<(), ObjdefLoaderError> {
        for (obj, (path, def)) in &self.object_definitions {
            for (key, value) in &def.metadata {
                if !self.restore_tracking && *key == Symbol::mk(crate::review::BASE_KEY) {
                    continue;
                }
                self.loader
                    .set_object_metadata(obj, *key, value.clone())
                    .map_err(|e| {
                        ObjdefLoaderError::CouldNotSetObjectMetadata(
                            path.clone(),
                            *obj,
                            key.to_string(),
                            e,
                        )
                    })?;
            }
        }
        Ok(())
    }

    /// Merge exact verb declarations. Preserve matching UUIDs and append new declarations in order.
    /// An overlapping alias alone does not identify the same declaration.
    pub fn define_verbs(
        &mut self,
        _options: &ObjDefLoaderOptions,
    ) -> Result<(), ObjdefLoaderError> {
        for (obj, (path, def)) in &self.object_definitions {
            let mut seen = std::collections::HashSet::new();
            for verb in &def.verbs {
                let error = |e| {
                    ObjdefLoaderError::CouldNotDefineVerb(path.clone(), *obj, verb.names.clone(), e)
                };
                if !seen.insert((verb.names.clone(), verb.argspec)) {
                    return Err(error(WorldStateError::DatabaseError(
                        "duplicate verb declaration".into(),
                    )));
                }
                let existing = self.loader.get_existing_verbs(obj).map_err(error)?;
                let matches = existing
                    .iter()
                    .filter(|d| d.names() == verb.names && d.args() == verb.argspec)
                    .collect::<Vec<_>>();
                if matches.len() > 1 {
                    return Err(error(WorldStateError::DatabaseError(
                        "ambiguous verb declaration".into(),
                    )));
                }
                let uuid = if let Some(existing) = matches.first() {
                    self.loader
                        .update_verb(
                            obj,
                            existing.uuid(),
                            &verb.names,
                            &verb.owner,
                            verb.flags,
                            verb.argspec,
                            verb.program.clone(),
                        )
                        .map_err(error)?;
                    existing.uuid()
                } else {
                    let before: std::collections::HashSet<_> =
                        existing.iter().map(|d| d.uuid()).collect();
                    self.loader
                        .add_verb(
                            obj,
                            &verb.names,
                            &verb.owner,
                            verb.flags,
                            verb.argspec,
                            verb.program.clone(),
                        )
                        .map_err(error)?;
                    self.loader
                        .get_existing_verbs(obj)
                        .map_err(error)?
                        .iter()
                        .find(|d| !before.contains(&d.uuid()))
                        .ok_or_else(|| {
                            error(WorldStateError::DatabaseError(
                                "new verb was not found".into(),
                            ))
                        })?
                        .uuid()
                };
                for (key, value) in &verb.metadata {
                    if !self.restore_tracking && *key == Symbol::mk(crate::review::BASE_KEY) {
                        continue;
                    }
                    self.loader
                        .set_verb_metadata(obj, uuid, *key, value.clone())
                        .map_err(error)?;
                }
            }
        }
        Ok(())
    }

    /// Apply explicitly declared property definitions, preserving exact values and clear states.
    pub fn define_properties(
        &mut self,
        _options: &ObjDefLoaderOptions,
    ) -> Result<(), ObjdefLoaderError> {
        for (obj, (path, def)) in &self.object_definitions {
            for property in &def.property_definitions {
                let error = |e| {
                    ObjdefLoaderError::CouldNotDefineProperty(
                        path.clone(),
                        *obj,
                        property.name.to_string(),
                        e,
                    )
                };
                let existing = self
                    .loader
                    .get_existing_properties(obj)
                    .map_err(error)?
                    .iter()
                    .find(|p| p.name() == property.name && p.definer() == *obj);
                if existing.is_some() {
                    self.loader
                        .set_property(
                            obj,
                            property.name,
                            Some(property.perms.owner()),
                            Some(property.perms.flags()),
                            property.value.clone(),
                        )
                        .map_err(error)?;
                    if property.clear_value {
                        self.loader
                            .clear_property_value(obj, property.name)
                            .map_err(error)?;
                    }
                } else {
                    self.loader
                        .define_property(
                            obj,
                            obj,
                            property.name,
                            &property.perms.owner(),
                            property.perms.flags(),
                            property.value.clone(),
                        )
                        .map_err(error)?;
                }
                for (key, value) in &property.metadata {
                    if !self.restore_tracking && *key == Symbol::mk(crate::review::BASE_KEY) {
                        continue;
                    }
                    self.loader
                        .set_property_metadata(obj, property.name, *key, value.clone())
                        .map_err(error)?;
                }
            }
        }
        Ok(())
    }

    fn set_properties(&mut self, _options: &ObjDefLoaderOptions) -> Result<(), ObjdefLoaderError> {
        for (obj, (path, def)) in &self.object_definitions {
            for property in &def.property_overrides {
                let error = |e| {
                    ObjdefLoaderError::CouldNotOverrideProperty(
                        path.clone(),
                        *obj,
                        property.name.to_string(),
                        e,
                    )
                };
                self.loader
                    .set_property(
                        obj,
                        property.name,
                        property.perms_update.as_ref().map(|p| p.owner()),
                        property.perms_update.as_ref().map(|p| p.flags()),
                        property.value.clone(),
                    )
                    .map_err(error)?;
                if property.clear_value {
                    self.loader
                        .clear_property_value(obj, property.name)
                        .map_err(error)?;
                }
                for (key, value) in &property.metadata {
                    if !self.restore_tracking && *key == Symbol::mk(crate::review::BASE_KEY) {
                        continue;
                    }
                    self.loader
                        .set_property_metadata(obj, property.name, *key, value.clone())
                        .map_err(error)?;
                }
            }
        }
        Ok(())
    }

    /// Load one object definition from a string.
    ///
    /// This is the scalar import path used by `load_object()`. It accepts exactly one object
    /// definition and optional constant substitutions. Omitted attributes and members survive.
    /// Explicit values are applied exactly, including case-only edits and clear states.
    pub fn load_single_object(
        &mut self,
        object_definition: &str,
        compile_options: CompileOptions,
        options: ObjDefLoaderOptions,
    ) -> Result<ObjDefLoaderResults, ObjdefLoaderError> {
        self.restore_tracking = false;
        self.mutation_started = false;
        self.object_definitions.clear();
        let start_time = Instant::now();
        let source_name = "<string>".to_string();

        let set = ObjDefSet::parse_sources(
            &compile_options,
            None,
            options.constants.as_ref(),
            [ObjDefSource::new(&source_name, object_definition)],
        )?;
        let definitions = set.graph().object_definitions();
        if definitions.len() != 1 {
            return Err(ObjdefLoaderError::SingleObjectExpected(
                source_name,
                definitions.len(),
            ));
        }
        let mut compiled_def = definitions.values().next().unwrap().1.clone();

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

        validate_relocation(&compiled_def, expected_oid)?;

        // Check if object already exists (only for specific Objid)
        let existing_obj = if let Some(obj_id) = expected_oid {
            self.loader
                .get_existing_object(&obj_id)
                .map_err(|e| ObjdefLoaderError::CouldNotSetObjectParent(source_name.clone(), e))?
        } else {
            None
        };

        if let Some(existing) = &existing_obj
            && let Some(present) = &compiled_def.declared_attributes
        {
            if !present.contains("name") {
                compiled_def.name = existing.name().unwrap_or_default().to_string();
            }
            if !present.contains("owner") {
                compiled_def.owner = existing.owner().unwrap_or(NOTHING);
            }
            if !present.contains("parent") {
                compiled_def.parent = existing.parent().unwrap_or(NOTHING);
            }
            if !present.contains("location") {
                compiled_def.location = existing.location().unwrap_or(NOTHING);
            }
            for (name, flag) in [
                ("wizard", ObjFlag::Wizard),
                ("programmer", ObjFlag::Programmer),
                ("player", ObjFlag::User),
                ("readable", ObjFlag::Read),
                ("writeable", ObjFlag::Write),
                ("fertile", ObjFlag::Fertile),
            ] {
                if !present.contains(name) && existing.flags().contains(flag) {
                    compiled_def.flags.set(flag);
                }
            }
        }
        self.mutation_started = true;

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
            .insert(oid, (source_name.clone(), compiled_def));

        // Apply supplied declarations in dependency order
        self.apply_attributes(&options)?;
        self.apply_object_metadata(&options)?;
        self.define_properties(&options)?;
        self.set_properties(&options)?;
        self.define_verbs(&options)?;

        let (num_loaded_verbs, num_loaded_property_definitions, num_loaded_property_overrides) =
            self.definition_counts();

        info!(
            "Loaded single object {} in {} ms",
            oid,
            start_time.elapsed().as_millis()
        );

        Ok(ObjDefLoaderResults {
            commit: true,
            loaded_objects: vec![oid],
            num_loaded_verbs,
            num_loaded_property_definitions,
            num_loaded_property_overrides,
        })
    }

    /// Replace one existing object with the contents of an objdef.
    ///
    /// Existing verbs and locally defined properties that are absent from the incoming definition
    /// are deleted. Verb definitions are recreated in source order, invalidating their baselines.
    /// Flags, attributes, properties, and ordinary metadata are replaced with source content.
    /// If `target_obj` is supplied, the incoming object ID is treated as the source identity
    /// but the mutation is applied to `target_obj`.
    ///
    /// # Arguments
    /// * `object_definition` - The MOO object definition string
    /// * `compile_options` - Runtime language features used to compile source and constants
    /// * `constants` - Optional constants (either as a map or as file content to parse)
    /// * `target_obj` - Optional target object ID. If None, uses the ID from the objdef
    pub fn reload_single_object(
        &mut self,
        object_definition: &str,
        compile_options: CompileOptions,
        constants: Option<Constants>,
        target_obj: Option<Obj>,
    ) -> Result<ObjDefLoaderResults, ObjdefLoaderError> {
        self.restore_tracking = false;
        self.mutation_started = false;
        self.object_definitions.clear();
        let start_time = Instant::now();
        let source_name = "<reload>".to_string();

        let set = ObjDefSet::parse_sources(
            &compile_options,
            None,
            constants.as_ref(),
            [ObjDefSource::new(&source_name, object_definition)],
        )?;
        let definitions = set.graph().object_definitions();
        if definitions.len() != 1 {
            return Err(ObjdefLoaderError::SingleObjectExpected(
                source_name,
                definitions.len(),
            ));
        }
        let compiled_def = definitions.values().next().unwrap().1.clone();

        // Determine the target object ID
        let target_oid = target_obj.unwrap_or(compiled_def.oid);

        validate_relocation(&compiled_def, Some(target_oid))?;

        // Check if object exists
        let existing_obj = self
            .loader
            .get_existing_object(&target_oid)
            .map_err(|e| ObjdefLoaderError::CouldNotSetObjectParent(source_name.clone(), e))?;

        if existing_obj.is_none() {
            return Err(ObjdefLoaderError::CouldNotSetObjectParent(
                source_name,
                WorldStateError::ObjectNotFound(moor_common::model::ObjectRef::Id(target_oid)),
            ));
        }
        self.mutation_started = true;
        self.loader
            .prepare_object_replacement(&target_oid, Symbol::mk(crate::review::BASE_KEY))
            .map_err(|e| ObjdefLoaderError::CouldNotSetObjectParent(source_name.clone(), e))?;
        for verb in self
            .loader
            .get_existing_verbs(&target_oid)
            .map_err(|e| ObjdefLoaderError::CouldNotSetObjectParent(source_name.clone(), e))?
            .iter()
        {
            self.loader
                .remove_verb(&target_oid, verb.uuid())
                .map_err(|e| {
                    ObjdefLoaderError::CouldNotDefineVerb(
                        source_name.clone(),
                        target_oid,
                        verb.names().to_vec(),
                        e,
                    )
                })?;
        }
        for property in self
            .loader
            .get_existing_properties(&target_oid)
            .map_err(|e| ObjdefLoaderError::CouldNotSetObjectParent(source_name.clone(), e))?
            .iter()
        {
            if property.definer() == target_oid
                && !compiled_def
                    .property_definitions
                    .iter()
                    .any(|p| p.name == property.name())
            {
                self.loader
                    .delete_property(&target_oid, property.name())
                    .map_err(|e| {
                        ObjdefLoaderError::CouldNotDefineProperty(
                            source_name.clone(),
                            target_oid,
                            property.name().to_string(),
                            e,
                        )
                    })?;
            }
        }

        // Store the definition for processing
        self.object_definitions
            .insert(target_oid, (source_name.clone(), compiled_def));

        // Replace source content and validate the resulting parent relationship.
        let apply_options = ObjDefLoaderOptions {
            object_kind: None,
            constants: None,
            validate_parent_changes: true,
        };

        self.apply_attributes(&apply_options)?;
        self.apply_object_metadata(&apply_options)?;
        self.define_properties(&apply_options)?;
        self.set_properties(&apply_options)?;
        self.define_verbs(&apply_options)?;

        let (num_loaded_verbs, num_loaded_property_definitions, num_loaded_property_overrides) =
            self.definition_counts();

        info!(
            "Reloaded object {} in {} ms",
            target_oid,
            start_time.elapsed().as_millis()
        );

        Ok(ObjDefLoaderResults {
            commit: true,
            loaded_objects: vec![target_oid],
            num_loaded_verbs,
            num_loaded_property_definitions,
            num_loaded_property_overrides,
        })
    }

    /// Create import_export_id metadata from constants when the input declares no explicit IDs.
    fn create_import_export_ids_if_needed(&mut self) -> Result<(), ObjdefLoaderError> {
        use moor_var::v_string;

        let import_export_id_sym = crate::import_export_id();

        // If any object has explicit naming metadata, do not infer IDs from constants.
        let any_have_id = self.object_definitions.values().any(|(_, def)| {
            def.metadata
                .iter()
                .any(|(key, _)| *key == import_export_id_sym)
        });

        // If any object has it, assume the import has explicit naming and do not infer IDs.
        if any_have_id {
            return Ok(());
        }

        // Extract the constant names from the context (these come from constants.moo).
        for (name, value) in self.parsed_constants.iter() {
            let Some(obj) = value.as_object() else {
                continue;
            };
            if !self.object_definitions.contains_key(&obj) {
                continue;
            }
            self.loader
                .set_object_metadata(
                    &obj,
                    import_export_id_sym,
                    v_string(name.to_string().to_lowercase()),
                )
                .map_err(|wse| {
                    ObjdefLoaderError::CouldNotSetObjectMetadata(
                        "<import_export_id>".to_string(),
                        obj,
                        "import_export_id".to_string(),
                        wse,
                    )
                })?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{ObjDefLoaderOptions, ObjdefLoaderError, ObjectDefinitionLoader};
    use moor_common::model::{HasUuid, Named, TaskPermissions, WorldStateSource};
    use moor_common::util::BitEnum;
    use moor_compiler::{CompileOptions, ObjFileContext};
    use moor_db::{Database, DatabaseConfig, TxDB};
    use moor_var::{Obj, SYSTEM_OBJECT, Symbol, v_str};
    use std::{fs, path::Path, sync::Arc};

    fn test_db(path: &Path) -> Arc<TxDB> {
        Arc::new(
            TxDB::try_open(Some(path), DatabaseConfig::default())
                .unwrap()
                .0,
        )
    }

    fn system_permissions() -> TaskPermissions {
        TaskPermissions::new(SYSTEM_OBJECT, BitEnum::new())
    }

    #[test]
    fn directory_import_uses_shared_set_constants_path() {
        let tmpdir = tempfile::tempdir().unwrap();
        let import_dir = tmpdir.path().join("import");
        fs::create_dir(&import_dir).unwrap();
        fs::write(import_dir.join("constants.moo"), "define ROOT = #1;").unwrap();
        fs::write(
            import_dir.join("root.moo"),
            r#"
            object ROOT
                name: "Root"
                owner: #-1
                parent: #-1
                location: #-1
                wizard: false
                programmer: false
                player: false
                fertile: false
                readable: true
            endobject
            "#,
        )
        .unwrap();

        let db = test_db(tmpdir.path());
        let mut loader = db.loader_client().unwrap();
        let mut object_loader = ObjectDefinitionLoader::new(loader.as_mut());
        object_loader
            .load_objdef_directory(
                CompileOptions::default(),
                &import_dir,
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        let tx = db.new_world_state().unwrap();
        assert_eq!(
            tx.name_of(&system_permissions(), &Obj::mk_id(1)).unwrap(),
            "Root"
        );
        assert_eq!(
            tx.get_object_metadata(
                &system_permissions(),
                &Obj::mk_id(1),
                Symbol::mk("import_export_id")
            )
            .unwrap()
            .unwrap()
            .as_string(),
            Some("root")
        );
    }

    #[test]
    fn directory_import_updates_existing_object() {
        let tmpdir = tempfile::tempdir().unwrap();
        let import_dir = tmpdir.path().join("import");
        fs::create_dir(&import_dir).unwrap();

        let db = test_db(tmpdir.path());
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        parser
            .load_single_object(
                r#"
                object #10
                    name: "Existing"
                    owner: #0
                    parent: #-1
                    location: #-1
                    wizard: false
                    programmer: false
                    player: false
                    fertile: false
                    readable: false
                endobject
                "#,
                CompileOptions::default(),
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        fs::write(
            import_dir.join("object_10.moo"),
            r#"
            object #10
                name: "Existing"
                owner: #0
                parent: #-1
                location: #-1
                wizard: true
                programmer: false
                player: false
                fertile: false
                readable: false
            endobject
            "#,
        )
        .unwrap();

        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let results = parser
            .load_objdef_directory(
                CompileOptions::default(),
                &import_dir,
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        assert_eq!(results.loaded_objects, vec![Obj::mk_id(10)]);
        loader.commit().unwrap();

        let ws = db.new_world_state().unwrap();
        let flags = ws.flags_of(&Obj::mk_id(10)).unwrap();
        assert!(flags.contains(moor_common::model::ObjFlag::Wizard));
    }

    #[test]
    fn ownership_import_uses_final_owner_and_preserves_existing_property_owners() {
        for bulk in [false, true] {
            let tmpdir = tempfile::tempdir().unwrap();
            let db = test_db(tmpdir.path());
            let mut loader = db.loader_client().unwrap();
            let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
            parser
                .load_single_object(
                    r#"object #0
                    owner: #0
                    wizard: true
                    property value (owner: #0, flags: "rc") = 1;
                endobject"#,
                    CompileOptions::default(),
                    ObjDefLoaderOptions::default(),
                )
                .unwrap();
            loader.commit().unwrap();

            for (owner, expected_property_owner) in [("#0", SYSTEM_OBJECT), ("#1", SYSTEM_OBJECT)] {
                let source = format!("object #1 owner: {owner} parent: #0 endobject");
                let mut loader = db.loader_client().unwrap();
                let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
                if bulk {
                    let import_dir = tmpdir.path().join("import");
                    fs::create_dir_all(&import_dir).unwrap();
                    fs::write(import_dir.join("child.moo"), &source).unwrap();
                    parser
                        .load_objdef_directory(
                            CompileOptions::default(),
                            &import_dir,
                            ObjDefLoaderOptions::default(),
                        )
                        .unwrap();
                } else {
                    parser
                        .load_single_object(
                            &source,
                            CompileOptions::default(),
                            ObjDefLoaderOptions::default(),
                        )
                        .unwrap();
                }
                loader.commit().unwrap();
                let ws = db.new_world_state().unwrap();
                assert_eq!(
                    ws.get_property_info(
                        &system_permissions(),
                        &Obj::mk_id(1),
                        Symbol::mk("value")
                    )
                    .unwrap()
                    .1
                    .owner(),
                    expected_property_owner
                );
                assert!(
                    ws.is_property_clear(
                        &system_permissions(),
                        &Obj::mk_id(1),
                        Symbol::mk("value")
                    )
                    .unwrap()
                );
            }
        }
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
            .load_single_object(spec, CompileOptions::default(), options)
            .unwrap();
        assert_eq!(results.loaded_objects.len(), 1);
        assert!(results.commit);
        loader.commit().unwrap();

        let oid = results.loaded_objects[0];
        assert_eq!(oid, Obj::mk_id(42));

        // Verify the object was created correctly
        let tx = db.new_world_state().unwrap();
        let name = tx.name_of(&system_permissions(), &oid).unwrap();
        let prop_value = tx
            .retrieve_property(&system_permissions(), &oid, Symbol::mk("test_prop"))
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
        let result = parser.load_single_object(spec, CompileOptions::default(), options);
        assert!(result.is_err());

        match result.unwrap_err() {
            ObjdefLoaderError::SingleObjectExpected(_, count) => {
                assert_eq!(count, 2);
            }
            _ => panic!("Expected SingleObjectExpected error"),
        }
    }

    #[test]
    fn test_merge_updates_flags() {
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
                ObjDefLoaderOptions::default(),
            )
            .unwrap();

        assert_eq!(results.loaded_objects, vec![Obj::mk_id(50)]);

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
    fn test_merge_updates_parent() {
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
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        // Verify initial parent is #1
        let ws = db.new_world_state().unwrap();
        let parent = ws
            .parent_of(&system_permissions(), &Obj::mk_id(53))
            .unwrap();
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
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        // Verify parent was updated to #2
        let ws = db.new_world_state().unwrap();
        let parent = ws
            .parent_of(&system_permissions(), &Obj::mk_id(53))
            .unwrap();
        assert_eq!(
            parent,
            Obj::mk_id(2),
            "Parent should be updated to #2 in clobber mode"
        );
    }

    #[test]
    fn test_merge_updates_location() {
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
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        // Verify initial location
        let ws = db.new_world_state().unwrap();
        let location = ws
            .location_of(&system_permissions(), &Obj::mk_id(54))
            .unwrap();
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
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        // Verify location was updated to #2
        let ws = db.new_world_state().unwrap();
        let location = ws
            .location_of(&system_permissions(), &Obj::mk_id(54))
            .unwrap();
        assert_eq!(
            location,
            Obj::mk_id(2),
            "Location should be updated to #2 in clobber mode"
        );
    }

    #[test]
    fn test_merge_updates_owner() {
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
    fn test_merge_updates_property_values() {
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
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        // Verify initial property value
        let ws = db.new_world_state().unwrap();
        let prop_value = ws
            .retrieve_property(
                &system_permissions(),
                &Obj::mk_id(56),
                Symbol::mk("test_prop"),
            )
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
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        // Verify property value was updated
        let ws = db.new_world_state().unwrap();
        let prop_value = ws
            .retrieve_property(
                &system_permissions(),
                &Obj::mk_id(56),
                Symbol::mk("test_prop"),
            )
            .unwrap();
        assert_eq!(
            prop_value,
            v_str("updated value"),
            "Property value should be updated to 'updated value' in clobber mode"
        );
    }

    #[test]
    fn test_merge_updates_verbs() {
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
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        let ws = db.new_world_state().unwrap();
        let initial_verbdef = ws
            .get_verb(
                &system_permissions(),
                &Obj::mk_id(58),
                Symbol::mk("test_verb"),
            )
            .unwrap();
        let (initial_program, _) = ws
            .retrieve_verb(
                &system_permissions(),
                &Obj::mk_id(58),
                initial_verbdef.uuid(),
            )
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
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        let ws = db.new_world_state().unwrap();
        let updated_verbdef = ws
            .get_verb(
                &system_permissions(),
                &Obj::mk_id(58),
                Symbol::mk("test_verb"),
            )
            .unwrap();
        let (updated_program, _) = ws
            .retrieve_verb(
                &system_permissions(),
                &Obj::mk_id(58),
                updated_verbdef.uuid(),
            )
            .unwrap();

        assert_eq!(
            initial_verbdef.names(),
            updated_verbdef.names(),
            "Verb name should be same"
        );
        assert_eq!(
            updated_verbdef.owner(),
            Obj::mk_id(58),
            "Verb owner should be correct"
        );
        assert_ne!(
            initial_program, updated_program,
            "Verb program should change in clobber mode"
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
            ObjDefLoaderOptions {
                object_kind: None,
                constants: None,
                validate_parent_changes: true,
            },
        );

        // Should fail with a cycle detection error
        assert!(result.is_err(), "Loading object with cycle should fail");
        match result.unwrap_err() {
            ObjdefLoaderError::CouldNotSetObjectParent(_, e) => {
                // Verify it's a cycle error from WorldStateError
                assert!(
                    matches!(e, moor_common::model::WorldStateError::RecursiveMove(_, _)),
                    "Expected RecursiveMove error, got {e:?}"
                );
            }
            other => panic!("Expected CouldNotSetObjectParent error, got {other:?}"),
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
            ObjDefLoaderOptions {
                object_kind: None,
                constants: None,
                validate_parent_changes: true,
            },
        );

        // Should fail with invalid parent error
        assert!(
            result.is_err(),
            "Loading object with invalid parent should fail"
        );
        match result.unwrap_err() {
            ObjdefLoaderError::CouldNotSetObjectParent(_, e) => {
                // Verify it's an invalid parent error
                assert!(
                    matches!(e, moor_common::model::WorldStateError::ObjectNotFound(_)),
                    "Expected ObjectNotFound error, got {e:?}"
                );
            }
            other => panic!("Expected CouldNotSetObjectParent error, got {other:?}"),
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
            ObjDefLoaderOptions {
                object_kind: None,
                constants: None,
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
    fn test_reload_single_object_basic() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());

        // Create initial object with some verbs and properties
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let initial_spec = r#"
            object #100
                name: "Test Object"
                owner: #0
                parent: #-1
                location: #-1
                wizard: false
                programmer: false
                player: false

                property old_prop (owner: #100, flags: "rc") = "old value";
                property keep_prop (owner: #100, flags: "rc") = "will be removed";

                verb "old_verb" (this none none) owner: #100 flags: "rxd"
                    return "old";
                endverb

                verb "keep_verb" (this none none) owner: #100 flags: "rxd"
                    return "will be removed";
                endverb
            endobject"#;

        parser
            .load_single_object(
                initial_spec,
                CompileOptions::default(),
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        // Verify initial state
        let ws = db.new_world_state().unwrap();
        assert!(
            ws.retrieve_property(
                &system_permissions(),
                &Obj::mk_id(100),
                Symbol::mk("old_prop")
            )
            .is_ok()
        );
        assert!(
            ws.retrieve_property(
                &system_permissions(),
                &Obj::mk_id(100),
                Symbol::mk("keep_prop")
            )
            .is_ok()
        );
        assert!(
            ws.get_verb(
                &system_permissions(),
                &Obj::mk_id(100),
                Symbol::mk("old_verb")
            )
            .is_ok()
        );
        assert!(
            ws.get_verb(
                &system_permissions(),
                &Obj::mk_id(100),
                Symbol::mk("keep_verb")
            )
            .is_ok()
        );

        // Now reload with different verbs and properties
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let reload_spec = r#"
            object #100
                name: "Test Object"
                owner: #0
                parent: #-1
                location: #-1
                wizard: true
                programmer: false
                player: false

                property new_prop (owner: #100, flags: "rc") = "new value";
                property old_prop (owner: #100, flags: "rc") = "updated value";

                verb "new_verb" (this none none) owner: #100 flags: "rxd"
                    return "new";
                endverb

                verb "old_verb" (this none none) owner: #100 flags: "rxd"
                    return "updated";
                endverb
            endobject"#;

        let results = parser
            .reload_single_object(reload_spec, CompileOptions::default(), None, None)
            .unwrap();

        assert_eq!(results.loaded_objects.len(), 1);
        assert_eq!(results.loaded_objects[0], Obj::mk_id(100));
        loader.commit().unwrap();

        // Verify final state
        let ws = db.new_world_state().unwrap();

        // New property should exist
        let new_prop = ws
            .retrieve_property(
                &system_permissions(),
                &Obj::mk_id(100),
                Symbol::mk("new_prop"),
            )
            .unwrap();
        assert_eq!(new_prop, v_str("new value"));

        // Old property should be updated
        let old_prop = ws
            .retrieve_property(
                &system_permissions(),
                &Obj::mk_id(100),
                Symbol::mk("old_prop"),
            )
            .unwrap();
        assert_eq!(old_prop, v_str("updated value"));

        // keep_prop should be GONE
        assert!(
            ws.retrieve_property(
                &system_permissions(),
                &Obj::mk_id(100),
                Symbol::mk("keep_prop")
            )
            .is_err()
        );

        // new_verb should exist
        assert!(
            ws.get_verb(
                &system_permissions(),
                &Obj::mk_id(100),
                Symbol::mk("new_verb")
            )
            .is_ok()
        );

        // old_verb should exist
        assert!(
            ws.get_verb(
                &system_permissions(),
                &Obj::mk_id(100),
                Symbol::mk("old_verb")
            )
            .is_ok()
        );

        // keep_verb should be GONE
        assert!(
            ws.get_verb(
                &system_permissions(),
                &Obj::mk_id(100),
                Symbol::mk("keep_verb")
            )
            .is_err()
        );

        // Wizard flag should be updated
        let flags = ws.flags_of(&Obj::mk_id(100)).unwrap();
        assert!(flags.contains(moor_common::model::ObjFlag::Wizard));
    }

    #[test]
    fn test_reload_with_target_override() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());

        // Create object #200
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let initial_spec = r#"
            object #200
                name: "Initial Object"
                owner: #0
                parent: #-1
                location: #-1
                property old_prop (owner: #200, flags: "rc") = "old";
            endobject"#;

        parser
            .load_single_object(
                initial_spec,
                CompileOptions::default(),
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        // Reload object #200 with objdef that says #999, but override to target #200
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let reload_spec = r#"
            object #999
                name: "Reloaded Object"
                owner: #0
                parent: #-1
                location: #-1
                property new_prop (owner: #200, flags: "rc") = "new";
            endobject"#;

        let results = parser
            .reload_single_object(
                reload_spec,
                CompileOptions::default(),
                None,
                Some(Obj::mk_id(200)),
            )
            .unwrap();

        assert_eq!(results.loaded_objects[0], Obj::mk_id(200)); // Should use target override
        loader.commit().unwrap();

        // Verify #200 was updated
        let ws = db.new_world_state().unwrap();
        let name = ws.name_of(&system_permissions(), &Obj::mk_id(200)).unwrap();
        assert_eq!(name, "Reloaded Object");

        // old_prop should be gone, new_prop should exist
        assert!(
            ws.retrieve_property(
                &system_permissions(),
                &Obj::mk_id(200),
                Symbol::mk("old_prop")
            )
            .is_err()
        );
        assert!(
            ws.retrieve_property(
                &system_permissions(),
                &Obj::mk_id(200),
                Symbol::mk("new_prop")
            )
            .is_ok()
        );

        // #999 should NOT exist
        assert!(!ws.valid(&Obj::mk_id(999)).unwrap());
    }

    #[test]
    fn test_reload_rejects_missing_object() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());

        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        assert!(
            parser
                .reload_single_object(
                    "object #300 endobject",
                    CompileOptions::default(),
                    None,
                    None
                )
                .is_err()
        );
        assert!(!parser.mutation_started());
        loader.commit().unwrap();
        assert!(
            !db.new_world_state()
                .unwrap()
                .valid(&Obj::mk_id(300))
                .unwrap()
        );
    }

    #[test]
    fn test_reload_preserves_inherited_properties() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());

        // Create parent with a property
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let parent_spec = r#"
            object #400
                name: "Parent"
                owner: #0
                parent: #-1
                location: #-1
                property inherited_prop (owner: #400, flags: "rc") = "from parent";
            endobject"#;

        parser
            .load_single_object(
                parent_spec,
                CompileOptions::default(),
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        // Create child with its own property
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let child_spec = r#"
            object #401
                name: "Child"
                owner: #0
                parent: #400
                location: #-1
                property own_prop (owner: #401, flags: "rc") = "own value";
            endobject"#;

        parser
            .load_single_object(
                child_spec,
                CompileOptions::default(),
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        // Reload child with different own property
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let reload_spec = r#"
            object #401
                name: "Child"
                owner: #0
                parent: #400
                location: #-1
                property new_own_prop (owner: #401, flags: "rc") = "new own value";
            endobject"#;

        parser
            .reload_single_object(reload_spec, CompileOptions::default(), None, None)
            .unwrap();
        loader.commit().unwrap();

        // Verify inherited property still accessible, old own property gone, new own property exists
        let ws = db.new_world_state().unwrap();

        // Inherited property should still be accessible
        let inherited = ws
            .retrieve_property(
                &system_permissions(),
                &Obj::mk_id(401),
                Symbol::mk("inherited_prop"),
            )
            .unwrap();
        assert_eq!(inherited, v_str("from parent"));

        // Old own property should be gone
        assert!(
            ws.retrieve_property(
                &system_permissions(),
                &Obj::mk_id(401),
                Symbol::mk("own_prop")
            )
            .is_err()
        );

        // New own property should exist
        let new_own = ws
            .retrieve_property(
                &system_permissions(),
                &Obj::mk_id(401),
                Symbol::mk("new_own_prop"),
            )
            .unwrap();
        assert_eq!(new_own, v_str("new own value"));
    }

    #[test]
    fn test_reload_reject_parent_cycle() {
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

        // Now try to reload #1 with parent #2, creating a cycle: #1 → #2 → #1
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let cycle_spec = r#"
            object #1
                name: "Object One"
                owner: #0
                parent: #2
                location: #-1
            endobject"#;

        let result = parser.reload_single_object(cycle_spec, CompileOptions::default(), None, None);

        // Should fail with a cycle detection error
        assert!(result.is_err(), "Reloading object with cycle should fail");
        match result.unwrap_err() {
            ObjdefLoaderError::CouldNotSetObjectParent(_, e) => {
                assert!(
                    matches!(e, moor_common::model::WorldStateError::RecursiveMove(_, _)),
                    "Expected RecursiveMove error, got {e:?}"
                );
            }
            other => panic!("Expected CouldNotSetObjectParent error, got {other:?}"),
        }
    }

    #[test]
    fn test_reload_reject_invalid_parent() {
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
                ObjDefLoaderOptions::default(),
            )
            .unwrap();
        loader.commit().unwrap();

        // Try to reload #1 with parent #999 which doesn't exist
        let mut loader = db.loader_client().unwrap();
        let mut parser = ObjectDefinitionLoader::new(loader.as_mut());
        let invalid_parent_spec = r#"
            object #1
                name: "Object One"
                owner: #0
                parent: #999
                location: #-1
            endobject"#;

        let result =
            parser.reload_single_object(invalid_parent_spec, CompileOptions::default(), None, None);

        // Should fail with invalid parent error
        assert!(
            result.is_err(),
            "Reloading object with invalid parent should fail"
        );
        match result.unwrap_err() {
            ObjdefLoaderError::CouldNotSetObjectParent(_, e) => {
                assert!(
                    matches!(e, moor_common::model::WorldStateError::ObjectNotFound(_)),
                    "Expected ObjectNotFound error, got {e:?}"
                );
            }
            other => panic!("Expected CouldNotSetObjectParent error, got {other:?}"),
        }
    }

    #[test]
    fn test_reload_reject_descendant_property_conflict() {
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

        // Now try to reload #50 with parent #20
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

        let result =
            parser.reload_single_object(conflict_spec, CompileOptions::default(), None, None);

        // Should fail with property name conflict error
        assert!(
            result.is_err(),
            "Reloading object with descendant property conflict should fail"
        );
        match result.unwrap_err() {
            ObjdefLoaderError::CouldNotSetObjectParent(_, e) => {
                assert!(
                    matches!(
                        e,
                        moor_common::model::WorldStateError::ChparentPropertyNameConflict(_, _, _)
                    ),
                    "Expected ChparentPropertyNameConflict error, got {e:?}"
                );
            }
            other => panic!("Expected CouldNotSetObjectParent error, got {other:?}"),
        }
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
            ObjDefLoaderOptions {
                object_kind: None,
                constants: None,
                validate_parent_changes: true,
            },
        );

        // Should fail with property name conflict error
        assert!(
            result.is_err(),
            "Loading object with descendant property conflict should fail"
        );
        match result.unwrap_err() {
            ObjdefLoaderError::CouldNotSetObjectParent(_, e) => {
                // Verify it's a property name conflict error
                assert!(
                    matches!(
                        e,
                        moor_common::model::WorldStateError::ChparentPropertyNameConflict(_, _, _)
                    ),
                    "Expected ChparentPropertyNameConflict error, got {e:?}"
                );
            }
            other => panic!("Expected CouldNotSetObjectParent error, got {other:?}"),
        }
    }
}
