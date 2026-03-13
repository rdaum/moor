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

impl Scheduler {
    /// Dumps an object's definition to a list of strings for export.
    ///
    /// Creates a database snapshot to avoid blocking ongoing operations, collects
    /// the object definition, and optionally builds index names from import_export_id
    /// properties when `use_constants` is true.
    ///
    /// # Arguments
    /// * `obj` - The object to dump
    /// * `use_constants` - If true, builds index names from all object definitions
    ///
    /// # Returns
    /// A vector of strings representing the object's definition, or an error
    pub(crate) fn handle_dump_object(
        &self,
        obj: Obj,
        use_constants: bool,
    ) -> Result<Vec<String>, Error> {
        // Create a snapshot to avoid blocking ongoing operations
        let snapshot = self.database.create_snapshot().map_err(|e| {
            E_INVARG.with_msg(|| format!("Failed to create database snapshot: {e:?}"))
        })?;

        // Collect the object definition
        let (_, _, _, object_def) = collect_object(snapshot.as_ref(), &obj)
            .map_err(|e| E_INVARG.with_msg(|| format!("Failed to collect object {obj}: {e:?}")))?;

        // Build index_names from import_export_id properties if requested
        let index_names = if use_constants {
            let all_objects = collect_object_definitions(snapshot.as_ref()).map_err(|e| {
                E_INVARG.with_msg(|| format!("Failed to collect object definitions: {e:?}"))
            })?;
            extract_index_names(&all_objects)
        } else {
            HashMap::new()
        };

        let lines = dump_object(&index_names, &object_def)
            .map_err(|e| E_INVARG.with_msg(|| format!("Failed to dump object {obj}: {e:?}")))?;

        Ok(lines)
    }

    /// Loads an object definition into the database.
    ///
    /// Creates a new world state, initializes an object definition loader,
    /// and loads a single object from the provided definition string.
    /// Commits the transaction if the loader result indicates success.
    ///
    /// # Arguments
    /// * `object_definition` - The object definition string to load
    /// * `options` - Loader options controlling the load behavior
    /// * `_return_conflicts` - Whether to return conflict information (unused)
    ///
    /// # Returns
    /// The loader results containing loaded object information, or a SchedulerError
    pub(crate) fn handle_load_object(
        &self,
        object_definition: String,
        options: moor_objdef::ObjDefLoaderOptions,
        _return_conflicts: bool,
    ) -> Result<moor_objdef::ObjDefLoaderResults, SchedulerError> {
        use moor_objdef::ObjectDefinitionLoader;

        // Create a new world state for loading
        let world_state = self
            .database
            .new_world_state()
            .map_err(|_| SchedulerError::CouldNotStartTask)?;

        let mut loader = Box::new(world_state)
            .as_loader_interface()
            .map_err(|_| SchedulerError::CouldNotStartTask)?;

        let mut object_loader = ObjectDefinitionLoader::new(loader.as_mut());

        // Load the object with the provided options
        let compile_options = self.config.features.compile_options();

        let result = object_loader
            .load_single_object(&object_definition, compile_options, options)
            .map_err(|_| SchedulerError::CouldNotStartTask)?;

        // Commit the transaction if the result says we should
        if result.commit {
            loader
                .commit()
                .map_err(|_| SchedulerError::CouldNotStartTask)?;
        }

        Ok(result)
    }

