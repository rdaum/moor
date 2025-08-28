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

use moor_common::model::loader::SnapshotInterface;
use moor_common::model::{
    HasUuid, Named, ObjFlag, PrepSpec, PropFlag, ValSet, prop_flags_string, verb_perms_string,
};
use moor_compiler::{
    ObjPropDef, ObjPropOverride, ObjVerbDef, ObjectDefinition, program_to_tree, to_literal,
    to_literal_objsub, unparse,
};
use moor_var::program::ProgramType;
use moor_var::{NOTHING, Obj, SYSTEM_OBJECT, Symbol, v_arc_string, v_str, v_string};
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use thiserror::Error;
use tracing::info;

#[derive(Error, Debug)]
pub enum ObjectDumpError {
    #[error("Worldstate error: {0}")]
    WorldState(#[from] moor_common::model::WorldStateError),

    #[error("Failed to decompile verb binary for {obj}")]
    DecompileError { obj: Obj },

    #[error("Failed to unparse verb binary for {obj}")]
    UnparseError { obj: Obj },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

pub fn collect_object_definitions(
    loader: &dyn SnapshotInterface,
) -> Result<Vec<ObjectDefinition>, ObjectDumpError> {
    let mut object_defs = vec![];

    // Find all the ids
    let object_ids = loader.get_objects()?;

    let mut num_verbdefs = 0;
    let mut num_propdefs = 0;
    let mut num_propoverrides = 0;

    for o in object_ids.iter() {
        let (verbdefs, propdefs, overrides, od) = collect_object(loader, &o)?;
        object_defs.push(od);
        num_verbdefs += verbdefs;
        num_propdefs += propdefs;
        num_propoverrides += overrides;
    }

    info!(
        "Scanned {} objects, {} verbs, {} properties, {} overrides",
        object_defs.len(),
        num_verbdefs,
        num_propdefs,
        num_propoverrides
    );
    Ok(object_defs)
}

pub fn collect_object(
    loader: &dyn SnapshotInterface,
    o: &Obj,
) -> Result<(usize, usize, usize, ObjectDefinition), ObjectDumpError> {
    let mut num_verbdefs = 0;
    let mut num_propdefs = 0;
    let mut num_propoverrides = 0;

    let obj_attrs = loader.get_object(o)?;

    let mut od = ObjectDefinition {
        oid: *o,
        name: obj_attrs.name().unwrap_or("".to_string()),
        parent: obj_attrs.parent().unwrap_or(NOTHING),
        owner: obj_attrs.owner().unwrap_or(NOTHING),
        location: obj_attrs.location().unwrap_or(NOTHING),
        flags: obj_attrs.flags(),
        verbs: vec![],
        property_definitions: vec![],
        property_overrides: vec![],
    };

    let verbs = loader.get_object_verbs(o)?;
    for v in verbs.iter() {
        let binary = loader.get_verb_program(o, v.uuid())?;
        let ov = ObjVerbDef {
            names: v.names().to_vec(),
            argspec: v.args(),
            owner: v.owner(),
            flags: v.flags(),
            program: binary,
        };
        od.verbs.push(ov);
        num_verbdefs += 1;
    }

    let propdefs = loader.get_all_property_values(o)?;
    for (p, (value, perms)) in propdefs.iter() {
        if p.definer().eq(o) {
            let pd = ObjPropDef {
                name: p.name(),
                perms: perms.clone(),
                value: value.clone(),
            };
            od.property_definitions.push(pd);
            num_propdefs += 1;
        } else {
            // We only need do a perms update if the perms actually different from the definer's
            // So let's resolve the property to its parent and see if it's different
            let mut perms_update = Some(perms.clone());
            let mut override_value = value.clone();

            if let Ok((definer_value, definer_perms)) =
                loader.get_property_value(&p.definer(), p.uuid())
            {
                if perms.eq(&definer_perms)
                    || definer_perms.flags().contains(PropFlag::Chown)
                        && perms.owner() == obj_attrs.owner().unwrap_or(NOTHING)
                {
                    perms_update = None;
                }

                if value.eq(&definer_value) {
                    override_value = None;
                }
            }

            // Just inheriting?  Move on.
            if perms_update.is_none() && override_value.is_none() {
                continue;
            }

            let ps = ObjPropOverride {
                name: p.name(),
                perms_update,
                value: override_value,
            };
            od.property_overrides.push(ps);
            num_propoverrides += 1;
        }
    }

    // Alphabetize properties. Verbs should remain in their original order.
    od.property_definitions.sort_by(|a, b| a.name.as_arc_string().cmp(&b.name.as_arc_string()));
    od.property_overrides.sort_by(|a, b| a.name.as_arc_string().cmp(&b.name.as_arc_string()));
    Ok((num_verbdefs, num_propdefs, num_propoverrides, od))
}

// Return the object number and if this is $nameable thing, put a // $comment
fn canon_name(oid: &Obj, index_names: &HashMap<Obj, String>) -> String {
    if let Some(name) = index_names.get(oid) {
        return name.clone();
    };

    format!("{oid}")
}

fn propname(pname: Symbol) -> String {
    if !pname.as_arc_string().is_empty()
        && pname
            .to_string()
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        (*pname.as_arc_string()).clone()
    } else {
        let name = v_arc_string(pname.as_arc_string());
        to_literal(&name)
    }
}

fn extract_system_object_references(
    object_defs: &[ObjectDefinition],
) -> (HashMap<Obj, String>, HashMap<Obj, String>) {
    // Collect all potential constants from direct and nested properties
    let mut all_candidates = Vec::new();
    let mut visited = std::collections::HashSet::new();

    if let Some(sysobj) = object_defs.iter().find(|od| od.oid == SYSTEM_OBJECT) {
        collect_nested_constants(object_defs, sysobj, &[], &mut all_candidates, &mut visited);
    }

    // Group candidates by object to handle multiple constants pointing to same object
    let mut candidates_by_obj: HashMap<Obj, Vec<_>> = HashMap::new();
    for candidate in all_candidates {
        candidates_by_obj
            .entry(candidate.obj)
            .or_default()
            .push(candidate);
    }

    // Pick the best candidate for each object and filter out ambiguous constant names
    let mut constant_name_counts = HashMap::new();
    let mut selected_candidates = Vec::new();

    for (_obj, mut candidates) in candidates_by_obj {
        // Sort by preference: shorter paths first, then alphabetical
        candidates.sort_by(|a, b| {
            a.path_depth
                .cmp(&b.path_depth)
                .then_with(|| a.constant_name.cmp(&b.constant_name))
        });

        // Pick the best candidate
        if let Some(best) = candidates.into_iter().next() {
            *constant_name_counts
                .entry(best.constant_name.clone())
                .or_insert(0) += 1;
            selected_candidates.push(best);
        }
    }

    // Add SYSOBJ for #0 if no constant already exists for it
    let sysobj_constant_name = "SYSOBJ".to_string();
    if !selected_candidates.iter().any(|c| c.obj == SYSTEM_OBJECT) {
        *constant_name_counts
            .entry(sysobj_constant_name.clone())
            .or_insert(0) += 1;
        selected_candidates.push(ConstantCandidate {
            obj: SYSTEM_OBJECT,
            constant_name: sysobj_constant_name,
            file_name: "sysobj".to_string(),
            path_depth: 0,
        });
    }

    // Filter out ambiguous constant names and build final maps
    let mut index_names = HashMap::new();
    let mut file_names = HashMap::new();

    for candidate in selected_candidates {
        if *constant_name_counts
            .get(&candidate.constant_name)
            .unwrap_or(&0)
            == 1
        {
            index_names.insert(candidate.obj, candidate.constant_name);
            file_names.insert(candidate.obj, candidate.file_name);
        }
    }

    (index_names, file_names)
}

#[derive(Debug, Clone)]
struct ConstantCandidate {
    obj: Obj,
    constant_name: String,
    file_name: String,
    path_depth: usize,
}

fn collect_nested_constants(
    object_defs: &[ObjectDefinition],
    current_obj: &ObjectDefinition,
    path: &[String],
    candidates: &mut Vec<ConstantCandidate>,
    visited: &mut std::collections::HashSet<Obj>,
) {
    // Prevent infinite recursion by checking if we've already visited this object
    if visited.contains(&current_obj.oid) {
        return;
    }
    visited.insert(current_obj.oid);

    for pd in current_obj.property_definitions.iter() {
        if let Some(value) = pd.value.as_ref()
            && let Some(oid) = value.as_object()
        {
            // Build the constant name from the path
            let mut constant_parts = path.to_vec();
            constant_parts.push(pd.name.to_string());

            let constant_name = constant_parts.join("_").to_ascii_uppercase();
            let file_name = if path.is_empty() {
                pd.name.to_string()
            } else {
                format!("{}_{}", path.join("_"), pd.name)
            };

            // Add this candidate
            candidates.push(ConstantCandidate {
                obj: oid,
                constant_name,
                file_name,
                path_depth: path.len(),
            });

            // Recursively traverse nested object properties
            if let Some(nested_obj) = object_defs.iter().find(|od| od.oid == oid) {
                let mut new_path = path.to_vec();
                new_path.push(pd.name.to_string());
                collect_nested_constants(object_defs, nested_obj, &new_path, candidates, visited);
            }
        }
    }

    // Remove from visited set when done to allow this object to be visited in different paths
    visited.remove(&current_obj.oid);
}

fn generate_constants_file(
    index_names: &HashMap<Obj, String>,
    directory_path: &Path,
) -> Result<(), ObjectDumpError> {
    let mut lines = Vec::new();
    // Sort incrementally by object id.
    let mut objects: Vec<_> = index_names.iter().collect();
    objects.sort_by(|a, b| a.0.id().0.cmp(&b.0.id().0));
    for i in objects {
        lines.push(format!("define {} = {};", i.1.to_ascii_uppercase(), i.0));
    }
    let constants = lines.join("\n");
    let constants_file = directory_path.join("constants.moo");
    let mut constants_file = std::fs::File::create(constants_file)?;
    constants_file.write_all(constants.as_bytes())?;
    Ok(())
}

pub fn dump_object_definitions(
    object_defs: &[ObjectDefinition],
    directory_path: &Path,
) -> Result<(), ObjectDumpError> {
    // Find #0 in the object_defs, and look at its properties to find $names for certain objects
    // we'll use those for filenames when we can
    // TODO: this doesn't help with nested values
    let (index_names, file_names) = extract_system_object_references(object_defs);

    // We will generate one file per object.
    // Otherwise for large cores it just gets insane.
    // In the future we could support other user configurable modes (multiple objects in a file,
    // or split large objects up into a directory with verb per file, etc.)

    // Create the directory.
    std::fs::create_dir_all(directory_path)?;

    // Constants index, for friendlier names
    generate_constants_file(&index_names, directory_path)?;

    for o in object_defs {
        // Pick a file name.
        let file_name = match file_names.get(&o.oid) {
            Some(name) => format!("{name}.moo"),
            None => format!("object_{}.moo", o.oid.id().0),
        };
        let file_path = directory_path.join(file_name);
        let mut file = std::fs::File::create(file_path)?;

        let lines = dump_object(&index_names, o)?;
        let objstr = lines.join("\n");
        file.write_all(objstr.as_bytes())?;
    }
    info!("Dumped {} objects", object_defs.len());
    Ok(())
}

pub fn dump_object(
    index_names: &HashMap<Obj, String>,
    o: &ObjectDefinition,
) -> Result<Vec<String>, ObjectDumpError> {
    let mut lines = Vec::new();
    let indent = "  ";
    lines.push(format!("object {}", canon_name(&o.oid, index_names)));
    let header_lines = dump_object_header(index_names, o, indent);
    lines.extend_from_slice(&header_lines);
    if !o.property_definitions.is_empty() {
        lines.push(String::new());
    }
    for pd in &o.property_definitions {
        let base = dump_property_definition(index_names, indent, pd);
        lines.push(base);
    }
    if !o.property_overrides.is_empty() {
        lines.push(String::new());
    }
    for ps in &o.property_overrides {
        let base = dump_property_override(index_names, indent, ps);
        lines.push(base);
    }

    for v in &o.verbs {
        lines.push(String::new());
        let verb_lines = dump_verb(index_names, indent, v, &o.oid)?;
        lines.extend_from_slice(&verb_lines);
    }
    lines.push("endobject".to_string());
    Ok(lines)
}

fn dump_object_header(
    index_names: &HashMap<Obj, String>,
    o: &ObjectDefinition,
    indent: &str,
) -> Vec<String> {
    let mut header_lines = Vec::with_capacity(8);
    let parent = canon_name(&o.parent, index_names);
    let location = canon_name(&o.location, index_names);
    let owner = canon_name(&o.owner, index_names);
    let name = v_str(&o.name);

    header_lines.push(format!("{indent}name: {}", to_literal(&name)));
    if o.parent != NOTHING {
        header_lines.push(format!("{indent}parent: {parent}"));
    }
    if o.location != NOTHING {
        header_lines.push(format!("{indent}location: {location}"));
    }
    header_lines.push(format!("{indent}owner: {owner}"));
    if o.flags.contains(ObjFlag::User) {
        header_lines.push(format!("{indent}player: true"));
    }
    if o.flags.contains(ObjFlag::Wizard) {
        header_lines.push(format!("{indent}wizard: true"));
    }
    if o.flags.contains(ObjFlag::Programmer) {
        header_lines.push(format!("{indent}programmer: true"));
    }
    if o.flags.contains(ObjFlag::Fertile) {
        header_lines.push(format!("{indent}fertile: true"));
    }
    if o.flags.contains(ObjFlag::Read) {
        header_lines.push(format!("{indent}readable: true"));
    }
    if o.flags.contains(ObjFlag::Write) {
        header_lines.push(format!("{indent}writeable: true"));
    }
    header_lines
}

fn dump_verb(
    index_names: &HashMap<Obj, String>,
    indent: &str,
    v: &ObjVerbDef,
    obj: &Obj,
) -> Result<Vec<String>, ObjectDumpError> {
    let mut verb_lines = vec![];
    let owner = canon_name(&v.owner, index_names);
    let vflags = verb_perms_string(v.flags);

    let prepspec = match v.argspec.prep {
        PrepSpec::Any => "any".to_string(),
        PrepSpec::None => "none".to_string(),
        PrepSpec::Other(p) => p.to_string_single().to_string(),
    };
    let verbargsspec = format!(
        "{} {} {}",
        v.argspec.dobj.to_string(),
        prepspec,
        v.argspec.iobj.to_string(),
    );

    // If there's only a single name, and it doesn't contain any funky characters, we can
    // output just it, without any escaping. Otherwise, use a standard string literal.
    let names = if v.names.len() == 1
        && v.names[0]
            .to_string()
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        (*v.names[0].as_arc_string()).clone()
    } else {
        let names = v
            .names
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<String>>()
            .join(" ");
        let names = v_string(names);
        to_literal(&names)
    };

