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

//! PostgreSQL world storage configuration, independent of Fjall table settings.
use super::{PostgresConnectOptions, PostgresError, PostgresSchema};
use moor_compiler::SourceProfile;
use std::time::{Duration, Instant};

/// PostgreSQL's acknowledgement policy for background persistence transactions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PostgresCommitPolicy {
    /// Confirm local WAL durability before advancing the applied watermark.
    #[default]
    Synchronous,
    /// Application can precede local WAL durability; explicit fences remain available.
    Asynchronous,
}

/// Connection and format policy for a single PostgreSQL world schema.
#[derive(Clone, Debug)]
pub struct PostgresStorageConfig {
    pub connection: PostgresConnectOptions,
    pub schema: PostgresSchema,
    pub profile: SourceProfile,
    pub connect_timeout: Duration,
    pub query_timeout: Duration,
    pub recovery_timeout: Duration,
    pub retry_interval: Duration,
    pub commit_policy: PostgresCommitPolicy,
}
impl PostgresStorageConfig {
    pub fn new(connection: PostgresConnectOptions, schema: PostgresSchema) -> Self {
        Self {
            connection,
            schema,
            profile: SourceProfile::default(),
            connect_timeout: Duration::from_secs(10),
            query_timeout: Duration::from_secs(30),
            recovery_timeout: Duration::from_secs(30),
            retry_interval: Duration::from_millis(50),
            commit_policy: PostgresCommitPolicy::default(),
        }
    }
    pub(crate) fn validate(&self) -> Result<(), PostgresError> {
        self.connection.validate()?;
        self.profile
            .validate()
            .map_err(|_| PostgresError::Configuration("unsupported compiler profile"))?;
        for timeout in [
            self.connect_timeout,
            self.query_timeout,
            self.recovery_timeout,
            self.retry_interval,
        ] {
            if timeout.is_zero() || Instant::now().checked_add(timeout).is_none() {
                return Err(PostgresError::Configuration(
                    "PostgreSQL deadlines must be finite positive durations",
                ));
            }
        }
        Ok(())
    }
}