    /// Reloads an object definition, updating an existing object in the database.
    ///
    /// Creates a new world state, initializes an object definition loader,
    /// and reloads a single object from the provided definition string.
    /// Unlike load, this always commits the transaction (no dry-run mode).
    ///
    /// # Arguments
    /// * `object_definition` - The object definition string to reload
    /// * `constants` - Optional constants to use during reload
    /// * `target_obj` - Optional target object to reload into
    ///
    /// # Returns
    /// The loader results containing reloaded object information, or a SchedulerError
    pub(crate) fn handle_reload_object(
        &self,
        object_definition: String,
        constants: Option<moor_objdef::Constants>,
        target_obj: Option<Obj>,
    ) -> Result<moor_objdef::ObjDefLoaderResults, SchedulerError> {
        use moor_objdef::ObjectDefinitionLoader;

        // Create a new world state for reloading
        let world_state = self
            .database
            .new_world_state()
            .map_err(|_| SchedulerError::CouldNotStartTask)?;

        let mut loader = Box::new(world_state)
            .as_loader_interface()
            .map_err(|_| SchedulerError::CouldNotStartTask)?;

        let mut object_loader = ObjectDefinitionLoader::new(loader.as_mut());

        // Reload the object with the provided constants and target
        let result = object_loader
            .reload_single_object(&object_definition, constants, target_obj)
            .map_err(|_| SchedulerError::CouldNotStartTask)?;

        // Always commit for reload operations (they don't have dry-run mode)
        loader
            .commit()
            .map_err(|_| SchedulerError::CouldNotStartTask)?;

        Ok(result)
    }

    /// Drains and processes all immediate wake tasks from the suspended queue.
    ///
    /// Iterates through tasks that have been signaled for immediate wake,
    /// extracts their return values based on wake condition type (Immediate,
    /// Time, TaskMessage), and resumes them with the appropriate action.
    /// Handles latency recording and trace events when enabled.
    ///
    /// This method holds the lifecycle lock throughout execution to safely
    /// manipulate the suspended task queue.
    pub(crate) fn drain_immediate_wakes(&self) {
        let mut lc = self.lifecycle.lock();
        while let Some((task_id, signaled_at)) = lc.task_q.suspended.pop_immediate_wake() {
            // Inline the wake logic here since we already hold the lock.
            let Some(sr) = lc.task_q.suspended.remove_task(task_id) else {
                // Task was already removed (e.g., killed), ignore
                continue;
            };
            let perfc = sched_counters();
            TaskQ::record_latency(
                &perfc.task_wake_signal_to_dispatch_start_latency,
                signaled_at.instant(),
            );

            // Extract the return value from the wake condition
            // Note: Time-based tasks may arrive here if their timer expired before insertion
            let return_value = match &sr.wake_condition {
                WakeCondition::Immediate(val) => val.clone().unwrap_or_else(|| v_int(0)),
                WakeCondition::Time(_) => v_int(0), // Expired timer - return 0 as suspend() normally does
                WakeCondition::TaskMessage(_) => {
                    // Task was waiting for messages — drain the queue and return as list
                    let messages = lc.task_q.drain_messages(task_id);
                    List::from_iter(messages).into()
                }
                _ => {
                    error!(
                        ?task_id,
                        "Immediate wake task has unexpected wake condition"
                    );
                    v_int(0)
                }
            };

            #[cfg(feature = "trace_events")]
            {
                let max_ticks = sr.task.vm_host.max_ticks;
                let tick_count = sr.task.vm_host.tick_count();

                trace_task_resume!(
                    task_id,
                    "Immediate",
                    "Immediate wake",
                    to_literal(&return_value),
                    max_ticks,
                    tick_count
                );
            }

            if let Err(e) = lc.task_q.wake_suspended_task(
                sr,
                ResumeAction::Return(return_value),
                self,
                self.database.as_ref(),
                self.builtin_registry.clone(),
                self.config.clone(),
            ) {
                error!(?task_id, ?e, "Error resuming immediate wake task");
            }
        }
    }