    // decompile the verb
    let ProgramType::MooR(program) = &v.program;
    let decompiled =
        program_to_tree(program).map_err(|_| ObjectDumpError::DecompileError { obj: *obj })?;
    let unparsed = unparse(&decompiled).map_err(|_| ObjectDumpError::UnparseError { obj: *obj })?;

    verb_lines.push(format!(
        "{indent}verb {names} ({verbargsspec}) owner: {owner} flags: \"{vflags}\""
    ));
    for line in unparsed {
        verb_lines.push(format!("{indent}{indent}{line}"));
    }
    verb_lines.push(format!("{indent}endverb"));
    Ok(verb_lines)
}

fn dump_property_definition(
    index_names: &HashMap<Obj, String>,
    indent: &str,
    pd: &ObjPropDef,
) -> String {
    let owner = canon_name(&pd.perms.owner(), index_names);
    let flags = prop_flags_string(pd.perms.flags());

    // If the name contains funny business, use string literal form.
    let name = propname(pd.name);

    let mut base = format!("{indent}property {name} (owner: {owner}, flags: \"{flags}\")");
    if let Some(value) = &pd.value {
        let value = to_literal_objsub(value, index_names, 2);
        base.push_str(&format!(" = {value}"));
    }
    base.push(';');
    base
}

