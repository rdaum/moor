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

use super::*;
use crate::ObjectDefinitionLoader;
use moor_common::model::{CommitResult, VerbAttrs, WorldStateSource};
use moor_common::util::BitEnum;
use moor_db::{Database, DatabaseConfig, TxDB};
use moor_var::{NOTHING, Obj};

const ROOT: Obj = Obj::mk_id(1);
const SOURCE: &str = r#"object #1 [import_export_id -> "review_root"]
owner: #1
wizard: true
verb test (this none this) owner: #1 flags: "rxd"
return "Base";
endverb
endobject"#;

fn database() -> TxDB {
    let db = TxDB::try_open(None, DatabaseConfig::default()).unwrap().0;
    let mut loader = db.loader_client().unwrap();
    ObjectDefinitionLoader::new(loader.as_mut())
        .load_single_object(SOURCE, Default::default(), Default::default())
        .unwrap();
    assert!(matches!(
        loader.commit().unwrap(),
        CommitResult::Success { .. }
    ));
    db
}
fn permissions() -> TaskPermissions {
    TaskPermissions::new(ROOT, BitEnum::new_with(ObjFlag::Wizard))
}
fn sources(text: &str) -> Var {
    v_list(&[record(&[
        ("label", v_str("root.moo")),
        ("text", v_str(text)),
    ])])
}
fn request(operation: &str) -> Var {
    record(&[
        ("schema", v_int(1)),
        ("operation", v_str(operation)),
        ("objects", v_list(&[v_obj(ROOT)])),
        ("fields", v_list(&[v_str("program")])),
    ])
}
fn get(value: &Var, name: &str) -> Var {
    value
        .as_map()
        .unwrap()
        .iter()
        .find(|(k, _)| k.as_string() == Some(name))
        .unwrap()
        .1
}
fn row(report: &Var) -> Var {
    get(report, "rows")
        .as_list()
        .unwrap()
        .iter()
        .next()
        .unwrap()
}
fn inspect(world: &dyn WorldState, source: &str, operation: &str, choices: Option<&Var>) -> Var {
    preview(
        world,
        &permissions(),
        &CompileOptions::default(),
        &sources(source),
        &request(operation),
        choices,
    )
    .unwrap()
}
fn stamp(world: &mut dyn WorldState) {
    let def = world
        .get_verb(&permissions(), &ROOT, Symbol::mk("test"))
        .unwrap();
    let (program, _) = world
        .retrieve_verb(&permissions(), &ROOT, def.uuid())
        .unwrap();
    world
        .set_verb_metadata(
            &permissions(),
            &ROOT,
            def.uuid(),
            Symbol::mk(BASE_KEY),
            record(&[
                ("schema", v_str(PROGRAM_SCHEMA)),
                ("program", v_str(&program_fingerprint(&program).unwrap())),
            ]),
        )
        .unwrap();
}

#[test]
fn unmatched_live_verbs_are_reported_and_preserved_by_apply() {
    let db = TxDB::try_open(None, DatabaseConfig::default()).unwrap().0;
    let mut loader = db.loader_client().unwrap();
    let local = SOURCE.replace(
        "endobject",
        "verb local_only (this none this) owner: #1 flags: \"rxd\"\nreturn 42;\nendverb\nendobject",
    );
    ObjectDefinitionLoader::new(loader.as_mut())
        .load_single_object(&local, Default::default(), Default::default())
        .unwrap();
    loader.commit().unwrap();
    let mut world = db.new_world_state().unwrap();
    stamp(world.as_mut());
    let report = inspect(world.as_ref(), SOURCE, "update", None);
    assert_eq!(get(&report, "rows").as_list().unwrap().len(), 1);
    assert_eq!(get(&row(&report), "classification"), v_str("unchanged"));
    let diagnostic = get(&report, "diagnostics")
        .as_list()
        .unwrap()
        .iter()
        .find(|d| get(d, "code") == v_str("live_definition_unmatched"))
        .unwrap();
    assert_eq!(get(&diagnostic, "object"), v_obj(ROOT));
    assert_eq!(get(&diagnostic, "names"), v_list(&[v_str("local_only")]));
    let definition = world
        .get_verb(&permissions(), &ROOT, Symbol::mk("local_only"))
        .unwrap();
    let original = world
        .retrieve_verb(&permissions(), &ROOT, definition.uuid())
        .unwrap()
        .0;
    let incoming = SOURCE.replace("Base", "Upstream");
    let report = inspect(world.as_ref(), &incoming, "update", None);
    apply_report(world.as_mut(), &incoming, "update", &report, &v_map(&[])).unwrap();
    assert_eq!(
        world
            .get_verb(&permissions(), &ROOT, Symbol::mk("local_only"))
            .unwrap()
            .uuid(),
        definition.uuid()
    );
    assert_eq!(
        program_fingerprint(
            &world
                .retrieve_verb(&permissions(), &ROOT, definition.uuid())
                .unwrap()
                .0
        )
        .unwrap(),
        program_fingerprint(&original).unwrap()
    );
}

#[test]
fn all_content_classifications() {
    for (b, l, i, want) in [
        (Some("a"), "a", "a", "unchanged"),
        (Some("a"), "a", "b", "upstream"),
        (Some("a"), "b", "a", "local"),
        (Some("a"), "b", "b", "converged"),
        (Some("a"), "b", "c", "conflict"),
        (None, "a", "a", "unbased"),
    ] {
        assert_eq!(classify(b, l, i), want);
    }
}

#[test]
fn preview_and_drafts_commit_no_mutations() {
    let db = database();
    let world = db.new_world_state().unwrap();
    let report = inspect(world.as_ref(), SOURCE, "adopt", None);
    assert_eq!(get(&row(&report), "classification"), v_str("unbased"));
    let id = get(&row(&report), "id");
    let draft = v_map(&[(
        id.clone(),
        record(&[("choice", v_str("edited")), ("program", v_str("return 3;"))]),
    )]);
    assert!(
        preview(
            world.as_ref(),
            &permissions(),
            &CompileOptions::default(),
            &sources(SOURCE),
            &request("adopt"),
            Some(&draft)
        )
        .is_err()
    );
    assert!(matches!(
        world.commit().unwrap(),
        CommitResult::Success {
            mutations_made: false,
            ..
        }
    ));

    let mut world = db.new_world_state().unwrap();
    stamp(world.as_mut());
    world.commit().unwrap();
    let world = db.new_world_state().unwrap();
    let report = inspect(world.as_ref(), SOURCE, "update", Some(&draft));
    let validation = get(&report, "validation")
        .as_list()
        .unwrap()
        .iter()
        .next()
        .unwrap();
    assert!(get(&validation, "valid").is_true());
    let invalid = v_map(&[(
        id,
        record(&[("choice", v_str("edited")), ("program", v_str("return (;"))]),
    )]);
    let report = inspect(world.as_ref(), SOURCE, "update", Some(&invalid));
    let validation = get(&report, "validation")
        .as_list()
        .unwrap()
        .iter()
        .next()
        .unwrap();
    assert!(!get(&validation, "valid").is_true());
    assert_eq!(get(&validation, "pane"), v_str("result"));
    assert!(matches!(
        world.commit().unwrap(),
        CommitResult::Success {
            mutations_made: false,
            ..
        }
    ));
}

#[test]
fn evidence_changes_with_live_code_baseline_and_definition_attributes() {
    let db = database();
    let mut world = db.new_world_state().unwrap();
    stamp(world.as_mut());
    world.commit().unwrap();
    let mut world = db.new_world_state().unwrap();
    let first = inspect(world.as_ref(), SOURCE, "update", None);
    let def = world
        .get_verb(&permissions(), &ROOT, Symbol::mk("test"))
        .unwrap();
    world
        .update_verb_with_id(
            &permissions(),
            &ROOT,
            def.uuid(),
            VerbAttrs {
                definer: None,
                owner: None,
                names: None,
                flags: None,
                args_spec: None,
                program: Some(ProgramType::MooR(
                    compile("return 2;", Default::default()).unwrap(),
                )),
            },
        )
        .unwrap();
    let second = inspect(world.as_ref(), SOURCE, "update", None);
    assert_ne!(get(&first, "evidence"), get(&second, "evidence"));
    assert_eq!(get(&row(&second), "classification"), v_str("local"));
    stamp(world.as_mut());
    let third = inspect(world.as_ref(), SOURCE, "update", None);
    assert_ne!(get(&second, "evidence"), get(&third, "evidence"));
    world
        .update_verb_with_id(
            &permissions(),
            &ROOT,
            def.uuid(),
            VerbAttrs {
                definer: None,
                owner: None,
                names: None,
                flags: Some(moor_common::model::VerbFlag::rx()),
                args_spec: None,
                program: None,
            },
        )
        .unwrap();
    let fourth = inspect(world.as_ref(), SOURCE, "update", None);
    assert_ne!(get(&third, "evidence"), get(&fourth, "evidence"));
}

#[test]
fn details_preserve_evidence_and_unknown_options_fail() {
    let db = database();
    let world = db.new_world_state().unwrap();
    let report = inspect(world.as_ref(), SOURCE, "adopt", None);
    let mut req = request("adopt")
        .as_map()
        .unwrap()
        .iter()
        .collect::<Vec<_>>();
    req.push((v_str("details"), v_list(&[get(&row(&report), "id")])));
    let detailed = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &sources(SOURCE),
        &v_map(&req),
        None,
    )
    .unwrap();
    assert_eq!(get(&report, "evidence"), get(&detailed, "evidence"));
    assert!(
        get(&row(&detailed), "live_text")
            .as_string()
            .unwrap()
            .contains("Base")
    );
    req.push((v_str("force"), v_bool(true)));
    assert!(
        preview(
            world.as_ref(),
            &permissions(),
            &CompileOptions::default(),
            &sources(SOURCE),
            &v_map(&req),
            None
        )
        .is_err()
    );
}

#[test]
fn forged_identity_and_unknown_schema_block_automatic_update() {
    let db = database();
    let mut world = db.new_world_state().unwrap();
    stamp(world.as_mut());
    let report = inspect(
        world.as_ref(),
        &SOURCE.replace("review_root", "different"),
        "update",
        None,
    );
    assert!(!get(&row(&report), "eligible").is_true());
    let def = world
        .get_verb(&permissions(), &ROOT, Symbol::mk("test"))
        .unwrap();
    world
        .set_verb_metadata(
            &permissions(),
            &ROOT,
            def.uuid(),
            Symbol::mk(BASE_KEY),
            record(&[("schema", v_str("future")), ("program", v_str("unknown"))]),
        )
        .unwrap();
    let report = inspect(world.as_ref(), SOURCE, "update", None);
    assert!(!get(&row(&report), "eligible").is_true());
    let report = inspect(world.as_ref(), SOURCE, "adopt", None);
    assert!(get(&row(&report), "eligible").is_true());
}

