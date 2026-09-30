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

#![recursion_limit = "256"]

use moor_common::model::loader::{LoaderInterface, SnapshotInterface};
use moor_common::model::{WorldState, WorldStateError, WorldStateSource};
use moor_common::threading::spawn_efficient;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

mod api;
mod cache;
mod config;
mod storage_settings;
pub use storage_settings::{PostgresCommitSetting, PostgresSettings, StorageSettings};
#[cfg(feature = "cli")]
mod storage_args;
#[cfg(feature = "cli")]
pub use storage_args::StorageArgs;
mod engine;
mod model;
mod provider;
mod tx;

use crate::engine::MoorDB;

pub use engine::DatabaseRelation;

pub use api::world_state::DbWorldState;
pub use api::{
    gc::{GCError, GCInterface},
    world_state::db_counters,
};
pub use cache::stats::CacheStats;
pub use cache::{ANCESTRY_CACHE_STATS, PROP_CACHE_STATS, VERB_CACHE_STATS};
pub use cache::{
    ancestry_cache::AncestryCache, prop_cache::PropResolutionCache, verb_cache::VerbResolutionCache,
};
pub use config::{
    AdmissionPolicy, DatabaseConfig, FjallStorageConfig, PersistenceConfig, StorageBackendKind,
    StorageConfig, TableConfig,
};
pub use model::{
    AnonymousObjectMetadata, BytesHolder, EntityMetadataKey, ObjAndUUIDHolder, StringHolder,
    SystemTimeHolder, UUIDHolder,
};

pub use provider::coordinator::PersistenceStatus;
pub use provider::logical::{PersistenceError, PersistenceReceipt, PublicationId, WriterEpoch};
pub use tx::{
    CheckRelation, ConflictResolver, Error, FailOnConflict, PotentialConflict, ProposedOp,
    Relation, RelationCodomain, RelationCodomainHashable, RelationDomain, RelationIndex,
    RelationTransaction, Timestamp, Tx, WorkingSet,
};

pub const DEFAULT_COMMIT_QUEUE_WARN: Duration = Duration::from_secs(1);
pub const DEFAULT_COMMIT_QUEUE_TIMEOUT: Duration = Duration::from_secs(5);

pub type SnapshotCallback = Box<
    dyn FnOnce(Result<Box<dyn SnapshotInterface>, WorldStateError>) -> Result<(), WorldStateError>
        + Send,
>;

/// Point-in-time storage-engine maintenance counters.
///
/// These values are observational and may change immediately after being read.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StorageMaintenanceStats {
    pub write_buffer_bytes: u64,
    pub outstanding_flushes: usize,
    pub active_compactions: usize,
    pub compactions_completed: usize,
    pub compaction_time: Duration,
    pub journal_count: usize,
    pub journal_bytes: u64,
    pub disk_bytes: u64,
}

impl StorageMaintenanceStats {
    /// Whether storage work is currently queued or executing.
    #[must_use]
    pub fn is_active(self) -> bool {
        self.outstanding_flushes != 0 || self.active_compactions != 0
    }
}

/// Outcome of major-compacting one database relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationCompactionResult {
    pub relation: DatabaseRelation,
    pub bytes_before: u64,
    pub bytes_after: u64,
    pub error: Option<String>,
}

impl RelationCompactionResult {
    fn completed(relation: DatabaseRelation, bytes_before: u64, bytes_after: u64) -> Self {
        Self {
            relation,
            bytes_before,
            bytes_after,
            error: None,
        }
    }

    fn failed(
        relation: DatabaseRelation,
        bytes_before: u64,
        bytes_after: u64,
        error: String,
    ) -> Self {
        Self {
            relation,
            bytes_before,
            bytes_after,
            error: Some(error),
        }
    }

    #[must_use]
    pub fn bytes_reclaimed(&self) -> u64 {
        self.bytes_before.saturating_sub(self.bytes_after)
    }
}

// Re-export sequence constants for use in VM
pub use engine::SEQUENCE_MAX_OBJECT;

