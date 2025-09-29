// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use flume::Sender;
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

use moor_common::model::{ObjectRef, PropDef, PropPerms, VerbDef, VerbDefs};
use moor_compiler::{Program, compile};
use moor_var::{List, Obj, Symbol, Var};

use crate::{config::FeaturesConfig, tasks::TaskHandle};
use moor_common::tasks::{SchedulerError, SchedulerError::CompilationError, Session};

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
    pub(crate) scheduler_sender: Sender<SchedulerClientMsg>,
}

impl SchedulerClient {
    pub fn new(scheduler_sender: Sender<SchedulerClientMsg>) -> Self {
        Self { scheduler_sender }
    }

    /// Submit a command to the scheduler for execution.
    pub fn submit_command_task(
        &self,
        handler_object: &Obj,
        player: &Obj,
        command: &str,
        session: Arc<dyn Session>,
    ) -> Result<TaskHandle, SchedulerError> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::SubmitCommandTask {
                handler_object: *handler_object,
                player: *player,
                command: command.to_string(),
                session,
                reply,
            })
            .map_err(|_| SchedulerError::SchedulerNotResponding)?;

        receive
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| SchedulerError::SchedulerNotResponding)?
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
        argstr: String,
        perms: &Obj,
        session: Arc<dyn Session>,
    ) -> Result<TaskHandle, SchedulerError> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::SubmitVerbTask {
                player: *player,
                vloc: vloc.clone(),
                verb,
                args,
                argstr,
                perms: *perms,
                session,
                reply,
            })
            .map_err(|_| SchedulerError::SchedulerNotResponding)?;

        receive
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| SchedulerError::SchedulerNotResponding)?
    }

    /// Receive input that the (suspended) task previously requested, using the given
    /// `input_request_id`.
    /// The request is identified by the `input_request_id`, and given the input and resumed under
    /// a new transaction.
    pub fn submit_requested_input(
        &self,
        player: &Obj,
        input_request_id: Uuid,
        input: Var,
    ) -> Result<(), SchedulerError> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::SubmitTaskInput {
                player: *player,
                input_request_id,
                input,
                reply,
            })
            .map_err(|_| SchedulerError::SchedulerNotResponding)?;

        receive
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| SchedulerError::SchedulerNotResponding)?
    }

    pub fn submit_out_of_band_task(
        &self,
        handler_object: &Obj,
        player: &Obj,
        command: Vec<String>,
        argstr: String,
        session: Arc<dyn Session>,
    ) -> Result<TaskHandle, SchedulerError> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::SubmitOobTask {
                handler_object: *handler_object,
                player: *player,
                command,
                argstr,
                session,
                reply,
            })
            .map_err(|_| SchedulerError::SchedulerNotResponding)?;

        receive
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| SchedulerError::SchedulerNotResponding)?
    }

    /// Submit an eval task to the scheduler for execution.
    pub fn submit_eval_task(
        &self,
        player: &Obj,
        perms: &Obj,
        code: String,
        sessions: Arc<dyn Session>,
        config: Arc<FeaturesConfig>,
    ) -> Result<TaskHandle, SchedulerError> {
        // Compile the text into a verb.
        let program = match compile(code.as_str(), config.compile_options()) {
            Ok(b) => b,
            Err(e) => return Err(CompilationError(e)),
        };

        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::SubmitEvalTask {
                player: *player,
                perms: *perms,
                program,
                sessions,
                reply,
            })
            .map_err(|_| SchedulerError::SchedulerNotResponding)?;

        receive
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| SchedulerError::SchedulerNotResponding)?
    }

    pub fn submit_shutdown(&self, msg: &str) -> Result<(), SchedulerError> {
        // If we can't deliver a shutdown message, that's really a cause for panic!
        let (send, reply) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::Shutdown(msg.to_string(), send))
            .map_err(|_| SchedulerError::SchedulerNotResponding)?;
        reply
            .recv()
            .map_err(|_| SchedulerError::SchedulerNotResponding)?
    }

    pub fn submit_verb_program(
        &self,
        player: &Obj,
        perms: &Obj,
        obj: &ObjectRef,
        verb_name: Symbol,
        code: Vec<String>,
    ) -> Result<(Obj, Symbol), SchedulerError> {
        use crate::tasks::world_state_action::{WorldStateAction, WorldStateRequest};

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
            Some(crate::tasks::world_state_action::WorldStateResponse::Success {
                result:
                    crate::tasks::world_state_action::WorldStateResult::VerbProgrammed { object, verb },
                ..
            }) => Ok((object, verb)),
            Some(crate::tasks::world_state_action::WorldStateResponse::Error { error, .. }) => {
                Err(error)
            }
            _ => Err(SchedulerError::SchedulerNotResponding),
        }
    }

    pub fn request_system_property(
        &self,
        player: &Obj,
        obj: &ObjectRef,
        property: Symbol,
    ) -> Result<Var, SchedulerError> {
        use crate::tasks::world_state_action::{WorldStateAction, WorldStateRequest};

        let action = WorldStateAction::RequestSystemProperty {
            player: *player,
            obj: obj.clone(),
            property,
        };
        let request = WorldStateRequest::new(action);
        let responses = self.execute_world_state_actions(vec![request], false)?;

        match responses.into_iter().next() {
            Some(crate::tasks::world_state_action::WorldStateResponse::Success {
                result: crate::tasks::world_state_action::WorldStateResult::SystemProperty(value),
                ..
            }) => Ok(value),
            Some(crate::tasks::world_state_action::WorldStateResponse::Error { error, .. }) => {
                Err(error)
            }
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
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::Checkpoint(blocking, reply))
            .map_err(|_| SchedulerError::SchedulerNotResponding)?;

        let timeout = if blocking {
            Duration::from_secs(600) // 10 minutes for large textdumps
        } else {
            Duration::from_secs(30) // 30 seconds for checkpoint initiation (snapshot creation can be slow)
        };

        receive
            .recv_timeout(timeout)
            .map_err(|_| SchedulerError::SchedulerNotResponding)?
    }

    /// Check if the scheduler is alive and responding (lightweight operation)
    pub fn check_status(&self) -> Result<(), SchedulerError> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::CheckStatus(reply))
            .map_err(|_| SchedulerError::SchedulerNotResponding)?;

        receive
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| SchedulerError::SchedulerNotResponding)?
    }

    /// Get garbage collection statistics from the scheduler
    pub fn get_gc_stats(&self) -> Result<GCStats, SchedulerError> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::GetGCStats(reply))
            .map_err(|_| SchedulerError::SchedulerNotResponding)?;

        receive
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| SchedulerError::SchedulerNotResponding)?
    }

    /// Request a garbage collection cycle from the scheduler
    pub fn request_gc(&self) -> Result<(), SchedulerError> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::RequestGC(reply))
            .map_err(|_| SchedulerError::SchedulerNotResponding)?;

        receive
            .recv_timeout(Duration::from_secs(10)) // Longer timeout since GC might take time
            .map_err(|_| SchedulerError::SchedulerNotResponding)?
    }

    pub fn request_verbs(
        &self,
        player: &Obj,
        perms: &Obj,
        obj: &ObjectRef,
        inherited: bool,
    ) -> Result<VerbDefs, SchedulerError> {
        use crate::tasks::world_state_action::{WorldStateAction, WorldStateRequest};

        let action = WorldStateAction::RequestVerbs {
            player: *player,
            perms: *perms,
            obj: obj.clone(),
            inherited,
        };
        let request = WorldStateRequest::new(action);
        let responses = self.execute_world_state_actions(vec![request], false)?;

        match responses.into_iter().next() {
            Some(crate::tasks::world_state_action::WorldStateResponse::Success {
                result: crate::tasks::world_state_action::WorldStateResult::Verbs(verbs),
                ..
            }) => Ok(verbs),
            Some(crate::tasks::world_state_action::WorldStateResponse::Error { error, .. }) => {
                Err(error)
            }
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
        use crate::tasks::world_state_action::{WorldStateAction, WorldStateRequest};

        let action = WorldStateAction::RequestVerbCode {
            player: *player,
            perms: *perms,
            obj: obj.clone(),
            verb,
        };
        let request = WorldStateRequest::new(action);
        let responses = self.execute_world_state_actions(vec![request], false)?;

        match responses.into_iter().next() {
            Some(crate::tasks::world_state_action::WorldStateResponse::Success {
                result: crate::tasks::world_state_action::WorldStateResult::VerbCode(verbdef, code),
                ..
            }) => Ok((verbdef, code)),
            Some(crate::tasks::world_state_action::WorldStateResponse::Error { error, .. }) => {
                Err(error)
            }
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
        use crate::tasks::world_state_action::{WorldStateAction, WorldStateRequest};

        let action = WorldStateAction::RequestProperties {
            player: *player,
            perms: *perms,
            obj: obj.clone(),
            inherited,
        };
        let request = WorldStateRequest::new(action);
        let responses = self.execute_world_state_actions(vec![request], false)?;

        match responses.into_iter().next() {
            Some(crate::tasks::world_state_action::WorldStateResponse::Success {
                result: crate::tasks::world_state_action::WorldStateResult::Properties(props),
                ..
            }) => Ok(props),
            Some(crate::tasks::world_state_action::WorldStateResponse::Error { error, .. }) => {
                Err(error)
            }
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
        use crate::tasks::world_state_action::{WorldStateAction, WorldStateRequest};

        let action = WorldStateAction::RequestProperty {
            player: *player,
            perms: *perms,
            obj: obj.clone(),
            property,
        };
        let request = WorldStateRequest::new(action);
        let responses = self.execute_world_state_actions(vec![request], false)?;

        match responses.into_iter().next() {
            Some(crate::tasks::world_state_action::WorldStateResponse::Success {
                result:
                    crate::tasks::world_state_action::WorldStateResult::Property(info, perms, value),
                ..
            }) => Ok((info, perms, value)),
            Some(crate::tasks::world_state_action::WorldStateResponse::Error { error, .. }) => {
                Err(error)
            }
            _ => Err(SchedulerError::SchedulerNotResponding),
        }
    }

    pub fn resolve_object(&self, player: Obj, obj: ObjectRef) -> Result<Var, SchedulerError> {
        use crate::tasks::world_state_action::{WorldStateAction, WorldStateRequest};

        let action = WorldStateAction::ResolveObject { player, obj };
        let request = WorldStateRequest::new(action);
        let responses = self.execute_world_state_actions(vec![request], false)?;

        match responses.into_iter().next() {
            Some(crate::tasks::world_state_action::WorldStateResponse::Success {
                result: crate::tasks::world_state_action::WorldStateResult::ResolvedObject(value),
                ..
            }) => Ok(value),
            Some(crate::tasks::world_state_action::WorldStateResponse::Error { error, .. }) => {
                Err(error)
            }
            _ => Err(SchedulerError::SchedulerNotResponding),
        }
    }

    /// Execute a batch of WorldStateActions.
    pub fn execute_world_state_actions(
        &self,
        actions: Vec<crate::tasks::world_state_action::WorldStateRequest>,
        rollback: bool,
    ) -> Result<Vec<crate::tasks::world_state_action::WorldStateResponse>, SchedulerError> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::ExecuteWorldStateActions {
                actions,
                rollback,
                reply,
            })
            .map_err(|_| SchedulerError::SchedulerNotResponding)?;

        receive
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| SchedulerError::SchedulerNotResponding)?
    }
}

