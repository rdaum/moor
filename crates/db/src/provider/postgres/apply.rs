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
    encode::{EncodedCommit, PropertyMutation, RelationBatch},
    rows::{self, RowKey},
    schema,
    seed::{self, Chains},
    sql,
    state::{self, Progress},
};
use crate::{
    ObjAndUUIDHolder, Timestamp,
    engine::moor_db::Relations,
    provider::{
        backend::SeededWorld,
        logical::WriterEpoch,
        property_value_store::{PROPERTY_VALUE_CHAIN_LIMITS, PropertyValueChain},
    },
};
use moor_var::Var;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use uuid::Uuid;

pub(super) struct Session {
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

type ChainChanges = ahash::AHashMap<ObjAndUUIDHolder, Option<PropertyValueChain>>;
struct PropertyPlan {
    batch: RelationBatch,
    changes: Vec<(ObjAndUUIDHolder, Option<PropertyValueChain>)>,
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
        mut rollup: impl FnMut(&ObjAndUUIDHolder, Var, Timestamp) -> Result<Value, PostgresError>,
    ) -> Result<usize, PostgresError> {
        if commits.is_empty() {
            return Ok(0);
        }
        let mut after = self.progress.clone();
        let mut changes = ChainChanges::default();
        let mut properties = Vec::with_capacity(commits.len());
        let mut bytes = 0usize;
        for commit in commits {
            let before = after.clone();
            if commit.publication.epoch().as_u64() != after.epoch {
                return Err(PostgresError::OwnershipLost);
            }
            after.applied = after
                .applied
                .checked_add(1)
                .ok_or_else(|| invalid("applied_version", "counter exhausted"))?;
            if after.applied != commit.publication.version() {
                return Err(invalid("applied_version", "nonconsecutive publication"));
            }
            after.commits = after
                .commits
                .checked_add(1)
                .ok_or_else(|| invalid("commit_sequence", "counter exhausted"))?;
            after.max_timestamp = after.max_timestamp.max(commit.timestamp.0);
            if !commit.properties.is_empty() {
                after.property_sequence = after
                    .property_sequence
                    .checked_add(1)
                    .ok_or_else(|| invalid("property_record_sequence", "counter exhausted"))?;
            }
            let plan =
                self.plan_properties(commit, after.property_sequence, &changes, &mut rollup)?;
            let member_bytes = commit
                .ordinary
                .iter()
                .map(RelationBatch::encoded_bytes)
                .sum::<usize>()
                + plan.batch.encoded_bytes()
                + commit.sequences.as_ref().map_or(0, String::len);
            // A rollup can expand a small suffix into a large full value. Seal before
            // that member; one indivisible commit may exceed the payload limit.
            if !properties.is_empty() && bytes.saturating_add(member_bytes) > max_bytes {
                after = before;
                break;
            }
            bytes = bytes.saturating_add(member_bytes);
            changes.extend(plan.changes);
            properties.push(plan.batch);
            if bytes >= max_bytes {
                break;
            }
        }
        self.execute(&after, false, |connection, deadline| {
            for (commit, properties) in commits.iter().zip(&properties) {
                for relation in &commit.ordinary {
                    apply_relation(connection, relation, deadline)?;
                }
                apply_relation(connection, properties, deadline)?;
                if let Some(sequences) = &commit.sequences {
                    connection.execute_prepared(
                        "sequence_maxima",
                        &[PostgresParam::Text(3802, sequences)],
                        deadline,
                        |_| unreachable!(),
                    )?;
                }
            }
            Ok(())
        })?;
        for (key, chain) in changes {
            match chain {
                Some(chain) => {
                    self.chains.insert(key, chain);
                }
                None => {
                    self.chains.remove(&key);
                }
            }
        }
        self.progress = after;
        Ok(properties.len())
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
        let mut after = self.progress.clone();
        after.durable_fence = after
            .durable_fence
            .checked_add(1)
            .ok_or_else(|| invalid("durable_fence", "counter exhausted"))?;
        // A real WAL-producing update, even when no world changes follow an async commit.
        self.execute(&after, true, |_, _| Ok(()))?;
        self.progress = after;
        Ok(())
    }

