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

use std::backtrace::Backtrace;

use moor_common::tasks::{EventLogPurgeResult, EventLogStats, Exception, ListenerInfo};

use crate::tasks::{
    TaskDescription,
    task_scheduler_client::{ActiveTaskDescriptions, TimeoutHandlerInfo},
};

use super::*;

static HANDLE_TASK_TIMEOUT_SYM: LazyLock<Symbol> =
    LazyLock::new(|| Symbol::mk("handle_task_timeout"));

impl Scheduler {
    pub fn handle_task_success(
        &self,
        task_id: TaskId,
        value: Var,
        mutations_made: bool,
        timestamp: u64,
    ) {
        // Extract session under lock, then commit outside.
        let session = {
            let mut lc = self.lifecycle.lock();

            if mutations_made {
                lc.last_mutation_timestamp = Some(timestamp);
            }

            let Some(task) = lc.task_q.active.get_mut(&task_id) else {
                warn!(task_id, "Task not found for success");
                return;
            };
            task.session.clone()
        };

        // Session commit (potential I/O) outside the lock.
        if session.commit().is_err() {
            warn!("Could not commit session; aborting task");
            let mut lc = self.lifecycle.lock();
            lc.discard_pending_sends(task_id);
            return lc.task_q.send_task_result(task_id, Err(TaskAbortedError));
        }

        let mut lc = self.lifecycle.lock();
        lc.flush_pending_sends(task_id);
        lc.task_q.remove_message_queue(task_id);
        lc.task_q.send_task_result(task_id, Ok(value))
    }

    pub fn handle_task_conflict_retry(&self, task_id: TaskId, mut task: Box<Task>) {
        let perfc = sched_counters();
        let _t = PerfTimerGuard::new(&perfc.task_conflict_retry);

        let mut lc = self.lifecycle.lock();

        lc.discard_pending_sends(task_id);

        // Make sure the old thread is dead.
        task.kill_switch.store(true, Ordering::SeqCst);

        // Remove from active tasks to get session/result_sender
        let Some(old_tc) = lc.task_q.active.remove(&task_id) else {
            error!(
                task_id,
                "Task not found for retry suspension, ignoring -- consistency issue!"
            );
            return;
        };

        // If the number of retries has been exceeded, abort immediately
        if task.retries >= self.server_options.load().max_task_retries {
            error!(
                "Maximum number of retries exceeded for task {}.  Aborting.",
                task.task_id
            );
            TaskQ::send_task_result_direct(
                task_id,
                old_tc.result_sender,
                Err(TaskAbortedError),
            );
            return;
        }
        task.retries += 1;

        // Calculate backoff time: 10-50ms base, exponentially backed off
        let mut rng = rand::rng();
        let base_delay_ms = rng.random_range(10u64..=50u64);
        // Exponential backoff: base * 2^(retries-1)
        // Cap shift at 10 to prevent excessive delays (max multiplier 1024x)
        let shift = (task.retries as u32).saturating_sub(1).min(10);
        let delay_ms = base_delay_ms << shift;
        let wake_time = Deadline::from_now(Duration::from_millis(delay_ms)).instant();

        debug!(
            task_id,
            retries = task.retries,
            delay_ms,
            "Suspending task for retry backoff"
        );

        // Add to suspension queue with retry wake condition
        lc.task_q.suspended.add_task(
            WakeCondition::Retry(wake_time),
            task,
            old_tc.session,
            old_tc.result_sender,
        );
    }

    pub fn handle_task_verb_not_found(&self, task_id: TaskId, who: Var, what: Symbol) {
        let mut lc = self.lifecycle.lock();
        lc.task_q.send_task_result(
            task_id,
            Err(SchedulerError::TaskAbortedVerbNotFound(who, what)),
        );
    }

    pub fn handle_task_command_error(&self, task_id: TaskId, error: CommandError) {
        let mut lc = self.lifecycle.lock();
        // This is a common occurrence, so we don't want to log it at warn level.
        lc.task_q
            .send_task_result(task_id, Err(CommandExecutionError(error)));
    }

