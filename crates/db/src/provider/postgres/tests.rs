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

//! Live storage contracts; scripts/test-postgres-adapter.sh supplies the disposable server.
use super::{
    apply::{FailurePoint, Session},
    encode::{EncodedCommit, PropertyMutation, PropertyMutationRow, RelationBatch},
    rows::RowKey,
    *,
};
use crate::{
    ObjAndUUIDHolder, StringHolder, Timestamp,
    engine::moor_db::Relations,
    provider::logical::{PublicationId, WriterEpoch},
};
use moor_var::{Obj, Var, v_int, v_list};
use serde_json::json;
use std::time::Instant;
use uuid::Uuid;

pub(super) fn config() -> PostgresStorageConfig {
    PostgresStorageConfig::new(
        PostgresConnectOptions::new(
            std::env::var("MOOR_PG_TEST_CONNINFO").expect("run scripts/test-postgres-adapter.sh"),
            PostgresEndpoint::Tcp("127.0.0.1".parse().unwrap()),
        ),
        PostgresSchema::new(&format!("moor_{}", Uuid::new_v4().simple())).unwrap(),
    )
}
pub(super) fn open(
    config: &PostgresStorageConfig,
) -> (Session, crate::provider::backend::SeededWorld, WriterEpoch) {
    let epoch = WriterEpoch::random();
    let (session, seed) = Session::open(
        config.clone(),
        &Relations::init(),
        epoch,
        PostgresShutdown::default(),
    )
    .unwrap();
    (session, seed, epoch)
}
pub(super) fn empty(epoch: WriterEpoch, version: u64, timestamp: u64) -> EncodedCommit {
    EncodedCommit {
        publication: PublicationId::new(epoch, version),
        timestamp: Timestamp(timestamp),
        ordinary: vec![],
        properties: vec![],
        sequences: None,
    }
}
fn apply(session: &mut Session, config: &PostgresStorageConfig, batch: &EncodedCommit) {
    session
        .apply(batch, |key, value, ts| {
            encode::property_row(key, &value, ts, false, &config.profile)
        })
        .unwrap();
}
fn client(config: &PostgresStorageConfig) -> PostgresConnection {
    PostgresConnection::connect(
        &config.connection,
        Instant::now() + config.connect_timeout,
        PostgresShutdown::default(),
    )
    .unwrap()
}
fn full(
    config: &PostgresStorageConfig,
    key: &ObjAndUUIDHolder,
    value: &Var,
    ts: u64,
) -> PropertyMutationRow {
    PropertyMutationRow {
        key: key.clone(),
        mutation: PropertyMutation::Full(
            encode::property_row(key, value, Timestamp(ts), false, &config.profile).unwrap(),
        ),
    }
}
fn append(
    config: &PostgresStorageConfig,
    key: &ObjAndUUIDHolder,
    suffix: &Var,
    final_value: &Var,
    ts: u64,
) -> PropertyMutationRow {
    PropertyMutationRow {
        key: key.clone(),
        mutation: PropertyMutation::Append {
            row: encode::property_row(key, suffix, Timestamp(ts), true, &config.profile).unwrap(),
            final_value: final_value.clone(),
        },
    }
}
fn count(config: &PostgresStorageConfig, relation: &str) -> usize {
    let mut value = 0;
    client(config)
        .query(
            &format!(
                "SELECT count(*) FROM {}",
                config.schema.qualify(relation).unwrap()
            ),
            &[],
            Instant::now() + config.query_timeout,
            |row| {
                value = std::str::from_utf8(row.columns[0].as_deref().unwrap())
                    .unwrap()
                    .parse()
                    .unwrap();
                Ok(())
            },
        )
        .unwrap();
    value
}

