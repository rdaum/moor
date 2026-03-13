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
use moor_common::model::ObjectRef;

static DO_OUT_OF_BAND_COMMAND: LazyLock<Symbol> =
    LazyLock::new(|| Symbol::mk("do_out_of_band_command"));

impl Scheduler {
    pub(crate) fn submit_command_task_inner(
        &self,
        handler_object: Obj,
        player: Obj,
        command: String,
        session: Arc<dyn Session>,
    ) -> Result<TaskHandle, SchedulerError> {
        let mut lc = self.lifecycle.lock();
        let task_id = lc.next_task_id;
        lc.next_task_id += 1;

        trace_task_create_command!(task_id, &player, &command, &handler_object);

        let task_start = TaskStart::StartCommandVerb {
            handler_object,
            player,
            command: command.to_string(),
        };

        self.submit_task(&mut lc, task_id, &player, &player, task_start, None, session)
    }

    pub(crate) fn submit_verb_task_inner(
        &self,
        player: Obj,
        vloc: ObjectRef,
        verb: Symbol,
        args: List,
        argstr: Var,
        perms: Obj,
        session: Arc<dyn Session>,
    ) -> Result<TaskHandle, SchedulerError> {
        // We need to translate Vloc and any of the arguments into valid references
        // before we can start the task.
        // If they're all just plain object references, we can just use them as-is, without
        // starting a transaction. Otherwise, we need to start a transaction to resolve them.
        let need_tx_oref = !matches!(vloc, ObjectRef::Id(_));
        let vloc = if need_tx_oref {
            let mut tx = self.database.new_world_state().unwrap();
            let Ok(vloc) = match_object_ref(&player, &perms, &vloc, tx.as_mut()) else {
                return Err(CommandExecutionError(CommandError::NoObjectMatch));
            };
            v_obj(vloc)
        } else {
            match vloc {
                ObjectRef::Id(id) => v_obj(id),
                _ => panic!("Unexpected object reference in vloc"),
            }
        };

        let mut lc = self.lifecycle.lock();
        let task_id = lc.next_task_id;
        lc.next_task_id += 1;

        trace_task_create_verb!(task_id, &player, &verb.as_string(), &vloc);

        let task_start = TaskStart::StartVerb {
            player,
            vloc,
            verb,
            args,
            argstr,
        };

        self.submit_task(&mut lc, task_id, &player, &perms, task_start, None, session)
    }

    pub(crate) fn submit_task_input_inner(
        &self,
        player: Obj,
        input_request_id: Uuid,
        input: Var,
    ) -> Result<(), SchedulerError> {
        let mut lc = self.lifecycle.lock();

        // Validate that the given input request is valid, and if so, resume the task, sending it
        // the given input, clearing the input request out.

        // Find the task that requested this input, if any
        let Some(sr) = lc
            .task_q
            .suspended
            .pull_task_for_input(input_request_id, &player)
        else {
            warn!(?input_request_id, "Input request not found");
            return Err(InputRequestNotFound(input_request_id.as_u128()));
        };

        // Wake and bake.
        lc.task_q.wake_suspended_task(
            sr,
            ResumeAction::Return(input),
            self,
            self.database.as_ref(),
            self.builtin_registry.clone(),
            self.config.clone(),
        )
    }

    pub(crate) fn submit_oob_task_inner(
        &self,
        handler_object: Obj,
        player: Obj,
        command: List,
        argstr: Var,
        session: Arc<dyn Session>,
    ) -> Result<TaskHandle, SchedulerError> {
        let mut lc = self.lifecycle.lock();
        let task_id = lc.next_task_id;
        lc.next_task_id += 1;

        let task_start = TaskStart::StartVerb {
            player,
            vloc: v_obj(handler_object),
            verb: *DO_OUT_OF_BAND_COMMAND,
            args: command,
            argstr,
        };

        self.submit_task(&mut lc, task_id, &player, &player, task_start, None, session)
    }