fn apply_report(
    world: &mut dyn WorldState,
    source: &str,
    operation: &str,
    report: &Var,
    choices: &Var,
) -> Result<Var, ApplyError> {
    apply(
        world,
        &permissions(),
        &CompileOptions::default(),
        &sources(source),
        &request(operation),
        &get(report, "evidence"),
        choices,
    )
}

fn live_program(world: &dyn WorldState) -> ProgramType {
    let definition = world
        .get_verb(&permissions(), &ROOT, Symbol::mk("test"))
        .unwrap();
    world
        .retrieve_verb(&permissions(), &ROOT, definition.uuid())
        .unwrap()
        .0
}

#[test]
fn adoption_keep_local_and_repeat_preserve_live_program() {
    let db = database();
    let mut world = db.new_world_state().unwrap();
    let original = program_fingerprint(&live_program(world.as_ref())).unwrap();
    let report = inspect(world.as_ref(), SOURCE, "adopt", None);
    apply_report(world.as_mut(), SOURCE, "adopt", &report, &v_map(&[])).unwrap();
    assert_eq!(
        program_fingerprint(&live_program(world.as_ref())).unwrap(),
        original
    );
    world.commit().unwrap();

    let incoming = SOURCE.replace("Base", "Upstream");
    let mut world = db.new_world_state().unwrap();
    let report = inspect(world.as_ref(), &incoming, "update", None);
    assert_eq!(get(&row(&report), "classification"), v_str("upstream"));
    let choice = v_map(&[(
        get(&row(&report), "id"),
        record(&[("choice", v_str("local"))]),
    )]);
    apply_report(world.as_mut(), &incoming, "update", &report, &choice).unwrap();
    world.commit().unwrap();
    let mut world = db.new_world_state().unwrap();
    assert_eq!(
        program_fingerprint(&live_program(world.as_ref())).unwrap(),
        original
    );
    let report = inspect(world.as_ref(), &incoming, "update", None);
    assert_eq!(get(&row(&report), "classification"), v_str("local"));
    apply_report(world.as_mut(), &incoming, "update", &report, &v_map(&[])).unwrap();
    assert!(matches!(
        world.commit().unwrap(),
        CommitResult::Success {
            mutations_made: false,
            ..
        }
    ));
}

#[test]
fn edited_choice_installs_result_but_accepts_incoming() {
    let db = database();
    let mut world = db.new_world_state().unwrap();
    stamp(world.as_mut());
    let definition = world
        .get_verb(&permissions(), &ROOT, Symbol::mk("test"))
        .unwrap();
    world
        .set_verb_metadata(
            &permissions(),
            &ROOT,
            definition.uuid(),
            Symbol::mk("unmanaged"),
            v_str("keep"),
        )
        .unwrap();
    let incoming = SOURCE.replace("Base", "Upstream");
    let report = inspect(world.as_ref(), &incoming, "update", None);
    let id = get(&row(&report), "id");
    let draft = record(&[
        ("choice", v_str("edited")),
        ("program", v_str("return \"Result\";")),
    ]);
    assert!(
        apply_report(
            world.as_mut(),
            &incoming,
            "update",
            &report,
            &v_map(&[(id.clone(), draft.clone())])
        )
        .is_err()
    );
    let validation = inspect(
        world.as_ref(),
        &incoming,
        "update",
        Some(&v_map(&[(id.clone(), draft)])),
    );
    let validation = get(&validation, "validation")
        .as_list()
        .unwrap()
        .iter()
        .next()
        .unwrap();
    let token = get(&validation, "validation");
    let choice = |source| {
        v_map(&[(
            id.clone(),
            record(&[
                ("choice", v_str("edited")),
                ("program", v_str(source)),
                ("validation", token.clone()),
            ]),
        )])
    };
    assert!(
        apply_report(
            world.as_mut(),
            &incoming,
            "update",
            &report,
            &choice("return \"Changed\";")
        )
        .is_err()
    );
    apply_report(
        world.as_mut(),
        &incoming,
        "update",
        &report,
        &choice("return \"Result\";"),
    )
    .unwrap();
    let updated = world
        .get_verb(&permissions(), &ROOT, Symbol::mk("test"))
        .unwrap();
    assert_eq!(updated.uuid(), definition.uuid());
    assert_eq!(updated.names(), definition.names());
    assert_eq!(updated.flags(), definition.flags());
    assert_eq!(updated.owner(), definition.owner());
    assert_eq!(
        world
            .get_verb_metadata(
                &permissions(),
                &ROOT,
                updated.uuid(),
                Symbol::mk("unmanaged")
            )
            .unwrap(),
        Some(v_str("keep"))
    );
    let next = inspect(world.as_ref(), &incoming, "update", None);
    assert_eq!(get(&row(&next), "classification"), v_str("local"));
    assert_eq!(get(&row(&next), "base"), get(&row(&report), "incoming"));
    assert_eq!(
        program_text(&live_program(world.as_ref())).unwrap(),
        v_str("return \"Result\";")
    );
    world.commit().unwrap();
}

#[test]
fn stale_evidence_and_unresolved_conflicts_write_nothing() {
    let db = database();
    let mut world = db.new_world_state().unwrap();
    stamp(world.as_mut());
    world.commit().unwrap();
    let world = db.new_world_state().unwrap();
    let source = SOURCE.replace("Base", "First");
    let report = inspect(world.as_ref(), &source, "update", None);
    world.commit().unwrap();
    let mut world = db.new_world_state().unwrap();
    assert!(
        apply_report(
            world.as_mut(),
            &SOURCE.replace("Base", "Other"),
            "update",
            &report,
            &v_map(&[])
        )
        .is_err()
    );
    assert!(matches!(
        world.commit().unwrap(),
        CommitResult::Success {
            mutations_made: false,
            ..
        }
    ));
    let mut world = db.new_world_state().unwrap();
    apply_report(world.as_mut(), &source, "update", &report, &v_map(&[])).unwrap();
    world.commit().unwrap();
    let mut world = db.new_world_state().unwrap();
    assert!(apply_report(world.as_mut(), &source, "update", &report, &v_map(&[])).is_err());
    // Establish an older baseline to model an independent local edit.
    let definition = world
        .get_verb(&permissions(), &ROOT, Symbol::mk("test"))
        .unwrap();
    world
        .set_verb_metadata(
            &permissions(),
            &ROOT,
            definition.uuid(),
            Symbol::mk(BASE_KEY),
            record(&[
                ("schema", v_str(PROGRAM_SCHEMA)),
                ("program", get(&row(&report), "base")),
            ]),
        )
        .unwrap();
    world.commit().unwrap();
    let source = SOURCE.replace("Base", "Second");
    let mut world = db.new_world_state().unwrap();
    let report = inspect(world.as_ref(), &source, "update", None);
    assert_eq!(get(&row(&report), "classification"), v_str("conflict"));
    assert!(apply_report(world.as_mut(), &source, "update", &report, &v_map(&[])).is_err());
    let choice = v_map(&[(
        get(&row(&report), "id"),
        record(&[("choice", v_str("defer"))]),
    )]);
    apply_report(world.as_mut(), &source, "update", &report, &choice).unwrap();
    assert!(matches!(
        world.commit().unwrap(),
        CommitResult::Success {
            mutations_made: false,
            ..
        }
    ));
}

#[test]
fn late_writer_failure_requires_rollback_of_program_and_baseline() {
    let db = database();
    let mut world = db.new_world_state().unwrap();
    let definition = world
        .get_verb(&permissions(), &ROOT, Symbol::mk("test"))
        .unwrap();
    let original = program_fingerprint(&live_program(world.as_ref())).unwrap();
    let actions = vec![
        ChangeAction {
            object: ROOT,
            target: ChangeTarget::Existing(definition.uuid()),
            program: Some(ProgramType::MooR(
                compile("return 9;", CompileOptions::default()).unwrap(),
            )),
            baseline: record(&[
                ("schema", v_str(PROGRAM_SCHEMA)),
                ("program", v_str("injected")),
            ]),
        },
        ChangeAction {
            object: ROOT,
            target: ChangeTarget::Existing(uuid::Uuid::nil()),
            program: None,
            baseline: v_int(0),
        },
    ];
    assert!(matches!(
        write_actions(world.as_mut(), &permissions(), actions),
        Err(ApplyError::Mutation(_))
    ));
    world.rollback().unwrap();
    let world = db.new_world_state().unwrap();
    assert_eq!(
        program_fingerprint(&live_program(world.as_ref())).unwrap(),
        original
    );
    assert_eq!(
        world
            .get_verb_metadata(
                &permissions(),
                &ROOT,
                definition.uuid(),
                Symbol::mk(BASE_KEY)
            )
            .unwrap(),
        None
    );
}

#[test]
fn ordinary_directory_import_leaves_unknown_baselines_unmanaged() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("root.moo"), SOURCE).unwrap();
    let db = TxDB::try_open(None, DatabaseConfig::default()).unwrap().0;
    let mut loader = db.loader_client().unwrap();
    let mut import = ObjectDefinitionLoader::new(loader.as_mut());
    import
        .load_objdef_directory(
            CompileOptions::default(),
            directory.path(),
            Default::default(),
        )
        .unwrap();
    loader.commit().unwrap();
    let world = db.new_world_state().unwrap();
    assert_eq!(
        get(
            &row(&inspect(world.as_ref(), SOURCE, "update", None)),
            "classification"
        ),
        v_str("unbased")
    );
    let report = inspect(world.as_ref(), SOURCE, "update", None);
    assert_eq!(get(&row(&report), "default"), v_str("defer"));
    assert_eq!(get(&row(&report), "eligible"), v_bool(false));
}

