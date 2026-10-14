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

use crate::{ObjectCollectionError, ObjectDefinitionLoader, collect_object_definitions};
use moor_common::{
    model::{
        CommitResult, HasUuid, PropAttrs, PropFlag, TaskPermissions, ValSet, VerbAttrs, VerbFlag,
        WorldStateError, WorldStateSource,
    },
    util::BitEnum,
};
use moor_compiler::CompileOptions;
use moor_db::{Database, DatabaseConfig, TxDB};
use moor_var::{Obj, Symbol, v_str};

const ROOT: Obj = Obj::mk_id(1);
const CHILD: Obj = Obj::mk_id(2);
const LEAF: Obj = Obj::mk_id(3);
const ROOT_SOURCE: &str = r#"
    object #1 [source -> "upstream"]
        name: "Root"
        owner: #1
        parent: #-1
        location: #-1
        wizard: true
        readable: true
        property title (owner: #1, flags: "rw") [doc -> "parent metadata"] = "Hello";
        property automatic_owner (owner: #1, flags: "c") = 1;
        verb "look l" (this none this) owner: #1 flags: "rxd"
            return "Hello";
        endverb
    endobject
"#;
const CHILD_SOURCE: &str = r#"
    object #2
        name: "Child"
        owner: #2
        parent: #1
        location: #-1
        readable: true
    endobject
"#;
const LEAF_SOURCE: &str = r#"
    object #3
        name: "Leaf"
        owner: #2
        parent: #2
        location: #-1
        readable: true
    endobject
"#;

fn permissions(principal: Obj) -> TaskPermissions {
    TaskPermissions::new(principal, BitEnum::new())
}

fn database() -> TxDB {
    let db = TxDB::try_open_temporary(DatabaseConfig::default())
        .unwrap()
        .0;
    let mut loader = db.loader_client().unwrap();
    for source in [ROOT_SOURCE, CHILD_SOURCE, LEAF_SOURCE] {
        ObjectDefinitionLoader::new(loader.as_mut())
            .load_single_object(source, CompileOptions::default(), Default::default())
            .unwrap();
    }
    // An unrelated object must not be added to the result.
    ObjectDefinitionLoader::new(loader.as_mut())
        .load_single_object(
            "object #99 owner: #1 parent: #-1 location: #-1 endobject",
            CompileOptions::default(),
            Default::default(),
        )
        .unwrap();
    assert!(matches!(
        loader.commit().unwrap(),
        CommitResult::Success { .. }
    ));
    db
}

#[test]
fn collection_reads_ancestors_without_expanding_selection_or_writing() {
    let db = database();
    let world = db.new_world_state().unwrap();
    let defs =
        collect_object_definitions(world.as_ref(), &permissions(ROOT), &[LEAF, ROOT]).unwrap();
    assert_eq!(defs.iter().map(|d| d.oid).collect::<Vec<_>>(), [ROOT, LEAF]);
    assert!(defs[1].property_definitions.is_empty());
    assert!(defs[1].property_overrides.is_empty());
    assert_eq!(
        defs[0].verbs[0].names,
        [Symbol::mk("look"), Symbol::mk("l")]
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
fn collection_preserves_equal_values_permissions_and_holder_metadata() {
    let db = database();
    let mut world = db.new_world_state().unwrap();
    world
        .update_property(
            &permissions(ROOT),
            &CHILD,
            Symbol::mk("title"),
            &v_str("Hello"),
        )
        .unwrap();
    world
        .set_property_metadata(
            &permissions(ROOT),
            &CHILD,
            Symbol::mk("title"),
            Symbol::mk("doc"),
            v_str("child metadata"),
        )
        .unwrap();
    let defs = collect_object_definitions(world.as_ref(), &permissions(ROOT), &[CHILD]).unwrap();
    let prop = &defs[0].property_overrides[0];
    assert_eq!(prop.value.as_ref().unwrap().as_string(), Some("Hello"));
    assert!(prop.perms_update.is_some());
    assert_eq!(
        prop.metadata,
        [(Symbol::mk("doc"), v_str("child metadata"))]
    );
    world
        .clear_property(&permissions(ROOT), &CHILD, Symbol::mk("title"))
        .unwrap();
    let defs = collect_object_definitions(world.as_ref(), &permissions(ROOT), &[CHILD]).unwrap();
    let prop = &defs[0].property_overrides[0];
    assert!(prop.value.is_none());
    assert!(prop.perms_update.is_some());
    assert_eq!(
        prop.metadata,
        [(Symbol::mk("doc"), v_str("child metadata"))]
    );
}

#[test]
fn collection_observes_pending_values_without_mixing_transactions() {
    let db = database();
    let reader = db.new_world_state().unwrap();
    let mut writer = db.new_world_state().unwrap();
    writer
        .update_property(
            &permissions(ROOT),
            &CHILD,
            Symbol::mk("title"),
            &v_str("HELLO"),
        )
        .unwrap();
    let defs = collect_object_definitions(writer.as_ref(), &permissions(ROOT), &[CHILD]).unwrap();
    assert_eq!(
        defs[0].property_overrides[0]
            .value
            .as_ref()
            .unwrap()
            .as_string(),
        Some("HELLO")
    );
    writer.commit().unwrap();
    let defs = collect_object_definitions(reader.as_ref(), &permissions(ROOT), &[CHILD]).unwrap();
    assert!(defs[0].property_overrides.is_empty());
}

#[test]
fn collection_rejects_duplicate_missing_and_unreadable_targets() {
    let db = database();
    let mut world = db.new_world_state().unwrap();
    assert!(matches!(
        collect_object_definitions(world.as_ref(), &permissions(ROOT), &[ROOT, ROOT]),
        Err(ObjectCollectionError::DuplicateObject(ROOT))
    ));
    assert!(matches!(
        collect_object_definitions(world.as_ref(), &permissions(ROOT), &[Obj::mk_id(404)]),
        Err(ObjectCollectionError::Read {
            source: WorldStateError::ObjectNotFound(_),
            ..
        })
    ));
    world
        .set_property_info(
            &permissions(ROOT),
            &ROOT,
            Symbol::mk("title"),
            PropAttrs {
                flags: Some(BitEnum::new()),
                ..PropAttrs::default()
            },
        )
        .unwrap();
    assert!(matches!(
        collect_object_definitions(world.as_ref(), &permissions(CHILD), &[CHILD]),
        Err(ObjectCollectionError::Read {
            source: WorldStateError::PropertyPermissionDenied,
            ..
        })
    ));
    world
        .set_property_info(
            &permissions(ROOT),
            &ROOT,
            Symbol::mk("title"),
            PropAttrs {
                flags: Some(BitEnum::new_with(PropFlag::Read)),
                ..PropAttrs::default()
            },
        )
        .unwrap();
    let id = world
        .verbs(&permissions(ROOT), &ROOT)
        .unwrap()
        .iter()
        .next()
        .unwrap()
        .uuid();
    world
        .update_verb_with_id(
            &permissions(ROOT),
            &ROOT,
            id,
            VerbAttrs {
                flags: Some(BitEnum::new_with(VerbFlag::Exec)),
                definer: None,
                owner: None,
                names: None,
                program: None,
                args_spec: None,
            },
        )
        .unwrap();
    assert!(matches!(
        collect_object_definitions(world.as_ref(), &permissions(CHILD), &[ROOT]),
        Err(ObjectCollectionError::Read {
            source: WorldStateError::VerbPermissionDenied,
            ..
        })
    ));
}

#[test]
fn single_object_dump_preserves_local_state_through_reimport() {
    use crate::{collect_object, dump_object};
    use std::collections::HashMap;

    let db = database();
    let mut world = db.new_world_state().unwrap();
    for name in ["case", "permissions", "cleared", "inherited"] {
        world
            .define_property(
                &permissions(ROOT),
                &ROOT,
                &ROOT,
                Symbol::mk(name),
                &ROOT,
                BitEnum::new_with(PropFlag::Read) | PropFlag::Write,
                Some(v_str("Hello")),
            )
            .unwrap();
    }
    for (name, value) in [
        ("title", "Hello"),
        ("case", "hello"),
        ("cleared", "temporary"),
    ] {
        world
            .update_property(&permissions(ROOT), &CHILD, Symbol::mk(name), &v_str(value))
            .unwrap();
    }
    world
        .clear_property(&permissions(ROOT), &CHILD, Symbol::mk("cleared"))
        .unwrap();
    world
        .set_property_info(
            &permissions(ROOT),
            &CHILD,
            Symbol::mk("permissions"),
            PropAttrs {
                flags: Some(BitEnum::new_with(PropFlag::Read) | PropFlag::Write),
                ..PropAttrs::default()
            },
        )
        .unwrap();
    world
        .set_property_metadata(
            &permissions(ROOT),
            &CHILD,
            Symbol::mk("automatic_owner"),
            Symbol::mk("doc"),
            v_str("local metadata"),
        )
        .unwrap();
    world.commit().unwrap();

    let snapshot = db.create_snapshot().unwrap();
    let (_, _, _, child) = collect_object(snapshot.as_ref(), &CHILD).unwrap();
    let title = child
        .property_overrides
        .iter()
        .find(|p| p.name == Symbol::mk("title"))
        .expect("an explicit equal-valued override must survive dumping");
    assert_eq!(title.value.as_ref().unwrap().as_string(), Some("Hello"));
    assert!(title.perms_update.is_some());
    let metadata = child
        .property_overrides
        .iter()
        .find(|p| p.name == Symbol::mk("automatic_owner"))
        .expect("holder metadata without a local permission row must survive dumping");
    assert!(metadata.value.is_none());
    assert!(metadata.perms_update.is_none());

    let mut export = snapshot.begin_export(&[]).unwrap();
    while let Some(record) = export.next_object().unwrap() {
        let oid = record.oid;
        let streamed = crate::dump::collect_export_object(record).unwrap();
        let (_, _, _, point) = collect_object(snapshot.as_ref(), &oid).unwrap();
        assert_eq!(
            dump_object(&HashMap::new(), &point).unwrap(),
            dump_object(&HashMap::new(), &streamed).unwrap()
        );
    }

    let imported = TxDB::try_open_temporary(DatabaseConfig::default())
        .unwrap()
        .0;
    let mut loader = imported.loader_client().unwrap();
    for object in [ROOT, CHILD] {
        let (_, _, _, definition) = collect_object(snapshot.as_ref(), &object).unwrap();
        let lines = dump_object(&HashMap::new(), &definition).unwrap();
        let source = lines
            .iter()
            .map(|line| line.as_string().unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        ObjectDefinitionLoader::new(loader.as_mut())
            .load_single_object(&source, CompileOptions::default(), Default::default())
            .unwrap();
    }
    loader.commit().unwrap();
    let mut world = imported.new_world_state().unwrap();
    for name in ["title", "case", "permissions", "cleared", "inherited"] {
        world
            .update_property(
                &permissions(ROOT),
                &ROOT,
                Symbol::mk(name),
                &v_str("Changed"),
            )
            .unwrap();
    }
    for (name, value) in [
        ("title", "Hello"),
        ("case", "hello"),
        ("permissions", "Changed"),
        ("cleared", "Changed"),
        ("inherited", "Changed"),
    ] {
        assert_eq!(
            world
                .retrieve_property(&permissions(ROOT), &CHILD, Symbol::mk(name))
                .unwrap()
                .as_string(),
            Some(value),
            "{name}"
        );
    }
    for name in ["title", "case", "permissions", "cleared"] {
        let id = world
            .get_property_info(&permissions(ROOT), &CHILD, Symbol::mk(name))
            .unwrap()
            .0
            .uuid();
        assert!(
            world
                .snapshot_property(&permissions(ROOT), &CHILD, id)
                .unwrap()
                .permissions
                .is_some(),
            "{name}"
        );
    }
    let id = world
        .get_property_info(&permissions(ROOT), &CHILD, Symbol::mk("automatic_owner"))
        .unwrap()
        .0
        .uuid();
    let state = world
        .snapshot_property(&permissions(ROOT), &CHILD, id)
        .unwrap();
    assert!(state.permissions.is_none());
    assert_eq!(
        state.metadata,
        [(Symbol::mk("doc"), v_str("local metadata"))]
    );
}