    pub(crate) fn submit_eval_task_inner(
        &self,
        player: Obj,
        perms: Obj,
        program: moor_compiler::Program,
        initial_env: Option<Vec<(Symbol, Var)>>,
        session: Arc<dyn Session>,
    ) -> Result<TaskHandle, SchedulerError> {
        let mut lc = self.lifecycle.lock();
        let task_id = lc.next_task_id;
        lc.next_task_id += 1;

        trace_task_create_eval!(task_id, &player);

        let task_start = TaskStart::StartEval {
            player,
            program,
            initial_env,
        };

        self.submit_task(&mut lc, task_id, &player, &perms, task_start, None, session)
    }

    pub(crate) fn handle_shutdown_request(
        &self,
        msg: String,
    ) -> Result<(), SchedulerError> {
        let mut lc = self.lifecycle.lock();

        // Send shutdown notification to all live tasks.
        for (_, task) in lc.task_q.active.iter() {
            let _ = task.session.notify_shutdown(Some(msg.clone()));
        }
        warn!("Issuing clean shutdown...");
        {
            // Send shut down to all the tasks.
            for (_, task) in lc.task_q.active.drain() {
                task.kill_switch.store(true, Ordering::SeqCst);
            }
        }
        warn!("Waiting for tasks to finish...");

        // Then spin until they're all done.
        loop {
            if lc.task_q.active.is_empty() {
                break;
            }
            // Drop the lock while spinning so tasks can complete.
            drop(lc);
            yield_now();
            lc = self.lifecycle.lock();
        }

        // Now ask the rpc server and hosts to shutdown
        self.system_control
            .shutdown(Some(msg))
            .expect("Could not cleanly shutdown system");

        warn!("All tasks finished.  Stopping scheduler.");
        lc.running = false;

        Ok(())
    }

    pub(crate) fn handle_checkpoint_request(
        &self,
        blocking: bool,
    ) -> Result<(), SchedulerError> {
        if blocking {
            self.checkpoint_blocking()
        } else {
            self.checkpoint()
        }
    }

    pub(crate) fn handle_check_status(&self) -> Result<(), SchedulerError> {
        // Lightweight status check - just confirm we're alive and responding
        Ok(())
    }

    pub(crate) fn handle_get_gc_stats(
        &self,
    ) -> Result<crate::tasks::scheduler_client::GCStats, SchedulerError> {
        let lc = self.lifecycle.lock();
        Ok(crate::tasks::scheduler_client::GCStats {
            cycle_count: lc.gc_cycle_count,
        })
    }

    pub(crate) fn handle_request_gc(&self) -> Result<(), SchedulerError> {
        debug!("Direct GC request received via scheduler client");

        let mut lc = self.lifecycle.lock();

        // Check if anonymous objects are enabled first
        if !self.config.features.anonymous_objects {
            warn!("GC requested but anonymous objects are disabled, ignoring request");
            Ok(())
        } else if lc.gc_collection_in_progress {
            info!(
                "GC already in progress, request acknowledged but no additional cycle started"
            );
            Ok(())
        } else if lc.task_q.active.is_empty() {
            // Can run GC immediately since no active tasks
            self.run_gc_cycle(&mut lc);
            Ok(())
        } else {
            // Set flag for GC to run when tasks complete
            lc.gc_force_collect = true;
            debug!("GC requested but tasks are active, will run when tasks complete");
            Ok(())
        }
    }

    pub(crate) fn handle_load_object_request(
        &self,
        object_definition: String,
        options: moor_objdef::ObjDefLoaderOptions,
        return_conflicts: bool,
    ) -> Result<moor_objdef::ObjDefLoaderResults, SchedulerError> {
        self.handle_load_object(object_definition, options, return_conflicts)
    }

    pub(crate) fn handle_reload_object_request(
        &self,
        object_definition: String,
        constants: Option<moor_objdef::Constants>,
        target_obj: Option<Obj>,
    ) -> Result<moor_objdef::ObjDefLoaderResults, SchedulerError> {
        self.handle_reload_object(object_definition, constants, target_obj)
    }