    pub fn handle_task_abort_cancelled(&self, task_id: TaskId) {
        let perfc = sched_counters();
        let _t = PerfTimerGuard::new(&perfc.task_abort_cancelled);

        warn!(?task_id, "Task cancelled");

        // Extract session and player under lock.
        let session = {
            let mut lc = self.lifecycle.lock();
            lc.discard_pending_sends(task_id);
            lc.task_q.remove_message_queue(task_id);

            let Some(task) = lc.task_q.active.get_mut(&task_id) else {
                warn!(task_id, "Task not found for abort");
                return;
            };
            let session = task.session.clone();
            let player = task.player;
            if let Err(send_error) = session.send_system_msg(player, "Aborted.") {
                warn!("Could not send abort message to player: {:?}", send_error);
            }
            session
        };

        // Session commit (potential I/O) outside the lock.
        if session.commit().is_err() {
            warn!("Could not commit aborted session; aborting task");
            let mut lc = self.lifecycle.lock();
            return lc.task_q.send_task_result(task_id, Err(TaskAbortedError));
        }

        let mut lc = self.lifecycle.lock();
        lc.task_q
            .send_task_result(task_id, Err(TaskAbortedCancelled));
    }

    pub fn handle_task_abort_panicked(
        &self,
        task_id: TaskId,
        panic_msg: String,
        _backtrace: Backtrace,
    ) {
        warn!(?task_id, ?panic_msg, "Task thread panicked");

        let mut lc = self.lifecycle.lock();

        lc.discard_pending_sends(task_id);
        lc.task_q.remove_message_queue(task_id);

        // Task already dead, can't access session. Just send error result directly.
        lc.task_q.send_task_result(task_id, Err(TaskAbortedError));
    }

    pub fn handle_task_abort_limits_reached(
        &self,
        task_id: TaskId,
        limit_reason: AbortLimitReason,
        this: Var,
        verb: Symbol,
        line_number: usize,
        handler_info: Box<TimeoutHandlerInfo>,
    ) {
        let perfc = sched_counters();
        let _t = PerfTimerGuard::new(&perfc.task_abort_limits);

        // Extract task and session under lock.
        let (mut task, session, player) = {
            let mut lc = self.lifecycle.lock();
            lc.discard_pending_sends(task_id);
            lc.task_q.remove_message_queue(task_id);

            let Some(task) = lc.task_q.active.remove(&task_id) else {
                warn!(task_id, "Task not found for abort");
                return;
            };
            let session = task.session.clone();
            let player = task.player;
            (task, session, player)
        };

        // Send abort notification and commit session outside the lock.
        let abort_reason_text = match limit_reason {
            AbortLimitReason::Ticks(t) => {
                warn!(?task_id, ticks = t, "Task aborted, ticks exceeded");
                format!(
                    "Abort: Task exceeded ticks limit of {t} @ {}:{verb}:{line_number}",
                    to_literal(&this)
                )
            }
            AbortLimitReason::Time(t) => {
                warn!(?task_id, time = ?t, "Task aborted, time exceeded");
                format!("Abort: Task exceeded time limit of {t:?}")
            }
        };

        if let Err(e) = session.send_system_msg(player, &abort_reason_text) {
            warn!("Could not send abort message to player: {e:?}");
        }

        let _ = session.commit();

        // Re-acquire lock for handler task submission.
        let mut lc = self.lifecycle.lock();

        // Attempt to invoke the handler verb as a separate task.
        let resource_str = match limit_reason {
            AbortLimitReason::Ticks(_) => "ticks",
            AbortLimitReason::Time(_) => "seconds",
        };

        let handler_args = List::from_iter(vec![
            v_str(resource_str),
            List::from_iter(handler_info.as_ref().stack.clone()).into(),
            List::from_iter(handler_info.as_ref().backtrace.clone()).into(),
        ]);

        let handler_task_start = TaskStart::StartVerb {
            player,
            vloc: v_obj(SYSTEM_OBJECT),
            verb: *HANDLE_TASK_TIMEOUT_SYM,
            args: handler_args,
            argstr: v_empty_str(),
        };

        let handler_task_id = lc.next_task_id;
        lc.next_task_id += 1;

        debug!(
            "Spawning handler task {} for timeout on task {}",
            handler_task_id, task_id
        );

        let handler_result = self.submit_task(
            &mut lc,
            handler_task_id,
            &player,
            &player,
            handler_task_start,
            None,
            session.clone().fork().unwrap_or_else(|_| session.clone()),
        );

        match handler_result {
            Ok(_) => {
                debug!("Handler task {} started successfully", handler_task_id);
            }
            Err(e) => {
                warn!("Failed to start handler task: {:?}", e);
            }
        }

        // Report the original task as aborted (handler outcome doesn't affect this)
        lc.task_q.suspended.enqueue_dependents_for(task_id);
        TaskQ::send_task_result_direct(
            task_id,
            task.result_sender.take(),
            Err(TaskAbortedLimit(limit_reason)),
        );
    }