#[test]
fn bulk_import_compares_working_source_against_explicit_ancestor() {
    let parse = |text: &str| {
        ObjDefSet::parse_sources(
            &CompileOptions::default(),
            None,
            None,
            [ObjDefSource::new("core.moo", text)],
        )
        .unwrap()
    };
    let base = parse(SOURCE);
    let local = SOURCE.replace("Base", "Local").replace(
        "endobject",
        "verb added (this none this) owner: #1 flags: \"rxd\"\nreturn 42;\nendverb\nendobject",
    );
    let provenance = record(&[("transport", v_str("git")), ("commit", v_str("ancestor"))]);
    let prepared = parse(&local)
        .with_baseline(&base, Some(provenance.clone()))
        .unwrap();
    let db = TxDB::try_open(None, DatabaseConfig::default()).unwrap().0;
    let mut loader = db.loader_client().unwrap();
    ObjectDefinitionLoader::new(loader.as_mut())
        .load_objdef_set(prepared, Default::default())
        .unwrap();
    loader.commit().unwrap();
    let world = db.new_world_state().unwrap();
    let report = inspect(world.as_ref(), SOURCE, "update", None);
    assert_eq!(get(&row(&report), "classification"), v_str("local"));
    assert_eq!(get(&row(&report), "default"), v_str("defer"));
    let definition = world
        .get_verb(&permissions(), &ROOT, Symbol::mk("test"))
        .unwrap();
    let baseline = world
        .get_verb_metadata(
            &permissions(),
            &ROOT,
            definition.uuid(),
            Symbol::mk(BASE_KEY),
        )
        .unwrap()
        .unwrap();
    assert_eq!(get(&baseline, "source"), provenance);
    let added = world
        .get_verb(&permissions(), &ROOT, Symbol::mk("added"))
        .unwrap();
    assert!(
        world
            .get_verb_metadata(&permissions(), &ROOT, added.uuid(), Symbol::mk(BASE_KEY))
            .unwrap()
            .is_none()
    );
    let report = inspect(
        world.as_ref(),
        &SOURCE.replace("Base", "Upstream"),
        "update",
        None,
    );
    assert_eq!(get(&row(&report), "classification"), v_str("conflict"));
    drop(world);

    // A prepared deployment survives export and reimport with the ancestor intact.
    let exported = tempfile::tempdir().unwrap();
    crate::dump_snapshot_object_definitions(
        db.create_snapshot().unwrap().as_ref(),
        exported.path(),
    )
    .unwrap();
    let restored = TxDB::try_open(None, DatabaseConfig::default()).unwrap().0;
    let mut loader = restored.loader_client().unwrap();
    ObjectDefinitionLoader::new(loader.as_mut())
        .load_objdef_directory(
            CompileOptions::default(),
            exported.path(),
            Default::default(),
        )
        .unwrap();
    loader.commit().unwrap();
    let world = restored.new_world_state().unwrap();
    let report = inspect(world.as_ref(), SOURCE, "update", None);
    assert_eq!(get(&row(&report), "classification"), v_str("local"));
    let definition = world
        .get_verb(&permissions(), &ROOT, Symbol::mk("test"))
        .unwrap();
    assert_eq!(
        get(
            &world
                .get_verb_metadata(
                    &permissions(),
                    &ROOT,
                    definition.uuid(),
                    Symbol::mk(BASE_KEY)
                )
                .unwrap()
                .unwrap(),
            "source"
        ),
        provenance
    );
}

#[test]
fn baseline_binding_rejects_reused_or_relocated_objects_and_ambiguous_verbs() {
    let parse = |text: &str| {
        ObjDefSet::parse_sources(
            &CompileOptions::default(),
            None,
            None,
            [ObjDefSource::new("core.moo", text)],
        )
        .unwrap()
    };
    let base = parse(SOURCE);
    for text in [
        SOURCE.replace("review_root", "different_object"),
        SOURCE.replace("#1", "#2"),
        SOURCE.replace(
            "endobject",
            "verb test (this none this) owner: #1 flags: \"rxd\"\nreturn 42;\nendverb\nendobject",
        ),
    ] {
        assert!(parse(&text).with_baseline(&base, None).is_err());
    }
}