    /// Handles a response from a worker task and resumes the suspended task.
    ///
    /// Converts worker responses (errors or successful responses) into appropriate
    /// resume actions, finds the suspended task associated with the request ID,
    /// and wakes it with the result. Handles error mapping from WorkerError to
    /// MOO error types and records trace events when enabled.
    ///
    /// # Arguments
    /// * `worker_response` - The response from a worker task containing either
    ///   an error or a successful result value
    ///
    /// # Notes
    /// If the suspended task is not found (e.g., was killed or expired), a warning
    /// is logged and the response is discarded.
    pub(crate) fn handle_worker_response(&self, worker_response: WorkerResponse) {
        let (request_id, resume_action) = match worker_response {
            WorkerResponse::Error { request_id, error } => {
                let err_msg = error.to_string();
                let err = match error {
                    WorkerError::PermissionDenied(_) => E_PERM.msg(err_msg),
                    WorkerError::NoWorkerAvailable(_) => E_TYPE.msg(err_msg),
                    WorkerError::InvalidRequest(_) => E_INVARG.msg(err_msg),
                    WorkerError::InternalError(_) => E_EXEC.msg(err_msg),
                    WorkerError::RequestTimedOut(_) => E_QUOTA.msg(err_msg),
                    WorkerError::RequestError(_) => E_INVARG.msg(err_msg),
                    WorkerError::WorkerDetached(_) => E_EXEC.msg(err_msg),
                };
                (request_id, ResumeAction::Raise(err))
            }
            WorkerResponse::Response {
                request_id,
                response,
            } => (request_id, ResumeAction::Return(response)),
        };

        let mut lc = self.lifecycle.lock();

        // Find the suspended task for this request.
        let task = lc.task_q.suspended.pull_task_for_worker(request_id);

        // Find the task that requested this input, if any
        let Some(sr) = task else {
            warn!(?request_id, "Task for worker request not found; expired?");
            return;
        };

        #[cfg(feature = "trace_events")]
        {
            let task_id = sr.task.task_id;
            let max_ticks = sr.task.vm_host.max_ticks;
            let tick_count = sr.task.vm_host.tick_count();

            let (return_value_str, wake_reason) = match &resume_action {
                ResumeAction::Return(v) => (to_literal(v), "Worker response"),
                ResumeAction::Raise(e) => (e.to_string(), "Worker error"),
            };

            trace_task_resume!(
                task_id,
                "Worker",
                wake_reason,
                return_value_str,
                max_ticks,
                tick_count
            );
        }

        if let Err(e) = lc.task_q.wake_suspended_task(
            sr,
            resume_action,
            self,
            self.database.as_ref(),
            self.builtin_registry.clone(),
            self.config.clone(),
        ) {
            error!("Failure to resume task after worker response: {:?}", e);
        }
    }

    pub(crate) fn checkpoint(&self) -> Result<(), SchedulerError> {
        start_checkpoint(
            self.database.as_ref(),
            self.config.as_ref(),
            &self.version,
            self.checkpoint_in_progress.clone(),
            CheckpointMode::NonBlocking,
        )
    }

    /// Request a checkpoint and wait for the textdump generation to complete.
    ///
    /// Unlike `checkpoint()`, this method blocks until the background textdump thread
    /// finishes, providing confirmation that the checkpoint has been written to disk.
    pub(crate) fn checkpoint_blocking(&self) -> Result<(), SchedulerError> {
        start_checkpoint(
            self.database.as_ref(),
            self.config.as_ref(),
            &self.version,
            self.checkpoint_in_progress.clone(),
            CheckpointMode::Blocking,
        )
    }

    /// Stop the scheduler run loop.
    pub(crate) fn stop(&self, msg: Option<String>) -> Result<(), SchedulerError> {
        // Send shutdown notification and kill all active tasks while holding the lock.
        {
            let mut lc = self.lifecycle.lock();

            // Notify all live tasks of shutdown.
            for (_, task) in lc.task_q.active.iter() {
                let _ = task.session.notify_shutdown(msg.clone());
            }
            warn!("Issuing clean shutdown...");

            // Kill all active tasks.
            for (_, task) in lc.task_q.active.drain() {
                task.kill_switch.store(true, Ordering::SeqCst);
            }
        }

        warn!("Waiting for tasks to finish...");

        // Wait for all active tasks to drain, polling with short sleeps.
        // Tasks complete quickly once killed, so this is bounded.
        loop {
            let is_empty = self.lifecycle.lock().task_q.active.is_empty();
            if is_empty {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }

        // Now ask the rpc server and hosts to shutdown (no lock held).
        self.system_control
            .shutdown(msg)
            .expect("Could not cleanly shutdown system");

        warn!("All tasks finished.  Stopping scheduler.");
        {
            let mut lc = self.lifecycle.lock();
            lc.running = false;
        }

        Ok(())
    }
}
