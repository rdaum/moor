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

//! Parsing and staging model for sets of objdef sources.
//!
//! This module one or more objdef texts, plus optional constants, into a proposed object graph that
//! later code can either inspect or apply.
//! Directory import uses before handing the graph to `ObjectDefinitionLoader`.
//!
//! Keep parsing, constants resolution, duplicate detection, and incoming identity derivation here.
//! Keep database effects in `load.rs`.

use crate::{Constants, ObjdefLoaderError};
use moor_compiler::{CompileOptions, ObjFileContext, ObjectDefinition, compile_object_definitions};
use moor_var::{Obj, Symbol, Var};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
/// One objdef input unit with a stable diagnostic label.
///
/// `path` is optional because callers may supply objdefs from memory rather than from a directory.
/// When present, it is used for include path resolution and for diagnostics. When absent, `label`
/// is used only for diagnostics and grants no filesystem access.
pub struct ObjDefSource {
    /// Human-readable source name for parse errors and duplicate diagnostics.
    pub label: String,
    /// Raw objdef text.
    pub contents: String,
    /// Filesystem path, when the source came from disk.
    pub path: Option<PathBuf>,
}

impl ObjDefSource {
    /// Build an in-memory objdef source.
    pub fn new(label: impl Into<String>, contents: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            contents: contents.into(),
            path: None,
        }
    }

    /// Build an objdef source read from a concrete path.
    pub fn from_path(path: PathBuf, contents: String) -> Self {
        Self {
            label: path.to_string_lossy().into_owned(),
            contents,
            path: Some(path),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
/// Stable incoming identity discovered while parsing an objdef set.
///
/// Constants are the symbolic names from `constants.moo` or supplied constants. `import_export_id`
/// is the stable exported metadata used by mooR's objdef dump format. Both are diagnostic identity
/// layers; object IDs are still interpreted literally in this phase.
pub struct ObjDefIdentity {
    /// Constant name that resolves to the object, when one exists in the incoming constants.
    pub constant: Option<Symbol>,
    /// `import_export_id` metadata value declared on the object, when present.
    pub import_export_id: Option<String>,
}

#[derive(Clone)]
/// Parsed incoming object graph before any database mutation.
///
/// The graph owns compiled object definitions keyed by their literal object IDs, plus identity
/// metadata derived from constants and `import_export_id`. It is the common input for both directory
/// import apply and future changelist analysis.
pub struct ProposedObjectGraph {
    object_definitions: HashMap<Obj, (String, ObjectDefinition)>,
    identities: HashMap<Obj, ObjDefIdentity>,
}

impl ProposedObjectGraph {
    /// Compiled object definitions keyed by the object ID named in the objdef text.
    pub fn object_definitions(&self) -> &HashMap<Obj, (String, ObjectDefinition)> {
        &self.object_definitions
    }

    /// Incoming identity metadata for one object, if any was discovered.
    pub fn identity(&self, obj: &Obj) -> Option<&ObjDefIdentity> {
        self.identities.get(obj)
    }

    /// All discovered incoming identities keyed by object ID.
    pub fn identities(&self) -> &HashMap<Obj, ObjDefIdentity> {
        &self.identities
    }
}

/// Parsed objdef set plus constants produced while parsing it.
///
/// `ObjDefSet` is read-only with respect to the database. It validates constants, parses all sources
/// through one `ObjFileContext`, detects duplicate object IDs, and derives incoming identity
/// metadata. Applying the result is a separate loader concern.
#[derive(Clone)]
pub struct ObjDefSet {
    graph: ProposedObjectGraph,
    constants: HashMap<Symbol, Var>,
}

impl ObjDefSet {
    /// Read a bulk source directory with the same constants and include rules as import.
    pub fn read_directory(
        compile_options: &CompileOptions,
        directory: &Path,
    ) -> Result<Self, ObjdefLoaderError> {
        fn collect(path: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
            for entry in std::fs::read_dir(path)? {
                let path = entry?.path();
                if path.is_dir() {
                    collect(&path, files)?;
                } else if path.is_file() && path.extension().is_some_and(|ext| ext == "moo") {
                    files.push(path);
                }
            }
            Ok(())
        }
        if !directory.is_dir() {
            return Err(ObjdefLoaderError::DirectoryNotFound(
                directory.to_path_buf(),
            ));
        }
        let mut files = Vec::new();
        collect(directory, &mut files)
            .map_err(|e| ObjdefLoaderError::ObjectFileReadError(directory.to_path_buf(), e))?;
        files.sort();
        let constants = directory.join("constants.moo");
        files.retain(|path| path.file_name().is_some_and(|name| name != "constants.moo"));
        if constants.is_file() {
            files.insert(0, constants);
        }
        let sources = files
            .into_iter()
            .map(|path| {
                let text = std::fs::read_to_string(&path)
                    .map_err(|e| ObjdefLoaderError::ObjectFileReadError(path.clone(), e))?;
                Ok(ObjDefSource::from_path(path, text))
            })
            .collect::<Result<Vec<_>, ObjdefLoaderError>>()?;
        let mut options = compile_options.clone();
        options.call_unsupported_builtins = true;
        Self::parse_sources(&options, Some(directory), None, sources)
    }

    /// Read deployment source, optionally preparing baselines from separate upstream history.
    /// Git preparation uses local history only and never modifies the working tree.
    pub fn read_directory_with_baseline(
        compile_options: &CompileOptions,
        directory: &Path,
        git_upstream: Option<&str>,
        baseline_directory: Option<&Path>,
    ) -> Result<Self, ObjdefLoaderError> {
        if git_upstream.is_some() && baseline_directory.is_some() {
            return Err(ObjdefLoaderError::InvalidBaseline(
                "choose either --git-upstream or --baseline-objdef-dir".into(),
            ));
        }
        let baseline = if let Some(upstream) = git_upstream {
            let (base, provenance) =
                crate::git_baseline::prepare(directory, upstream, compile_options)
                    .map_err(|e| ObjdefLoaderError::InvalidBaseline(format!("{e:#}")))?;
            Some((base, Some(provenance)))
        } else if let Some(path) = baseline_directory {
            Some((Self::read_directory(compile_options, path)?, None))
        } else {
            None
        };
        let local = Self::read_directory(compile_options, directory)?;
        match baseline {
            Some((base, provenance)) => local.with_baseline(&base, provenance),
            None => Ok(local),
        }
    }

    /// Derive program, property, and attribute baselines from separately identified source.
    /// Unmatched local declarations have no baseline. Object relocation is not inferred.
    pub fn with_baseline(
        mut self,
        baseline: &Self,
        provenance: Option<Var>,
    ) -> Result<Self, ObjdefLoaderError> {
        use crate::{
            fingerprint::{PROGRAM_SCHEMA, program_fingerprint},
            review::{BASE_KEY, record},
        };
        use moor_var::v_str;
        if provenance.as_ref().is_some_and(|v| v.as_map().is_none()) {
            return Err(ObjdefLoaderError::InvalidBaseline(
                "source provenance must be a map".into(),
            ));
        }
        let key = Symbol::mk(BASE_KEY);
        for (oid, (label, local)) in &mut self.graph.object_definitions {
            let identity = self.graph.identities.get(oid).cloned().unwrap_or_default();
            if let Some(id) = &identity.import_export_id {
                let candidates = baseline
                    .graph
                    .identities
                    .iter()
                    .filter(|(_, identity)| identity.import_export_id.as_ref() == Some(id))
                    .map(|(oid, _)| oid)
                    .collect::<Vec<_>>();
                if candidates.len() > 1
                    || candidates.first().is_some_and(|base_oid| *base_oid != oid)
                {
                    return Err(ObjdefLoaderError::InvalidBaseline(format!(
                        "{label}: object identity {id} is ambiguous or has moved; explicit rebinding is required"
                    )));
                }
            }
            let base = baseline
                .graph
                .object_definitions
                .get(oid)
                .map(|(_, definition)| definition);
            if base.is_some() {
                let base_identity = baseline.graph.identity(oid).cloned().unwrap_or_default();
                if identity.import_export_id != base_identity.import_export_id {
                    return Err(ObjdefLoaderError::InvalidBaseline(format!(
                        "{label}: object {oid} has a different identity in the baseline"
                    )));
                }
            }
            local
                .metadata
                .retain(|(metadata_key, _)| *metadata_key != key);
            if let Some(base) = base {
                local.metadata.push((
                    key,
                    crate::review::inspection::baseline(base, &baseline.graph, provenance.as_ref()),
                ));
            }
            for (index, verb) in local.verbs.iter().enumerate() {
                if local.verbs[..index]
                    .iter()
                    .any(|other| other.names == verb.names && other.argspec == verb.argspec)
                {
                    return Err(ObjdefLoaderError::InvalidBaseline(format!(
                        "{label}: ambiguous local verb declaration"
                    )));
                }
            }
            for verb in &mut local.verbs {
                verb.metadata
                    .retain(|(metadata_key, _)| *metadata_key != key);
                let Some(base) = base else {
                    continue;
                };
                let candidates = base
                    .verbs
                    .iter()
                    .filter(|candidate| {
                        candidate.names == verb.names && candidate.argspec == verb.argspec
                    })
                    .collect::<Vec<_>>();
                if candidates.len() > 1 {
                    return Err(ObjdefLoaderError::InvalidBaseline(format!(
                        "{label}: ambiguous verb declaration in baseline binding"
                    )));
                }
                let Some(base_verb) = candidates.first() else {
                    continue;
                };
                let hash = program_fingerprint(&base_verb.program)
                    .map_err(ObjdefLoaderError::InvalidBaseline)?;
                let mut fields = vec![("schema", v_str(PROGRAM_SCHEMA)), ("program", v_str(&hash))];
                if let Some(source) = &provenance {
                    fields.push(("source", source.clone()));
                }
                verb.metadata.push((key, record(&fields)));
            }
        }
        Ok(self)
    }

    /// Parse objdef sources into a proposed graph without mutating the database.
    ///
    /// `root_path` is the include security boundary for filesystem-backed source sets. `constants`
    /// are applied before `sources`, so callers can supply constants without manufacturing a
    /// `constants.moo` source. Sources may also contain `define` declarations; all definitions share
    /// one context so constants work across files the same way they do in directory import.
    pub fn parse_sources<I>(
        compile_options: &CompileOptions,
        root_path: Option<&Path>,
        constants: Option<&Constants>,
        sources: I,
    ) -> Result<Self, ObjdefLoaderError>
    where
        I: IntoIterator<Item = ObjDefSource>,
    {
        let mut context = ObjFileContext::new();
        if let Some(root_path) = root_path {
            context.set_root_path(root_path);
        }

        if let Some(constants) = constants {
            apply_constants(constants, compile_options, &mut context, "<constants>")?;
        }

        let mut object_definitions: HashMap<Obj, (String, ObjectDefinition)> = HashMap::new();
        let mut memory_bytes = 0usize;
        let mut memory_units = 0usize;
        for source in sources {
            if source.path.is_none() {
                memory_units += 1;
                memory_bytes = memory_bytes.saturating_add(source.contents.len());
                if memory_units > 4096 || memory_bytes > 16 * 1024 * 1024 {
                    return Err(ObjdefLoaderError::InputLimit("4096 units or 16 MiB".into()));
                }
            }
            match source.path.as_deref() {
                Some(path) => context.set_base_path(path),
                None => context.clear_base_path(),
            }
            let compiled_defs = compile_normalized_object_definitions(
                &source.contents,
                compile_options,
                &mut context,
            )
            .map_err(|e| {
                ObjdefLoaderError::ObjectDefParseError(source.label.clone(), Box::new(e))
            })?;

            for compiled_def in compiled_defs {
                let oid = compiled_def.oid;
                if let Some((first_source, _)) = object_definitions.get(&oid) {
                    return Err(ObjdefLoaderError::DuplicateObjectDefinition(
                        source.label.clone(),
                        oid,
                        first_source.clone(),
                    ));
                }
                object_definitions.insert(oid, (source.label.clone(), compiled_def));
                if source.path.is_none() && object_definitions.len() > 32768 {
                    return Err(ObjdefLoaderError::InputLimit(
                        "32768 object declarations".into(),
                    ));
                }
            }
        }

        let constants = context.constants().clone();
        let identities = derive_identities(&object_definitions, &constants);
        Ok(Self {
            graph: ProposedObjectGraph {
                object_definitions,
                identities,
            },
            constants,
        })
    }

    /// Proposed graph built from the incoming sources.
    pub fn graph(&self) -> &ProposedObjectGraph {
        &self.graph
    }

    /// Constants accumulated while parsing the set.
    pub fn constants(&self) -> &HashMap<Symbol, Var> {
        &self.constants
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        HashMap<Obj, (String, ObjectDefinition)>,
        HashMap<Symbol, Var>,
    ) {
        (self.graph.object_definitions, self.constants)
    }
}

pub(crate) fn normalize_legacy_naming_properties(definition: &mut ObjectDefinition) {
    for key in [crate::import_export_id(), crate::import_export_hierarchy()] {
        let has_metadata = definition
            .metadata
            .iter()
            .any(|(metadata_key, _)| *metadata_key == key);
        let mut legacy_value = None;

        definition.property_definitions = std::mem::take(&mut definition.property_definitions)
            .into_iter()
            .filter_map(|property| {
                if property.name != key {
                    return Some(property);
                }
                if legacy_value.is_none() {
                    legacy_value = property.value;
                }
                None
            })
            .collect();
        definition.property_overrides = std::mem::take(&mut definition.property_overrides)
            .into_iter()
            .filter_map(|property| {
                if property.name != key {
                    return Some(property);
                }
                if property.value.is_some() {
                    legacy_value = property.value;
                }
                None
            })
            .collect();

        if !has_metadata && let Some(value) = legacy_value {
            definition.metadata.push((key, value));
        }
    }
}

pub(crate) fn compile_normalized_object_definitions(
    source: &str,
    compile_options: &CompileOptions,
    context: &mut ObjFileContext,
) -> Result<Vec<ObjectDefinition>, moor_compiler::ObjDefParseError> {
    let mut definitions = compile_object_definitions(source, compile_options, context)?;
    definitions
        .iter_mut()
        .for_each(normalize_legacy_naming_properties);
    Ok(definitions)
}

pub(crate) fn apply_constants(
    constants: &Constants,
    compile_options: &CompileOptions,
    context: &mut ObjFileContext,
    source_name: &str,
) -> Result<(), ObjdefLoaderError> {
    match constants {
        Constants::Map(map) => {
            for (key, value) in map.iter() {
                let key_symbol = key.as_symbol().map_err(|_| {
                    ObjdefLoaderError::ObjectDefParseError(
                        source_name.to_string(),
                        Box::new(moor_compiler::ObjDefParseError::ConstantNotFound(format!(
                            "Constants map key must be string or symbol, got: {key:?}"
                        ))),
                    )
                })?;
                add_constant_checked(context, key_symbol, value.clone(), source_name)?;
            }
        }
        Constants::FileContent(content) => {
            compile_object_definitions(content, compile_options, context).map_err(|e| {
                ObjdefLoaderError::ObjectDefParseError(source_name.to_string(), Box::new(e))
            })?;
        }
    }
    Ok(())
}

fn add_constant_checked(
    context: &mut ObjFileContext,
    name: Symbol,
    value: Var,
    source_name: &str,
) -> Result<(), ObjdefLoaderError> {
    if let Some(existing) = context.constants().get(&name) {
        return Err(ObjdefLoaderError::ObjectDefParseError(
            source_name.to_string(),
            Box::new(moor_compiler::ObjDefParseError::DuplicateConstant(
                name.to_string(),
                format!("{existing:?}"),
            )),
        ));
    }
    for (existing_name, existing_value) in context.constants().iter() {
        if *existing_value == value {
            return Err(ObjdefLoaderError::ObjectDefParseError(
                source_name.to_string(),
                Box::new(moor_compiler::ObjDefParseError::DuplicateConstant(
                    format!("{name} = {value:?}"),
                    format!("conflicts with {existing_name} = {existing_value:?}"),
                )),
            ));
        }
    }
    context.add_constant(name, value);
    Ok(())
}

fn derive_identities(
    object_definitions: &HashMap<Obj, (String, ObjectDefinition)>,
    constants: &HashMap<Symbol, Var>,
) -> HashMap<Obj, ObjDefIdentity> {
    let mut identities = HashMap::<Obj, ObjDefIdentity>::new();
    for (name, value) in constants {
        let Some(obj) = value.as_object() else {
            continue;
        };
        if object_definitions.contains_key(&obj) {
            identities.entry(obj).or_default().constant = Some(*name);
        }
    }

    let import_export_id = crate::import_export_id();
    for (obj, (_, definition)) in object_definitions {
        let import_export_id = definition.metadata.iter().find_map(|(key, value)| {
            if *key != import_export_id {
                return None;
            }
            value.as_string().map(str::to_string)
        });
        if let Some(import_export_id) = import_export_id {
            identities.entry(*obj).or_default().import_export_id = Some(import_export_id);
        }
    }
    identities
}

#[cfg(test)]
mod tests {
    use crate::{Constants, ObjDefSet, ObjDefSource, ObjdefLoaderError};
    use moor_compiler::CompileOptions;
    use moor_var::{Obj, Symbol, v_map, v_obj, v_sym};

    #[test]
    fn parses_in_memory_sources_with_constants_and_identity() {
        let constants = ObjDefSource::new("constants.moo", "define ROOT = #1;");
        let object = ObjDefSource::new(
            "root.moo",
            r#"
            object ROOT [
                import_export_id -> "root"
            ]
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
        );

        let set =
            ObjDefSet::parse_sources(&CompileOptions::default(), None, None, [constants, object])
                .unwrap();
        let graph = set.graph();
        assert_eq!(graph.object_definitions().len(), 1);
        assert!(graph.object_definitions().contains_key(&Obj::mk_id(1)));
        assert_eq!(
            set.constants()
                .get(&Symbol::mk("ROOT"))
                .and_then(|v| v.as_object()),
            Some(Obj::mk_id(1))
        );

        let identity = graph.identity(&Obj::mk_id(1)).unwrap();
        assert_eq!(identity.constant, Some(Symbol::mk("ROOT")));
        assert_eq!(identity.import_export_id.as_deref(), Some("root"));
    }

    #[test]
    fn normalizes_legacy_naming_properties_before_staging() {
        let object = ObjDefSource::new(
            "root.moo",
            r#"
            object #1
                name: "Root"
                owner: #1
                parent: #-1
                location: #-1
                property import_export_id (owner: #1, flags: "r") = "root";
                property import_export_hierarchy (owner: #1, flags: "r") = {"core"};
            endobject
            "#,
        );

        let set =
            ObjDefSet::parse_sources(&CompileOptions::default(), None, None, [object]).unwrap();
        let (_, definition) = set
            .graph()
            .object_definitions()
            .get(&Obj::mk_id(1))
            .unwrap();
        assert!(definition.property_definitions.is_empty());
        assert!(definition.property_overrides.is_empty());
        assert!(definition.metadata.iter().any(|(key, value)| {
            *key == Symbol::mk("import_export_id") && value.as_string() == Some("root")
        }));
        assert_eq!(
            set.graph()
                .identity(&Obj::mk_id(1))
                .unwrap()
                .import_export_id
                .as_deref(),
            Some("root")
        );
    }

    #[test]
    fn reports_duplicate_object_ids_with_source_labels() {
        let first = ObjDefSource::new(
            "first.moo",
            r#"
            object #1
                name: "First"
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
        );
        let second = ObjDefSource::new(
            "second.moo",
            r#"
            object #1
                name: "Second"
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
        );

        let err =
            match ObjDefSet::parse_sources(&CompileOptions::default(), None, None, [first, second])
            {
                Ok(_) => panic!("expected duplicate object diagnostic"),
                Err(err) => err,
            };
        match err {
            ObjdefLoaderError::DuplicateObjectDefinition(source, obj, first_source) => {
                assert_eq!(source, "second.moo");
                assert_eq!(obj, Obj::mk_id(1));
                assert_eq!(first_source, "first.moo");
            }
            other => panic!("expected duplicate object diagnostic, got {other:?}"),
        }
    }

    #[test]
    fn parses_multiple_objects_from_one_source() {
        let source = ObjDefSource::new(
            "bundle.moo",
            r#"
            object #1
                name: "First"
                owner: #-1
                parent: #-1
                location: #-1
                wizard: false
                programmer: false
                player: false
                fertile: false
                readable: true
            endobject

            object #2
                name: "Second"
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
        );

        let set =
            ObjDefSet::parse_sources(&CompileOptions::default(), None, None, [source]).unwrap();
        assert_eq!(set.graph().object_definitions().len(), 2);
        assert!(
            set.graph()
                .object_definitions()
                .contains_key(&Obj::mk_id(1))
        );
        assert!(
            set.graph()
                .object_definitions()
                .contains_key(&Obj::mk_id(2))
        );
    }

    #[test]
    fn reports_malformed_source_label() {
        let source = ObjDefSource::new("bad.moo", "not an objdef");

        let err = match ObjDefSet::parse_sources(&CompileOptions::default(), None, None, [source]) {
            Ok(_) => panic!("expected parse diagnostic"),
            Err(err) => err,
        };
        match err {
            ObjdefLoaderError::ObjectDefParseError(source, _) => {
                assert_eq!(source, "bad.moo");
            }
            other => panic!("expected parse diagnostic, got {other:?}"),
        }
    }

    #[test]
    fn rejects_conflicting_constant_map_values() {
        let constants = v_map(&[
            (v_sym("FIRST"), v_obj(Obj::mk_id(1))),
            (v_sym("ALSO_FIRST"), v_obj(Obj::mk_id(1))),
        ]);
        let err = match ObjDefSet::parse_sources(
            &CompileOptions::default(),
            None,
            Some(&Constants::Map(constants.as_map().unwrap().clone())),
            Vec::<ObjDefSource>::new(),
        ) {
            Ok(_) => panic!("expected duplicate constant diagnostic"),
            Err(err) => err,
        };
        match err {
            ObjdefLoaderError::ObjectDefParseError(_, parse_error) => {
                assert!(matches!(
                    parse_error.as_ref(),
                    moor_compiler::ObjDefParseError::DuplicateConstant(_, _)
                ));
            }
            other => panic!("expected duplicate constant diagnostic, got {other:?}"),
        }
    }
    #[test]
    fn memory_labels_never_authorize_includes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("secret"), "private").unwrap();
        for binary in [false, true] {
            let macro_name = if binary { "include_bin" } else { "include" };
            let text = format!(
                "object #1 property secret (owner: #1, flags: \"r\") = {macro_name}!(\"secret\"); endobject"
            );
            let label = dir.path().join("input.moo").to_string_lossy().into_owned();
            let error = ObjDefSet::parse_sources(
                &CompileOptions::default(),
                Some(dir.path()),
                None,
                [ObjDefSource::new(label, text)],
            )
            .err()
            .expect("memory include must fail");
            assert!(
                error.to_string().contains("file-based compilation context"),
                "{error}"
            );
        }
    }

    #[test]
    fn memory_source_does_not_inherit_previous_file_authority() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("secret"), "private").unwrap();
        let disk =
            ObjDefSource::from_path(dir.path().join("constants.moo"), "define ROOT = #1;".into());
        let memory = ObjDefSource::new(
            "next.moo",
            "object ROOT property secret (owner: ROOT, flags: \"r\") = include!(\"secret\"); endobject",
        );
        let error = ObjDefSet::parse_sources(
            &CompileOptions::default(),
            Some(dir.path()),
            None,
            [disk, memory],
        )
        .err()
        .expect("memory source must clear include context");
        assert!(
            error.to_string().contains("file-based compilation context"),
            "{error}"
        );
    }

    #[test]
    fn constants_use_the_configured_compilation_profile() {
        let options = CompileOptions {
            bool_type: false,
            ..CompileOptions::default()
        };
        let source = "object #1 verb test (this none this) owner: #1 flags: \"rxd\"\nreturn true;\nendverb\nendobject";
        let error = ObjDefSet::parse_sources(
            &options,
            None,
            Some(&Constants::FileContent(source.into())),
            Vec::<ObjDefSource>::new(),
        )
        .err()
        .expect("constants must use caller compile options");
        assert!(error.compile_error().is_some(), "{error}");
    }
}
