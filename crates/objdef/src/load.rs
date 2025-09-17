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
use moor_common::model::loader::LoaderInterface;
use moor_common::model::{ObjAttrs, ObjFlag, PropDef, PropFlag, VerbDef};
use moor_compiler::{CompileOptions, ObjFileContext, ObjectDefinition, compile_object_definitions};
use moor_var::program::ProgramType;
use moor_var::{NOTHING, Obj, Symbol, Var};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tracing::info;
use uuid::Uuid;

pub struct ObjectDefinitionLoader<'a> {
    object_definitions: HashMap<Obj, (PathBuf, ObjectDefinition)>,
    loader: &'a mut dyn LoaderInterface,
}

/// How to handle a situation where:
///     * Object already exists
///     * Provided flags or builtin-props differ from loaded objdef file
///     * Parentage differs from loaded objdef file
///     * An existing defined property differs in value or flags from loaded objdef file
///     * An existing overridden property differs in value or flags from loaded objdef file
///     * A verb differs in flags or content from loaded objdef file
#[derive(Debug)]
pub enum ConflictMode {
    /// Indiscriminately overwrite the existing entity with the new value.
    Clobber,
    /// Skip all conflicts entirely and only add new verbs and properties that do not conflict.
    Skip,
}

/// Entities for which we can give instructions for overrides and removals.
#[derive(Debug)]
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

#[derive(Debug)]
pub struct ObjDefLoaderOptions {
    /// True if we're running in "dry-run" mode where we test, and collect conflicts.
    pub dry_run: bool,
    /// How to handle conflicts.
    pub conflict_mode: ConflictMode,
    /// The set of entities for which we will allow overriding and treat as if their specific
    /// ConflicTMode was "Clobber"
    pub overrides: Vec<(Obj, Entity)>,
    /// The set of entities which we will consider value for deletion
    /// Note that flags, builtin props and parentage are not valid values here.
    pub removals: Vec<(Obj, Entity)>,
}

