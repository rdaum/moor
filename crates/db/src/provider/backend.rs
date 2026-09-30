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

//! Backend dispatch is confined to opening, snapshot acquisition, and maintenance.
use super::{
    batch_writer::BatchWriter, fjall_format, fjall_relations::FjallRelations,
    property_value_store::PROPERTY_VALUE_CHAIN_LIMITS, snapshot_loader::SnapshotLoader,
};
use crate::{
    DatabaseConfig, DatabaseOpenError, StorageMaintenanceStats,
    config::StorageConfig,
    engine::moor_db::{DatabaseRelation, Relations, SEQUENCE_COUNT, WorldStateSnapshot},
};
use fjall::{Database, KeyspaceCreateOptions};
use moor_common::model::{HasUuid, ValSet, loader::SnapshotInterface};
use std::sync::Arc;
use tempfile::TempDir;

/// Backend snapshot receipt after its applied barrier.
pub(crate) enum StorageSnapshot {
    Fjall(fjall::Snapshot),
}
/// Keep temporary storage alive until the last reader and cursor release their leases.
pub(crate) struct FjallReadSnapshot {
    pub(crate) snapshot: fjall::Snapshot,
    pub(crate) _directory: Arc<Option<TempDir>>,
}
impl std::ops::Deref for FjallReadSnapshot {
    type Target = fjall::Snapshot;
    fn deref(&self) -> &Self::Target {
        &self.snapshot
    }
}
pub(crate) enum StorageBackend {
    Fjall(FjallStorage),
    #[cfg(feature = "postgres")]
    Postgres(Arc<std::sync::atomic::AtomicU64>),
}
pub(crate) struct FjallStorage {
    pub(crate) database: Database,
    pub(crate) relations: Arc<FjallRelations>,
    directory: Arc<Option<TempDir>>,
}
pub(crate) struct SeededWorld {
    pub(crate) root: WorldStateSnapshot,
    pub(crate) sequences: Vec<i64>,
}
pub(crate) struct OpenedStorage {
    pub(crate) backend: StorageBackend,
    pub(crate) seed: SeededWorld,
    pub(crate) writer: super::writer::StorageWriter,
    pub(crate) fresh: bool,
    pub(crate) start_tx_num: u64,
}
impl StorageBackend {
    pub(crate) fn open(
        storage: StorageConfig,
        config: DatabaseConfig,
        relations: &Arc<Relations>,
        _epoch: super::logical::WriterEpoch,
    ) -> Result<OpenedStorage, DatabaseOpenError> {
        let fjall = match storage {
            StorageConfig::Fjall(fjall) => fjall,
            #[cfg(feature = "postgres")]
            StorageConfig::Postgres(postgres) => {
                if config != DatabaseConfig::default() {
                    return Err(DatabaseOpenError::StorageConfiguration(
                        "Fjall table tuning is not supported by PostgreSQL storage",
                    ));
                }
                let (writer, seed, fresh, start_tx_num) =
                    super::postgres::PostgresWriter::open(*postgres, relations.clone(), _epoch)?;
                return Ok(OpenedStorage {
                    backend: Self::Postgres(writer.storage_bytes()),
                    seed,
                    writer: super::writer::StorageWriter::Postgres(writer),
                    fresh,
                    start_tx_num,
                });
            }
        };
        let tmpdir = Arc::new(if fjall.path.is_none() {
            Some(TempDir::new().map_err(|source| DatabaseOpenError::TempDir { source })?)
        } else {
            None
        });
        let path_buf = fjall
            .path
            .clone()
            .unwrap_or_else(|| tmpdir.as_ref().as_ref().unwrap().path().to_path_buf());
        let path = path_buf.as_path();

        fjall_format::fjall_check_format(path).map_err(|e| DatabaseOpenError::Format {
            path: path_buf.clone(),
            detail: e.to_string(),
        })?;

        let keyspace = Database::builder(path)
            .open()
            .map_err(|e| DatabaseOpenError::Open {
                path: path_buf.clone(),
                detail: e.to_string(),
            })?;

        let sequences_partition = keyspace
            .keyspace("sequences", KeyspaceCreateOptions::default)
            .map_err(|e| DatabaseOpenError::Keyspace {
                path: path_buf.clone(),
                keyspace: "sequences",
                detail: e.to_string(),
            })?;

        let fresh = !keyspace.keyspace_exists("object_location");

        let storage_relations = Arc::new(FjallRelations::open(&keyspace, &config, &path_buf)?);
        // All relations, sequence recovery, and writer bootstrap share one read view.
        let opener_snapshot = Arc::new(FjallReadSnapshot {
            snapshot: keyspace.snapshot(),
            _directory: tmpdir.clone(),
        });

        let initial_sequences =
            read_sequences(&opener_snapshot.snapshot, &sequences_partition, &path_buf)?;

        let (initial_root, property_value_chains) =
            storage_relations.seed(relations, &opener_snapshot, &path_buf)?;
        let start_tx_num = initial_root
            .committed_ts
            .0
            .checked_add(1)
            .ok_or_else(|| DatabaseOpenError::TransactionTimestampExhausted {
                path: path_buf.clone(),
            })?
            .max(1);
        let property_names = initial_root
            .object_propdefs
            .iter()
            .flat_map(|(_, entry)| entry.value.iter())
            .map(|definition| (definition.uuid(), definition.name()))
            .collect();
        let writer = BatchWriter::with_property_value_state(
            keyspace.clone(),
            sequences_partition,
            initial_sequences.clone(),
            Some(storage_relations.clone()),
            property_names,
            property_value_chains,
            PROPERTY_VALUE_CHAIN_LIMITS,
        );

        Ok(OpenedStorage {
            backend: Self::Fjall(FjallStorage {
                database: keyspace,
                relations: storage_relations,
                directory: tmpdir,
            }),
            seed: SeededWorld {
                root: initial_root,
                sequences: initial_sequences,
            },
            writer: writer.into(),
            fresh,
            start_tx_num,
        })
    }
    pub(crate) fn snapshot_loader(&self, snapshot: StorageSnapshot) -> Box<dyn SnapshotInterface> {
        match (self, snapshot) {
            (Self::Fjall(storage), StorageSnapshot::Fjall(snapshot)) => {
                let lease = Arc::new(FjallReadSnapshot {
                    snapshot,
                    _directory: storage.directory.clone(),
                });
                Box::new(SnapshotLoader {
                    readers: storage.relations.readers(&lease),
                })
            }
            #[cfg(feature = "postgres")]
            (Self::Postgres(_), _) => {
                unreachable!("PostgreSQL writer cannot issue a Fjall snapshot")
            }
        }
    }
    pub(crate) fn usage_bytes(&self) -> usize {
        match self {
            Self::Fjall(storage) => storage.database.disk_space().unwrap_or_default() as usize,
            #[cfg(feature = "postgres")]
            Self::Postgres(bytes) => {
                usize::try_from(bytes.load(std::sync::atomic::Ordering::Acquire))
                    .unwrap_or(usize::MAX)
            }
        }
    }
    pub(crate) fn compact_relations(
        &self,
        relations: &[DatabaseRelation],
    ) -> Vec<crate::RelationCompactionResult> {
        match self {
            Self::Fjall(storage) => storage.relations.compact_relations(relations),
            #[cfg(feature = "postgres")]
            Self::Postgres(_) => relations
                .iter()
                .map(|relation| {
                    crate::RelationCompactionResult::failed(
                        *relation,
                        0,
                        0,
                        "PostgreSQL does not support Fjall relation compaction".into(),
                    )
                })
                .collect(),
        }
    }
    pub(crate) fn maintenance_stats(&self) -> Option<StorageMaintenanceStats> {
        match self {
            Self::Fjall(storage) => {
                let db = &storage.database;
                Some(StorageMaintenanceStats {
                    write_buffer_bytes: db.write_buffer_size(),
                    outstanding_flushes: db.outstanding_flushes(),
                    active_compactions: db.active_compactions(),
                    compactions_completed: db.compactions_completed(),
                    compaction_time: db.time_compacting(),
                    journal_count: db.journal_count(),
                    journal_bytes: db.journal_disk_space().unwrap_or_default(),
                    disk_bytes: db.disk_space().unwrap_or_default(),
                })
            }
            #[cfg(feature = "postgres")]
            Self::Postgres(_) => None,
        }
    }
    #[cfg(test)]
    pub(crate) fn fjall(&self) -> &FjallStorage {
        match self {
            Self::Fjall(storage) => storage,
            #[cfg(feature = "postgres")]
            Self::Postgres(_) => panic!("Fjall fixture required"),
        }
    }
}

/// Sequence recovery uses the same snapshot as the relation seed.
pub(crate) fn read_sequences(
    snapshot: &fjall::Snapshot,
    partition: &fjall::Keyspace,
    path: &std::path::Path,
) -> Result<Vec<i64>, DatabaseOpenError> {
    (0..SEQUENCE_COUNT)
        .map(|index| {
            fjall::Readable::get(snapshot, partition, index.to_le_bytes())
                .map_err(|error| DatabaseOpenError::ReadSequence {
                    path: path.to_path_buf(),
                    index,
                    detail: error.to_string(),
                })?
                .map(|bytes| {
                    let value: [u8; 8] = bytes.as_ref().try_into().map_err(|_| {
                        DatabaseOpenError::DecodeSequence {
                            path: path.to_path_buf(),
                            index,
                            len: bytes.len(),
                        }
                    })?;
                    Ok(i64::from_le_bytes(value))
                })
                .transpose()
                .map(|value| value.unwrap_or(-1))
        })
        .collect()
}
