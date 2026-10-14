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

use crate::{
    config::Config,
    tasks::{
        NoopTasksDb, TaskStart,
        registry::WakeCondition,
        schedule_q::{ScheduleKind, ScheduleOptions},
        scheduler::{ResumeAction, Scheduler},
        task::Task,
        task_control::TaskControl,
    },
};
use moor_common::{
    model::{
        WorldState, WorldStateError, WorldStateSource,
        loader::{LoaderInterface, SnapshotInterface},
    },
    tasks::{
        ConnectionDetails, NarrativeEvent, NoopClientSession, NoopSystemControl, SchedulerError,
        Session, SessionError,
    },
};
use moor_db::{Database, DatabaseConfig, GCInterface, SnapshotCallback, TxDB};
use moor_var::{List, Obj, SYSTEM_OBJECT, Symbol, Var, v_int};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, SystemTime},
};
use uuid::Uuid;

struct FailingDatabase {
    calls: AtomicUsize,
}

impl WorldStateSource for FailingDatabase {
    fn new_world_state(&self) -> Result<Box<dyn WorldState>, WorldStateError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Err(WorldStateError::DatabaseError(
            "injected transaction failure".into(),
        ))
    }

    fn checkpoint(&self) -> Result<(), WorldStateError> {
        unreachable!("wakeup must not checkpoint")
    }
}

impl Database for FailingDatabase {
    fn loader_client(&self) -> Result<Box<dyn LoaderInterface>, WorldStateError> {
        unreachable!("wakeup must not load objects")
    }

    fn create_snapshot(&self) -> Result<Box<dyn SnapshotInterface>, WorldStateError> {
        unreachable!("wakeup must not create a snapshot")
    }

    fn create_snapshot_async(&self, _: SnapshotCallback) -> Result<(), WorldStateError> {
        unreachable!("wakeup must not create a snapshot")
    }

    fn gc_interface(&self) -> Result<Box<dyn GCInterface>, WorldStateError> {
        unreachable!("wakeup must not start GC")
    }
}

#[derive(Clone, Copy)]
enum Failure {
    WakeTransaction,
    RetryTransaction,
    RetrySession,
}

#[test]
fn failed_wake_transaction_settles_task() {
    check_failed_wakeup(Failure::WakeTransaction);
}

#[test]
fn failed_retry_transaction_settles_task() {
    check_failed_wakeup(Failure::RetryTransaction);
}

#[test]
fn failed_retry_session_settles_task() {
    check_failed_wakeup(Failure::RetrySession);
}

