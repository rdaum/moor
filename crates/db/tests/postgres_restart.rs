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

fn crash_config(schema: &str, policy: PostgresCommitPolicy) -> PostgresStorageConfig {
    let mut config = PostgresStorageConfig::new(
        PostgresConnectOptions::new(
            std::env::var("MOOR_PG_TEST_CONNINFO").unwrap(),
            PostgresEndpoint::Tcp("127.0.0.1".parse().unwrap()),
        ),
        PostgresSchema::new(schema).unwrap(),
    );
    config.commit_policy = policy;
    config
}
fn crash_open(config: &PostgresStorageConfig) -> TxDB {
    TxDB::try_open(
        StorageConfig::postgres(config.clone()),
        DatabaseConfig::default(),
        PersistenceConfig::default(),
    )
    .unwrap()
    .0
}
fn wait_marker(path: &std::path::Path) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !path.exists() {
        assert!(
            Instant::now() < deadline,
            "process fixture marker timed out: {}",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Kill a real mooR host process after publication, while its SQL transaction is blocked.
#[test]
#[ignore = "requires PostgreSQL fixture and subprocesses"]
fn process_crash_keeps_fenced_prefix_and_discards_inflight_transaction() {
    use moor_db::PostgresParam;
    use moor_var::Obj;
    use std::{fs, process::Stdio};

    if let Ok(directory) = std::env::var("MOOR_PG_CRASH_CHILD") {
        let directory = std::path::Path::new(&directory);
        let policy = match std::env::var("MOOR_PG_CRASH_POLICY").unwrap().as_str() {
            "synchronous" => PostgresCommitPolicy::Synchronous,
            "asynchronous" => PostgresCommitPolicy::Asynchronous,
            _ => unreachable!(),
        };
        let config = crash_config(&std::env::var("MOOR_PG_CRASH_SCHEMA").unwrap(), policy);
        let db = crash_open(&config);
        let mut loader = db.loader_client().unwrap();
        let object = loader
            .create_object(
                ObjectKind::NextObjid,
                &ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "process crash"),
            )
            .unwrap();
        assert_eq!(object, Obj::mk_id(0));
        loader
            .define_property(
                &object,
                &object,
                Symbol::mk("append"),
                &object,
                BitEnum::new(),
                Some(v_list(&[v_int(1)])),
            )
            .unwrap();
        loader.commit().unwrap();
        db.wait_for_durability(Duration::from_secs(10)).unwrap();
        fs::write(directory.join("fenced"), b"ready").unwrap();
        wait_marker(&directory.join("resume"));
        let mut loader = db.loader_client().unwrap();
        loader
            .set_property(
                &object,
                Symbol::mk("append"),
                None,
                None,
                Some(v_list(&[v_int(1), v_int(2)])),
            )
            .unwrap();
        loader.commit().unwrap();
        fs::write(directory.join("published"), b"ready").unwrap();
        loop {
            std::thread::park_timeout(Duration::from_secs(1));
        }
    }

    struct KillOnDrop(std::process::Child);
    impl Drop for KillOnDrop {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    for (policy, label) in [
        (PostgresCommitPolicy::Synchronous, "synchronous"),
        (PostgresCommitPolicy::Asynchronous, "asynchronous"),
    ] {
        let config = crash_config(&format!("process_{}", Uuid::new_v4().simple()), policy);
        initialize_postgres_schema(&config).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let log = fs::File::create(directory.path().join("child.log")).unwrap();
        let mut child = KillOnDrop(
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "process_crash_keeps_fenced_prefix_and_discards_inflight_transaction",
                    "--ignored",
                    "--nocapture",
                ])
                .env("MOOR_PG_CRASH_CHILD", directory.path())
                .env("MOOR_PG_CRASH_SCHEMA", config.schema.as_str())
                .env("MOOR_PG_CRASH_POLICY", label)
                .stdout(Stdio::from(log.try_clone().unwrap()))
                .stderr(Stdio::from(log))
                .spawn()
                .unwrap(),
        );
        wait_marker(&directory.path().join("fenced"));
        let mut blocker = PostgresConnection::connect(
            &config.connection,
            Instant::now() + Duration::from_secs(10),
            PostgresShutdown::default(),
        )
        .unwrap();
        let table = config.schema.qualify("object_propvalues").unwrap();
        blocker
            .query(
                "BEGIN",
                &[],
                Instant::now() + Duration::from_secs(10),
                |_| unreachable!(),
            )
            .unwrap();
        blocker
            .query(
                &format!("LOCK TABLE {table} IN ACCESS EXCLUSIVE MODE"),
                &[],
                Instant::now() + Duration::from_secs(10),
                |_| unreachable!(),
            )
            .unwrap();
        fs::write(directory.path().join("resume"), b"ready").unwrap();
        wait_marker(&directory.path().join("published"));
        // Observe the writer inside SQL, after its progress update and before COMMIT.
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            blocker
                .query(
                    "SELECT pg_catalog.pg_stat_clear_snapshot()",
                    &[],
                    deadline,
                    |_| Ok(()),
                )
                .unwrap();
            let mut blocked = false;
            blocker.query("SELECT EXISTS(SELECT 1 FROM pg_catalog.pg_stat_activity WHERE pid<>pg_backend_pid() AND wait_event_type='Lock' AND query LIKE $1)",
                &[PostgresParam::Text(25, &format!("%{table}%"))], deadline,
                |row| { blocked = row.columns[0].as_deref() == Some(b"t"); Ok(()) }).unwrap();
            if blocked {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "writer never reached blocked SQL"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        child.0.kill().unwrap();
        assert!(!child.0.wait().unwrap().success());
        drop(blocker);
        // Wait for PostgreSQL to notice the dead client and release its ownership lock.
        let deadline = Instant::now() + Duration::from_secs(10);
        let db = loop {
            match TxDB::try_open(
                StorageConfig::postgres(config.clone()),
                DatabaseConfig::default(),
                PersistenceConfig::default(),
            ) {
                Ok((db, _)) => break db,
                Err(error) => {
                    assert!(
                        Instant::now() < deadline,
                        "reopen after process crash failed: {error}"
                    );
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        };
        let mut loader = db.loader_client().unwrap();
        assert_eq!(
            loader
                .get_existing_property_value(&Obj::mk_id(0), Symbol::mk("append"))
                .unwrap()
                .unwrap()
                .0,
            v_list(&[v_int(1)])
        );
        loader
            .set_property(
                &Obj::mk_id(0),
                Symbol::mk("append"),
                None,
                None,
                Some(v_list(&[v_int(1), v_int(3)])),
            )
            .unwrap();
        loader.commit().unwrap();
        db.wait_for_durability(Duration::from_secs(10)).unwrap();
        drop(db);
        let db = crash_open(&config);
        assert_eq!(
            db.loader_client()
                .unwrap()
                .get_existing_property_value(&Obj::mk_id(0), Symbol::mk("append"))
                .unwrap()
                .unwrap()
                .0,
            v_list(&[v_int(1), v_int(3)])
        );
    }
}