    pub fn handle_task_exception(&self, task_id: TaskId, exception: Box<Exception>) {
        let perfc = sched_counters();
        let _t = PerfTimerGuard::new(&perfc.task_exception);

        // Extract session under lock, send traceback event.
        let session = {
            let lc = self.lifecycle.lock();
            let Some(task) = lc.task_q.active.get(&task_id) else {
                warn!(task_id, "Task not found for abort");
                return;
            };
            let session = task.session.clone();
            if let Err(send_error) = session.send_event(
                task.player,
                Box::new(NarrativeEvent {
                    event_id: Uuid::now_v7(),
                    timestamp: SystemTime::now(),
                    author: v_obj(task.player),
                    event: Event::Traceback(exception.as_ref().clone()),
                }),
            ) {
                warn!("Could not send traceback to player: {:?}", send_error);
            }
            session
        };

        // Session commit (potential I/O) outside the lock.
        let _ = session.commit();

        let mut lc = self.lifecycle.lock();
        lc.flush_pending_sends(task_id);
        lc.task_q.remove_message_queue(task_id);
        lc.task_q.send_task_result(
            task_id,
            Err(TaskAbortedException(exception.as_ref().clone())),
        );
    }

    pub fn handle_task_request_fork(
        &self,
        task_id: TaskId,
        fork_request: Box<Fork>,
    ) -> TaskId {
        let perfc = sched_counters();
        let _t = PerfTimerGuard::new(&perfc.fork_task);

        let mut lc = self.lifecycle.lock();

        // Task has requested a fork. Dispatch it and reply with the new task id.
        let new_session = {
            let Some(task) = lc.task_q.active.get_mut(&task_id) else {
                warn!(task_id, "Task not found for fork request");
                // Return a sentinel; caller should handle missing task.
                return 0;
            };
            task.session.clone()
        };

        // Fork the session.
        let forked_session = new_session.fork().unwrap();

        let suspended = fork_request.delay.is_some();
        let player = fork_request.player;
        let delay = fork_request.delay;
        let progr = fork_request.progr;

        let task_start = TaskStart::StartFork {
            fork_request,
            suspended,
        };
        let new_task_id = lc.next_task_id;
        lc.next_task_id += 1;
        if let Err(e) = self.submit_task(
            &mut lc,
            new_task_id,
            &player,
            &progr,
            task_start,
            delay,
            forked_session,
        ) {
            error!(?e, "Could not fork task");
        }

        new_task_id
    }

