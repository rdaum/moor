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

use super::*;
use crate::{
    config::Config,
    task_context::TaskGuard,
    tasks::{
        NoopTasksDb, TaskNotification,
        registry::{RunningTask, RunningTaskPhase},
        scheduler::{Scheduler, lifecycle::SchedulerState},
        task_control::TaskControl,
    },
};
use moor_common::{
    model::{ObjFlag, ObjectKind, TaskPermissions, VerbArgsSpec, VerbFlag, WorldStateSource},
    tasks::{CommandError, NoopClientSession, NoopSystemControl, SchedulerError, TaskId},
    util::{BitEnum, Instant},
};
use moor_compiler::{CompileOptions, compile};
use moor_db::{DatabaseConfig, TxDB};
use moor_var::{E_DIV, NOTHING, Symbol, program::ProgramType};
use std::sync::{Arc, OnceLock};

const TASK_ID: TaskId = 71;

struct DispatchTest {
    _guard: TaskGuard,
    scheduler: Scheduler,
    task: Box<Task>,
    client: TaskSchedulerClient,
    session: Arc<dyn Session>,
    results: flume::Receiver<(TaskId, Result<TaskNotification, SchedulerError>)>,
}

fn dispatch_test(unreadable_handler: bool) -> DispatchTest {
    let (db, _) = TxDB::try_open_temporary(DatabaseConfig::default()).unwrap();
    let permissions = TaskPermissions::new(SYSTEM_OBJECT, ObjFlag::all_flags());
    let mut world = db.new_world_state().unwrap();
    world
        .create_object(
            &permissions,
            &NOTHING,
            &SYSTEM_OBJECT,
            ObjFlag::all_flags(),
            ObjectKind::NextObjid,
        )
        .unwrap();
    let player = if unreadable_handler {
        let player = world
            .create_object(
                &permissions,
                &NOTHING,
                &SYSTEM_OBJECT,
                BitEnum::new(),
                ObjectKind::NextObjid,
            )
            .unwrap();
        world
            .add_verb(
                &permissions,
                &SYSTEM_OBJECT,
                vec![Symbol::mk("handle_uncaught_error")],
                &SYSTEM_OBJECT,
                BitEnum::new_with(VerbFlag::Exec),
                VerbArgsSpec::this_none_this(),
                ProgramType::MooR(compile("return 0;", CompileOptions::default()).unwrap()),
            )
            .unwrap();
        player
    } else {
        SYSTEM_OBJECT
    };
    world.commit().unwrap();

    let scheduler = Scheduler::new(
        semver::Version::new(0, 0, 0),
        Box::new(db),
        Box::new(NoopTasksDb {}),
        Arc::new(Config::default()),
        Arc::new(NoopSystemControl::default()),
        None,
        None,
    );
    let control = Arc::new(TaskControl::new());
    let task_start = TaskStart::StartEval {
        player,
        program: Default::default(),
        initial_env: None,
    };
    let mut task = Task::new(
        TASK_ID,
        player,
        player,
        task_start.clone(),
        scheduler.server_options.load().as_ref(),
        control.clone(),
    );
    let session: Arc<dyn Session> = Arc::new(NoopClientSession::new());
    let (sender, results) = flume::unbounded();
    {
        let mut lc = scheduler.lifecycle.lock();
        lc.state = SchedulerState::Running;
        let registration = lc.task_q.register_task(TASK_ID);
        lc.task_q.insert_active(
            TASK_ID,
            RunningTask {
                registration,
                effects: Default::default(),
                phase: RunningTaskPhase::Running,
                player,
                task_start,
                dispatched_at: Instant::now(),
                run_baseline: Arc::new(OnceLock::new()),
                abort_error: None,
                control,
                session: session.clone(),
                result_sender: Some(sender),
            },
        );
    }
    let client = TaskSchedulerClient::new(TASK_ID, scheduler.clone());
    let guard = TaskGuard::new(
        scheduler.database.new_world_state().unwrap(),
        client.clone(),
        TASK_ID,
        player,
        session.clone(),
    );
    task.refresh_authority_principal_flags();
    DispatchTest {
        _guard: guard,
        scheduler,
        task,
        client,
        session,
        results,
    }
}

fn assert_command_failure(
    scheduler: &Scheduler,
    results: &flume::Receiver<(TaskId, Result<TaskNotification, SchedulerError>)>,
) -> CommandError {
    let (_, result) = results
        .try_recv()
        .expect("dispatch error must settle the task");
    let Err(SchedulerError::CommandExecutionError(error)) = result else {
        panic!("expected command error, got {result:?}");
    };
    assert!(!scheduler.handle_task_exists(TASK_ID));
    assert!(
        !scheduler
            .lifecycle
            .lock()
            .task_q
            .active
            .contains_key(&TASK_ID)
    );
    assert!(results.try_recv().is_err(), "completion must occur once");
    error
}

