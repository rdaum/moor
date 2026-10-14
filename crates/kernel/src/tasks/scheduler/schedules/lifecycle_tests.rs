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
use crate::tasks::{
    TaskStart, TasksDbError,
    registry::SuspendedTask,
    schedule_q::{Outcome, RetireReason, ScheduleEntry, ScheduleId},
    task::Task,
    task_control::TaskControl,
};
use moor_common::tasks::{NoopSystemControl, Session, SessionError, SessionFactory, TaskId};
use moor_db::{DatabaseConfig, TxDB};
use moor_var::{List, Obj, SYSTEM_OBJECT, Symbol, v_int, v_str};
use std::time::SystemTime;

use crate::tasks::scheduler::test_support::*;

#[test]
fn adaptive_return_invalidates_collected_expiry() {
    use crate::tasks::schedule_q::{OverlapPolicy, ScheduleKind, ScheduleOptions};

    struct UnexpectedSessionFactory;
    impl SessionFactory for UnexpectedSessionFactory {
        fn mk_background_session(
            self: Arc<Self>,
            _player: &Obj,
        ) -> Result<Arc<dyn Session>, SessionError> {
            panic!("a replaced expiry must not start a session");
        }
    }

    let scheduler = scheduler_with_system_control(Arc::new(NoopSystemControl::default()));
    let t0 = SystemTime::now();
    let i0 = std::time::Instant::now();
    let interval = Duration::from_secs(1);
    let (id, to_fire) = {
        let mut lc = scheduler.lifecycle.lock();
        lc.state = SchedulerState::Running;
        lc.bg_session_factory = Some(Arc::new(UnexpectedSessionFactory));
        let mut opts = ScheduleOptions::for_kind(&ScheduleKind::Every { interval });
        opts.adaptive = true;
        opts.overlap = OverlapPolicy::Concurrent;
        let q = &mut lc.schedule_q;
        q.expired(i0, t0);
        let id = q
            .add_every(
                interval,
                SYSTEM_OBJECT,
                Symbol::mk("tick"),
                List::mk_list(&[]),
                SYSTEM_OBJECT,
                SYSTEM_OBJECT,
                opts,
                t0,
            )
            .unwrap();
        assert_eq!(q.expired(i0 + interval, t0 + interval), vec![id]);
        q.mark_fired(id, 1, t0 + interval);
        let due = q.expired(i0 + interval * 2, t0 + interval * 2);
        assert_eq!(due, vec![id]);
        let to_fire = due
            .into_iter()
            .map(|id| (q.expiry(id).unwrap(), q.info(id).unwrap().clone()))
            .collect();
        (id, to_fire)
    };
    // A worker finishes while the timer has released the collection lock.
    scheduler.lifecycle.lock().schedule_q.complete(
        id,
        1,
        Outcome::Success(v_int(60)),
        t0 + interval * 2,
    );
    scheduler.fire_collected_schedules(to_fire, t0 + interval * 2);
    let lc = scheduler.lifecycle.lock();
    let entry = lc.schedule_q.info(id).unwrap();
    assert!(entry.running.is_empty());
    assert_eq!(entry.next_run, Some(t0 + Duration::from_secs(61)));
}

/// Tasks DB double holding only a schedule id high-water mark.
struct ScheduleIdTasksDb(Mutex<Option<ScheduleId>>);