    pub fn handle_task_suspend(
        &self,
        task_id: TaskId,
        wake_condition: TaskSuspend,
        task: Box<Task>,
    ) {
        // Remove from active and extract session under lock.
        let tc = {
            let mut lc = self.lifecycle.lock();
            let Some(tc) = lc.task_q.active.remove(&task_id) else {
                warn!(task_id, "Task not found for suspend request");
                return;
            };
            tc
        };

        // Session commit (potential I/O) outside the lock.
        if tc.session.commit().is_err() {
            warn!("Could not commit session; aborting task");
            let mut lc = self.lifecycle.lock();
            lc.discard_pending_sends(task_id);
            return lc.task_q.send_task_result(task_id, Err(TaskAbortedError));
        }

        let mut lc = self.lifecycle.lock();
        lc.flush_pending_sends(task_id);

        // And insert into the suspended list.
        let wake_condition = match wake_condition {
            TaskSuspend::Never => WakeCondition::Never,
            TaskSuspend::Timed(t) => WakeCondition::Time(Deadline::from_now(t).instant()),
            TaskSuspend::WaitTask(task_id) => WakeCondition::Task(task_id),
            TaskSuspend::Commit(return_value) => {
                WakeCondition::Immediate(Some(return_value))
            }
            TaskSuspend::WorkerRequest(worker_type, args, timeout) => {
                let worker_request_id = Uuid::new_v4();
                // Send request to the worker process.
                // If no workers are configured, abort the task.
                let Some(workers_sender) = self.worker_request_send.as_ref() else {
                    warn!("No workers configured for scheduler; aborting task");
                    return lc.task_q.send_task_result(task_id, Err(TaskAbortedError));
                };

                if let Err(e) = workers_sender.send(WorkerRequest::Request {
                    request_id: worker_request_id,
                    request_type: worker_type,
                    perms: task.perms,
                    request: args,
                    timeout,
                }) {
                    error!(?e, "Could not send worker request; aborting task");
                    return lc.task_q.send_task_result(task_id, Err(TaskAbortedError));
                }

                WakeCondition::Worker(worker_request_id)
            }
            TaskSuspend::RecvMessages(Some(duration)) => {
                // Check if there are already messages in the queue after commit
                let messages = lc.task_q.drain_messages(task_id);
                if !messages.is_empty() {
                    // Messages available — wake immediately with them
                    WakeCondition::Immediate(Some(List::from_iter(messages).into()))
                } else {
                    // No messages — suspend with deadline, wake on message
                    // arrival or timeout
                    WakeCondition::TaskMessage(Deadline::from_now(duration).instant())
                }
            }
            TaskSuspend::RecvMessages(None) => {
                // Immediate fast path — drain queue and wake immediately
                let messages = lc.task_q.drain_messages(task_id);
                WakeCondition::Immediate(Some(List::from_iter(messages).into()))
            }
        };

        if !matches!(wake_condition, WakeCondition::Immediate(_))
            && let Some(sender) = tc.result_sender.as_ref()
        {
            let _ = sender.send((task_id, Ok(TaskNotification::Suspended)));
        }

        let needs_timer_wake = matches!(
            wake_condition,
            WakeCondition::Time(_) | WakeCondition::Retry(_) | WakeCondition::TaskMessage(_)
        );

        lc.task_q
            .suspended
            .add_task(wake_condition, task, tc.session, tc.result_sender);

        // Wake the timer thread so it can recompute its sleep duration for the
        // newly-inserted deadline.
        if needs_timer_wake {
            drop(lc);
            self.wake_timer_thread();
        }
    }

