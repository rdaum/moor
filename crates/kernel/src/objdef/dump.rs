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

use moor_compiler::{
    program_to_tree, to_literal, to_literal_objsub, unparse, ObjPropDef, ObjPropOverride,
    ObjVerbDef, ObjectDefinition, Program,
};
use moor_db::loader::LoaderInterface;
use moor_values::model::{
    prop_flags_string, verb_perms_string, HasUuid, Named, ObjFlag, PrepSpec, PropFlag, ValSet,
};
use moor_values::{v_str, v_string, AsByteBuffer, Obj, Symbol, Variant, NOTHING, SYSTEM_OBJECT};
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use tracing::info;

pub fn collect_object_definitions(loader: &dyn LoaderInterface) -> Vec<ObjectDefinition> {
    let mut object_defs = vec![];

    // Find all the ids
    let object_ids = loader.get_objects().expect("Failed to get objects");

    let mut num_verbdefs = 0;
    let mut num_propdefs = 0;
    let mut num_propoverrides = 0;

    for o in object_ids.iter() {
        let obj_attrs = loader
            .get_object(&o)
            .expect("Failed to get object attributes");

        let mut od = ObjectDefinition {
            oid: o.clone(),
            name: obj_attrs.name().unwrap_or("".to_string()),
            parent: obj_attrs.parent().unwrap_or(NOTHING),
            owner: obj_attrs.owner().unwrap_or(NOTHING),
            location: obj_attrs.location().unwrap_or(NOTHING),
            flags: obj_attrs.flags(),
            verbs: vec![],
            property_definitions: vec![],
            property_overrides: vec![],
        };

        let verbs = loader
            .get_object_verbs(&o)
            .expect("Failed to get object verbs");
        for v in verbs.iter() {
            let binary = loader
                .get_verb_binary(&o, v.uuid())
                .expect("Failed to get verb binary");
            let ov = ObjVerbDef {
                names: v.names().iter().map(|s| Symbol::mk(s)).collect(),
                argspec: v.args(),
                owner: v.owner(),
                flags: v.flags(),
                binary,
            };
            od.verbs.push(ov);
            num_verbdefs += 1;
        }

        let propdefs = loader
            .get_all_property_values(&o)
            .expect("Failed to get property values/definitions");
        for (p, (value, perms)) in propdefs.iter() {
            if p.definer() == o {
                let pd = ObjPropDef {
                    name: Symbol::mk(p.name()),
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
                    name: Symbol::mk(p.name()),
                    perms_update,
                    value: override_value,
                };
                od.property_overrides.push(ps);
                num_propoverrides += 1;
            }
        }

        // Alphabetize verbs and properties
        od.verbs.sort_by(|a, b| a.names[0].cmp(&b.names[0]));
        od.property_definitions.sort_by(|a, b| a.name.cmp(&b.name));
        od.property_overrides.sort_by(|a, b| a.name.cmp(&b.name));

        object_defs.push(od);
    }

    info!(
        "Scanned {} objects, {} verbs, {} properties, {} overrides",
        object_defs.len(),
        num_verbdefs,
        num_propdefs,
        num_propoverrides
    );
    object_defs
}

// Return the object number and if this is $nameable thing, put a // $comment
fn canon_name(oid: &Obj, index_names: &HashMap<Obj, String>) -> String {
    if let Some(name) = index_names.get(oid) {
        return name.clone();
    };

    format!("{}", oid)
}

fn propname(pname: Symbol) -> String {
    if !pname.as_str().is_empty()
        && pname
            .to_string()
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        pname.as_str().to_string()
    } else {
        let name = v_str(pname.as_str());
        to_literal(&name)
    }
}

