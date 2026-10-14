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

//! Atomic application and bounded progress-based recovery on the writer's libpq session.
use super::{
    PostgresCommitPolicy, PostgresConnection, PostgresError, PostgresParam, PostgresShutdown,
    PostgresStorageConfig,
    codec::invalid,
    encode::{EncodedCommit, RelationBatch},
    metrics::Metrics,
    plan::TransactionPlan,
    schema,
    seed::{self, Chains},
    sql,
    state::{self, Progress},
};
use crate::{
    ObjAndUUIDHolder, PostgresGroupEnd, Timestamp,
    engine::moor_db::Relations,
    provider::{backend::SeededWorld, logical::WriterEpoch},
};
use moor_common::model::WorldStateTimerOp;
use moor_var::Var;
use serde_json::Value;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use uuid::Uuid;

pub(super) struct Session {
    pub metrics: Arc<Metrics>,
    pub group_end: PostgresGroupEnd,
    config: PostgresStorageConfig,
    connection: Option<PostgresConnection>,
    shutdown: PostgresShutdown,
    identity: Uuid,
    pub progress: Progress,
    chains: Chains,
    #[cfg(test)]
    pub failure: Option<FailurePoint>,
}
#[cfg(test)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum FailurePoint {
    BeforeCommit,
    AfterCommit,
}

impl Session {
    /// Open, claim ownership durably, and seed before accepting any publications.
    pub fn open(
        config: PostgresStorageConfig,
        relations: &Relations,
        epoch: WriterEpoch,
        shutdown: PostgresShutdown,
    ) -> Result<(Self, SeededWorld), PostgresError> {
        config.validate()?;
        let mut connection = PostgresConnection::connect(
            &config.connection,
            Instant::now() + config.connect_timeout,
            shutdown.clone(),
        )?;
        let (identity, progress) = state::claim(&mut connection, &config, epoch)?;
        sql::prepare(
            &mut connection,
            &config,
            Instant::now() + config.query_timeout,
        )?;
        let loaded = seed::load(&mut connection, &config, relations, identity, &progress)?;
        let info = connection.info()?;
        tracing::info!(
            database = %info.database,
            schema = config.schema.as_str(),
            user = %info.user,
            host = %info.host,
            port = %info.port,
            server_version = %info.server_version,
            database_id = %loaded.identity,
            commit_policy = match config.commit_policy {
                PostgresCommitPolicy::Synchronous => "synchronous",
                PostgresCommitPolicy::Asynchronous => "asynchronous",
            },
            committed_transactions = loaded.progress.commits,
            "Opened PostgreSQL world database"
        );
        Ok((
            Self {
                metrics: Arc::new(Metrics::default()),
                group_end: PostgresGroupEnd::Available,
                config,
                connection: Some(connection),
                shutdown,
                identity: loaded.identity,
                progress: loaded.progress,
                chains: loaded.chains,
                #[cfg(test)]
                failure: None,
            },
            loaded.seed,
        ))
    }

    #[cfg(test)]
    pub fn apply(
        &mut self,
        commit: &EncodedCommit,
        rollup: impl FnMut(&ObjAndUUIDHolder, Var, Timestamp) -> Result<Value, PostgresError>,
    ) -> Result<(), PostgresError> {
        self.apply_group(std::slice::from_ref(commit), usize::MAX, rollup)
            .map(|_| ())
    }

