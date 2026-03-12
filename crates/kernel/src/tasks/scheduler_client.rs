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

use std::sync::Arc;

use moor_common::model::{ObjectRef, PropDef, PropPerms, VerbDef, VerbDefs};
use moor_common::tasks::{SchedulerError, SchedulerError::CompilationError, Session};
use moor_common::util::PerfTimerGuard;
use moor_compiler::compile;
use moor_var::{List, Obj, Symbol, Var};

use crate::tasks::scheduler::Scheduler;
use crate::tasks::world_state_action::{
    WorldStateAction, WorldStateRequest, WorldStateResponse, WorldStateResult,
};
use crate::{
    config::FeaturesConfig,
    tasks::{TaskHandle, sched_counters},
};

/// Garbage collection statistics
#[derive(Debug, Clone)]
pub struct GCStats {
    /// Total number of GC cycles completed
    pub cycle_count: u64,
}

/// A handle for talking to the scheduler from the outside world.
/// This is not meant to be used by running tasks, but by the rpc daemon, tests, etc.
/// Handles requests for task submission, shutdown, etc.
#[derive(Clone)]
pub struct SchedulerClient {
    scheduler: Scheduler,
}

impl SchedulerClient {
    pub fn new(scheduler: Scheduler) -> Self {
        Self { scheduler }
    }

    /// Submit a command to the scheduler for execution.
    pub fn submit_command_task(
        &self,
        handler_object: &Obj,
        player: &Obj,
        command: &str,
        session: Arc<dyn Session>,
    ) -> Result<TaskHandle, SchedulerError> {
        let _timer = PerfTimerGuard::new(&sched_counters().submit_command_task_latency);

        self.scheduler.submit_command_task_inner(
            *handler_object,
            *player,
            command.to_string(),
            session,
        )
    }

    /// Submit a verb task to the scheduler for execution.
    /// (This path is really only used for the invocations from the serving processes like login,
    /// user_connected, or the do_command invocation which precedes an internal parser attempt.)
    // Yes yes I know it's a lot of arguments, but wrapper object here is redundant.
    #[allow(clippy::too_many_arguments)]
    pub fn submit_verb_task(
        &self,
        player: &Obj,
        vloc: &ObjectRef,
        verb: Symbol,
        args: List,
        argstr: Var,
        perms: &Obj,
        session: Arc<dyn Session>,
    ) -> Result<TaskHandle, SchedulerError> {
        let _timer = PerfTimerGuard::new(&sched_counters().submit_verb_task_latency);

        self.scheduler.submit_verb_task_inner(
            *player,
            vloc.clone(),
            verb,
            args,
            argstr,
            *perms,
            session,
        )
    }

    /// Receive input that the (suspended) task previously requested, using the given
    /// `input_request_id`.
    /// The request is identified by the `input_request_id`, and given the input and resumed under
    /// a new transaction.
    pub fn submit_requested_input(
        &self,
        player: &Obj,
        input_request_id: uuid::Uuid,
        input: Var,
    ) -> Result<(), SchedulerError> {
        self.scheduler
            .submit_task_input_inner(*player, input_request_id, input)
    }

    pub fn submit_out_of_band_task(
        &self,
        handler_object: &Obj,
        player: &Obj,
        command: List,
        argstr: Var,
        session: Arc<dyn Session>,
    ) -> Result<TaskHandle, SchedulerError> {
        let _timer = PerfTimerGuard::new(&sched_counters().submit_oob_task_latency);

        self.scheduler.submit_oob_task_inner(
            *handler_object,
            *player,
            command,
            argstr,
            session,
        )
    }

    /// Submit an eval task to the scheduler for execution.
    pub fn submit_eval_task(
        &self,
        player: &Obj,
        perms: &Obj,
        code: String,
        initial_env: Option<Vec<(Symbol, Var)>>,
        sessions: Arc<dyn Session>,
        config: Arc<FeaturesConfig>,
    ) -> Result<TaskHandle, SchedulerError> {
        let _timer = PerfTimerGuard::new(&sched_counters().submit_eval_task_latency);

        // Compile the text into a verb.
        let program = match compile(code.as_str(), config.compile_options()) {
            Ok(b) => b,
            Err(e) => return Err(CompilationError(e)),
        };

        self.scheduler.submit_eval_task_inner(
            *player,
            *perms,
            program,
            initial_env,
            sessions,
        )
    }

