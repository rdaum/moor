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

//! Shared storage arguments for daemon, server, import, and maintenance binaries.
use crate::{
    DatabaseOpenError, PostgresCommitSetting, StorageBackendKind, StorageConfig, StorageSettings,
};
use serde::{Deserialize, Serialize};
use std::{net::IpAddr, path::PathBuf};

#[derive(Clone, Debug, Default, clap_derive::Args, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StorageArgs {
    /// World storage backend: fjall or postgres (default: fjall).
    #[arg(long)]
    pub storage_backend: Option<StorageBackendKind>,
    /// Persistence drain and worker-stop budget in seconds (default: 30; both backends).
    #[arg(long)]
    pub persistence_shutdown_timeout_seconds: Option<u64>,
    /// Initialize a new PostgreSQL schema and exit.
    #[arg(long)]
    pub init_storage: bool,
    /// Validate PostgreSQL storage without claiming writer ownership, then exit.
    #[arg(long, conflicts_with_all = ["init_storage", "install_storage_views"])]
    pub validate_storage: bool,
    /// Install PostgreSQL inspection views in an existing schema, then exit.
    #[arg(long, conflicts_with = "init_storage")]
    pub install_storage_views: bool,
    /// Service name in the explicit PGSERVICEFILE.
    #[arg(long)]
    pub pg_service: Option<String>,
    /// PostgreSQL world schema (default: moor).
    #[arg(long)]
    pub pg_schema: Option<String>,
    /// Numeric server address; service host remains available for TLS verification.
    #[arg(long, conflicts_with = "pg_socket_dir")]
    pub pg_hostaddr: Option<IpAddr>,
    /// Absolute Unix socket directory.
    #[arg(long, conflicts_with = "pg_hostaddr")]
    pub pg_socket_dir: Option<PathBuf>,
    #[arg(long)]
    pub pg_connect_timeout_seconds: Option<u64>,
    #[arg(long)]
    pub pg_query_timeout_seconds: Option<u64>,
    #[arg(long)]
    pub pg_recovery_timeout_seconds: Option<u64>,
    #[arg(long)]
    pub pg_retry_interval_ms: Option<u64>,
    #[arg(long)]
    pub pg_max_row_bytes: Option<usize>,
    /// Maximum concurrent PostgreSQL snapshot readers (default: 2, range: 1..64).
    #[arg(long)]
    pub pg_max_exports: Option<usize>,
    /// SQL commit policy: synchronous or asynchronous (default: synchronous).
    #[arg(long)]
    pub pg_commit_policy: Option<PostgresCommitSetting>,
}
impl StorageArgs {
    /// Apply only explicit command-line values to the configured storage settings.
    pub fn merge(&self, configured: &StorageSettings) -> StorageSettings {
        let mut settings = configured.clone();
        if let Some(seconds) = self.persistence_shutdown_timeout_seconds {
            settings.shutdown_timeout_seconds = Some(seconds);
        }
        if let Some(backend) = self.storage_backend {
            settings.backend = backend;
        }
        macro_rules! override_field {
            ($arg:ident, $field:ident) => {
                if let Some(value) = &self.$arg {
                    settings.postgres.get_or_insert_default().$field = Some(value.clone());
                }
            };
        }
        override_field!(pg_service, service);
        override_field!(pg_schema, schema);
        override_field!(pg_hostaddr, hostaddr);
        override_field!(pg_socket_dir, socket_dir);
        if self.pg_hostaddr.is_some() {
            settings.postgres.as_mut().unwrap().socket_dir = None;
        }
        if self.pg_socket_dir.is_some() {
            settings.postgres.as_mut().unwrap().hostaddr = None;
        }
        override_field!(pg_connect_timeout_seconds, connect_timeout_seconds);
        override_field!(pg_query_timeout_seconds, query_timeout_seconds);
        override_field!(pg_recovery_timeout_seconds, recovery_timeout_seconds);
        override_field!(pg_retry_interval_ms, retry_interval_ms);
        override_field!(pg_max_row_bytes, max_row_bytes);
        override_field!(pg_max_exports, max_exports);
        override_field!(pg_commit_policy, commit_policy);
        settings
    }

    pub fn resolve(
        &self,
        configured: &StorageSettings,
        fjall_path: impl FnOnce() -> Option<PathBuf>,
        explicit_db: bool,
        explicit_tables: bool,
    ) -> Result<StorageConfig, DatabaseOpenError> {
        let settings = self.merge(configured);
        if (self.init_storage || self.validate_storage || self.install_storage_views)
            && settings.backend != StorageBackendKind::Postgres
        {
            return Err(DatabaseOpenError::StorageConfiguration(
                "storage administration requires PostgreSQL storage",
            ));
        }
        settings.resolve(fjall_path, explicit_db, explicit_tables)
    }

    /// Explicitly add optional inspection projections without rewriting stored world rows.
    pub fn install_views(&self, storage: &StorageConfig) -> Result<bool, DatabaseOpenError> {
        if !self.install_storage_views {
            return Ok(false);
        }
        match storage {
            #[cfg(feature = "postgres")]
            StorageConfig::Postgres(config) => {
                crate::install_postgres_inspection(config)?;
                Ok(true)
            }
            _ => Err(DatabaseOpenError::StorageConfiguration(
                "--install-storage-views requires PostgreSQL storage",
            )),
        }
    }

