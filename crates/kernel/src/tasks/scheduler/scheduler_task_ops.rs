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
    pub(super) fn handle_switch_player(
        &mut self,
        task_id: TaskId,
        new_player: Obj,
    ) -> Result<(), Error> {
        // Get the current task to access its session
        let Some(task) = self.task_q.active.get_mut(&task_id) else {
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

        // Switch the player through the system control (which handles connection registry and host notification)
        self.system_control
            .switch_player(connection_obj, new_player)?;

        // Update the task's player
        task.player = new_player;

        Ok(())
    }

    pub(super) fn handle_dump_object(
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

    pub(super) fn handle_load_object(
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

    pub(super) fn handle_reload_object(
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

    pub(super) fn handle_get_workers_info(&self) -> Vec<WorkerInfo> {
        let Some(workers_sender) = self.worker_request_send.as_ref() else {
            warn!("No workers configured for scheduler; returning empty worker list");
            return vec![];
        };

        let request_id = Uuid::new_v4();

        // Send the workers info request
        if let Err(e) = workers_sender.send(WorkerRequest::GetWorkersInfo { request_id }) {
            error!("Failed to send workers info request: {e}");
            return vec![];
        }

        // Wait for the response (with timeout)
        let Some(worker_recv) = self.worker_request_recv.as_ref() else {
            error!("No worker response channel configured");
            return vec![];
        };

        match worker_recv.recv_timeout(Duration::from_secs(5)) {
            Ok(WorkerResponse::WorkersInfo {
                request_id: resp_id,
                workers_info,
            }) => {
                if resp_id == request_id {
                    workers_info
                } else {
                    warn!("Received workers info response with mismatched ID");
                    vec![]
                }
            }
            Ok(other_response) => {
                warn!("Received unexpected response type: {:?}", other_response);
                vec![]
            }
            Err(e) => {
                warn!("Timeout or error waiting for workers info response: {e}");
                vec![]
            }
        }
    }

    pub(super) fn drain_immediate_wakes(&mut self) {
        while let Some((task_id, signaled_at)) = self.task_q.suspended.pop_immediate_wake() {
            self.handle_immediate_wake(task_id, signaled_at);
        }
    }

    fn handle_immediate_wake(&mut self, task_id: TaskId, signaled_at: Timestamp) {
        // Handle a task queued for immediate wake
        let Some(sr) = self.task_q.suspended.remove_task(task_id) else {
            // Task was already removed (e.g., killed), ignore
            return;
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
                let messages = self.task_q.drain_messages(task_id);
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

        if let Err(e) = self.task_q.wake_suspended_task(
            sr,
            ResumeAction::Return(return_value),
            &self.task_control_sender,
            self.database.as_ref(),
            self.builtin_registry.clone(),
            self.config.clone(),
        ) {
            error!(?task_id, ?e, "Error resuming immediate wake task");
        }
    }

    pub(super) fn handle_worker_response(&mut self, worker_response: WorkerResponse) {
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
            WorkerResponse::WorkersInfo {
                request_id: _,
                workers_info: _,
            } => {
                // Workers info responses are handled synchronously in handle_get_workers_info
                // This shouldn't happen in the normal worker response flow
                warn!("Received unexpected WorkersInfo response in handle_worker_response");
                return;
            }
        };

        // Find the suspended task for this request.
        let task = self.task_q.suspended.pull_task_for_worker(request_id);

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

        if let Err(e) = self.task_q.wake_suspended_task(
            sr,
            resume_action,
            &self.task_control_sender,
            self.database.as_ref(),
            self.builtin_registry.clone(),
            self.config.clone(),
        ) {
            error!("Failure to resume task after worker response: {:?}", e);
        }
    }

    pub(super) fn checkpoint(&self) -> Result<(), SchedulerError> {
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
    pub(super) fn checkpoint_blocking(&self) -> Result<(), SchedulerError> {
        start_checkpoint(
            self.database.as_ref(),
            self.config.as_ref(),
            &self.version,
            self.checkpoint_in_progress.clone(),
            CheckpointMode::Blocking,
        )
    }

    pub(super) fn process_fork_request(
        &mut self,
        fork_request: Box<Fork>,
        reply: oneshot::Sender<TaskId>,
        session: Arc<dyn Session>,
    ) {
        // Fork the session.
        let forked_session = session.fork().unwrap();

        let suspended = fork_request.delay.is_some();
        let player = fork_request.player;
        let delay = fork_request.delay;
        let progr = fork_request.progr;

        let task_start = TaskStart::StartFork {
            fork_request,
            suspended,
        };
        let task_id = self.next_task_id;
        self.next_task_id += 1;
        if let Err(e) =
            self.submit_task(task_id, &player, &progr, task_start, delay, forked_session)
        {
            error!(?e, "Could not fork task");
            return;
        }

        if let Err(e) = reply.send(task_id) {
            error!(task = task_id, error = ?e, "Could not send fork reply. Parent task gone?");
        }
    }

    /// Stop the scheduler run loop.
    pub(super) fn stop(&mut self, msg: Option<String>) -> Result<(), SchedulerError> {
        // Send shutdown notification to all live tasks.
        for (_, task) in self.task_q.active.iter() {
            let _ = task.session.notify_shutdown(msg.clone());
        }
        warn!("Issuing clean shutdown...");
        {
            // Send shut down to all the tasks.
            for (_, task) in self.task_q.active.drain() {
                task.kill_switch.store(true, Ordering::SeqCst);
            }
        }
        warn!("Waiting for tasks to finish...");

        // Then spin until they're all done.
        loop {
            {
                if self.task_q.active.is_empty() {
                    break;
                }
            }
            yield_now();
        }

        // Now ask the rpc server and hosts to shutdown
        self.system_control
            .shutdown(msg)
            .expect("Could not cleanly shutdown system");

        warn!("All tasks finished.  Stopping scheduler.");
        self.running = false;

        Ok(())
    }
}
