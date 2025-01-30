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

use crate::config::FeaturesConfig;
use crate::objdef::DirDumpReaderError;
use moor_compiler::{compile_object_definitions, CompileOptions, ObjectDefinition};
use moor_db::loader::LoaderInterface;
use moor_values::model::ObjAttrs;
use moor_values::{Obj, NOTHING};
use std::collections::HashMap;
use std::path::Path;
use tracing::info;

pub struct ObjectDefinitionLoader<'a> {
    object_definitions: HashMap<Obj, ObjectDefinition>,
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
        features_config: FeaturesConfig,
        dirpath: &Path,
    ) -> Result<(), DirDumpReaderError> {
        // Check that the directory exists
        if !dirpath.exists() {
            return Err(DirDumpReaderError::DirectoryNotFound(dirpath.to_path_buf()));
        }

        // Read the objects first, going through and creating them all
        let compile_options = features_config.compile_options();
        // Create all the objects first with no attributes, and then update after, so that the
        // inheritance/location etc hierarchy is set up right
        for object_path in dirpath.read_dir().unwrap() {
            let object_file = object_path.map_err(DirDumpReaderError::ObjectFileReadError)?;

            if object_file.path().extension().unwrap() != "moo" {
                continue;
            }

            let object_file_contents = std::fs::read_to_string(object_file.path())
                .map_err(DirDumpReaderError::ObjectFileReadError)?;

            self.parse_objects(&object_file_contents, &compile_options)?;
        }

        self.apply_attributes()?;
        self.define_properties()?;
        self.set_properties()?;
        self.define_verbs()?;

        Ok(())
    }

    fn parse_objects(
        &mut self,
        object_file_contents: &str,
        compile_options: &CompileOptions,
    ) -> Result<(), DirDumpReaderError> {
        let compiled_defs = compile_object_definitions(object_file_contents, compile_options)
            .map_err(DirDumpReaderError::ObjectFileParseError)?;

        let mut total_verbs = 0;
        let mut total_propdefs = 0;

        for compiled_def in compiled_defs {
            let oid = compiled_def.oid.clone();

            total_verbs += compiled_def.verbs.len();
            total_propdefs += compiled_def.property_definitions.len();

            self.loader
                .create_object(
                    Some(oid.clone()),
                    &ObjAttrs::new(
                        NOTHING,
                        NOTHING,
                        NOTHING,
                        compiled_def.flags,
                        &compiled_def.name,
                    ),
                )
                .map_err(|wse| DirDumpReaderError::CouldNotCreateObject(oid.clone(), wse))?;

            self.object_definitions.insert(oid, compiled_def);
        }
        info!(
            "Loaded {} objects with a total of {total_verbs} verbs and {total_propdefs} total properties",
            self.object_definitions.len()
        );

        Ok(())
    }

    pub fn apply_attributes(&mut self) -> Result<(), DirDumpReaderError> {
        for (obj, def) in &self.object_definitions {
            if def.parent != NOTHING {
                self.loader
                    .set_object_parent(obj, &def.parent)
                    .map_err(DirDumpReaderError::CouldNotSetObjectParent)?;
            }
            if def.location != NOTHING {
                self.loader
                    .set_object_location(obj, &def.location)
                    .map_err(DirDumpReaderError::CouldNotSetObjectLocation)?;
            }
            if def.owner != NOTHING {
                self.loader
                    .set_object_owner(obj, &def.owner)
                    .map_err(DirDumpReaderError::CouldNotSetObjectOwner)?;
            }
        }
        Ok(())
    }

    pub fn define_verbs(&mut self) -> Result<(), DirDumpReaderError> {
        for (obj, def) in &self.object_definitions {
            for v in &def.verbs {
                let names = v.names.iter().map(|s| s.as_str());
                self.loader
                    .add_verb(
                        obj,
                        names.collect(),
                        &v.owner,
                        v.flags,
                        v.argspec,
                        v.binary.to_vec(),
                    )
                    .map_err(|wse| {
                        DirDumpReaderError::CouldNotDefineVerb(obj.clone(), v.names.clone(), wse)
                    })?;
            }
        }
        Ok(())
    }
    pub fn define_properties(&mut self) -> Result<(), DirDumpReaderError> {
        for (obj, def) in &self.object_definitions {
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
                            obj.clone(),
                            pd.name.as_str().to_string(),
                            wse,
                        )
                    })?;
            }
        }

        Ok(())
    }

    fn set_properties(&mut self) -> Result<(), DirDumpReaderError> {
        for (obj, def) in &self.object_definitions {
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
                            obj.clone(),
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
    use crate::objdef::load::ObjectDefinitionLoader;
    use moor_compiler::CompileOptions;
    use moor_db::{Database, DatabaseConfig, TxDB};
    use moor_values::model::{Named, PrepSpec, WorldStateSource};
    use moor_values::{v_str, Obj, Symbol, NOTHING, SYSTEM_OBJECT};
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
        parser
            .parse_objects(spec, &CompileOptions::default())
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

        parser
            .parse_objects(spec, &CompileOptions::default())
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