    /// Apply a consecutive publication range atomically. Retries reuse the exact plan;
    /// tentative chain state becomes visible only after the whole group is confirmed.
    /// Return the confirmed prefix length; the caller retains all remaining commits and permits.
    pub fn apply_group(
        &mut self,
        commits: &[EncodedCommit],
        max_bytes: usize,
        rollup: impl FnMut(&ObjAndUUIDHolder, Var, Timestamp) -> Result<Value, PostgresError>,
    ) -> Result<usize, PostgresError> {
        if commits.is_empty() {
            return Ok(0);
        }
        let plan =
            TransactionPlan::group(&self.progress, &self.chains, commits, max_bytes, rollup)?;
        self.execute(&plan)?;
        let count = plan.commits.len();
        let statements = plan.statements();
        self.metrics.update(|m| {
            let reason = if count < commits.len() {
                PostgresGroupEnd::RollupExpansion
            } else {
                self.group_end
            };
            m.group_end_reasons[reason as usize] += 1;
            m.groups += 1;
            m.group_commits += count as u64;
            m.group_payload_bytes += plan.bytes as u64;
            m.group_sql_statements += statements;
            m.last_group_commits = count as u64;
            m.last_group_payload_bytes = plan.bytes as u64;
            m.last_group_sql_statements = statements;
            m.last_group_first = commits[0].publication.version();
            m.last_group_last = plan.after.applied;
        });
        for (key, chain) in plan.changes {
            match chain {
                Some(chain) => {
                    self.chains.insert(key, chain);
                }
                None => {
                    self.chains.remove(&key);
                }
            }
        }
        self.progress = plan.after;
        Ok(count)
    }

    /// Refresh the schema's physical size on the persistence worker, never on a task worker.
    pub fn storage_bytes(&mut self) -> Result<u64, PostgresError> {
        let connection = self.connection.as_mut().ok_or(PostgresError::Closed)?;
        let mut bytes = None;
        connection.query(
            "SELECT COALESCE(sum(pg_catalog.pg_total_relation_size(c.oid)),0)::text FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=$1 AND c.relkind='r'",
            &[PostgresParam::Text(25, self.config.schema.as_str())],
            Instant::now() + self.config.query_timeout,
            |row| {
                bytes = row.columns.first().and_then(|v| v.as_deref()).and_then(|v| std::str::from_utf8(v).ok()).and_then(|v| v.parse().ok());
                Ok(())
            },
        )?;
        bytes.ok_or_else(|| invalid("storage_bytes", "invalid schema size"))
    }

    pub fn fence(&mut self) -> Result<(), PostgresError> {
        let _timer = self.metrics.timer(WorldStateTimerOp::PostgresFence);
        self.metrics.update(|m| m.fence_calls += 1);
        // A real WAL-producing update, even when no world changes follow an async commit.
        let plan = TransactionPlan::fence(&self.progress)?;
        self.execute(&plan)?;
        self.progress = plan.after;
        Ok(())
    }

    fn execute(&mut self, plan: &TransactionPlan<'_>) -> Result<(), PostgresError> {
        let after = &plan.after;
        let first = self.transaction(plan, Instant::now() + self.config.query_timeout);
        match first {
            Ok(()) => return Ok(()),
            Err(error) if retryable(&error) => {}
            Err(error) => return Err(error),
        }
        // Drop the failed session before trying to acquire its lock again.
        self.connection.take();
        let _timer = self.metrics.timer(WorldStateTimerOp::PostgresRecovery);
        let first = if after.applied > plan.before.applied {
            plan.before.applied + 1
        } else {
            after.applied
        };
        self.metrics.update(|m| {
            m.recovery_first = first;
            m.recovery_last = after.applied;
        });
        tracing::warn!(
            epoch = after.epoch,
            first,
            last = after.applied,
            "Recovering PostgreSQL publication range"
        );
        let deadline = Instant::now() + self.config.recovery_timeout;
        loop {
            self.shutdown.check(deadline)?;
            self.metrics.update(|m| m.recovery_attempts += 1);
            match self.reconnect(deadline) {
                Ok(observed) => {
                    if observed == *after {
                        return Ok(());
                    }
                    if observed != plan.before {
                        return Err(invalid(
                            "writer_progress",
                            "unexpected progress after connection loss",
                        ));
                    }
                    match self.transaction(
                        plan,
                        deadline.min(Instant::now() + self.config.query_timeout),
                    ) {
                        Ok(()) => return Ok(()),
                        Err(error) if retryable(&error) => {}
                        Err(error) => return Err(error),
                    }
                }
                Err(error) if retryable(&error) => {}
                Err(error) => return Err(error),
            }
            self.connection.take();
            let retry_at = deadline.min(Instant::now() + self.config.retry_interval);
            while Instant::now() < retry_at {
                self.shutdown.check(deadline)?;
                std::thread::sleep(
                    retry_at
                        .saturating_duration_since(Instant::now())
                        .min(Duration::from_millis(25)),
                );
            }
        }
    }