    pub fn submit_shutdown(&self, msg: &str) -> Result<(), SchedulerError> {
        self.scheduler.handle_shutdown_request(msg.to_string())
    }

    pub fn submit_verb_program(
        &self,
        player: &Obj,
        perms: &Obj,
        obj: &ObjectRef,
        verb_name: Symbol,
        code: Vec<String>,
    ) -> Result<(Obj, Symbol), SchedulerError> {
        let action = WorldStateAction::ProgramVerb {
            player: *player,
            perms: *perms,
            obj: obj.clone(),
            verb_name,
            code,
        };
        let request = WorldStateRequest::new(action);
        let responses = self.execute_world_state_actions(vec![request], false)?;

        match responses.into_iter().next() {
            Some(WorldStateResponse::Success {
                result: WorldStateResult::VerbProgrammed { object, verb },
                ..
            }) => Ok((object, verb)),
            Some(WorldStateResponse::Error { error, .. }) => Err(error),
            _ => Err(SchedulerError::SchedulerNotResponding),
        }
    }

    pub fn request_system_property(
        &self,
        player: &Obj,
        obj: &ObjectRef,
        property: Symbol,
    ) -> Result<Var, SchedulerError> {
        let action = WorldStateAction::RequestSystemProperty {
            player: *player,
            obj: obj.clone(),
            property,
        };
        let request = WorldStateRequest::new(action);
        let responses = self.execute_world_state_actions(vec![request], false)?;

        match responses.into_iter().next() {
            Some(WorldStateResponse::Success {
                result: WorldStateResult::SystemProperty(value),
                ..
            }) => Ok(value),
            Some(WorldStateResponse::Error { error, .. }) => Err(error),
            _ => Err(SchedulerError::SchedulerNotResponding),
        }
    }

    pub fn request_checkpoint(&self) -> Result<(), SchedulerError> {
        self.request_checkpoint_with_blocking(false)
    }

    /// Request a checkpoint and wait for the textdump generation to complete.
    ///
    /// This method blocks until the background textdump thread finishes, providing
    /// confirmation that the checkpoint has actually been written to disk.
    /// Uses a longer timeout (10 minutes) to accommodate large database exports.
    pub fn request_checkpoint_blocking(&self) -> Result<(), SchedulerError> {
        self.request_checkpoint_with_blocking(true)
    }

    /// Request a checkpoint with optional blocking behavior.
    ///
    /// If `blocking` is true, waits for the textdump generation to complete.
    /// If false, returns immediately after initiating the checkpoint.
    pub fn request_checkpoint_with_blocking(&self, blocking: bool) -> Result<(), SchedulerError> {
        let _timer = PerfTimerGuard::new(&sched_counters().checkpoint_latency);

        self.scheduler.handle_checkpoint_request(blocking)
    }

    /// Check if the scheduler is alive and responding (lightweight operation)
    pub fn check_status(&self) -> Result<(), SchedulerError> {
        self.scheduler.handle_check_status()
    }

    /// Get garbage collection statistics from the scheduler
    pub fn get_gc_stats(&self) -> Result<GCStats, SchedulerError> {
        self.scheduler.handle_get_gc_stats()
    }

    /// Request a garbage collection cycle from the scheduler
    pub fn request_gc(&self) -> Result<(), SchedulerError> {
        self.scheduler.handle_request_gc()
    }

    pub fn request_verbs(
        &self,
        player: &Obj,
        perms: &Obj,
        obj: &ObjectRef,
        inherited: bool,
    ) -> Result<VerbDefs, SchedulerError> {
        let action = WorldStateAction::RequestVerbs {
            player: *player,
            perms: *perms,
            obj: obj.clone(),
            inherited,
        };
        let request = WorldStateRequest::new(action);
        let responses = self.execute_world_state_actions(vec![request], false)?;

        match responses.into_iter().next() {
            Some(WorldStateResponse::Success {
                result: WorldStateResult::Verbs(verbs),
                ..
            }) => Ok(verbs),
            Some(WorldStateResponse::Error { error, .. }) => Err(error),
            _ => Err(SchedulerError::SchedulerNotResponding),
        }
    }