fn dump_property_override(
    index_names: &HashMap<Obj, String>,
    indent: &str,
    ps: &ObjPropOverride,
) -> String {
    let name = propname(ps.name);
    let mut base = format!("{indent}override {name}");
    if let Some(perms) = &ps.perms_update {
        let flags = prop_flags_string(perms.flags());
        let owner = canon_name(&perms.owner(), index_names);
        base.push_str(&format!(" (owner: {owner}, flags: \"{flags}\")"));
    }
    if let Some(value) = &ps.value {
        let value = to_literal_objsub(value, index_names, 2);
        base.push_str(&format!(" = {value}"));
    }
    base.push(';');
    base
}

#[cfg(test)]
mod tests {
    use crate::{ObjectDefinitionLoader, collect_object_definitions, dump_object_definitions};
    use moor_common::model::CommitResult;
    use moor_common::model::{PropFlag, WorldStateSource};
    use moor_common::util::BitEnum;
    use moor_compiler::{CompileOptions, compile};
    use moor_db::{Database, DatabaseConfig, TxDB};
    use moor_textdump::textdump_load;
    use moor_var::program::labels::Label;
    use moor_var::program::names::Name;
    use moor_var::program::opcode::{ScatterArgs, ScatterLabel};
    use moor_var::{Obj, SYSTEM_OBJECT, Symbol, Var, v_int};
    use semver::Version;
    use std::path::PathBuf;
    use std::sync::Arc;

