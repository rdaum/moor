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

//! Shared sparse-property and program contract for storage snapshot readers.
use moor_common::{
    model::{
        HasUuid, ObjAttrs, ObjectKind, PropFlag, TaskPermissions, ValSet, VerbArgsSpec, VerbFlag,
        WorldStateSource,
    },
    util::BitEnum,
};
use moor_db::{Database, DatabaseConfig, PersistenceConfig, StorageConfig, TxDB};
use moor_var::{NOTHING, Obj, Symbol, v_int, v_str};
use std::collections::BTreeMap;

fn contract(storage: StorageConfig) {
    let (db, _) = TxDB::try_open(
        storage,
        DatabaseConfig::default(),
        PersistenceConfig::default(),
    )
    .unwrap();
    let root = Obj::mk_id(0);
    let property = Symbol::mk("Shared\0Property");
    let metadata = Symbol::mk("Straße\0Key");
    let mut loader = db.loader_client().unwrap();
    loader
        .create_object(
            ObjectKind::Objid(root),
            &ObjAttrs::new(root, NOTHING, NOTHING, BitEnum::new(), "root"),
        )
        .unwrap();
    loader
        .define_property(
            &root,
            &root,
            property,
            &root,
            PropFlag::rw(),
            Some(v_str("inherited")),
        )
        .unwrap();
    for id in [1, 2, 3, 256] {
        let object = Obj::mk_id(id);
        // Use the final owner at creation so inherited permissions stay lazy.
        let owner = if id == 256 { object } else { root };
        loader
            .create_object(
                ObjectKind::Objid(object),
                &ObjAttrs::new(owner, root, NOTHING, BitEnum::new(), "child"),
            )
            .unwrap();
    }
    loader
        .set_property(
            &Obj::mk_id(2),
            property,
            Some(Obj::mk_id(2)),
            Some(BitEnum::new_with(PropFlag::Read)),
            None,
        )
        .unwrap();
    loader
        .set_property_metadata(&Obj::mk_id(3), property, metadata, v_str("local only"))
        .unwrap();
    let profile = moor_compiler::SourceProfile::default();
    let program = moor_compiler::read_persistent_source("return 40 + 2;", &profile).unwrap();
    loader
        .add_verb(
            &root,
            &[Symbol::mk("answer")],
            &root,
            BitEnum::new_with(VerbFlag::Exec),
            VerbArgsSpec::this_none_this(),
            program.clone(),
        )
        .unwrap();
    let verb = loader
        .get_existing_verbs(&root)
        .unwrap()
        .iter()
        .next()
        .unwrap()
        .uuid();
    loader
        .set_verb_metadata(&root, verb, metadata, v_str("program metadata"))
        .unwrap();
    let chown = Symbol::mk("chown");
    loader
        .define_property(
            &root,
            &root,
            chown,
            &root,
            BitEnum::new_with(PropFlag::Chown),
            Some(v_int(99)),
        )
        .unwrap();
    let chown_uuid = loader
        .get_existing_properties(&root)
        .unwrap()
        .iter()
        .find(|d| d.name() == chown)
        .unwrap()
        .uuid();
    loader.commit().unwrap();
    let mut tx = db.new_world_state().unwrap();
    tx.update_property(
        &TaskPermissions::new(root, BitEnum::new()),
        &Obj::mk_id(1),
        property,
        &v_int(17),
    )
    .unwrap();
    tx.commit().unwrap();
    let error = db
        .create_snapshot_with_timeout(std::time::Duration::ZERO)
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("resources are busy"), "{error}");
    let snapshot = db.create_snapshot().unwrap();
    let (value, permissions) = snapshot
        .get_property_value(&Obj::mk_id(256), chown_uuid)
        .unwrap();
    assert!(value.is_none());
    assert_eq!(permissions.owner(), Obj::mk_id(256));
    let mut expected_source = String::new();
    moor_compiler::write_persistent_source(&program, &profile, &mut expected_source).unwrap();
    let mut source = String::new();
    moor_compiler::write_persistent_source(
        &snapshot.get_verb_program(&root, verb).unwrap(),
        &profile,
        &mut source,
    )
    .unwrap();
    assert_eq!(source, expected_source);
    assert_eq!(
        snapshot.get_verb_metadata(&root, verb).unwrap(),
        vec![(metadata, v_str("program metadata"))]
    );
    let mut exported = BTreeMap::new();
    let mut export = snapshot.begin_export(&[]).unwrap();
    while let Some(object) = export.next_object().unwrap() {
        exported.insert(object.oid, object);
    }
    assert_eq!(exported.len(), 5);
    for id in [1, 2, 3, 256] {
        let id = Obj::mk_id(id);
        let point = snapshot.get_property_snapshots(&id).unwrap();
        let streamed = &exported[&id].properties;
        assert_eq!(point.len(), streamed.len());
        if id == Obj::mk_id(256) {
            assert!(point.is_empty());
            continue;
        }
        assert_eq!(point.len(), 1);
        for rows in [&point, streamed] {
            let row = &rows[0];
            assert_eq!(row.definition.definer(), root);
            assert_eq!(row.definition.name(), property);
            if id == Obj::mk_id(1) {
                assert_eq!(row.value, Some(v_int(17)));
                assert_eq!(row.permissions.as_ref().unwrap().owner(), root);
                assert!(row.metadata.is_empty());
            } else if id == Obj::mk_id(2) {
                assert!(row.value.is_none());
                assert_eq!(row.permissions.as_ref().unwrap().owner(), id);
                assert!(row.metadata.is_empty());
            } else {
                assert!(row.value.is_none());
                assert!(row.permissions.is_none());
                assert_eq!(row.metadata, vec![(metadata, v_str("local only"))]);
            }
        }
    }
    let root_export = &exported[&root];
    assert_eq!(root_export.verbs.len(), 1);
    assert_eq!(
        root_export.verbs[0].metadata,
        vec![(metadata, v_str("program metadata"))]
    );
    let mut streamed_source = String::new();
    moor_compiler::write_persistent_source(
        &root_export.verbs[0].program,
        &profile,
        &mut streamed_source,
    )
    .unwrap();
    assert_eq!(streamed_source, expected_source);
}

#[test]
fn fjall_snapshot_preserves_sparse_local_state_and_programs() {
    contract(StorageConfig::temporary_fjall());
}

#[cfg(feature = "postgres")]
#[test]
#[ignore = "requires PostgreSQL fixture"]
fn postgres_snapshot_preserves_sparse_local_state_and_programs() {
    use moor_db::{
        PostgresConnectOptions, PostgresEndpoint, PostgresSchema, PostgresStorageConfig,
        initialize_postgres_schema,
    };
    let config = PostgresStorageConfig::new(
        PostgresConnectOptions::new(
            std::env::var("MOOR_PG_TEST_CONNINFO").expect("run scripts/test-postgres-adapter.sh"),
            PostgresEndpoint::Tcp("127.0.0.1".parse().unwrap()),
        ),
        PostgresSchema::new(&format!("snapshot_{}", uuid::Uuid::new_v4().simple())).unwrap(),
    );
    initialize_postgres_schema(&config).unwrap();
    contract(StorageConfig::postgres(config.clone()));
    moor_db::validate_postgres_storage(&config).unwrap();
}