    /// Return a JSON validation report before any runtime or local stores are opened.
    pub fn validate(&self, storage: &StorageConfig) -> Result<Option<String>, DatabaseOpenError> {
        if !self.validate_storage {
            return Ok(None);
        }
        match storage {
            #[cfg(feature = "postgres")]
            StorageConfig::Postgres(config) => {
                let report = crate::validate_postgres_storage(config)?;
                Ok(Some(
                    serde_json::to_string_pretty(&report)
                        .expect("validation report is serializable"),
                ))
            }
            _ => Err(DatabaseOpenError::StorageConfiguration(
                "--validate-storage requires PostgreSQL storage",
            )),
        }
    }

    /// Explicit setup returns a database identity. Ordinary startup never invokes initialization.
    pub fn initialize(
        &self,
        storage: &StorageConfig,
    ) -> Result<Option<uuid::Uuid>, DatabaseOpenError> {
        if !self.init_storage {
            return Ok(None);
        }
        match storage {
            #[cfg(feature = "postgres")]
            StorageConfig::Postgres(config) => crate::initialize_postgres_schema(config)
                .map(Some)
                .map_err(Into::into),
            _ => Err(DatabaseOpenError::StorageConfiguration(
                "--init-storage requires PostgreSQL storage",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    #[derive(clap_derive::Parser)]
    struct Command {
        #[command(flatten)]
        storage: StorageArgs,
        #[arg(long)]
        db: Option<PathBuf>,
    }
    #[test]
    fn explicit_paths_are_rejected_without_resolving_the_default_path() {
        let command =
            Command::try_parse_from(["test", "--storage-backend", "postgres", "--db", "world.db"])
                .unwrap();
        let result = command.storage.resolve(
            &StorageSettings::default(),
            || panic!("PostgreSQL must not resolve a Fjall path"),
            command.db.is_some(),
            false,
        );
        assert!(result.is_err());
        if cfg!(feature = "postgres") {
            assert!(matches!(
                result,
                Err(DatabaseOpenError::StorageConfiguration(_))
            ));
        } else {
            assert!(matches!(
                result,
                Err(DatabaseOpenError::PostgresFeatureDisabled)
            ));
        }
    }
    #[test]
    fn endpoint_conflicts_and_backend_settings_are_explicit() {
        assert!(
            Command::try_parse_from([
                "test",
                "--pg-hostaddr",
                "127.0.0.1",
                "--pg-socket-dir",
                "/tmp"
            ])
            .is_err()
        );
        let command = Command::try_parse_from(["test", "--pg-schema", "world"]).unwrap();
        assert!(
            command
                .storage
                .resolve(&StorageSettings::default(), || None, false, false)
                .is_err()
        );
        let command = Command::try_parse_from(["test", "--init-storage"]).unwrap();
        assert!(
            command
                .storage
                .resolve(&StorageSettings::default(), || None, false, false)
                .is_err()
        );
    }
    #[test]
    fn shutdown_budget_defaults_and_cli_precedence_are_backend_independent() {
        let configured = StorageSettings {
            shutdown_timeout_seconds: Some(45),
            ..Default::default()
        };
        let command = Command::try_parse_from(["test"]).unwrap();
        assert_eq!(
            command
                .storage
                .merge(&configured)
                .persistence_config()
                .unwrap()
                .shutdown_timeout
                .as_secs(),
            45
        );
        let command =
            Command::try_parse_from(["test", "--persistence-shutdown-timeout-seconds", "9"])
                .unwrap();
        assert_eq!(
            command
                .storage
                .merge(&configured)
                .persistence_config()
                .unwrap()
                .shutdown_timeout
                .as_secs(),
            9
        );
        assert_eq!(
            StorageSettings::default()
                .persistence_config()
                .unwrap()
                .shutdown_timeout
                .as_secs(),
            30
        );
        for seconds in [0, u64::MAX] {
            let configured = StorageSettings {
                shutdown_timeout_seconds: Some(seconds),
                ..Default::default()
            };
            assert!(
                configured
                    .resolve(
                        || panic!("invalid deadline must fail before resolving local paths"),
                        false,
                        false
                    )
                    .is_err()
            );
        }
    }

    #[test]
    fn explicit_export_limit_overrides_yaml_without_changing_the_endpoint() {
        let command = Command::try_parse_from(["test", "--pg-max-exports", "3"]).unwrap();
        let configured = StorageSettings {
            backend: StorageBackendKind::Postgres,
            postgres: Some(crate::PostgresSettings {
                service: Some("world".into()),
                max_exports: Some(1),
                ..Default::default()
            }),
            ..Default::default()
        };
        let merged = command.storage.merge(&configured).postgres.unwrap();
        assert_eq!(merged.max_exports, Some(3));
        assert_eq!(merged.service.as_deref(), Some("world"));
    }
    #[test]
    fn absent_cli_backend_preserves_yaml_selection() {
        let command = Command::try_parse_from(["test", "--pg-schema", "override"]).unwrap();
        let settings = StorageSettings {
            backend: StorageBackendKind::Postgres,
            postgres: Some(crate::PostgresSettings {
                service: Some("world".into()),
                schema: Some("configured".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let merged = command.storage.merge(&settings);
        assert_eq!(merged.backend, StorageBackendKind::Postgres);
        let postgres = merged.postgres.unwrap();
        assert_eq!(postgres.service.as_deref(), Some("world"));
        assert_eq!(postgres.schema.as_deref(), Some("override"));
    }
}