    fn reconnect(&mut self, deadline: Instant) -> Result<Progress, PostgresError> {
        let mut connection = PostgresConnection::connect(
            &self.config.connection,
            deadline.min(Instant::now() + self.config.connect_timeout),
            self.shutdown.clone(),
        )?;
        schema::lock_writer(&mut connection, &self.config, deadline)?;
        if state::validate_metadata(&mut connection, &self.config, deadline)? != self.identity {
            return Err(PostgresError::OwnershipLost);
        }
        let progress = state::read_progress(&mut connection, &self.config, deadline)?;
        if progress.epoch != self.progress.epoch {
            return Err(PostgresError::OwnershipLost);
        }
        sql::prepare(&mut connection, &self.config, deadline)?;
        self.connection = Some(connection);
        Ok(progress)
    }

    fn transaction(
        &mut self,
        plan: &TransactionPlan<'_>,
        deadline: Instant,
    ) -> Result<(), PostgresError> {
        let application_timer = self.metrics.timer(WorldStateTimerOp::PostgresApply);
        let connection = self.connection.as_mut().ok_or(PostgresError::Closed)?;
        connection
            .query("BEGIN", &[], deadline, |_| unreachable!())
            .map_err(|source| operation("writer_progress", "BEGIN", source))?;
        let sync =
            if plan.force_sync || self.config.commit_policy == PostgresCommitPolicy::Synchronous {
                "SET LOCAL synchronous_commit=on"
            } else {
                "SET LOCAL synchronous_commit=off"
            };
        connection
            .query(sync, &[], deadline, |_| unreachable!())
            .map_err(|source| operation("writer_progress", "SET synchronous_commit", source))?;
        sql::advance_progress(
            connection,
            &self.config,
            &plan.before,
            &plan.after,
            deadline,
        )?;
        for (commit, properties) in plan.commits.iter().zip(&plan.properties) {
            for relation in &commit.ordinary {
                apply_relation(connection, relation, deadline)?;
            }
            for relation in properties {
                apply_relation(connection, relation, deadline)?;
            }
            if let Some(sequences) = &commit.sequences {
                connection
                    .execute_prepared(
                        "sequence_maxima",
                        &[PostgresParam::Text(3802, sequences)],
                        deadline,
                        |_| unreachable!(),
                    )
                    .map_err(|source| operation("sequences", "sequence_maxima", source))?;
            }
        }
        #[cfg(test)]
        if self.failure == Some(FailurePoint::BeforeCommit) {
            self.failure = None;
            self.connection.take();
            return Err(PostgresError::Connection);
        }
        drop(application_timer);
        let commit_timer = self.metrics.timer(WorldStateTimerOp::PostgresCommit);
        connection
            .query("COMMIT", &[], deadline, |_| unreachable!())
            .map_err(|source| operation("writer_progress", "COMMIT", source))?;
        drop(commit_timer);
        #[cfg(test)]
        if self.failure == Some(FailurePoint::AfterCommit) {
            self.failure = None;
            self.connection.take();
            return Err(PostgresError::Connection);
        }
        Ok(())
    }
}
fn operation(
    relation: &'static str,
    operation: &'static str,
    source: PostgresError,
) -> PostgresError {
    PostgresError::Operation {
        relation,
        operation,
        source: Box::new(source),
    }
}
fn apply_relation(
    connection: &mut PostgresConnection,
    relation: &RelationBatch,
    deadline: Instant,
) -> Result<(), PostgresError> {
    for (operation, rows) in [("delete", &relation.deletes), ("put", &relation.puts)] {
        if let Some(rows) = rows {
            connection
                .execute_prepared(
                    &format!("{operation}_{}", relation.relation),
                    &[PostgresParam::Text(3802, rows)],
                    deadline,
                    |_| unreachable!(),
                )
                .map_err(|source| PostgresError::Operation {
                    relation: relation.relation,
                    operation,
                    source: Box::new(source),
                })?;
        }
    }
    Ok(())
}
fn retryable(error: &PostgresError) -> bool {
    match error {
        PostgresError::Operation { source, .. } => retryable(source),
        PostgresError::Connection | PostgresError::Closed | PostgresError::Timeout => true,
        PostgresError::SqlState(state) => {
            state.starts_with("08")
                || matches!(
                    state.as_str(),
                    "40001" | "40P01" | "57P01" | "57P02" | "57P03" | "53300"
                )
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        initialize_postgres_schema,
        tests::{config, empty, open},
    };
    use super::*;

    #[test]
    #[ignore = "requires PostgreSQL fixture"]
    fn ownership_takeover_prevents_replay_from_an_old_lifetime() {
        let config = config();
        initialize_postgres_schema(&config).unwrap();
        let (mut old, _, epoch) = open(&config);
        old.connection.take();
        let (replacement, _, replacement_epoch) = open(&config);
        assert_ne!(epoch, replacement_epoch);
        drop(replacement);
        let error = old
            .apply(&empty(epoch, 1, 1), |_, _, _| unreachable!())
            .unwrap_err();
        assert_eq!(error, PostgresError::OwnershipLost);
        let (session, _, _) = open(&config);
        assert_eq!(session.progress.commits, 0);
    }

    #[test]
    #[ignore = "requires PostgreSQL fixture"]
    fn unavailable_server_exhausts_one_bounded_recovery_deadline() {
        for policy in [
            PostgresCommitPolicy::Synchronous,
            PostgresCommitPolicy::Asynchronous,
        ] {
            let mut config = config();
            config.commit_policy = policy;
            initialize_postgres_schema(&config).unwrap();
            let (mut session, _, epoch) = open(&config);
            session.connection.take();
            session.config.connection.connection.push_str(" port=1");
            session.config.connect_timeout = Duration::from_millis(50);
            session.config.recovery_timeout = Duration::from_millis(150);
            session.config.retry_interval = Duration::from_millis(5);
            let started = Instant::now();
            let error = session
                .apply(&empty(epoch, 1, 1), |_, _, _| unreachable!())
                .unwrap_err();
            assert_eq!(error, PostgresError::Timeout);
            assert!(started.elapsed() < Duration::from_secs(2));
            assert_eq!(session.progress.applied, 0);
        }
    }

    #[test]
    #[ignore = "requires PostgreSQL fixture"]
    fn counter_exhaustion_fails_before_writing_any_relation() {
        let config = config();
        initialize_postgres_schema(&config).unwrap();
        let (mut session, _, epoch) = open(&config);
        let table = config.schema.qualify("writer_progress").unwrap();
        session
            .connection
            .as_mut()
            .unwrap()
            .query(
                &format!("UPDATE {table} SET property_record_sequence=9223372036854775807"),
                &[],
                Instant::now() + config.query_timeout,
                |_| unreachable!(),
            )
            .unwrap();
        session.progress.property_sequence = i64::MAX;
        let mut commit = empty(epoch, 1, 1);
        commit
            .properties
            .push(super::super::encode::PropertyMutationRow {
                key: ObjAndUUIDHolder::new(&moor_var::Obj::mk_id(1), Uuid::new_v4()),
                mutation: super::super::encode::PropertyMutation::Delete,
            });
        assert!(matches!(
            session.apply(&commit, |_, _, _| unreachable!()),
            Err(PostgresError::Format {
                field: "property_record_sequence",
                ..
            })
        ));
        assert_eq!(
            state::read_progress(
                session.connection.as_mut().unwrap(),
                &config,
                Instant::now() + config.query_timeout
            )
            .unwrap()
            .commits,
            0
        );
        session.progress.commits = u64::MAX;
        assert!(matches!(
            session.apply(&empty(epoch, 1, 1), |_, _, _| unreachable!()),
            Err(PostgresError::Format {
                field: "commit_sequence",
                ..
            })
        ));
    }
}
