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

//! PostgreSQL world-storage contracts against the disposable TLS fixture.
#![cfg(feature = "postgres")]
use moor_db::{
    PostgresConnectOptions, PostgresConnection, PostgresEndpoint, PostgresError, PostgresParam,
    PostgresSchema, PostgresShutdown, PostgresStorageConfig, initialize_postgres_schema,
};
use std::time::{Duration, Instant};
use uuid::Uuid;

fn config() -> PostgresStorageConfig {
    let connection = PostgresConnectOptions::new(
        std::env::var("MOOR_PG_TEST_CONNINFO").expect("run scripts/test-postgres-adapter.sh"),
        PostgresEndpoint::Tcp("127.0.0.1".parse().unwrap()),
    );
    PostgresStorageConfig::new(
        connection,
        PostgresSchema::new(&format!("moor_{}", Uuid::new_v4().simple())).unwrap(),
    )
}
fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(10)
}
fn connect(config: &PostgresStorageConfig) -> PostgresConnection {
    PostgresConnection::connect(&config.connection, deadline(), PostgresShutdown::default())
        .unwrap()
}

#[test]
#[ignore = "requires PostgreSQL fixture"]
fn initialization_is_explicit_atomic_and_dumpable() {
    let config = config();
    let identity = initialize_postgres_schema(&config).unwrap();
    assert!(
        matches!(initialize_postgres_schema(&config),Err(PostgresError::SqlState(state)) if state=="42P06")
    );
    let mut client = connect(&config);
    client.query("SELECT count(*) FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON c.relnamespace=n.oid WHERE n.nspname=$1 AND c.relkind='r'", &[PostgresParam::Text(25,config.schema.as_str())], deadline(), |row| {
        assert_eq!(row.columns[0].as_deref(),Some(b"16".as_slice())); Ok(())
    }).unwrap();
    let metadata = config.schema.qualify("world_metadata").unwrap();
    client.query(&format!("SELECT database_id, schema_version, literal_format, source_format, compiler_profile, profile->>'language' FROM {metadata}"), &[], deadline(), |row| {
        assert_eq!(row.columns,vec![Some(identity.to_string().into_bytes()),Some(b"1".to_vec()),Some(b"1".to_vec()),Some(b"1".to_vec()),Some(b"moo-v1".to_vec()),Some(b"moo".to_vec())]); Ok(())
    }).unwrap();
    let progress = config.schema.qualify("writer_progress").unwrap();
    client
        .query(
            &format!(
                "SELECT commit_sequence, max_timestamp, property_record_sequence FROM {progress}"
            ),
            &[],
            deadline(),
            |row| {
                assert_eq!(row.columns, vec![Some(b"0".to_vec()); 3]);
                Ok(())
            },
        )
        .unwrap();
}

#[test]
#[ignore = "requires PostgreSQL fixture"]
fn numeric_domains_preserve_rust_ranges_and_reject_overflow() {
    let config = config();
    initialize_postgres_schema(&config).unwrap();
    let name = config.schema.qualify("anonymous_object_metadata").unwrap();
    let max64 = u64::MAX.to_string();
    let max128 = u128::MAX.to_string();
    let mut client = connect(&config);
    client
        .query(
            &format!("INSERT INTO {name} VALUES ('#anon_048D05-1234567890',$1,$2,$2)"),
            &[
                PostgresParam::Text(1700, &max64),
                PostgresParam::Text(1700, &max128),
            ],
            deadline(),
            |_| unreachable!(),
        )
        .unwrap();
    client
        .query(
            &format!("SELECT logical_timestamp, created_micros, last_accessed_micros FROM {name}"),
            &[],
            deadline(),
            |row| {
                assert_eq!(
                    row.columns,
                    vec![
                        Some(max64.as_bytes().to_vec()),
                        Some(max128.as_bytes().to_vec()),
                        Some(max128.as_bytes().to_vec())
                    ]
                );
                Ok(())
            },
        )
        .unwrap();
    for statement in [
        format!("UPDATE {name} SET logical_timestamp=18446744073709551616"),
        format!("UPDATE {name} SET created_micros=340282366920938463463374607431768211456"),
        format!("UPDATE {name} SET logical_timestamp=-1"),
    ] {
        let error = connect(&config)
            .query(&statement, &[], deadline(), |_| unreachable!())
            .unwrap_err();
        assert!(matches!(error,PostgresError::SqlState(state) if state=="23514"));
    }
}
