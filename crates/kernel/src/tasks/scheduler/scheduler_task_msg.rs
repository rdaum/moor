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

static HANDLE_TASK_TIMEOUT_SYM: LazyLock<Symbol> =
    LazyLock::new(|| Symbol::mk("handle_task_timeout"));

impl Scheduler {
    /// Handle task control messages inbound from tasks.
    /// Note: this function should never be allowed to panic, as it is called from the scheduler main loop.
    pub(super) fn handle_task_msg(&mut self, task_id: TaskId, msg: TaskControlMsg) {
        let counters = sched_counters();
        let _t = PerfTimerGuard::new(&counters.handle_task_msg);

        match msg {
            TaskControlMsg::TaskSuccess(value, mutations_made, timestamp) => {
                // Record that this is the transaction to have last mutated the world.
                // Used by e.g. concurrent GC algorithm.
                if mutations_made {
                    self.last_mutation_timestamp = Some(timestamp);
                }

                // Commit the session.
                let Some(task) = self.task_q.active.get_mut(&task_id) else {
                    warn!(task_id, "Task not found for success");
                    return;
                };
                let Ok(()) = task.session.commit() else {
                    warn!("Could not commit session; aborting task");
                    self.discard_pending_sends(task_id);
                    return self.task_q.send_task_result(task_id, Err(TaskAbortedError));
                };
                self.flush_pending_sends(task_id);
                self.task_q.remove_message_queue(task_id);
                self.task_q.send_task_result(task_id, Ok(value))
            }
            TaskControlMsg::TaskConflictRetry(mut task) => {
                let perfc = sched_counters();
                let _t = PerfTimerGuard::new(&perfc.task_conflict_retry);

                self.discard_pending_sends(task_id);

                // Make sure the old thread is dead.
                task.kill_switch.store(true, Ordering::SeqCst);

                // Remove from active tasks to get session/result_sender
                let Some(old_tc) = self.task_q.active.remove(&task_id) else {
                    error!(
                        task_id,
                        "Task not found for retry suspension, ignoring -- consistency issue!"
                    );
                    return;
                };

                // If the number of retries has been exceeded, abort immediately
                if task.retries >= self.server_options.max_task_retries {
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
                self.task_q.suspended.add_task(
                    WakeCondition::Retry(wake_time),
                    task,
                    old_tc.session,
                    old_tc.result_sender,
                );
            }
            TaskControlMsg::TaskVerbNotFound(who, what) => {
                self.task_q.send_task_result(
                    task_id,
                    Err(SchedulerError::TaskAbortedVerbNotFound(who, what)),
                );
            }
            TaskControlMsg::TaskCommandError(parse_command_error) => {
                // This is a common occurrence, so we don't want to log it at warn level.
                self.task_q
                    .send_task_result(task_id, Err(CommandExecutionError(parse_command_error)));
            }
            TaskControlMsg::TaskAbortCancelled => {
                let perfc = sched_counters();
                let _t = PerfTimerGuard::new(&perfc.task_abort_cancelled);

                self.discard_pending_sends(task_id);
                self.task_q.remove_message_queue(task_id);

                warn!(?task_id, "Task cancelled");

                // Rollback the session.
                let Some(task) = self.task_q.active.get_mut(&task_id) else {
                    warn!(task_id, "Task not found for abort");
                    return;
                };
                if let Err(send_error) = task
                    .session
                    .send_system_msg(task.player, "Aborted.".to_string().as_str())
                {
                    warn!("Could not send abort message to player: {:?}", send_error);
                };

                let Ok(()) = task.session.commit() else {
                    warn!("Could not commit aborted session; aborting task");
                    return self.task_q.send_task_result(task_id, Err(TaskAbortedError));
                };
                self.task_q
                    .send_task_result(task_id, Err(TaskAbortedCancelled));
            }
            TaskControlMsg::TaskAbortPanicked(panic_msg, _backtrace) => {
                warn!(?task_id, ?panic_msg, "Task thread panicked");

                self.discard_pending_sends(task_id);
                self.task_q.remove_message_queue(task_id);

                // Task already dead, can't access session. Just send error result directly.
                self.task_q.send_task_result(task_id, Err(TaskAbortedError));
            }
            TaskControlMsg::TaskAbortLimitsReached(
                limit_reason,
                this,
                verb,
                line_number,
                handler_info,
            ) => {
                self.discard_pending_sends(task_id);
                self.task_q.remove_message_queue(task_id);

                let perfc = sched_counters();
                let _t = PerfTimerGuard::new(&perfc.task_abort_limits);

                // Get the task's session and player for notifications and handler invocation
                let Some(mut task) = self.task_q.active.remove(&task_id) else {
                    warn!(task_id, "Task not found for abort");
                    return;
                };

                let player = task.player;
                let session = task.session.clone();

                // Send abort notification to player
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

                // Attempt to invoke the handler verb as a separate task
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

                let handler_task_id = self.next_task_id;
                self.next_task_id += 1;

                debug!(
                    "Spawning handler task {} for timeout on task {}",
                    handler_task_id, task_id
                );

                let handler_result = self.submit_task(
                    handler_task_id,
                    &player,
                    &player, // Use player as permissions for handler
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
                self.task_q.suspended.enqueue_dependents_for(task_id);
                TaskQ::send_task_result_direct(
                    task_id,
                    task.result_sender.take(),
                    Err(TaskAbortedLimit(limit_reason)),
                );
            }
            TaskControlMsg::TaskException(exception) => {
                let perfc = sched_counters();
                let _t = PerfTimerGuard::new(&perfc.task_exception);

                let Some(task) = self.task_q.active.get_mut(&task_id) else {
                    warn!(task_id, "Task not found for abort");
                    return;
                };

                // Compose a string out of the backtrace
                if let Err(send_error) = task.session.send_event(
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

                let _ = task.session.commit();
                self.flush_pending_sends(task_id);
                self.task_q.remove_message_queue(task_id);

                self.task_q.send_task_result(
                    task_id,
                    Err(TaskAbortedException(exception.as_ref().clone())),
                );
            }
            TaskControlMsg::TaskRequestFork(fork_request, reply) => {
                let perfc = sched_counters();
                let _t = PerfTimerGuard::new(&perfc.fork_task);

                // Task has requested a fork. Dispatch it and reply with the new task id.
                // Gotta dump this out til we exit the loop tho, since self.tasks is already
                // borrowed here.
                let new_session = {
                    let Some(task) = self.task_q.active.get_mut(&task_id) else {
                        warn!(task_id, "Task not found for fork request");
                        return;
                    };
                    task.session.clone()
                };
                self.process_fork_request(fork_request, reply, new_session);
            }
            TaskControlMsg::TaskSuspend(wake_condition, task) => {
                // Task is suspended. The resume time (if any) is the system time at which
                // the scheduler should try to wake us up.

                // Remove from the local task control...
                let Some(tc) = self.task_q.active.remove(&task_id) else {
                    warn!(task_id, "Task not found for suspend request");
                    return;
                };

                // Commit the session.
                let Ok(()) = tc.session.commit() else {
                    warn!("Could not commit session; aborting task");
                    self.discard_pending_sends(task_id);
                    return self.task_q.send_task_result(task_id, Err(TaskAbortedError));
                };
                self.flush_pending_sends(task_id);

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
                        // Send out a message over the workers channel.
                        // If we're not set up to do workers, just abort the task.
                        let Some(workers_sender) = self.worker_request_send.as_ref() else {
                            warn!("No workers configured for scheduler; aborting task");
                            return self.task_q.send_task_result(task_id, Err(TaskAbortedError));
                        };

                        if let Err(e) = workers_sender.send(WorkerRequest::Request {
                            request_id: worker_request_id,
                            request_type: worker_type,
                            perms: task.perms,
                            request: args,
                            timeout,
                        }) {
                            error!(?e, "Could not send worker request; aborting task");
                            return self.task_q.send_task_result(task_id, Err(TaskAbortedError));
                        }

                        WakeCondition::Worker(worker_request_id)
                    }
                    TaskSuspend::RecvMessages(Some(duration)) => {
                        // Check if there are already messages in the queue after commit
                        let messages = self.task_q.drain_messages(task_id);
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
                        let messages = self.task_q.drain_messages(task_id);
                        WakeCondition::Immediate(Some(List::from_iter(messages).into()))
                    }
                };

                if !matches!(wake_condition, WakeCondition::Immediate(_))
                    && let Some(sender) = tc.result_sender.as_ref()
                {
                    let _ = sender.send((task_id, Ok(TaskNotification::Suspended)));
                }

                self.task_q
                    .suspended
                    .add_task(wake_condition, task, tc.session, tc.result_sender);
            }
            TaskControlMsg::TaskRequestInput(task, metadata) => {
                // Task has gone into suspension waiting for input from the client.
                // Create a unique ID for this request, and we'll wake the task when the
                // session receives input.

                let input_request_id = Uuid::new_v4();
                let Some(tc) = self.task_q.active.remove(&task_id) else {
                    warn!(task_id, "Task not found for input request");
                    return;
                };
                // Commit the session (not DB transaction) to make sure current output is
                // flushed up to the prompt point.
                let Ok(()) = tc.session.commit() else {
                    warn!("Could not commit session; aborting task");
                    self.discard_pending_sends(task_id);
                    return self.task_q.send_task_result(task_id, Err(TaskAbortedError));
                };
                self.flush_pending_sends(task_id);

                let Ok(()) = tc
                    .session
                    .request_input(tc.player, input_request_id, metadata)
                else {
                    warn!("Could not request input from session; aborting task");
                    return self.task_q.send_task_result(task_id, Err(TaskAbortedError));
                };
                self.task_q.suspended.add_task(
                    WakeCondition::Input(input_request_id),
                    task,
                    tc.session,
                    tc.result_sender,
                );
            }

            TaskControlMsg::RequestTasks(reply) => {
                let tasks = self.task_q.suspended.tasks();
                if let Err(e) = reply.send(tasks) {
                    error!(?e, "Could not send task description to requester");
                    // TODO: murder this errant task
                }
                // TODO: add non-queued tasks.
            }
            TaskControlMsg::TaskExists {
                task_id: check_task_id,
                result_sender,
            } => {
                // Check both suspended and active tasks atomically
                let owner = self.task_q.task_owner(check_task_id);
                if let Err(e) = result_sender.send(owner) {
                    error!(?e, "Could not send task exists result to requester");
                }
            }
            TaskControlMsg::KillTask {
                victim_task_id,
                sender_permissions,
                result_sender,
            } => {
                // Task is asking to kill another task.
                let kr = self.task_q.kill_task(victim_task_id, sender_permissions);
                if let Err(e) = result_sender.send(kr) {
                    error!(?e, "Could not send kill task result to requester");
                }
            }
            TaskControlMsg::ResumeTask {
                queued_task_id,
                sender_permissions,
                return_value,
                result_sender,
            } => {
                let rr = self.task_q.resume_task(
                    task_id,
                    queued_task_id,
                    sender_permissions,
                    return_value,
                    &self.task_control_sender,
                    self.database.as_ref(),
                    self.builtin_registry.clone(),
                    self.config.clone(),
                );
                if let Err(e) = result_sender.send(rr) {
                    error!(?e, "Could not send resume task result to requester");
                }
            }
            TaskControlMsg::BootPlayer { player } => {
                // Task is asking to boot a player.
                self.task_q.disconnect_task(task_id, &player);
            }
            TaskControlMsg::Notify { player, event } => {
                // Task is asking to notify a player of an event.
                let Some(task) = self.task_q.active.get_mut(&task_id) else {
                    warn!(task_id, "Task not found for notify request");
                    return;
                };
                let Ok(()) = task.session.send_event(player, event) else {
                    warn!("Could not notify player; aborting task");
                    return self.task_q.send_task_result(task_id, Err(TaskAbortedError));
                };
            }
            TaskControlMsg::LogEvent { player, event } => {
                // Task is asking to log an event without broadcasting.
                let Some(task) = self.task_q.active.get_mut(&task_id) else {
                    warn!(task_id, "Task not found for log_event request");
                    return;
                };
                let Ok(()) = task.session.log_event(player, event) else {
                    warn!("Could not log event; aborting task");
                    return self.task_q.send_task_result(task_id, Err(TaskAbortedError));
                };
            }
            TaskControlMsg::GetListeners(reply) => {
                let listeners = self
                    .system_control
                    .listeners()
                    .expect("Could not get listeners");
                if let Err(e) = reply.send(listeners) {
                    error!(?e, "Could not send listeners to requester");
                }
            }
            TaskControlMsg::Listen {
                handler_object,
                host_type,
                port,
                options,
                reply,
            } => {
                let Some(_task) = self.task_q.active.get_mut(&task_id) else {
                    warn!(task_id, "Task not found for listen request");
                    return;
                };
                let result = self
                    .system_control
                    .listen(handler_object, &host_type, port, *options)
                    .err();
                reply.send(result).expect("Could not send listen reply");
            }
            TaskControlMsg::Unlisten {
                host_type,
                port,
                reply,
            } => {
                let Some(_task) = self.task_q.active.get_mut(&task_id) else {
                    warn!(task_id, "Task not found for unlisten request");
                    return;
                };
                let result = match self.system_control.unlisten(port, &host_type) {
                    Ok(_) => None,
                    Err(_) => Some(E_PERM.msg("Permission denied on unlisten")),
                };
                reply.send(result).expect("Could not send unlisten reply");
            }
            TaskControlMsg::Shutdown(msg) => {
                info!("Shutting down scheduler. Reason: {msg:?}");
                self.stop(msg)
                    .expect("Could not shutdown scheduler cleanly");
            }
            TaskControlMsg::ForceInput { who, line, reply } => {
                let new_session = {
                    let Some(task) = self.task_q.active.get_mut(&task_id) else {
                        warn!(task_id, "Task not found for force input request");
                        reply.send(Err(E_INVIND.msg("Task not found"))).ok();
                        return;
                    };
                    task.session.clone().fork().unwrap()
                };
                let task_start = TaskStart::StartCommandVerb {
                    handler_object: SYSTEM_OBJECT,
                    player: who,
                    command: line,
                };

                let new_task_id = self.next_task_id;
                self.next_task_id += 1;
                let result =
                    self.submit_task(new_task_id, &who, &who, task_start, None, new_session);
                match result {
                    Err(e) => {
                        error!(?e, "Could not start task thread");
                        reply
                            .send(Err(E_INVIND.with_msg(|| {
                                format!("Could not start thread for force_input: {e:?}")
                            })))
                            .ok();
                    }
                    Ok(th) => {
                        reply.send(Ok(th.0)).ok();
                    }
                }
            }
            TaskControlMsg::Checkpoint(reply) => {
                let result = if reply.is_some() {
                    self.checkpoint_blocking()
                } else {
                    self.checkpoint()
                };

                if let Some(reply) = reply {
                    let _ = reply.send(result);
                } else if let Err(e) = result {
                    error!(?e, "Could not checkpoint");
                }
            }
            TaskControlMsg::RefreshServerOptions => {
                self.reload_server_options();
            }
            TaskControlMsg::ActiveTasks { reply } => {
                let mut results = vec![];
                for (task_id, tc) in self.task_q.active.iter() {
                    results.push((*task_id, tc.player, tc.task_start.clone()));
                }
                if let Err(e) = reply.send(Ok(results)) {
                    error!(?e, "Could not send active tasks to requester");
                }
            }
            TaskControlMsg::SwitchPlayer { new_player, reply } => {
                let result = self.handle_switch_player(task_id, new_player);
                if let Err(e) = reply.send(result) {
                    error!(?e, "Could not send switch player reply to requester");
                }
            }
            TaskControlMsg::DumpObject {
                obj,
                use_constants,
                reply,
            } => {
                let result = self.handle_dump_object(obj, use_constants);
                if let Err(e) = reply.send(result) {
                    error!(?e, "Could not send dump object reply to requester");
                }
            }
            TaskControlMsg::GetWorkersInfo { reply } => {
                let result = self.handle_get_workers_info();
                if let Err(e) = reply.send(result) {
                    error!(?e, "Could not send workers info reply to requester");
                }
            }
            TaskControlMsg::RequestNewTransaction(reply) => {
                self.flush_pending_sends(task_id);
                let result = self
                    .database
                    .new_world_state()
                    .map_err(|_| SchedulerError::CouldNotStartTask);
                if let Err(e) = reply.send(result) {
                    error!(?e, "Could not send new transaction reply to requester");
                }
            }
            TaskControlMsg::RotateEnrollmentToken { reply } => {
                let result = self.system_control.rotate_enrollment_token();
                if let Err(e) = reply.send(result) {
                    error!(
                        ?e,
                        "Could not send rotate enrollment token reply to requester"
                    );
                }
            }
            TaskControlMsg::PlayerEventLogStats {
                player,
                since,
                until,
                reply,
            } => {
                let result = self
                    .system_control
                    .player_event_log_stats(player, since, until);
                if let Err(e) = reply.send(result) {
                    error!(
                        ?e,
                        "Could not send player event log stats reply to requester"
                    );
                }
            }
            TaskControlMsg::PurgePlayerEventLog {
                player,
                before,
                drop_pubkey,
                reply,
            } => {
                let result =
                    self.system_control
                        .purge_player_event_log(player, before, drop_pubkey);
                if let Err(e) = reply.send(result) {
                    error!(
                        ?e,
                        "Could not send purge player event log reply to requester"
                    );
                }
            }
            TaskControlMsg::ForceGC => {
                info!("Forcing garbage collection via gc_collect() builtin");
                if !self.config.features.anonymous_objects {
                    warn!(
                        "GC force requested but anonymous objects are disabled, ignoring request"
                    );
                } else {
                    self.gc_force_collect = true;
                }
            }
            TaskControlMsg::TaskSend {
                target_task_id,
                value,
                sender_permissions,
                result_sender,
            } => {
                let Some(owner) = self.task_q.task_owner(target_task_id) else {
                    let _ =
                        result_sender.send(v_error(E_INVARG.with_msg(|| {
                            format!("Task ({target_task_id}) not found for task_send")
                        })));
                    return;
                };

                let is_wizard = sender_permissions
                    .check_is_wizard()
                    .expect("Could not check wizard status for task_send");
                if !is_wizard && sender_permissions.who != owner {
                    let _ = result_sender.send(v_error(E_PERM.with_msg(|| {
                        format!("Permission denied for task_send to task ({target_task_id})")
                    })));
                    return;
                }

                // Check mailbox size limit (committed queue + pending sends
                // from this task to same target)
                let committed_len = self.task_q.mailbox_len(target_task_id);
                let pending_len = self.pending_task_sends.get(&task_id).map_or(0, |sends| {
                    sends
                        .iter()
                        .filter(|(tid, _)| *tid == target_task_id)
                        .count()
                });
                if committed_len + pending_len >= self.server_options.max_task_mailbox {
                    let _ = result_sender.send(v_error(E_QUOTA.with_msg(|| {
                        format!(
                            "Task mailbox full ({} messages) for task ({target_task_id})",
                            committed_len + pending_len
                        )
                    })));
                    return;
                }

                // Buffer the message for delivery at commit time
                self.pending_task_sends
                    .entry(task_id)
                    .or_default()
                    .push((target_task_id, value));

                let _ = result_sender.send(v_int(0));
            }
            TaskControlMsg::TaskRecv { result_sender } => {
                // Drain all messages from the calling task's queue
                let (messages, total_wait_nanos, message_count) =
                    self.task_q.drain_messages_with_wait_nanos(task_id);
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
                if let Err(e) = result_sender.send(messages) {
                    error!(?e, "Could not send task_recv result to requester");
                }
            }
        }
    }

    /// Deliver all buffered messages from the given task to their target queues.
    /// Called when a task commits (success, suspend, input request, exception, new transaction).
    pub(super) fn flush_pending_sends(&mut self, task_id: TaskId) {
        if let Some(sends) = self.pending_task_sends.remove(&task_id) {
            for (target_task_id, value) in sends {
                self.task_q.deliver_message(target_task_id, value);
            }
        }
    }

    /// Discard all buffered messages from the given task without delivering.
    /// Called when a task aborts (conflict retry, cancelled, panicked, limits reached).
    pub(super) fn discard_pending_sends(&mut self, task_id: TaskId) {
        self.pending_task_sends.remove(&task_id);
    }
}
