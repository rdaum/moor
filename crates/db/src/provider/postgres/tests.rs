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
        payload_lease: None,
        byte_lease: None,
        sizes: Default::default(),
        publication: PublicationId::new(epoch, version),
        timestamp: Timestamp(timestamp),
        ordinary: vec![],
        properties: vec![],
        sequences: None,
        group_bytes: 0,
        group_operations: 0,
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
            retained_bytes: moor_var::ByteSized::size_bytes(final_value),
            retained_serialized_bytes: 0,
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
        let diagnostics = session.metrics.snapshot();
        assert!(diagnostics.recovery_attempts > 0);
        assert!(diagnostics.recovery_ns > 0);
        assert_eq!(
            (diagnostics.recovery_first, diagnostics.recovery_last),
            (2, 2)
        );
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
fn measured_group_sizes_pack_rendered_payloads_and_cover_sql_fields() {
    use super::{plan::TransactionPlan, seed::Chains, state::Progress};
    use crate::provider::logical::{
        LogicalCommit, PreparedPropertyValueMutation, PreparedPropertyValueOp,
    };
    use moor_compiler::SourceProfile;
    let epoch = WriterEpoch::random();
    let profile = SourceProfile::default();
    let cache = validation_cache::ValidationCache::default();
    let key = ObjAndUUIDHolder::new(&Obj::mk_uuobjid_generated(), Uuid::new_v4());
    let value = v_list(&[moor_var::v_str(
        &"ordinary text with \"escaping\\\n牛".repeat(5000),
    )]);
    let mut chains = Chains::default();
    chains.insert(
        key.clone(),
        crate::provider::property_value_store::PropertyValueChain::full(i64::MAX as u64 - 10),
    );
    let before = Progress {
        epoch: epoch.as_u64(),
        applied: 0,
        commits: 0,
        max_timestamp: 0,
        property_sequence: i64::MAX - 10,
        durable_fence: 0,
    };
    for mutation in [
        PreparedPropertyValueMutation::Replace {
            value: value.clone(),
        },
        PreparedPropertyValueMutation::AppendList {
            suffix: value.as_list().unwrap().clone(),
            final_value: value.clone(),
        },
        PreparedPropertyValueMutation::Delete,
    ] {
        let commits = (1..=4)
            .map(|version| {
                let mut changes = changes();
                changes.object_propvalues.push(PreparedPropertyValueOp {
                    property: key.clone(),
                    base_timestamp: None,
                    mutation: mutation.clone(),
                });
                encode::prepare(
                    LogicalCommit {
                        publication: PublicationId::new(epoch, version),
                        timestamp: Timestamp(u64::MAX),
                        changes,
                        sequences: vec![],
                        property_definition_changes: vec![],
                    },
                    &profile,
                    usize::MAX,
                    &cache,
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let estimated = commits.iter().map(|c| c.group_bytes).sum::<usize>();
        assert!(
            estimated < 1024 * 1024,
            "four measured payloads should fit together"
        );
        let plan = TransactionPlan::group(
            &before,
            &chains,
            &commits,
            estimated,
            |_, _, _| unreachable!(),
        )
        .unwrap();
        assert_eq!(plan.commits.len(), 4);
        assert!(
            plan.bytes <= estimated,
            "SQL keys and maximum-width sequences must fit"
        );
    }
}

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
    // Hold a later group member inside SQL, then inspect the last confirmed prefix.
    let mut blocker = client(&config);
    let mut lock_key = String::new();
    blocker
        .query(
            "SELECT pg_backend_pid()::text",
            &[],
            Instant::now() + config.query_timeout,
            |row| {
                lock_key = String::from_utf8(row.columns[0].clone().unwrap()).unwrap();
                Ok(())
            },
        )
        .unwrap();
    let function = config.schema.qualify("hold_group").unwrap();
    let properties = config.schema.qualify("object_propvalues").unwrap();
    for sql in [
        format!("SELECT pg_advisory_lock(728694, {lock_key})"),
        format!(
            "CREATE FUNCTION {function}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.record_sequence=100 THEN PERFORM pg_advisory_lock(728694, {lock_key}); PERFORM pg_advisory_unlock(728694, {lock_key}); END IF; RETURN NEW; END $$"
        ),
        format!(
            "CREATE TRIGGER hold_group BEFORE INSERT ON {properties} FOR EACH ROW EXECUTE FUNCTION {function}()"
        ),
    ] {
        blocker
            .query(&sql, &[], Instant::now() + config.query_timeout, |_| Ok(()))
            .unwrap();
    }
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
    // Full rollups exceed the group byte budget, so the writer must retain each unapplied tail.
    let prefix = moor_var::v_str(&"x".repeat(1024 * 1024));
    let total = 200;
    let through = coordinator.published(total);
    let submit = |version| {
        let permit = coordinator.admit(Timestamp(version)).unwrap();
        let mut changes = changes();
        let value = v_list(
            &std::iter::once(prefix.clone())
                .chain((1..=version).map(|n| v_int(n as i64)))
                .collect::<Vec<_>>(),
        );
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
            base_timestamp: None,
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
    // This test validates 200 one-MiB values in a debug build.
    // Allow constrained CI runners to reach the lock; ordering is asserted below.
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let mut blocked = false;
        blocker.query(
            &format!("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE locktype='advisory' AND classid=728694 AND objid={lock_key} AND objsubid=2 AND NOT granted)::text"),
            &[], deadline, |row| {
                blocked = row.columns[0].as_deref() == Some(b"true");
                Ok(())
            },
        ).unwrap();
        if blocked {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "writer never reached the held group member"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    let status = coordinator.status();
    assert!(status.applied > 0 && status.applied < 100);
    assert_eq!(status.outstanding, (total - status.applied) as usize);
    let stored = state::read_progress(&mut blocker, &config, deadline).unwrap();
    assert_eq!(stored.applied, status.applied);
    assert_eq!(stored.commits, status.applied);
    assert_eq!(stored.property_sequence as u64, status.applied);
    blocker
        .query(
            &format!("SELECT pg_advisory_unlock(728694, {lock_key})"),
            &[],
            deadline,
            |_| Ok(()),
        )
        .unwrap();
    coordinator
        .wait_applied(through, Duration::from_secs(60))
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
        v_list(
            &std::iter::once(prefix)
                .chain((1..=total).map(|n| v_int(n as i64)))
                .collect::<Vec<_>>()
        )
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
        let inspected = inspection_value(&config, key).unwrap();
        let inspected = read_persistent_literal(&inspected[0], &config.profile).unwrap();
        let mut inspected_text = String::new();
        write_persistent_literal(&inspected, &config.profile, &mut inspected_text).unwrap();
        assert_eq!(inspected_text, expected);
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
    let pool = super::snapshot::ExportPool::new(1);
    let read_session = super::snapshot::ReadSession::open(
        config.clone(),
        &pool,
        PublicationId::new(epoch, 1),
        Instant::now() + config.query_timeout,
        || true,
    )
    .unwrap();
    let snapshot = crate::provider::snapshot_loader::SnapshotLoader {
        readers: super::reader::readers(read_session),
    };
    use moor_common::model::loader::SnapshotInterface;
    let error = snapshot
        .get_verb_program(&Obj::mk_id(7), key.uuid())
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("object_verbs")
            && error.contains(&key.uuid().to_string())
            && error.contains("line 2, column"),
        "{error}"
    );
    drop(snapshot);
    assert_eq!(pool.diagnostics().0, 0);
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
    assert!(message.contains("line 2, column"), "{message}");
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
    let snapshot = db.create_snapshot().unwrap();
    for (holder, expected) in [(child, "child metadata"), (other, "other metadata")] {
        let properties = snapshot.get_property_snapshots(&holder).unwrap();
        assert_eq!(properties.len(), 1);
        assert!(properties[0].value.is_none() && properties[0].permissions.is_none());
        assert_eq!(properties[0].metadata, vec![(metadata, v_str(expected))]);
        assert_eq!(
            snapshot
                .get_property_metadata(&holder, definition.uuid())
                .unwrap(),
            vec![(metadata, v_str(expected))]
        );
    }
    let mut export = snapshot.begin_export(&[]).unwrap();
    while let Some(object) = export.next_object().unwrap() {
        if object.oid == child || object.oid == other {
            assert_eq!(object.properties.len(), 1);
            assert!(
                object.properties[0].value.is_none() && object.properties[0].permissions.is_none()
            );
            assert_eq!(object.properties[0].metadata.len(), 1);
        }
    }
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

#[test]
#[ignore = "requires PostgreSQL fixture"]
fn grouped_commits_recover_with_ordered_chains_and_logical_counters() {
    for policy in [
        PostgresCommitPolicy::Synchronous,
        PostgresCommitPolicy::Asynchronous,
    ] {
        for failure in [
            None,
            Some(FailurePoint::BeforeCommit),
            Some(FailurePoint::AfterCommit),
        ] {
            let mut config = config();
            config.commit_policy = policy;
            initialize_postgres_schema(&config).unwrap();
            let (mut session, _, epoch) = open(&config);
            let object = Obj::mk_id(7);
            let key = ObjAndUUIDHolder::new(&object, Uuid::new_v4());
            let mut commits = Vec::new();
            let mut values = Vec::new();
            for version in 1..=70 {
                let ts = 10000 - version;
                let mut commit = empty(epoch, version, ts);
                commit.sequences = Some(json!([{"slot":0, "high_water":1000-version}]).to_string());
                commit.ordinary.push(RelationBatch {
                    relation: "object_name",
                    puts: (version != 66).then(|| {
                        json!([rows::encode(
                            "object_name",
                            Timestamp(ts),
                            &object,
                            &StringHolder(format!("name {version}")),
                            &config.profile,
                        )
                        .unwrap()])
                        .to_string()
                    }),
                    deletes: (version == 66)
                        .then(|| json!([object.encode_key("object_name")]).to_string()),
                });
                let property = match version {
                    1 | 69 => {
                        values = vec![v_int(version as i64)];
                        full(&config, &key, &v_list(&values), ts)
                    }
                    66 => {
                        values.clear();
                        PropertyMutationRow {
                            key: key.clone(),
                            mutation: PropertyMutation::Delete,
                        }
                    }
                    _ => {
                        values.push(v_int(version as i64));
                        append(
                            &config,
                            &key,
                            &v_list(&[v_int(version as i64)]),
                            &v_list(&values),
                            ts,
                        )
                    }
                };
                commit.properties.push(property);
                commits.push(commit);
            }
            apply(&mut session, &config, &commits[0]);
            session.failure = failure;
            let mut rollups = 0;
            session
                .apply_group(&commits[1..65], 1024 * 1024, |key, value, ts| {
                    rollups += 1;
                    encode::property_row(key, &value, ts, false, &config.profile)
                })
                .unwrap();
            assert_eq!(rollups, 1);
            assert_eq!(count(&config, "object_propvalues"), 1);
            assert_eq!(session.progress.applied, 65);
            assert_eq!(session.progress.commits, 65);
            assert_eq!(session.progress.property_sequence, 65);
            // Deletion hides the pre-group base; a later full replacement hides both appends.
            session.failure = failure;
            session
                .apply_group(&commits[65..], 1024 * 1024, |key, value, ts| {
                    rollups += 1;
                    encode::property_row(key, &value, ts, false, &config.profile)
                })
                .unwrap();
            assert_eq!(rollups, 2);
            assert_eq!(count(&config, "object_propvalues"), 2);
            assert_eq!(session.progress.applied, 70);
            assert_eq!(session.progress.commits, 70);
            assert_eq!(session.progress.property_sequence, 70);
            session.fence().unwrap();
            drop(session);
            let (session, seed, _) = open(&config);
            assert_eq!(session.progress.commits, 70);
            assert_eq!(session.progress.property_sequence, 70);
            assert_eq!(seed.root.committed_ts, Timestamp(9999));
            assert_eq!(seed.sequences[0], 999);
            assert_eq!(
                seed.root.object_name.index_lookup(&object).unwrap().value.0,
                "name 70"
            );
            assert_eq!(
                seed.root
                    .object_propvalues
                    .index_lookup(&key)
                    .unwrap()
                    .value,
                v_list(&values)
            );
        }
    }
}

#[test]
#[ignore = "requires PostgreSQL fixture"]
fn failure_in_a_later_group_member_rolls_back_every_member() {
    let config = config();
    initialize_postgres_schema(&config).unwrap();
    let (mut session, _, epoch) = open(&config);
    let key = ObjAndUUIDHolder::new(&Obj::mk_id(7), Uuid::new_v4());
    let mut first = empty(epoch, 1, 1);
    first
        .properties
        .push(full(&config, &key, &v_list(&[v_int(1)]), 1));
    let mut second = empty(epoch, 2, 2);
    second.ordinary.push(RelationBatch {
        relation: "object_name",
        puts: Some(
            json!([{"object_ref":"#7", "logical_timestamp":"-1", "name":"invalid"}]).to_string(),
        ),
        deletes: None,
    });
    let error = session
        .apply_group(&[first, second], 1024 * 1024, |_, _, _| unreachable!())
        .unwrap_err();
    assert!(matches!(
        error,
        PostgresError::Operation {
            relation: "object_name",
            operation: "put",
            ..
        }
    ));
    assert_eq!(session.progress.applied, 0);
    assert_eq!(session.progress.commits, 0);
    assert_eq!(session.progress.property_sequence, 0);
    assert_eq!(count(&config, "object_propvalues"), 0);
    drop(session);
    let (session, seed, _) = open(&config);
    assert_eq!(session.progress.commits, 0);
    assert!(seed.root.object_propvalues.index_lookup(&key).is_none());
}

#[test]
#[ignore = "requires PostgreSQL fixture"]
fn rollup_expansion_seals_the_group_without_publishing_tentative_chains() {
    let config = config();
    initialize_postgres_schema(&config).unwrap();
    let (mut session, _, epoch) = open(&config);
    let first_key = ObjAndUUIDHolder::new(&Obj::mk_id(7), Uuid::new_v4());
    let large_key = ObjAndUUIDHolder::new(&Obj::mk_id(7), Uuid::new_v4());
    let mut first = empty(epoch, 1, 1);
    first
        .properties
        .push(full(&config, &first_key, &v_int(1), 1));
    let large = moor_var::v_str(&"x".repeat(16 * 1024));
    let mut second = empty(epoch, 2, 2);
    second.properties.push(append(
        &config,
        &large_key,
        &v_list(&[v_int(2)]),
        &v_list(&[large.clone(), v_int(2)]),
        2,
    ));
    let mut third = empty(epoch, 3, 3);
    let expected = v_list(&[large, v_int(2), v_int(3)]);
    third.properties.push(append(
        &config,
        &large_key,
        &v_list(&[v_int(3)]),
        &expected,
        3,
    ));
    let commits = [first, second, third];
    let mut rollups = 0;
    let mut render = |key: &ObjAndUUIDHolder, value: Var, ts| {
        rollups += 1;
        encode::property_row(key, &value, ts, false, &config.profile)
    };
    assert_eq!(session.apply_group(&commits, 4096, &mut render).unwrap(), 1);
    assert_eq!(
        session.metrics.snapshot().group_end_reasons
            [crate::PostgresGroupEnd::RollupExpansion as usize],
        1
    );
    assert_eq!(session.progress.applied, 1);
    assert_eq!(session.progress.property_sequence, 1);
    assert_eq!(count(&config, "object_propvalues"), 1);
    // The large member is retried alone. Its successor must use the confirmed full base.
    assert_eq!(
        session
            .apply_group(&commits[1..], 4096, &mut render)
            .unwrap(),
        1
    );
    assert_eq!(
        session
            .apply_group(&commits[2..], 4096, &mut render)
            .unwrap(),
        1
    );
    assert_eq!(rollups, 2);
    assert_eq!(session.progress.commits, 3);
    assert_eq!(session.progress.property_sequence, 3);
    assert_eq!(count(&config, "object_propvalues"), 3);
    drop(session);
    let (_, seed, _) = open(&config);
    assert_eq!(
        seed.root
            .object_propvalues
            .index_lookup(&large_key)
            .unwrap()
            .value,
        expected
    );
}

// Both codecs expand escaping before SQL adds its JSON transport representation.
// Successful publication must remain reloadable under the same row limit.
fn expanded_payload_is_rejected_or_reloadable(verb: bool) {
    use crate::{Database, DatabaseConfig, PersistenceConfig, StorageConfig, TxDB};
    use moor_common::{
        model::{ObjAttrs, ObjectKind, VerbArgsSpec, VerbFlag},
        util::BitEnum,
    };
    use moor_compiler::{
        read_persistent_source, write_persistent_literal, write_persistent_source,
    };
    use moor_var::{NOTHING, Symbol, v_str};
    use std::time::Duration;

    let config = config();
    initialize_postgres_schema(&config).unwrap();
    let (db, _) = TxDB::try_open(
        StorageConfig::postgres(config.clone()),
        DatabaseConfig::default(),
        PersistenceConfig::default(),
    )
    .unwrap();
    let before = db.publication();
    let mut loader = db.loader_client().unwrap();
    let object = loader
        .create_object(
            ObjectKind::NextObjid,
            &ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "boundary"),
        )
        .unwrap();
    let expected = v_str(&"\\".repeat(config.connection.max_row_bytes / 2));
    let mut literal = String::new();
    write_persistent_literal(&expected, &config.profile, &mut literal).unwrap();
    let mut canonical_source = String::new();
    if verb {
        let program =
            read_persistent_source(&format!("return {literal};"), &config.profile).unwrap();
        write_persistent_source(&program, &config.profile, &mut canonical_source).unwrap();
        loader
            .add_verb(
                &object,
                &[Symbol::mk("boundary")],
                &object,
                BitEnum::new_with(VerbFlag::Exec),
                VerbArgsSpec::this_none_this(),
                program,
            )
            .unwrap();
    } else {
        loader
            .define_property(
                &object,
                &object,
                Symbol::mk("boundary"),
                &object,
                BitEnum::new(),
                Some(expected.clone()),
            )
            .unwrap();
    }
    if loader.commit().is_err() {
        assert_eq!(
            db.publication(),
            before,
            "unsupported payload must fail before publication"
        );
        assert!(
            db.persistence_status().healthy,
            "rejection must not poison the writer"
        );
        return;
    }
    db.wait_for_durability(Duration::from_secs(60)).unwrap();
    drop(db);
    let (db, _) = TxDB::try_open(
        StorageConfig::postgres(config.clone()),
        DatabaseConfig::default(),
        PersistenceConfig::default(),
    )
    .unwrap_or_else(|error| panic!("accepted payload cannot reload: {error}"));
    let loader = db.loader_client().unwrap();
    if verb {
        let (uuid, _) = loader
            .get_existing_verb_by_names(&object, &[Symbol::mk("boundary")])
            .unwrap()
            .unwrap();
        let loaded = loader.get_verb_program(&object, uuid).unwrap();
        let mut source = String::new();
        write_persistent_source(&loaded, &config.profile, &mut source).unwrap();
        assert_eq!(source, canonical_source);
    } else {
        assert_eq!(
            loader
                .get_existing_property_value(&object, Symbol::mk("boundary"))
                .unwrap()
                .unwrap()
                .0,
            expected
        );
    }
}

#[test]
#[ignore = "requires PostgreSQL fixture"]
fn expanded_property_is_rejected_before_publication_or_reloads() {
    expanded_payload_is_rejected_or_reloadable(false);
}

#[test]
#[ignore = "requires PostgreSQL fixture"]
fn expanded_verb_source_is_rejected_before_publication_or_reloads() {
    expanded_payload_is_rejected_or_reloadable(true);
}

#[test]
#[ignore = "requires PostgreSQL fixture"]
fn near_limit_replacements_and_rollups_reload_and_rejections_leave_writer_usable() {
    use crate::{Database, DatabaseConfig, PersistenceConfig, StorageConfig, TxDB};
    use moor_common::{
        model::{ObjAttrs, ObjectKind},
        util::BitEnum,
    };
    use moor_var::{NOTHING, Symbol, v_str};
    use std::time::Duration;
    let mut config = config();
    config.connection.max_row_bytes = 8192;
    initialize_postgres_schema(&config).unwrap();
    let (db, _) = TxDB::try_open(
        StorageConfig::postgres(config.clone()),
        DatabaseConfig::default(),
        PersistenceConfig::default(),
    )
    .unwrap();
    let mut loader = db.loader_client().unwrap();
    let object = loader
        .create_object(
            ObjectKind::NextObjid,
            &ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "limits"),
        )
        .unwrap();
    let property = Symbol::mk("boundary");
    let mut expected = v_list(&[v_str(&"x".repeat(7000))]);
    loader
        .define_property(
            &object,
            &object,
            property,
            &object,
            BitEnum::new(),
            Some(expected.clone()),
        )
        .unwrap();
    let source = format!("return \"{}\";", "x".repeat(7600));
    let program = moor_compiler::read_persistent_source(&source, &config.profile).unwrap();
    loader
        .add_verb(
            &object,
            &[Symbol::mk("near_limit")],
            &object,
            BitEnum::new_with(moor_common::model::VerbFlag::Exec),
            moor_common::model::VerbArgsSpec::this_none_this(),
            program,
        )
        .unwrap();
    loader.commit().unwrap();
    let appends =
        crate::provider::property_value_store::PROPERTY_VALUE_CHAIN_LIMITS.max_records + 1;
    for n in 0..appends {
        let mut loader = db.loader_client().unwrap();
        expected = expected
            .as_list()
            .unwrap()
            .clone()
            .append_owned(&v_list(&[v_int(n as i64)]))
            .unwrap();
        loader
            .set_property(&object, property, None, None, Some(expected.clone()))
            .unwrap();
        loader.commit().unwrap();
    }
    db.wait_for_durability(Duration::from_secs(20)).unwrap();
    // A full replacement must have removed at least one earlier chain.
    assert!(count(&config, "object_propvalues") < appends);
    let stats = db.persistence_status().postgres.unwrap();
    assert_eq!(stats.append_validation_cache_hits, appends as u64);
    assert_eq!(stats.append_validation_cache_misses, 0);
    let before = db.publication();
    let mut loader = db.loader_client().unwrap();
    let too_large = expected
        .as_list()
        .unwrap()
        .clone()
        .append_owned(&v_list(&[v_str(&"\\".repeat(256))]))
        .unwrap();
    loader
        .set_property(&object, property, None, None, Some(too_large))
        .unwrap();
    let error = loader.commit().unwrap_err();
    assert!(error.to_string().contains("row exceeds"));
    assert_eq!(db.publication(), before);
    assert!(db.persistence_status().healthy);
    assert_eq!(db.persistence_status().outstanding, 0);
    // An ordinary write after the rejection proves that the permit and encoder remain usable.
    let mut loader = db.loader_client().unwrap();
    loader
        .set_object_name(&object, "after rejection".into())
        .unwrap();
    loader.commit().unwrap();
    db.wait_for_durability(Duration::from_secs(20)).unwrap();
    drop(db);
    let (db, _) = TxDB::try_open(
        StorageConfig::postgres(config.clone()),
        DatabaseConfig::default(),
        PersistenceConfig::default(),
    )
    .unwrap();
    let loader = db.loader_client().unwrap();
    assert_eq!(
        loader
            .get_existing_property_value(&object, property)
            .unwrap()
            .unwrap()
            .0,
        expected
    );
    assert_eq!(
        loader
            .get_existing_object(&object)
            .unwrap()
            .unwrap()
            .name()
            .as_deref(),
        Some("after rejection")
    );
    let (uuid, _) = loader
        .get_existing_verb_by_names(&object, &[Symbol::mk("near_limit")])
        .unwrap()
        .unwrap();
    let program = loader.get_verb_program(&object, uuid).unwrap();
    let mut loaded_source = String::new();
    moor_compiler::write_persistent_source(&program, &config.profile, &mut loaded_source).unwrap();
    assert_eq!(loaded_source, source);
    expected = loader
        .get_existing_property_value(&object, property)
        .unwrap()
        .unwrap()
        .0;
    drop(loader);
    for n in 0..2 {
        let mut loader = db.loader_client().unwrap();
        expected = expected
            .as_list()
            .unwrap()
            .clone()
            .append_owned(&v_list(&[v_int(n)]))
            .unwrap();
        loader
            .set_property(&object, property, None, None, Some(expected.clone()))
            .unwrap();
        loader.commit().unwrap();
    }
    let stats = db.persistence_status().postgres.unwrap();
    assert_eq!(stats.append_validation_cache_misses, 1);
    assert_eq!(stats.append_validation_cache_hits, 1);
    db.wait_for_durability(Duration::from_secs(20)).unwrap();
    drop(db);
    let (db, _) = TxDB::try_open(
        StorageConfig::postgres(config),
        DatabaseConfig::default(),
        PersistenceConfig::default(),
    )
    .unwrap();
    assert_eq!(
        db.loader_client()
            .unwrap()
            .get_existing_property_value(&object, property)
            .unwrap()
            .unwrap()
            .0,
        expected
    );
}

#[test]
#[ignore = "requires PostgreSQL fixture"]
fn concurrent_preparations_persist_the_rows_of_successful_publications() {
    use crate::{Database, DatabaseConfig, PersistenceConfig, StorageConfig, TxDB};
    use moor_common::{
        model::{CommitResult, ObjAttrs, ObjectKind},
        util::BitEnum,
    };
    use moor_var::NOTHING;
    use std::time::Duration;
    let config = config();
    initialize_postgres_schema(&config).unwrap();
    let (db, _) = TxDB::try_open(
        StorageConfig::postgres(config.clone()),
        DatabaseConfig::default(),
        PersistenceConfig::default(),
    )
    .unwrap();
    let barrier = std::sync::Barrier::new(4);
    let sampling_done = std::sync::atomic::AtomicBool::new(false);
    let expected = std::thread::scope(|scope| {
        let sampler = scope.spawn(|| {
            let mut samples = 0;
            while !sampling_done.load(std::sync::atomic::Ordering::Acquire) {
                let status = db.persistence_status();
                assert!(status.durable <= status.applied);
                assert!(status.applied <= status.last_submitted);
                assert!(status.last_submitted <= status.published);
                if status.sampling_consistent && status.healthy {
                    assert!(status.outstanding as u64 >= status.published - status.applied);
                }
                samples += 1;
                std::thread::yield_now();
            }
            samples
        });
        let barrier = &barrier;
        let handles: Vec<_> = (0..4)
            .map(|worker| {
                let db = &db;
                scope.spawn(move || {
                    barrier.wait();
                    let mut created = Vec::new();
                    for n in 0..25 {
                        let name = format!("worker-{worker}-{n}");
                        for attempt in 0..100 {
                            assert!(attempt < 99, "concurrent commit did not make progress");
                            let mut loader = db.loader_client().unwrap();
                            let object = loader
                                .create_object(
                                    ObjectKind::NextObjid,
                                    &ObjAttrs::new(
                                        NOTHING,
                                        NOTHING,
                                        NOTHING,
                                        BitEnum::new(),
                                        &name,
                                    ),
                                )
                                .unwrap();
                            if matches!(loader.commit().unwrap(), CommitResult::Success { .. }) {
                                created.push((object, name));
                                break;
                            }
                        }
                    }
                    created
                })
            })
            .collect();
        let expected = handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();
        sampling_done.store(true, std::sync::atomic::Ordering::Release);
        assert!(sampler.join().unwrap() > 0);
        expected
    });
    db.wait_for_durability(Duration::from_secs(20)).unwrap();
    assert_eq!(db.publication().version(), 100);
    assert_eq!(db.persistence_status().outstanding, 0);
    assert_eq!(db.persistence_status().postgres.unwrap().admission_bytes, 0);
    drop(db);
    let (db, _) = TxDB::try_open(
        StorageConfig::postgres(config),
        DatabaseConfig::default(),
        PersistenceConfig::default(),
    )
    .unwrap();
    let loader = db.loader_client().unwrap();
    for (object, name) in expected {
        assert_eq!(
            loader
                .get_existing_object(&object)
                .unwrap()
                .unwrap()
                .name()
                .as_deref(),
            Some(name.as_str())
        );
    }
}

#[test]
#[ignore = "requires PostgreSQL fixture"]
fn diagnostics_measure_blocked_payloads_then_release_them_after_application() {
    use crate::{Database, DatabaseConfig, PersistenceConfig, StorageConfig, TxDB};
    use moor_common::{
        model::{ObjAttrs, ObjectKind},
        util::BitEnum,
    };
    use moor_var::{NOTHING, Symbol, v_str};
    use std::time::Duration;
    let mut config = config();
    config.commit_policy = PostgresCommitPolicy::Asynchronous;
    initialize_postgres_schema(&config).unwrap();
    let (db, _) = TxDB::try_open(
        StorageConfig::postgres(config.clone()),
        DatabaseConfig::default(),
        PersistenceConfig::default(),
    )
    .unwrap();
    let mut loader = db.loader_client().unwrap();
    let object = loader
        .create_object(
            ObjectKind::NextObjid,
            &ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "diagnostics"),
        )
        .unwrap();
    let property = Symbol::mk("values");
    let base = v_list(&[v_str(&"x".repeat(64 * 1024))]);
    loader
        .define_property(
            &object,
            &object,
            property,
            &object,
            BitEnum::new(),
            Some(base.clone()),
        )
        .unwrap();
    loader.commit().unwrap();
    db.wait_for_durability(Duration::from_secs(10)).unwrap();
    let prior = db.persistence_status();
    let mut blocker = client(&config);
    blocker
        .query(
            "BEGIN",
            &[],
            Instant::now() + config.query_timeout,
            |_| unreachable!(),
        )
        .unwrap();
    blocker
        .query(
            &format!(
                "LOCK TABLE {} IN ACCESS EXCLUSIVE MODE",
                config.schema.qualify("object_propvalues").unwrap()
            ),
            &[],
            Instant::now() + config.query_timeout,
            |_| unreachable!(),
        )
        .unwrap();
    let mut loader = db.loader_client().unwrap();
    let appended = base
        .as_list()
        .unwrap()
        .clone()
        .append_owned(&v_list(&[v_str(&"y".repeat(64 * 1024))]))
        .unwrap();
    loader
        .set_property(&object, property, None, None, Some(appended))
        .unwrap();
    loader.commit().unwrap();
    assert!(
        db.create_snapshot_with_timeout(Duration::from_millis(20))
            .is_err()
    );
    assert_eq!(db.persistence_status().postgres.unwrap().active_exports, 0);
    let status = db.persistence_status();
    assert_eq!(status.published, prior.published + 1);
    assert_eq!(status.applied, prior.applied);
    let held = status.postgres.unwrap();
    assert_eq!(held.unapplied_commits, 1);
    assert!(held.retained_encoded_bytes >= 64 * 1024);
    assert!(held.retained_append_value_bytes >= 128 * 1024);
    assert!(held.encoding_calls >= 2);
    assert!(held.encoding_ns > 0);
    blocker
        .query(
            "COMMIT",
            &[],
            Instant::now() + config.query_timeout,
            |_| unreachable!(),
        )
        .unwrap();
    db.wait_for_durability(Duration::from_secs(10)).unwrap();
    let status = db.persistence_status();
    assert!(
        status.durable <= status.applied
            && status.applied <= status.last_submitted
            && status.last_submitted <= status.published
    );
    let done = status.postgres.unwrap();
    assert_eq!(done.unapplied_commits, 0);
    assert_eq!(done.retained_encoded_bytes, 0);
    assert_eq!(done.retained_append_value_bytes, 0);
    assert!(done.sql_application_ns > 0 && done.sql_commit_ns > 0 && done.fence_ns > 0);
    assert!(done.group_sql_statements > 0 && done.group_payload_bytes > 0);
    assert_eq!(done.group_end_reasons.iter().sum::<u64>(), done.groups);
    assert_eq!(done.group_commits, status.applied);
}

#[test]
#[ignore = "requires PostgreSQL fixture"]
fn snapshots_keep_one_prefix_across_point_reads_export_and_writer_shutdown() {
    use crate::{Database, DatabaseConfig, PersistenceConfig, StorageConfig, TxDB};
    use moor_common::{
        model::{HasUuid, ObjAttrs, ObjectKind, ValSet},
        util::BitEnum,
    };
    use moor_var::{NOTHING, Symbol, v_str};
    use std::{collections::BTreeMap, time::Duration};
    let mut config = config();
    config.max_exports = 1;
    initialize_postgres_schema(&config).unwrap();
    let (db, _) = TxDB::try_open(
        StorageConfig::postgres(config),
        DatabaseConfig::default(),
        PersistenceConfig::default(),
    )
    .unwrap();
    let root = Obj::mk_id(0);
    let property = Symbol::mk("history");
    let name_key = Symbol::mk("display\0name");
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
            BitEnum::new(),
            Some(v_list(&[v_int(1)])),
        )
        .unwrap();
    let mut names = BTreeMap::from([(root, "root".to_owned())]);
    for (index, kind) in [
        ObjectKind::Objid(Obj::mk_id(1)),
        ObjectKind::Objid(Obj::mk_id(256)),
        ObjectKind::Objid(Obj::mk_id(65536)),
        ObjectKind::UuObjId,
        ObjectKind::Anonymous,
    ]
    .into_iter()
    .enumerate()
    {
        let name = format!("child-{index}");
        let object = loader
            .create_object(
                kind,
                &ObjAttrs::new(root, root, NOTHING, BitEnum::new(), &name),
            )
            .unwrap();
        loader
            .set_object_metadata(&object, name_key, v_str(&name))
            .unwrap();
        names.insert(object, name);
    }
    let uuid = loader
        .get_existing_properties(&root)
        .unwrap()
        .iter()
        .next()
        .unwrap()
        .uuid();
    loader.commit().unwrap();
    // Enough appended records to cross a fetch boundary without triggering a rollup.
    for n in 2..=12 {
        let mut loader = db.loader_client().unwrap();
        loader
            .set_property(
                &root,
                property,
                None,
                None,
                Some(v_list(&(1..=n).map(v_int).collect::<Vec<_>>())),
            )
            .unwrap();
        loader.commit().unwrap();
    }
    let snapshot = db.create_snapshot().unwrap();
    assert_eq!(db.persistence_status().postgres.unwrap().active_exports, 1);
    let started = Instant::now();
    assert!(
        db.create_snapshot_with_timeout(Duration::from_millis(40))
            .is_err()
    );
    assert!(started.elapsed() < Duration::from_secs(1));
    assert_eq!(db.persistence_status().postgres.unwrap().active_exports, 1);
    let before = snapshot.get_property_value(&root, uuid).unwrap().0.unwrap();
    assert_eq!(before, v_list(&(1..=12).map(v_int).collect::<Vec<_>>()));
    let mut export = snapshot.begin_export(&[name_key]).unwrap();
    assert_eq!(export.object_count(), names.len());
    let mut loader = db.loader_client().unwrap();
    loader.set_object_name(&root, "later".into()).unwrap();
    loader
        .set_property(&root, property, None, None, Some(v_int(999)))
        .unwrap();
    loader.commit().unwrap();
    db.wait_for_durability(Duration::from_secs(10)).unwrap();
    assert_eq!(
        snapshot.get_object(&root).unwrap().name().as_deref(),
        Some("root")
    );
    assert_eq!(
        snapshot.get_property_value(&root, uuid).unwrap().0,
        Some(before.clone())
    );
    // The owned read session is independent of the writer and its shutdown signal.
    drop(db);
    let mut exported = BTreeMap::new();
    while let Some(object) = export.next_object().unwrap() {
        if object.oid == root {
            assert_eq!(object.properties[0].value, Some(before.clone()));
        } else {
            assert!(object.properties.is_empty());
        }
        exported.insert(object.oid, object.name);
    }
    assert_eq!(exported, names);
    drop(export);
    assert_eq!(
        snapshot.get_property_value(&root, uuid).unwrap().0,
        Some(before)
    );
}

