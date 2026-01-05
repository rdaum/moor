// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

#[path = "support/mod.rs"]
mod support;

use moor_common::model::{CommitResult, HasUuid, Named, ValSet, VerbArgsSpec};
use moor_compiler::{CompileOptions, program_to_tree, unparse};
use moor_db::{Database, DatabaseConfig, TxDB};
use moor_textdump::{TextdumpImportOptions, read_textdump};
use moor_var::{Obj, Symbol, Var, program::ProgramType, v_str};
use semver::Version;
use std::io::{BufReader, Cursor};

#[test]
fn builds_configured_database() {
    let _ = support::builder::DbBuildConfig::default();
}

#[test]
fn generated_textdump_round_trips() {
    let config = support::builder::DbBuildConfig {
        object_count: 12,
        inheritance_stride: 2,
        props_per_object: 3,
        verbs_per_object: 2,
        prop_override_ratio: 1.0,
        verb_override_ratio: 1.0,
        rng_seed: 0xdeadcafe,
    };
    let generated = support::builder::TestDbBuilder::new(config).build();

    let mut dump = Vec::new();
    let snapshot = generated.db.create_snapshot().unwrap();
    {
        let verbs = snapshot.get_object_verbs(&generated.object_ids[0]).unwrap();
        let target = verbs
            .find_named(Symbol::mk("verb_0_0"))
            .first()
            .cloned()
            .expect("missing verb_0_0");
        let program =
            program_to_text(&snapshot.get_verb_program(&generated.object_ids[0], target.uuid()).unwrap());
        assert_eq!(program, "return 1;");
    }
    support::textdump_writer::write_textdump(
        snapshot.as_ref(),
        &mut dump,
        &support::textdump_writer::TextdumpWriteConfig::default(),
    )
    .unwrap();

    let (db, _) = TxDB::open(None, DatabaseConfig::default());
    let mut loader = db.loader_client().unwrap();
    let cursor = BufReader::new(Cursor::new(dump));
    read_textdump(
        loader.as_mut(),
        cursor,
        Version::new(0, 1, 0),
        CompileOptions::default(),
        TextdumpImportOptions::default(),
    )
    .unwrap();
    assert!(matches!(loader.commit(), Ok(CommitResult::Success { .. })));

    let snapshot = db.create_snapshot().unwrap();
    assert_eq!(
        snapshot.get_objects().unwrap().len(),
        generated.object_ids.len()
    );

    let original_snapshot = generated.db.create_snapshot().unwrap();
    for obj in &generated.object_ids {
        let original = snapshot_object(original_snapshot.as_ref(), *obj);
        let loaded = snapshot_object(snapshot.as_ref(), *obj);
        assert_eq!(original, loaded, "mismatch for {obj}");
    }

    let target_obj = generated.object_ids[3];
    let definer = generated.object_ids[0];
    let propdefs = snapshot.get_object_properties(&definer).unwrap();
    let prop = propdefs
        .find_named(Symbol::mk("prop_0_0"))
        .first()
        .cloned()
        .expect("missing prop_0_0");
    let (value, _) = snapshot.get_property_value(&target_obj, prop.uuid()).unwrap();
    assert_eq!(value, Some(v_str("override_3_prop_0_0")));

    let verbs = snapshot.get_object_verbs(&target_obj).unwrap();
    let overridden = verbs.find_named(Symbol::mk("verb_0_0"));
    assert!(!overridden.is_empty());
}

#[derive(Debug, PartialEq)]
struct ObjSnapshot {
    attrs: ObjAttrsSnapshot,
    props: Vec<PropSnapshot>,
    verbs: Vec<VerbSnapshot>,
}

#[derive(Debug, PartialEq)]
struct ObjAttrsSnapshot {
    owner: Option<Obj>,
    parent: Option<Obj>,
    location: Option<Obj>,
    flags: u16,
    name: Option<String>,
}

#[derive(Debug, PartialEq)]
struct PropSnapshot {
    definer: Obj,
    location: Obj,
    name: String,
    owner: Obj,
    flags: u16,
    value: Option<Var>,
}

#[derive(Debug, PartialEq)]
struct VerbSnapshot {
    names: String,
    owner: Obj,
    flags: u16,
    args: VerbArgsSpec,
    program: String,
}

fn snapshot_object(snapshot: &dyn moor_common::model::loader::SnapshotInterface, obj: Obj) -> ObjSnapshot {
    let attrs = snapshot.get_object(&obj).unwrap();
    let attrs = ObjAttrsSnapshot {
        owner: attrs.owner(),
        parent: attrs.parent(),
        location: attrs.location(),
        flags: attrs.flags().to_u16(),
        name: attrs.name(),
    };

    let mut props: Vec<PropSnapshot> = snapshot
        .get_all_property_values(&obj)
        .unwrap()
        .into_iter()
        .map(|(propdef, (value, perms))| PropSnapshot {
            definer: propdef.definer(),
            location: propdef.location(),
            name: propdef.name().to_string(),
            owner: perms.owner(),
            flags: perms.flags().to_u16(),
            value,
        })
        .collect();
    props.sort_by_key(|prop| (prop.definer, prop.name.clone()));

    let mut verbs: Vec<VerbSnapshot> = snapshot
        .get_object_verbs(&obj)
        .unwrap()
        .iter()
        .map(|verb| VerbSnapshot {
            names: verb.names().iter().map(|s| s.to_string()).collect::<Vec<_>>().join(" "),
            owner: verb.owner(),
            flags: verb.flags().to_u16(),
            args: verb.args(),
            program: program_to_text(&snapshot.get_verb_program(&obj, verb.uuid()).unwrap()),
        })
        .collect();
    verbs.sort_by_key(|verb| verb.names.clone());

    ObjSnapshot { attrs, props, verbs }
}

fn program_to_text(program: &ProgramType) -> String {
    match program {
        ProgramType::MooR(program) => {
            let tree = program_to_tree(program).unwrap();
            let lines = unparse(&tree, false, false).unwrap();
            lines.join("\n")
        }
    }
}
