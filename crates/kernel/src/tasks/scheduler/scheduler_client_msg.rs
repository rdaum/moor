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

static DO_OUT_OF_BAND_COMMAND: LazyLock<Symbol> =
    LazyLock::new(|| Symbol::mk("do_out_of_band_command"));

impl Scheduler {
    pub(super) fn handle_scheduler_msg(&mut self, msg: SchedulerClientMsg) {
        let counters = sched_counters();
        let _t = PerfTimerGuard::new(&counters.handle_scheduler_msg);
        match msg {
            SchedulerClientMsg::SubmitCommandTask {
                handler_object,
                player,
                command,
                session,
                reply,
            } => {
                let task_start = TaskStart::StartCommandVerb {
                    handler_object,
                    player,
                    command: command.to_string(),
                };

                let task_id = self.next_task_id;
                self.next_task_id += 1;

                trace_task_create_command!(task_id, &player, &command, &handler_object);

                let result = self.submit_task(task_id, &player, &player, task_start, None, session);

                reply
                    .send(result)
                    .expect("Could not send task handle reply");
            }
            SchedulerClientMsg::SubmitVerbTask {
                player,
                vloc,
                verb,
                args,
                argstr,
                perms,
                session,
                reply,
            } => {
                // We need to translate Vloc and any of the arguments into valid references
                // before we can start the task.
                // If they're all just plain object references, we can just use them as-is, without
                // starting a transaction. Otherwise, we need to start a transaction to resolve them.
                let need_tx_oref = !matches!(vloc, ObjectRef::Id(_));
                let vloc = if need_tx_oref {
                    let mut tx = self.database.new_world_state().unwrap();
                    let Ok(vloc) = match_object_ref(&player, &perms, &vloc, tx.as_mut()) else {
                        reply
                            .send(Err(CommandExecutionError(CommandError::NoObjectMatch)))
                            .expect("Could not send task handle reply");
                        return;
                    };
                    v_obj(vloc)
                } else {
                    match vloc {
                        ObjectRef::Id(id) => v_obj(id),
                        _ => panic!("Unexpected object reference in vloc"),
                    }
                };

                let task_id = self.next_task_id;
                self.next_task_id += 1;

                trace_task_create_verb!(task_id, &player, &verb.as_string(), &vloc);

                let task_start = TaskStart::StartVerb {
                    player,
                    vloc,
                    verb,
                    args,
                    argstr,
                };

                let result = self.submit_task(task_id, &player, &perms, task_start, None, session);
                reply
                    .send(result)
                    .expect("Could not send task handle reply");
            }
            SchedulerClientMsg::SubmitTaskInput {
                player,
                input_request_id,
                input,
                reply,
            } => {
                // Validate that the given input request is valid, and if so, resume the task, sending it
                // the given input, clearing the input request out.

                // Find the task that requested this input, if any
                let Some(sr) = self
                    .task_q
                    .suspended
                    .pull_task_for_input(input_request_id, &player)
                else {
                    warn!(?input_request_id, "Input request not found");
                    reply
                        .send(Err(InputRequestNotFound(input_request_id.as_u128())))
                        .expect("Could not send input request not found reply");
                    return;
                };

                // Wake and bake.
                let response = self.task_q.wake_suspended_task(
                    sr,
                    ResumeAction::Return(input),
                    &self.task_control_sender,
                    self.database.as_ref(),
                    self.builtin_registry.clone(),
                    self.config.clone(),
                );
                reply.send(response).expect("Could not send input reply");
            }
            SchedulerClientMsg::SubmitOobTask {
                handler_object,
                player,
                command,
                argstr,
                session,
                reply,
            } => {
                let task_id = self.next_task_id;
                self.next_task_id += 1;

                let task_start = TaskStart::StartVerb {
                    player,
                    vloc: v_obj(handler_object),
                    verb: *DO_OUT_OF_BAND_COMMAND,
                    args: command,
                    argstr,
                };

                let result = self.submit_task(task_id, &player, &player, task_start, None, session);
                reply
                    .send(result)
                    .expect("Could not send task handle reply");
            }
            SchedulerClientMsg::SubmitEvalTask {
                player,
                perms,
                program,
                initial_env,
                sessions,
                reply,
            } => {
                let task_id = self.next_task_id;
                self.next_task_id += 1;

                trace_task_create_eval!(task_id, &player);

                let task_start = TaskStart::StartEval {
                    player,
                    program,
                    initial_env,
                };

                let result = self.submit_task(task_id, &player, &perms, task_start, None, sessions);
                reply
                    .send(result)
                    .expect("Could not send task handle reply");
            }
            SchedulerClientMsg::Shutdown(msg, reply) => {
                // Send shutdown notifications to all live tasks.

                let result = self.stop(Some(msg));
                reply.send(result).expect("Could not send shutdown reply");
            }
            SchedulerClientMsg::Checkpoint(blocking, reply) => {
                let result = if blocking {
                    self.checkpoint_blocking()
                } else {
                    self.checkpoint()
                };
                if reply.send(result).is_err() {
                    error!("Could not send checkpoint reply (client likely timed out)");
                }
            }
            SchedulerClientMsg::CheckStatus(reply) => {
                // Lightweight status check - just confirm we're alive and responding
                reply.send(Ok(())).expect("Could not send status reply");
            }
            SchedulerClientMsg::GetGCStats(reply) => {
                use crate::tasks::scheduler_client::GCStats;
                let stats = GCStats {
                    cycle_count: self.gc_cycle_count,
                };
                reply
                    .send(Ok(stats))
                    .expect("Could not send GC stats reply");
            }
            SchedulerClientMsg::RequestGC(reply) => {
                debug!("Direct GC request received via scheduler client");

                // Check if anonymous objects are enabled first
                if !self.config.features.anonymous_objects {
                    warn!("GC requested but anonymous objects are disabled, ignoring request");
                    reply.send(Ok(())).expect("Could not send GC request reply");
                } else if self.gc_collection_in_progress {
                    info!(
                        "GC already in progress, request acknowledged but no additional cycle started"
                    );
                    reply.send(Ok(())).expect("Could not send GC request reply");
                } else if self.task_q.active.is_empty() {
                    // Can run GC immediately since no active tasks
                    self.run_gc_cycle();
                    reply.send(Ok(())).expect("Could not send GC request reply");
                } else {
                    // Set flag for GC to run when tasks complete
                    self.gc_force_collect = true;
                    debug!("GC requested but tasks are active, will run when tasks complete");
                    reply.send(Ok(())).expect("Could not send GC request reply");
                }
            }
            SchedulerClientMsg::LoadObject {
                object_definition,
                options,
                return_conflicts,
                reply,
            } => {
                let result = self.handle_load_object(object_definition, options, return_conflicts);
                if let Err(e) = reply.send(result) {
                    error!(?e, "Could not send load_object reply to requester");
                }
            }
            SchedulerClientMsg::ReloadObject {
                object_definition,
                constants,
                target_obj,
                reply,
            } => {
                let result = self.handle_reload_object(object_definition, constants, target_obj);
                if let Err(e) = reply.send(result) {
                    error!(?e, "Could not send reload_object reply to requester");
                }
            }
            SchedulerClientMsg::GCMarkPhaseComplete {
                unreachable_objects,
                mutation_timestamp_before_mark,
            } => {
                // Clear the concurrent GC flag
                self.gc_mark_in_progress = false;

                debug!(
                    "GC mark phase completed, received {} unreachable objects",
                    unreachable_objects.len()
                );

                // Check if mutations happened during mark phase
                if mutation_timestamp_before_mark != self.last_mutation_timestamp {
                    info!(
                        "Minor GC cycle #{}: mark phase invalidated by mutation during marking (before: {:?}, after: {:?}), skipping sweep phase",
                        self.gc_cycle_count,
                        mutation_timestamp_before_mark,
                        self.last_mutation_timestamp
                    );
                    self.task_q.suspended.enqueue_gc_waiting_tasks();
                    return;
                }

                // Check if there's work to do
                if unreachable_objects.is_empty() {
                    debug!(
                        "Minor GC cycle #{}: mark phase found no objects to collect, skipping sweep phase",
                        self.gc_cycle_count
                    );
                    self.task_q.suspended.enqueue_gc_waiting_tasks();
                    return;
                }

                // Start blocking sweep phase
                let _ = self.run_blocking_sweep_phase(unreachable_objects);
                self.task_q.suspended.enqueue_gc_waiting_tasks();
            }
            SchedulerClientMsg::SubmitSystemHandlerTask {
                player,
                handler_type,
                args,
                session,
                reply,
            } => {
                // If no provided (auth'd) player, we use #0 itself
                let player = if player == NOTHING {
                    SYSTEM_OBJECT
                } else {
                    player
                };
                debug!(
                    "Processing system handler task: handler_type={}, player={}, args_count={}",
                    handler_type,
                    player,
                    args.len()
                );

                // Construct specific verb name: invoke_<handler_type>_handler
                let verb_name = format!("invoke_{handler_type}_handler");
                let invoke_handler_sym = Symbol::mk(&verb_name);

                // Prepare arguments: [args...] (handler_type is now encoded in the verb name)
                let handler_args = args;

                let task_id = self.next_task_id;
                self.next_task_id += 1;
                debug!("Created system handler task with id={}", task_id);

                let task_start = TaskStart::StartVerb {
                    player,
                    vloc: v_obj(SYSTEM_OBJECT),
                    verb: invoke_handler_sym,
                    args: List::mk_list(&handler_args),
                    argstr: v_empty_str(),
                };

                let result = self.submit_task(
                    task_id, &player, &player, // Use the same player as permissions object
                    task_start, None, session,
                );
                debug!("System handler task submission result: {:?}", result);
                reply
                    .send(result)
                    .expect("Could not send task handle reply");
            }
            SchedulerClientMsg::ExecuteWorldStateActions {
                actions,
                rollback,
                reply,
            } => {
                // Create transaction in scheduler thread
                let tx = match self.database.new_world_state() {
                    Ok(tx) => tx,
                    Err(e) => {
                        reply
                            .send(Err(CommandExecutionError(CommandError::DatabaseError(e))))
                            .expect("Could not send batch execution reply");
                        return;
                    }
                };

                // Extract just the actions from the requests
                let action_vec: Vec<WorldStateAction> =
                    actions.iter().map(|req| req.action.clone()).collect();
                let config = self.config.clone();

                // Spawn thread to execute actions, moving transaction into the thread
                spawn_perf("ws-actions", move || {
                    let executor = WorldStateActionExecutor::new(tx, config);

                    match executor.execute_batch(action_vec, rollback) {
                        Ok(results) => {
                            // Build responses with the original request IDs
                            let responses: Vec<WorldStateResponse> = actions
                                .into_iter()
                                .zip(results)
                                .map(|(request, result)| WorldStateResponse::Success {
                                    id: request.id,
                                    result,
                                })
                                .collect();

                            reply
                                .send(Ok(responses))
                                .expect("Could not send batch execution reply");
                        }
                        Err(error) => {
                            reply
                                .send(Err(error))
                                .expect("Could not send batch execution reply");
                        }
                    }
                })
                .expect("Could not spawn WorldStateAction execution thread");
            }
            SchedulerClientMsg::SubmitBatchWorldStateTask {
                player,
                perms,
                actions,
                rollback,
                result_sink,
                session,
                reply,
            } => {
                let task_id = self.next_task_id;
                self.next_task_id += 1;

                let task_start = TaskStart::StartBatchWorldState {
                    player,
                    perms,
                    actions,
                    rollback,
                    result_sink,
                };

                let result = self.submit_task(task_id, &player, &perms, task_start, None, session);
                reply
                    .send(result)
                    .expect("Could not send batch task handle reply");
            }
        }
    }
}