#[derive(Debug, thiserror::Error)]
pub enum DatabaseOpenError {
    #[error("invalid storage configuration: {0}")]
    StorageConfiguration(&'static str),
    #[cfg(feature = "postgres")]
    #[error("failed to open PostgreSQL world: {0}")]
    Postgres(#[from] PostgresError),
    #[error("PostgreSQL support is disabled; rebuild with the postgres Cargo feature")]
    PostgresFeatureDisabled,

    #[error("failed to create temporary database directory: {source}")]
    TempDir { source: std::io::Error },

    #[error("invalid database format at {path:?}: {detail}")]
    Format { path: PathBuf, detail: String },

    #[error("failed to open database at {path:?}: {detail}")]
    Open { path: PathBuf, detail: String },

    #[error("failed to open keyspace {keyspace:?} in database at {path:?}: {detail}")]
    Keyspace {
        path: PathBuf,
        keyspace: &'static str,
        detail: String,
    },

    #[error("failed to read sequence {index} from database at {path:?}: {detail}")]
    ReadSequence {
        path: PathBuf,
        index: usize,
        detail: String,
    },

    #[error(
        "failed to decode sequence {index} from database at {path:?}: expected 8 bytes, got {len}"
    )]
    DecodeSequence {
        path: PathBuf,
        index: usize,
        len: usize,
    },

    #[error("failed to seed relation {relation:?} from database at {path:?}: {detail}")]
    SeedRelation {
        path: PathBuf,
        relation: &'static str,
        detail: String,
    },

    #[error("transaction timestamp space is exhausted in database at {path:?}")]
    TransactionTimestampExhausted { path: PathBuf },
}

pub trait Database: Send + Sync + WorldStateSource {
    fn loader_client(&self) -> Result<Box<dyn LoaderInterface>, WorldStateError>;
    fn create_snapshot(&self) -> Result<Box<dyn SnapshotInterface>, WorldStateError>;
    fn create_snapshot_async(&self, callback: SnapshotCallback) -> Result<(), WorldStateError>;
    fn gc_interface(&self) -> Result<Box<dyn GCInterface>, WorldStateError>;

    /// Update the wait policy for admission to the database commit queue.
    fn set_commit_queue_policy(&self, _warn_after: Duration, _timeout: Duration) {}

    /// Major-compact selected relation keyspaces and return one result for each relation.
    fn compact_relations(
        &self,
        _relations: &[DatabaseRelation],
    ) -> Result<Vec<RelationCompactionResult>, WorldStateError> {
        Err(WorldStateError::DatabaseError(
            "Storage engine does not support relation compaction".to_string(),
        ))
    }

    /// Return storage maintenance counters when supported by the engine.
    fn storage_maintenance_stats(&self) -> Option<StorageMaintenanceStats> {
        None
    }
}

#[derive(Clone)]
pub struct TxDB {
    storage: Arc<MoorDB>,
}

impl TxDB {
    /// Open a database with an explicit storage backend and persistence policy.
    pub fn try_open(
        storage: StorageConfig,
        database_config: DatabaseConfig,
        persistence: PersistenceConfig,
    ) -> Result<(Self, bool), DatabaseOpenError> {
        let (storage, fresh) = MoorDB::try_open(storage, database_config, persistence)?;
        Ok((Self { storage }, fresh))
    }

    /// Open a Fjall world at `path` with the default persistence policy.
    pub fn try_open_fjall(
        path: impl AsRef<Path>,
        database_config: DatabaseConfig,
    ) -> Result<(Self, bool), DatabaseOpenError> {
        Self::try_open(
            StorageConfig::fjall(path.as_ref()),
            database_config,
            PersistenceConfig::default(),
        )
    }

    /// Open a temporary Fjall world with the default persistence policy.
    pub fn try_open_temporary(
        database_config: DatabaseConfig,
    ) -> Result<(Self, bool), DatabaseOpenError> {
        Self::try_open(
            StorageConfig::temporary_fjall(),
            database_config,
            PersistenceConfig::default(),
        )
    }

    /// Capture the current published world. The token is valid only for this database open.
    pub fn publication(&self) -> PublicationId {
        self.storage.publication()
    }

    /// Read the current persistence progress, admission usage, and writer health.
    pub fn persistence_status(&self) -> PersistenceStatus {
        self.storage.persistence_status()
    }

    /// Wait for application through a captured publication, without requesting a durable flush.
    pub fn wait_applied(
        &self,
        publication: PublicationId,
        timeout: Duration,
    ) -> Result<PersistenceReceipt, PersistenceError> {
        self.storage.wait_applied(publication, timeout)
    }

    /// Establish a local durable fence through a captured publication.
    pub fn wait_durable(
        &self,
        publication: PublicationId,
        timeout: Duration,
    ) -> Result<PersistenceReceipt, PersistenceError> {
        self.storage.wait_durable(publication, timeout)
    }

