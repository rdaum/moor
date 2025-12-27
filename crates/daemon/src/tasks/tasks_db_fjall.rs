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
use moor_common::tasks::TaskId;
use moor_kernel::{
    SuspendedTask,
    tasks::{
        TasksDb, TasksDbError,
        convert_task::{suspended_task_from_ref, suspended_task_to_flatbuffer},
    },
};
use planus::{ReadAsRoot, WriteAsOffset};
use std::path::Path;
use tracing::error;

pub struct FjallTasksDB {
    keyspace: Keyspace,
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
                keyspace,
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

            // Deserialize FlatBuffer directly from ref to avoid copying
            let fb_task =
                moor_schema::task::SuspendedTaskRef::read_as_root(tasks_bytes).map_err(|e| {
                    error!("Failed to read FlatBuffer: {:?}", e);
                    TasksDbError::CouldNotLoadTasks
                })?;

            let task = suspended_task_from_ref(fb_task).map_err(|e| {
                error!("Failed to convert FlatBuffer to SuspendedTask: {:?}", e);
                TasksDbError::CouldNotLoadTasks
            })?;

            if task_id != task.task.task_id {
                panic!("Task ID mismatch: {:?} != {:?}", task_id, task.task.task_id);
            }
            tasks.push(task);
        }
        Ok(tasks)
    }

    fn save_task(&self, task: &SuspendedTask) -> Result<(), TasksDbError> {
        let task_id = task.task.task_id.to_le_bytes();

        // Convert to FlatBuffer
        let fb_task = suspended_task_to_flatbuffer(task).map_err(|e| {
            error!("Failed to convert task to FlatBuffer: {:?}", e);
            TasksDbError::CouldNotSaveTask
        })?;

        // Serialize to bytes using planus
        let mut builder = planus::Builder::new();
        let offset = fb_task.prepare(&mut builder);
        let task_bytes = builder.finish(offset, None);

        self.tasks_partition
            .insert(task_id, task_bytes)
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

    fn compact(&self) {
        if let Err(e) = self.keyspace.persist(fjall::PersistMode::SyncAll) {
            error!("Failed to compact tasks database: {:?}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::tasks::tasks_db_fjall::FjallTasksDB;
    use moor_common::tasks::NoopClientSession;
    use moor_kernel::tasks::DEFAULT_MAX_TASK_RETRIES;
    use moor_kernel::{
        SuspendedTask, Task, WakeCondition,
        tasks::{ServerOptions, TaskStart, TasksDb},
    };
    use moor_var::{SYSTEM_OBJECT, v_int};
    use std::{
        sync::{Arc, atomic::AtomicBool},
        time::Duration,
    };
    use uuid::Uuid;

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
            dump_interval: None,
            gc_interval: None,
            max_task_retries: DEFAULT_MAX_TASK_RETRIES,
        };

        /*
         perms: Objid,
        server_options: &ServerOptions,
        kill_switch: Arc<AtomicBool>,
         */
        let task = Task::new(
            task_id,
            SYSTEM_OBJECT,
            SYSTEM_OBJECT,
            TaskStart::StartEval {
                player: SYSTEM_OBJECT,
                program: Default::default(),
                initial_env: None,
            },
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
                dump_interval: None,
                gc_interval: None,
                max_task_retries: DEFAULT_MAX_TASK_RETRIES,
            };

            let task = Task::new(
                task_id,
                SYSTEM_OBJECT,
                SYSTEM_OBJECT,
                TaskStart::StartEval {
                    player: SYSTEM_OBJECT,
                    program: Default::default(),
                    initial_env: None,
                },
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
                dump_interval: None,
                gc_interval: None,
                max_task_retries: DEFAULT_MAX_TASK_RETRIES,
            };

            let task = Task::new(
                task_id,
                SYSTEM_OBJECT,
                SYSTEM_OBJECT,
                TaskStart::StartEval {
                    player: SYSTEM_OBJECT,
                    program: Default::default(),
                    initial_env: None,
                },
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

    // Test time-based wake conditions across save/load cycles
    #[test]
    fn test_time_wake_conditions() {
        let tmpdir = tempfile::tempdir().expect("Unable to create temporary directory");
        let path = tmpdir.path();

        let so = ServerOptions {
            bg_seconds: 0,
            bg_ticks: 0,
            fg_seconds: 0,
            fg_ticks: 0,
            max_stack_depth: 0,
            dump_interval: None,
            gc_interval: None,
            max_task_retries: DEFAULT_MAX_TASK_RETRIES,
        };

        // Create tasks with various time-based wake conditions
        let now = minstant::Instant::now();
        let input_uuid = Uuid::new_v4();

        let mut tasks = vec![];
        let test_cases = [
            ("future_5s", 0),
            ("future_1min", 1),
            ("future_1hr", 2),
            ("past_1s", 3),
            ("never", 4),
            ("input", 5),
            ("immediate", 6),
        ];

        for (name, i) in test_cases.iter() {
            let wake_condition = match *i {
                0 => WakeCondition::Time(now + Duration::from_secs(5)),
                1 => WakeCondition::Time(now + Duration::from_secs(60)),
                2 => WakeCondition::Time(now + Duration::from_secs(3600)),
                3 => WakeCondition::Time(now.checked_sub(Duration::from_secs(1)).unwrap_or(now)),
                4 => WakeCondition::Never,
                5 => WakeCondition::Input(input_uuid),
                6 => WakeCondition::Immediate(Some(v_int(0))),
                _ => unreachable!(),
            };

            let task = Task::new(
                *i,
                SYSTEM_OBJECT,
                SYSTEM_OBJECT,
                TaskStart::StartEval {
                    player: SYSTEM_OBJECT,
                    program: Default::default(),
                    initial_env: None,
                },
                &so,
                Arc::new(AtomicBool::new(false)),
            );

            let suspended = SuspendedTask {
                wake_condition,
                task,
                session: Arc::new(NoopClientSession::new()),
                result_sender: None,
            };
            tasks.push((*name, suspended));
        }

        // Save all tasks
        {
            let (db, is_fresh) = FjallTasksDB::open(path);
            assert!(is_fresh);
            for (_, task) in &tasks {
                db.save_task(task).unwrap();
            }
        }

        // Simulate time passing and reload
        std::thread::sleep(Duration::from_millis(10));

        // Load and verify
        {
            let (db, is_fresh) = FjallTasksDB::open(path);
            assert!(!is_fresh);
            let loaded_tasks = db.load_tasks().unwrap();
            assert_eq!(loaded_tasks.len(), tasks.len());

            // Verify each task type was preserved correctly
            for (original_name, original_task) in &tasks {
                let loaded_task = loaded_tasks
                    .iter()
                    .find(|t| t.task.task_id == original_task.task.task_id)
                    .unwrap_or_else(|| panic!("Could not find loaded task for {original_name}"));

                match (&original_task.wake_condition, &loaded_task.wake_condition) {
                    (WakeCondition::Time(_), WakeCondition::Time(_)) => {
                        // Time conditions should be preserved (though exact instant may differ slightly)
                        // This is expected due to the serialization round-trip
                    }
                    (WakeCondition::Never, WakeCondition::Never) => {}
                    (WakeCondition::Input(uuid1), WakeCondition::Input(uuid2)) => {
                        assert_eq!(uuid1, uuid2, "Input UUID mismatch for {original_name}");
                    }
                    (WakeCondition::Immediate(_), WakeCondition::Immediate(_)) => {}
                    _ => panic!(
                        "Wake condition type mismatch for {}: {:?} vs {:?}",
                        original_name, original_task.wake_condition, loaded_task.wake_condition
                    ),
                }
            }
        }
    }

    // Test edge cases for time serialization
    #[test]
    fn test_time_edge_cases() {
        let tmpdir = tempfile::tempdir().expect("Unable to create temporary directory");
        let _path = tmpdir.path();

        let so = ServerOptions {
            bg_seconds: 0,
            bg_ticks: 0,
            fg_seconds: 0,
            fg_ticks: 0,
            max_stack_depth: 0,
            dump_interval: None,
            gc_interval: None,
            max_task_retries: DEFAULT_MAX_TASK_RETRIES,
        };

        let now = minstant::Instant::now();

        // Test various edge cases
        let edge_cases = [
            now + Duration::from_secs(86400 * 365), // 1 year
            // Very near future
            now + Duration::from_millis(1),
            // Past times (if supported by the system)
            now.checked_sub(Duration::from_millis(1)).unwrap_or(now),
            now.checked_sub(Duration::from_secs(60)).unwrap_or(now),
        ];

        for (i, wake_time) in edge_cases.iter().enumerate() {
            let task = Task::new(
                i,
                SYSTEM_OBJECT,
                SYSTEM_OBJECT,
                TaskStart::StartEval {
                    player: SYSTEM_OBJECT,
                    program: Default::default(),
                    initial_env: None,
                },
                &so,
                Arc::new(AtomicBool::new(false)),
            );

            let suspended = SuspendedTask {
                wake_condition: WakeCondition::Time(*wake_time),
                task,
                session: Arc::new(NoopClientSession::new()),
                result_sender: None,
            };

            // Test save/load cycle for this edge case
            let (db, _) = FjallTasksDB::open(&tmpdir.path().join(format!("edge_case_{i}")));

            // Should not panic during save
            db.save_task(&suspended)
                .unwrap_or_else(|_| panic!("Failed to save edge case {i}"));

            // Should not panic during load
            let loaded_tasks = db
                .load_tasks()
                .unwrap_or_else(|_| panic!("Failed to load edge case {i}"));
            assert_eq!(loaded_tasks.len(), 1);

            // Should have correct wake condition type
            match &loaded_tasks[0].wake_condition {
                WakeCondition::Time(_) => {
                    // Success - time was preserved as a time condition
                }
                other => panic!("Edge case {i} changed wake condition type to {other:?}"),
            }
        }
    }

    // Test robustness against system clock changes
    #[test]
    fn test_clock_robustness() {
        let tmpdir = tempfile::tempdir().expect("Unable to create temporary directory");
        let path = tmpdir.path();

        let so = ServerOptions {
            bg_seconds: 0,
            bg_ticks: 0,
            fg_seconds: 0,
            fg_ticks: 0,
            max_stack_depth: 0,
            dump_interval: None,
            gc_interval: None,
            max_task_retries: DEFAULT_MAX_TASK_RETRIES,
        };

        // Create a task with a future wake time
        let task = Task::new(
            999,
            SYSTEM_OBJECT,
            SYSTEM_OBJECT,
            TaskStart::StartEval {
                player: SYSTEM_OBJECT,
                program: Default::default(),
                initial_env: None,
            },
            &so,
            Arc::new(AtomicBool::new(false)),
        );

        let wake_time = minstant::Instant::now() + Duration::from_secs(30);
        let suspended = SuspendedTask {
            wake_condition: WakeCondition::Time(wake_time),
            task,
            session: Arc::new(NoopClientSession::new()),
            result_sender: None,
        };

        // Save the task
        {
            let (db, _) = FjallTasksDB::open(path);
            db.save_task(&suspended).unwrap();
        }

        // Simulate some time passing (less than the wake time)
        std::thread::sleep(Duration::from_millis(100));

        // Load the task - should not panic even with time drift
        {
            let (db, _) = FjallTasksDB::open(path);
            let loaded_tasks = db.load_tasks().unwrap();
            assert_eq!(loaded_tasks.len(), 1);

            // Verify it's still a time-based wake condition
            match &loaded_tasks[0].wake_condition {
                WakeCondition::Time(_) => {
                    // Success - even with potential clock drift, we got a valid time back
                }
                other => panic!("Clock robustness test failed: got {other:?}"),
            }
        }
    }
}