#[test]
#[ignore = "requires PostgreSQL fixture"]
fn snapshot_disconnect_fails_without_reconnecting_and_releases_capacity() {
    use crate::{Database, DatabaseConfig, PersistenceConfig, StorageConfig, TxDB};
    use moor_common::{
        model::{ObjAttrs, ObjectKind},
        util::BitEnum,
    };
    use moor_var::NOTHING;
    use std::time::Duration;
    let mut config = config();
    config.max_exports = 1;
    initialize_postgres_schema(&config).unwrap();
    let (db, _) = TxDB::try_open(
        StorageConfig::postgres(config.clone()),
        DatabaseConfig::default(),
        PersistenceConfig::default(),
    )
    .unwrap();
    let mut loader = db.loader_client().unwrap();
    let object = loader
        .create_object(
            ObjectKind::NextObjid,
            &ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "before"),
        )
        .unwrap();
    loader.commit().unwrap();
    let snapshot = db.create_snapshot().unwrap();
    // Only this schema's snapshot transaction has this exact last progress query.
    let mut admin = client(&config);
    let progress = config.schema.qualify("writer_progress").unwrap();
    let mut killed = 0;
    admin.query("SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE pid<>pg_backend_pid() AND state='idle in transaction' AND query LIKE $1", &[PostgresParam::Text(25, &format!("%FROM {progress}%"))], Instant::now()+config.query_timeout, |_| { killed += 1; Ok(()) }).unwrap();
    assert_eq!(killed, 1);
    assert!(snapshot.get_object(&object).is_err());
    assert!(snapshot.get_object(&object).is_err());
    let replacement = db
        .create_snapshot_with_timeout(Duration::from_secs(2))
        .unwrap();
    assert_eq!(
        replacement.get_object(&object).unwrap().name().as_deref(),
        Some("before")
    );
    drop(replacement);
    assert_eq!(db.persistence_status().postgres.unwrap().active_exports, 0);
}