#[test]
#[ignore = "requires PostgreSQL fixture"]
fn claims_are_exclusive_and_open_never_initializes_or_repairs() {
    let config = config();
    assert!(
        Session::open(
            config.clone(),
            &Relations::init(),
            WriterEpoch::random(),
            PostgresShutdown::default()
        )
        .is_err()
    );
    initialize_postgres_schema(&config).unwrap();
    let (session, seed, _) = open(&config);
    assert_eq!(seed.root.committed_ts, Timestamp(0));
    assert!(seed.root.object_name.is_fully_resident());
    assert!(matches!(
        Session::open(
            config.clone(),
            &Relations::init(),
            WriterEpoch::random(),
            PostgresShutdown::default()
        ),
        Err(PostgresError::OwnershipLost)
    ));
    drop(session);
    let table = config.schema.qualify("world_metadata").unwrap();
    client(&config)
        .query(
            &format!(
                "UPDATE {table} SET profile=jsonb_set(profile,'{{compiler_profile_version}}','9')"
            ),
            &[],
            Instant::now() + config.query_timeout,
            |_| unreachable!(),
        )
        .unwrap();
    assert!(matches!(
        Session::open(
            config.clone(),
            &Relations::init(),
            WriterEpoch::random(),
            PostgresShutdown::default()
        ),
        Err(PostgresError::Format { .. })
    ));
}

#[test]
#[ignore = "requires PostgreSQL fixture"]
fn ordered_values_sequences_and_deleted_counters_survive_reopen() {
    let config = config();
    initialize_postgres_schema(&config).unwrap();
    let (mut session, _, epoch) = open(&config);
    let object = Obj::mk_id(7);
    let property = ObjAndUUIDHolder::new(&object, Uuid::new_v4());
    let mut first = empty(epoch, 1, 100);
    first.ordinary.push(RelationBatch {
        relation: "object_name",
        puts: Some(
            json!([rows::encode(
                "object_name",
                Timestamp(100),
                &object,
                &StringHolder("Before\0Name".into()),
                &config.profile
            )
            .unwrap()])
            .to_string(),
        ),
        deletes: None,
    });
    first
        .properties
        .push(full(&config, &property, &v_list(&[v_int(1)]), 100));
    first.sequences = Some(json!([{"slot":0,"high_water":600}]).to_string());
    apply(&mut session, &config, &first);
    let mut second = empty(epoch, 2, 40);
    second.ordinary.push(RelationBatch {
        relation: "object_name",
        puts: Some(
            json!([rows::encode(
                "object_name",
                Timestamp(40),
                &object,
                &StringHolder("After\0Name".into()),
                &config.profile
            )
            .unwrap()])
            .to_string(),
        ),
        deletes: None,
    });
    second.properties.push(append(
        &config,
        &property,
        &v_list(&[v_int(2)]),
        &v_list(&[v_int(1), v_int(2)]),
        40,
    ));
    second.sequences = Some(json!([{"slot":0,"high_water":20}]).to_string());
    apply(&mut session, &config, &second);
    drop(session);
    let (mut session, seed, epoch) = open(&config);
    assert_eq!(
        seed.root.object_name.index_lookup(&object).unwrap().value.0,
        "After\0Name"
    );
    assert_eq!(
        seed.root
            .object_propvalues
            .index_lookup(&property)
            .unwrap()
            .value,
        v_list(&[v_int(1), v_int(2)])
    );
    assert_eq!(
        seed.root
            .object_propvalues
            .index_lookup(&property)
            .unwrap()
            .ts,
        Timestamp(40)
    );
    assert_eq!(seed.root.committed_ts, Timestamp(100));
    assert_eq!(seed.sequences[0], 600);
    assert_eq!(session.progress.property_sequence, 2);
    let mut deleted = empty(epoch, 1, 200);
    deleted.ordinary.push(RelationBatch {
        relation: "object_name",
        puts: None,
        deletes: Some(json!([object.encode_key("object_name")]).to_string()),
    });
    deleted.properties.push(PropertyMutationRow {
        key: property.clone(),
        mutation: PropertyMutation::Delete,
    });
    apply(&mut session, &config, &deleted);
    drop(session);
    let (session, seed, _) = open(&config);
    assert_eq!(session.progress.property_sequence, 3);
    assert_eq!(session.progress.commits, 3);
    assert_eq!(seed.root.committed_ts, Timestamp(200));
    assert!(
        seed.root
            .object_propvalues
            .index_lookup(&property)
            .is_none()
    );
    assert!(seed.root.object_name.index_lookup(&object).is_none());
    assert_eq!(seed.sequences[0], 600);
}

