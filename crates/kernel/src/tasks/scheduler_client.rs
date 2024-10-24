// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
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

use moor_compiler::{compile, Program};
use moor_values::model::{ObjectRef, PropDef, PropPerms, VerbDef, VerbDefs};
use moor_values::{Objid, Symbol, Var};

use crate::config::Config;
use crate::tasks::sessions::Session;
use crate::tasks::TaskHandle;
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
        player: Objid,
        command: &str,
        session: Arc<dyn Session>,
    ) -> Result<TaskHandle, SchedulerError> {
        trace!(?player, ?command, "Command submitting");
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::SubmitCommandTask {
                player,
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
        player: Objid,
        vloc: ObjectRef,
        verb: Symbol,
        args: Vec<Var>,
        argstr: String,
        perms: Objid,
        session: Arc<dyn Session>,
    ) -> Result<TaskHandle, SchedulerError> {
        trace!(?player, ?verb, ?args, "Verb submitting");
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::SubmitVerbTask {
                player,
                vloc,
                verb: Symbol::mk_case_insensitive(verb.as_str()),
                args,
                argstr,
                perms,
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
        player: Objid,
        input_request_id: Uuid,
        input: String,
    ) -> Result<(), SchedulerError> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::SubmitTaskInput {
                player,
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
        player: Objid,
        command: Vec<String>,
        argstr: String,
        session: Arc<dyn Session>,
    ) -> Result<TaskHandle, SchedulerError> {
        trace!(?player, ?command, "Out-of-band task submitting");
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::SubmitOobTask {
                player,
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
        player: Objid,
        perms: Objid,
        code: String,
        sessions: Arc<dyn Session>,
        config: Arc<Config>,
    ) -> Result<TaskHandle, SchedulerError> {
        // Compile the text into a verb.
        let program = match compile(code.as_str(), config.compile_options()) {
            Ok(b) => b,
            Err(e) => return Err(CompilationError(e)),
        };

        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::SubmitEvalTask {
                player,
                perms,
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
        player: Objid,
        perms: Objid,
        obj: ObjectRef,
        verb_name: Symbol,
        code: Vec<String>,
    ) -> Result<(Objid, Symbol), SchedulerError> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::SubmitProgramVerb {
                player,
                perms,
                obj,
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
        player: Objid,
        obj: ObjectRef,
        property: Symbol,
    ) -> Result<Var, SchedulerError> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::RequestSystemProperty {
                player,
                obj,
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
        player: Objid,
        perms: Objid,
        obj: ObjectRef,
    ) -> Result<VerbDefs, SchedulerError> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::RequestVerbs {
                player,
                perms,
                obj,
                reply,
            })
            .map_err(|_| SchedulerError::SchedulerNotResponding)?;

        receive
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| SchedulerError::SchedulerNotResponding)?
    }

    pub fn request_verb(
        &self,
        player: Objid,
        perms: Objid,
        obj: ObjectRef,
        verb: Symbol,
    ) -> Result<(VerbDef, Vec<String>), SchedulerError> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::RequestVerbCode {
                player,
                perms,
                obj,
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
        player: Objid,
        perms: Objid,
        obj: ObjectRef,
    ) -> Result<Vec<(PropDef, PropPerms)>, SchedulerError> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::RequestProperties {
                player,
                perms,
                obj,
                reply,
            })
            .map_err(|_| SchedulerError::SchedulerNotResponding)?;

        receive
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| SchedulerError::SchedulerNotResponding)?
    }

    pub fn request_property(
        &self,
        player: Objid,
        perms: Objid,
        obj: ObjectRef,
        property: Symbol,
    ) -> Result<(PropDef, PropPerms, Var), SchedulerError> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send(SchedulerClientMsg::RequestProperty {
                player,
                perms,
                obj,
                property,
                reply,
            })
            .map_err(|_| SchedulerError::SchedulerNotResponding)?;

        receive
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| SchedulerError::SchedulerNotResponding)?
    }

    pub fn resolve_object(&self, player: Objid, obj: ObjectRef) -> Result<Var, SchedulerError> {
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
        player: Objid,
        command: String,
        session: Arc<dyn Session>,
        reply: oneshot::Sender<Result<TaskHandle, SchedulerError>>,
    },
    /// Submit a top-level verb (method) invocation to be executed on behalf of the player.
    SubmitVerbTask {
        player: Objid,
        vloc: ObjectRef,
        verb: Symbol,
        args: Vec<Var>,
        argstr: String,
        perms: Objid,
        session: Arc<dyn Session>,
        reply: oneshot::Sender<Result<TaskHandle, SchedulerError>>,
    },
    /// Submit input to a task that is waiting for it.
    SubmitTaskInput {
        player: Objid,
        input_request_id: Uuid,
        input: String,
        reply: oneshot::Sender<Result<(), SchedulerError>>,
    },
    /// Submit an out-of-band task to be executed
    SubmitOobTask {
        player: Objid,
        command: Vec<String>,
        argstr: String,
        session: Arc<dyn Session>,
        reply: oneshot::Sender<Result<TaskHandle, SchedulerError>>,
    },
    /// Submit an eval task
    SubmitEvalTask {
        player: Objid,
        perms: Objid,
        program: Program,
        sessions: Arc<dyn Session>,
        reply: oneshot::Sender<Result<TaskHandle, SchedulerError>>,
    },
    /// Submit a request to program a verb
    SubmitProgramVerb {
        player: Objid,
        perms: Objid,
        obj: ObjectRef,
        verb_name: Symbol,
        code: Vec<String>,
        reply: oneshot::Sender<Result<(Objid, Symbol), SchedulerError>>,
    },
    /// Request the value of a $property.
    /// (Used by the login process, unauthenticated)
    RequestSystemProperty {
        player: Objid,
        obj: ObjectRef,
        property: Symbol,
        reply: oneshot::Sender<Result<Var, SchedulerError>>,
    },
    /// Request the list of visible verbs on an object.
    RequestVerbs {
        player: Objid,
        perms: Objid,
        obj: ObjectRef,
        reply: oneshot::Sender<Result<VerbDefs, SchedulerError>>,
    },
    /// Request the decompiled code of a verb along with its definition.
    RequestVerbCode {
        player: Objid,
        perms: Objid,
        obj: ObjectRef,
        verb: Symbol,
        reply: oneshot::Sender<Result<(VerbDef, Vec<String>), SchedulerError>>,
    },
    /// Request the list of visible properties on an object.
    RequestProperties {
        player: Objid,
        perms: Objid,
        obj: ObjectRef,
        reply: oneshot::Sender<Result<Vec<(PropDef, PropPerms)>, SchedulerError>>,
    },
    /// Request the description and contents of a property.
    RequestProperty {
        player: Objid,
        perms: Objid,
        obj: ObjectRef,
        property: Symbol,
        reply: oneshot::Sender<Result<(PropDef, PropPerms, Var), SchedulerError>>,
    },
    /// Resolve an ObjectRef into a Var
    ResolveObject {
        player: Objid,
        obj: ObjectRef,
        reply: oneshot::Sender<Result<Var, SchedulerError>>,
    },
    /// Submit a request to checkpoint the database.
    Checkpoint(oneshot::Sender<Result<(), SchedulerError>>),
    /// Submit a (non-task specific) request to shutdown the scheduler
    Shutdown(String, oneshot::Sender<Result<(), SchedulerError>>),
}