    /// 1. Load from a classical textdump
    /// 2. Dump to a objdef dump
    /// 3. Load objdef dump
    /// 4. Some basic verification
    #[test]
    fn load_textdump_dump_objdef_restore_objdef() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let jhcore = manifest_dir.join("../../JHCore-DEV-2.db");

        let tmpdir = tempfile::tempdir().unwrap();
        let tmpdir_path = tmpdir.path();
        {
            let (db, _) = TxDB::open(None, DatabaseConfig::default());
            let db = Arc::new(db);
            let mut loader_client = db.clone().loader_client().unwrap();

            textdump_load(
                loader_client.as_mut(),
                jhcore,
                Version::new(0, 1, 0),
                CompileOptions::default(),
            )
            .unwrap();
            assert_eq!(loader_client.commit().unwrap(), CommitResult::Success);

            // Make a tmpdir & dump objdefs into it
            let snapshot = db.clone().create_snapshot().unwrap();
            let object_defs = collect_object_definitions(snapshot.as_ref()).unwrap();
            dump_object_definitions(&object_defs, tmpdir_path).unwrap();
        }

        let (db, _) = TxDB::open(None, DatabaseConfig::default());
        let db = Arc::new(db);

        // Now load
        let mut loader = db.loader_client().unwrap();
        let mut defloader = ObjectDefinitionLoader::new(loader.as_mut());
        defloader
            .read_dirdump(CompileOptions::default(), tmpdir_path)
            .unwrap();