    pub fn request_verb(
        &self,
        player: &Obj,
        perms: &Obj,
        obj: &ObjectRef,
        verb: Symbol,
    ) -> Result<(VerbDef, Vec<String>), SchedulerError> {
        let action = WorldStateAction::RequestVerbCode {
            player: *player,
            perms: *perms,
            obj: obj.clone(),
            verb,
        };
        let request = WorldStateRequest::new(action);
        let responses = self.execute_world_state_actions(vec![request], false)?;

        match responses.into_iter().next() {
            Some(WorldStateResponse::Success {
                result: WorldStateResult::VerbCode(verbdef, code),
                ..
            }) => Ok((verbdef, code)),
            Some(WorldStateResponse::Error { error, .. }) => Err(error),
            _ => Err(SchedulerError::SchedulerNotResponding),
        }
    }

    pub fn request_properties(
        &self,
        player: &Obj,
        perms: &Obj,
        obj: &ObjectRef,
        inherited: bool,
    ) -> Result<Vec<(PropDef, PropPerms)>, SchedulerError> {
        let action = WorldStateAction::RequestProperties {
            player: *player,
            perms: *perms,
            obj: obj.clone(),
            inherited,
        };
        let request = WorldStateRequest::new(action);
        let responses = self.execute_world_state_actions(vec![request], false)?;

        match responses.into_iter().next() {
            Some(WorldStateResponse::Success {
                result: WorldStateResult::Properties(props),
                ..
            }) => Ok(props),
            Some(WorldStateResponse::Error { error, .. }) => Err(error),
            _ => Err(SchedulerError::SchedulerNotResponding),
        }
    }

    pub fn request_property(
        &self,
        player: &Obj,
        perms: &Obj,
        obj: &ObjectRef,
        property: Symbol,
    ) -> Result<(PropDef, PropPerms, Var), SchedulerError> {
        let action = WorldStateAction::RequestProperty {
            player: *player,
            perms: *perms,
            obj: obj.clone(),
            property,
        };
        let request = WorldStateRequest::new(action);
        let responses = self.execute_world_state_actions(vec![request], false)?;

        match responses.into_iter().next() {
            Some(WorldStateResponse::Success {
                result: WorldStateResult::Property(info, perms, value),
                ..
            }) => Ok((info, perms, value)),
            Some(WorldStateResponse::Error { error, .. }) => Err(error),
            _ => Err(SchedulerError::SchedulerNotResponding),
        }
    }

    pub fn resolve_object(&self, player: Obj, obj: ObjectRef) -> Result<Var, SchedulerError> {
        let action = WorldStateAction::ResolveObject { player, obj };
        let request = WorldStateRequest::new(action);
        let responses = self.execute_world_state_actions(vec![request], false)?;

        match responses.into_iter().next() {
            Some(WorldStateResponse::Success {
                result: WorldStateResult::ResolvedObject(value),
                ..
            }) => Ok(value),
            Some(WorldStateResponse::Error { error, .. }) => Err(error),
            _ => Err(SchedulerError::SchedulerNotResponding),
        }
    }

    /// Execute a batch of WorldStateActions.
    pub fn execute_world_state_actions(
        &self,
        actions: Vec<WorldStateRequest>,
        rollback: bool,
    ) -> Result<Vec<WorldStateResponse>, SchedulerError> {
        self.scheduler
            .execute_world_state_actions_inner(actions, rollback)
    }

    /// Submit a batch of WorldStateActions as a tracked task.
    /// Returns a `TaskHandle` and a shared sink where the batch results will be
    /// deposited before the task reports success.
    /// Unlike `execute_world_state_actions`, this creates a proper task visible
    /// to `queued_tasks()` and subject to task limits.
    pub fn submit_batch_world_state_task(
        &self,
        player: &Obj,
        perms: &Obj,
        actions: Vec<WorldStateAction>,
        rollback: bool,
        session: Arc<dyn Session>,
    ) -> Result<
        (
            TaskHandle,
            Arc<std::sync::Mutex<Option<Result<Vec<WorldStateResult>, SchedulerError>>>>,
        ),
        SchedulerError,
    > {
        let result_sink: Arc<
            std::sync::Mutex<Option<Result<Vec<WorldStateResult>, SchedulerError>>>,
        > = Arc::new(std::sync::Mutex::new(None));

        let handle = self.scheduler.submit_batch_world_state_task_inner(
            *player,
            *perms,
            actions,
            rollback,
            result_sink.clone(),
            session,
        )?;

        Ok((handle, result_sink))
    }