#[test]
#[ignore = "requires PostgreSQL fixture"]
fn snapshots_preserve_value_only_rows_and_report_malformed_chains_without_payloads() {
    use crate::{Database, DatabaseConfig, PersistenceConfig, StorageConfig, TxDB};
    use moor_common::{
        model::{HasUuid, ObjAttrs, ObjectKind, ValSet},
        util::BitEnum,
    };
    use moor_var::{NOTHING, Symbol, v_str};
    use std::time::Duration;
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
    let property = Symbol::mk("value");
    loader
        .define_property(
            &parent,
            &parent,
            property,
            &parent,
            BitEnum::new(),
            Some(v_int(1)),
        )
        .unwrap();
    let child = loader
        .create_object(
            ObjectKind::NextObjid,
            &ObjAttrs::new(parent, parent, NOTHING, BitEnum::new(), "child"),
        )
        .unwrap();
    loader
        .set_property(&child, property, None, None, Some(v_str("local")))
        .unwrap();
    let uuid = loader
        .get_existing_properties(&parent)
        .unwrap()
        .iter()
        .next()
        .unwrap()
        .uuid();
    loader.commit().unwrap();
    db.wait_for_durability(Duration::from_secs(10)).unwrap();
    drop(db);
    let mut admin = client(&config);
    // Construct the supported value-only stored shape while the writer is offline.
    admin
        .query(
            &format!(
                "DELETE FROM {} WHERE object_ref=$1",
                config.schema.qualify("object_propflags").unwrap()
            ),
            &[PostgresParam::Text(25, &child.to_literal())],
            Instant::now() + config.query_timeout,
            |_| unreachable!(),
        )
        .unwrap();
    let (db, _) = TxDB::try_open(
        StorageConfig::postgres(config.clone()),
        DatabaseConfig::default(),
        PersistenceConfig::default(),
    )
    .unwrap();
    let snapshot = db.create_snapshot().unwrap();
    let rows = snapshot.get_property_snapshots(&child).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].value, Some(v_str("local")));
    assert!(rows[0].permissions.is_none());
    let mut export = snapshot.begin_export(&[]).unwrap();
    while let Some(object) = export.next_object().unwrap() {
        if object.oid == child {
            assert_eq!(object.properties[0].value, Some(v_str("local")));
            assert!(object.properties[0].permissions.is_none());
        }
    }
    drop(export);
    // Corruption injection is fixture-only; snapshots must preserve safe row context.
    admin.query(&format!("UPDATE {} SET value_literal='\"private-payload\"', value_kind='list' WHERE object_ref=$1",config.schema.qualify("object_propvalues").unwrap()), &[PostgresParam::Text(25,&child.to_literal())], Instant::now()+config.query_timeout, |_|unreachable!()).unwrap();
    let malformed = db.create_snapshot().unwrap();
    let message = malformed
        .get_property_value(&child, uuid)
        .unwrap_err()
        .to_string();
    assert!(
        message.contains("object_propvalues") && message.contains(&uuid.to_string()),
        "{message}"
    );
    assert!(!message.contains("private-payload"));
    assert_eq!(
        snapshot.get_property_value(&child, uuid).unwrap().0,
        Some(v_str("local"))
    );
    drop(malformed);
    drop(snapshot);
    assert_eq!(db.persistence_status().postgres.unwrap().active_exports, 0);
    let pool = super::snapshot::ExportPool::new(1);
    let error = super::snapshot::ReadSession::open(
        config.clone(),
        &pool,
        PublicationId::new(WriterEpoch::random(), 0),
        Instant::now() + config.query_timeout,
        || true,
    )
    .err()
    .unwrap();
    assert_eq!(error, PostgresError::OwnershipLost);
    assert_eq!(pool.diagnostics().0, 0);
    admin
        .query(
            &format!(
                "INSERT INTO {}(object_ref,logical_timestamp,parent_ref) VALUES($2,0,$1) ON CONFLICT(object_ref) DO UPDATE SET parent_ref=excluded.parent_ref",
                config.schema.qualify("object_parent").unwrap()
            ),
            &[
                PostgresParam::Text(25, &child.to_literal()),
                PostgresParam::Text(25, &parent.to_literal()),
            ],
            Instant::now() + config.query_timeout,
            |_| unreachable!(),
        )
        .unwrap();
    let cyclic = db.create_snapshot().unwrap();
    assert!(
        cyclic
            .get_property_snapshots(&parent)
            .unwrap_err()
            .to_string()
            .contains("Cycle")
    );
    assert!(cyclic.begin_export(&[]).is_err());
}

