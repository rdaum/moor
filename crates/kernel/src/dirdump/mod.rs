use crate::config::FeaturesConfig;
use moor_compiler::{compile, CompileOptions};
use moor_db::loader::LoaderInterface;
use moor_values::model::{
    ArgSpec, CompileError, ObjAttrs, ObjFlag, PrepSpec, VerbArgsSpec, VerbFlag, WorldStateError,
};
use moor_values::util::BitEnum;
use moor_values::{AsByteBuffer, Obj, NOTHING};
use regex::Regex;
use semver::Version;
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::str::Lines;

const VERB_LINE_REGEX: &str = r#""verb\((?P<verb_names>[^)]+)\):\s*argspec\((?P<argspec>\w+\s\w+\s\w+)\),\s*owner:\s*(?P<owner>#\d+),\s*flags:\s*(?P<flags>\w+)"#;

#[derive(Debug, thiserror::Error)]
pub enum DirDumpReaderError {
    #[error("Directory not found: {0}")]
    DirectoryNotFound(PathBuf),
    #[error("Invalid object file name: {0} (should be number)")]
    InvalidObjectFilename(String),
    #[error("Error reading object file: {0}")]
    ObjectFileReadError(#[from] io::Error),
    #[error("Bad object format")]
    BadObjectFormat,
    #[error("Missing attribute: {0}")]
    MissingAttribute(String),
    #[error("Invalid attribute: {0}: ({1})")]
    InvalidAttribute(String, String),
    #[error("Could not create object: {0}")]
    CouldNotCreateObject(Obj, WorldStateError),
    #[error("Could not set object parent: {0}")]
    CouldNotSetObjectParent(WorldStateError),
    #[error("Could not set object location: {0}")]
    CouldNotSetObjectLocation(WorldStateError),
    #[error("Could not set object owner: {0}")]
    CouldNotSetObjectOwner(WorldStateError),
    #[error("Could not read verb file: {0}")]
    CouldNotReadVerbFile(io::Error),
    #[error("Invalid verb file, missing {0} attr: {1}")]
    MissingVerbAttr(String, PathBuf),
    #[error("Invalid verb specification: {0}")]
    BadVerbFormat(String),
    #[error("Invalid verb file, bad verb flags spec: {0}")]
    InvalidVerbFlagStr(String),
    #[error("Invalid verb file, bad verb owner: {0}")]
    BadVerbOwner(String),
    #[error("Verb compilation error: {0}")]
    VerbCompileError(CompileError),
    #[error("Invalid verb argument spec: {0}")]
    InvalidVerbArgs(String),
}

fn parse_object_obj_attribute(
    object_data: &HashMap<String, String>,
    attribute: &str,
) -> Result<Obj, DirDumpReaderError> {
    let object_id = object_data
        .get(attribute)
        .ok_or_else(|| DirDumpReaderError::MissingAttribute(attribute.to_string()))?;
    let mut oparts = object_id.split("#");
    oparts.next().ok_or(DirDumpReaderError::BadObjectFormat)?;
    let Some(object_id) = oparts.next() else {
        return Err(DirDumpReaderError::BadObjectFormat);
    };
    let object_id = object_id.parse::<i32>().map_err(|_| {
        DirDumpReaderError::InvalidAttribute(attribute.to_string(), object_id.to_string())
    })?;
    Ok(Obj::mk_id(object_id))
}

fn parse_bool_attribute(
    object_data: &HashMap<String, String>,
    attribute: &str,
) -> Result<bool, DirDumpReaderError> {
    let value = object_data
        .get(attribute)
        .ok_or_else(|| DirDumpReaderError::MissingAttribute(attribute.to_string()))?;
    match value.as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(DirDumpReaderError::InvalidAttribute(
            attribute.to_string(),
            value.clone(),
        )),
    }
}

struct Attrs {
    owner: Obj,
    parent: Obj,
    location: Obj,
}