#[test]
#[ignore = "requires PostgreSQL fixture"]
fn ambiguous_commit_and_rollback_do_not_duplicate_suffixes() {
    for failure in [FailurePoint::BeforeCommit, FailurePoint::AfterCommit] {
        let config = config();
        initialize_postgres_schema(&config).unwrap();
        let (mut session, _, epoch) = open(&config);
        let property = ObjAndUUIDHolder::new(&Obj::mk_id(7), Uuid::new_v4());
        let mut first = empty(epoch, 1, 1);
        first
            .properties
            .push(full(&config, &property, &v_list(&[v_int(1)]), 1));
        apply(&mut session, &config, &first);
        let mut second = empty(epoch, 2, 2);
        second.properties.push(append(
            &config,
            &property,
            &v_list(&[v_int(2)]),
            &v_list(&[v_int(1), v_int(2)]),
            2,
        ));
        session.failure = Some(failure);
        apply(&mut session, &config, &second);
        assert_eq!(count(&config, "object_propvalues"), 2);
        assert_eq!(session.progress.commits, 2);
        drop(session);
        let (_, seed, _) = open(&config);
        assert_eq!(
            seed.root
                .object_propvalues
                .index_lookup(&property)
                .unwrap()
                .value,
            v_list(&[v_int(1), v_int(2)])
        );
    }
}

#[test]
#[ignore = "requires PostgreSQL fixture"]
fn bounded_chain_rollup_and_async_durable_fence() {
    let mut config = config();
    config.commit_policy = PostgresCommitPolicy::Asynchronous;
    initialize_postgres_schema(&config).unwrap();
    let (mut session, _, epoch) = open(&config);
    let property = ObjAndUUIDHolder::new(&Obj::mk_id(7), Uuid::new_v4());
    let mut values = vec![v_int(0)];
    let mut first = empty(epoch, 1, 1);
    first
        .properties
        .push(full(&config, &property, &v_list(&values), 1));
    apply(&mut session, &config, &first);
    let mut rollups = 0;
    for version in 2..=66 {
        values.push(v_int(version as i64));
        let mut batch = empty(epoch, version, version);
        batch.properties.push(append(
            &config,
            &property,
            &v_list(&[v_int(version as i64)]),
            &v_list(&values),
            version,
        ));
        session
            .apply(&batch, |key, value, ts| {
                rollups += 1;
                encode::property_row(key, &value, ts, false, &config.profile)
            })
            .unwrap();
    }
    assert_eq!(rollups, 1);
    assert_eq!(count(&config, "object_propvalues"), 2);
    session.failure = Some(FailurePoint::AfterCommit);
    session.fence().unwrap();
    assert_eq!(session.progress.durable_fence, 1);
    assert_eq!(session.progress.commits, 66);
    assert_eq!(session.progress.property_sequence, 66);
    drop(session);
    let (_, seed, _) = open(&config);
    assert_eq!(
        seed.root
            .object_propvalues
            .index_lookup(&property)
            .unwrap()
            .value,
        v_list(&values)
    );
}