fn assert_pending(
    scheduler: &Scheduler,
    results: &flume::Receiver<(TaskId, Result<TaskNotification, SchedulerError>)>,
) {
    assert!(
        results.try_recv().is_err(),
        "handler must not deliver a result"
    );
    assert!(scheduler.handle_task_exists(TASK_ID));
    assert_eq!(
        scheduler.lifecycle.lock().task_q.active[&TASK_ID].phase,
        RunningTaskPhase::Running
    );
}

fn finish_terminal(
    outcome: ExecutionOutcome,
    client: &TaskSchedulerClient,
    scheduler: &Scheduler,
    results: &flume::Receiver<(TaskId, Result<TaskNotification, SchedulerError>)>,
) {
    assert_pending(scheduler, results);
    let ExecutionOutcome::Finish(request) = outcome else {
        panic!("expected a terminal request");
    };
    request.finish(client);
}

#[test]
fn failed_command_fallback_does_not_return_continuation() {
    let mut test = dispatch_test(false);
    test.task.state = TaskState::Prepared(TaskStart::StartDoCommand {
        handler_object: SYSTEM_OBJECT,
        player: SYSTEM_OBJECT,
        command: "missing-command".to_string(),
    });
    let outcome = test.task.dispatch_success(v_int(0), test.session.as_ref());
    finish_terminal(outcome, &test.client, &test.scheduler, &test.results);
    assert!(matches!(
        assert_command_failure(&test.scheduler, &test.results),
        CommandError::NoCommandMatch
    ));
}

#[test]
fn missing_pending_exception_settles_the_task() {
    let mut test = dispatch_test(false);
    test.task.handling_uncaught_error = true;
    test.task.pending_exception = None;
    let outcome = test.task.dispatch_success(v_int(0), test.session.as_ref());
    finish_terminal(outcome, &test.client, &test.scheduler, &test.results);
    assert!(matches!(
        test.results.try_recv().unwrap().1,
        Err(SchedulerError::TaskAbortedCancelled)
    ));
    assert!(!test.scheduler.handle_task_exists(TASK_ID));
    assert!(
        !test
            .scheduler
            .lifecycle
            .lock()
            .task_q
            .active
            .contains_key(&TASK_ID)
    );
    assert!(test.results.try_recv().is_err());
}

#[test]
fn unreadable_exception_handler_settles_the_task() {
    let test = dispatch_test(true);
    let exception = Box::new(Exception {
        error: E_DIV.into(),
        stack: vec![],
        backtrace: vec![],
    });
    let outcome = test
        .task
        .dispatch_exception(exception, test.session.as_ref());
    finish_terminal(outcome, &test.client, &test.scheduler, &test.results);
    assert!(matches!(
        assert_command_failure(&test.scheduler, &test.results),
        CommandError::DatabaseError(WorldStateError::VerbPermissionDenied)
    ));
}

#[test]
fn terminal_success_waits_for_handoff_and_wins_late_cancellation() {
    let test = dispatch_test(false);
    let control = test.task.control.clone();
    let outcome = test.task.dispatch_success(v_int(42), test.session.as_ref());
    assert!(
        !crate::task_context::has_active_task(),
        "database commit already completed"
    );
    assert_eq!(
        control.request_cancel(),
        crate::tasks::task_control::CancelResult::Completing
    );
    finish_terminal(outcome, &test.client, &test.scheduler, &test.results);
    assert!(
        matches!(test.results.try_recv().unwrap().1, Ok(TaskNotification::Result(value)) if value == v_int(42))
    );
    assert!(!test.scheduler.handle_task_exists(TASK_ID));
    assert!(test.results.try_recv().is_err());
}

#[test]
fn cancellation_before_commit_returns_a_terminal_request() {
    let test = dispatch_test(false);
    test.task.control.request_cancel();
    let outcome = test.task.dispatch_success(v_int(42), test.session.as_ref());
    assert!(
        !crate::task_context::has_active_task(),
        "cancelled transaction was rolled back"
    );
    finish_terminal(outcome, &test.client, &test.scheduler, &test.results);
    assert!(matches!(
        test.results.try_recv().unwrap().1,
        Err(SchedulerError::TaskAbortedCancelled)
    ));
}