    pub(crate) fn handle_gc_mark_complete(
        &self,
        unreachable_objects: std::collections::HashSet<Obj>,
        mutation_timestamp_before_mark: Option<u64>,
    ) {
        let mut lc = self.lifecycle.lock();

        // Clear the concurrent GC flag
        lc.gc_mark_in_progress = false;

        debug!(
            "GC mark phase completed, received {} unreachable objects",
            unreachable_objects.len()
        );

        // Check if mutations happened during mark phase
        if mutation_timestamp_before_mark != lc.last_mutation_timestamp {
            info!(
                "Minor GC cycle #{}: mark phase invalidated by mutation during marking (before: {:?}, after: {:?}), skipping sweep phase",
                lc.gc_cycle_count,
                mutation_timestamp_before_mark,
                lc.last_mutation_timestamp
            );
            lc.task_q.suspended.enqueue_gc_waiting_tasks();
            return;
        }

        // Check if there's work to do
        if unreachable_objects.is_empty() {
            debug!(
                "Minor GC cycle #{}: mark phase found no objects to collect, skipping sweep phase",
                lc.gc_cycle_count
            );
            lc.task_q.suspended.enqueue_gc_waiting_tasks();
            return;
        }

        // Start blocking sweep phase - drop lock first since run_blocking_sweep_phase manages its own locking
        drop(lc);
        let _ = self.run_blocking_sweep_phase(unreachable_objects);
        let mut lc = self.lifecycle.lock();
        lc.task_q.suspended.enqueue_gc_waiting_tasks();
    }

    pub(crate) fn submit_system_handler_task_inner(
        &self,
        player: Obj,
        handler_type: String,
        args: Vec<Var>,
        session: Arc<dyn Session>,
    ) -> Result<TaskHandle, SchedulerError> {
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

        let mut lc = self.lifecycle.lock();
        let task_id = lc.next_task_id;
        lc.next_task_id += 1;
        debug!("Created system handler task with id={}", task_id);

        let task_start = TaskStart::StartVerb {
            player,
            vloc: v_obj(SYSTEM_OBJECT),
            verb: invoke_handler_sym,
            args: List::mk_list(&handler_args),
            argstr: v_empty_str(),
        };

        let result = self.submit_task(
            &mut lc,
            task_id,
            &player,
            &player, // Use the same player as permissions object
            task_start,
            None,
            session,
        );
        debug!("System handler task submission result: {:?}", result);
        result
    }

    pub(crate) fn execute_world_state_actions_inner(
        &self,
        actions: Vec<crate::tasks::world_state_action::WorldStateRequest>,
        rollback: bool,
    ) -> Result<Vec<WorldStateResponse>, SchedulerError> {
        // Create transaction in caller's context
        let tx = self
            .database
            .new_world_state()
            .map_err(|e| CommandExecutionError(CommandError::DatabaseError(e)))?;

        // Extract just the actions from the requests
        let action_vec: Vec<WorldStateAction> =
            actions.iter().map(|req| req.action.clone()).collect();
        let config = self.config.clone();

        // Use a oneshot channel to get the result back from the spawned thread
        let (tx_send, rx_recv) = std::sync::mpsc::channel();

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

                    let _ = tx_send.send(Ok(responses));
                }
                Err(error) => {
                    let _ = tx_send.send(Err(error));
                }
            }
        })
        .expect("Could not spawn WorldStateAction execution thread");

        rx_recv
            .recv()
            .map_err(|_| SchedulerError::CouldNotStartTask)?
    }

    pub(crate) fn submit_batch_world_state_task_inner(
        &self,
        player: Obj,
        perms: Obj,
        actions: Vec<WorldStateAction>,
        rollback: bool,
        result_sink: Arc<
            std::sync::Mutex<
                Option<
                    Result<
                        Vec<crate::tasks::world_state_action::WorldStateResult>,
                        SchedulerError,
                    >,
                >,
            >,
        >,
        session: Arc<dyn Session>,
    ) -> Result<TaskHandle, SchedulerError> {
        let mut lc = self.lifecycle.lock();
        let task_id = lc.next_task_id;
        lc.next_task_id += 1;

        let task_start = TaskStart::StartBatchWorldState {
            player,
            perms,
            actions,
            rollback,
            result_sink,
        };

        self.submit_task(&mut lc, task_id, &player, &perms, task_start, None, session)
    }
}
