// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

//! An implementation of the TasksDb using Wiredtiger

use moor_db_wiredtiger::{
    Connection, CreateConfig, CursorConfig, DataSource, Datum, Error, Isolation, LogConfig,
    OpenConfig, SessionConfig, SyncMethod, TransactionSync,
};
use moor_kernel::tasks::{TasksDb, TasksDbError};
use moor_kernel::SuspendedTask;
use moor_values::tasks::TaskId;
use moor_values::BINCODE_CONFIG;
use std::path::Path;
use std::sync::Arc;
use tracing::error;

pub struct WiredTigerTasksDb {
    connection: Arc<Connection>,
    tasks_table: DataSource,
    session_config: SessionConfig,
}

impl WiredTigerTasksDb {
    pub fn open(path: Option<&Path>) -> (Self, bool) {
        let tmpdir = match path {
            Some(_path) => None,
            None => {
                let tmpdir = tempfile::tempdir().expect("Unable to create temporary directory");
                Some(tmpdir)
            }
        };
        let db_path = match path {
            Some(path) => path,
            None => {
                let path = tmpdir.as_ref().unwrap().path();
                path
            }
        };

        let transient = path.is_none();
        let mut options = OpenConfig::new()
            .create(true)
            .cache_size(1 << 30)
            .cache_cursors(true)
            .in_memory(transient);

        if !transient {
            std::fs::create_dir_all(db_path).expect("Failed to create database directory");
            options = options
                .log(LogConfig::new().enabled(true))
                .transaction_sync(
                    TransactionSync::new()
                        .enabled(true)
                        .method(SyncMethod::Fsync),
                );
        }
        let connection = Connection::open(db_path, options).unwrap();
        let session_config = SessionConfig::new().isolation(Isolation::Snapshot);
        let tasks_table = DataSource::Table("tasks".to_string());

        // Check for existence of tasks table, and if it's not there, create it.
        let check_session = connection
            .clone()
            .open_session(session_config.clone())
            .expect("Failed to open session");
        let is_fresh = if check_session.open_cursor(&tasks_table, None).is_err() {
            let config = CreateConfig::new().columns(&["task_id", "task"]);

            check_session
                .create(&tasks_table, Some(config))
                .expect("Failed to create tasks table");
            true
        } else {
            false
        };

        (
            Self {
                connection,
                tasks_table,
                session_config,
            },
            is_fresh,
        )
    }
}