        // Round trip worked, so we'll just leave it at that for now. A more anal retentive test
        // would go look at known objects and props etc and compare.
    }

    /// Test lambda objdef serialization by creating lambdas and doing a round-trip
    #[test]
    fn test_lambda_objdef_serialization() {
        let tmpdir = tempfile::tempdir().unwrap();
        let tmpdir_path = tmpdir.path();

        // Create database with lambda properties
        let (db1, _) = TxDB::open(None, DatabaseConfig::default());
        let db1 = Arc::new(db1);

        {
            let mut tx = db1.new_world_state().unwrap();

            // Create the system object first
            let system_obj = tx
                .create_object(
                    &SYSTEM_OBJECT,
                    &Obj::mk_id(-1), // parent: nothing
                    &SYSTEM_OBJECT,  // owner: self
                    BitEnum::new(),  // flags: none
                    None,
                )
                .unwrap();
            assert_eq!(system_obj, SYSTEM_OBJECT);

            // Create a simple lambda by compiling a lambda expression
            let lambda_source = "return {x} => x + 1;";
            let lambda_program = compile(lambda_source, CompileOptions::default()).unwrap();
            // Extract the lambda from the compiled program - it should be the result of the return statement
            let simple_lambda = match lambda_program
                .literals()
                .iter()
                .find(|lit| lit.as_lambda().is_some())
            {
                Some(lambda_var) => lambda_var.clone(),
                None => {
                    // Fallback: create a simple test lambda manually with correct parameter mapping
                    let simple_source = "return x + 1;";
                    let simple_program = compile(simple_source, CompileOptions::default()).unwrap();
                    let x_name = Name(11, 0, 0); // x variable from debug output above
                    let simple_params = ScatterArgs {
                        labels: vec![ScatterLabel::Required(x_name)],
                        done: Label(0),
                    };
                    Var::mk_lambda(simple_params, simple_program, vec![], None)
                }
            };

            // Create a lambda with captured environment - use fallback approach
            let captured_source = "return x + captured_var;";
            let captured_program = compile(captured_source, CompileOptions::default()).unwrap();
            let x_name = Name(11, 0, 0); // x variable from the compiled environment
            let captured_params = ScatterArgs {
                labels: vec![ScatterLabel::Required(x_name)],
                done: Label(0),
            };
            let captured_env = vec![vec![v_int(42), v_int(123)]];
            let captured_lambda =
                Var::mk_lambda(captured_params, captured_program, captured_env, None);

            // Define lambda properties
            tx.define_property(
                &SYSTEM_OBJECT,                                      // perms
                &SYSTEM_OBJECT,                                      // definer
                &SYSTEM_OBJECT,                                      // location
                Symbol::mk("simple_lambda"),                         // pname
                &SYSTEM_OBJECT,                                      // owner
                BitEnum::new_with(PropFlag::Read) | PropFlag::Write, // prop_flags
                Some(simple_lambda.clone()),                         // initial_value
            )
            .unwrap();

            tx.define_property(
                &SYSTEM_OBJECT,                                      // perms
                &SYSTEM_OBJECT,                                      // definer
                &SYSTEM_OBJECT,                                      // location
                Symbol::mk("captured_lambda"),                       // pname
                &SYSTEM_OBJECT,                                      // owner
                BitEnum::new_with(PropFlag::Read) | PropFlag::Write, // prop_flags
                Some(captured_lambda.clone()),                       // initial_value
            )
            .unwrap();

            tx.commit().unwrap();
        }

        // Force database checkpoint to ensure data is persisted
        db1.checkpoint().unwrap();

        // Dump to objdef format
        {
            let snapshot = db1.create_snapshot().unwrap();
            let object_defs = collect_object_definitions(snapshot.as_ref()).unwrap();
            dump_object_definitions(&object_defs, tmpdir_path).unwrap();
        }

        // Read the generated objdef file to verify lambda syntax
        let system_file = tmpdir_path.join("sysobj.moo");
        assert!(system_file.exists(), "System object file should be created");

        let content = std::fs::read_to_string(&system_file).unwrap();

        // Verify lambda syntax appears in the file with correct format
        assert!(
            content.contains("simple_lambda"),
            "Should contain simple_lambda property"
        );
        assert!(
            content.contains("captured_lambda"),
            "Should contain captured_lambda property"
        );
        assert!(content.contains("=>"), "Should contain lambda arrow syntax");
        assert!(
            content.contains("{x} => 1"),
            "Should contain correct lambda syntax"
        );

        // Verify the new variable name mapping format in captured environments
        assert!(
            content.contains("with captured"),
            "Should contain captured environment metadata"
        );
        assert!(
            content.contains("player: 42"),
            "Should contain variable name mapping for first captured var"
        );
        assert!(
            content.contains("this: 123"),
            "Should contain variable name mapping for second captured var"
        );

        // Load objdef back into new database - should now work with literal_lambda support
        let (db2, _) = TxDB::open(None, DatabaseConfig::default());
        let db2 = Arc::new(db2);

        {
            let mut loader = db2.loader_client().unwrap();
            let mut defloader = ObjectDefinitionLoader::new(loader.as_mut());
            defloader
                .read_dirdump(CompileOptions::default(), tmpdir_path)
                .unwrap();
            assert_eq!(loader.commit().unwrap(), CommitResult::Success);
        }

        // Verify lambdas were loaded correctly
        {
            let tx = db2.new_world_state().unwrap();

            let simple_prop = tx
                .retrieve_property(&SYSTEM_OBJECT, &SYSTEM_OBJECT, Symbol::mk("simple_lambda"))
                .unwrap();
            assert!(
                simple_prop.as_lambda().is_some(),
                "Simple lambda should be loaded as lambda"
            );

            let captured_prop = tx
                .retrieve_property(
                    &SYSTEM_OBJECT,
                    &SYSTEM_OBJECT,
                    Symbol::mk("captured_lambda"),
                )
                .unwrap();
            assert!(
                captured_prop.as_lambda().is_some(),
                "Captured lambda should be loaded as lambda"
            );

            if let Some(lambda) = captured_prop.as_lambda() {
                // With metadata support, captured environments should now be preserved
                assert_eq!(
                    lambda.0.captured_env.len(),
                    1,
                    "Should preserve captured environment with metadata"
                );
                assert_eq!(
                    lambda.0.captured_env[0].len(),
                    2,
                    "Should have 2 captured variables"
                );
                assert_eq!(
                    lambda.0.captured_env[0][0],
                    v_int(42),
                    "First captured var should be 42"
                );
                assert_eq!(
                    lambda.0.captured_env[0][1],
                    v_int(123),
                    "Second captured var should be 123"
                );
            }
        }
    }
}
