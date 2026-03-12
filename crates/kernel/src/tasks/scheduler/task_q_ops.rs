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

/// Result of submitting a new task - either already suspended (delayed/GC-blocked)
/// or needs immediate wake by the caller.
pub(super) enum TaskSubmission {
    /// Task is suspended with a delay or waiting for GC - no further action needed
    Suspended(TaskHandle),
    /// Task should start immediately - caller must wake it
    NeedsWake {
        handle: TaskHandle,
        task: Box<Task>,
        session: Arc<dyn Session>,
        result_sender: Option<Sender<(TaskId, Result<TaskNotification, SchedulerError>)>>,
    },
}

impl TaskQ {
    #[inline]
    pub(super) fn record_latency(counter: &PerfCounter, started_at: Instant) {
        counter.record_elapsed_from_with(PerfIntensity::HotPath, started_at);
    }

    #[inline]
    pub(super) fn wake_suspended_task(
        &mut self,
        suspended_task: SuspendedTask,
        resume_action: ResumeAction,
        scheduler: &Scheduler,
        database: &dyn Database,
        builtin_registry: BuiltinRegistry,
        config: Arc<Config>,
    ) -> Result<(), SchedulerError> {
        let SuspendedTask {
            task,
            session,
            result_sender,
            ..
        } = suspended_task;
        self.wake_task_thread(
            task,
            resume_action,
            session,
            result_sender,
            scheduler,
            database,
            builtin_registry,
            config,
        )
    }