#[test]
fn baseline_survives_process_restart() {
    if let Ok(path) = std::env::var("MOOR_REVIEW_RESTART_DB") {
        let db = TxDB::try_open(Some(std::path::Path::new(&path)), DatabaseConfig::default())
            .unwrap()
            .0;
        if std::env::var("MOOR_REVIEW_RESTART_PHASE").unwrap() == "enroll" {
            let mut loader = db.loader_client().unwrap();
            ObjectDefinitionLoader::new(loader.as_mut())
                .load_single_object(SOURCE, Default::default(), Default::default())
                .unwrap();
            loader.commit().unwrap();
            let mut world = db.new_world_state().unwrap();
            let report = inspect(world.as_ref(), SOURCE, "adopt", None);
            apply_report(world.as_mut(), SOURCE, "adopt", &report, &v_map(&[])).unwrap();
            world.commit().unwrap();
            db.wait_for_persistence().unwrap();
        } else {
            let world = db.new_world_state().unwrap();
            assert_eq!(
                get(
                    &row(&inspect(world.as_ref(), SOURCE, "update", None)),
                    "classification"
                ),
                v_str("unchanged")
            );
        }
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    for phase in ["enroll", "verify"] {
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "review::tests::baseline_survives_process_restart",
                "--exact",
            ])
            .env("MOOR_REVIEW_RESTART_DB", directory.path())
            .env("MOOR_REVIEW_RESTART_PHASE", phase)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

#[test]
fn directory_import_preserves_supplied_baseline_for_local_program() {
    let directory = tempfile::tempdir().unwrap();
    let base = program_fingerprint(&ProgramType::MooR(
        compile("return \"Upstream\";", CompileOptions::default()).unwrap(),
    ))
    .unwrap();
    let source = SOURCE.replace(
        "flags: \"rxd\"",
        &format!("flags: \"rxd\" [objdef_base -> [\"schema\" -> \"{PROGRAM_SCHEMA}\", \"program\" -> \"{base}\"]]"),
    );
    std::fs::write(directory.path().join("root.moo"), source).unwrap();
    let db = TxDB::try_open(None, DatabaseConfig::default()).unwrap().0;
    let mut loader = db.loader_client().unwrap();
    ObjectDefinitionLoader::new(loader.as_mut())
        .load_objdef_directory(
            CompileOptions::default(),
            directory.path(),
            Default::default(),
        )
        .unwrap();
    loader.commit().unwrap();
    let world = db.new_world_state().unwrap();
    let report = inspect(world.as_ref(), SOURCE, "update", None);
    assert_eq!(get(&row(&report), "base"), v_str(&base));
    assert_eq!(get(&row(&report), "classification"), v_str("converged"));
    drop(world);

    // Backups preserve the accepted baseline; source exports leave it unknown on import.
    let snapshot = db.create_snapshot().unwrap();
    for include_baselines in [true, false] {
        let export = tempfile::tempdir().unwrap();
        if include_baselines {
            crate::dump_snapshot_object_definitions(snapshot.as_ref(), export.path()).unwrap();
        } else {
            crate::dump_snapshot_object_definitions_with_options(
                snapshot.as_ref(),
                export.path(),
                crate::ObjectDumpOptions {
                    include_baselines: false,
                },
            )
            .unwrap();
        }
        let text = std::fs::read_to_string(export.path().join("review_root.moo")).unwrap();
        assert_eq!(text.contains("objdef_base ->"), include_baselines);
        let restored = TxDB::try_open(None, DatabaseConfig::default()).unwrap().0;
        let mut loader = restored.loader_client().unwrap();
        ObjectDefinitionLoader::new(loader.as_mut())
            .load_objdef_directory(CompileOptions::default(), export.path(), Default::default())
            .unwrap();
        loader.commit().unwrap();
        let world = restored.new_world_state().unwrap();
        let report = inspect(world.as_ref(), SOURCE, "update", None);
        let expected = if include_baselines {
            v_str(&base)
        } else {
            v_list(&[])
        };
        assert_eq!(get(&row(&report), "base"), expected);
    }
    // Exporting without baselines must not alter the source database's tracking state.
    let world = db.new_world_state().unwrap();
    assert_eq!(
        get(
            &row(&inspect(world.as_ref(), SOURCE, "update", None)),
            "base"
        ),
        v_str(&base)
    );
}

#[test]
fn invalid_imported_baselines_fail_without_committing_objects() {
    for baseline in [
        "[\"schema\" -> \"future\", \"program\" -> \"unknown\"]".to_owned(),
        format!("[\"schema\" -> \"{PROGRAM_SCHEMA}\", \"program\" -> \"invalid\"]"),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let source = SOURCE.replace(
            "flags: \"rxd\"",
            &format!("flags: \"rxd\" [objdef_base -> {baseline}]"),
        );
        std::fs::write(directory.path().join("root.moo"), source).unwrap();
        let db = TxDB::try_open(None, DatabaseConfig::default()).unwrap().0;
        let mut loader = db.loader_client().unwrap();
        assert!(
            ObjectDefinitionLoader::new(loader.as_mut())
                .load_objdef_directory(
                    CompileOptions::default(),
                    directory.path(),
                    Default::default()
                )
                .is_err()
        );
        drop(loader);
        assert!(
            db.loader_client()
                .unwrap()
                .get_existing_object(&ROOT)
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn bundled_cores_declare_complete_default_package_bindings() {
    for core in ["cowbell", "snore"] {
        let db = TxDB::try_open(None, DatabaseConfig::default()).unwrap().0;
        let mut loader = db.loader_client().unwrap();
        let options = CompileOptions {
            custom_errors: true,
            ..CompileOptions::default()
        };
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../cores/{core}/src"));
        let imported = ObjectDefinitionLoader::new(loader.as_mut())
            .load_objdef_directory(options, &path, Default::default())
            .unwrap();
        let manager = moor_var::Obj::mk_id(2000);
        let packages = loader
            .get_existing_property_value(&manager, Symbol::mk("packages"))
            .unwrap()
            .unwrap()
            .0;
        let package = get(&packages, core);
        let bindings = get(&package, "objects");
        let objects = bindings
            .as_list()
            .unwrap()
            .iter()
            .map(|v| v.as_object().unwrap())
            .collect::<BTreeSet<_>>();
        assert_eq!(objects, imported.loaded_objects.iter().copied().collect());
        assert_eq!(
            get(&package, "fields"),
            v_list(&if core == "cowbell" {
                vec![v_str("program"), v_str("property"), v_str("object")]
            } else {
                vec![v_str("program")]
            })
        );
        let constants = get(&package, "constants");
        assert!(!constants.as_map().unwrap().is_empty());
        let default = loader
            .get_existing_property_value(&manager, Symbol::mk("default_package"))
            .unwrap()
            .unwrap()
            .0;
        assert_eq!(default, v_str(core));
    }
}

#[test]
fn explicit_owner_trust_is_guarded_and_does_not_allow_public_writes() {
    let db = database();
    let source = "object #2\nowner: #2\nverb test (this none this) owner: #2 flags: \"rxd\"\nreturn 42;\nendverb\nendobject";
    let mut loader = db.loader_client().unwrap();
    ObjectDefinitionLoader::new(loader.as_mut())
        .load_single_object(source, Default::default(), Default::default())
        .unwrap();
    loader.commit().unwrap();
    let mut world = db.new_world_state().unwrap();
    let request = |trusted: bool| {
        record(&[
            ("schema", v_int(1)),
            ("operation", v_str("adopt")),
            ("objects", v_list(&[v_obj(Obj::mk_id(2))])),
            ("fields", v_list(&[v_str("program")])),
            (
                "trusted_owners",
                v_list(&if trusted {
                    vec![v_obj(Obj::mk_id(2))]
                } else {
                    vec![]
                }),
            ),
        ])
    };
    let options = CompileOptions::default();
    let untrusted = preview(
        world.as_ref(),
        &permissions(),
        &options,
        &sources(source),
        &request(false),
        None,
    )
    .unwrap();
    assert_eq!(get(&row(&untrusted), "eligible"), v_bool(false));
    let trusted = preview(
        world.as_ref(),
        &permissions(),
        &options,
        &sources(source),
        &request(true),
        None,
    )
    .unwrap();
    assert_eq!(get(&row(&trusted), "eligible"), v_bool(true));
    assert_ne!(get(&untrusted, "evidence"), get(&trusted, "evidence"));
    assert!(
        apply(
            world.as_mut(),
            &permissions(),
            &options,
            &sources(source),
            &request(false),
            &get(&trusted, "evidence"),
            &v_map(&[])
        )
        .is_err()
    );
    apply(
        world.as_mut(),
        &permissions(),
        &options,
        &sources(source),
        &request(true),
        &get(&trusted, "evidence"),
        &v_map(&[]),
    )
    .unwrap();
    world
        .set_flags_of(
            &permissions(),
            &Obj::mk_id(2),
            BitEnum::new_with(ObjFlag::Write),
        )
        .unwrap();
    let writable = preview(
        world.as_ref(),
        &permissions(),
        &options,
        &sources(source),
        &request(true),
        None,
    )
    .unwrap();
    assert_eq!(get(&row(&writable), "eligible"), v_bool(false));
    assert!(
        apply(
            world.as_mut(),
            &permissions(),
            &options,
            &sources(source),
            &request(true),
            &get(&trusted, "evidence"),
            &v_map(&[])
        )
        .is_err()
    );
}

#[test]
fn inspection_browses_unmatched_contents_and_property_changes_without_applying() {
    let local = SOURCE.replace(
        "endobject",
        "property title (owner: #1, flags: \"r\") = \"Local\";\nendobject",
    );
    let db = TxDB::try_open(None, DatabaseConfig::default()).unwrap().0;
    let mut loader = db.loader_client().unwrap();
    let mut importer = ObjectDefinitionLoader::new(loader.as_mut());
    importer
        .load_single_object(&local, Default::default(), Default::default())
        .unwrap();
    importer
        .load_single_object(
            &local
                .replace("#1", "#2")
                .replace("review_root", "local_object")
                .replace("wizard: true", "wizard: false"),
            Default::default(),
            Default::default(),
        )
        .unwrap();
    loader.commit().unwrap();
    let mut world = db.new_world_state().unwrap();
    let incoming = local.replace("Local", "local");
    let req = record(&[
        ("schema", v_int(1)),
        ("operation", v_str("inspect")),
        ("objects", v_list(&[v_obj(ROOT), v_obj(Obj::mk_id(2))])),
        (
            "fields",
            v_list(&[
                v_str("object"),
                v_str("attribute"),
                v_str("property"),
                v_str("program"),
            ]),
        ),
    ]);
    let report = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &sources(&incoming),
        &req,
        None,
    )
    .unwrap();
    let rows = get(&report, "rows");
    let rows = rows.as_list().unwrap().iter().collect::<Vec<_>>();
    let local_program = rows
        .iter()
        .find(|row| {
            get(row, "field") == v_str("program") && get(row, "object") == v_obj(Obj::mk_id(2))
        })
        .unwrap();
    assert_eq!(get(local_program, "classification"), v_str("local_only"));
    assert_eq!(get(local_program, "choices"), v_list(&[]));
    assert_eq!(get(local_program, "incoming_present"), v_bool(false));
    assert!(
        !local_program
            .as_map()
            .unwrap()
            .contains_key(&v_str("live_text"), true)
            .unwrap()
    );
    assert!(rows.iter().any(|row| get(row, "field") == v_str("property")
        && get(row, "classification") == v_str("unbased")));
    assert!(rows.iter().any(|row| get(row, "field") == v_str("property")
        && get(row, "classification") == v_str("local_only")));
    assert!(rows.iter().any(|row| get(row, "field") == v_str("object")
        && get(row, "classification") == v_str("local_only")));
    let detail_req = req
        .as_map()
        .unwrap()
        .set(&v_str("details"), &v_list(&[get(local_program, "id")]))
        .unwrap();
    let details = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &sources(&incoming),
        &detail_req,
        None,
    )
    .unwrap();
    let rows = get(&details, "rows");
    let detail = rows
        .as_list()
        .unwrap()
        .iter()
        .find(|row| get(row, "id") == get(local_program, "id"))
        .unwrap();
    assert_eq!(get(&detail, "live_text"), v_str("return \"Base\";"));
    assert_eq!(get(&detail, "incoming_text"), v_str(""));
    assert!(
        apply(
            world.as_mut(),
            &permissions(),
            &CompileOptions::default(),
            &sources(&incoming),
            &req,
            &v_list(&[]),
            &record(&[])
        )
        .is_err()
    );
    let unauthorized = TaskPermissions::new(Obj::mk_id(2), BitEnum::new());
    assert!(
        preview(
            world.as_ref(),
            &unauthorized,
            &CompileOptions::default(),
            &sources(&incoming),
            &req,
            None
        )
        .is_err()
    );
}

#[test]
fn property_and_attribute_edits_use_the_imported_ancestor() {
    let source = |value: &str| {
        SOURCE
            .replace(
                "owner: #1\nwizard",
                &format!("name: \"{value}\"\nowner: #1\nwizard"),
            )
            .replace(
                "endobject",
                &format!("property title (owner: #1, flags: \"r\") = \"{value}\";\nendobject"),
            )
    };
    let parse = |text: &str| {
        ObjDefSet::parse_sources(
            &CompileOptions::default(),
            None,
            None,
            [ObjDefSource::new("core.moo", text)],
        )
        .unwrap()
    };
    let base = parse(&source("Base"));
    for (local, incoming, expected) in [
        ("Base", "Base", None),
        ("Local", "Base", Some("local")),
        ("Base", "Upstream", Some("upstream")),
        ("Local", "Upstream", Some("conflict")),
        ("Same", "Same", Some("converged")),
    ] {
        let db = TxDB::try_open(None, DatabaseConfig::default()).unwrap().0;
        let mut loader = db.loader_client().unwrap();
        let local_source = if expected == Some("local") {
            source(local).replace("wizard: true", "wizard: true\nreadable: true")
        } else {
            source(local)
        };
        ObjectDefinitionLoader::new(loader.as_mut())
            .load_objdef_set(
                parse(&local_source).with_baseline(&base, None).unwrap(),
                Default::default(),
            )
            .unwrap();
        loader.commit().unwrap();
        let world = db.new_world_state().unwrap();
        let req = record(&[
            ("schema", v_int(1)),
            ("operation", v_str("inspect")),
            ("objects", v_list(&[v_obj(ROOT)])),
            ("fields", v_list(&[v_str("attribute"), v_str("property")])),
        ]);
        let report = preview(
            world.as_ref(),
            &permissions(),
            &CompileOptions::default(),
            &sources(&source(incoming)),
            &req,
            None,
        )
        .unwrap();
        let rows = get(&report, "rows");
        let rows = rows.as_list().unwrap();
        if let Some(expected) = expected {
            assert_eq!(rows.len(), if expected == "local" { 3 } else { 2 });
            for row in rows.iter() {
                assert_eq!(get(&row, "classification"), v_str(expected));
                assert_eq!(get(&row, "read_only"), v_bool(true));
            }
        } else {
            assert!(rows.is_empty());
        }
    }
}

#[test]
fn inherited_property_inspection_uses_imported_permissions() {
    let parse = |text: &str| {
        ObjDefSet::parse_sources(
            &CompileOptions::default(),
            None,
            None,
            [ObjDefSource::new("core.moo", text)],
        )
        .unwrap()
    };
    for flags in ["r", "rc"] {
        let owner = if flags == "rc" { 2 } else { 1 };
        let source = |child_permissions: &str| {
            format!(
                r#"
object #1
owner: #1
wizard: true
property foo (owner: #1, flags: "{flags}") = 0;
property note (owner: #1, flags: "{flags}") = 0;
endobject
object #2
parent: #1
owner: #1
override foo (owner: #1, flags: "w") = 2;
endobject
object #3
parent: #2
owner: #2
override foo {child_permissions} = 1;
override note [comment -> "Child metadata"];
endobject
"#
            )
        };
        let same = format!("(owner: #{owner}, flags: \"{flags}\")");
        let edited = format!("(owner: #{owner}, flags: \"w\")");
        let other = format!("(owner: #{owner}, flags: \"rw\")");
        let base = parse(&source(""));
        for (local, incoming, expected) in [
            ("", "", None),
            (same.as_str(), "", None),
            ("", same.as_str(), None),
            (edited.as_str(), "", Some("local")),
            ("", edited.as_str(), Some("upstream")),
            (edited.as_str(), edited.as_str(), Some("converged")),
            (edited.as_str(), other.as_str(), Some("conflict")),
        ] {
            let db = TxDB::try_open(None, DatabaseConfig::default()).unwrap().0;
            let mut loader = db.loader_client().unwrap();
            ObjectDefinitionLoader::new(loader.as_mut())
                .load_objdef_set(
                    parse(&source(local)).with_baseline(&base, None).unwrap(),
                    Default::default(),
                )
                .unwrap();
            loader.commit().unwrap();
            let world = db.new_world_state().unwrap();
            let req = record(&[
                ("schema", v_int(1)),
                ("operation", v_str("inspect")),
                ("objects", v_list(&[v_obj(Obj::mk_id(3))])),
                ("fields", v_list(&[v_str("property")])),
            ]);
            let report = preview(
                world.as_ref(),
                &permissions(),
                &CompileOptions::default(),
                &sources(&source(incoming)),
                &req,
                None,
            )
            .unwrap();
            let rows = get(&report, "rows");
            let rows = rows.as_list().unwrap();
            if let Some(expected) = expected {
                assert_eq!(rows.len(), 1);
                let row = rows.iter().next().unwrap();
                assert_eq!(get(&row, "name"), v_str("foo"));
                assert_eq!(get(&row, "classification"), v_str(expected));
            } else {
                assert!(
                    rows.is_empty(),
                    "{flags}: local={local}, incoming={incoming}: {rows:?}"
                );
            }
        }
    }
}

fn with_field(value: &Var, key: &str, field: &Var) -> Var {
    value.as_map().unwrap().set(&v_str(key), field).unwrap()
}

#[test]
fn historical_details_verify_member_identity_and_hash_without_changing_evidence() {
    let db = database();
    let mut world = db.new_world_state().unwrap();
    stamp(world.as_mut());
    let definition = world
        .get_verb(&permissions(), &ROOT, Symbol::mk("test"))
        .unwrap();
    world
        .update_verb_with_id(
            &permissions(),
            &ROOT,
            definition.uuid(),
            VerbAttrs {
                definer: None,
                owner: None,
                names: None,
                flags: None,
                args_spec: None,
                program: Some(ProgramType::MooR(
                    compile("return \"Local\";", Default::default()).unwrap(),
                )),
            },
        )
        .unwrap();
    world.commit().unwrap();
    let world = db.new_world_state().unwrap();
    let incoming = sources(&SOURCE.replace("Base", "Incoming"));
    let report = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &request("update"),
        None,
    )
    .unwrap();
    assert_eq!(get(&row(&report), "classification"), v_str("conflict"));
    let details = with_field(
        &request("update"),
        "details",
        &v_list(&[get(&row(&report), "id")]),
    );
    let historical = with_field(&details, "base_sources", &sources(SOURCE));
    let verified = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &historical,
        None,
    )
    .unwrap();
    assert_eq!(get(&verified, "evidence"), get(&report, "evidence"));
    assert_eq!(get(&row(&verified), "base_text_available"), v_bool(true));
    assert_eq!(get(&row(&verified), "base_text"), v_str("return \"Base\";"));
    assert_eq!(
        get(&row(&verified), "live_text"),
        v_str("return \"Local\";")
    );
    assert_eq!(
        get(&row(&verified), "incoming_text"),
        v_str("return \"Incoming\";")
    );
    for bad in [
        SOURCE.replace("Base", "Wrong"),
        SOURCE.replace("verb test", "verb other"),
        SOURCE.replace("review_root", "wrong_identity"),
        SOURCE.replace("object #1", "object #2"),
    ] {
        let request = with_field(&details, "base_sources", &sources(&bad));
        assert!(
            preview(
                world.as_ref(),
                &permissions(),
                &CompileOptions::default(),
                &incoming,
                &request,
                None
            )
            .is_err()
        );
    }
    let invalid_scope = with_field(&request("update"), "base_sources", &sources(SOURCE));
    assert!(
        preview(
            world.as_ref(),
            &permissions(),
            &CompileOptions::default(),
            &incoming,
            &invalid_scope,
            None
        )
        .is_err()
    );
    assert!(matches!(
        world.commit().unwrap(),
        CommitResult::Success {
            mutations_made: false,
            ..
        }
    ));
}

#[test]
fn accepted_baselines_retain_incoming_provenance_for_each_decision() {
    for decision in ["incoming", "local", "edited", "defer"] {
        let db = database();
        let mut world = db.new_world_state().unwrap();
        let origin = record(&[
            ("transport", v_str("git")),
            ("commit", v_str("ancestor")),
            ("label", v_str("untrusted label")),
        ]);
        let adopt = with_field(&request("adopt"), "provenance", &origin);
        let initial = preview(
            world.as_ref(),
            &permissions(),
            &CompileOptions::default(),
            &sources(SOURCE),
            &adopt,
            None,
        )
        .unwrap();
        apply(
            world.as_mut(),
            &permissions(),
            &CompileOptions::default(),
            &sources(SOURCE),
            &adopt,
            &get(&initial, "evidence"),
            &v_map(&[]),
        )
        .unwrap();
        let id = get(&row(&initial), "id");
        let incoming = sources(&SOURCE.replace("Base", "Incoming"));
        let next_origin = with_field(&origin, "commit", &v_str("incoming"));
        let update = with_field(&request("update"), "provenance", &next_origin);
        let report = preview(
            world.as_ref(),
            &permissions(),
            &CompileOptions::default(),
            &incoming,
            &update,
            None,
        )
        .unwrap();
        let mut selected = record(&[("choice", v_str(decision))]);
        if decision == "edited" {
            selected = with_field(&selected, "program", &v_str("return \"Edited\";"));
            let validation = preview(
                world.as_ref(),
                &permissions(),
                &CompileOptions::default(),
                &incoming,
                &update,
                Some(&v_map(&[(id.clone(), selected.clone())])),
            )
            .unwrap();
            let valid = get(&validation, "validation")
                .as_list()
                .unwrap()
                .iter()
                .next()
                .unwrap();
            selected = with_field(&selected, "validation", &get(&valid, "validation"));
        }
        // Provenance is part of the reviewed write, even if the program input is unchanged.
        let changed = with_field(&update, "provenance", &origin);
        assert!(
            apply(
                world.as_mut(),
                &permissions(),
                &CompileOptions::default(),
                &incoming,
                &changed,
                &get(&report, "evidence"),
                &v_map(&[(id.clone(), selected.clone())])
            )
            .is_err()
        );
        apply(
            world.as_mut(),
            &permissions(),
            &CompileOptions::default(),
            &incoming,
            &update,
            &get(&report, "evidence"),
            &v_map(&[(id.clone(), selected)]),
        )
        .unwrap();
        world.commit().unwrap();
        let world = db.new_world_state().unwrap();
        let details = with_field(&request("update"), "details", &v_list(&[id]));
        let report = preview(
            world.as_ref(),
            &permissions(),
            &CompileOptions::default(),
            &incoming,
            &details,
            None,
        )
        .unwrap();
        let row = row(&report);
        let source = get(&row, "base_source");
        assert_eq!(
            get(&source, "commit"),
            v_str(if decision == "defer" {
                "ancestor"
            } else {
                "incoming"
            })
        );
        assert_eq!(get(&source, "label"), v_str("root.moo"));
        let accepted = if decision == "defer" {
            sources(SOURCE)
        } else {
            incoming.clone()
        };
        assert_eq!(get(&source, "digest"), v_str(&digest(&accepted).unwrap()));
        let verified = with_field(&details, "base_sources", &accepted);
        assert!(
            preview(
                world.as_ref(),
                &permissions(),
                &CompileOptions::default(),
                &incoming,
                &verified,
                None
            )
            .is_ok()
        );
        if decision == "edited" {
            assert_eq!(get(&row, "live_text"), v_str("return \"Edited\";"));
            assert_eq!(get(&row, "base"), get(&row, "incoming"));
        }
    }
}

#[test]
fn provenance_is_bounded_and_requires_a_map() {
    let db = database();
    let world = db.new_world_state().unwrap();
    for invalid in [v_str("git"), record(&[("text", v_str(&"x".repeat(8193)))])] {
        let request = with_field(&request("adopt"), "provenance", &invalid);
        assert!(
            preview(
                world.as_ref(),
                &permissions(),
                &CompileOptions::default(),
                &sources(SOURCE),
                &request,
                None
            )
            .is_err()
        );
    }
}

fn source_with_helper() -> String {
    SOURCE
        .replace("return \"Base\";", "return this:helper();")
        .replace(
            "endobject",
            "verb helper (this none this) owner: #1 flags: \"rxd\"\nreturn 42;\nendverb\nendobject",
        )
}

fn grouped_request(report: &Var) -> Var {
    let members = get(report, "rows")
        .as_list()
        .unwrap()
        .iter()
        .map(|row| get(&row, "member"))
        .collect::<Vec<_>>();
    with_field(
        &request("update"),
        "policy",
        &record(&[
            ("schema", v_int(1)),
            (
                "groups",
                v_list(&[record(&[
                    ("name", v_str("feature")),
                    ("members", v_list(&members)),
                    ("requires", v_list(&[])),
                ])]),
            ),
        ]),
    )
}

#[test]
fn verb_additions_require_a_complete_explicit_group_and_allocate_only_on_apply() {
    let db = database();
    let mut world = db.new_world_state().unwrap();
    stamp(world.as_mut());
    world.commit().unwrap();
    let mut world = db.new_world_state().unwrap();
    let incoming = sources(&source_with_helper());
    let blocked = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &request("update"),
        None,
    )
    .unwrap();
    assert_eq!(get(&blocked, "rows").as_list().unwrap().len(), 2);
    for row in get(&blocked, "rows").as_list().unwrap().iter() {
        assert!(!get(&row, "eligible").is_true());
        assert!(
            get(&row, "blockers")
                .as_list()
                .unwrap()
                .iter()
                .any(|reason| reason == v_str("change_group_required"))
        );
    }
    let grouped = grouped_request(&blocked);
    let report = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &grouped,
        None,
    )
    .unwrap();
    assert_eq!(world.verbs(&permissions(), &ROOT).unwrap().len(), 1);
    let rows = get(&report, "rows");
    let addition = rows
        .as_list()
        .unwrap()
        .iter()
        .find(|row| get(row, "classification") == v_str("incoming_only"))
        .unwrap();
    let detailed = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &with_field(&grouped, "details", &v_list(&[get(&addition, "id")])),
        None,
    )
    .unwrap();
    let definition = get(&detailed, "rows")
        .as_list()
        .unwrap()
        .iter()
        .find(|row| get(row, "id") == get(&addition, "id"))
        .unwrap();
    assert_eq!(
        get(&definition, "definition_text"),
        v_str("Owner: #1\nFlags: rxd\nArguments: this none this")
    );
    let caller = rows
        .as_list()
        .unwrap()
        .iter()
        .find(|row| get(row, "classification") == v_str("upstream"))
        .unwrap();
    let only_caller = v_map(&[(get(&caller, "id"), record(&[("choice", v_str("incoming"))]))]);
    let draft = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &grouped,
        Some(&only_caller),
    )
    .unwrap();
    assert_eq!(get(&draft, "group_validation").as_list().unwrap().len(), 1);
    assert!(
        apply(
            world.as_mut(),
            &permissions(),
            &CompileOptions::default(),
            &incoming,
            &grouped,
            &get(&report, "evidence"),
            &only_caller
        )
        .is_err()
    );
    assert_eq!(
        program_text(&live_program(world.as_ref())).unwrap(),
        v_str("return \"Base\";")
    );
    let choices = v_map(
        &rows
            .as_list()
            .unwrap()
            .iter()
            .map(|row| (get(&row, "id"), record(&[("choice", v_str("incoming"))])))
            .collect::<Vec<_>>(),
    );
    apply(
        world.as_mut(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &grouped,
        &get(&report, "evidence"),
        &choices,
    )
    .unwrap();
    world.commit().unwrap();
    let world = db.new_world_state().unwrap();
    assert_eq!(world.verbs(&permissions(), &ROOT).unwrap().len(), 2);
    let next = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &grouped,
        None,
    )
    .unwrap();
    for row in get(&next, "rows").as_list().unwrap().iter() {
        assert_eq!(get(&row, "classification"), v_str("unchanged"));
        assert_eq!(get(&row, "group"), v_str("feature"));
        assert!(get(&row, "eligible").is_true());
    }
}