#[test]
fn suspension_transfers_registration_only_when_consumed() {
    let test = dispatch_test(false);
    let outcome =
        test.task
            .dispatch_suspend(TaskSuspend::Never, &test.client, test.session.as_ref());
    assert_pending(&test.scheduler, &test.results);
    let ExecutionOutcome::Suspend(request) = outcome else {
        panic!("expected suspension")
    };
    assert!(
        test.scheduler
            .lifecycle
            .lock()
            .task_q
            .suspended
            .get(TASK_ID)
            .is_none()
    );
    request.handoff(&test.client);
    let lc = test.scheduler.lifecycle.lock();
    assert!(!lc.task_q.active.contains_key(&TASK_ID));
    assert!(lc.task_q.suspended.get(TASK_ID).is_some());
    drop(lc);
    assert!(test.scheduler.handle_task_exists(TASK_ID));
    assert!(matches!(
        test.results.try_recv().unwrap().1,
        Ok(TaskNotification::Suspended)
    ));
}

#[test]
fn cancellation_between_boundary_and_handoff_prevents_suspension() {
    let test = dispatch_test(false);
    let control = test.task.control.clone();
    let outcome =
        test.task
            .dispatch_suspend(TaskSuspend::Never, &test.client, test.session.as_ref());
    assert_pending(&test.scheduler, &test.results);
    assert_eq!(
        control.request_cancel(),
        crate::tasks::task_control::CancelResult::AfterBoundary
    );
    let ExecutionOutcome::Suspend(request) = outcome else {
        panic!("expected suspension")
    };
    request.handoff(&test.client);
    assert!(!test.scheduler.handle_task_exists(TASK_ID));
    assert!(
        test.scheduler
            .lifecycle
            .lock()
            .task_q
            .suspended
            .get(TASK_ID)
            .is_none()
    );
    assert!(matches!(
        test.results.try_recv().unwrap().1,
        Err(SchedulerError::TaskAbortedCancelled)
    ));
}

#[test]
fn retry_keeps_active_registration_until_handoff() {
    let test = dispatch_test(false);
    let outcome = test.task.dispatch_retry(test.session.as_ref());
    assert_pending(&test.scheduler, &test.results);
    assert!(!crate::task_context::has_active_task());
    let ExecutionOutcome::Retry(request) = outcome else {
        panic!("expected retry")
    };
    request.handoff(&test.client);
    let lc = test.scheduler.lifecycle.lock();
    assert!(!lc.task_q.active.contains_key(&TASK_ID));
    let suspended = lc.task_q.suspended.get(TASK_ID).unwrap();
    assert!(matches!(
        suspended.wake_condition,
        crate::tasks::registry::WakeCondition::Retry(_)
    ));
    assert_eq!(suspended.task.retries, 1);
    drop(lc);
    assert!(test.scheduler.handle_task_exists(TASK_ID));
    assert!(test.results.try_recv().is_err());
}

#[test]
fn terminal_conflict_returns_retry_without_delivering_success() {
    let test = dispatch_test(false);
    let permissions = TaskPermissions::new(SYSTEM_OBJECT, ObjFlag::all_flags());
    with_current_transaction_mut(|world| {
        world.update_property(
            &permissions,
            &SYSTEM_OBJECT,
            Symbol::mk("name"),
            &v_str("first"),
        )
    })
    .unwrap();
    let mut winner = test.scheduler.database.new_world_state().unwrap();
    winner
        .update_property(
            &permissions,
            &SYSTEM_OBJECT,
            Symbol::mk("name"),
            &v_str("second"),
        )
        .unwrap();
    assert!(matches!(
        winner.commit().unwrap(),
        CommitResult::Success { .. }
    ));

    let outcome = test.task.dispatch_success(v_int(42), test.session.as_ref());
    assert_pending(&test.scheduler, &test.results);
    assert!(!crate::task_context::has_active_task());
    let ExecutionOutcome::Retry(request) = outcome else {
        panic!("expected retry after conflict")
    };
    request.handoff(&test.client);
    let lc = test.scheduler.lifecycle.lock();
    assert!(!lc.task_q.active.contains_key(&TASK_ID));
    assert_eq!(lc.task_q.suspended.get(TASK_ID).unwrap().task.retries, 1);
    assert!(test.results.try_recv().is_err());
}

#[test]
fn delayed_terminal_request_cannot_complete_a_replacement_dispatch() {
    let test = dispatch_test(false);
    let outcome = test.task.dispatch_success(v_int(42), test.session.as_ref());
    assert_pending(&test.scheduler, &test.results);
    let replacement = Arc::new(TaskControl::new());
    test.scheduler
        .lifecycle
        .lock()
        .task_q
        .active
        .get_mut(&TASK_ID)
        .unwrap()
        .control = replacement.clone();
    let ExecutionOutcome::Finish(request) = outcome else {
        panic!("expected completion")
    };
    request.finish(&test.client);
    assert_pending(&test.scheduler, &test.results);
    assert!(!replacement.is_cancelled());
}
