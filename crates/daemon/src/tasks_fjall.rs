// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use fjall::{Config, Keyspace, PartitionCreateOptions, PartitionHandle};
use moor_kernel::SuspendedTask;
use moor_kernel::tasks::{TasksDb, TasksDbError};
use moor_values::BINCODE_CONFIG;
use moor_values::tasks::TaskId;
use std::path::Path;
use tracing::error;

pub struct FjallTasksDB {
    _keyspace: Keyspace,
    tasks_partition: PartitionHandle,
}

impl FjallTasksDB {
    pub fn open(path: &Path) -> (Self, bool) {
        let keyspace = Config::new(path).open().unwrap();
        let fresh = keyspace.partition_count() == 0;
        let tasks_partition = keyspace
            .open_partition("tasks", PartitionCreateOptions::default())
            .unwrap();
        (
            Self {
                _keyspace: keyspace,
                tasks_partition,
            },
            fresh,
        )
    }
}

impl TasksDb for FjallTasksDB {
    fn load_tasks(&self) -> Result<Vec<SuspendedTask>, TasksDbError> {
        let pi = self.tasks_partition.iter();
        let mut tasks = vec![];
        for entry in pi {
            let entry = entry.map_err(|_| TasksDbError::CouldNotLoadTasks)?;
            let task_id = TaskId::from_le_bytes(entry.0.as_ref().try_into().map_err(|e| {
                error!("Failed to deserialize TaskId from record: {:?}", e);
                TasksDbError::CouldNotLoadTasks
            })?);
            let tasks_bytes = entry.1.as_ref();
            let (task, _): (SuspendedTask, usize) =
                bincode::decode_from_slice(tasks_bytes, *BINCODE_CONFIG)
                    .map_err(|e| {
                        error!("Failed to deserialize SuspendedTask record: {:?}", e);
                        TasksDbError::CouldNotLoadTasks
                    })
                    .expect("Failed to deserialize record");
            if task_id != task.task.task_id {
                panic!("Task ID mismatch: {:?} != {:?}", task_id, task.task.task_id);
            }
            tasks.push(task);
        }
        Ok(tasks)
    }

    fn save_task(&self, task: &SuspendedTask) -> Result<(), TasksDbError> {
        let task_id = task.task.task_id.to_le_bytes();
        let task_bytes = bincode::encode_to_vec(task, *BINCODE_CONFIG).map_err(|e| {
            error!("Failed to serialize record: {:?}", e);
            TasksDbError::CouldNotSaveTask
        })?;

        self.tasks_partition
            .insert(task_id, &task_bytes)
            .map_err(|e| {
                error!("Failed to insert record: {:?}", e);
                TasksDbError::CouldNotSaveTask
            })?;

        Ok(())
    }

    fn delete_task(&self, task_id: TaskId) -> Result<(), TasksDbError> {
        let task_id = task_id.to_le_bytes();
        self.tasks_partition.remove(task_id).map_err(|e| {
            error!("Failed to delete record: {:?}", e);
            TasksDbError::CouldNotDeleteTask
        })?;
        Ok(())
    }