    pub fn handle_task_request_input(
        &self,
        task_id: TaskId,
        task: Box<Task>,
        metadata: Option<Vec<(Symbol, Var)>>,
    ) {
        let input_request_id = Uuid::new_v4();

        // Remove from active under lock.
        let tc = {
            let mut lc = self.lifecycle.lock();
            let Some(tc) = lc.task_q.active.remove(&task_id) else {
                warn!(task_id, "Task not found for input request");
                return;
            };
            tc
        };

        // Session commit (potential I/O) outside the lock — flushes output
        // up to the prompt point.
        if tc.session.commit().is_err() {
            warn!("Could not commit session; aborting task");
            let mut lc = self.lifecycle.lock();
            lc.discard_pending_sends(task_id);
            return lc.task_q.send_task_result(task_id, Err(TaskAbortedError));
        }

        if tc
            .session
            .request_input(tc.player, input_request_id, metadata)
            .is_err()
        {
            warn!("Could not request input from session; aborting task");
            let mut lc = self.lifecycle.lock();
            return lc.task_q.send_task_result(task_id, Err(TaskAbortedError));
        }

        let mut lc = self.lifecycle.lock();
        lc.flush_pending_sends(task_id);
        lc.task_q.suspended.add_task(
            WakeCondition::Input(input_request_id),
            task,
            tc.session,
            tc.result_sender,
        );
    }

    pub fn handle_request_tasks(&self, _task_id: TaskId) -> Vec<TaskDescription> {
        let lc = self.lifecycle.lock();
        lc.task_q.suspended.tasks()
        // TODO: add non-queued tasks.
    }

    pub fn handle_task_exists(&self, check_task_id: TaskId) -> Option<Obj> {
        let lc = self.lifecycle.lock();
        // Check both suspended and active tasks atomically
        lc.task_q.task_owner(check_task_id)
    }

    pub fn handle_kill_task(
        &self,
        _task_id: TaskId,
        victim_task_id: TaskId,
        sender_permissions: Perms,
    ) -> Var {
        let mut lc = self.lifecycle.lock();
        lc.task_q.kill_task(victim_task_id, sender_permissions)
    }

    pub fn handle_resume_task(
        &self,
        task_id: TaskId,
        queued_task_id: TaskId,
        sender_permissions: Perms,
        return_value: Var,
    ) -> Var {
        let mut lc = self.lifecycle.lock();
        lc.task_q.resume_task(
            task_id,
            queued_task_id,
            sender_permissions,
            return_value,
            self,
            self.database.as_ref(),
            self.builtin_registry.clone(),
            self.config.clone(),
        )
    }

    pub fn handle_boot_player(&self, task_id: TaskId, player: Obj) {
        let mut lc = self.lifecycle.lock();
        // Task is asking to boot a player.
        lc.task_q.disconnect_task(task_id, &player);
    }

    pub fn handle_notify(
        &self,
        task_id: TaskId,
        player: Obj,
        event: Box<NarrativeEvent>,
    ) {
        let mut lc = self.lifecycle.lock();
        // Task is asking to notify a player of an event.
        let Some(task) = lc.task_q.active.get_mut(&task_id) else {
            warn!(task_id, "Task not found for notify request");
            return;
        };
        let Ok(()) = task.session.send_event(player, event) else {
            warn!("Could not notify player; aborting task");
            return lc.task_q.send_task_result(task_id, Err(TaskAbortedError));
        };
    }

    pub fn handle_log_event(
        &self,
        task_id: TaskId,
        player: Obj,
        event: Box<NarrativeEvent>,
    ) {
        let mut lc = self.lifecycle.lock();
        // Task is asking to log an event without broadcasting.
        let Some(task) = lc.task_q.active.get_mut(&task_id) else {
            warn!(task_id, "Task not found for log_event request");
            return;
        };
        let Ok(()) = task.session.log_event(player, event) else {
            warn!("Could not log event; aborting task");
            return lc.task_q.send_task_result(task_id, Err(TaskAbortedError));
        };
    }

    pub fn handle_get_listeners(&self) -> Vec<ListenerInfo> {
        self.system_control
            .listeners()
            .expect("Could not get listeners")
    }

    pub fn handle_listen(
        &self,
        task_id: TaskId,
        handler_object: Obj,
        host_type: String,
        port: u16,
        options: Vec<(Symbol, Var)>,
    ) -> Option<Error> {
        let lc = self.lifecycle.lock();
        let Some(_task) = lc.task_q.active.get(&task_id) else {
            warn!(task_id, "Task not found for listen request");
            return Some(E_INVARG.msg("Task not found"));
        };
        drop(lc);

        self.system_control
            .listen(handler_object, &host_type, port, options)
            .err()
    }

