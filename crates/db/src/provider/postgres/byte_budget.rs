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

//! Credits for prepared payloads, retained until publication is abandoned or SQL applies them.
use crate::{AdmissionPolicy, provider::coordinator::CommitAdmissionError};
use parking_lot::{Condvar, Mutex};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

pub(super) struct ByteBudget {
    limit: usize,
    state: Mutex<State>,
    changed: Condvar,
}
#[derive(Default)]
struct State {
    used: usize,
    waiters: u64,
    wait_ns: u64,
    timeouts: u64,
    warned: bool,
}
pub(super) struct ByteLease {
    budget: Arc<ByteBudget>,
    bytes: usize,
}
impl Drop for ByteLease {
    fn drop(&mut self) {
        let mut state = self.budget.state.lock();
        state.used -= self.bytes;
        self.budget.changed.notify_all();
    }
}
impl ByteBudget {
    pub fn new(limit: usize) -> Arc<Self> {
        assert!(limit > 0);
        Arc::new(Self {
            limit,
            state: Mutex::default(),
            changed: Condvar::new(),
        })
    }

    pub fn reserve(
        self: &Arc<Self>,
        bytes: usize,
        policy: AdmissionPolicy,
        running: impl Fn() -> bool,
    ) -> Result<ByteLease, CommitAdmissionError> {
        let started = Instant::now();
        let mut state = self.state.lock();
        let fits = |state: &State| {
            // A single indivisible payload can exceed the target while the budget is empty.
            state.used == 0
                || state
                    .used
                    .checked_add(bytes)
                    .is_some_and(|sum| sum <= self.limit)
        };
        if !running() {
            return Err(CommitAdmissionError::Unavailable);
        }
        if fits(&state) {
            state.used += bytes;
            return Ok(ByteLease {
                budget: self.clone(),
                bytes,
            });
        }
        state.waiters += 1;
        let result = loop {
            let waited = started.elapsed();
            if !running() {
                break Err(CommitAdmissionError::Unavailable);
            }
            if waited >= policy.timeout {
                state.timeouts += 1;
                break Err(CommitAdmissionError::Timeout { waited });
            }
            if fits(&state) {
                state.used += bytes;
                break Ok(ByteLease {
                    budget: self.clone(),
                    bytes,
                });
            }
            if waited >= policy.warn_after && !state.warned {
                state.warned = true;
                tracing::warn!(
                    used_bytes = state.used,
                    limit_bytes = self.limit,
                    waiters = state.waiters,
                    ?waited,
                    "PostgreSQL prepared payload byte budget remains full"
                );
            }
            self.changed.wait_for(
                &mut state,
                Duration::from_millis(10).min(policy.timeout - waited),
            );
        };
        state.waiters -= 1;
        state.wait_ns = state
            .wait_ns
            .saturating_add(started.elapsed().as_nanos().min(u64::MAX as u128) as u64);
        if state.waiters == 0 {
            state.warned = false;
        }
        result
    }

    pub fn diagnostics(&self, stats: &mut crate::PostgresPersistenceStats) {
        let state = self.state.lock();
        stats.admission_bytes = state.used as u64;
        stats.admission_limit_bytes = self.limit as u64;
        stats.admission_waiters = state.waiters;
        stats.admission_wait_ns = state.wait_ns;
        stats.admission_timeouts = state.timeouts;
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        PostgresConnection, PostgresShutdown, PostgresStorageConfig, initialize_postgres_schema,
    };
    use super::*;
    use crate::{Database, DatabaseConfig, PersistenceConfig, StorageConfig, TxDB};
    use moor_common::{
        model::{ObjAttrs, ObjectKind},
        util::BitEnum,
    };
    use moor_var::{NOTHING, Obj, Symbol, v_int, v_list, v_str};