#[test]
#[ignore = "requires PostgreSQL fixture"]
fn validation_is_read_only_and_detects_corrupt_rows_and_relationships() {
    use crate::{Database, DatabaseConfig, PersistenceConfig, StorageConfig, TxDB};
    use moor_common::{
        model::{ObjAttrs, ObjectKind, VerbArgsSpec},
        util::BitEnum,
    };
    use moor_var::{NOTHING, Symbol};
    use std::time::Duration;
    let config = config();
    let identity = initialize_postgres_schema(&config).unwrap();
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
            &ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "root"),
        )
        .unwrap();
    loader
        .define_property(
            &parent,
            &parent,
            Symbol::mk("history"),
            &parent,
            BitEnum::new(),
            Some(v_list(&[v_int(1)])),
        )
        .unwrap();
    let program = moor_compiler::read_persistent_source("return 42;", &config.profile).unwrap();
    loader
        .add_verb(
            &parent,
            &[Symbol::mk("answer")],
            &parent,
            BitEnum::new(),
            VerbArgsSpec::this_none_this(),
            program,
        )
        .unwrap();
    loader
        .create_object(
            ObjectKind::NextObjid,
            &ObjAttrs::new(parent, parent, NOTHING, BitEnum::new(), "child"),
        )
        .unwrap();
    loader.commit().unwrap();
    db.wait_for_durability(Duration::from_secs(10)).unwrap();
    let mut admin = client(&config);
    let before =
        state::read_progress(&mut admin, &config, Instant::now() + config.query_timeout).unwrap();
    // Writer still owns the schema lock. Validation must neither claim it nor advance progress.
    let report = validate_postgres_storage(&config).unwrap();
    assert_eq!(report.database_id, identity);
    assert_eq!(report.writer_epoch, before.epoch);
    assert_eq!(report.relation_rows["object_flags"], 2);
    assert_eq!(report.relation_rows["object_verbs"], 1);
    assert_eq!(report.property_values, 1);
    assert_eq!(
        before,
        state::read_progress(&mut admin, &config, Instant::now() + config.query_timeout).unwrap()
    );
    drop(db);
    let s = format!("\"{}\"", config.schema.as_str());
    let cases = [
        ("world_metadata", "SET source_format=999", "world_metadata"),
        (
            "sequence_slots",
            "SET high_water=-1 WHERE slot=0",
            "high-water",
        ),
        (
            "object_name",
            "SET logical_timestamp=18446744073709551615",
            "object_name",
        ),
        (
            "object_propvalues",
            "SET value_literal='secret malformed ['",
            "object_propvalues",
        ),
        (
            "object_propvalues",
            "SET record_kind='list_append'",
            "property chain",
        ),
        (
            "object_verbs",
            "SET source='secret malformed ['",
            "object_verbs",
        ),
        ("object_parent", "SET parent_ref='#1'", "cycle"),
        ("object_parent", "SET parent_ref='#999'", "absent object"),
    ];
    for (table, mutation, expected) in cases {
        admin
            .query(
                &format!("CREATE TEMP TABLE validation_backup AS SELECT * FROM {s}.{table}"),
                &[],
                Instant::now() + config.query_timeout,
                |_| unreachable!(),
            )
            .unwrap();
        admin
            .query(
                &format!("UPDATE {s}.{table} {mutation}"),
                &[],
                Instant::now() + config.query_timeout,
                |_| unreachable!(),
            )
            .unwrap();
        let error = validate_postgres_storage(&config).unwrap_err().to_string();
        assert!(error.contains(expected), "{expected}: {error}");
        assert!(!error.contains("secret"), "{error}");
        if table == "object_verbs" {
            assert!(error.contains("line"), "{error}");
        }
        for restore in [
            format!("DELETE FROM {s}.{table}"),
            format!("INSERT INTO {s}.{table} SELECT * FROM validation_backup"),
            "DROP TABLE validation_backup".into(),
        ] {
            admin
                .query(
                    &restore,
                    &[],
                    Instant::now() + config.query_timeout,
                    |_| unreachable!(),
                )
                .unwrap();
        }
        validate_postgres_storage(&config).unwrap();
    }
    for (table, expected) in [
        ("sequence_slots", "missing slot"),
        ("object_propflags", "canonical property permissions"),
        ("object_verbs", "verb definition and source"),
    ] {
        admin
            .query(
                &format!("CREATE TEMP TABLE validation_backup AS SELECT * FROM {s}.{table}"),
                &[],
                Instant::now() + config.query_timeout,
                |_| unreachable!(),
            )
            .unwrap();
        admin
            .query(
                &format!("DELETE FROM {s}.{table}"),
                &[],
                Instant::now() + config.query_timeout,
                |_| unreachable!(),
            )
            .unwrap();
        let error = validate_postgres_storage(&config).unwrap_err().to_string();
        assert!(error.contains(expected), "{expected}: {error}");
        for restore in [
            format!("INSERT INTO {s}.{table} SELECT * FROM validation_backup"),
            "DROP TABLE validation_backup".into(),
        ] {
            admin
                .query(
                    &restore,
                    &[],
                    Instant::now() + config.query_timeout,
                    |_| unreachable!(),
                )
                .unwrap();
        }
    }
    admin
        .query(
            &format!("INSERT INTO {s}.object_name VALUES ('#999',0,'orphan','utf8')"),
            &[],
            Instant::now() + config.query_timeout,
            |_| unreachable!(),
        )
        .unwrap();
    assert!(
        validate_postgres_storage(&config)
            .unwrap_err()
            .to_string()
            .contains("absent object")
    );
    admin
        .query(
            &format!("DELETE FROM {s}.object_name WHERE object_ref='#999'"),
            &[],
            Instant::now() + config.query_timeout,
            |_| unreachable!(),
        )
        .unwrap();
    // Deleted definitions leave inactive stored values/permissions under the current engine contract.
    admin
        .query(
            &format!("DELETE FROM {s}.object_propdefs"),
            &[],
            Instant::now() + config.query_timeout,
            |_| unreachable!(),
        )
        .unwrap();
    assert_eq!(
        validate_postgres_storage(&config)
            .unwrap()
            .inactive_property_entries,
        2
    );
}

