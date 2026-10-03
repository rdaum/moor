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
    pub max_exports: usize,
    /// Target for admitted encoded payloads and retained append values; one commit is indivisible.
    pub max_pending_bytes: usize,
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
            max_exports: 2,
            max_pending_bytes: 64 * 1024 * 1024,
        }
    }
    pub(crate) fn validate(&self) -> Result<(), PostgresError> {
        self.connection.validate()?;
        if self.max_pending_bytes == 0 {
            return Err(PostgresError::Configuration(
                "max_pending_bytes must be positive",
            ));
        }
        if !(1..=64).contains(&self.max_exports) {
            return Err(PostgresError::Configuration(
                "max_exports must be between 1 and 64",
            ));
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pending_byte_target_is_positive_and_defaults_to_64_mib() {
        let mut config = PostgresStorageConfig::new(
            PostgresConnectOptions::new(
                "",
                super::super::PostgresEndpoint::Tcp("127.0.0.1".parse().unwrap()),
            ),
            PostgresSchema::new("moor").unwrap(),
        );
        assert_eq!(config.max_pending_bytes, 64 * 1024 * 1024);
        config.max_pending_bytes = 0;
        assert!(config.validate().is_err());
        config.max_pending_bytes = 1;
        assert!(config.validate().is_ok());
    }
}