fn check_failed_wakeup(failure: Failure) {
    let (database, _) = TxDB::try_open_temporary(DatabaseConfig::default()).unwrap();
    let scheduler = Scheduler::new(
        semver::Version::new(0, 0, 0),
        Box::new(database),
        Box::new(NoopTasksDb {}),
        Arc::new(Config::default()),
        Arc::new(NoopSystemControl::default()),
        None,
        None,
    );
    let mut lc = scheduler.lifecycle.lock();
    lc.state = crate::tasks::scheduler::lifecycle::SchedulerState::Running;
    let now = SystemTime::now();
    let interval = Duration::from_secs(60);
    let schedule_id = lc
        .schedule_q
        .add_every(
            interval,
            SYSTEM_OBJECT,
            Symbol::mk("tick"),
            List::mk_list(&[]),
            SYSTEM_OBJECT,
            SYSTEM_OBJECT,
            ScheduleOptions::for_kind(&ScheduleKind::Every { interval }),
            now,
        )
        .unwrap();
    lc.schedule_q.mark_fired(schedule_id, 10, now);

    let (send, recv) = flume::unbounded();
    for id in [10, 11] {
        let task = Task::new(
            id,
            SYSTEM_OBJECT,
            SYSTEM_OBJECT,
            TaskStart::StartEval {
                player: SYSTEM_OBJECT,
                program: Default::default(),
                initial_env: None,
            },
            &scheduler.server_options.load(),
            Arc::new(TaskControl::new()),
        );
        let registration = lc.task_q.register_task(id);
        let session: Arc<dyn Session> = if id == 10 && matches!(failure, Failure::RetrySession) {
            Arc::new(FailingRetrySession)
        } else {
            Arc::new(NoopClientSession::new())
        };
        lc.task_q.suspended.add_task(
            if id == 10 {
                WakeCondition::Never
            } else {
                WakeCondition::Task(10)
            },
            task,
            session,
            (id == 10).then(|| send.clone()),
            registration,
        );
    }
    drop(send);
    lc.task_q.deliver_message(10, v_int(42));
    let suspended = lc.task_q.suspended.remove_task(10).unwrap();
    let dispatch_database = FailingDatabase {
        calls: AtomicUsize::new(0),
    };
    match failure {
        Failure::WakeTransaction => {
            let result = lc.wake_suspended_task(
                suspended,
                ResumeAction::Return(v_int(0)),
                &scheduler,
                &dispatch_database,
                scheduler.builtin_registry.clone(),
                scheduler.config.clone(),
            );
            assert!(matches!(result, Err(SchedulerError::CouldNotStartTask)));
        }
        Failure::RetryTransaction | Failure::RetrySession => {
            lc.wake_retry_suspended_task(
                suspended,
                &scheduler,
                &dispatch_database,
                scheduler.builtin_registry.clone(),
                scheduler.config.clone(),
            );
        }
    }

    let expected_transactions = usize::from(!matches!(failure, Failure::RetrySession));
    assert_eq!(
        dispatch_database.calls.load(Ordering::Relaxed),
        expected_transactions
    );
    assert!(!scheduler.handle_task_exists(10));
    assert!(!lc.task_q.active.contains_key(&10));
    assert!(lc.task_q.suspended.get(10).is_none());
    assert_eq!(lc.task_q.mailbox_len(10), 0);
    assert!(matches!(
        recv.try_recv(),
        Ok((10, Err(SchedulerError::CouldNotStartTask)))
    ));
    assert!(matches!(
        recv.try_recv(),
        Err(flume::TryRecvError::Disconnected)
    ));
    assert_eq!(
        lc.task_q.suspended.pop_immediate_wake().map(|(id, _)| id),
        Some(11)
    );
    assert!(lc.task_q.suspended.pop_immediate_wake().is_none());
    assert!(scheduler.handle_task_exists(11));
    assert!(matches!(
        lc.task_q.settled_results.as_slice(),
        [(10, Err(SchedulerError::CouldNotStartTask))]
    ));

    lc.settle_schedule_firings();
    let entry = lc.schedule_q.info(schedule_id).unwrap();
    assert!(entry.running.is_empty());
    assert_eq!(entry.fault_count, 1);
    assert!(lc.schedule_q.schedule_for_task(10).is_none());
    assert!(lc.task_q.settled_results.is_empty());
}

struct FailingRetrySession;

impl Session for FailingRetrySession {
    fn commit(&self) -> Result<(), SessionError> {
        Ok(())
    }
    fn rollback(&self) -> Result<(), SessionError> {
        Ok(())
    }

    fn fork(self: Arc<Self>) -> Result<Arc<dyn Session>, SessionError> {
        panic!("retry must use fork_retry, not fork")
    }

    fn fork_retry(self: Arc<Self>) -> Result<Arc<dyn Session>, SessionError> {
        Err(SessionError::DeliveryError)
    }

    fn request_input(
        &self,
        player: Obj,
        _input_request_id: Uuid,
        _metadata: Option<Vec<(Symbol, Var)>>,
    ) -> Result<(), SessionError> {
        panic!("unexpected input request for player {player}")
    }

    fn send_event(&self, _player: Obj, _msg: Box<NarrativeEvent>) -> Result<(), SessionError> {
        Ok(())
    }

    fn log_event(&self, _player: Obj, _event: Box<NarrativeEvent>) -> Result<(), SessionError> {
        Ok(())
    }

    fn send_system_msg(&self, _player: Obj, _msg: &str) -> Result<(), SessionError> {
        Ok(())
    }

    fn notify_shutdown(&self, _msg: Option<String>) -> Result<(), SessionError> {
        Ok(())
    }

    fn connection_name(&self, player: Obj) -> Result<String, SessionError> {
        Ok(format!("player-{player}"))
    }
    fn disconnect(&self, _player: Obj) -> Result<(), SessionError> {
        Ok(())
    }
    fn connected_players(&self, _include_all: bool) -> Result<Vec<Obj>, SessionError> {
        Ok(vec![])
    }