#[test]
fn concurrent_verb_creation_rejects_reviewed_absence_without_touching_the_caller() {
    let db = database();
    let mut world = db.new_world_state().unwrap();
    stamp(world.as_mut());
    world.commit().unwrap();
    let mut reviewed = db.new_world_state().unwrap();
    let incoming = sources(&source_with_helper());
    let blocked = preview(
        reviewed.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &request("update"),
        None,
    )
    .unwrap();
    let grouped = grouped_request(&blocked);
    let report = preview(
        reviewed.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &grouped,
        None,
    )
    .unwrap();
    let choices = v_map(
        &get(&report, "rows")
            .as_list()
            .unwrap()
            .iter()
            .map(|row| (get(&row, "id"), record(&[("choice", v_str("incoming"))])))
            .collect::<Vec<_>>(),
    );
    let mut other = db.new_world_state().unwrap();
    let definition = other
        .get_verb(&permissions(), &ROOT, Symbol::mk("test"))
        .unwrap();
    other
        .add_verb(
            &permissions(),
            &ROOT,
            vec![Symbol::mk("helper")],
            &ROOT,
            definition.flags(),
            definition.args(),
            ProgramType::MooR(compile("return 99;", Default::default()).unwrap()),
        )
        .unwrap();
    other.commit().unwrap();
    // The original snapshot cannot commit its addition over a concurrent definition.
    apply(
        reviewed.as_mut(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &grouped,
        &get(&report, "evidence"),
        &choices,
    )
    .unwrap();
    assert!(matches!(
        reviewed.commit().unwrap(),
        CommitResult::ConflictRetry { .. }
    ));
    let mut current = db.new_world_state().unwrap();
    assert!(
        apply(
            current.as_mut(),
            &permissions(),
            &CompileOptions::default(),
            &incoming,
            &grouped,
            &get(&report, "evidence"),
            &choices
        )
        .is_err()
    );
    assert_eq!(
        program_text(&live_program(current.as_ref())).unwrap(),
        v_str("return \"Base\";")
    );
    assert_eq!(current.verbs(&permissions(), &ROOT).unwrap().len(), 2);
}

#[test]
fn groups_reject_cycles_duplicate_members_and_missing_prerequisites() {
    let db = database();
    let mut world = db.new_world_state().unwrap();
    stamp(world.as_mut());
    let incoming = sources(&source_with_helper());
    let initial = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &request("update"),
        None,
    )
    .unwrap();
    let rows: Vec<_> = get(&initial, "rows").as_list().unwrap().iter().collect();
    let group = |name: &str, member: Var, requires: &[Var]| {
        record(&[
            ("name", v_str(name)),
            ("members", v_list(&[member])),
            ("requires", v_list(requires)),
        ])
    };
    let policy_request = |groups: &[Var]| {
        with_field(
            &request("update"),
            "policy",
            &record(&[("schema", v_int(1)), ("groups", v_list(groups))]),
        )
    };
    for invalid in [
        vec![group("caller", get(&rows[0], "member"), &[v_str("helper")])],
        vec![
            group("caller", get(&rows[0], "member"), &[v_str("helper")]),
            group("helper", get(&rows[1], "member"), &[v_str("caller")]),
        ],
        vec![
            group("caller", get(&rows[0], "member"), &[]),
            group("helper", get(&rows[0], "member"), &[]),
        ],
    ] {
        assert!(
            preview(
                world.as_ref(),
                &permissions(),
                &CompileOptions::default(),
                &incoming,
                &policy_request(&invalid),
                None
            )
            .is_err()
        );
    }
    let request = policy_request(&[
        group("caller", get(&rows[0], "member"), &[v_str("helper")]),
        group("helper", get(&rows[1], "member"), &[]),
    ]);
    let choices = v_map(&[(
        get(&rows[0], "id"),
        record(&[("choice", v_str("incoming"))]),
    )]);
    let report = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &request,
        Some(&choices),
    )
    .unwrap();
    let issue = get(&report, "group_validation")
        .as_list()
        .unwrap()
        .iter()
        .next()
        .unwrap();
    assert_eq!(get(&issue, "code"), v_str("missing_prerequisite"));
    assert!(
        apply(
            world.as_mut(),
            &permissions(),
            &CompileOptions::default(),
            &incoming,
            &request,
            &get(&report, "evidence"),
            &choices
        )
        .is_err()
    );
}