impl TasksDb for WiredTigerTasksDb {
    fn load_tasks(&self) -> Result<Vec<SuspendedTask>, TasksDbError> {
        // Scan the entire tasks table, deserializing each record into a SuspendedTask
        let session = self
            .connection
            .clone()
            .open_session(self.session_config.clone())
            .map_err(|e| {
                error!("Failed to open session: {:?}", e);
                TasksDbError::CouldNotLoadTasks
            })?;

        session.begin_transaction(None).unwrap();
        let cursor = session
            .open_cursor(&self.tasks_table, Some(CursorConfig::new().raw(true)))
            .map_err(|e| {
                error!("Failed to open cursor: {:?}", e);
                TasksDbError::CouldNotLoadTasks
            })?;

        cursor.reset().map_err(|e| {
            error!("Failed to reset cursor to start: {:?}", e);
            TasksDbError::CouldNotLoadTasks
        })?;

        let mut tasks = vec![];
        loop {
            match cursor.next() {
                Ok(_) => {}
                Err(Error::NotFound) => {
                    break;
                }
                Err(e) => {
                    error!("Failed to advance cursor: {:?}", e);
                    return Err(TasksDbError::CouldNotLoadTasks);
                }
            }

            let Ok(task_id) = cursor.get_key() else {
                return Ok(tasks);
            };

            let task_id = TaskId::from_le_bytes(task_id.as_slice().try_into().unwrap());

            let Ok(task_bytes) = cursor.get_value() else {
                return Err(TasksDbError::CouldNotLoadTasks);
            };

            let (task, _): (SuspendedTask, usize) =
                bincode::decode_from_slice(task_bytes.as_slice(), *BINCODE_CONFIG)
                    .map_err(|e| {
                        error!("Failed to deserialize record: {:?}", e);
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
        let session = self
            .connection
            .clone()
            .open_session(self.session_config.clone())
            .map_err(|e| {
                error!("Failed to open session: {:?}", e);
                TasksDbError::CouldNotSaveTask
            })?;

        session.begin_transaction(None).unwrap();
        let cursor = session
            .open_cursor(&self.tasks_table, Some(CursorConfig::new().raw(true)))
            .map_err(|e| {
                error!("Failed to open cursor: {:?}", e);
                TasksDbError::CouldNotSaveTask
            })?;

        let task_id = task.task.task_id.to_le_bytes();
        let task_bytes = bincode::encode_to_vec(task, *BINCODE_CONFIG).map_err(|e| {
            error!("Failed to serialize record: {:?}", e);
            TasksDbError::CouldNotSaveTask
        })?;

        cursor
            .set_key(Datum::from_vec(task_id.to_vec()))
            .map_err(|e| {
                error!("Failed to set key: {:?}", e);
                TasksDbError::CouldNotSaveTask
            })?;
        cursor.set_value(Datum::from_vec(task_bytes)).map_err(|e| {
            error!("Failed to set value: {:?}", e);
            TasksDbError::CouldNotSaveTask
        })?;

        cursor.insert().map_err(|e| {
            error!("Failed to insert record: {:?}", e);
            TasksDbError::CouldNotSaveTask
        })?;

        session.commit().map_err(|e| {
            error!("Failed to commit transaction: {:?}", e);
            TasksDbError::CouldNotSaveTask
        })?;

        Ok(())
    }

    fn delete_task(&self, task_id: TaskId) -> Result<(), TasksDbError> {
        let session = self
            .connection
            .clone()
            .open_session(self.session_config.clone())
            .map_err(|e| {
                error!("Failed to open session: {:?}", e);
                TasksDbError::CouldNotDeleteTask
            })?;

        session.begin_transaction(None).unwrap();
        let cursor = session
            .open_cursor(&self.tasks_table, Some(CursorConfig::new().raw(true)))
            .map_err(|e| {
                error!("Failed to open cursor: {:?}", e);
                TasksDbError::CouldNotDeleteTask
            })?;

        let task_id = task_id.to_le_bytes();

        cursor
            .set_key(Datum::from_vec(task_id.to_vec()))
            .map_err(|e| {
                error!("Failed to set key: {:?}", e);
                TasksDbError::CouldNotDeleteTask
            })?;
        cursor.remove().map_err(|e| {
            error!("Failed to remove record: {:?}", e);
            TasksDbError::CouldNotDeleteTask
        })?;

        session.commit().map_err(|e| {
            error!("Failed to commit transaction: {:?}", e);
            TasksDbError::CouldNotDeleteTask
        })?;

        Ok(())
    }

    fn delete_all_tasks(&self) -> Result<(), TasksDbError> {
        // Scan the entire tasks table, deserializing each record into a SuspendedTask
        let session = self
            .connection
            .clone()
            .open_session(self.session_config.clone())
            .map_err(|e| {
                error!("Failed to open session: {:?}", e);
                TasksDbError::CouldNotDeleteTask
            })?;

        session.begin_transaction(None).unwrap();
        let cursor = session
            .open_cursor(&self.tasks_table, Some(CursorConfig::new().raw(true)))
            .map_err(|e| {
                error!("Failed to open cursor: {:?}", e);
                TasksDbError::CouldNotDeleteTask
            })?;

        cursor.reset().map_err(|e| {
            error!("Failed to reset cursor to start: {:?}", e);
            TasksDbError::CouldNotDeleteTask
        })?;

        loop {
            match cursor.next() {
                Ok(_) => {}
                Err(Error::NotFound) => {
                    break;
                }
                Err(e) => {
                    error!("Failed to advance cursor: {:?}", e);
                    return Err(TasksDbError::CouldNotDeleteTask);
                }
            }

            cursor.remove().map_err(|e| {
                error!("Failed to remove record: {:?}", e);
                TasksDbError::CouldNotDeleteTask
            })?;
        }

        session.commit().map_err(|e| {
            error!("Failed to commit transaction: {:?}", e);
            TasksDbError::CouldNotDeleteTask
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::tasks_wt::WiredTigerTasksDb;
    use moor_kernel::tasks::sessions::NoopClientSession;
    use moor_kernel::tasks::{ServerOptions, TaskStart, TasksDb};
    use moor_kernel::{SuspendedTask, Task, WakeCondition};
    use moor_values::SYSTEM_OBJECT;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    // Verify creation of an empty DB, including creation of tables.
    #[test]
    fn open_reopen() {
        let tmpdir = tempfile::tempdir().expect("Unable to create temporary directory");
        let path = tmpdir.path();
        {
            let (db, is_fresh) = WiredTigerTasksDb::open(Some(path));
            assert!(is_fresh);
            let tasks = db.load_tasks().unwrap();
            assert_eq!(tasks.len(), 0);
        }
        {
            let (db, is_fresh) = WiredTigerTasksDb::open(Some(path));
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
            let (db, is_fresh) = WiredTigerTasksDb::open(Some(path));
            assert!(is_fresh);
            db.save_task(&suspended).unwrap();
            let tasks = db.load_tasks().unwrap();
            assert_eq!(tasks.len(), 1);
            assert_eq!(tasks[0].task.task_id, task_id);
        }

        {
            let (db, is_fresh) = WiredTigerTasksDb::open(Some(path));
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
            let (db, is_fresh) = WiredTigerTasksDb::open(Some(path));
            assert!(is_fresh);
            for task in tasks.iter() {
                db.save_task(task).unwrap();
            }
        }

        // Load em
        let (db, is_fresh) = WiredTigerTasksDb::open(Some(path));
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
            let (db, is_fresh) = WiredTigerTasksDb::open(Some(path));
            assert!(is_fresh);
            for task in tasks.iter() {
                db.save_task(task).unwrap();
            }
        }

        {
            // Delete some
            let (db, is_fresh) = WiredTigerTasksDb::open(Some(path));
            assert!(!is_fresh);
            for task_id in 0..50 {
                if task_id % 2 == 0 {
                    db.delete_task(task_id).unwrap();
                }
            }
        }

        // Load em
        let (db, is_fresh) = WiredTigerTasksDb::open(Some(path));
        assert!(!is_fresh);
        let loaded_tasks = db.load_tasks().unwrap();
        assert_eq!(loaded_tasks.len(), 25);

        // Go through the loaded tasks and make sure the deleted ones are not there.
        for task in loaded_tasks.iter() {
            assert!(task.task.task_id % 2 != 0);
        }
    }
}