    fn connected_seconds(&self, _player: Obj) -> Result<f64, SessionError> {
        Ok(0.0)
    }

    fn idle_seconds(&self, _player: Obj) -> Result<f64, SessionError> {
        Ok(0.0)
    }

    fn connections(&self, _player: Option<Obj>) -> Result<Vec<Obj>, SessionError> {
        Err(SessionError::NoConnectionForPlayer(SYSTEM_OBJECT))
    }

    fn connection_details(
        &self,
        _player: Option<Obj>,
    ) -> Result<Vec<ConnectionDetails>, SessionError> {
        Err(SessionError::NoConnectionForPlayer(SYSTEM_OBJECT))
    }

    fn connection_attributes(&self, _obj: Obj) -> Result<Var, SessionError> {
        use moor_var::v_list;
        Ok(v_list(&[]))
    }

    fn set_connection_attribute(
        &self,
        _connection_obj: Obj,
        _key: Symbol,
        _value: Var,
    ) -> Result<(), SessionError> {
        Ok(())
    }
}

#[test]
fn immediate_wakes_wait_for_admission() {
    use crate::tasks::scheduler::{gc::GcPhase, lifecycle::SchedulerState};
    for (state, phase) in [
        (SchedulerState::Created, GcPhase::Idle),
        (SchedulerState::Stopping, GcPhase::Idle),
        (SchedulerState::Stopped, GcPhase::Idle),
        (SchedulerState::Running, GcPhase::Sweeping(1)),
    ] {
        let mut scheduler = crate::tasks::scheduler::test_support::scheduler();
        let database = Arc::new(FailingDatabase {
            calls: AtomicUsize::new(0),
        });
        scheduler.database = database.clone();
        let task = crate::tasks::scheduler::test_support::suspended_task(71).task;
        let (sender, results) = flume::unbounded();
        {
            let mut lc = scheduler.lifecycle.lock();
            lc.state = state;
            lc.gc_phase = phase;
            let registration = lc.task_q.register_task(71);
            lc.task_q.suspended.add_task(
                WakeCondition::Immediate(Some(v_int(42))),
                task,
                Arc::new(NoopClientSession::new()),
                Some(sender),
                registration,
            );
        }
        scheduler.drain_immediate_wakes();
        assert_eq!(
            database.calls.load(Ordering::Relaxed),
            0,
            "dispatch bypassed {state:?}/{phase:?}"
        );
        assert!(results.try_recv().is_err());
        assert!(scheduler.handle_task_exists(71));
        {
            let mut lc = scheduler.lifecycle.lock();
            assert!(lc.task_q.suspended.get(71).is_some());
            lc.state = SchedulerState::Running;
            lc.gc_phase = GcPhase::Idle;
        }
        scheduler.drain_immediate_wakes();
        assert_eq!(database.calls.load(Ordering::Relaxed), 1);
        assert!(matches!(
            results.try_recv(),
            Ok((71, Err(SchedulerError::CouldNotStartTask)))
        ));
        assert!(!scheduler.handle_task_exists(71));
    }
}

#[test]
fn expired_retry_uses_retry_session_preparation() {
    use crate::tasks::scheduler::lifecycle::SchedulerState;
    let mut scheduler = crate::tasks::scheduler::test_support::scheduler();
    let database = Arc::new(FailingDatabase {
        calls: AtomicUsize::new(0),
    });
    scheduler.database = database.clone();
    let task = crate::tasks::scheduler::test_support::suspended_task(71).task;
    let (sender, results) = flume::unbounded();
    {
        let mut lc = scheduler.lifecycle.lock();
        lc.state = SchedulerState::Running;
        let registration = lc.task_q.register_task(71);
        lc.task_q.suspended.add_task(
            WakeCondition::Retry(moor_common::util::Instant::ZERO),
            task,
            Arc::new(FailingRetrySession),
            Some(sender),
            registration,
        );
    }
    scheduler.drain_immediate_wakes();
    assert_eq!(
        database.calls.load(Ordering::Relaxed),
        0,
        "retry session fails before transaction creation"
    );
    assert!(matches!(
        results.try_recv(),
        Ok((71, Err(SchedulerError::CouldNotStartTask)))
    ));
    assert!(!scheduler.handle_task_exists(71));
}

