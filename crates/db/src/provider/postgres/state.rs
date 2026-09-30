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

//! Dumpable progress and writer ownership. Every connection stays on its creating thread.
use super::{
    PostgresConnection, PostgresError, PostgresParam, PostgresStorageConfig,
    codec::{self, invalid},
    rows, schema,
};
use crate::provider::logical::WriterEpoch;
use moor_compiler::{PERSISTENT_LITERAL_VERSION, PERSISTENT_SOURCE_VERSION};
use serde_json::Value;
use std::time::Instant;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Progress {
    pub epoch: u64,
    pub applied: u64,
    pub commits: u64,
    pub max_timestamp: u64,
    pub property_sequence: i64,
    pub durable_fence: u64,
}

/// Require exactly one metadata row; an empty or damaged schema is never a fresh world.
pub(super) fn singleton(
    connection: &mut PostgresConnection,
    sql: &str,
    params: &[PostgresParam<'_>],
    deadline: Instant,
) -> Result<rows::Row, PostgresError> {
    let mut value = None;
    connection.query(sql, params, deadline, |row| {
        if value.is_some() {
            return Err(invalid("metadata", "multiple singleton rows"));
        }
        let Value::Object(row) = rows::parse_row(row)? else {
            return Err(invalid("metadata", "expected JSON object"));
        };
        value = Some(row);
        Ok(())
    })?;
    value.ok_or_else(|| invalid("metadata", "missing singleton row"))
}

pub(super) fn validate_metadata(
    connection: &mut PostgresConnection,
    config: &PostgresStorageConfig,
    deadline: Instant,
) -> Result<Uuid, PostgresError> {
    let table = config.schema.qualify("world_metadata")?;
    let row = singleton(
        connection,
        &format!("SELECT to_jsonb(t)::text FROM {table} t"),
        &[],
        deadline,
    )?;
    if row.get("singleton") != Some(&Value::Bool(true))
        || rows::number::<u32>(&row, "schema_version")? != schema::SCHEMA_VERSION
        || rows::number::<u32>(&row, "literal_format")? != PERSISTENT_LITERAL_VERSION
        || rows::number::<u32>(&row, "source_format")? != PERSISTENT_SOURCE_VERSION
        || rows::text(&row, "compiler_profile")? != schema::PROFILE_ID
        || row.get("profile") != Some(&codec::profile_json(&config.profile)?)
    {
        return Err(invalid(
            "world_metadata",
            "unsupported schema or source profile",
        ));
    }
    Uuid::parse_str(rows::text(&row, "database_id")?)
        .map_err(|_| invalid("database_id", "invalid database identity"))
}

pub(super) fn read_progress(
    connection: &mut PostgresConnection,
    config: &PostgresStorageConfig,
    deadline: Instant,
) -> Result<Progress, PostgresError> {
    let table = config.schema.qualify("writer_progress")?;
    let row = singleton(
        connection,
        &format!(
            "SELECT jsonb_build_object('writer_epoch',writer_epoch::text,'applied_version',applied_version::text,'commit_sequence',commit_sequence::text,'max_timestamp',max_timestamp::text,'property_record_sequence',property_record_sequence::text,'durable_fence',durable_fence::text,'singleton',singleton)::text FROM {table}"
        ),
        &[],
        deadline,
    )?;
    let result = Progress {
        epoch: rows::number(&row, "writer_epoch")?,
        applied: rows::number(&row, "applied_version")?,
        commits: rows::number(&row, "commit_sequence")?,
        max_timestamp: rows::number(&row, "max_timestamp")?,
        property_sequence: rows::number(&row, "property_record_sequence")?,
        durable_fence: rows::number(&row, "durable_fence")?,
    };
    if row.get("singleton") != Some(&Value::Bool(true))
        || result.property_sequence < 0
        || result.applied > result.commits
    {
        return Err(invalid("writer_progress", "inconsistent progress counters"));
    }
    Ok(result)
}

/// Claim a new lifetime durably before the runtime may publish anything.
/// Recovery must instead reacquire the lock and verify the existing epoch.
pub(super) fn claim(
    connection: &mut PostgresConnection,
    config: &PostgresStorageConfig,
    epoch: WriterEpoch,
) -> Result<(Uuid, Progress), PostgresError> {
    let deadline = Instant::now() + config.query_timeout;
    schema::lock_writer(connection, config, deadline)?;
    connection.query("BEGIN", &[], deadline, |_| unreachable!())?;
    connection.query(
        "SET LOCAL synchronous_commit=on",
        &[],
        deadline,
        |_| unreachable!(),
    )?;
    let identity = validate_metadata(connection, config, deadline)?;
    let mut progress = read_progress(connection, config, deadline)?;
    let table = config.schema.qualify("writer_progress")?;
    let new_epoch = epoch.as_u64().to_string();
    let old_epoch = progress.epoch.to_string();
    let old_version = progress.applied.to_string();
    let result = connection.query(&format!("UPDATE {table} SET writer_epoch=$1, applied_version=0 WHERE singleton AND writer_epoch=$2 AND applied_version=$3"), &[
        PostgresParam::Text(1700, &new_epoch), PostgresParam::Text(1700, &old_epoch), PostgresParam::Text(1700, &old_version),
    ], deadline, |_| unreachable!())?;
    if result.affected_rows != Some(1) {
        return Err(PostgresError::OwnershipLost);
    }
    connection.query("COMMIT", &[], deadline, |_| unreachable!())?;
    progress.epoch = epoch.as_u64();
    progress.applied = 0;
    Ok((identity, progress))
}
