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

//! Selectable fixtures for shared engine and API semantic contracts.
use crate::{DatabaseConfig, MoorDB, PersistenceConfig, StorageConfig, TxDB};
use std::{ops::Deref, sync::Arc, time::Duration};

/// Every successful test fences its final publication, so async SQL failures fail the test.
pub(crate) struct Checked<T> {
    value: T,
    engine: Arc<MoorDB>,
}
impl<T> Deref for Checked<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.value
    }
}
impl<T> Drop for Checked<T> {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            self.engine
                .wait_for_durability(Duration::from_secs(30))
                .expect("shared contract failed its final durability fence");
        }
    }
}

fn storage() -> StorageConfig {
    match std::env::var("MOOR_DB_TEST_BACKEND").as_deref() {
        Err(std::env::VarError::NotPresent) | Ok("fjall") => StorageConfig::temporary_fjall(),
        #[cfg(feature = "postgres")]
        Ok("postgres") => {
            use crate::{
                PostgresCommitPolicy, PostgresConnectOptions, PostgresEndpoint, PostgresSchema,
                PostgresStorageConfig, initialize_postgres_schema,
            };
            let mut config = PostgresStorageConfig::new(
                PostgresConnectOptions::new(
                    std::env::var("MOOR_PG_TEST_CONNINFO")
                        .expect("use scripts/test-postgres-adapter.sh"),
                    PostgresEndpoint::Tcp("127.0.0.1".parse().unwrap()),
                ),
                PostgresSchema::new(&format!("moor_contract_{}", uuid::Uuid::new_v4().simple()))
                    .unwrap(),
            );
            config.commit_policy = match std::env::var("MOOR_DB_TEST_COMMIT_POLICY").as_deref() {
                Ok("synchronous") => PostgresCommitPolicy::Synchronous,
                Ok("asynchronous") => PostgresCommitPolicy::Asynchronous,
                _ => panic!("select synchronous or asynchronous shared-contract policy"),
            };
            initialize_postgres_schema(&config).unwrap();
            StorageConfig::postgres(config)
        }
        _ => panic!("unsupported shared-contract backend or missing postgres feature"),
    }
}

fn open() -> Arc<MoorDB> {
    MoorDB::try_open(
        storage(),
        DatabaseConfig::default(),
        PersistenceConfig::default(),
    )
    .unwrap()
    .0
}
pub(crate) fn engine() -> Checked<Arc<MoorDB>> {
    let engine = open();
    Checked {
        value: engine.clone(),
        engine,
    }
}
pub(crate) fn api() -> Checked<TxDB> {
    let engine = open();
    Checked {
        value: TxDB {
            storage: engine.clone(),
        },
        engine,
    }
}