#[derive(Clone, Copy, Debug)]
enum WakeSource {
    Input,
    WorkerValue,
    WorkerError,
    Explicit,
    Checkpoint,
    Compaction,
}

impl WakeSource {
    fn condition(self, request: Uuid) -> WakeCondition {
        match self {
            Self::Input => WakeCondition::Input(request),
            Self::WorkerValue | Self::WorkerError => WakeCondition::Worker(request),
            Self::Explicit => WakeCondition::Never,
            Self::Checkpoint => WakeCondition::Checkpoint(9),
            Self::Compaction => WakeCondition::StorageCompaction(9),
        }
    }

    fn deliver(self, scheduler: &Scheduler, request: Uuid) {
        use crate::tasks::workers::WorkerResponse;
        use moor_common::{
            model::{ObjFlag, TaskPermissions},
            tasks::WorkerError,
        };
        use moor_var::v_bool_int;
        match self {
            Self::Input => {
                scheduler
                    .submit_task_input_inner(SYSTEM_OBJECT, SYSTEM_OBJECT, request, v_int(42))
                    .unwrap();
            }
            Self::WorkerValue => scheduler.handle_worker_response(WorkerResponse::Response {
                request_id: request,
                response: v_int(42),
            }),
            Self::WorkerError => scheduler.handle_worker_response(WorkerResponse::Error {
                request_id: request,
                error: WorkerError::PermissionDenied("test denied".into()),
            }),
            Self::Explicit => assert_eq!(
                scheduler.handle_resume_task(
                    70,
                    71,
                    TaskPermissions::new(SYSTEM_OBJECT, ObjFlag::all_flags()),
                    v_int(42)
                ),
                v_bool_int(false),
            ),
            Self::Checkpoint => scheduler.handle_checkpoint_task_completion(71, 9, Ok(())),
            Self::Compaction => {
                scheduler.handle_storage_compaction_task_completion(71, 9, &[], Ok(vec![]))
            }
        }
    }
}

struct BlockedWake {
    scheduler: Scheduler,
    database: Arc<FailingDatabase>,
    request: Uuid,
    results: flume::Receiver<(
        usize,
        Result<crate::tasks::TaskNotification, SchedulerError>,
    )>,
}

impl BlockedWake {
    fn new(source: WakeSource) -> Self {
        use crate::tasks::scheduler::{gc::GcPhase, lifecycle::SchedulerState};
        let mut scheduler = crate::tasks::scheduler::test_support::scheduler();
        let database = Arc::new(FailingDatabase {
            calls: AtomicUsize::new(0),
        });
        scheduler.database = database.clone();
        let request = Uuid::new_v4();
        let task = crate::tasks::scheduler::test_support::suspended_task(71).task;
        let (sender, results) = flume::unbounded();
        {
            let mut lc = scheduler.lifecycle.lock();
            lc.state = SchedulerState::Running;
            lc.gc_phase = GcPhase::Sweeping(1);
            let registration = lc.task_q.register_task(71);
            lc.task_q.suspended.add_task(
                source.condition(request),
                task,
                Arc::new(NoopClientSession::new()),
                Some(sender),
                registration,
            );
        }
        Self {
            scheduler,
            database,
            request,
            results,
        }
    }

    fn open_admission(&self) {
        self.scheduler.lifecycle.lock().gc_phase = crate::tasks::scheduler::gc::GcPhase::Idle;
        self.scheduler.drain_immediate_wakes();
    }
}

fn check_direct_wake_admission(source: WakeSource) {
    let test = BlockedWake::new(source);
    source.deliver(&test.scheduler, test.request);
    assert_eq!(
        test.database.calls.load(Ordering::Relaxed),
        0,
        "{source:?} dispatched during sweep"
    );
    assert!(test.scheduler.handle_task_exists(71));
    assert!(test.results.try_recv().is_err());
    {
        let lc = test.scheduler.lifecycle.lock();
        assert!(lc.task_q.suspended.get(71).is_some());
        assert!(lc.task_q.active.is_empty());
    }
    test.open_admission();
    assert_eq!(test.database.calls.load(Ordering::Relaxed), 1);
    assert!(matches!(
        test.results.try_recv(),
        Ok((71, Err(SchedulerError::CouldNotStartTask)))
    ));
    assert!(!test.scheduler.handle_task_exists(71));
}