#[test]
#[ignore = "requires PostgreSQL fixture"]
fn engine_publication_encoding_source_compilation_and_reopen() {
    use crate::{Database, DatabaseConfig, PersistenceConfig, StorageConfig, TxDB};
    use moor_common::{
        model::{ObjAttrs, ObjFlag, ObjectKind, VerbArgsSpec, VerbFlag},
        util::BitEnum,
    };
    use moor_compiler::{read_persistent_source, write_persistent_source};
    use moor_var::{NOTHING, Symbol, v_str};
    use std::time::Duration;
    for policy in [
        PostgresCommitPolicy::Synchronous,
        PostgresCommitPolicy::Asynchronous,
    ] {
        let mut config = config();
        config.commit_policy = policy;
        initialize_postgres_schema(&config).unwrap();
        let (db, fresh) = TxDB::try_open(
            StorageConfig::postgres(config.clone()),
            DatabaseConfig::default(),
            PersistenceConfig::default(),
        )
        .unwrap();
        assert!(fresh);
        let mut loader = db.loader_client().unwrap();
        let object = loader
            .create_object(
                ObjectKind::NextObjid,
                &ObjAttrs::new(
                    NOTHING,
                    NOTHING,
                    NOTHING,
                    BitEnum::new_with(ObjFlag::Wizard),
                    "Readable\0world",
                ),
            )
            .unwrap();
        let property = Symbol::mk("Value");
        let expected = v_list(&[v_int(1), v_str("a\0b")]);
        loader
            .define_property(
                &object,
                &object,
                property,
                &object,
                BitEnum::from_u16(0x8001),
                Some(expected.clone()),
            )
            .unwrap();
        let source = "return {42, \"hello\"};";
        let program = read_persistent_source(source, &config.profile).unwrap();
        loader
            .add_verb(
                &object,
                &[Symbol::mk("Run")],
                &object,
                BitEnum::new_with(VerbFlag::Exec),
                VerbArgsSpec::this_none_this(),
                program,
            )
            .unwrap();
        loader
            .set_object_metadata(&object, Symbol::mk("Straße\0Key"), v_str("metadata"))
            .unwrap();
        loader.commit().unwrap();
        let publication = db.publication();
        db.wait_applied(publication, Duration::from_secs(10))
            .unwrap();
        db.wait_durable(publication, Duration::from_secs(10))
            .unwrap();
        assert!(db.persistence_status().durable >= publication.version());
        assert_eq!(db.persistence_status().outstanding, 0);
        assert!(db.storage_maintenance_stats().is_none());
        drop(db);
        let (db, fresh) = TxDB::try_open(
            StorageConfig::postgres(config.clone()),
            DatabaseConfig::default(),
            PersistenceConfig::default(),
        )
        .unwrap();
        assert!(!fresh);
        let mut loader = db.loader_client().unwrap();
        assert_eq!(
            loader
                .get_existing_object(&object)
                .unwrap()
                .unwrap()
                .name()
                .as_deref(),
            Some("Readable\0world")
        );
        assert_eq!(
            loader
                .get_existing_property_value(&object, property)
                .unwrap()
                .map(|(value, _)| value),
            Some(expected)
        );
        let (uuid, _) = loader
            .get_existing_verb_by_names(&object, &[Symbol::mk("Run")])
            .unwrap()
            .unwrap();
        let loaded = loader.get_verb_program(&object, uuid).unwrap();
        let mut text = String::new();
        write_persistent_source(&loaded, &config.profile, &mut text).unwrap();
        assert!(text.contains("42"));
        assert!(text.contains("hello"));
        // Allocation resumes above the stored sequence high-water mark.
        let next = loader
            .create_object(
                ObjectKind::NextObjid,
                &ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "next"),
            )
            .unwrap();
        assert_ne!(next, object);
        loader.commit().unwrap();
        db.wait_for_persistence().unwrap();
    }
}

macro_rules! empty_changes {
    ($( $field:ident $category:ident $policy:ident $arrow:tt $domain:ty, $codomain:ty ),* $(,)?) => {
        fn changes() -> crate::engine::moor_db::RelationChanges {
            crate::engine::moor_db::RelationChanges { $($field: Default::default(),)* }
        }
    };
}
crate::relation_registry::relation_registry!(empty_changes);

#[test]
#[ignore = "requires PostgreSQL fixture"]
fn out_of_order_encoders_hold_permits_and_rollups_have_an_independent_reply_path() {
    use crate::{
        PersistenceConfig,
        provider::{
            coordinator::PersistenceCoordinator,
            logical::{LogicalCommit, PreparedPropertyValueMutation, PreparedPropertyValueOp},
            writer::StorageWriter,
        },
    };
    use std::{sync::Arc, time::Duration};
    let config = config();
    initialize_postgres_schema(&config).unwrap();
    let epoch = WriterEpoch::random();
    let (writer, _, _, _) =
        PostgresWriter::open(config.clone(), Arc::new(Relations::init()), epoch).unwrap();
    let coordinator = PersistenceCoordinator::new(
        epoch,
        PersistenceConfig {
            shutdown_timeout: Duration::from_secs(3),
            ..Default::default()
        },
        StorageWriter::Postgres(writer),
    );
    let key = ObjAndUUIDHolder::new(&Obj::mk_id(9), Uuid::new_v4());
    let total = 200;
    let through = coordinator.published(total);
    let submit = |version| {
        let permit = coordinator.admit(Timestamp(version)).unwrap();
        let mut changes = changes();
        let value = v_list(&(1..=version).map(|n| v_int(n as i64)).collect::<Vec<_>>());
        let mutation = if version == 1 {
            PreparedPropertyValueMutation::Replace { value }
        } else {
            PreparedPropertyValueMutation::AppendList {
                suffix: v_list(&[v_int(version as i64)]).as_list().unwrap().clone(),
                final_value: value,
            }
        };
        changes.object_propvalues.push(PreparedPropertyValueOp {
            property: key.clone(),
            mutation,
        });
        coordinator
            .submit(
                LogicalCommit {
                    publication: PublicationId::new(epoch, version),
                    timestamp: Timestamp(version),
                    changes,
                    sequences: vec![],
                    property_definition_changes: vec![],
                },
                permit,
            )
            .unwrap();
    };
    for version in (2..=total).rev() {
        submit(version);
    }
    assert!(
        coordinator
            .wait_applied(through, Duration::from_millis(30))
            .is_err()
    );
    assert_eq!(coordinator.status().applied, 0);
    assert_eq!(coordinator.status().outstanding, (total - 1) as usize);
    submit(1);
    coordinator
        .wait_applied(through, Duration::from_secs(10))
        .unwrap();
    assert_eq!(coordinator.status().outstanding, 0);
    assert_eq!(count(&config, "object_propvalues"), 8);
    coordinator.shutdown().unwrap();
    let (_, seed, _) = open(&config);
    assert_eq!(
        seed.root
            .object_propvalues
            .index_lookup(&key)
            .unwrap()
            .value,
        v_list(&(1..=total).map(|n| v_int(n as i64)).collect::<Vec<_>>())
    );
}