pub fn dump_object_definitions(object_defs: &[ObjectDefinition], directory_path: &Path) {
    // Find #0 in the object_defs, and look at its properties to find $names for certain objects
    // we'll use those for filenames when we can
    // TODO: this doesn't help with nested values
    let mut index_names = HashMap::new();
    let mut file_names = HashMap::new();
    if let Some(sysobj) = object_defs.iter().find(|od| od.oid == SYSTEM_OBJECT) {
        for pd in sysobj.property_definitions.iter() {
            if let Some(value) = pd.value.as_ref() {
                if let Variant::Obj(oid) = value.variant() {
                    index_names.insert(oid.clone(), pd.name.to_string().to_ascii_uppercase());
                    file_names.insert(oid.clone(), pd.name.to_string());
                }
            }
        }
    }

    // We will generate one file per object.
    // Otherwise for large cores it just gets insane.
    // In the future we could support other user configurable modes.

    // Create the directory.

    if let Err(e) = std::fs::create_dir_all(directory_path) {
        panic!(
            "Failed to create directory {}: {}",
            directory_path.display(),
            e
        );
    }

    // Output a "constants.moo" file "define X = #5;";
    {
        let mut constants = String::new();
        // Sort incrementall by object id.
        let mut objects: Vec<_> = index_names.iter().collect();
        objects.sort_by(|a, b| a.0.id().0.cmp(&b.0.id().0));
        for i in objects {
            constants.push_str(&format!("define {} = {};\n", i.1.to_ascii_uppercase(), i.0));
        }
        let constants_file = directory_path.join("constants.moo");
        let mut constants_file = std::fs::File::create(constants_file).unwrap();
        constants_file.write_all(constants.as_bytes()).unwrap();
    }

    for o in object_defs {
        // Pick a file name.
        let file_name = match file_names.get(&o.oid) {
            Some(name) => format!("{}.moo", name),
            None => format!("object_{}.moo", o.oid.id().0),
        };
        let file_path = directory_path.join(file_name);
        let mut file = std::fs::File::create(file_path).unwrap();

        let mut objstr = String::new();

        let parent = canon_name(&o.parent, &index_names);
        let location = canon_name(&o.location, &index_names);
        let owner = canon_name(&o.owner, &index_names);

        let name = v_str(&o.name);
        let indent = "    ";

        objstr.push_str(&format!("object {}\n", canon_name(&o.oid, &index_names)));
        objstr.push_str(&format!("{indent}name: {}\n", to_literal(&name)));
        if o.parent != NOTHING {
            objstr.push_str(&format!("{indent}parent: {}\n", parent));
        }
        if o.location != NOTHING {
            objstr.push_str(&format!("{indent}location: {}\n", location));
        }
        objstr.push_str(&format!("{indent}owner: {}\n", owner));
        if o.flags.contains(ObjFlag::User) {
            objstr.push_str(&format!("{indent}player: true\n"));
        }
        if o.flags.contains(ObjFlag::Wizard) {
            objstr.push_str(&format!("{indent}wizard: true\n"));
        }
        if o.flags.contains(ObjFlag::Programmer) {
            objstr.push_str(&format!("{indent}programmer: true\n"));
        }
        if o.flags.contains(ObjFlag::Fertile) {
            objstr.push_str(&format!("{indent}fertile: true\n"));
        }
        if o.flags.contains(ObjFlag::Read) {
            objstr.push_str(&format!("{indent}readable: true\n"));
        }
        if o.flags.contains(ObjFlag::Write) {
            objstr.push_str(&format!("{indent}writeable: true\n"));
        }

        if !o.property_definitions.is_empty() {
            objstr.push('\n');
        }
        for pd in &o.property_definitions {
            let owner = canon_name(&pd.perms.owner(), &index_names);
            let flags = prop_flags_string(pd.perms.flags());

            // If the name contains funny business, use string literal form.
            let name = propname(pd.name);

            let mut base = format!("{indent}property {name} (owner: {owner}, flags: \"{flags}\")");
            if let Some(value) = &pd.value {
                let value = to_literal_objsub(value, &index_names);
                base.push_str(&format!(" = {}", value));
            }
            base.push_str(";\n");
            objstr.push_str(&base);
        }

        if !o.property_overrides.is_empty() {
            objstr.push('\n');
        }
        for ps in &o.property_overrides {
            let name = propname(ps.name);
            let mut base = format!("{indent}override {}", name);
            if let Some(perms) = &ps.perms_update {
                let flags = prop_flags_string(perms.flags());
                let owner = canon_name(&perms.owner(), &index_names);
                base.push_str(&format!(" (owner: {owner}, flags: \"{flags}\")"));
            }
            if let Some(value) = &ps.value {
                let value = to_literal_objsub(value, &index_names);
                base.push_str(&format!(" = {}", value));
            }
            base.push_str(";\n");
            objstr.push_str(&base);
        }

        for v in &o.verbs {
            objstr.push('\n');
            let owner = canon_name(&v.owner, &index_names);
            let vflags = verb_perms_string(v.flags);

            let prepspec = match v.argspec.prep {
                PrepSpec::Any => "any".to_string(),
                PrepSpec::None => "none".to_string(),
                PrepSpec::Other(p) => p.to_string_single().to_string(),
            };
            let verbargsspec = format!(
                "{} {} {}",
                v.argspec.iobj.to_string(),
                prepspec,
                v.argspec.dobj.to_string()
            );

            // If there's only a single name, and it doesn't contain any funky characters, we can
            // output just it, without any escaping. Otherwise, use a standard string literal.
            let names = if v.names.len() == 1
                && v.names[0]
                    .to_string()
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                v.names[0].as_str().to_string()
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
            let program =
                Program::from_bytes(v.binary.clone()).expect("Failed to parse verb binary");
            let decompiled = program_to_tree(&program).expect("Failed to decompile verb binary");
            let unparsed = unparse(&decompiled).expect("Failed to unparse verb binary");
            let mut body = String::new();
            for line in unparsed {
                body.push_str(indent);
                body.push_str(indent);
                body.push_str(&line);
                body.push('\n');
            }
            let body = body.trim_end().to_string();
            let decl = format!("{indent}verb {names} ({verbargsspec}) owner: {owner} flags: \"{vflags}\"\n{body}\n{indent}endverb\n");
            objstr.push_str(&decl);
        }
        objstr.push_str("endobject\n");
        file.write_all(objstr.as_bytes()).unwrap();
    }
    info!("Dumped {} objects", object_defs.len());
}

#[cfg(test)]
mod tests {
    use crate::config::FeaturesConfig;
    use crate::objdef::{
        collect_object_definitions, dump_object_definitions, ObjectDefinitionLoader,
    };
    use crate::textdump::textdump_load;
    use moor_db::{Database, DatabaseConfig, TxDB};
    use moor_values::model::CommitResult;
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
                FeaturesConfig::default(),
            )
            .unwrap();
            assert_eq!(loader_client.commit().unwrap(), CommitResult::Success);

            // Make a tmpdir & dump objdefs into it
            let loader_client = db.clone().loader_client().unwrap();
            let object_defs = collect_object_definitions(loader_client.as_ref());
            dump_object_definitions(&object_defs, tmpdir_path);
        }

        let (db, _) = TxDB::open(None, DatabaseConfig::default());
        let db = Arc::new(db);

        // Now load
        let mut loader = db.loader_client().unwrap();
        let mut defloader = ObjectDefinitionLoader::new(loader.as_mut());
        defloader
            .read_dirdump(FeaturesConfig::default(), tmpdir_path)
            .unwrap();

        // Round trip worked, so we'll just leave it at that for now. A more anal retentive test
        // would go look at known objects and props etc and compare.
    }
}