#[test]
fn input_waits_for_sweep_admission() {
    check_direct_wake_admission(WakeSource::Input);
}
#[test]
fn worker_value_waits_for_sweep_admission() {
    check_direct_wake_admission(WakeSource::WorkerValue);
}
#[test]
fn worker_error_waits_for_sweep_admission() {
    check_direct_wake_admission(WakeSource::WorkerError);
}
#[test]
fn explicit_resume_waits_for_sweep_admission() {
    check_direct_wake_admission(WakeSource::Explicit);
}
#[test]
fn checkpoint_waits_for_sweep_admission() {
    check_direct_wake_admission(WakeSource::Checkpoint);
}
#[test]
fn compaction_waits_for_sweep_admission() {
    check_direct_wake_admission(WakeSource::Compaction);
}

#[test]
fn duplicate_responses_cannot_replace_a_deferred_resume() {
    use moor_common::model::{ObjFlag, TaskPermissions};
    use moor_var::{E_INVARG, v_err};
    for source in [
        WakeSource::Input,
        WakeSource::WorkerValue,
        WakeSource::WorkerError,
        WakeSource::Explicit,
        WakeSource::Checkpoint,
        WakeSource::Compaction,
    ] {
        let test = BlockedWake::new(source);
        source.deliver(&test.scheduler, test.request);
        // The request index is consumed even though the task has not run yet.
        if matches!(source, WakeSource::Input) {
            assert!(matches!(
                test.scheduler.submit_task_input_inner(
                    SYSTEM_OBJECT,
                    SYSTEM_OBJECT,
                    test.request,
                    v_int(99)
                ),
                Err(SchedulerError::InputRequestNotFound(_))
            ));
        } else if !matches!(source, WakeSource::Explicit) {
            source.deliver(&test.scheduler, test.request);
        }
        assert_eq!(
            test.scheduler.handle_resume_task(
                70,
                71,
                TaskPermissions::new(SYSTEM_OBJECT, ObjFlag::all_flags()),
                v_int(99)
            ),
            v_err(E_INVARG)
        );
        assert_eq!(test.database.calls.load(Ordering::Relaxed), 0);
        test.open_admission();
        assert_eq!(test.database.calls.load(Ordering::Relaxed), 1);
        assert!(matches!(
            test.results.try_recv(),
            Ok((71, Err(SchedulerError::CouldNotStartTask)))
        ));
        assert!(test.results.try_recv().is_err());
    }
}

#[test]
fn shutdown_settles_deferred_responses_without_dispatch() {
    for source in [
        WakeSource::Input,
        WakeSource::WorkerValue,
        WakeSource::WorkerError,
        WakeSource::Explicit,
        WakeSource::Checkpoint,
        WakeSource::Compaction,
    ] {
        let test = BlockedWake::new(source);
        source.deliver(&test.scheduler, test.request);
        test.scheduler
            .lifecycle
            .lock()
            .task_q
            .deliver_message(71, v_int(7));
        test.scheduler.stop(None).unwrap();
        assert_eq!(test.database.calls.load(Ordering::Relaxed), 0);
        assert!(matches!(
            test.results.try_recv(),
            Ok((71, Err(SchedulerError::TaskAbortedCancelled)))
        ));
        assert!(!test.scheduler.handle_task_exists(71));
        let mut lc = test.scheduler.lifecycle.lock();
        assert!(lc.task_q.suspended.get(71).is_none());
        assert!(lc.task_q.drain_messages(71).is_empty());
        assert_eq!(lc.task_q.settled_results.len(), 1);
    }
}

#[test]
fn cancelling_a_deferred_response_prevents_later_dispatch() {
    let test = BlockedWake::new(WakeSource::WorkerError);
    WakeSource::WorkerError.deliver(&test.scheduler, test.request);
    assert!(matches!(
        test.scheduler.lifecycle.lock().task_q.abort_task(71),
        crate::tasks::AbortTaskOutcome::Cancelled
    ));
    test.open_admission();
    assert_eq!(test.database.calls.load(Ordering::Relaxed), 0);
    assert!(!test.scheduler.handle_task_exists(71));
    assert!(matches!(
        test.results.try_recv(),
        Err(flume::TryRecvError::Disconnected)
    ));
}