pub enum SchedulerClientMsg {
    /// Submit a command to be executed by the player.
    SubmitCommandTask {
        handler_object: Obj,
        player: Obj,
        command: String,
        session: Arc<dyn Session>,
        reply: oneshot::Sender<Result<TaskHandle, SchedulerError>>,
    },
    /// Submit a top-level verb (method) invocation to be executed on behalf of the player.
    SubmitVerbTask {
        player: Obj,
        vloc: ObjectRef,
        verb: Symbol,
        args: List,
        argstr: String,
        perms: Obj,
        session: Arc<dyn Session>,
        reply: oneshot::Sender<Result<TaskHandle, SchedulerError>>,
    },
    /// Submit input to a task that is waiting for it.
    SubmitTaskInput {
        player: Obj,
        input_request_id: Uuid,
        input: Var,
        reply: oneshot::Sender<Result<(), SchedulerError>>,
    },
    /// Submit an out-of-band task to be executed
    SubmitOobTask {
        handler_object: Obj,
        player: Obj,
        command: Vec<String>,
        argstr: String,
        session: Arc<dyn Session>,
        reply: oneshot::Sender<Result<TaskHandle, SchedulerError>>,
    },
    /// Submit an eval task
    SubmitEvalTask {
        player: Obj,
        perms: Obj,
        program: Program,
        sessions: Arc<dyn Session>,
        reply: oneshot::Sender<Result<TaskHandle, SchedulerError>>,
    },
    /// Submit a request to checkpoint the database.
    /// If the boolean is true, waits for textdump generation to complete.
    Checkpoint(bool, oneshot::Sender<Result<(), SchedulerError>>),
    /// Submit a (non-task specific) request to shutdown the scheduler
    Shutdown(String, oneshot::Sender<Result<(), SchedulerError>>),
    /// Check if the scheduler is alive and responding (lightweight operation)
    CheckStatus(oneshot::Sender<Result<(), SchedulerError>>),
    /// Execute a batch of WorldStateActions
    ExecuteWorldStateActions {
        actions: Vec<crate::tasks::world_state_action::WorldStateRequest>,
        /// Rollback after performing the operations, leaving the world in the state it was before
        /// we ran. (For exploratory actions)
        rollback: bool,
        reply: oneshot::Sender<
            Result<Vec<crate::tasks::world_state_action::WorldStateResponse>, SchedulerError>,
        >,
    },
    /// Get garbage collection statistics
    GetGCStats(oneshot::Sender<Result<GCStats, SchedulerError>>),
    /// Request a garbage collection cycle
    RequestGC(oneshot::Sender<Result<(), SchedulerError>>),
    /// Internal message from GC thread when mark phase completes
    GCMarkPhaseComplete {
        unreachable_objects: std::collections::HashSet<Obj>,
        mutation_timestamp_before_mark: Option<u64>,
    },
}
