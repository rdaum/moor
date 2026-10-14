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

//! Background execution for relation-scoped storage compaction.

use std::sync::Arc;

use moor_common::{tasks::SchedulerError, threading::spawn_efficient};
use moor_db::{Database, DatabaseRelation, RelationCompactionResult};
use moor_var::{Var, v_int, v_list_iter, v_map, v_none, v_str};
use parking_lot::Mutex;
use tracing::error;

use crate::tasks::maintenance::{MaintenanceCoordinator, MaintenanceKind, MaintenanceTicket};

type CompletionCallback =
    Box<dyn FnOnce(u64, Result<Vec<RelationCompactionResult>, SchedulerError>) + Send + 'static>;

/// An admitted compaction which has not yet been handed to its worker thread.
pub(crate) struct StorageCompactionJob {
    coordinator: MaintenanceCoordinator,
    ticket: MaintenanceTicket,
    relations: Vec<DatabaseRelation>,
    launched: bool,
}

impl StorageCompactionJob {
    #[inline]
    pub(crate) fn generation(&self) -> u64 {
        self.ticket.generation()
    }

    #[inline]
    pub(crate) fn relations(&self) -> &[DatabaseRelation] {
        &self.relations
    }

    pub(crate) fn launch(
        mut self,
        database: Arc<dyn Database>,
        callback: CompletionCallback,
    ) -> Result<MaintenanceTicket, SchedulerError> {
        let ticket = self.ticket.clone();
        let worker_ticket = ticket.clone();
        let coordinator = self.coordinator.clone();
        let relations = std::mem::take(&mut self.relations);
        let callback = Arc::new(Mutex::new(Some(callback)));
        let worker_callback = callback.clone();
        self.launched = true;

        let spawn = spawn_efficient("moor-storage-compaction", move || {
            let results = database.compact_relations(&relations).map_err(|error| {
                error!(
                    ?error,
                    "Storage compaction failed before producing relation results"
                );
                SchedulerError::CouldNotStartTask
            });
            let maintenance_outcome = match results.as_ref() {
                Ok(results) if results.iter().all(|result| result.error.is_none()) => Ok(()),
                Ok(_) | Err(_) => Err(SchedulerError::CouldNotStartTask),
            };
            coordinator.complete(&worker_ticket, maintenance_outcome);
            if let Some(callback) = worker_callback.lock().take() {
                callback(worker_ticket.generation(), results);
            }
        });

        if let Err(error) = spawn {
            error!(?error, "Could not start storage compaction worker");
            self.coordinator
                .complete(&ticket, Err(SchedulerError::CouldNotStartTask));
            if let Some(callback) = callback.lock().take() {
                callback(ticket.generation(), Err(SchedulerError::CouldNotStartTask));
            }
            return Err(SchedulerError::CouldNotStartTask);
        }

        Ok(ticket)
    }
}

impl Drop for StorageCompactionJob {
    fn drop(&mut self) {
        if self.launched {
            return;
        }
        self.coordinator
            .complete(&self.ticket, Err(SchedulerError::CouldNotStartTask));
    }
}

pub(crate) fn prepare_storage_compaction(
    coordinator: &MaintenanceCoordinator,
    relations: Vec<DatabaseRelation>,
) -> Result<StorageCompactionJob, SchedulerError> {
    if relations.is_empty() {
        return Err(SchedulerError::CouldNotStartTask);
    }
    let ticket = coordinator.admit(MaintenanceKind::StorageCompaction)?;
    Ok(StorageCompactionJob {
        coordinator: coordinator.clone(),
        ticket,
        relations,
        launched: false,
    })
}

pub(crate) fn compaction_results_to_var(results: &[RelationCompactionResult]) -> Var {
    v_list_iter(results.iter().map(|result| {
        let error = result.error.as_deref().map_or_else(v_none, v_str);
        v_map(&[
            (v_str("relation"), v_str(result.relation.as_str())),
            (
                v_str("bytes_before"),
                v_int(saturating_byte_count(result.bytes_before)),
            ),
            (
                v_str("bytes_after"),
                v_int(saturating_byte_count(result.bytes_after)),
            ),
            (
                v_str("bytes_reclaimed"),
                v_int(saturating_byte_count(result.bytes_reclaimed())),
            ),
            (v_str("error"), error),
        ])
    }))
}

pub(crate) fn compaction_failure_to_var(
    relations: &[DatabaseRelation],
    error: &SchedulerError,
) -> Var {
    let message = error.to_string();
    v_list_iter(relations.iter().map(|relation| {
        v_map(&[
            (v_str("relation"), v_str(relation.as_str())),
            (v_str("bytes_before"), v_int(0)),
            (v_str("bytes_after"), v_int(0)),
            (v_str("bytes_reclaimed"), v_int(0)),
            (v_str("error"), v_str(&message)),
        ])
    }))
}

fn saturating_byte_count(bytes: u64) -> i64 {
    i64::try_from(bytes).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;
    use moor_db::{DatabaseConfig, TxDB};

    #[test]
    fn compaction_runs_on_its_dedicated_thread() {
        let (database, _) = TxDB::try_open_temporary(DatabaseConfig::default()).unwrap();
        let coordinator = MaintenanceCoordinator::new();
        let job =
            prepare_storage_compaction(&coordinator, vec![DatabaseRelation::ObjectPropvalues])
                .unwrap();
        let (send, receive) = mpsc::channel();

        let ticket = job
            .launch(
                Arc::new(database),
                Box::new(move |_, results| {
                    send.send((
                        std::thread::current().name().map(str::to_owned),
                        moor_common::threading::current_task_worker_index(),
                        results,
                    ))
                    .unwrap();
                }),
            )
            .unwrap();

        let (thread_name, task_worker, results) = receive.recv().unwrap();
        assert_eq!(thread_name.as_deref(), Some("moor-storage-compaction"));
        assert_eq!(task_worker, None);
        assert_eq!(results.unwrap().len(), 1);
        ticket.wait().unwrap();
    }
}