struct ObjectsParser<'a> {
    verb_re: Regex,
    object_attributes: HashMap<Obj, Attrs>,
    loader: &'a mut dyn LoaderInterface,
}

impl<'a> ObjectsParser<'a> {
    pub fn new(loader: &'a mut dyn LoaderInterface) -> Self {
        Self {
            verb_re: Regex::new(VERB_LINE_REGEX).unwrap(),
            object_attributes: HashMap::new(),
            loader,
        }
    }

    pub fn read_dirdump<T: io::Read>(
        &mut self,
        moo_version: Version,
        features_config: FeaturesConfig,
        dirpath: &Path,
    ) -> Result<(), DirDumpReaderError> {
        // Check that the directory exists
        if !dirpath.exists() {
            return Err(DirDumpReaderError::DirectoryNotFound(dirpath.to_path_buf()));
        }

        // Read the objects first, going through and creating them all
        let objects_path = dirpath.join("objects");
        if !objects_path.exists() {
            return Err(DirDumpReaderError::DirectoryNotFound(objects_path));
        }

        let vere = Regex::new(VERB_LINE_REGEX).unwrap();

        // Create all the objects first with no attributes, and then update after, so that the
        // inheritance/location etc hierarchy is set up right
        for object_path in objects_path.read_dir().unwrap() {
            let object_file =
                object_path.map_err(|e| DirDumpReaderError::ObjectFileReadError(e))?;

            let object_file_contents = std::fs::read_to_string(object_file.path())
                .map_err(|e| DirDumpReaderError::ObjectFileReadError(e))?;

            let object_id = object_file.file_name().to_string_lossy().to_string();

            // object id should be a u32.
            let object_id = object_id
                .parse::<u32>()
                .map_err(|_| DirDumpReaderError::InvalidObjectFilename(object_id.clone()))?;
            let object_id = Obj::mk_id(object_id as i32);

            self.parse_object(&object_file_contents, object_id)?;
        }

        self.apply_attributes()?;

        Ok(())
    }

    fn parse_object(
        &mut self,
        object_file_contents: &str,
        object_id: Obj,
    ) -> Result<(), DirDumpReaderError> {
        // Header contents of file is a series of key-value pairs colon-separated, then a new line.
        let mut object_data_lines = object_file_contents.lines();
        let mut object_data = HashMap::new();
        loop {
            let Some(line) = object_data_lines.next() else {
                break;
            };

            let line = line.trim();
            if line == "" {
                break;
            }

            let mut parts = line.splitn(2, ':');

            let Some(key) = parts.next() else {
                return Err(DirDumpReaderError::BadObjectFormat);
            };
            let Some(value) = parts.next() else {
                return Err(DirDumpReaderError::BadObjectFormat);
            };
            object_data.insert(key.trim().to_string(), value.trim().to_string());
        }

        let parent = parse_object_obj_attribute(&object_data, "parent")?;
        let owner = parse_object_obj_attribute(&object_data, "owner")?;
        let location = parse_object_obj_attribute(&object_data, "location")?;

        self.object_attributes.insert(
            object_id.clone(),
            Attrs {
                owner,
                parent,
                location,
            },
        );

        let name = object_data
            .get("name")
            .ok_or_else(|| DirDumpReaderError::MissingAttribute("name".to_string()))?
            .to_string();

        let wizard = parse_bool_attribute(&object_data, "wizard")?;
        let programmer = parse_bool_attribute(&object_data, "programmer")?;
        let fertile = parse_bool_attribute(&object_data, "fertile")?;
        let player = parse_bool_attribute(&object_data, "player")?;

        let mut flags = BitEnum::new();
        if wizard {
            flags.set(ObjFlag::Wizard);
        }
        if programmer {
            flags.set(ObjFlag::Programmer);
        }
        if fertile {
            flags.set(ObjFlag::Fertile);
        }
        if player {
            flags.set(ObjFlag::User);
        }

        self.loader
            .create_object(
                Some(object_id.clone()),
                &ObjAttrs::new(NOTHING, NOTHING, NOTHING, flags, &name),
            )
            .map_err(|e| DirDumpReaderError::CouldNotCreateObject(object_id.clone(), e))?;

        // Now we can parse each section which is either a verb, or a property.

        loop {
            let Some(next_line) = object_data_lines.next() else {
                break;
            };
            let next_line = next_line.trim();

            if next_line.starts_with("verb") {
                self.process_verb_section(next_line, &mut object_data_lines)?;
            } else if next_line.starts_with("define property") {
            } else if next_line.starts_with("set property") {
            } else if next_line.trim() == "" {
                continue;
            }
        }
        Ok(())
    }