#[derive(Debug)]
pub enum ConflictEntity {
    ObjectFlags(ObjFlag),
    BuiltinProps(Symbol, Var),
    Parentage(Obj),
    PropertyDef(Symbol, PropDef),
    PropertyValue(Symbol, Var),
    PropertyFlag(Symbol, PropFlag),
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
        }
    }

    /// Read an entire directory of objdef files, along with `constants.moo`, process them, and
    /// load them into the database.
    pub fn load_objdef_directory(
        &mut self,
        compile_options: CompileOptions,
        dirpath: &Path,
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
        self.apply_attributes()?;
        info!("Defining {} properties...", num_loaded_property_definitions);
        self.define_properties()?;
        info!(
            "Overriding {} property values...",
            num_loaded_property_overrides
        );
        self.set_properties()?;
        info!("Defining and compiling {} verbs...", num_loaded_verbs);
        self.define_verbs()?;

        Ok(ObjDefLoaderResults {
            commit: false,
            conflicts: vec![],
            removals: vec![],
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
                    Some(oid),
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

    pub fn apply_attributes(&mut self) -> Result<(), ObjdefLoaderError> {
        for (obj, (path, def)) in &self.object_definitions {
            if def.parent != NOTHING {
                self.loader
                    .set_object_parent(obj, &def.parent)
                    .map_err(|e| {
                        ObjdefLoaderError::CouldNotSetObjectParent(
                            path.to_string_lossy().to_string(),
                            e,
                        )
                    })?;
            }
            if def.location != NOTHING {
                self.loader
                    .set_object_location(obj, &def.location)
                    .map_err(|e| {
                        ObjdefLoaderError::CouldNotSetObjectLocation(
                            path.to_string_lossy().to_string(),
                            e,
                        )
                    })?;
            }
            if def.owner != NOTHING {
                self.loader.set_object_owner(obj, &def.owner).map_err(|e| {
                    ObjdefLoaderError::CouldNotSetObjectOwner(path.to_string_lossy().to_string(), e)
                })?;
            }
        }
        Ok(())
    }

    pub fn define_verbs(&mut self) -> Result<(), ObjdefLoaderError> {
        for (obj, (path, def)) in &self.object_definitions {
            for v in &def.verbs {
                self.loader
                    .add_verb(
                        obj,
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
        }
        Ok(())
    }
    pub fn define_properties(&mut self) -> Result<(), ObjdefLoaderError> {
        for (obj, (path, def)) in &self.object_definitions {
            for pd in &def.property_definitions {
                self.loader
                    .define_property(
                        obj,
                        obj,
                        pd.name,
                        &pd.perms.owner(),
                        pd.perms.flags(),
                        pd.value.clone(),
                    )
                    .map_err(|wse| {
                        ObjdefLoaderError::CouldNotDefineProperty(
                            path.to_string_lossy().to_string(),
                            *obj,
                            (*pd.name.as_arc_string()).clone(),
                            wse,
                        )
                    })?;
            }
        }

        Ok(())
    }

    fn set_properties(&mut self) -> Result<(), ObjdefLoaderError> {
        for (obj, (path, def)) in &self.object_definitions {
            for pv in &def.property_overrides {
                let pu = &pv.perms_update;
                self.loader
                    .set_property(
                        obj,
                        pv.name,
                        pu.as_ref().map(|p| p.owner()),
                        pu.as_ref().map(|p| p.flags()),
                        pv.value.clone(),
                    )
                    .map_err(|wse| {
                        ObjdefLoaderError::CouldNotOverrideProperty(
                            path.to_string_lossy().to_string(),
                            *obj,
                            (*pv.name.as_arc_string()).clone(),
                            wse,
                        )
                    })?;
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
        target_object: Option<moor_var::Obj>,
        constants: Option<moor_var::Map>,
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
        let oid = if let Some(target_obj) = target_object {
            if !target_obj.is_positive() {
                // Negative numeric object ID means allocate next available (max + 1)
                let max_obj = self.loader.max_object().map_err(|e| {
                    ObjdefLoaderError::ObjectDefParseError(
                        source_name.clone(),
                        moor_compiler::ObjDefParseError::ConstantNotFound(format!(
                            "Failed to get max object: {e}"
                        )),
                    )
                })?;
                Obj::mk_id(max_obj.id().0 + 1)
            } else {
                target_obj
            }
        } else {
            compiled_def.oid
        };

        // Create the object (using target_object override if provided)
        self.loader
            .create_object(
                Some(oid),
                &ObjAttrs::new(
                    NOTHING,
                    NOTHING,
                    NOTHING,
                    compiled_def.flags,
                    &compiled_def.name,
                ),
            )
            .map_err(|wse| {
                ObjdefLoaderError::CouldNotCreateObject(source_name.clone(), oid, wse)
            })?;

        // Store the definition for processing
        self.object_definitions
            .insert(oid, (PathBuf::from(&source_name), compiled_def));

        // Apply attributes
        if let Some((_, def)) = self.object_definitions.get(&oid) {
            if def.parent != NOTHING {
                self.loader
                    .set_object_parent(&oid, &def.parent)
                    .map_err(|e| {
                        ObjdefLoaderError::CouldNotSetObjectParent(source_name.clone(), e)
                    })?;
            }
            if def.location != NOTHING {
                self.loader
                    .set_object_location(&oid, &def.location)
                    .map_err(|e| {
                        ObjdefLoaderError::CouldNotSetObjectLocation(source_name.clone(), e)
                    })?;
            }
            if def.owner != NOTHING {
                self.loader
                    .set_object_owner(&oid, &def.owner)
                    .map_err(|e| {
                        ObjdefLoaderError::CouldNotSetObjectOwner(source_name.clone(), e)
                    })?;
            }
        }

        // Define properties
        if let Some((_, def)) = self.object_definitions.get(&oid) {
            for pd in &def.property_definitions {
                self.loader
                    .define_property(
                        &oid,
                        &oid,
                        pd.name,
                        &pd.perms.owner(),
                        pd.perms.flags(),
                        pd.value.clone(),
                    )
                    .map_err(|wse| {
                        ObjdefLoaderError::CouldNotDefineProperty(
                            source_name.clone(),
                            oid,
                            (*pd.name.as_arc_string()).clone(),
                            wse,
                        )
                    })?;
            }
        }

        // Set property overrides
        if let Some((_, def)) = self.object_definitions.get(&oid) {
            for pv in &def.property_overrides {
                let pu = &pv.perms_update;
                self.loader
                    .set_property(
                        &oid,
                        pv.name,
                        pu.as_ref().map(|p| p.owner()),
                        pu.as_ref().map(|p| p.flags()),
                        pv.value.clone(),
                    )
                    .map_err(|wse| {
                        ObjdefLoaderError::CouldNotOverrideProperty(
                            source_name.clone(),
                            oid,
                            (*pv.name.as_arc_string()).clone(),
                            wse,
                        )
                    })?;
            }
        }

        // Define verbs
        if let Some((_, def)) = self.object_definitions.get(&oid) {
            for v in &def.verbs {
                self.loader
                    .add_verb(
                        &oid,
                        &v.names,
                        &v.owner,
                        v.flags,
                        v.argspec,
                        v.program.clone(),
                    )
                    .map_err(|wse| {
                        ObjdefLoaderError::CouldNotDefineVerb(
                            source_name.clone(),
                            oid,
                            v.names.clone(),
                            wse,
                        )
                    })?;
            }
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
            "Loaded single object {} in {} ms",
            oid,
            start_time.elapsed().as_millis()
        );

        Ok(ObjDefLoaderResults {
            commit: true,
            conflicts: vec![],
            removals: vec![],
            loaded_objects: vec![oid],
            num_loaded_verbs,
            num_loaded_property_definitions,
            num_loaded_property_overrides,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::ObjectDefinitionLoader;
    use moor_common::model::{Named, PrepSpec, WorldStateSource};
    use moor_compiler::{CompileOptions, ObjFileContext};
    use moor_db::{Database, DatabaseConfig, TxDB};
    use moor_var::{NOTHING, Obj, SYSTEM_OBJECT, Symbol, v_str};
    use std::path::Path;
    use std::sync::Arc;

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

        parser.apply_attributes().unwrap();
        parser.define_verbs().unwrap();
        parser.define_properties().unwrap();
        parser.set_properties().unwrap();

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
        parser.apply_attributes().unwrap();
        parser.define_verbs().unwrap();
        parser.define_properties().unwrap();
        parser.set_properties().unwrap();
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

        let results = parser
            .load_single_object(spec, CompileOptions::default(), None, None)
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

        let result = parser.load_single_object(spec, CompileOptions::default(), None, None);
        assert!(result.is_err());

        match result.unwrap_err() {
            crate::ObjdefLoaderError::SingleObjectExpected(_, count) => {
                assert_eq!(count, 2);
            }
            _ => panic!("Expected SingleObjectExpected error"),
        }
    }
}