impl TasksDb for ScheduleIdTasksDb {
    fn load_tasks(&self) -> Result<Vec<SuspendedTask>, TasksDbError> {
        Ok(vec![])
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

    fn load_next_schedule_id(&self) -> Result<Option<ScheduleId>, TasksDbError> {
        Ok(*self.0.lock())
    }

    fn save_next_schedule_id(&self, next_id: ScheduleId) -> Result<(), TasksDbError> {
        *self.0.lock() = Some(next_id);
        Ok(())
    }

    fn compact(&self) {}
}

#[test]
fn restored_schedule_id_mark_advances_allocator() {
    let (database, _) = TxDB::try_open_temporary(DatabaseConfig::default()).unwrap();
    let scheduler = Scheduler::new(
        semver::Version::new(0, 0, 0),
        Box::new(database),
        Box::new(ScheduleIdTasksDb(Mutex::new(Some(42)))),
        Arc::new(Config::default()),
        Arc::new(NoopSystemControl::default()),
        None,
        None,
    );

    let threads = scheduler
        .start(Arc::new(NoopSessionFactory))
        .expect("scheduler should start");
    {
        let mut lifecycle = scheduler.lifecycle.lock();
        // No schedule records survived, yet allocation continues from
        // the persisted mark, and the new mark is written back.
        assert!(lifecycle.schedule_q.all_ids().is_empty());
        assert_eq!(lifecycle.reserve_schedule_id(), 42);
        let saved = lifecycle
            .task_q
            .suspended
            .tasks_db()
            .load_next_schedule_id()
            .unwrap();
        assert_eq!(saved, Some(43));
    }

    scheduler.stop(None).expect("scheduler should stop");
    threads
        .join()
        .expect("all scheduler-owned threads should stop");
}

/// A tasks database holding one restored suspended task and the
/// persisted schedules.
struct RestoredScheduleDb {
    tasks: Mutex<Option<Vec<SuspendedTask>>>,
    schedules: Vec<ScheduleEntry>,
}

impl TasksDb for RestoredScheduleDb {
    fn load_tasks(&self) -> Result<Vec<SuspendedTask>, TasksDbError> {
        Ok(self.tasks.lock().take().unwrap())
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

    fn load_schedules(&self) -> Result<Vec<ScheduleEntry>, TasksDbError> {
        Ok(self.schedules.clone())
    }

    fn compact(&self) {}
}

/// A one-shot that fired as task 12 and called `suspend()` before the
/// restart: its persisted deadline is overdue.
fn restored_firing_scheduler(schedule_id: ScheduleId, task_id: TaskId) -> Scheduler {
    use crate::tasks::schedule_q::{ScheduleKind, ScheduleOptions};
    let now = SystemTime::now();
    let fired_at = now - Duration::from_secs(60);
    let mut entry = ScheduleEntry::from_persisted(
        schedule_id,
        SYSTEM_OBJECT,
        Symbol::mk("tick"),
        List::mk_list(&[]),
        SYSTEM_OBJECT,
        SYSTEM_OBJECT,
        ScheduleKind::At,
        ScheduleOptions::for_kind(&ScheduleKind::At),
        fired_at,
        Some(fired_at),
        Some(fired_at),
        Some(fired_at),
        0,
        0,
        0,
        0,
        0,
        false,
    );
    entry.running.push(crate::tasks::schedule_q::RunningFiring {
        task: task_id,
        deadline: fired_at,
        started_at: fired_at,
    });
    let mut task = suspended_task(task_id);
    task.task = Task::new(
        task_id,
        SYSTEM_OBJECT,
        SYSTEM_OBJECT,
        TaskStart::StartScheduled {
            schedule_id,
            player: SYSTEM_OBJECT,
            vloc: moor_common::model::ObjectRef::Id(SYSTEM_OBJECT),
            verb: Symbol::mk("tick"),
            args: List::mk_list(&[]),
        },
        &ServerOptions {
            bg_seconds: 0.0,
            bg_ticks: 0,
            fg_seconds: 0.0,
            fg_ticks: 0,
            max_stack_depth: 0,
            dump_interval: None,
            gc_interval: None,
            max_task_retries: DEFAULT_MAX_TASK_RETRIES,
            max_task_mailbox: DEFAULT_MAX_TASK_MAILBOX,
            db_commit_queue_warn: DEFAULT_DB_COMMIT_QUEUE_WARN,
            db_commit_queue_timeout: DEFAULT_DB_COMMIT_QUEUE_TIMEOUT,
            rollback_on_task_limit: false,
        },
        Arc::new(TaskControl::new()),
    );
    let (database, _) = TxDB::try_open_temporary(DatabaseConfig::default()).unwrap();
    Scheduler::new(
        semver::Version::new(0, 0, 0),
        Box::new(database),
        Box::new(RestoredScheduleDb {
            tasks: Mutex::new(Some(vec![task])),
            schedules: vec![entry],
        }),
        Arc::new(Config::default()),
        Arc::new(NoopSystemControl::default()),
        None,
        None,
    )
}

#[test]
fn restored_firing_is_relinked_to_its_schedule() {
    let schedule_id: ScheduleId = 3;
    let task_id: TaskId = 12;
    let scheduler = restored_firing_scheduler(schedule_id, task_id);
    let threads = scheduler
        .start(Arc::new(NoopSessionFactory))
        .expect("scheduler should start");
    // Give the timer loop several ticks in which it could fire the
    // overdue deadline a second time.
    std::thread::sleep(Duration::from_millis(100));
    {
        let mut lc = scheduler.lifecycle.lock();
        assert_eq!(lc.schedule_q.schedule_for_task(task_id), Some(schedule_id));
        let entry = lc.schedule_q.info(schedule_id).unwrap();
        assert_eq!(
            entry.running.iter().map(|r| r.task).collect::<Vec<_>>(),
            vec![task_id]
        );
        assert!(entry.retired.is_none(), "{:?}", entry.retired);
        assert!(lc.schedule_q.is_valid(schedule_id));
        assert_eq!(lc.next_task_id, task_id + 1, "no second firing was started");

        // The restored task's result settles the one-shot.
        lc.task_q.settled_results.push((task_id, Ok(v_str("done"))));
        lc.settle_schedule_firings();
        let entry = lc.schedule_q.info(schedule_id).unwrap();
        assert_eq!(entry.retired, Some(RetireReason::OneShotDone));
        assert_eq!(entry.run_count, 1);
        assert!(entry.running.is_empty());
        assert_eq!(lc.schedule_q.schedule_for_task(task_id), None);
    }
    scheduler.stop(None).expect("scheduler should stop");
    threads
        .join()
        .expect("all scheduler-owned threads should stop");
}