    fn delete_all_tasks(&self) -> Result<(), TasksDbError> {
        for entry in self.tasks_partition.iter() {
            let entry = entry.map_err(|_| TasksDbError::CouldNotDeleteTask)?;
            self.tasks_partition.remove(entry.0.as_ref()).map_err(|e| {
                error!("Failed to delete record: {:?}", e);
                TasksDbError::CouldNotDeleteTask
            })?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::tasks_fjall::FjallTasksDB;
    use moor_kernel::tasks::sessions::NoopClientSession;
    use moor_kernel::tasks::{ServerOptions, TaskStart, TasksDb};
    use moor_kernel::{SuspendedTask, Task, WakeCondition};
    use moor_values::SYSTEM_OBJECT;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    // Verify creation of an empty DB, including creation of tables.
    #[test]
    fn open_reopen() {
        let tmpdir = tempfile::tempdir().expect("Unable to create temporary directory");
        let path = tmpdir.path();
        {
            let (db, is_fresh) = FjallTasksDB::open(path);
            assert!(is_fresh);
            let tasks = db.load_tasks().unwrap();
            assert_eq!(tasks.len(), 0);
        }
        {
            let (db, is_fresh) = FjallTasksDB::open(path);
            assert!(!is_fresh);
            let tasks = db.load_tasks().unwrap();
            assert_eq!(tasks.len(), 0);
        }
    }

    // Verify putting a single task into a fresh db, closing it and reopening it, and getting it out
    #[test]
    fn save_load() {
        let task_id = 0;
        let so = ServerOptions {
            bg_seconds: 0,
            bg_ticks: 0,
            fg_seconds: 0,
            fg_ticks: 0,
            max_stack_depth: 0,
        };

        /*
         perms: Objid,
        server_options: &ServerOptions,
        kill_switch: Arc<AtomicBool>,
         */
        let task = Task::new(
            task_id,
            SYSTEM_OBJECT,
            Arc::new(TaskStart::StartEval {
                player: SYSTEM_OBJECT,
                program: Default::default(),
            }),
            SYSTEM_OBJECT,
            &so,
            Arc::new(AtomicBool::new(false)),
        );

        // Mock task...
        let suspended = SuspendedTask {
            wake_condition: WakeCondition::Never,
            task,
            session: Arc::new(NoopClientSession::new()),
            result_sender: None,
        };
        let tmpdir = tempfile::tempdir().expect("Unable to create temporary directory");
        let path = tmpdir.path();

        {
            let (db, is_fresh) = FjallTasksDB::open(path);
            assert!(is_fresh);
            db.save_task(&suspended).unwrap();
            let tasks = db.load_tasks().unwrap();
            assert_eq!(tasks.len(), 1);
            assert_eq!(tasks[0].task.task_id, task_id);
        }

        {
            let (db, is_fresh) = FjallTasksDB::open(path);
            assert!(!is_fresh);
            let tasks = db.load_tasks().unwrap();
            assert_eq!(tasks.len(), 1);
            assert_eq!(tasks[0].task.task_id, task_id);
        }
    }

    // Create a series of tasks, save them, load them, and verify they are the same.
    #[test]
    fn save_load_multiple() {
        let mut tasks = vec![];
        for task_id in 0..50 {
            let so = ServerOptions {
                bg_seconds: 0,
                bg_ticks: 0,
                fg_seconds: 0,
                fg_ticks: 0,
                max_stack_depth: 0,
            };

            let task = Task::new(
                task_id,
                SYSTEM_OBJECT,
                Arc::new(TaskStart::StartEval {
                    player: SYSTEM_OBJECT,
                    program: Default::default(),
                }),
                SYSTEM_OBJECT,
                &so,
                Arc::new(AtomicBool::new(false)),
            );

            // Mock task...
            let suspended = SuspendedTask {
                wake_condition: WakeCondition::Never,
                task,
                session: Arc::new(NoopClientSession::new()),
                result_sender: None,
            };
            tasks.push(suspended);
        }

        // Write em
        let tmpdir = tempfile::tempdir().expect("Unable to create temporary directory");
        let path = tmpdir.path();
        {
            let (db, is_fresh) = FjallTasksDB::open(path);
            assert!(is_fresh);
            for task in tasks.iter() {
                db.save_task(task).unwrap();
            }
        }

        // Load em
        let (db, is_fresh) = FjallTasksDB::open(path);
        assert!(!is_fresh);
        let loaded_tasks = db.load_tasks().unwrap();
        assert_eq!(loaded_tasks.len(), tasks.len());
        for (task, loaded_task) in tasks.iter().zip(loaded_tasks.iter()) {
            assert_eq!(task.task.task_id, loaded_task.task.task_id);
        }
    }

    // Create a series of tasks, save them, delete a few, and load verify the rest are there and
    // the deleted are not.
    #[test]
    fn save_delete_load_multiple() {
        let mut tasks = vec![];
        for task_id in 0..50 {
            let so = ServerOptions {
                bg_seconds: 0,
                bg_ticks: 0,
                fg_seconds: 0,
                fg_ticks: 0,
                max_stack_depth: 0,
            };

            let task = Task::new(
                task_id,
                SYSTEM_OBJECT,
                Arc::new(TaskStart::StartEval {
                    player: SYSTEM_OBJECT,
                    program: Default::default(),
                }),
                SYSTEM_OBJECT,
                &so,
                Arc::new(AtomicBool::new(false)),
            );

            // Mock task...
            let suspended = SuspendedTask {
                wake_condition: WakeCondition::Never,
                task,
                session: Arc::new(NoopClientSession::new()),
                result_sender: None,
            };
            tasks.push(suspended);
        }

        // Write em
        let tmpdir = tempfile::tempdir().expect("Unable to create temporary directory");
        let path = tmpdir.path();
        {
            let (db, is_fresh) = FjallTasksDB::open(path);
            assert!(is_fresh);
            for task in tasks.iter() {
                db.save_task(task).unwrap();
            }
        }

        {
            // Delete some
            let (db, is_fresh) = FjallTasksDB::open(path);
            assert!(!is_fresh);
            for task_id in 0..50 {
                if task_id % 2 == 0 {
                    db.delete_task(task_id).unwrap();
                }
            }
        }

        // Load em
        let (db, is_fresh) = FjallTasksDB::open(path);
        assert!(!is_fresh);
        let loaded_tasks = db.load_tasks().unwrap();
        assert_eq!(loaded_tasks.len(), 25);

        // Go through the loaded tasks and make sure the deleted ones are not there.
        for task in loaded_tasks.iter() {
            assert!(task.task.task_id % 2 != 0);
        }
    }
}
