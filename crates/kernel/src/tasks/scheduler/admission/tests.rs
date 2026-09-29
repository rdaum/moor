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

use crate::tasks::scheduler::*;
use crate::tasks::{TasksDbError, registry::SuspendedTask};
use moor_common::tasks::{NoopClientSession, NoopSystemControl, TaskId};
use moor_db::{DatabaseConfig, TxDB};
use moor_var::SYSTEM_OBJECT;

use crate::tasks::scheduler::test_support::*;

struct LoadedTasksDb(Mutex<Option<Vec<SuspendedTask>>>);

impl TasksDb for LoadedTasksDb {
    fn load_tasks(&self) -> Result<Vec<SuspendedTask>, TasksDbError> {
        Ok(self.0.lock().take().unwrap())
    }

    fn save_task(&self, _task: &SuspendedTask) -> Result<(), TasksDbError> {
        Ok(())
    }

    fn delete_task(&self, _task_id: TaskId) -> Result<(), TasksDbError> {
        Ok(())
    }

    fn delete_all_tasks(&self) -> Result<(), TasksDbError> {
        Ok(())
    }

    fn compact(&self) {}
}

#[test]
fn fresh_task_ids_exclude_zero_sentinel() {
    let scheduler = scheduler();
    let threads = scheduler.start(Arc::new(NoopSessionFactory)).unwrap();
    let handle = scheduler
        .submit_command_task_inner(
            SYSTEM_OBJECT,
            SYSTEM_OBJECT,
            "look".to_string(),
            Arc::new(NoopClientSession::new()),
        )
        .unwrap();
    assert_ne!(handle.task_id(), 0);
    assert!(!scheduler.handle_task_exists(0));
    scheduler.stop(None).unwrap();
    threads.join().unwrap();
}

#[test]
fn restored_task_ids_advance_allocator() {
    let (database, _) = TxDB::try_open_temporary(DatabaseConfig::default()).unwrap();
    let tasks = vec![suspended_task(4), suspended_task(81)];
    let scheduler = Scheduler::new(
        semver::Version::new(0, 0, 0),
        Box::new(database),
        Box::new(LoadedTasksDb(Mutex::new(Some(tasks)))),
        Arc::new(Config::default()),
        Arc::new(NoopSystemControl::default()),
        None,
        None,
    );

    let threads = scheduler
        .start(Arc::new(NoopSessionFactory))
        .expect("scheduler should start");
    {
        let lifecycle = scheduler.lifecycle.lock();
        assert_eq!(lifecycle.next_task_id, 82);
        assert!(lifecycle.task_q.suspended.get(4).is_some());
        assert!(lifecycle.task_q.suspended.get(81).is_some());
    }
    assert!(scheduler.handle_task_exists(4));
    assert!(scheduler.handle_task_exists(81));

    scheduler.stop(None).expect("scheduler should stop");
    threads
        .join()
        .expect("all scheduler-owned threads should stop");
}

#[test]
fn lifecycle_rejects_work_before_start_and_after_stop() {
    let scheduler = scheduler();
    let client = scheduler.client().unwrap();
    let session = Arc::new(NoopClientSession::new());

    assert_eq!(scheduler.state(), SchedulerState::Created);
    assert_eq!(
        client.check_status(),
        Err(SchedulerError::SchedulerNotResponding)
    );
    assert!(matches!(
        client.submit_command_task(&SYSTEM_OBJECT, &SYSTEM_OBJECT, "look", session.clone()),
        Err(SchedulerError::SchedulerNotResponding)
    ));
    assert!(matches!(
        client.submit_eval_task(
            &SYSTEM_OBJECT,
            &SYSTEM_OBJECT,
            "not valid moo code".to_string(),
            None,
            session.clone(),
            Arc::new(crate::config::FeaturesConfig::default()),
        ),
        Err(SchedulerError::SchedulerNotResponding)
    ));

    let timer = scheduler
        .start(Arc::new(NoopSessionFactory))
        .expect("scheduler should start once");
    assert_eq!(scheduler.state(), SchedulerState::Running);
    assert_eq!(client.check_status(), Ok(()));
    assert!(matches!(
        scheduler.start(Arc::new(NoopSessionFactory)),
        Err(SchedulerError::SchedulerNotResponding)
    ));

    scheduler.stop(None).expect("scheduler should stop once");
    timer.join().expect("timer thread should stop");

    assert_eq!(scheduler.state(), SchedulerState::Stopped);
    assert_eq!(
        client.check_status(),
        Err(SchedulerError::SchedulerNotResponding)
    );
    assert!(matches!(
        client.submit_command_task(&SYSTEM_OBJECT, &SYSTEM_OBJECT, "look", session),
        Err(SchedulerError::SchedulerNotResponding)
    ));
    assert!(matches!(
        client.submit_eval_task(
            &SYSTEM_OBJECT,
            &SYSTEM_OBJECT,
            "not valid moo code".to_string(),
            None,
            Arc::new(NoopClientSession::new()),
            Arc::new(crate::config::FeaturesConfig::default()),
        ),
        Err(SchedulerError::SchedulerNotResponding)
    ));
    assert_eq!(
        scheduler.stop(None),
        Err(SchedulerError::SchedulerNotResponding)
    );
}