fn inspection_value(
    config: &PostgresStorageConfig,
    key: &ObjAndUUIDHolder,
) -> Result<Vec<String>, PostgresError> {
    let mut result = Vec::new();
    client(config).query(
        &format!(
            "SELECT value_literal, logical_timestamp::text, record_count::text FROM {}($1,$2)",
            config.schema.qualify("read_property_value").unwrap()
        ),
        &[
            PostgresParam::Text(25, &key.obj().to_literal()),
            PostgresParam::Text(2950, &key.uuid().to_string()),
        ],
        Instant::now() + config.query_timeout,
        |row| {
            result = row
                .columns
                .into_iter()
                .map(|c| String::from_utf8(c.unwrap()).unwrap())
                .collect();
            Ok(())
        },
    )?;
    Ok(result)
}

#[test]
#[ignore = "requires PostgreSQL fixture"]
fn inspection_reconstructs_nested_lists_in_record_order_and_checks_chain_shapes() {
    use moor_compiler::{read_persistent_literal, write_persistent_literal};
    use moor_var::{v_map, v_str};
    let config = config();
    initialize_postgres_schema(&config).unwrap();
    let (mut session, _, epoch) = open(&config);
    assert!(matches!(
        install_postgres_inspection(&config),
        Err(PostgresError::OwnershipLost)
    ));
    let key = ObjAndUUIDHolder::new(&Obj::mk_id(7), Uuid::new_v4());
    let lambda =
        read_persistent_literal("{} => x with captured [{x: 42}]", &config.profile).unwrap();
    let first = v_list(&[v_str("braces }, {, quotes \" and NUL\0"), lambda.clone()]);
    let suffix = v_list(&[v_map(&[(v_str("a,b"), v_list(&[v_int(3)]))]), lambda]);
    let expected = first
        .as_list()
        .unwrap()
        .clone()
        .append_owned(&suffix)
        .unwrap();
    let mut batch = empty(epoch, 1, 100);
    batch.properties.push(full(&config, &key, &first, 100));
    use moor_common::{
        model::{PropDef, PropDefs, ValSet, VerbArgsSpec, VerbDef, VerbDefs},
        util::BitEnum,
    };
    use moor_var::Symbol;
    let definitions = PropDefs::from_items(&[PropDef::new(
        key.uuid(),
        key.obj(),
        key.obj(),
        Symbol::mk("Named\0Property"),
    )]);
    let verbs = VerbDefs::from_items(&[VerbDef::new(
        Uuid::new_v4(),
        key.obj(),
        key.obj(),
        &[Symbol::mk("answer"), Symbol::mk("Alias\0Name")],
        BitEnum::new(),
        VerbArgsSpec::this_none_this(),
    )]);
    for (relation, row) in [
        (
            "object_propdefs",
            rows::encode(
                "object_propdefs",
                Timestamp(100),
                &key.obj(),
                &definitions,
                &config.profile,
            )
            .unwrap(),
        ),
        (
            "object_verbdefs",
            rows::encode(
                "object_verbdefs",
                Timestamp(100),
                &key.obj(),
                &verbs,
                &config.profile,
            )
            .unwrap(),
        ),
    ] {
        batch.ordinary.push(RelationBatch {
            relation,
            puts: Some(json!([row]).to_string()),
            deletes: None,
        });
    }
    apply(&mut session, &config, &batch);
    let mut batch = empty(epoch, 2, 90);
    batch
        .properties
        .push(append(&config, &key, &suffix, &expected, 90));
    apply(&mut session, &config, &batch);
    let value = inspection_value(&config, &key).unwrap();
    assert_eq!(&value[1..], ["90", "2"]);
    let mut expected_text = String::new();
    let mut actual_text = String::new();
    write_persistent_literal(&expected, &config.profile, &mut expected_text).unwrap();
    write_persistent_literal(
        &read_persistent_literal(&value[0], &config.profile).unwrap(),
        &config.profile,
        &mut actual_text,
    )
    .unwrap();
    assert_eq!(actual_text, expected_text);
    assert!(
        inspection_value(
            &config,
            &ObjAndUUIDHolder::new(&Obj::mk_id(999), key.uuid())
        )
        .unwrap()
        .is_empty()
    );
    drop(session);
    let mut admin = client(&config);
    for (view, projection, expected) in [
        (
            "property_definitions",
            "property_name, name_encoding",
            "Named\0Property",
        ),
        ("verb_names", "name, name_encoding", "Alias\0Name"),
    ] {
        let mut found = false;
        admin
            .query(
                &format!(
                    "SELECT {projection} FROM {} WHERE name_encoding='json_string'",
                    config.schema.qualify(view).unwrap()
                ),
                &[],
                Instant::now() + config.query_timeout,
                |row| {
                    assert_eq!(
                        serde_json::from_slice::<String>(row.columns[0].as_ref().unwrap()).unwrap(),
                        expected
                    );
                    assert_eq!(row.columns[1].as_deref(), Some(b"json_string".as_slice()));
                    found = true;
                    Ok(())
                },
            )
            .unwrap();
        assert!(found);
    }
    let before =
        state::read_progress(&mut admin, &config, Instant::now() + config.query_timeout).unwrap();
    admin
        .query(
            &format!("DROP VIEW {}", config.schema.qualify("objects").unwrap()),
            &[],
            Instant::now() + config.query_timeout,
            |_| unreachable!(),
        )
        .unwrap();
    install_postgres_inspection(&config).unwrap();
    assert_eq!(
        before,
        state::read_progress(&mut admin, &config, Instant::now() + config.query_timeout).unwrap()
    );
    let table = config.schema.qualify("object_propvalues").unwrap();
    admin
        .query(
            &format!("CREATE TEMP TABLE inspection_backup AS SELECT * FROM {table}"),
            &[],
            Instant::now() + config.query_timeout,
            |_| unreachable!(),
        )
        .unwrap();
    admin
        .query(
            &format!("UPDATE {table} SET value_literal='{{}}' WHERE record_sequence=1"),
            &[],
            Instant::now() + config.query_timeout,
            |_| unreachable!(),
        )
        .unwrap();
    let empty_base = inspection_value(&config, &key).unwrap();
    let mut empty_base_text = String::new();
    let mut suffix_text = String::new();
    write_persistent_literal(
        &read_persistent_literal(&empty_base[0], &config.profile).unwrap(),
        &config.profile,
        &mut empty_base_text,
    )
    .unwrap();
    write_persistent_literal(&suffix, &config.profile, &mut suffix_text).unwrap();
    assert_eq!(empty_base_text, suffix_text);
    for restore in [
        format!("DELETE FROM {table}"),
        format!("INSERT INTO {table} SELECT * FROM inspection_backup"),
    ] {
        admin
            .query(
                &restore,
                &[],
                Instant::now() + config.query_timeout,
                |_| unreachable!(),
            )
            .unwrap();
    }
    for mutation in [
        "SET record_kind='list_append' WHERE record_sequence=1",
        "SET record_kind='full' WHERE record_sequence=2",
        "SET value_literal='{}' WHERE record_sequence=2",
        "SET value_literal='7',value_kind='int' WHERE record_sequence=1",
        "SET literal_format=99",
        "SET value_literal='bad' WHERE record_sequence=1",
        "SET value_literal='{' || repeat('1,',2097152) || '1}' WHERE record_sequence=2",
    ] {
        admin
            .query(
                &format!("UPDATE {table} {mutation}"),
                &[],
                Instant::now() + config.query_timeout,
                |_| unreachable!(),
            )
            .unwrap();
        assert_eq!(
            inspection_value(&config, &key).unwrap_err(),
            PostgresError::SqlState("22000".into())
        );
        for restore in [
            format!("DELETE FROM {table}"),
            format!("INSERT INTO {table} SELECT * FROM inspection_backup"),
        ] {
            admin
                .query(
                    &restore,
                    &[],
                    Instant::now() + config.query_timeout,
                    |_| unreachable!(),
                )
                .unwrap();
        }
    }
    admin.query(&format!("INSERT INTO {table} SELECT object_ref, property_uuid, n, logical_timestamp, 'list_append', literal_format, 'list', '{{1}}' FROM inspection_backup CROSS JOIN generate_series(3,65) n WHERE record_sequence=1"), &[], Instant::now() + config.query_timeout, |_| unreachable!()).unwrap();
    assert_eq!(
        inspection_value(&config, &key).unwrap_err(),
        PostgresError::SqlState("22000".into())
    );
    for restore in [
        format!("DELETE FROM {table}"),
        format!("INSERT INTO {table} SELECT * FROM inspection_backup"),
    ] {
        admin
            .query(
                &restore,
                &[],
                Instant::now() + config.query_timeout,
                |_| unreachable!(),
            )
            .unwrap();
    }
    // Scale the unrelated key range so targeted lookup must use the physical primary key.
    admin.query(&format!("INSERT INTO {table} SELECT '#'||n, property_uuid, 1, 0, 'full', literal_format, 'list', '{{0}}' FROM inspection_backup CROSS JOIN generate_series(1000,20999) n WHERE record_sequence=1"), &[], Instant::now() + config.query_timeout, |_| unreachable!()).unwrap();
    admin
        .query(
            &format!("VACUUM ANALYZE {table}"),
            &[],
            Instant::now() + config.query_timeout,
            |_| unreachable!(),
        )
        .unwrap();
    let views = config.schema.qualify("property_values").unwrap();
    for (name, sql) in [
        (
            "records",
            format!(
                "SELECT * FROM {table} WHERE object_ref=$1 AND property_uuid=$2 ORDER BY record_sequence"
            ),
        ),
        (
            "view",
            format!("SELECT * FROM {views} WHERE object_ref=$1 AND property_uuid=$2"),
        ),
    ] {
        let mut plan = serde_json::Value::Null;
        admin
            .query(
                &format!("EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) {sql}"),
                &[
                    PostgresParam::Text(25, "#7"),
                    PostgresParam::Text(2950, &key.uuid().to_string()),
                ],
                Instant::now() + config.query_timeout,
                |row| {
                    plan = serde_json::from_slice(row.columns[0].as_ref().unwrap()).unwrap();
                    Ok(())
                },
            )
            .unwrap();
        fn check_plan(node: &serde_json::Value, found: &mut bool) {
            if node["Relation Name"] == "object_propvalues" {
                assert!(
                    matches!(
                        node["Node Type"].as_str().unwrap(),
                        "Index Scan" | "Index Only Scan" | "Bitmap Heap Scan"
                    ),
                    "{node}"
                );
                let condition = node["Index Cond"]
                    .as_str()
                    .or_else(|| node["Recheck Cond"].as_str())
                    .unwrap();
                assert!(
                    condition.contains("object_ref") && condition.contains("property_uuid"),
                    "{node}"
                );
                assert!(node["Actual Rows"].as_f64().unwrap() <= 2.0, "{node}");
                *found = true;
            }
            if let Some(children) = node["Plans"].as_array() {
                for child in children {
                    check_plan(child, found);
                }
            }
        }
        let mut found = false;
        check_plan(&plan[0]["Plan"], &mut found);
        assert!(found, "{plan}");
        println!(
            "inspection {name}: 20000 unrelated properties; execution_ms={}, shared_hit_blocks={}",
            plan[0]["Execution Time"], plan[0]["Plan"]["Shared Hit Blocks"]
        );
        if let Ok(directory) = std::env::var("MOOR_PG_PLAN_DIR") {
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(
                std::path::Path::new(&directory)
                    .join(format!("{}-{name}.json", config.schema.as_str())),
                serde_json::to_string_pretty(&plan).unwrap(),
            )
            .unwrap();
        }
    }
}