    pub fn handle_unlisten(
        &self,
        task_id: TaskId,
        host_type: String,
        port: u16,
    ) -> Option<Error> {
        let lc = self.lifecycle.lock();
        let Some(_task) = lc.task_q.active.get(&task_id) else {
            warn!(task_id, "Task not found for unlisten request");
            return Some(E_INVARG.msg("Task not found"));
        };
        drop(lc);

        match self.system_control.unlisten(port, &host_type) {
            Ok(_) => None,
            Err(_) => Some(E_PERM.msg("Permission denied on unlisten")),
        }
    }

    pub fn handle_refresh_server_options(&self) {
        self.reload_server_options();
    }

    pub fn handle_shutdown(&self, msg: Option<String>) {
        info!("Shutting down scheduler. Reason: {msg:?}");
        self.stop(msg)
            .expect("Could not shutdown scheduler cleanly");
    }

    pub fn handle_force_input(
        &self,
        task_id: TaskId,
        who: Obj,
        line: String,
    ) -> Result<TaskId, Error> {
        let mut lc = self.lifecycle.lock();

        let new_session = {
            let Some(task) = lc.task_q.active.get_mut(&task_id) else {
                warn!(task_id, "Task not found for force input request");
                return Err(E_INVIND.msg("Task not found"));
            };
            task.session.clone().fork().unwrap()
        };
        let task_start = TaskStart::StartCommandVerb {
            handler_object: SYSTEM_OBJECT,
            player: who,
            command: line,
        };

        let new_task_id = lc.next_task_id;
        lc.next_task_id += 1;
        let result =
            self.submit_task(&mut lc, new_task_id, &who, &who, task_start, None, new_session);
        match result {
            Err(e) => {
                error!(?e, "Could not start task thread");
                Err(E_INVIND.with_msg(|| {
                    format!("Could not start thread for force_input: {e:?}")
                }))
            }
            Ok(th) => Ok(th.0),
        }
    }

    pub fn handle_active_tasks(&self, _task_id: TaskId) -> Result<ActiveTaskDescriptions, Error> {
        let lc = self.lifecycle.lock();
        let mut results = vec![];
        for (task_id, tc) in lc.task_q.active.iter() {
            results.push((*task_id, tc.player, tc.task_start.clone()));
        }
        Ok(results)
    }

    pub fn handle_checkpoint_from_task(
        &self,
        _task_id: TaskId,
        blocking: bool,
    ) -> Result<(), SchedulerError> {
        if blocking {
            self.checkpoint_blocking()
        } else {
            self.checkpoint()
        }
    }

    pub fn handle_task_send(
        &self,
        task_id: TaskId,
        target_task_id: TaskId,
        value: Var,
        sender_permissions: Perms,
    ) -> Var {
        let mut lc = self.lifecycle.lock();

        let Some(owner) = lc.task_q.task_owner(target_task_id) else {
            return v_error(E_INVARG.with_msg(|| {
                format!("Task ({target_task_id}) not found for task_send")
            }));
        };

        let is_wizard = sender_permissions
            .check_is_wizard()
            .expect("Could not check wizard status for task_send");
        if !is_wizard && sender_permissions.who != owner {
            return v_error(E_PERM.with_msg(|| {
                format!("Permission denied for task_send to task ({target_task_id})")
            }));
        }

        // Check mailbox size limit (committed queue + pending sends
        // from this task to same target)
        let committed_len = lc.task_q.mailbox_len(target_task_id);
        let pending_len = lc.pending_task_sends.get(&task_id).map_or(0, |sends| {
            sends
                .iter()
                .filter(|(tid, _)| *tid == target_task_id)
                .count()
        });
        if committed_len + pending_len >= self.server_options.load().max_task_mailbox {
            return v_error(E_QUOTA.with_msg(|| {
                format!(
                    "Task mailbox full ({} messages) for task ({target_task_id})",
                    committed_len + pending_len
                )
            }));
        }

        // Buffer the message for delivery at commit time
        lc.pending_task_sends
            .entry(task_id)
            .or_default()
            .push((target_task_id, value));

        v_int(0)
    }