    fn plan_properties(
        &self,
        commit: &EncodedCommit,
        sequence: i64,
        overlay: &ChainChanges,
        rollup: &mut impl FnMut(&ObjAndUUIDHolder, Var, Timestamp) -> Result<Value, PostgresError>,
    ) -> Result<PropertyPlan, PostgresError> {
        let mut puts = Vec::new();
        let mut deletes = Vec::new();
        let mut changes = Vec::with_capacity(commit.properties.len());
        for property in &commit.properties {
            let key = &property.key;
            let (mut row, chain, replace) = match &property.mutation {
                PropertyMutation::Delete => {
                    deletes.push(Value::Object(key.encode_key("object_propvalues")));
                    changes.push((key.clone(), None));
                    continue;
                }
                PropertyMutation::Full(row) => {
                    (row.clone(), PropertyValueChain::full(sequence as u64), true)
                }
                PropertyMutation::Append { row, final_value } => {
                    let bytes = rows::text(
                        row.as_object()
                            .ok_or_else(|| invalid("row", "invalid property row"))?,
                        "value_literal",
                    )?
                    .len();
                    let previous = match overlay.get(key) {
                        Some(chain) => chain.as_ref(),
                        None => self.chains.get(key),
                    };
                    if previous
                        .is_none_or(|chain| chain.reaches_limit(bytes, PROPERTY_VALUE_CHAIN_LIMITS))
                    {
                        (
                            rollup(key, final_value.clone(), commit.timestamp)?,
                            PropertyValueChain::full(sequence as u64),
                            true,
                        )
                    } else {
                        let mut chain = previous.unwrap().clone();
                        chain.push_append(sequence as u64, bytes);
                        (row.clone(), chain, false)
                    }
                }
            };
            row["record_sequence"] = json!(sequence);
            puts.push(row);
            if replace {
                deletes.push(Value::Object(key.encode_key("object_propvalues")));
            }
            changes.push((key.clone(), Some(chain)));
        }
        Ok(PropertyPlan {
            batch: RelationBatch {
                relation: "object_propvalues",
                puts: array(puts),
                deletes: array(deletes),
            },
            changes,
        })
    }

    fn execute(
        &mut self,
        after: &Progress,
        force_sync: bool,
        apply: impl Fn(&mut PostgresConnection, Instant) -> Result<(), PostgresError>,
    ) -> Result<(), PostgresError> {
        let first = self.transaction(
            after,
            force_sync,
            Instant::now() + self.config.query_timeout,
            &apply,
        );
        match first {
            Ok(()) => return Ok(()),
            Err(error) if retryable(&error) => {}
            Err(error) => return Err(error),
        }
        // Drop the failed session before trying to acquire its lock again.
        self.connection.take();
        let deadline = Instant::now() + self.config.recovery_timeout;
        loop {
            self.shutdown.check(deadline)?;
            match self.reconnect(deadline) {
                Ok(observed) => {
                    if observed == *after {
                        return Ok(());
                    }
                    if observed != self.progress {
                        return Err(invalid(
                            "writer_progress",
                            "unexpected progress after connection loss",
                        ));
                    }
                    match self.transaction(
                        after,
                        force_sync,
                        deadline.min(Instant::now() + self.config.query_timeout),
                        &apply,
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
        after: &Progress,
        force_sync: bool,
        deadline: Instant,
        apply: &impl Fn(&mut PostgresConnection, Instant) -> Result<(), PostgresError>,
    ) -> Result<(), PostgresError> {
        let connection = self.connection.as_mut().ok_or(PostgresError::Closed)?;
        connection.query("BEGIN", &[], deadline, |_| unreachable!())?;
        let sync = if force_sync || self.config.commit_policy == PostgresCommitPolicy::Synchronous {
            "SET LOCAL synchronous_commit=on"
        } else {
            "SET LOCAL synchronous_commit=off"
        };
        connection.query(sync, &[], deadline, |_| unreachable!())?;
        let table = self.config.schema.qualify("writer_progress")?;
        let before = &self.progress;
        let numbers = [
            after.epoch,
            after.applied,
            after.commits,
            after.max_timestamp,
            after.durable_fence,
            before.applied,
            before.commits,
            before.max_timestamp,
            before.durable_fence,
        ];
        let mut values: Vec<_> = numbers.iter().map(ToString::to_string).collect();
        values.push(after.property_sequence.to_string());
        values.push(before.property_sequence.to_string());
        let params: Vec<_> = values
            .iter()
            .map(|value| PostgresParam::Text(1700, value))
            .collect();
        let result = connection.query(&format!("UPDATE {table} SET applied_version=$2,commit_sequence=$3,max_timestamp=GREATEST(max_timestamp,$4),durable_fence=$5,property_record_sequence=$10::bigint WHERE singleton AND writer_epoch=$1 AND applied_version=$6 AND commit_sequence=$7 AND max_timestamp=$8 AND durable_fence=$9 AND property_record_sequence=$11::bigint"), &params, deadline, |_| unreachable!())?;
        if result.affected_rows != Some(1) {
            return Err(PostgresError::OwnershipLost);
        }
        apply(connection, deadline)?;
        #[cfg(test)]
        if self.failure == Some(FailurePoint::BeforeCommit) {
            self.failure = None;
            self.connection.take();
            return Err(PostgresError::Connection);
        }
        connection.query("COMMIT", &[], deadline, |_| unreachable!())?;
        #[cfg(test)]
        if self.failure == Some(FailurePoint::AfterCommit) {
            self.failure = None;
            self.connection.take();
            return Err(PostgresError::Connection);
        }
        Ok(())
    }
}
fn array(rows: Vec<Value>) -> Option<String> {
    if rows.is_empty() {
        return None;
    }
    Some(Value::Array(rows).to_string())
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
        let config = config();
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
                mutation: PropertyMutation::Delete,
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
