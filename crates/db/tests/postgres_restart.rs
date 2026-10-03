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

//! Exclusive fixture tests: PostgreSQL restarts while the mooR writer retains its epoch.
#![cfg(feature = "postgres")]
use moor_common::{
    model::{ObjAttrs, ObjectKind},
    util::BitEnum,
};
use moor_db::{
    Database, DatabaseConfig, PersistenceConfig, PostgresCommitPolicy, PostgresConnectOptions,
    PostgresConnection, PostgresEndpoint, PostgresSchema, PostgresShutdown, PostgresStorageConfig,
    StorageConfig, TxDB, initialize_postgres_schema,
};
use moor_var::{NOTHING, Symbol, v_int, v_list};
use std::{
    process::Command,
    time::{Duration, Instant},
};
use uuid::Uuid;

#[test]
#[ignore = "requires exclusive restarting PostgreSQL fixture"]
fn server_crash_preserves_fenced_progress_and_recovers_pending_appends() {
    let container = std::env::var("MOOR_PG_TEST_CONTAINER_ID").ok();
    let native = std::env::var("MOOR_PG_TEST_NATIVE_DATA")
        .ok()
        .map(|data| (std::env::var("MOOR_PG_TEST_NATIVE_BIN").unwrap(), data));
    assert!(
        container.is_some() || native.is_some(),
        "run scripts/test-postgres-adapter.sh"
    );
    for policy in [
        PostgresCommitPolicy::Synchronous,
        PostgresCommitPolicy::Asynchronous,
    ] {
        let connection = PostgresConnectOptions::new(
            std::env::var("MOOR_PG_TEST_CONNINFO").unwrap(),
            PostgresEndpoint::Tcp("127.0.0.1".parse().unwrap()),
        );
        let mut config = PostgresStorageConfig::new(
            connection,
            PostgresSchema::new(&format!("restart_{}", Uuid::new_v4().simple())).unwrap(),
        );
        config.commit_policy = policy;
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
                &ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "restart"),
            )
            .unwrap();
        let property = Symbol::mk("append");
        loader
            .define_property(
                &object,
                &object,
                property,
                &object,
                BitEnum::new(),
                Some(v_list(&[v_int(1)])),
            )
            .unwrap();
        loader.commit().unwrap();
        let epoch = db.publication().epoch();
        db.wait_for_durability(Duration::from_secs(10)).unwrap();
        // Immediate shutdown skips a clean checkpoint. Both fixtures retain the data directory
        // and database identity. The Docker supervisor restarts the server automatically.
        let stopped = if let Some((bin, data)) = &native {
            Command::new(format!("{bin}/pg_ctl"))
                .args(["-D", data, "-m", "immediate", "-w", "stop"])
                .output()
                .unwrap()
        } else {
            Command::new("docker")
                .args([
                    "exec",
                    "--user",
                    "postgres",
                    container.as_ref().unwrap(),
                    "sh",
                    "-c",
                    "pg_ctl -D \"$PGDATA\" -m immediate -w stop",
                ])
                .output()
                .unwrap()
        };
        assert!(
            stopped.status.success(),
            "{}",
            String::from_utf8_lossy(&stopped.stderr)
        );
        let mut loader = db.loader_client().unwrap();
        let base = loader
            .get_existing_property_value(&object, property)
            .unwrap()
            .unwrap()
            .0;
        let appended = base
            .as_list()
            .unwrap()
            .clone()
            .append_owned(&v_list(&[v_int(2)]))
            .unwrap();
        loader
            .set_property(&object, property, None, None, Some(appended.clone()))
            .unwrap();
        loader.commit().unwrap();
        if let Some((bin, data)) = &native {
            let started = Command::new(format!("{bin}/pg_ctl"))
                .args([
                    "-D",
                    data,
                    "-l",
                    &format!("{data}/restart.log"),
                    "-w",
                    "start",
                ])
                .output()
                .unwrap();
            assert!(
                started.status.success(),
                "{}",
                String::from_utf8_lossy(&started.stderr)
            );
        }
        db.wait_for_durability(Duration::from_secs(20)).unwrap();
        assert_eq!(db.publication().epoch(), epoch);
        assert!(db.persistence_status().healthy);
        let mut connection = PostgresConnection::connect(
            &config.connection,
            Instant::now() + Duration::from_secs(10),
            PostgresShutdown::default(),
        )
        .unwrap();
        connection
            .query(
                &format!(
                    "SELECT string_agg(record_kind,',' ORDER BY record_sequence) FROM {}",
                    config.schema.qualify("object_propvalues").unwrap()
                ),
                &[],
                Instant::now() + Duration::from_secs(10),
                |row| {
                    assert_eq!(
                        row.columns[0].as_deref(),
                        Some(b"full,list_append".as_slice())
                    );
                    Ok(())
                },
            )
            .unwrap();
        drop(connection);
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
            appended
        );
    }
}
