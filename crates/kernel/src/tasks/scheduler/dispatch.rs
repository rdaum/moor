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

//! Transaction preparation and worker dispatch under the lifecycle lock.
//!
//! Admission or a resume transition supplies the registered task and owns acceptance policy.
//! Preparation opens the transaction before installing active metadata. The worker owns task
//! context and execution; its dispatch-bound client settles either completion or panic.

use super::{ResumeAction, Scheduler, lifecycle::TaskLifecycle};
use crate::tasks::task_telemetry::record_latency;
use crate::{
    config::Config,
    task_context::TaskGuard,
    tasks::{
        SchedulerOp, TaskNotification,
        registry::{LiveTaskRegistration, RunningTask, RunningTaskPhase},
        sched_counters,
        task::Task,
        task_control::TaskControl,
        task_scheduler_client::TaskSchedulerClient,
        task_telemetry::TaskRunBaseline,
    },
    vm::builtins::BuiltinRegistry,
};
use flume::Sender;
use moor_common::{
    tasks::{SchedulerError, Session, TaskId},
    util::Instant,
};
use moor_db::Database;
use std::sync::{Arc, OnceLock};
use tracing::error;

impl TaskLifecycle {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn dispatch_task(
        &mut self,
        mut task: Box<Task>,
        resume_action: ResumeAction,
        session: Arc<dyn Session>,
        result_sender: Option<Sender<(TaskId, Result<TaskNotification, SchedulerError>)>>,
        scheduler: &Scheduler,
        database: &dyn Database,
        builtin_registry: BuiltinRegistry,
        config: Arc<Config>,
        registration: LiveTaskRegistration,
    ) -> Result<(), SchedulerError> {
        let perfc = sched_counters();
        let _t = perfc.timers.start(SchedulerOp::ResumeTask);

        // Start its new transaction...
        let world_state = match database.new_world_state() {
            Ok(ws) => ws,
            Err(e) => {
                error!(error = ?e, "Could not start transaction for task resumption due to DB error");
                self.task_q
                    .finish_failed_wakeup(registration, result_sender);
                return Err(SchedulerError::CouldNotStartTask);
            }
        };

        let task_id = task.task_id;
        let player = task.player();

        let control = Arc::new(TaskControl::new());
        task.control = control.clone();
        let run_baseline = Arc::new(OnceLock::new());
        let task_control = RunningTask {
            registration,
            effects: Default::default(),
            phase: RunningTaskPhase::Running,
            player,
            control,
            session: session.clone(),
            result_sender,
            task_start: task.state.task_start().clone(),
            dispatched_at: Instant::now(),
            run_baseline: run_baseline.clone(),
            abort_error: None,
        };

        self.task_q.insert_active(task_id, task_control);

        let task_scheduler_client =
            TaskSchedulerClient::for_dispatch(task_id, scheduler.clone(), task.control.clone());

        // Check if this is a brand new task or a resuming task
        let is_created = matches!(task.state, crate::tasks::task::TaskState::Pending(_));

        let wake_to_dispatch_started_at = Instant::now();
        let dispatch_started_at = Instant::now();
        self.task_q.thread_pool.spawn(move || {
            run_baseline.set(TaskRunBaseline::capture()).ok();
            let panic_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let perfc = sched_counters();
                record_latency(
                    &perfc.timers,
                    SchedulerOp::TaskWakeToDispatchLatency,
                    wake_to_dispatch_started_at,
                );
                record_latency(
                    &perfc.timers,
                    SchedulerOp::TaskThreadHandoffLatency,
                    dispatch_started_at,
                );

                if is_created {
                    record_latency(
                        &perfc.timers,
                        SchedulerOp::TaskSubmitToFirstRunLatency,
                        task.creation_time,
                    );
                }

                // Set up transaction context for this thread
                let _tx_guard = TaskGuard::new(
                    world_state,
                    task_scheduler_client.clone(),
                    task_id,
                    player,
                    session.clone(),
                );

                if is_created {
                    // Brand new task - call setup_task_start and transition to Running
                    let setup_success = task.setup_task_start(&task_scheduler_client, &config);
                    if !setup_success {
                        // Setup failed (e.g., verb not found)
                        return;
                    }

                    // Transition to Running state
                    if let crate::tasks::task::TaskState::Pending(start) = &task.state {
                        task.state = crate::tasks::task::TaskState::Prepared(start.clone());
                    }

                    task.retry_state = task.vm_host.vm_exec_state().clone();
                } else {
                    // Resuming an existing task - handle the resume action
                    task.reclaim_program_cache();
                    match resume_action {
                        ResumeAction::Return(value) => {
                            task.vm_host.resume_execution(value);
                        }
                        ResumeAction::Raise(error) => {
                            task.vm_host.resume_with_error(error);
                        }
                    }
                    // A resumed transaction must retry with the delivered value or error,
                    // not the placeholder saved when its external wait began.
                    task.retry_state = task.vm_host.vm_exec_state().clone();
                }

                Task::run_task_loop(
                    task,
                    &task_scheduler_client,
                    session,
                    builtin_registry,
                    config,
                );
            }));

            if let Err(panic_payload) = panic_result {
                // Task thread panicked - extract panic message and log it
                let panic_msg = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "Task panicked with unknown payload".to_string()
                };

                let backtrace = std::backtrace::Backtrace::capture();
                error!(
                    task_id,
                    ?player,
                    panic_msg,
                    ?backtrace,
                    "Task thread panicked"
                );

                // Send panic abort directly to scheduler
                task_scheduler_client.abort_panicked(panic_msg, backtrace);
            }
        });

        Ok(())
    }
}