    /// Load an object from objdef text.
    pub fn load_object(
        &self,
        object_definition: String,
        options: moor_objdef::ObjDefLoaderOptions,
        return_conflicts: bool,
    ) -> Result<moor_objdef::ObjDefLoaderResults, SchedulerError> {
        let _timer = PerfTimerGuard::new(&sched_counters().load_object_latency);

        self.scheduler
            .handle_load_object_request(object_definition, options, return_conflicts)
    }

    /// Submit a system handler task with proper permissions lookup.
    /// This method looks up the #0.invoke_handler_perms property and uses that user
    /// as the permissions object for the verb invocation.
    pub fn submit_system_handler_task(
        &self,
        player: &Obj,
        handler_type: String,
        args: Vec<Var>,
        session: Arc<dyn Session>,
    ) -> Result<TaskHandle, SchedulerError> {
        let _timer = PerfTimerGuard::new(&sched_counters().submit_system_handler_task_latency);

        self.scheduler
            .submit_system_handler_task_inner(*player, handler_type, args, session)
    }

    /// Reload an existing object from objdef text, completely replacing its contents.
    pub fn reload_object(
        &self,
        object_definition: String,
        constants: Option<moor_objdef::Constants>,
        target_obj: Option<Obj>,
    ) -> Result<moor_objdef::ObjDefLoaderResults, SchedulerError> {
        let _timer = PerfTimerGuard::new(&sched_counters().reload_object_latency);

        self.scheduler
            .handle_reload_object_request(object_definition, constants, target_obj)
    }

    /// Get all objects in the database (for tab completion)
    pub fn request_all_objects(&self, player: Obj) -> Result<Vec<Obj>, SchedulerError> {
        let action = WorldStateAction::RequestAllObjects { player };
        let request = WorldStateRequest::new(action);
        let responses = self.execute_world_state_actions(vec![request], false)?;

        match responses.into_iter().next() {
            Some(WorldStateResponse::Success {
                result: WorldStateResult::AllObjects(objects),
                ..
            }) => Ok(objects),
            Some(WorldStateResponse::Error { error, .. }) => Err(error),
            _ => Err(SchedulerError::SchedulerNotResponding),
        }
    }

    /// List all objects with metadata (for object browser)
    pub fn list_objects(
        &self,
        player: &Obj,
    ) -> Result<Vec<(Obj, moor_common::model::ObjAttrs, usize, usize)>, SchedulerError> {
        let action = WorldStateAction::ListObjects { player: *player };
        let request = WorldStateRequest::new(action);
        let responses = self.execute_world_state_actions(vec![request], false)?;

        match responses.into_iter().next() {
            Some(WorldStateResponse::Success {
                result: WorldStateResult::ObjectsList(objects),
                ..
            }) => Ok(objects),
            Some(WorldStateResponse::Error { error, .. }) => Err(error),
            _ => Err(SchedulerError::SchedulerNotResponding),
        }
    }

    /// Get flags for a specific object
    pub fn get_object_flags(&self, obj: &Obj) -> Result<u16, SchedulerError> {
        let action = WorldStateAction::GetObjectFlags { obj: *obj };
        let request = WorldStateRequest::new(action);
        let responses = self.execute_world_state_actions(vec![request], false)?;

        match responses.into_iter().next() {
            Some(WorldStateResponse::Success {
                result: WorldStateResult::ObjectFlags(flags),
                ..
            }) => Ok(flags),
            Some(WorldStateResponse::Error { error, .. }) => Err(error),
            _ => Err(SchedulerError::SchedulerNotResponding),
        }
    }

    /// Update a property value
    pub fn update_property(
        &self,
        player: &Obj,
        perms: &Obj,
        obj: &ObjectRef,
        property: Symbol,
        value: Var,
    ) -> Result<(), SchedulerError> {
        let action = WorldStateAction::UpdateProperty {
            player: *player,
            perms: *perms,
            obj: obj.clone(),
            property,
            value,
        };
        let request = WorldStateRequest::new(action);
        let responses = self.execute_world_state_actions(vec![request], false)?;

        match responses.into_iter().next() {
            Some(WorldStateResponse::Success {
                result: WorldStateResult::PropertyUpdated,
                ..
            }) => Ok(()),
            Some(WorldStateResponse::Error { error, .. }) => Err(error),
            _ => Err(SchedulerError::SchedulerNotResponding),
        }
    }
}
