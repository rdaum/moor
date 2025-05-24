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

use crate::DirDumpReaderError;
use moor_common::model::ObjAttrs;
use moor_common::model::loader::LoaderInterface;
use moor_compiler::{CompileOptions, ObjFileContext, ObjectDefinition, compile_object_definitions};
use moor_var::{NOTHING, Obj};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tracing::info;

pub struct ObjectDefinitionLoader<'a> {
    object_definitions: HashMap<Obj, (PathBuf, ObjectDefinition)>,
    loader: &'a mut dyn LoaderInterface,
}

impl<'a> ObjectDefinitionLoader<'a> {
    pub fn new(loader: &'a mut dyn LoaderInterface) -> Self {
        Self {
            object_definitions: HashMap::new(),
            loader,
        }
    }

    pub fn read_dirdump(
        &mut self,
        compile_options: CompileOptions,
        dirpath: &Path,
    ) -> Result<(), DirDumpReaderError> {
        let start_time = Instant::now();

        // Check that the directory exists
        if !dirpath.exists() {
            return Err(DirDumpReaderError::DirectoryNotFound(dirpath.to_path_buf()));
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
                .map_err(|e| DirDumpReaderError::ObjectFileReadError(constants_file.clone(), e))?;
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
                .map_err(|e| DirDumpReaderError::ObjectFileReadError(object_file.clone(), e))?;

            self.parse_objects(
                &object_file,
                &mut context,
                &object_file_contents,
                &compile_options,
            )?;
        }

        let num_total_verbs = self
            .object_definitions
            .values()
            .map(|(_, d)| d.verbs.len())
            .sum::<usize>();
        let num_total_properties = self
            .object_definitions
            .values()
            .map(|(_, d)| d.property_definitions.len())
            .sum::<usize>();
        let num_total_property_overrides = self
            .object_definitions
            .values()
            .map(|(_, d)| d.property_overrides.len())
            .sum::<usize>();

        info!(
            "Created {} objects. Adjusting inheritance, location, and ownership attributes...",
            self.object_definitions.len()
        );
        self.apply_attributes()?;
        info!("Defining {} properties...", num_total_properties);
        self.define_properties()?;
        info!(
            "Overriding {} property values...",
            num_total_property_overrides
        );
        self.set_properties()?;
        info!("Defining and compiling {} verbs...", num_total_verbs);
        self.define_verbs()?;

        info!(
            "Imported {} objects w/ {} verbs, {} properties and {} property overrides in {} seconds.",
            self.object_definitions.len(),
            num_total_verbs,
            num_total_properties,
            num_total_property_overrides,
            start_time.elapsed().as_secs()
        );

        Ok(())
    }

    fn parse_objects(
        &mut self,
        path: &Path,
        context: &mut ObjFileContext,
        object_file_contents: &str,
        compile_options: &CompileOptions,
    ) -> Result<(), DirDumpReaderError> {
        let compiled_defs =
            compile_object_definitions(object_file_contents, compile_options, context)
                .map_err(|e| DirDumpReaderError::ObjectFileParseError(path.to_path_buf(), e))?;

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
                    DirDumpReaderError::CouldNotCreateObject(path.to_path_buf(), oid, wse)
                })?;

            self.object_definitions
                .insert(oid, (path.to_path_buf(), compiled_def));
        }
        Ok(())
    }

    pub fn apply_attributes(&mut self) -> Result<(), DirDumpReaderError> {
        for (obj, (path, def)) in &self.object_definitions {
            if def.parent != NOTHING {
                self.loader
                    .set_object_parent(obj, &def.parent)
                    .map_err(|e| DirDumpReaderError::CouldNotSetObjectParent(path.clone(), e))?;
            }
            if def.location != NOTHING {
                self.loader
                    .set_object_location(obj, &def.location)
                    .map_err(|e| DirDumpReaderError::CouldNotSetObjectLocation(path.clone(), e))?;
            }
            if def.owner != NOTHING {
                self.loader
                    .set_object_owner(obj, &def.owner)
                    .map_err(|e| DirDumpReaderError::CouldNotSetObjectOwner(path.clone(), e))?;
            }
        }
        Ok(())
    }

    pub fn define_verbs(&mut self) -> Result<(), DirDumpReaderError> {
        for (obj, (path, def)) in &self.object_definitions {
            for v in &def.verbs {
                let names = v.names.iter().map(|s| s.as_str());
                self.loader
                    .add_verb(
                        obj,
                        names.collect(),
                        &v.owner,
                        v.flags,
                        v.argspec,
                        v.program.clone(),
                    )
                    .map_err(|wse| {
                        DirDumpReaderError::CouldNotDefineVerb(
                            path.clone(),
                            *obj,
                            v.names.clone(),
                            wse,
                        )
                    })?;
            }
        }
        Ok(())
    }
    pub fn define_properties(&mut self) -> Result<(), DirDumpReaderError> {
        for (obj, (path, def)) in &self.object_definitions {
            for pd in &def.property_definitions {
                self.loader
                    .define_property(
                        obj,
                        obj,
                        pd.name.as_str(),
                        &pd.perms.owner(),
                        pd.perms.flags(),
                        pd.value.clone(),
                    )
                    .map_err(|wse| {
                        DirDumpReaderError::CouldNotDefineProperty(
                            path.clone(),
                            *obj,
                            pd.name.as_str().to_string(),
                            wse,
                        )
                    })?;
            }
        }

        Ok(())
    }

    fn set_properties(&mut self) -> Result<(), DirDumpReaderError> {
        for (obj, (path, def)) in &self.object_definitions {
            for pv in &def.property_overrides {
                let pu = &pv.perms_update;
                self.loader
                    .set_property(
                        obj,
                        pv.name.as_str(),
                        pu.as_ref().map(|p| p.owner()),
                        pu.as_ref().map(|p| p.flags()),
                        pv.value.clone(),
                    )
                    .map_err(|wse| {
                        DirDumpReaderError::CouldNotOverrideProperty(
                            path.clone(),
                            *obj,
                            pv.name.as_str().to_string(),
                            wse,
                        )
                    })?;
            }
        }
        Ok(())
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

        let o = loader.get_object(&Obj::mk_id(1)).unwrap();
        assert_eq!(o.owner().unwrap(), SYSTEM_OBJECT);
        assert_eq!(o.name().unwrap(), "Test Object");
        assert_eq!(o.parent(), None);
        assert_eq!(o.location(), None);
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

        assert!(v.unwrap().1.names().contains(&"look"));

        let p = ws
            .retrieve_property(&SYSTEM_OBJECT, &Obj::mk_id(2), Symbol::mk("description"))
            .unwrap();
        assert_eq!(p, v_str("This is a generic thing"));
    }
}