    #[inline]
    pub(super) fn wake_retry_suspended_task(
        &mut self,
        suspended_task: SuspendedTask,
        scheduler: &Scheduler,
        database: &dyn Database,
        builtin_registry: BuiltinRegistry,
        config: Arc<Config>,
    ) {
        let SuspendedTask {
            task,
            session,
            result_sender,
            ..
        } = suspended_task;
        self.wake_retry_task(
            task,
            session,
            result_sender,
            scheduler,
            database,
            builtin_registry,
            config,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn submit_new_task(
        &mut self,
        task_id: TaskId,
        player: &Obj,
        perms: &Obj,
        task_start: TaskStart,
        delay_start: Option<Duration>,
        session: Arc<dyn Session>,
        server_options: &ServerOptions,
        gc_in_progress: bool,
    ) -> TaskSubmission {
        let perfc = sched_counters();
        let _t = PerfTimerGuard::new(&perfc.start_task);
        let (sender, receiver) = flume::unbounded();

        let kill_switch = Arc::new(AtomicBool::new(false));
        let task = Task::new(
            task_id,
            *player,
            *perms,
            task_start.clone(),
            server_options,
            kill_switch.clone(),
        );

        let handle = TaskHandle(task_id, receiver);

        // Delayed tasks go into suspension
        if let Some(delay) = delay_start {
            self.suspended.add_task(
                WakeCondition::Time(Deadline::from_now(delay).instant()),
                task,
                session,
                Some(sender),
            );
            return TaskSubmission::Suspended(handle);
        }

        // GC-blocked tasks go into suspension
        if gc_in_progress {
            self.suspended
                .add_task(WakeCondition::GCComplete, task, session, Some(sender));
            return TaskSubmission::Suspended(handle);
        }

        // Immediate start - return task directly, skip suspension queue entirely
        TaskSubmission::NeedsWake {
            handle,
            task,
            session,
            result_sender: Some(sender),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn wake_task_thread(
        &mut self,
        mut task: Box<Task>,
        resume_action: ResumeAction,
        session: Arc<dyn Session>,
        result_sender: Option<Sender<(TaskId, Result<TaskNotification, SchedulerError>)>>,
        scheduler: &Scheduler,
        database: &dyn Database,
        builtin_registry: BuiltinRegistry,
        config: Arc<Config>,
    ) -> Result<(), SchedulerError> {
        let perfc = sched_counters();
        let _t = PerfTimerGuard::new(&perfc.resume_task);

        // Start its new transaction...
        let world_state = match database.new_world_state() {
            Ok(ws) => ws,
            Err(e) => {
                error!(error = ?e, "Could not start transaction for task resumption due to DB error");
                return Err(SchedulerError::CouldNotStartTask);
            }
        };

        let task_id = task.task_id;
        let player = task.perms;

        // Brand new kill switch for the resumed task.
        let kill_switch = Arc::new(AtomicBool::new(false));
        task.kill_switch = kill_switch.clone();
        let task_control = RunningTask {
            player,
            kill_switch,
            session: session.clone(),
            result_sender,
            task_start: task.state.task_start().clone(),
        };

        self.active.insert(task_id, task_control);

        let scheduler_clone = scheduler.clone();
        let task_scheduler_client = TaskSchedulerClient::new(task_id, scheduler.clone());

        // Check if this is a brand new task or a resuming task
        let is_created = matches!(task.state, crate::tasks::task::TaskState::Pending(_));

        let wake_to_dispatch_started_at = Instant::now();
        let dispatch_started_at = Instant::now();
        self.thread_pool.spawn(move || {
            let panic_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let perfc = sched_counters();
                Self::record_latency(
                    &perfc.task_wake_to_dispatch_latency,
                    wake_to_dispatch_started_at,
                );
                Self::record_latency(&perfc.task_thread_handoff_latency, dispatch_started_at);

                if is_created {
                    Self::record_latency(
                        &perfc.task_submit_to_first_run_latency,
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
                scheduler_clone.handle_task_abort_panicked(
                    task_id,
                    panic_msg,
                    Box::new(backtrace),
                );
            }
        });

        Ok(())
    }

    pub(super) fn send_task_result(
        &mut self,
        task_id: TaskId,
        result: Result<Var, SchedulerError>,
    ) {
        let Some(mut task_control) = self.active.remove(&task_id) else {
            warn!(task_id, "Task not found for notification, ignoring");
            return;
        };
        self.suspended.enqueue_dependents_for(task_id);
        let result_sender = task_control.result_sender.take();
        Self::send_task_result_direct(task_id, result_sender, result);
    }

    /// Send task result directly with an explicit result_sender (for tasks not in active queue)
    pub(super) fn send_task_result_direct(
        task_id: TaskId,
        result_sender: Option<Sender<(TaskId, Result<TaskNotification, SchedulerError>)>>,
        result: Result<Var, SchedulerError>,
    ) {
        let Some(result_sender) = result_sender else {
            warn!(
                task_id,
                "Task not found for (direct) notification, ignoring"
            );
            return;
        };
        let result = result.map(|v| TaskNotification::Result(v.clone()));
        result_sender.send((task_id, result)).ok();
    }

    /// Wake a task that was suspended for retry backoff
    #[allow(clippy::too_many_arguments)]
    pub(super) fn wake_retry_task(
        &mut self,
        mut task: Box<Task>,
        session: Arc<dyn Session>,
        result_sender: Option<Sender<(TaskId, Result<TaskNotification, SchedulerError>)>>,
        scheduler: &Scheduler,
        database: &dyn Database,
        builtin_registry: BuiltinRegistry,
        config: Arc<Config>,
    ) {
        let perfc = sched_counters();
        let _t = PerfTimerGuard::new(&perfc.retry_task);

        let task_id = task.task_id;

        // Restore the VM state from its last snapshot
        task.vm_host.restore_state(&task.retry_state);
        task.reclaim_program_cache();
        task.vm_host.reset_time();

        // Fork the session for the new attempt
        let new_session = session.fork().unwrap();

        // Brand new kill switch for the retried task
        let kill_switch = Arc::new(AtomicBool::new(false));
        task.kill_switch = kill_switch.clone();

        let task_control = RunningTask {
            player: task.player,
            kill_switch,
            session: new_session.clone(),
            result_sender,
            task_start: task.state.task_start().clone(),
        };

        self.active.insert(task_id, task_control);

        let scheduler_clone = scheduler.clone();

        let world_state = match database.new_world_state() {
            Ok(ws) => ws,
            Err(e) => {
                panic!("Could not start transaction for retry wake task due to DB error: {e:?}");
            }
        };
        let task_scheduler_client = TaskSchedulerClient::new(task_id, scheduler.clone());
        let player = task.player;
        let wake_to_dispatch_started_at = Instant::now();
        let dispatch_started_at = Instant::now();
        self.thread_pool.spawn(move || {
            let panic_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let perfc = sched_counters();
                Self::record_latency(
                    &perfc.task_wake_to_dispatch_latency,
                    wake_to_dispatch_started_at,
                );
                Self::record_latency(&perfc.task_thread_handoff_latency, dispatch_started_at);

                let _tx_guard = TaskGuard::new(
                    world_state,
                    task_scheduler_client.clone(),
                    task_id,
                    player,
                    new_session.clone(),
                );

                info!(
                    ?task_id,
                    retries = task.retries,
                    "Waking retry task from suspension"
                );
                Task::run_task_loop(
                    task,
                    &task_scheduler_client,
                    new_session,
                    builtin_registry,
                    config,
                );
            }));

            if let Err(panic_payload) = panic_result {
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
                    "Retry task thread panicked"
                );

                scheduler_clone.handle_task_abort_panicked(
                    task_id,
                    panic_msg,
                    Box::new(backtrace),
                );
            }
        });
    }

    pub(super) fn kill_task(&mut self, victim_task_id: TaskId, sender_permissions: Perms) -> Var {
        let perfc = sched_counters();
        let _t = PerfTimerGuard::new(&perfc.kill_task);

        let is_suspended = if self.suspended.tasks.contains_key(&victim_task_id) {
            let is_wizard = sender_permissions
                .check_is_wizard()
                .expect("Could not check wizard status for kill request");
            if !is_wizard
                && !self
                    .suspended
                    .perms_check(victim_task_id, sender_permissions.who, false)
            {
                return v_err(E_PERM);
            }
            true
        } else if self.active.contains_key(&victim_task_id) {
            let tc = self.active.get(&victim_task_id).unwrap();
            if !sender_permissions
                .check_is_wizard()
                .expect("Could not check wizard status for kill request")
                && sender_permissions.who != tc.player
            {
                return v_err(E_PERM);
            }
            false
        } else {
            return v_err(E_INVARG);
        };

        if is_suspended {
            if self
                .suspended
                .remove_task_terminal(victim_task_id)
                .is_none()
            {
                error!(
                    task = victim_task_id,
                    "Task not found in suspended list for kill request"
                );
            }
            return v_bool_int(false);
        }

        let victim_task = match self.active.remove(&victim_task_id) {
            Some(victim_task) => victim_task,
            None => {
                return v_err(E_INVARG);
            }
        };
        self.suspended.enqueue_dependents_for(victim_task_id);
        victim_task.kill_switch.store(true, Ordering::SeqCst);
        v_bool_int(false)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn resume_task(
        &mut self,
        requesting_task_id: TaskId,
        queued_task_id: TaskId,
        sender_permissions: Perms,
        return_value: Var,
        scheduler: &Scheduler,
        database: &dyn Database,
        builtin_registry: BuiltinRegistry,
        config: Arc<Config>,
    ) -> Var {
        if requesting_task_id == queued_task_id {
            error!(
                task = requesting_task_id,
                "Task requested to resume itself. Ignoring"
            );
            return v_err(E_INVARG);
        }

        if !self
            .suspended
            .perms_check(queued_task_id, sender_permissions.who, true)
        {
            if !sender_permissions
                .check_is_wizard()
                .expect("Could not check wizard status for resume request")
            {
                return v_err(E_PERM);
            }
            if !self.suspended.tasks.contains_key(&queued_task_id) {
                error!(task = queued_task_id, "Task not found for resume request");
                return v_err(E_INVARG);
            }
        }

        let sr = self.suspended.remove_task(queued_task_id).unwrap();

        if self
            .wake_suspended_task(
                sr,
                ResumeAction::Return(return_value),
                scheduler,
                database,
                builtin_registry,
                config,
            )
            .is_err()
        {
            error!(task = queued_task_id, "Could not resume task");
            return v_err(E_INVARG);
        }
        v_bool_int(false)
    }

    pub(super) fn disconnect_task(&mut self, disconnect_task_id: TaskId, player: &Obj) {
        let Some(task) = self.active.get_mut(&disconnect_task_id) else {
            warn!(task = disconnect_task_id, "Disconnecting task not found");
            return;
        };
        warn!(?player, ?disconnect_task_id, "Disconnecting player");
        if let Err(e) = task.session.disconnect(*player) {
            warn!(?player, ?disconnect_task_id, error = ?e, "Could not disconnect player's session");
            return;
        }

        for (task_id, tc) in self.active.iter() {
            if *task_id == disconnect_task_id {
                continue;
            }
            if tc.player.eq(player) {
                continue;
            }
            warn!(
                ?player,
                task_id, "Aborting task from disconnected player..."
            );
            tc.kill_switch.store(true, Ordering::SeqCst);
        }
        self.suspended.prune_foreground_tasks(player);
    }
}