    fn fixture(
        count: usize,
        shutdown_timeout: Duration,
    ) -> (TxDB, PostgresStorageConfig, Vec<Obj>, Symbol) {
        let mut config = super::super::tests::config();
        config.max_pending_bytes = 4096;
        initialize_postgres_schema(&config).unwrap();
        let (db, _) = TxDB::try_open(
            StorageConfig::postgres(config.clone()),
            DatabaseConfig::default(),
            PersistenceConfig {
                shutdown_timeout,
                ..Default::default()
            },
        )
        .unwrap();
        let mut loader = db.loader_client().unwrap();
        let property = Symbol::mk("payload");
        let objects = (0..count)
            .map(|_| {
                let object = loader
                    .create_object(
                        ObjectKind::NextObjid,
                        &ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "bytes"),
                    )
                    .unwrap();
                loader
                    .define_property(
                        &object,
                        &object,
                        property,
                        &object,
                        BitEnum::new(),
                        Some(v_list(&[v_str(&"x".repeat(1000))])),
                    )
                    .unwrap();
                object
            })
            .collect();
        loader.commit().unwrap();
        db.wait_for_durability(Duration::from_secs(10)).unwrap();
        (db, config, objects, property)
    }
    fn lock_properties(config: &PostgresStorageConfig) -> PostgresConnection {
        let mut blocker = PostgresConnection::connect(
            &config.connection,
            Instant::now() + config.connect_timeout,
            PostgresShutdown::default(),
        )
        .unwrap();
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
        blocker
    }
    fn wait_for_preparations(db: &TxDB, count: u64) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while db.persistence_status().postgres.unwrap().admission_waiters < count {
            assert!(Instant::now() < deadline, "{:?}", db.persistence_status());
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    fn stats(budget: &ByteBudget) -> crate::PostgresPersistenceStats {
        let mut stats = crate::PostgresPersistenceStats::default();
        budget.diagnostics(&mut stats);
        stats
    }
    fn wait_for_waiters(budget: &ByteBudget) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while stats(budget).admission_waiters == 0 {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    #[test]
    fn released_credit_wakes_waiters_and_oversized_payloads_run_alone() {
        let budget = ByteBudget::new(10);
        let policy = AdmissionPolicy::default();
        let first = budget.reserve(8, policy, || true).unwrap();
        std::thread::scope(|scope| {
            let waiting = scope.spawn(|| budget.reserve(4, policy, || true).unwrap());
            wait_for_waiters(&budget);
            assert_eq!(stats(&budget).admission_bytes, 8);
            drop(first);
            let next = waiting.join().unwrap();
            assert_eq!(stats(&budget).admission_bytes, 4);
            drop(next);
        });
        let oversized = budget.reserve(20, policy, || true).unwrap();
        let immediate = AdmissionPolicy {
            timeout: Duration::ZERO,
            ..policy
        };
        assert!(matches!(
            budget.reserve(1, immediate, || true),
            Err(CommitAdmissionError::Timeout { .. })
        ));
        assert_eq!(stats(&budget).admission_bytes, 20);
        assert_eq!(stats(&budget).admission_timeouts, 1);
        drop(oversized);
        assert_eq!(stats(&budget).admission_bytes, 0);
    }
    #[test]
    fn cancellation_releases_waiter_state_without_taking_credit() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let budget = ByteBudget::new(10);
        let policy = AdmissionPolicy::default();
        let first = budget.reserve(10, policy, || true).unwrap();
        let running = AtomicBool::new(true);
        std::thread::scope(|scope| {
            let waiting =
                scope.spawn(|| budget.reserve(1, policy, || running.load(Ordering::Acquire)));
            wait_for_waiters(&budget);
            running.store(false, Ordering::Release);
            assert!(matches!(
                waiting.join().unwrap(),
                Err(CommitAdmissionError::Unavailable)
            ));
        });
        assert_eq!(stats(&budget).admission_waiters, 0);
        assert_eq!(stats(&budget).admission_bytes, 10);
        drop(first);
        assert_eq!(stats(&budget).admission_bytes, 0);
    }

    #[test]
    #[ignore = "requires PostgreSQL fixture"]
    fn timeout_precedes_publication_and_drain_releases_credit() {
        use super::super::{
            PostgresCommitPolicy, PostgresConnection, PostgresShutdown, initialize_postgres_schema,
        };
        use crate::{Database, DatabaseConfig, PersistenceConfig, StorageConfig, TxDB};
        use moor_common::{
            model::{ObjAttrs, ObjectKind, WorldStateError},
            util::BitEnum,
        };
        use moor_var::{NOTHING, Symbol, v_str};
        let mut config = super::super::tests::config();
        config.max_pending_bytes = 4096;
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
                &ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "bytes"),
            )
            .unwrap();
        let property = Symbol::mk("payload");
        loader
            .define_property(
                &object,
                &object,
                property,
                &object,
                BitEnum::new(),
                Some(v_str("")),
            )
            .unwrap();
        loader.commit().unwrap();
        db.wait_for_durability(Duration::from_secs(10)).unwrap();
        let mut blocker = PostgresConnection::connect(
            &config.connection,
            Instant::now() + config.connect_timeout,
            PostgresShutdown::default(),
        )
        .unwrap();
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
        loader
            .set_property(
                &object,
                property,
                None,
                None,
                Some(v_str(&"x".repeat(2000))),
            )
            .unwrap();
        loader.commit().unwrap();
        let before = db.publication();
        let admitted = db.persistence_status().postgres.unwrap().admission_bytes;
        assert!((2000..=4096).contains(&admitted));
        db.set_commit_queue_policy(Duration::from_secs(1), Duration::from_millis(20));
        let mut loader = db.loader_client().unwrap();
        loader
            .set_property(
                &object,
                property,
                None,
                None,
                Some(v_str(&"y".repeat(2000))),
            )
            .unwrap();
        assert!(matches!(
            loader.commit(),
            Err(WorldStateError::DatabaseOverloaded(_))
        ));
        assert_eq!(db.publication(), before);
        let status = db.persistence_status();
        assert!(status.healthy);
        assert!(status.outstanding < 1000);
        let stats = status.postgres.unwrap();
        assert_eq!(stats.admission_bytes, admitted);
        assert_eq!(stats.admission_timeouts, 1);
        assert_eq!(stats.admission_waiters, 0);
        blocker
            .query(
                "COMMIT",
                &[],
                Instant::now() + config.query_timeout,
                |_| unreachable!(),
            )
            .unwrap();
        db.wait_for_durability(Duration::from_secs(10)).unwrap();
        assert_eq!(db.persistence_status().postgres.unwrap().admission_bytes, 0);
        // A valid indivisible write larger than the target can proceed when credit is empty.
        let mut loader = db.loader_client().unwrap();
        let expected = v_str(&"z".repeat(10000));
        loader
            .set_property(&object, property, None, None, Some(expected.clone()))
            .unwrap();
        loader.commit().unwrap();
        db.wait_for_durability(Duration::from_secs(10)).unwrap();
        assert_eq!(db.persistence_status().postgres.unwrap().admission_bytes, 0);
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
    fn occupied_encoders_do_not_block_submission_or_rollups() {
        let (db, config, objects, property) = fixture(9, Duration::from_secs(10));
        db.set_commit_queue_policy(Duration::from_secs(10), Duration::from_secs(10));
        let mut values = vec![v_list(&[v_str(&"x".repeat(1000))]); objects.len()];
        for n in 1..crate::provider::property_value_store::PROPERTY_VALUE_CHAIN_LIMITS.max_records {
            let mut loader = db.loader_client().unwrap();
            for (object, value) in objects.iter().zip(&mut values) {
                *value = value
                    .as_list()
                    .unwrap()
                    .clone()
                    .append_owned(&v_list(&[v_int(n as i64)]))
                    .unwrap();
                loader
                    .set_property(object, property, None, None, Some(value.clone()))
                    .unwrap();
            }
            loader.commit().unwrap();
        }
        db.wait_for_durability(Duration::from_secs(10)).unwrap();
        let mut blocker = lock_properties(&config);
        for value in &mut values {
            *value = value
                .as_list()
                .unwrap()
                .clone()
                .append_owned(&v_list(&[v_int(999)]))
                .unwrap();
        }
        let mut loader = db.loader_client().unwrap();
        loader
            .set_property(&objects[0], property, None, None, Some(values[0].clone()))
            .unwrap();
        loader.commit().unwrap();
        std::thread::scope(|scope| {
            let mut waiting = Vec::new();
            for (object, value) in objects[1..].iter().zip(&values[1..]) {
                let db = &db;
                waiting.push(scope.spawn(move || {
                    let mut loader = db.loader_client().unwrap();
                    loader
                        .set_property(object, property, None, None, Some(value.clone()))
                        .unwrap();
                    loader.commit().unwrap();
                }));
            }
            let encoders = std::thread::available_parallelism().map_or(1, |n| n.get().min(8));
            wait_for_preparations(&db, encoders as u64);
            let status = db.persistence_status().postgres.unwrap();
            assert_eq!(status.prepared_commits, encoders as u64);
            assert_eq!(status.unapplied_commits, 1);
            blocker
                .query(
                    "COMMIT",
                    &[],
                    Instant::now() + config.query_timeout,
                    |_| unreachable!(),
                )
                .unwrap();
            for waiting in waiting {
                waiting.join().unwrap();
            }
        });
        db.wait_for_durability(Duration::from_secs(10)).unwrap();
        assert_eq!(db.persistence_status().postgres.unwrap().admission_bytes, 0);
        drop(db);
        let (db, _) = TxDB::try_open(
            StorageConfig::postgres(config),
            DatabaseConfig::default(),
            PersistenceConfig::default(),
        )
        .unwrap();
        let loader = db.loader_client().unwrap();
        for (object, value) in objects.iter().zip(values) {
            assert_eq!(
                loader
                    .get_existing_property_value(object, property)
                    .unwrap()
                    .unwrap()
                    .0,
                value
            );
        }
    }

    #[test]
    #[ignore = "requires PostgreSQL fixture"]
    fn writer_failure_and_shutdown_cancel_byte_waits() {
        for shutdown in [false, true] {
            let (db, config, objects, property) = fixture(2, Duration::from_millis(100));
            let mut blocker = lock_properties(&config);
            let mut loader = db.loader_client().unwrap();
            loader
                .set_property(
                    &objects[0],
                    property,
                    None,
                    None,
                    Some(v_str(&"x".repeat(2000))),
                )
                .unwrap();
            loader.commit().unwrap();
            std::thread::scope(|scope| {
                let waiting = scope.spawn(|| {
                    let mut loader = db.loader_client().unwrap();
                    loader
                        .set_property(
                            &objects[1],
                            property,
                            None,
                            None,
                            Some(v_str(&"y".repeat(2000))),
                        )
                        .unwrap();
                    loader.commit()
                });
                wait_for_preparations(&db, 1);
                if shutdown {
                    let started = Instant::now();
                    assert!(db.storage.stop().is_err());
                    assert!(started.elapsed() < Duration::from_secs(2));
                } else {
                    blocker.query(&format!("ALTER TABLE {} ADD CONSTRAINT reject_writes CHECK (false) NOT VALID", config.schema.qualify("object_propvalues").unwrap()), &[], Instant::now()+config.query_timeout, |_| unreachable!()).unwrap();
                }
                blocker
                    .query(
                        "COMMIT",
                        &[],
                        Instant::now() + config.query_timeout,
                        |_| unreachable!(),
                    )
                    .unwrap();
                assert!(waiting.join().unwrap().is_err());
            });
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                let status = db.persistence_status().postgres.unwrap();
                if status.admission_bytes == 0
                    && status.prepared_commits == 0
                    && status.admission_waiters == 0
                {
                    break;
                }
                assert!(Instant::now() < deadline, "{status:?}");
                std::thread::sleep(Duration::from_millis(1));
            }
            assert!(!db.persistence_status().healthy);
        }
    }
}