#[test]
#[ignore = "requires PostgreSQL fixture"]
fn every_value_kind_and_large_full_values_survive_sql_and_restart() {
    use moor_compiler::{read_persistent_literal, write_persistent_literal};
    use moor_var::{
        Error, ErrorCode, List, NOTHING, Symbol, v_binary, v_bool, v_error, v_float, v_flyweight,
        v_map, v_none, v_obj, v_str, v_symbol_str,
    };
    let config = config();
    initialize_postgres_schema(&config).unwrap();
    let (mut session, _, epoch) = open(&config);
    let lambda =
        read_persistent_literal("{} => x with captured [{x: 42}]", &config.profile).unwrap();
    let values = vec![
        v_none(),
        v_bool(true),
        v_int(i64::MIN),
        v_int(i64::MAX),
        v_float(-0.0),
        v_float(f64::from_bits(1)),
        v_float(f64::from_bits(0x7ff8000000001234)),
        v_obj(NOTHING),
        v_obj(Obj::mk_uuobjid_generated()),
        read_persistent_literal("#anon_048D05-1234567890", &config.profile).unwrap(),
        v_symbol_str(Symbol::mk("MiXeD\0Case")),
        v_str("UTF8 α🐄\0"),
        v_list(&[v_bool(false), v_int(0), v_none()]),
        v_map(&[(v_int(1), lambda.clone()), (v_str("MiXeD"), v_none())]),
        v_error(Error::new(
            ErrorCode::ErrCustom(Symbol::mk("MiXeD\0Error")),
            Some("message\0".into()),
            Some(v_none()),
        )),
        v_flyweight(
            Obj::mk_id(1),
            &[(Symbol::mk("MiXeD\0Slot"), v_none())],
            List::mk_list(&[v_str("contents")]),
        ),
        v_binary((0..=255).collect()),
        lambda,
        // The append-byte limit must not reject a larger full value.
        v_str(&"x".repeat(4 * 1024 * 1024 + 32)),
    ];
    let keys: Vec<_> = values
        .iter()
        .enumerate()
        .map(|(i, _)| ObjAndUUIDHolder::new(&Obj::mk_id(1), Uuid::from_u128(i as u128 + 1)))
        .collect();
    let mut commit = empty(epoch, 1, 1);
    for (key, value) in keys.iter().zip(&values) {
        commit.properties.push(full(&config, key, value, 1));
    }
    apply(&mut session, &config, &commit);
    drop(session);
    let (_, seed, _) = open(&config);
    for (key, value) in keys.iter().zip(&values) {
        let loaded = &seed.root.object_propvalues.index_lookup(key).unwrap().value;
        let mut expected = String::new();
        let mut actual = String::new();
        write_persistent_literal(value, &config.profile, &mut expected).unwrap();
        write_persistent_literal(loaded, &config.profile, &mut actual).unwrap();
        assert_eq!(actual, expected);
        if let (Some(expected), Some(actual)) = (value.as_float(), loaded.as_float()) {
            assert_eq!(expected.to_bits(), actual.to_bits());
        }
    }
}

