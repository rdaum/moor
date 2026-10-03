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

//! Serializable storage selection for hosts and command-line tools.
use crate::{DatabaseOpenError, StorageBackendKind, StorageConfig};
use serde::{Deserialize, Serialize};
use std::{net::IpAddr, path::PathBuf};

/// Storage settings are distinct from Fjall's per-table tuning.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StorageSettings {
    pub backend: StorageBackendKind,
    pub postgres: Option<PostgresSettings>,
}

/// PostgreSQL settings remain parseable in builds without libpq support.
/// Credentials and TLS policy belong in the explicitly selected libpq service file.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PostgresSettings {
    pub service: Option<String>,
    pub schema: Option<String>,
    pub hostaddr: Option<IpAddr>,
    pub socket_dir: Option<PathBuf>,
    pub connect_timeout_seconds: Option<u64>,
    pub query_timeout_seconds: Option<u64>,
    pub recovery_timeout_seconds: Option<u64>,
    pub retry_interval_ms: Option<u64>,
    pub max_row_bytes: Option<usize>,
    pub max_exports: Option<usize>,
    pub commit_policy: Option<PostgresCommitSetting>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PostgresCommitSetting {
    Synchronous,
    Asynchronous,
}
impl std::str::FromStr for PostgresCommitSetting {
    type Err = &'static str;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "synchronous" => Ok(Self::Synchronous),
            "asynchronous" => Ok(Self::Asynchronous),
            _ => Err("commit policy must be synchronous or asynchronous"),
        }
    }
}

impl StorageSettings {
    /// Validate before creating directories. Resolve a world path only for Fjall.
    pub fn resolve(
        &self,
        fjall_path: impl FnOnce() -> Option<PathBuf>,
        explicit_db: bool,
        explicit_table_settings: bool,
    ) -> Result<StorageConfig, DatabaseOpenError> {
        self.backend.check_available()?;
        if self.backend == StorageBackendKind::Fjall {
            if self.postgres.is_some() {
                return Err(DatabaseOpenError::StorageConfiguration(
                    "PostgreSQL settings require --storage-backend=postgres",
                ));
            }
            return Ok(StorageConfig::Fjall(crate::FjallStorageConfig {
                path: fjall_path(),
            }));
        }
        if explicit_db {
            return Err(DatabaseOpenError::StorageConfiguration(
                "--db/--db-path selects a Fjall world path and cannot be used with PostgreSQL",
            ));
        }
        if explicit_table_settings {
            return Err(DatabaseOpenError::StorageConfiguration(
                "Fjall database table settings cannot be used with PostgreSQL",
            ));
        }
        #[cfg(feature = "postgres")]
        {
            let settings = self.postgres.as_ref().ok_or(DatabaseOpenError::StorageConfiguration("PostgreSQL requires a service and an explicit numeric address or socket directory"))?;
            settings.resolve().map(StorageConfig::postgres)
        }
        #[cfg(not(feature = "postgres"))]
        Err(DatabaseOpenError::PostgresFeatureDisabled)
    }
}

#[cfg(feature = "postgres")]
impl PostgresSettings {
    fn resolve(&self) -> Result<crate::PostgresStorageConfig, DatabaseOpenError> {
        use crate::{
            PostgresCommitPolicy, PostgresConnectOptions, PostgresEndpoint, PostgresSchema,
            PostgresStorageConfig,
        };
        use std::time::Duration;
        let endpoint = match (&self.hostaddr, &self.socket_dir) {
            (Some(address), None) => PostgresEndpoint::Tcp(*address),
            (None, Some(path)) => PostgresEndpoint::Unix(path.clone()),
            _ => {
                return Err(DatabaseOpenError::StorageConfiguration(
                    "PostgreSQL requires exactly one of pg-hostaddr or pg-socket-dir",
                ));
            }
        };
        let service = self
            .service
            .as_deref()
            .filter(|s| !s.is_empty() && !s.contains('\0'))
            .ok_or(DatabaseOpenError::StorageConfiguration(
                "PostgreSQL requires a nonempty pg-service",
            ))?;
        let quoted_service = service.replace('\\', "\\\\").replace('\'', "\\'");
        let mut connection =
            PostgresConnectOptions::new(format!("service='{quoted_service}'"), endpoint);
        if let Some(limit) = self.max_row_bytes {
            connection.max_row_bytes = limit;
        }
        let mut config = PostgresStorageConfig::new(
            connection,
            PostgresSchema::new(self.schema.as_deref().unwrap_or("moor"))?,
        );
        if let Some(seconds) = self.connect_timeout_seconds {
            config.connect_timeout = Duration::from_secs(seconds);
        }
        if let Some(seconds) = self.query_timeout_seconds {
            config.query_timeout = Duration::from_secs(seconds);
        }
        if let Some(seconds) = self.recovery_timeout_seconds {
            config.recovery_timeout = Duration::from_secs(seconds);
        }
        if let Some(millis) = self.retry_interval_ms {
            config.retry_interval = Duration::from_millis(millis);
        }
        if let Some(limit) = self.max_exports {
            config.max_exports = limit;
        }
        if let Some(policy) = self.commit_policy {
            config.commit_policy = match policy {
                PostgresCommitSetting::Synchronous => PostgresCommitPolicy::Synchronous,
                PostgresCommitSetting::Asynchronous => PostgresCommitPolicy::Asynchronous,
            };
        }
        config.validate()?;
        Ok(config)
    }
}