#[test]
#[ignore = "requires PostgreSQL fixture"]
fn incompatible_format_combinations_fail_before_claiming_a_writer_epoch() {
    let config = config();
    initialize_postgres_schema(&config).unwrap();
    let mut connection = client(&config);
    let table = config.schema.qualify("world_metadata").unwrap();
    let original = serde_json::Value::Object(
        state::singleton(
            &mut connection,
            &format!("SELECT to_jsonb(t)::text FROM {table} t"),
            &[],
            Instant::now() + config.query_timeout,
        )
        .unwrap(),
    );
    let before = state::read_progress(
        &mut connection,
        &config,
        Instant::now() + config.query_timeout,
    )
    .unwrap();
    let mut incompatible = Vec::new();
    for field in ["schema_version", "literal_format", "source_format"] {
        for version in [0, 2] {
            let mut changed = original.clone();
            changed[field] = json!(version);
            incompatible.push(changed);
        }
    }
    let mut changed = original.clone();
    changed["compiler_profile"] = json!("moo-v2");
    incompatible.push(changed);
    for field in [
        "literal_version",
        "source_version",
        "compiler_profile_version",
    ] {
        let mut changed = original.clone();
        changed["profile"][field] = json!(2);
        incompatible.push(changed);
    }
    let mut changed = original.clone();
    changed["profile"]["language"] = json!("unknown");
    incompatible.push(changed);
    for option in [
        "flyweight_type",
        "bool_type",
        "symbol_type",
        "custom_errors",
        "call_unsupported_builtins",
        "legacy_type_constants",
    ] {
        let mut changed = original.clone();
        changed["profile"]["options"][option] =
            json!(!changed["profile"]["options"][option].as_bool().unwrap());
        incompatible.push(changed);
    }
    for changed in incompatible.iter().chain(std::iter::once(&original)) {
        connection.query(&format!("UPDATE {table} SET (schema_version,literal_format,source_format,compiler_profile,profile)=(SELECT schema_version,literal_format,source_format,compiler_profile,profile FROM pg_catalog.jsonb_populate_record(NULL::{table},$1))"),
            &[PostgresParam::Text(3802, &changed.to_string())], Instant::now() + config.query_timeout,
            |_| unreachable!()).unwrap();
        if changed == &original {
            break;
        }
        assert!(matches!(
            Session::open(
                config.clone(),
                &Relations::init(),
                WriterEpoch::random(),
                PostgresShutdown::default()
            ),
            Err(PostgresError::Format {
                field: "world_metadata",
                ..
            })
        ));
        assert!(validate_postgres_storage(&config).is_err());
        assert_eq!(
            state::read_progress(
                &mut connection,
                &config,
                Instant::now() + config.query_timeout
            )
            .unwrap(),
            before
        );
        let stored = state::singleton(
            &mut connection,
            &format!("SELECT to_jsonb(t)::text FROM {table} t"),
            &[],
            Instant::now() + config.query_timeout,
        )
        .unwrap();
        assert_eq!(
            serde_json::Value::Object(stored),
            *changed,
            "startup must not repair formats"
        );
    }
    let (session, _, _) = open(&config);
    assert_eq!(session.progress.commits, 0);
}