    pub fn handle_task_recv(&self, task_id: TaskId) -> Vec<Var> {
        let mut lc = self.lifecycle.lock();
        // Drain all messages from the calling task's queue
        let (messages, total_wait_nanos, message_count) =
            lc.task_q.drain_messages_with_wait_nanos(task_id);
        if message_count > 0 {
            let perfc = sched_counters();
            perfc
                .task_message_delivery_to_recv_latency
                .invocations()
                .add(message_count as isize);
            perfc
                .task_message_delivery_to_recv_latency
                .cumulative_duration_nanos()
                .add(total_wait_nanos as isize);
        }
        messages
    }

    pub fn handle_force_gc(&self) {
        info!("Forcing garbage collection via gc_collect() builtin");
        if !self.config.features.anonymous_objects {
            warn!(
                "GC force requested but anonymous objects are disabled, ignoring request"
            );
        } else {
            {
                let mut lc = self.lifecycle.lock();
                lc.gc_force_collect = true;
            }
            self.wake_timer_thread();
        }
    }

    pub fn handle_rotate_enrollment_token(&self) -> Result<String, Error> {
        self.system_control.rotate_enrollment_token()
    }

    pub fn handle_player_event_log_stats(
        &self,
        player: Obj,
        since: Option<SystemTime>,
        until: Option<SystemTime>,
    ) -> Result<EventLogStats, Error> {
        self.system_control
            .player_event_log_stats(player, since, until)
    }

    pub fn handle_purge_player_event_log(
        &self,
        player: Obj,
        before: Option<SystemTime>,
        drop_pubkey: bool,
    ) -> Result<EventLogPurgeResult, Error> {
        self.system_control
            .purge_player_event_log(player, before, drop_pubkey)
    }

    pub fn handle_request_new_transaction(
        &self,
        task_id: TaskId,
    ) -> Result<Box<dyn WorldState>, SchedulerError> {
        let mut lc = self.lifecycle.lock();
        lc.flush_pending_sends(task_id);
        drop(lc);

        self.database
            .new_world_state()
            .map_err(|_| SchedulerError::CouldNotStartTask)
    }

    pub fn handle_dump_object_from_task(
        &self,
        obj: Obj,
        use_constants: bool,
    ) -> Result<Vec<String>, Error> {
        self.handle_dump_object(obj, use_constants)
    }

    pub fn handle_get_workers_info_from_task(&self) -> Vec<WorkerInfo> {
        self.handle_get_workers_info()
    }

    pub fn handle_switch_player_from_task(
        &self,
        task_id: TaskId,
        new_player: Obj,
    ) -> Result<(), Error> {
        let mut lc = self.lifecycle.lock();

        // Get the current task to access its session
        let Some(task) = lc.task_q.active.get_mut(&task_id) else {
            return Err(E_INVARG.with_msg(|| "Task not found for switch_player".to_string()));
        };

        // Get the connection details for the current session (player=None means "this session")
        let connection_details = task.session.connection_details(None).map_err(|e| {
            E_INVARG
                .with_msg(|| format!("Failed to get connection details for current session: {e:?}"))
        })?;

        // There should be exactly one connection for the current session
        let connection_obj = connection_details
            .first()
            .ok_or_else(|| {
                E_INVARG.with_msg(|| "No connection found for current session".to_string())
            })?
            .connection_obj;

        // Update the task's player
        task.player = new_player;

        drop(lc);

        // Switch the player through the system control (which handles connection registry and host notification)
        self.system_control
            .switch_player(connection_obj, new_player)
    }
}