#[test]
fn new_verbs_reject_alias_collisions_and_untrusted_definition_owners() {
    for source in [
        source_with_helper().replace("verb helper", "verb \"te*st\""),
        source_with_helper().replace("verb helper", "verb \"*suffix\""),
        source_with_helper().replace(
            "verb helper (this none this) owner: #1",
            "verb helper (this none this) owner: #9",
        ),
    ] {
        let db = database();
        let mut world = db.new_world_state().unwrap();
        stamp(world.as_mut());
        let incoming = sources(&source);
        let initial = preview(
            world.as_ref(),
            &permissions(),
            &CompileOptions::default(),
            &incoming,
            &request("update"),
            None,
        )
        .unwrap();
        let request = grouped_request(&initial);
        let report = preview(
            world.as_ref(),
            &permissions(),
            &CompileOptions::default(),
            &incoming,
            &request,
            None,
        )
        .unwrap();
        let addition = get(&report, "rows")
            .as_list()
            .unwrap()
            .iter()
            .find(|row| get(row, "classification") == v_str("incoming_only"))
            .unwrap();
        assert!(!get(&addition, "eligible").is_true());
        let choices = v_map(&[(
            get(&addition, "id"),
            record(&[("choice", v_str("incoming"))]),
        )]);
        assert!(
            apply(
                world.as_mut(),
                &permissions(),
                &CompileOptions::default(),
                &incoming,
                &request,
                &get(&report, "evidence"),
                &choices
            )
            .is_err()
        );
        assert_eq!(world.verbs(&permissions(), &ROOT).unwrap().len(), 1);
    }
}

#[test]
fn a_late_failure_rolls_back_a_new_verb_and_its_baseline() {
    let db = database();
    let mut world = db.new_world_state().unwrap();
    let parsed = ObjDefSet::parse_sources(
        &CompileOptions::default(),
        None,
        None,
        decode_sources(&sources(&source_with_helper())).unwrap(),
    )
    .unwrap();
    let helper = parsed
        .graph()
        .object_definitions()
        .get(&ROOT)
        .unwrap()
        .1
        .verbs
        .iter()
        .find(|verb| verb.names == vec![Symbol::mk("helper")])
        .unwrap()
        .clone();
    let baseline = record(&[
        ("schema", v_str(PROGRAM_SCHEMA)),
        (
            "program",
            v_str(&program_fingerprint(&helper.program).unwrap()),
        ),
    ]);
    let actions = vec![
        ChangeAction {
            object: ROOT,
            target: ChangeTarget::AddVerb(helper),
            program: None,
            baseline: baseline.clone(),
        },
        ChangeAction {
            object: ROOT,
            target: ChangeTarget::Existing(uuid::Uuid::nil()),
            program: None,
            baseline,
        },
    ];
    assert!(matches!(
        write_actions(world.as_mut(), &permissions(), actions),
        Err(ApplyError::Mutation(_))
    ));
    drop(world);
    let world = db.new_world_state().unwrap();
    assert_eq!(world.verbs(&permissions(), &ROOT).unwrap().len(), 1);
    assert_eq!(
        program_text(&live_program(world.as_ref())).unwrap(),
        v_str("return \"Base\";")
    );
}

fn property_source() -> String {
    source_with_helper()
        .replace("return 42;", "return this.item_schema;")
        .replace(
            "endobject",
            "property item_schema (owner: #1, flags: \"r\") = 42;\nendobject",
        )
}
fn property_request(report: &Var, descendants: bool) -> Var {
    let grouped = grouped_request(report);
    let policy = with_field(
        &get(&grouped, "policy"),
        "initialization",
        &v_list(&[record(&[
            (
                "member",
                v_str(&properties::member_key(ROOT, Symbol::mk("item_schema"))),
            ),
            ("mode", v_str("absent")),
            ("descendants", v_bool(descendants)),
        ])]),
    );
    with_field(
        &with_field(&grouped, "policy", &policy),
        "fields",
        &v_list(&[v_str("program"), v_str("property")]),
    )
}
fn all_incoming(report: &Var) -> Var {
    v_map(
        &get(report, "rows")
            .as_list()
            .unwrap()
            .iter()
            .map(|row| {
                let choices = get(&row, "choices");
                let choice = if choices
                    .as_list()
                    .unwrap()
                    .iter()
                    .any(|v| v == v_str("incoming"))
                {
                    "incoming"
                } else {
                    "local"
                };
                (get(&row, "id"), record(&[("choice", v_str(choice))]))
            })
            .collect::<Vec<_>>(),
    )
}

#[test]
fn property_initialization_is_absent_only_and_keeps_later_live_data() {
    let db = database();
    let mut world = db.new_world_state().unwrap();
    stamp(world.as_mut());
    let incoming = sources(&property_source());
    let fields_request = with_field(
        &request("update"),
        "fields",
        &v_list(&[v_str("program"), v_str("property")]),
    );
    let initial = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &fields_request,
        None,
    )
    .unwrap();
    assert_eq!(get(&initial, "rows").as_list().unwrap().len(), 3);
    let properties_only = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &with_field(&fields_request, "fields", &v_list(&[v_str("property")])),
        None,
    )
    .unwrap();
    assert_eq!(get(&properties_only, "rows").as_list().unwrap().len(), 1);
    assert_eq!(get(&row(&properties_only), "field"), v_str("property"));
    assert!(
        world
            .get_property_info(&permissions(), &ROOT, Symbol::mk("item_schema"))
            .is_err()
    );
    let req = property_request(&initial, false);
    let report = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &req,
        None,
    )
    .unwrap();
    let choices = all_incoming(&report);
    let receipt = apply(
        world.as_mut(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &req,
        &get(&report, "evidence"),
        &choices,
    )
    .unwrap();
    assert_eq!(get(&receipt, "created").as_list().unwrap().len(), 2);
    assert_eq!(
        world
            .retrieve_property(&permissions(), &ROOT, Symbol::mk("item_schema"))
            .unwrap(),
        v_int(42)
    );
    let metadata = world
        .get_object_metadata(&permissions(), &ROOT, Symbol::mk(BASE_KEY))
        .unwrap();
    world
        .update_property(&permissions(), &ROOT, Symbol::mk("item_schema"), &v_int(99))
        .unwrap();
    let report = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &req,
        None,
    )
    .unwrap();
    let property = get(&report, "rows")
        .as_list()
        .unwrap()
        .iter()
        .find(|row| get(row, "field") == v_str("property"))
        .unwrap();
    assert_eq!(get(&property, "classification"), v_str("local"));
    assert_eq!(
        get(&property, "choices"),
        v_list(&[v_str("local"), v_str("defer")])
    );
    apply(
        world.as_mut(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &req,
        &get(&report, "evidence"),
        &all_incoming(&report),
    )
    .unwrap();
    assert_eq!(
        world
            .retrieve_property(&permissions(), &ROOT, Symbol::mk("item_schema"))
            .unwrap(),
        v_int(99)
    );
    assert_eq!(
        world
            .get_object_metadata(&permissions(), &ROOT, Symbol::mk(BASE_KEY))
            .unwrap(),
        metadata
    );
}