#[test]
#[ignore = "requires PostgreSQL fixture"]
fn property_deletes_use_key_and_tuple_lookups_and_remove_complete_chains() {
    let config = config();
    initialize_postgres_schema(&config).unwrap();
    let mut connection = client(&config);
    let table = config.schema.qualify("object_propvalues").unwrap();
    let deadline = Instant::now() + config.query_timeout;
    sql::prepare(&mut connection, &config, deadline).unwrap();
    // Exercise the prepared statement while the relation is empty, beyond the
    // five executions after which PostgreSQL can select a generic plan.
    for _ in 0..6 {
        connection
            .execute_prepared(
                "delete_object_propvalues",
                &[PostgresParam::Text(3802, "[]")],
                deadline,
                |_| unreachable!(),
            )
            .unwrap();
    }
    connection.query(&format!("INSERT INTO {table} SELECT '#'||n::text,'00000000-0000-0000-0000-000000000001'::uuid,1,1,'full',1,'list','{{1}}' FROM generate_series(1,20000) n"), &[], deadline, |_| unreachable!()).unwrap();
    connection
        .query(
            &format!("ALTER TABLE {table} SET (autovacuum_enabled=false)"),
            &[],
            deadline,
            |_| unreachable!(),
        )
        .unwrap();
    // Leave dead index entries from many older full replacements, as sustained writes do.
    connection.query(&format!("INSERT INTO {table} SELECT '#'||n::text,'00000000-0000-0000-0000-000000000001'::uuid,v,v,'full',1,'list','{{1}}' FROM generate_series(1,512) n CROSS JOIN generate_series(2,128) v"), &[], deadline, |_| unreachable!()).unwrap();
    connection.query(&format!("DELETE FROM {table} WHERE record_sequence<128 AND object_ref IN (SELECT '#'||n::text FROM generate_series(1,512) n)"), &[], deadline, |_| unreachable!()).unwrap();
    connection.query(&format!("INSERT INTO {table} SELECT '#'||n::text,'00000000-0000-0000-0000-000000000001'::uuid,129,129,'list_append',1,'list','{{2}}' FROM generate_series(1,512) n"), &[], deadline, |_| unreachable!()).unwrap();
    connection
        .query(
            &format!("ANALYZE {table}"),
            &[],
            deadline,
            |_| unreachable!(),
        )
        .unwrap();
    let mut keys: Vec<_> = (1..=512).map(|id| json!({"object_ref":format!("#{id}"), "property_uuid":"00000000-0000-0000-0000-000000000001", "record_sequence":128})).collect();
    keys.push(keys[0].clone()); // Repeated input keys must not delete a tuple twice.
    let keys = serde_json::Value::Array(keys).to_string();
    let predicate = "t.object_ref=d.object_ref AND t.property_uuid=d.property_uuid";
    // The fixture keys contain no SQL quotes. Use the statement prepared before
    // growth so this also covers the plan-cache behavior of a live writer.
    assert!(!keys.contains('\''));
    let indexed = format!("EXECUTE delete_object_propvalues('{keys}')");
    let previous = format!(
        "DELETE FROM {table} t USING pg_catalog.jsonb_populate_recordset(NULL::{table},$1) d WHERE {predicate}"
    );
    let mut plans = Vec::new();
    for (name, statement) in [("previous", previous), ("indexed", indexed)] {
        connection
            .query("BEGIN", &[], deadline, |_| unreachable!())
            .unwrap();
        let mut plan = None;
        let params = [PostgresParam::Text(3802, &keys)];
        connection
            .query(
                &format!("EXPLAIN (ANALYZE,BUFFERS,WAL,FORMAT JSON) {statement}"),
                if name == "previous" { &params } else { &[] },
                deadline,
                |row| {
                    plan = Some(rows::parse_row(row)?);
                    Ok(())
                },
            )
            .unwrap();
        connection.query(&format!("SELECT count(*),count(*) FILTER (WHERE object_ref='#1'),count(*) FILTER (WHERE object_ref='#513') FROM {table}"), &[], deadline,
            |row| {
                assert_eq!(row.columns[0].as_deref(), Some(b"19488".as_slice()));
                assert_eq!(row.columns[1].as_deref(), Some(b"0".as_slice()));
                assert_eq!(row.columns[2].as_deref(), Some(b"1".as_slice()));
                Ok(())
            }).unwrap();
        connection
            .query("ROLLBACK", &[], deadline, |_| unreachable!())
            .unwrap();
        let plan = plan.unwrap();
        if name == "indexed" {
            fn nodes<'a>(node: &'a serde_json::Value, out: &mut Vec<&'a serde_json::Value>) {
                out.push(node);
                if let Some(children) = node["Plans"].as_array() {
                    for child in children {
                        nodes(child, out);
                    }
                }
            }
            let mut all = Vec::new();
            nodes(&plan[0]["Plan"], &mut all);
            assert!(all.iter().any(|n| n["Node Type"] == "Tid Scan"), "{plan}");
            assert!(
                all.iter()
                    .any(|n| n["Index Name"] == "object_propvalues_pkey"),
                "{plan}"
            );
            for node in all
                .iter()
                .filter(|n| n["Index Name"] == "object_propvalues_pkey")
            {
                assert!(
                    node["Index Cond"]
                        .as_str()
                        .unwrap()
                        .contains("record_sequence"),
                    "{plan}"
                );
                assert!(
                    node["Actual Rows"].as_f64().unwrap() <= 2.0,
                    "old index entries must be excluded: {plan}"
                );
            }
            assert!(
                !all.iter()
                    .any(|n| n["Node Type"] == "Seq Scan"
                        && n["Relation Name"] == "object_propvalues"),
                "{plan}"
            );
        }
        plans.push(json!({"variant":name,"plan":plan}));
    }
    assert_eq!(count(&config, "object_propvalues"), 20512);
    connection
        .query(
            "SELECT generic_plans,custom_plans FROM pg_prepared_statements WHERE name='delete_object_propvalues'",
            &[],
            deadline,
            |row| {
                assert_eq!(row.columns[0].as_deref(), Some(b"0".as_slice()));
                assert_eq!(row.columns[1].as_deref(), Some(b"7".as_slice()));
                Ok(())
            },
        )
        .unwrap();
    if let Ok(directory) = std::env::var("MOOR_PG_PLAN_DIR") {
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            std::path::Path::new(&directory).join("property-delete.json"),
            serde_json::to_vec_pretty(&plans).unwrap(),
        )
        .unwrap();
    }
}