#[test]
#[ignore = "requires PostgreSQL fixture"]
fn invalid_stored_source_reports_the_verb_and_source_position() {
    use moor_compiler::read_persistent_source;
    let config = config();
    initialize_postgres_schema(&config).unwrap();
    let (mut session, _, epoch) = open(&config);
    let key = ObjAndUUIDHolder::new(&Obj::mk_id(7), Uuid::new_v4());
    let program = read_persistent_source("return 1;", &config.profile).unwrap();
    let mut commit = empty(epoch, 1, 1);
    commit.ordinary.push(RelationBatch {
        relation: "object_verbs",
        puts: Some(
            json!([rows::encode(
                "object_verbs",
                Timestamp(1),
                &key,
                &program,
                &config.profile
            )
            .unwrap()])
            .to_string(),
        ),
        deletes: None,
    });
    apply(&mut session, &config, &commit);
    drop(session);
    let table = config.schema.qualify("object_verbs").unwrap();
    client(&config)
        .query(
            &format!("UPDATE {table} SET source=$1"),
            &[PostgresParam::Text(25, "\nreturn +;\n")],
            Instant::now() + config.query_timeout,
            |_| unreachable!(),
        )
        .unwrap();
    let result = Session::open(
        config.clone(),
        &Relations::init(),
        WriterEpoch::random(),
        PostgresShutdown::default(),
    );
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("invalid source was accepted"),
    };
    let message = error.to_string();
    assert!(message.contains("object_verbs"), "{message}");
    assert!(message.contains(&format!("#7/{}", key.uuid())), "{message}");
    assert!(message.contains("@ 2/"), "{message}");
}

#[test]
#[ignore = "requires PostgreSQL fixture"]
fn inherited_properties_remain_sparse_and_metadata_keeps_its_holder() {
    use crate::{Database, DatabaseConfig, PersistenceConfig, StorageConfig, TxDB};
    use moor_common::{
        model::{HasUuid, ObjAttrs, ObjectKind, ValSet},
        util::BitEnum,
    };
    use moor_var::{NOTHING, Symbol, v_str};
    let config = config();
    initialize_postgres_schema(&config).unwrap();
    let (db, _) = TxDB::try_open(
        StorageConfig::postgres(config.clone()),
        DatabaseConfig::default(),
        PersistenceConfig::default(),
    )
    .unwrap();
    let mut loader = db.loader_client().unwrap();
    let parent = loader
        .create_object(
            ObjectKind::NextObjid,
            &ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "parent"),
        )
        .unwrap();
    let property = Symbol::mk("Mixed\0Property");
    loader
        .define_property(
            &parent,
            &parent,
            property,
            &parent,
            BitEnum::new(),
            Some(v_str("inherited")),
        )
        .unwrap();
    let child = loader
        .create_object(
            ObjectKind::NextObjid,
            &ObjAttrs::new(parent, parent, NOTHING, BitEnum::new(), "child"),
        )
        .unwrap();
    let other = loader
        .create_object(
            ObjectKind::NextObjid,
            &ObjAttrs::new(parent, parent, NOTHING, BitEnum::new(), "other"),
        )
        .unwrap();
    let metadata = Symbol::mk("Straße\0Metadata");
    loader
        .set_property_metadata(&child, property, metadata, v_str("child metadata"))
        .unwrap();
    loader
        .set_property_metadata(&other, property, metadata, v_str("other metadata"))
        .unwrap();
    loader.commit().unwrap();
    db.wait_for_persistence().unwrap();
    assert_eq!(count(&config, "object_propvalues"), 1);
    assert_eq!(count(&config, "object_propflags"), 1);
    drop(db);
    let (db, _) = TxDB::try_open(
        StorageConfig::postgres(config.clone()),
        DatabaseConfig::default(),
        PersistenceConfig::default(),
    )
    .unwrap();
    let loader = db.loader_client().unwrap();
    let definitions = loader.get_existing_properties(&parent).unwrap();
    let definition = definitions.iter().next().unwrap();
    assert_eq!(definition.name().as_str(), property.as_str());
    let tx = db.storage.start_transaction();
    assert_eq!(
        tx.property_metadata(&child, definition.uuid()).unwrap(),
        vec![(metadata, v_str("child metadata"))]
    );
    assert_eq!(
        tx.property_metadata(&other, definition.uuid()).unwrap(),
        vec![(metadata, v_str("other metadata"))]
    );
    // A metadata-only local row must not materialize a property value or permission row.
    assert_eq!(count(&config, "object_propvalues"), 1);
    assert_eq!(count(&config, "object_propflags"), 1);
}