#[test]
fn property_addition_rechecks_absence_and_rolls_back_late_failure() {
    let db = database();
    let mut world = db.new_world_state().unwrap();
    stamp(world.as_mut());
    world.commit().unwrap();
    let mut world = db.new_world_state().unwrap();
    let incoming = sources(&property_source());
    let fields_request = with_field(
        &request("update"),
        "fields",
        &v_list(&[v_str("program"), v_str("property")]),
    );
    let initial = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &fields_request,
        None,
    )
    .unwrap();
    let req = property_request(&initial, false);
    let report = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &req,
        None,
    )
    .unwrap();
    world
        .define_property(
            &permissions(),
            &ROOT,
            &ROOT,
            Symbol::mk("item_schema"),
            &ROOT,
            moor_common::model::PropFlag::r(),
            Some(v_int(77)),
        )
        .unwrap();
    assert!(
        apply(
            world.as_mut(),
            &permissions(),
            &CompileOptions::default(),
            &incoming,
            &req,
            &get(&report, "evidence"),
            &all_incoming(&report)
        )
        .is_err()
    );
    assert_eq!(world.verbs(&permissions(), &ROOT).unwrap().len(), 1);
    drop(world);
    let mut world = db.new_world_state().unwrap();
    let set = ObjDefSet::parse_sources(
        &CompileOptions::default(),
        None,
        None,
        vec![ObjDefSource::new("property.moo", property_source())],
    )
    .unwrap();
    let property = set.graph().object_definitions()[&ROOT]
        .1
        .property_definitions[0]
        .clone();
    let result = write_actions(
        world.as_mut(),
        &permissions(),
        vec![
            ChangeAction {
                object: ROOT,
                target: ChangeTarget::AddProperty(property),
                program: None,
                baseline: v_list(&[]),
            },
            ChangeAction {
                object: ROOT,
                target: ChangeTarget::Existing(uuid::Uuid::nil()),
                program: None,
                baseline: v_list(&[]),
            },
        ],
    );
    assert!(matches!(result, Err(ApplyError::Mutation(_))));
    drop(world);
    let world = db.new_world_state().unwrap();
    assert!(
        world
            .get_property_info(&permissions(), &ROOT, Symbol::mk("item_schema"))
            .is_err()
    );
    assert!(
        world
            .get_object_metadata(&permissions(), &ROOT, Symbol::mk(BASE_KEY))
            .unwrap()
            .is_none()
    );
}

#[test]
fn property_additions_require_explicit_descendant_permission_and_reject_collisions() {
    let db = database();
    let mut world = db.new_world_state().unwrap();
    stamp(world.as_mut());
    let child = world
        .create_object(
            &permissions(),
            &ROOT,
            &ROOT,
            BitEnum::new(),
            moor_common::model::ObjectKind::NextObjid,
        )
        .unwrap();
    let incoming = sources(&property_source());
    let fields_request = with_field(
        &request("update"),
        "fields",
        &v_list(&[v_str("program"), v_str("property")]),
    );
    let initial = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &fields_request,
        None,
    )
    .unwrap();
    let req = property_request(&initial, false);
    let report = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &req,
        None,
    )
    .unwrap();
    let property = get(&report, "rows")
        .as_list()
        .unwrap()
        .iter()
        .find(|row| get(row, "field") == v_str("property"))
        .unwrap();
    assert_eq!(get(&property, "descendants"), v_int(1));
    assert!(
        get(&property, "blockers")
            .as_list()
            .unwrap()
            .iter()
            .any(|value| value == v_str("inherited_effects_require_approval"))
    );
    let req = property_request(&initial, true);
    let report = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &req,
        None,
    )
    .unwrap();
    world
        .define_property(
            &permissions(),
            &child,
            &child,
            Symbol::mk("item_schema"),
            &ROOT,
            moor_common::model::PropFlag::r(),
            Some(v_int(77)),
        )
        .unwrap();
    assert!(
        apply(
            world.as_mut(),
            &permissions(),
            &CompileOptions::default(),
            &incoming,
            &req,
            &get(&report, "evidence"),
            &all_incoming(&report)
        )
        .is_err()
    );
    let current = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &req,
        None,
    )
    .unwrap();
    let property = get(&current, "rows")
        .as_list()
        .unwrap()
        .iter()
        .find(|row| get(row, "field") == v_str("property"))
        .unwrap();
    assert!(
        get(&property, "blockers")
            .as_list()
            .unwrap()
            .iter()
            .any(|value| value == v_str("property_name_collision"))
    );
    world
        .delete_property(&permissions(), &child, Symbol::mk("item_schema"))
        .unwrap();
    let report = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &req,
        None,
    )
    .unwrap();
    apply(
        world.as_mut(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &req,
        &get(&report, "evidence"),
        &all_incoming(&report),
    )
    .unwrap();
    assert_eq!(
        world
            .retrieve_property(&permissions(), &child, Symbol::mk("item_schema"))
            .unwrap(),
        v_int(42)
    );
}

#[test]
fn concurrent_property_creation_prevents_partial_group_publication() {
    let db = database();
    let mut initial = db.new_world_state().unwrap();
    stamp(initial.as_mut());
    initial.commit().unwrap();
    let mut review = db.new_world_state().unwrap();
    let incoming = sources(&property_source());
    let fields_request = with_field(
        &request("update"),
        "fields",
        &v_list(&[v_str("program"), v_str("property")]),
    );
    let first = preview(
        review.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &fields_request,
        None,
    )
    .unwrap();
    let req = property_request(&first, false);
    let report = preview(
        review.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &req,
        None,
    )
    .unwrap();
    let mut concurrent = db.new_world_state().unwrap();
    concurrent
        .define_property(
            &permissions(),
            &ROOT,
            &ROOT,
            Symbol::mk("item_schema"),
            &ROOT,
            moor_common::model::PropFlag::r(),
            Some(v_int(77)),
        )
        .unwrap();
    assert!(matches!(
        concurrent.commit().unwrap(),
        CommitResult::Success { .. }
    ));
    apply(
        review.as_mut(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &req,
        &get(&report, "evidence"),
        &all_incoming(&report),
    )
    .unwrap();
    assert!(matches!(
        review.commit().unwrap(),
        CommitResult::ConflictRetry { .. }
    ));
    let mut fresh = db.new_world_state().unwrap();
    assert!(
        apply(
            fresh.as_mut(),
            &permissions(),
            &CompileOptions::default(),
            &incoming,
            &req,
            &get(&report, "evidence"),
            &all_incoming(&report)
        )
        .is_err()
    );
    assert_eq!(fresh.verbs(&permissions(), &ROOT).unwrap().len(), 1);
    assert_eq!(
        fresh
            .retrieve_property(&permissions(), &ROOT, Symbol::mk("item_schema"))
            .unwrap(),
        v_int(77)
    );
}

fn object_source() -> String {
    format!(
        "{}\nobject HELPER\nowner: #1\nname: \"Helper\"\nproperty counter (owner: #1, flags: \"r\") = 42;\nmethod run owner: #1\nreturn this.counter;\nendmethod\nmethod initialize owner: #1\nthis.counter = 999;\nendmethod\nendobject",
        SOURCE.replace("return \"Base\";", "return HELPER:run();")
    )
}
fn object_request() -> Var {
    let req = with_field(
        &request("update"),
        "objects",
        &v_list(&[v_obj(ROOT), v_obj(Obj::mk_id(2))]),
    );
    let req = with_field(
        &req,
        "fields",
        &v_list(&[v_str("program"), v_str("property"), v_str("object")]),
    );
    let req = with_field(
        &req,
        "constants",
        &record(&[("HELPER", v_obj(Obj::mk_id(2)))]),
    );
    with_field(
        &req,
        "policy",
        &record(&[
            ("schema", v_int(1)),
            ("groups", v_list(&[])),
            (
                "creations",
                v_list(&[record(&[
                    ("object", v_obj(Obj::mk_id(2))),
                    ("mode", v_str("absent")),
                ])]),
            ),
        ]),
    )
}
fn group_object_request(req: &Var, report: &Var) -> Var {
    let groups = get(&get(&grouped_request(report), "policy"), "groups");
    with_field(
        req,
        "policy",
        &with_field(&get(req, "policy"), "groups", &groups),
    )
}
#[test]
fn object_bindings_create_initial_contents_without_running_initialize() {
    let db = database();
    let mut world = db.new_world_state().unwrap();
    stamp(world.as_mut());
    let incoming = sources(&object_source());
    let req = object_request();
    let first = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &req,
        None,
    )
    .unwrap();
    assert!(!world.valid(&Obj::mk_id(2)).unwrap());
    assert_eq!(get(&first, "rows").as_list().unwrap().len(), 2);
    let req = group_object_request(&req, &first);
    let report = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &req,
        None,
    )
    .unwrap();
    let receipt = apply(
        world.as_mut(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &req,
        &get(&report, "evidence"),
        &all_incoming(&report),
    )
    .unwrap();
    assert_eq!(
        world
            .retrieve_property(&permissions(), &Obj::mk_id(2), Symbol::mk("counter"))
            .unwrap(),
        v_int(42)
    );
    let created = get(&receipt, "created")
        .as_list()
        .unwrap()
        .iter()
        .find(|entry| get(entry, "field") == v_str("object"))
        .unwrap();
    let installed = with_field(
        &get(&get(&req, "policy"), "creations")
            .as_list()
            .unwrap()
            .iter()
            .next()
            .unwrap(),
        "installed",
        &get(&created, "creation"),
    );
    let req = with_field(
        &req,
        "policy",
        &with_field(&get(&req, "policy"), "creations", &v_list(&[installed])),
    );
    let next = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &req,
        None,
    )
    .unwrap();
    for row in get(&next, "rows").as_list().unwrap().iter() {
        assert_eq!(get(&row, "classification"), v_str("unchanged"));
    }
    world
        .update_property(
            &permissions(),
            &Obj::mk_id(2),
            Symbol::mk("counter"),
            &v_int(73),
        )
        .unwrap();
    let next = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &req,
        None,
    )
    .unwrap();
    apply(
        world.as_mut(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &req,
        &get(&next, "evidence"),
        &all_incoming(&next),
    )
    .unwrap();
    assert_eq!(
        world
            .retrieve_property(&permissions(), &Obj::mk_id(2), Symbol::mk("counter"))
            .unwrap(),
        v_int(73)
    );
    world
        .clear_object_metadata(&permissions(), &Obj::mk_id(2), Symbol::mk(BASE_KEY))
        .unwrap();
    let invalid = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &req,
        None,
    )
    .unwrap();
    let bound_rows: Vec<_> = get(&invalid, "rows")
        .as_list()
        .unwrap()
        .iter()
        .filter(|row| get(row, "object") == v_obj(Obj::mk_id(2)))
        .collect();
    assert_eq!(
        bound_rows.len(),
        1,
        "missing creation identity must block every member update"
    );
    assert!(!get(&bound_rows[0], "eligible").is_true());
}