    fn apply_attributes(&mut self) -> Result<(), DirDumpReaderError> {
        for (obj, attr) in &self.object_attributes {
            self.loader
                .set_object_parent(&obj, &attr.parent)
                .map_err(DirDumpReaderError::CouldNotSetObjectParent)?;
            self.loader
                .set_object_location(&obj, &attr.location)
                .map_err(DirDumpReaderError::CouldNotSetObjectLocation)?;
            self.loader
                .set_object_owner(&obj, &attr.owner)
                .map_err(DirDumpReaderError::CouldNotSetObjectOwner)?;
        }
        Ok(())
    }

    fn process_verb_section<'b>(
        &self,
        spec: &str,
        object_data_lines: &mut Lines<'b>,
    ) -> Result<(), DirDumpReaderError> {
        let Some(captures) = self.verb_re.captures(spec) else {
            return Err(DirDumpReaderError::BadVerbFormat(spec.to_string()));
        };

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::dirdump::ObjectsParser;
    use moor_db::{Database, DatabaseConfig, TxDB};
    use moor_values::{Obj, SYSTEM_OBJECT};
    use std::path::Path;
    use std::sync::Arc;

    fn test_db(path: &Path) -> Arc<TxDB> {
        Arc::new(TxDB::open(Some(path), DatabaseConfig::default()).0)
    }

    #[test]
    fn test_simple_obj_parse() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());
        let mut loader = db.loader_client().unwrap();

        let mut parser = ObjectsParser::new(loader.as_mut());

        let spec = r#"parent: #1
        owner: #2
        name: Test Object
        location: #3
        wizard: false
        programmer: false
        player: false
        fertile: true
        "#;
        parser.parse_object(spec, SYSTEM_OBJECT).unwrap();

        parser.apply_attributes().unwrap();

        let o = loader.get_object(&SYSTEM_OBJECT).unwrap();
        assert_eq!(o.owner().unwrap(), Obj::mk_id(2));
        assert_eq!(o.name().unwrap(), "Test Object");
        assert_eq!(o.parent().unwrap(), Obj::mk_id(1));
        assert_eq!(o.location().unwrap(), Obj::mk_id(3));
    }

    #[test]
    fn test_simple_obj_verb_parse() {
        let tmpdir = tempfile::tempdir().unwrap();
        let db = test_db(tmpdir.path());
        let mut loader = db.loader_client().unwrap();

        let mut parser = ObjectsParser::new(loader.as_mut());

        let spec = r#"parent: #1
        owner: #2
        name: Test Object
        location: #3
        wizard: false
        programmer: false
        player: false
        fertile: true

        verb(look_self, look_*): argsec(any to none), owner: #2, flags: rxd
        return 5;

        "#;
        parser.parse_object(spec, SYSTEM_OBJECT).unwrap();

        parser.apply_attributes().unwrap();

        let o = loader.get_object(&SYSTEM_OBJECT).unwrap();
        assert_eq!(o.owner().unwrap(), Obj::mk_id(2));
        assert_eq!(o.name().unwrap(), "Test Object");
        assert_eq!(o.parent().unwrap(), Obj::mk_id(1));
        assert_eq!(o.location().unwrap(), Obj::mk_id(3));
    }
}
