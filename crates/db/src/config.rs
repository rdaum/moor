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

use fjall::KeyspaceCreateOptions;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

use crate::{DEFAULT_COMMIT_QUEUE_TIMEOUT, DEFAULT_COMMIT_QUEUE_WARN};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatabaseConfig {
    /// Per-table configurations
    pub object_location: Option<TableConfig>,
    pub object_contents: Option<TableConfig>,
    pub object_flags: Option<TableConfig>,
    pub object_parent: Option<TableConfig>,
    pub object_children: Option<TableConfig>,
    pub object_owner: Option<TableConfig>,
    pub object_name: Option<TableConfig>,
    pub object_verbdefs: Option<TableConfig>,
    pub object_verbs: Option<TableConfig>,
    pub object_propdefs: Option<TableConfig>,
    pub object_propvalues: Option<TableConfig>,
    pub object_propflags: Option<TableConfig>,
    pub entity_metadata: Option<TableConfig>,
    pub object_last_move: Option<TableConfig>,
    pub anonymous_object_metadata: Option<TableConfig>,
}

const LARGE_MEMTABLE_SIZE: u64 = 512 * 1024 * 1024; // 512 MiB

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            object_location: None,
            object_contents: None,
            object_flags: None,
            object_parent: None,
            object_children: None,
            object_owner: None,
            object_name: None,
            object_verbdefs: None,
            // Verbs and propvalues are hot tables under write pressure - larger memtables
            // reduce L0 segment accumulation and fjall backpressure stalls
            object_verbs: Some(TableConfig {
                max_memtable_size: Some(LARGE_MEMTABLE_SIZE),
            }),
            object_propdefs: None,
            object_propvalues: Some(TableConfig {
                max_memtable_size: Some(LARGE_MEMTABLE_SIZE),
            }),
            object_propflags: None,
            entity_metadata: None,
            object_last_move: None,
            anonymous_object_metadata: None,
        }
    }
}

/// How long a transaction may wait for a persistence admission permit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmissionPolicy {
    /// Log a warning once a wait has lasted this long.
    pub warn_after: Duration,
    /// Reject the commit once a wait has lasted this long.
    pub timeout: Duration,
}

impl Default for AdmissionPolicy {
    fn default() -> Self {
        Self {
            warn_after: DEFAULT_COMMIT_QUEUE_WARN,
            timeout: DEFAULT_COMMIT_QUEUE_TIMEOUT,
        }
    }
}

/// Backend-independent persistence lifecycle configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PersistenceConfig {
    pub admission: AdmissionPolicy,
    /// Finite deadline for draining and stopping persistence workers.
    pub shutdown_timeout: Duration,
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self {
            admission: AdmissionPolicy::default(),
            shutdown_timeout: Duration::from_secs(30),
        }
    }
}

/// Fjall storage selection.
#[derive(Clone, Debug)]
pub struct FjallStorageConfig {
    /// Database directory, or `None` for a temporary directory.
    pub path: Option<PathBuf>,
}

/// Selects the compiled-in persistence backend and its storage location.
#[derive(Clone, Debug)]
pub enum StorageConfig {
    Fjall(FjallStorageConfig),
    #[cfg(feature = "postgres")]
    Postgres(Box<crate::PostgresStorageConfig>),
}

impl StorageConfig {
    pub fn kind(&self) -> StorageBackendKind {
        match self {
            Self::Fjall(_) => StorageBackendKind::Fjall,
            #[cfg(feature = "postgres")]
            Self::Postgres(_) => StorageBackendKind::Postgres,
        }
    }

    /// Select an explicitly initialized PostgreSQL world schema.
    #[cfg(feature = "postgres")]
    pub fn postgres(config: crate::PostgresStorageConfig) -> Self {
        Self::Postgres(Box::new(config))
    }

    /// Select a Fjall database at `path`.
    #[must_use]
    pub fn fjall(path: impl Into<PathBuf>) -> Self {
        Self::Fjall(FjallStorageConfig {
            path: Some(path.into()),
        })
    }

    /// Select a temporary Fjall database.
    #[must_use]
    pub fn temporary_fjall() -> Self {
        Self::Fjall(FjallStorageConfig { path: None })
    }
}

/// Per-table configuration.
#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TableConfig {
    /// Various fjall keyspace creation options.
    /// Refer to the fjall documentation for more information.
    pub max_memtable_size: Option<u64>,
}

impl TableConfig {
    pub fn keyspace_options(&self) -> KeyspaceCreateOptions {
        let mut opts = KeyspaceCreateOptions::default();
        if let Some(max_memtable_size) = self.max_memtable_size {
            opts = opts.max_memtable_size(max_memtable_size);
        }
        opts
    }
}

/// Backend selection remains parseable even when an adapter is not compiled in.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageBackendKind {
    #[default]
    Fjall,
    Postgres,
}

impl std::fmt::Display for StorageBackendKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Fjall => "fjall",
            Self::Postgres => "postgres",
        })
    }
}

impl std::str::FromStr for StorageBackendKind {
    type Err = &'static str;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "fjall" => Ok(Self::Fjall),
            "postgres" => Ok(Self::Postgres),
            _ => Err("storage backend must be fjall or postgres"),
        }
    }
}

impl StorageBackendKind {
    /// Check world-storage support before creating local directories, keys, or stores.
    pub fn check_available(self) -> Result<(), crate::DatabaseOpenError> {
        if self == Self::Fjall {
            return Ok(());
        }
        if !cfg!(feature = "postgres") {
            return Err(crate::DatabaseOpenError::PostgresFeatureDisabled);
        }
        Ok(())
    }
}

#[cfg(test)]
mod backend_selection_tests {
    use super::*;
    #[test]
    fn postgres_selection_never_falls_back_to_fjall() {
        assert_eq!(
            "postgres".parse::<StorageBackendKind>().unwrap(),
            StorageBackendKind::Postgres
        );
        assert!(StorageBackendKind::Fjall.check_available().is_ok());
        let result = StorageBackendKind::Postgres.check_available();
        if cfg!(feature = "postgres") {
            assert!(result.is_ok());
        } else {
            assert!(matches!(
                result,
                Err(crate::DatabaseOpenError::PostgresFeatureDisabled)
            ));
        }
    }
}