#[test]
#[ignore = "requires PostgreSQL fixture"]
fn connection_loss_during_a_statement_rolls_back_the_whole_commit_before_replay() {
    let config = config();
    initialize_postgres_schema(&config).unwrap();
    let (mut session, _, epoch) = open(&config);
    let key = ObjAndUUIDHolder::new(&Obj::mk_id(3), Uuid::new_v4());
    let mut first = empty(epoch, 1, 1);
    first
        .properties
        .push(full(&config, &key, &v_list(&[v_int(1)]), 1));
    apply(&mut session, &config, &first);
    let sequence = config.schema.qualify("disconnect_sequence").unwrap();
    let function = config.schema.qualify("disconnect_once").unwrap();
    let properties = config.schema.qualify("object_propvalues").unwrap();
    for statement in [
        format!("CREATE SEQUENCE {sequence}"),
        format!(
            "CREATE FUNCTION {function}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF nextval('{sequence}')=1 THEN PERFORM pg_terminate_backend(pg_backend_pid()); END IF; RETURN NEW; END $$"
        ),
        format!(
            "CREATE TRIGGER disconnect_once BEFORE INSERT ON {properties} FOR EACH ROW EXECUTE FUNCTION {function}()"
        ),
    ] {
        client(&config)
            .query(
                &statement,
                &[],
                Instant::now() + config.query_timeout,
                |_| unreachable!(),
            )
            .unwrap();
    }
    let mut second = empty(epoch, 2, 2);
    second.ordinary.push(RelationBatch {
        relation: "object_name",
        puts: Some(
            json!([rows::encode(
                "object_name",
                Timestamp(2),
                &Obj::mk_id(3),
                &StringHolder("atomic".into()),
                &config.profile
            )
            .unwrap()])
            .to_string(),
        ),
        deletes: None,
    });
    second.properties.push(append(
        &config,
        &key,
        &v_list(&[v_int(2)]),
        &v_list(&[v_int(1), v_int(2)]),
        2,
    ));
    apply(&mut session, &config, &second);
    assert_eq!(session.progress.commits, 2);
    assert_eq!(session.progress.property_sequence, 2);
    assert_eq!(count(&config, "object_propvalues"), 2);
    drop(session);
    let (_, seed, _) = open(&config);
    assert_eq!(
        seed.root
            .object_name
            .index_lookup(&Obj::mk_id(3))
            .unwrap()
            .value
            .0,
        "atomic"
    );
    assert_eq!(
        seed.root
            .object_propvalues
            .index_lookup(&key)
            .unwrap()
            .value,
        v_list(&[v_int(1), v_int(2)])
    );
}

#[test]
#[ignore = "requires PostgreSQL fixture"]
fn shutdown_timeout_fails_waiters_and_is_not_reported_as_success_on_retry() {
    use crate::{
        PersistenceConfig,
        provider::{
            coordinator::PersistenceCoordinator, logical::LogicalCommit, writer::StorageWriter,
        },
    };
    use std::{sync::Arc, time::Duration};
    let config = config();
    initialize_postgres_schema(&config).unwrap();
    let epoch = WriterEpoch::random();
    let (writer, _, _, _) =
        PostgresWriter::open(config.clone(), Arc::new(Relations::init()), epoch).unwrap();
    let coordinator = Arc::new(PersistenceCoordinator::new(
        epoch,
        PersistenceConfig {
            shutdown_timeout: Duration::from_millis(100),
            ..Default::default()
        },
        StorageWriter::Postgres(writer),
    ));
    let permit = coordinator.admit(Timestamp(2)).unwrap();
    let publication = coordinator.published(2);
    // Publication one represents an encoder that has not delivered its result.
    coordinator
        .submit(
            LogicalCommit {
                publication,
                timestamp: Timestamp(2),
                changes: changes(),
                sequences: vec![],
                property_definition_changes: vec![],
            },
            permit,
        )
        .unwrap();
    let waiter = coordinator.clone();
    let waiting = std::thread::spawn(move || waiter.wait_applied_unbounded(publication));
    let started = Instant::now();
    assert!(coordinator.shutdown().is_err());
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(waiting.join().unwrap().is_err());
    assert!(coordinator.shutdown().is_err());
    assert_eq!(coordinator.status().applied, 0);
    assert!(!coordinator.status().healthy);
}
