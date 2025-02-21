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

use crossbeam_channel::Sender;
use std::sync::Arc;
use std::time::Duration;
use tracing::{instrument, trace};
use uuid::Uuid;

use moor_compiler::{Program, compile};
use moor_values::model::{ObjectRef, PropDef, PropPerms, VerbDef, VerbDefs};
use moor_values::{List, Obj, Symbol, Var};

use crate::config::FeaturesConfig;
use crate::tasks::TaskHandle;
use crate::tasks::sessions::Session;
use moor_values::tasks::SchedulerError;
use moor_values::tasks::SchedulerError::CompilationError;

/// A handle for talking to the scheduler from the outside world.
/// This is not meant to be used by running tasks, but by the rpc daemon, tests, etc.
/// Handles requests for task submission, shutdown, etc.
#[derive(Clone)]
pub struct SchedulerClient {
    scheduler_sender: Sender<SchedulerClientMsg>,
}

impl SchedulerClient {
    pub fn new(scheduler_sender: Sender<SchedulerClientMsg>) -> Self {
        Self { scheduler_sender }
    }

    /// Submit a command to the scheduler for execution.
    #[instrument(skip(self, session))]
    pub fn submit_command_task(
        &self,
        handler_object: &Obj,
        player: &Obj,
        command: &str,
        session: Arc<dyn Session>,
    ) -> Result<TaskHandle, SchedulerError> {
        trace!(?player, ?command, "Command submitting");
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::SubmitCommandTask {
                handler_object: handler_object.clone(),
                player: player.clone(),
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
    #[instrument(skip(self, session))]
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
        trace!(?player, ?verb, ?args, "Verb submitting");
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::SubmitVerbTask {
                player: player.clone(),
                vloc: vloc.clone(),
                verb: Symbol::mk_case_insensitive(verb.as_str()),
                args,
                argstr,
                perms: perms.clone(),
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
        input: String,
    ) -> Result<(), SchedulerError> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::SubmitTaskInput {
                player: player.clone(),
                input_request_id,
                input,
                reply,
            })
            .map_err(|_| SchedulerError::SchedulerNotResponding)?;

        receive
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| SchedulerError::SchedulerNotResponding)?
    }

    #[instrument(skip(self, session))]
    pub fn submit_out_of_band_task(
        &self,
        handler_object: &Obj,
        player: &Obj,
        command: Vec<String>,
        argstr: String,
        session: Arc<dyn Session>,
    ) -> Result<TaskHandle, SchedulerError> {
        trace!(?player, ?command, "Out-of-band task submitting");
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::SubmitOobTask {
                handler_object: handler_object.clone(),
                player: player.clone(),
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
    #[instrument(skip(self, sessions))]
    pub fn submit_eval_task(
        &self,
        player: &Obj,
        perms: &Obj,
        code: String,
        sessions: Arc<dyn Session>,
        config: FeaturesConfig,
    ) -> Result<TaskHandle, SchedulerError> {
        // Compile the text into a verb.
        let program = match compile(code.as_str(), config.compile_options()) {
            Ok(b) => b,
            Err(e) => return Err(CompilationError(e)),
        };

        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::SubmitEvalTask {
                player: player.clone(),
                perms: perms.clone(),
                program,
                sessions,
                reply,
            })
            .map_err(|_| SchedulerError::SchedulerNotResponding)?;

        receive
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| SchedulerError::SchedulerNotResponding)?
    }

    #[instrument(skip(self))]
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
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::SubmitProgramVerb {
                player: player.clone(),
                perms: perms.clone(),
                obj: obj.clone(),
                verb_name,
                code,
                reply,
            })
            .map_err(|_| SchedulerError::SchedulerNotResponding)?;

        receive
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| SchedulerError::SchedulerNotResponding)?
    }

    pub fn request_system_property(
        &self,
        player: &Obj,
        obj: &ObjectRef,
        property: Symbol,
    ) -> Result<Var, SchedulerError> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::RequestSystemProperty {
                player: player.clone(),
                obj: obj.clone(),
                property,
                reply,
            })
            .map_err(|_| SchedulerError::SchedulerNotResponding)?;

        receive
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| SchedulerError::SchedulerNotResponding)?
    }

    pub fn request_checkpoint(&self) -> Result<(), SchedulerError> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::Checkpoint(reply))
            .map_err(|_| SchedulerError::SchedulerNotResponding)?;

        receive
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| SchedulerError::SchedulerNotResponding)?
    }

    pub fn request_verbs(
        &self,
        player: &Obj,
        perms: &Obj,
        obj: &ObjectRef,
    ) -> Result<VerbDefs, SchedulerError> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::RequestVerbs {
                player: player.clone(),
                perms: perms.clone(),
                obj: obj.clone(),
                reply,
            })
            .map_err(|_| SchedulerError::SchedulerNotResponding)?;

        receive
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| SchedulerError::SchedulerNotResponding)?
    }

    pub fn request_verb(
        &self,
        player: &Obj,
        perms: &Obj,
        obj: &ObjectRef,
        verb: Symbol,
    ) -> Result<(VerbDef, Vec<String>), SchedulerError> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::RequestVerbCode {
                player: player.clone(),
                perms: perms.clone(),
                obj: obj.clone(),
                verb,
                reply,
            })
            .map_err(|_| SchedulerError::SchedulerNotResponding)?;

        receive
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| SchedulerError::SchedulerNotResponding)?
    }

    pub fn request_properties(
        &self,
        player: &Obj,
        perms: &Obj,
        obj: &ObjectRef,
    ) -> Result<Vec<(PropDef, PropPerms)>, SchedulerError> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::RequestProperties {
                player: player.clone(),
                perms: perms.clone(),
                obj: obj.clone(),
                reply,
            })
            .map_err(|_| SchedulerError::SchedulerNotResponding)?;

        receive
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| SchedulerError::SchedulerNotResponding)?
    }

    pub fn request_property(
        &self,
        player: &Obj,
        perms: &Obj,
        obj: &ObjectRef,
        property: Symbol,
    ) -> Result<(PropDef, PropPerms, Var), SchedulerError> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::RequestProperty {
                player: player.clone(),
                perms: perms.clone(),
                obj: obj.clone(),
                property,
                reply,
            })
            .map_err(|_| SchedulerError::SchedulerNotResponding)?;

        receive
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| SchedulerError::SchedulerNotResponding)?
    }

    pub fn resolve_object(&self, player: Obj, obj: ObjectRef) -> Result<Var, SchedulerError> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::ResolveObject { player, obj, reply })
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
        input: String,
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
    /// Submit a request to program a verb
    SubmitProgramVerb {
        player: Obj,
        perms: Obj,
        obj: ObjectRef,
        verb_name: Symbol,
        code: Vec<String>,
        reply: oneshot::Sender<Result<(Obj, Symbol), SchedulerError>>,
    },
    /// Request the value of a $property.
    /// (Used by the login process, unauthenticated)
    RequestSystemProperty {
        player: Obj,
        obj: ObjectRef,
        property: Symbol,
        reply: oneshot::Sender<Result<Var, SchedulerError>>,
    },
    /// Request the list of visible verbs on an object.
    RequestVerbs {
        player: Obj,
        perms: Obj,
        obj: ObjectRef,
        reply: oneshot::Sender<Result<VerbDefs, SchedulerError>>,
    },
    /// Request the decompiled code of a verb along with its definition.
    RequestVerbCode {
        player: Obj,
        perms: Obj,
        obj: ObjectRef,
        verb: Symbol,
        reply: oneshot::Sender<Result<(VerbDef, Vec<String>), SchedulerError>>,
    },
    /// Request the list of visible properties on an object.
    RequestProperties {
        player: Obj,
        perms: Obj,
        obj: ObjectRef,
        reply: oneshot::Sender<Result<Vec<(PropDef, PropPerms)>, SchedulerError>>,
    },
    /// Request the description and contents of a property.
    RequestProperty {
        player: Obj,
        perms: Obj,
        obj: ObjectRef,
        property: Symbol,
        reply: oneshot::Sender<Result<(PropDef, PropPerms, Var), SchedulerError>>,
    },
    /// Resolve an ObjectRef into a Var
    ResolveObject {
        player: Obj,
        obj: ObjectRef,
        reply: oneshot::Sender<Result<Var, SchedulerError>>,
    },
    /// Submit a request to checkpoint the database.
    Checkpoint(oneshot::Sender<Result<(), SchedulerError>>),
    /// Submit a (non-task specific) request to shutdown the scheduler
    Shutdown(String, oneshot::Sender<Result<(), SchedulerError>>),
}