    /// Wait until the current published state has been applied to storage.
    ///
    /// This does not request an fsync or wait for LSM maintenance.
    pub fn wait_for_persistence(&self) -> Result<(), WorldStateError> {
        self.storage
            .wait_for_persistence()
            .map_err(WorldStateError::DatabaseError)
    }

    /// Deadline-bounded variant of [`Self::wait_for_persistence`].
    pub fn wait_for_persistence_with_deadline(
        &self,
        deadline: Duration,
    ) -> Result<(), WorldStateError> {
        self.storage
            .wait_for_persistence_with_deadline(deadline)
            .map_err(WorldStateError::DatabaseError)
    }

    /// Wait until the current published state has crossed the local durable-storage fence.
    pub fn wait_for_durability(&self, deadline: Duration) -> Result<(), WorldStateError> {
        self.storage
            .wait_for_durability(deadline)
            .map_err(WorldStateError::DatabaseError)
    }

    /// Create a snapshot with an explicit acquisition deadline.
    ///
    /// The deadline covers the applied barrier, queue submission, and snapshot receipt. Unlike
    /// [`Self::wait_for_persistence_with_deadline`], a returned snapshot is not silently expired
    /// halfway through an export; query deadlines are the loader's own responsibility.
    pub fn create_snapshot_with_timeout(
        &self,
        timeout: Duration,
    ) -> Result<Box<dyn SnapshotInterface>, WorldStateError> {
        self.storage
            .create_snapshot_with_timeout(timeout)
            .map_err(|error| WorldStateError::DatabaseError(error.to_string()))
    }
}

impl WorldStateSource for TxDB {
    fn new_world_state(&self) -> Result<Box<dyn WorldState>, WorldStateError> {
        let tx = self.storage.start_transaction();
        let tx = api::world_state::DbWorldState { tx };
        Ok(Box::new(tx))
    }

    fn checkpoint(&self) -> Result<(), WorldStateError> {
        // TODO: noop for now... but this should probably do a sync of sequences to disk and make
        //   sure all data is durable.
        Ok(())
    }
}

impl Database for TxDB {
    fn loader_client(&self) -> Result<Box<dyn LoaderInterface>, WorldStateError> {
        let tx = self.storage.start_transaction();
        let tx = api::world_state::DbWorldState { tx };
        Ok(Box::new(tx))
    }

    fn create_snapshot(&self) -> Result<Box<dyn SnapshotInterface>, WorldStateError> {
        self.storage
            .create_snapshot()
            .map_err(|e| WorldStateError::DatabaseError(e.to_string()))
    }

    fn create_snapshot_async(&self, callback: SnapshotCallback) -> Result<(), WorldStateError> {
        let storage = self.storage.clone();
        spawn_efficient("moor-snapshot", move || {
            let snapshot_result = storage
                .create_snapshot()
                .map_err(|e| WorldStateError::DatabaseError(e.to_string()));

            if let Err(e) = callback(snapshot_result) {
                tracing::error!("Snapshot callback failed: {}", e);
            }
        })
        .map_err(|e| {
            WorldStateError::DatabaseError(format!("Failed to spawn snapshot thread: {e}"))
        })?;
        Ok(())
    }

    fn gc_interface(&self) -> Result<Box<dyn GCInterface>, WorldStateError> {
        let tx = self.storage.start_transaction();
        let tx = api::world_state::DbWorldState { tx };
        Ok(Box::new(tx))
    }

    fn set_commit_queue_policy(&self, warn_after: Duration, timeout: Duration) {
        self.storage.set_commit_queue_policy(warn_after, timeout);
    }

    fn compact_relations(
        &self,
        relations: &[DatabaseRelation],
    ) -> Result<Vec<RelationCompactionResult>, WorldStateError> {
        self.storage
            .compact_relations(relations)
            .map_err(WorldStateError::DatabaseError)
    }

    fn storage_maintenance_stats(&self) -> Option<StorageMaintenanceStats> {
        self.storage.storage_maintenance_stats()
    }
}

mod relation_registry;

#[cfg(feature = "postgres")]
pub use provider::postgres::{
    PostgresCommitPolicy, PostgresConnectOptions, PostgresConnection, PostgresEndpoint,
    PostgresError, PostgresParam, PostgresRow, PostgresSchema, PostgresShutdown,
    PostgresStatementResult, PostgresStorageConfig, initialize_postgres_schema,
};
