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

//! Backend dispatch at persistence admission, wait, and lifecycle boundaries.
use super::{
    backend::StorageSnapshot,
    batch_writer::{BatchWriter, WriterWaitError},
    coordinator::CommitAdmission,
    logical::LogicalCommit,
};
use std::{
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

/// Backend preparation retained by the admission permit until publication or abandonment.
#[cfg(feature = "postgres")]
#[derive(Default)]
pub(crate) struct StoragePreparation {
    pub(crate) postgres: Option<Box<super::postgres::EncodedCommit>>,
}

pub(crate) enum StorageWriter {
    Fjall(BatchWriter),
    #[cfg(feature = "postgres")]
    Postgres(super::postgres::PostgresWriter),
}
impl From<BatchWriter> for StorageWriter {
    fn from(value: BatchWriter) -> Self {
        Self::Fjall(value)
    }
}
impl StorageWriter {
    pub(crate) fn prepare(
        &self,
        _changes: &crate::engine::moor_db::RelationWorkingSets,
        _timestamp: crate::Timestamp,
        _admission: &mut CommitAdmission,
    ) -> Result<(), String> {
        match self {
            Self::Fjall(_) => Ok(()),
            #[cfg(feature = "postgres")]
            Self::Postgres(writer) => {
                _admission.preparation.postgres =
                    Some(Box::new(writer.prepare(_changes, _timestamp)?));
                Ok(())
            }
        }
    }
    pub(crate) fn health_flag(&self) -> Arc<AtomicBool> {
        match self {
            Self::Fjall(writer) => writer.health_flag(),
            #[cfg(feature = "postgres")]
            Self::Postgres(writer) => writer.health_flag(),
        }
    }
    pub(crate) fn healthy(&self) -> bool {
        match self {
            Self::Fjall(writer) => writer.healthy(),
            #[cfg(feature = "postgres")]
            Self::Postgres(writer) => writer.healthy(),
        }
    }
    pub(crate) fn completed_version(&self) -> u64 {
        match self {
            Self::Fjall(writer) => writer.completed_version(),
            #[cfg(feature = "postgres")]
            Self::Postgres(writer) => writer.completed_version(),
        }
    }
    pub(crate) fn durable_version(&self) -> u64 {
        match self {
            Self::Fjall(writer) => writer.durable_version(),
            #[cfg(feature = "postgres")]
            Self::Postgres(writer) => writer.durable_version(),
        }
    }
    pub(crate) fn submit(
        &self,
        commit: LogicalCommit,
        admission: CommitAdmission,
    ) -> Result<(), String> {
        match self {
            Self::Fjall(writer) => writer.submit(commit, admission),
            #[cfg(feature = "postgres")]
            Self::Postgres(writer) => writer.submit(commit, admission),
        }
    }
    pub(crate) fn wait_applied(
        &self,
        version: u64,
        timeout: Duration,
    ) -> Result<(), WriterWaitError> {
        match self {
            Self::Fjall(writer) => writer.wait_applied(version, timeout),
            #[cfg(feature = "postgres")]
            Self::Postgres(writer) => writer.wait_applied(version, timeout),
        }
    }
    pub(crate) fn wait_applied_unbounded(&self, version: u64) -> Result<(), WriterWaitError> {
        match self {
            Self::Fjall(writer) => writer.wait_applied_unbounded(version),
            #[cfg(feature = "postgres")]
            Self::Postgres(writer) => writer.wait_applied_unbounded(version),
        }
    }
    pub(crate) fn wait_durable(
        &self,
        version: u64,
        timeout: Duration,
    ) -> Result<(), WriterWaitError> {
        match self {
            Self::Fjall(writer) => writer.wait_durable(version, timeout),
            #[cfg(feature = "postgres")]
            Self::Postgres(writer) => writer.wait_durable(version, timeout),
        }
    }
    pub(crate) fn stop_with_deadline(&self, timeout: Duration) -> Result<(), String> {
        match self {
            Self::Fjall(writer) => writer.stop_with_deadline(timeout),
            #[cfg(feature = "postgres")]
            Self::Postgres(writer) => writer.stop_with_deadline(timeout),
        }
    }
    pub(crate) fn cancel_waiters(&self) {
        match self {
            Self::Fjall(writer) => writer.cancel_waiters(),
            #[cfg(feature = "postgres")]
            Self::Postgres(writer) => writer.cancel_waiters(),
        }
    }
    pub(crate) fn snapshot(
        &self,
        version: u64,
        timeout: Duration,
    ) -> Result<StorageSnapshot, WriterWaitError> {
        match self {
            Self::Fjall(writer) => writer
                .snapshot(version, timeout)
                .map(StorageSnapshot::Fjall),
            #[cfg(feature = "postgres")]
            Self::Postgres(_) => Err(WriterWaitError::Failed {
                detail: "PostgreSQL snapshot export is not implemented yet".into(),
            }),
        }
    }
    #[cfg(test)]
    pub(crate) fn try_send_request(
        &self,
        request: super::batch_writer::EncodeRequest,
        admission: Option<CommitAdmission>,
    ) -> Result<(), String> {
        match self {
            Self::Fjall(writer) => writer.try_send_request(request, admission),
            #[cfg(feature = "postgres")]
            Self::Postgres(_) => Err("Fjall test request sent to PostgreSQL writer".into()),
        }
    }
}