#[test]
fn occupied_object_binding_never_updates_that_object() {
    let db = database();
    let mut world = db.new_world_state().unwrap();
    stamp(world.as_mut());
    let incoming = sources(&object_source());
    let req = object_request();
    let first = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &req,
        None,
    )
    .unwrap();
    let req = group_object_request(&req, &first);
    let report = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &req,
        None,
    )
    .unwrap();
    world
        .create_object(
            &permissions(),
            &NOTHING,
            &ROOT,
            BitEnum::new(),
            moor_common::model::ObjectKind::Objid(Obj::mk_id(2)),
        )
        .unwrap();
    assert!(
        apply(
            world.as_mut(),
            &permissions(),
            &CompileOptions::default(),
            &incoming,
            &req,
            &get(&report, "evidence"),
            &all_incoming(&report)
        )
        .is_err()
    );
    let current = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &req,
        None,
    )
    .unwrap();
    let object = get(&current, "rows")
        .as_list()
        .unwrap()
        .iter()
        .find(|row| get(row, "field") == v_str("object"))
        .unwrap();
    assert!(
        get(&object, "blockers")
            .as_list()
            .unwrap()
            .iter()
            .any(|blocker| blocker == v_str("object_address_occupied"))
    );
    assert!(
        world
            .verbs(&permissions(), &Obj::mk_id(2))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn new_objects_resolve_new_parent_properties_and_require_the_parent_selection() {
    let db = database();
    let mut world = db.new_world_state().unwrap();
    stamp(world.as_mut());
    let source = object_source()
        .replace("name: \"Helper\"", "name: \"Helper\"\nparent: #3")
        .replace(
            "property counter (owner: #1, flags: \"r\") = 42;",
            "override counter = 7;",
        )
        + "\nobject #3\nowner: #1\nproperty counter (owner: #1, flags: \"r\") = 1;\nendobject";
    let incoming = sources(&source);
    let req = with_field(
        &object_request(),
        "objects",
        &v_list(&[v_obj(ROOT), v_obj(Obj::mk_id(2)), v_obj(Obj::mk_id(3))]),
    );
    let policy = with_field(
        &get(&req, "policy"),
        "creations",
        &v_list(
            &[2, 3]
                .map(|id| record(&[("object", v_obj(Obj::mk_id(id))), ("mode", v_str("absent"))])),
        ),
    );
    let req = with_field(&req, "policy", &policy);
    let first = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &req,
        None,
    )
    .unwrap();
    let separate_groups = get(&first, "rows")
        .as_list()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(index, row)| {
            record(&[
                ("name", v_str(&format!("group{index}"))),
                ("members", v_list(&[get(&row, "member")])),
                ("requires", v_list(&[])),
            ])
        })
        .collect::<Vec<_>>();
    let separate = with_field(
        &req,
        "policy",
        &with_field(&policy, "groups", &v_list(&separate_groups)),
    );
    let report = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &separate,
        None,
    )
    .unwrap();
    let child = get(&report, "rows")
        .as_list()
        .unwrap()
        .iter()
        .find(|row| get(row, "object") == v_obj(Obj::mk_id(2)))
        .unwrap();
    let only_child = v_map(&[(get(&child, "id"), record(&[("choice", v_str("incoming"))]))]);
    let previewed = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &separate,
        Some(&only_child),
    )
    .unwrap();
    assert!(
        get(&previewed, "group_validation")
            .as_list()
            .unwrap()
            .iter()
            .any(|issue| get(&issue, "code") == v_str("missing_object_prerequisite"))
    );
    assert!(
        apply(
            world.as_mut(),
            &permissions(),
            &CompileOptions::default(),
            &incoming,
            &separate,
            &get(&report, "evidence"),
            &only_child
        )
        .is_err()
    );
    let req = group_object_request(&req, &first);
    let report = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &req,
        None,
    )
    .unwrap();
    apply(
        world.as_mut(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &req,
        &get(&report, "evidence"),
        &all_incoming(&report),
    )
    .unwrap();
    assert_eq!(
        world
            .retrieve_property(&permissions(), &Obj::mk_id(2), Symbol::mk("counter"))
            .unwrap(),
        v_int(7)
    );
    assert_eq!(
        world
            .retrieve_property(&permissions(), &Obj::mk_id(3), Symbol::mk("counter"))
            .unwrap(),
        v_int(1)
    );
}

#[test]
fn object_creation_conflicts_and_late_failures_publish_no_partial_group() {
    let db = database();
    let mut world = db.new_world_state().unwrap();
    stamp(world.as_mut());
    world.commit().unwrap();
    let mut world = db.new_world_state().unwrap();
    let incoming = sources(&object_source());
    let req = object_request();
    let first = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &req,
        None,
    )
    .unwrap();
    let req = group_object_request(&req, &first);
    let report = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &req,
        None,
    )
    .unwrap();
    let mut other = db.new_world_state().unwrap();
    other
        .create_object(
            &permissions(),
            &NOTHING,
            &ROOT,
            BitEnum::new(),
            moor_common::model::ObjectKind::Objid(Obj::mk_id(2)),
        )
        .unwrap();
    assert!(matches!(
        other.commit().unwrap(),
        CommitResult::Success { .. }
    ));
    apply(
        world.as_mut(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &req,
        &get(&report, "evidence"),
        &all_incoming(&report),
    )
    .unwrap();
    assert!(matches!(
        world.commit().unwrap(),
        CommitResult::ConflictRetry { .. }
    ));
    let mut fresh = db.new_world_state().unwrap();
    assert!(
        apply(
            fresh.as_mut(),
            &permissions(),
            &CompileOptions::default(),
            &incoming,
            &req,
            &get(&report, "evidence"),
            &all_incoming(&report)
        )
        .is_err()
    );
    assert!(
        fresh
            .verbs(&permissions(), &Obj::mk_id(2))
            .unwrap()
            .is_empty()
    );
    drop(fresh);

    let db = database();
    let mut world = db.new_world_state().unwrap();
    let analysis = analyze(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &req,
    )
    .unwrap();
    let object = analysis
        .rows
        .into_iter()
        .find(|row| matches!(row.target, ChangeTarget::AddObject(_)))
        .unwrap();
    let result = write_actions(
        world.as_mut(),
        &permissions(),
        vec![
            ChangeAction {
                object: object.object,
                target: object.target,
                program: None,
                baseline: record(&[("program", v_str(&object.incoming_hash))]),
            },
            ChangeAction {
                object: ROOT,
                target: ChangeTarget::Existing(uuid::Uuid::nil()),
                program: None,
                baseline: v_list(&[]),
            },
        ],
    );
    assert!(matches!(result, Err(ApplyError::Mutation(_))));
    drop(world);
    assert!(!db.new_world_state().unwrap().valid(&Obj::mk_id(2)).unwrap());
}

#[test]
fn complete_scope_reports_unsupported_removals_and_partial_scope_preserves_omissions() {
    let db = database();
    let world = db.new_world_state().unwrap();
    let incoming = sources("object #1\nowner: #1\nendobject");
    let partial = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &request("update"),
        None,
    )
    .unwrap();
    assert!(
        get(&partial, "diagnostics")
            .as_list()
            .unwrap()
            .iter()
            .any(|row| get(&row, "code") == v_str("live_definition_unmatched"))
    );
    let req = with_field(
        &request("update"),
        "policy",
        &record(&[
            ("schema", v_int(1)),
            ("groups", v_list(&[])),
            ("complete", v_list(&[v_obj(ROOT)])),
        ]),
    );
    let complete = preview(
        world.as_ref(),
        &permissions(),
        &CompileOptions::default(),
        &incoming,
        &req,
        None,
    )
    .unwrap();
    assert!(
        get(&complete, "diagnostics")
            .as_list()
            .unwrap()
            .iter()
            .any(|row| get(&row, "code") == v_str("unsupported_removal"))
    );
    assert_eq!(world.verbs(&permissions(), &ROOT).unwrap().len(), 1);
}

#[test]
fn new_object_review_rejects_invalid_relationships_owners_and_aliases() {
    for (change, reason) in [
        ("parent: #2", "invalid_object_parent"),
        ("location: #2", "invalid_object_location"),
        ("parent: #98765", "invalid_object_parent"),
        (
            "method initialize owner: #98765\nreturn 1;\nendmethod",
            "untrusted_target_authority",
        ),
        (
            "method run owner: #1\nreturn 0;\nendmethod",
            "verb_name_collision",
        ),
    ] {
        let db = database();
        let world = db.new_world_state().unwrap();
        let source =
            object_source().replace("name: \"Helper\"", &format!("name: \"Helper\"\n{change}"));
        let incoming = sources(&source);
        let req = object_request();
        let report = preview(
            world.as_ref(),
            &permissions(),
            &CompileOptions::default(),
            &incoming,
            &req,
            None,
        )
        .unwrap();
        let object = get(&report, "rows")
            .as_list()
            .unwrap()
            .iter()
            .find(|row| get(row, "field") == v_str("object"))
            .unwrap();
        assert!(
            get(&object, "blockers")
                .as_list()
                .unwrap()
                .iter()
                .any(|blocker| blocker == v_str(reason)),
            "{reason}"
        );
        assert!(!world.valid(&Obj::mk_id(2)).unwrap());
    }
}