#[test]
fn direct_wakes_cannot_dispatch_after_shutdown() {
    use crate::tasks::scheduler::lifecycle::SchedulerState;
    use moor_common::model::{ObjFlag, TaskPermissions};
    use moor_var::{E_INVARG, v_err};
    for source in [
        WakeSource::Input,
        WakeSource::WorkerValue,
        WakeSource::WorkerError,
        WakeSource::Explicit,
        WakeSource::Checkpoint,
        WakeSource::Compaction,
    ] {
        for state in [
            SchedulerState::Created,
            SchedulerState::Stopping,
            SchedulerState::Stopped,
        ] {
            let test = BlockedWake::new(source);
            test.scheduler.lifecycle.lock().state = state;
            match source {
                WakeSource::Input => assert!(matches!(
                    test.scheduler.submit_task_input_inner(
                        SYSTEM_OBJECT,
                        SYSTEM_OBJECT,
                        test.request,
                        v_int(42)
                    ),
                    Err(SchedulerError::SchedulerNotResponding)
                )),
                WakeSource::Explicit => assert_eq!(
                    test.scheduler.handle_resume_task(
                        70,
                        71,
                        TaskPermissions::new(SYSTEM_OBJECT, ObjFlag::all_flags()),
                        v_int(42)
                    ),
                    v_err(E_INVARG)
                ),
                _ => source.deliver(&test.scheduler, test.request),
            }
            assert_eq!(test.database.calls.load(Ordering::Relaxed), 0);
            assert!(matches!(
                test.results.try_recv(),
                Ok((71, Err(SchedulerError::TaskAbortedCancelled)))
            ));
            assert!(!test.scheduler.handle_task_exists(71));
        }
    }
}

#[test]
fn deferred_returns_and_errors_reach_the_vm() {
    use crate::tasks::{TaskNotification, scheduler::gc::GcPhase};
    use moor_common::model::{ObjFlag, ObjectKind, TaskPermissions};
    use moor_var::{E_PERM, NOTHING, v_err};
    for (action, expected) in [
        (ResumeAction::Return(v_int(42)), v_int(42)),
        (
            ResumeAction::Raise(E_PERM.msg("deferred worker error")),
            v_err(E_PERM),
        ),
    ] {
        let scheduler = crate::tasks::scheduler::test_support::scheduler();
        let mut world = scheduler.database.new_world_state().unwrap();
        world
            .create_object(
                &TaskPermissions::new(SYSTEM_OBJECT, ObjFlag::all_flags()),
                &NOTHING,
                &SYSTEM_OBJECT,
                ObjFlag::all_flags(),
                ObjectKind::NextObjid,
            )
            .unwrap();
        world.commit().unwrap();
        scheduler.lifecycle.lock().state =
            crate::tasks::scheduler::lifecycle::SchedulerState::Running;
        let program = moor_compiler::compile(
            "try return suspend(600); except e (ANY) return e[1]; endtry",
            Default::default(),
        )
        .unwrap();
        let handle = scheduler
            .submit_eval_task_inner(
                SYSTEM_OBJECT,
                SYSTEM_OBJECT,
                program,
                None,
                Arc::new(NoopClientSession::new()),
            )
            .unwrap();
        assert!(matches!(
            handle.1.recv_timeout(Duration::from_secs(5)).unwrap().1,
            Ok(TaskNotification::Suspended)
        ));
        {
            let mut lc = scheduler.lifecycle.lock();
            lc.gc_phase = GcPhase::Sweeping(1);
            let task = lc.task_q.suspended.remove_task(handle.0).unwrap();
            lc.wake_suspended_task(
                task,
                action,
                &scheduler,
                scheduler.database.as_ref(),
                scheduler.builtin_registry.clone(),
                scheduler.config.clone(),
            )
            .unwrap();
            assert!(lc.task_q.active.is_empty());
        }
        assert!(handle.1.try_recv().is_err());
        scheduler.lifecycle.lock().gc_phase = GcPhase::Idle;
        scheduler.drain_immediate_wakes();
        match handle.1.recv_timeout(Duration::from_secs(5)).unwrap().1 {
            Ok(TaskNotification::Result(value)) => assert_eq!(value, expected),
            other => panic!("unexpected result: {other:?}"),
        }
        scheduler.stop(None).unwrap();
    }
}
