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

//! A task is a concurrent, transactionally isolated, thread of execution. It starts with the
//! execution of a 'verb' (or 'command verb' or 'eval' etc) and runs through to completion or
//! suspension or abort.
//! Within the task many verbs may be executed as subroutine calls from the root verb/command
//! Each task has its own VM host which is responsible for executing the program.
//! Each task has its own isolated transactional world state.
//! Each task is given a semi-isolated "session" object through which I/O is performed.
//! When a task fails, both the world state and I/O should be rolled back.
//! A task is generally tied 1:1 with a player connection, and usually come from one command, but
//! they can also be 'forked' from other tasks.
//!
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use bincode::de::{BorrowDecoder, Decoder};
use bincode::enc::Encoder;
use bincode::error::{DecodeError, EncodeError};
use bincode::{BorrowDecode, Decode, Encode};
use byteview::ByteView;
use crossbeam_channel::Sender;
use lazy_static::lazy_static;
use tracing::{error, trace, warn};

use moor_common::model::{CommitResult, VerbDef, WorldState, WorldStateError};
use moor_common::tasks::CommandError;
use moor_common::tasks::CommandError::PermissionDenied;
use moor_common::tasks::TaskId;
use moor_common::util::{PerfTimerGuard, parse_into_words};
use moor_var::{List, v_int, v_str};
use moor_var::{NOTHING, SYSTEM_OBJECT};
use moor_var::{Obj, v_obj};
use moor_var::{Symbol, Variant};

use crate::config::{Config, FeaturesConfig};
use crate::tasks::task_scheduler_client::{TaskControlMsg, TaskSchedulerClient};
use crate::tasks::{ServerOptions, TaskStart, sched_counters};
use crate::vm::builtins::BuiltinRegistry;
use crate::vm::exec_state::VMExecState;
use crate::vm::vm_host::VmHost;
use crate::vm::{VMHostResponse, VerbCall};
use moor_common::matching::{
    CommandParser, DefaultObjectNameMatcher, DefaultParseCommand, ParseCommandError, ParsedCommand,
    WsMatchEnv,
};
use moor_common::tasks::Session;

lazy_static! {
    static ref HUH_SYM: Symbol = Symbol::mk("huh");
}

#[derive(Debug)]
pub struct Task {
    /// My unique task id.
    pub task_id: TaskId,
    /// What I was asked to do.
    pub(crate) task_start: TaskStart,
    /// The player on behalf of whom this task is running. Who owns this task.
    pub(crate) player: Obj,
    /// The permissions of the task -- the object on behalf of which all permissions are evaluated.
    pub(crate) perms: Obj,
    /// The actual VM host which is managing the execution of this task.
    pub(crate) vm_host: VmHost,
    /// True if the task should die.
    pub(crate) kill_switch: Arc<AtomicBool>,
    /// The number of retries this process has undergone.
    pub(crate) retries: u8,
    /// A copy of the VM state at the time the task was created or last committed/suspended.
    /// For restoring on retry.
    pub(crate) retry_state: VMExecState,
}

impl Task {
    // Yes yes I know it's a lot of arguments, but wrapper object here is redundant.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        task_id: TaskId,
        player: Obj,
        task_start: TaskStart,
        perms: Obj,
        server_options: &ServerOptions,
        kill_switch: Arc<AtomicBool>,
    ) -> Self {
        let is_background = task_start.is_background();

        // Find out max ticks, etc. for this task. These are either pulled from server constants in
        // the DB or from default constants.
        let (max_seconds, max_ticks, max_stack_depth) = server_options.max_vm_values(is_background);

        let vm_host = VmHost::new(
            task_id,
            max_stack_depth,
            max_ticks,
            Duration::from_secs(max_seconds),
        );

        let retry_state = vm_host.snapshot_state();
        Task {
            task_id,
            player,
            task_start,
            vm_host,
            perms,
            kill_switch,
            retries: 0,
            retry_state,
        }
    }

    pub fn run_task_loop(
        mut task: Task,
        task_scheduler_client: &TaskSchedulerClient,
        session: Arc<dyn Session>,
        mut world_state: Box<dyn WorldState>,
        builtin_registry: BuiltinRegistry,
        config: Arc<Config>,
    ) {
        while task.vm_host.is_running() {
            // Check kill switch.
            if task.kill_switch.load(std::sync::atomic::Ordering::Relaxed) {
                trace!(task_id = ?task.task_id, "Task killed");
                task_scheduler_client.abort_cancelled();
                break;
            }
            if let Some(continuation_task) = task.vm_dispatch(
                task_scheduler_client,
                session.clone(),
                world_state,
                builtin_registry.clone(),
                config.features_config.clone(),
            ) {
                (task, world_state) = continuation_task;
            } else {
                break;
            }
        }
    }

    /// Call out to the vm_host and ask it to execute the next instructions, and it will return
    /// back telling us next steps.
    /// Results of VM execution are looked at, and if they involve a scheduler action, we will
    /// send a message back to the scheduler to handle it.
    /// If the scheduler action is some kind of suspension, we move ourselves into the message
    /// itself.
    /// If we are to be consumed (because ownership transferred back to the scheduler), we will
    /// return None, otherwise we will return ourselves.
    fn vm_dispatch(
        mut self,
        task_scheduler_client: &TaskSchedulerClient,
        session: Arc<dyn Session>,
        mut world_state: Box<dyn WorldState>,
        builtin_registry: BuiltinRegistry,
        config: FeaturesConfig,
    ) -> Option<(Self, Box<dyn WorldState>)> {
        let perfc = sched_counters();
        let _t = PerfTimerGuard::new(&perfc.vm_dispatch);

        // Call the VM
        let vm_exec_result = self.vm_host.exec_interpreter(
            self.task_id,
            world_state.as_mut(),
            task_scheduler_client.clone(),
            session.clone(),
            builtin_registry,
            config,
        );
        drop(_t);

        let _t = PerfTimerGuard::new(&perfc.post_vm_dispatch);

        // Having done that, what should we now do?
        match vm_exec_result {
            VMHostResponse::DispatchFork(fork_request) => {
                trace!(task_id = self.task_id, ?fork_request, "Task fork");
                // To fork a new task, we need to get the scheduler to do some work for us. So we'll
                // send a message back asking it to fork the task and return the new task id on a
                // reply channel.
                // We will then take the new task id and send it back to the caller.
                let task_id_var = fork_request.task_id;
                let task_id = task_scheduler_client.request_fork(fork_request);
                if let Some(task_id_var) = task_id_var {
                    self.vm_host
                        .set_variable(&task_id_var, v_int(task_id as i64));
                }
                Some((self, world_state))
            }
            VMHostResponse::Suspend(delay) => {
                trace!(task_id = self.task_id, delay = ?delay, "Task suspend");

                // VMHost is now suspended for execution, and we'll be waiting for a Resume
                let commit_result = world_state
                    .commit()
                    .expect("Could not commit world state before suspend");
                if let CommitResult::ConflictRetry = commit_result {
                    warn!("Conflict during commit before suspend");
                    session.rollback().unwrap();
                    task_scheduler_client.conflict_retry(self);
                    return None;
                }

                self.retry_state = self.vm_host.snapshot_state();
                trace!(task_id = self.task_id, "Task suspended");
                self.vm_host.stop();

                // Let the scheduler know about our suspension, which can be of the form:
                //      * Indefinite, wake-able only with Resume
                //      * Scheduled, a duration is given, and we'll wake up after that duration
                // In both cases we'll rely on the scheduler to wake us up in its processing loop
                // rather than sleep here, which would make this thread unresponsive to other
                // messages.
                task_scheduler_client.suspend(delay, self);
                None
            }
            VMHostResponse::SuspendNeedInput => {
                trace!(task_id = self.task_id, "Task suspend need input");

                // VMHost is now suspended for input, and we'll be waiting for a ResumeReceiveInput

                // Attempt commit... See comments/notes on Suspend above.
                let commit_result = world_state
                    .commit()
                    .expect("Could not commit world state before suspend");
                if let CommitResult::ConflictRetry = commit_result {
                    warn!("Conflict during commit before suspend");
                    session.rollback().unwrap();
                    task_scheduler_client.conflict_retry(self);
                    return None;
                }
                self.retry_state = self.vm_host.snapshot_state();
                trace!(task_id = self.task_id, "Task suspended for input");
                self.vm_host.stop();

                // Consume us, passing back to the scheduler that we're waiting for input.
                task_scheduler_client.request_input(self);
                None
            }
            VMHostResponse::ContinueOk => Some((self, world_state)),

            VMHostResponse::CompleteSuccess(result) => {
                trace!(task_id = self.task_id, result = ?result, "Task complete, success");

                // Special case: in case of return from $do_command @ top-level, we need to look at the results:
                //      non-true value? => parse_command and restart (in same transaction)
                //      true value? => commit and return success.
                if let TaskStart::StartDoCommand {
                    handler_object,
                    player,
                    command,
                } = &self.task_start
                {
                    let (player, command) = (player.clone(), command.clone());
                    if !result.is_true() {
                        // Intercept and rewrite us back to StartVerbCommand and do old school parse.
                        self.task_start = TaskStart::StartCommandVerb {
                            handler_object: handler_object.clone(),
                            player: player.clone(),
                            command: command.clone(),
                        };

                        if let Err(e) =
                            self.setup_start_parse_command(&player, &command, world_state.as_mut())
                        {
                            task_scheduler_client.command_error(e);
                        }
                        return Some((self, world_state));
                    }
                }

                let CommitResult::Success = world_state.commit().expect("Could not attempt commit")
                else {
                    warn!("Conflict during commit before complete, asking scheduler to retry task");
                    session.rollback().unwrap();
                    task_scheduler_client.conflict_retry(self);
                    return None;
                };

                self.vm_host.stop();

                task_scheduler_client.success(result);
                None
            }
            VMHostResponse::CompleteAbort => {
                error!(task_id = self.task_id, "Task aborted");

                world_state
                    .rollback()
                    .expect("Could not rollback world state transaction");

                self.vm_host.stop();

                task_scheduler_client.abort_cancelled();
                None
            }
            VMHostResponse::CompleteException(exception) => {
                // Commands that end in exceptions are still expected to be committed, to
                // conform with MOO's expectations.
                // However a conflict-retry here is maybe not the best idea here, I think.
                // So we'll just panic the task (abort) if we can't commit for now.
                // TODO: Should tasks that throw exception always commit?
                //   Right now to preserve MOO semantics, we do.
                //   We may revisit this later and add a user-selectable mode for this, and
                //   evaluate this behaviour generally.

                let CommitResult::Success = world_state.commit().expect("Could not attempt commit")
                else {
                    warn!(
                        "Conflict during commit before complete, asking scheduler to retry task ({})",
                        self.task_id
                    );
                    session.rollback().unwrap();
                    task_scheduler_client.conflict_retry(self);
                    return None;
                };

                warn!(task_id = self.task_id, ?exception, "Task exception");
                self.vm_host.stop();

                task_scheduler_client.exception(exception);
                None
            }
            VMHostResponse::CompleteRollback(commit_session) => {
                // Rollback the transaction
                world_state
                    .rollback()
                    .expect("Could not rollback world state transaction");

                // And then decide if we are going to rollback th session as well.
                if !commit_session {
                    session.rollback().expect("Could not rollback session");
                } else {
                    session.commit().expect("Could not commit session");
                }
                self.vm_host.stop();
                task_scheduler_client.abort_cancelled();
                None
            }

            VMHostResponse::AbortLimit(reason) => {
                warn!(task_id = self.task_id, "Task abort limit reached");

                let this = self.vm_host.this();
                let verb_name = self.vm_host.verb_name();
                let line_number = self.vm_host.line_number();

                self.vm_host.stop();
                world_state
                    .rollback()
                    .expect("Could not rollback world state");
                task_scheduler_client.abort_limits_reached(reason, this, verb_name, line_number);
                None
            }
            VMHostResponse::RollbackRetry => {
                warn!(task_id = self.task_id, "Task rollback requested, retrying");

                self.vm_host.stop();
                world_state
                    .rollback()
                    .expect("Could not rollback world state");
                task_scheduler_client.conflict_retry(self);
                None
            }
        }
    }

    /// Set the task up to start executing, based on the task start configuration.
    pub(crate) fn setup_task_start(
        &mut self,
        control_sender: &Sender<(TaskId, TaskControlMsg)>,
        world_state: &mut dyn WorldState,
    ) -> bool {
        let perfc = sched_counters();
        let _t = PerfTimerGuard::new(&perfc.setup_task);
        match &self.task_start {
            // We've been asked to start a command.
            // We need to set up the VM and then execute it.
            TaskStart::StartCommandVerb {
                handler_object,
                player,
                command,
            } => {
                let (handler_object, player, command) =
                    (handler_object.clone(), player.clone(), command.clone());
                if let Err(e) =
                    self.start_command(&handler_object, &player, command.as_str(), world_state)
                {
                    control_sender
                        .send((self.task_id, TaskControlMsg::TaskCommandError(e)))
                        .expect("Could not send start response");
                };
            }
            TaskStart::StartVerb {
                player,
                vloc,
                verb,
                args,
                argstr,
            } => {
                let verb_call = VerbCall {
                    verb_name: *verb,
                    location: vloc.clone(),
                    this: vloc.clone(),
                    player: player.clone(),
                    args: args.clone(),
                    argstr: argstr.clone(),
                    caller: v_obj(NOTHING),
                };
                // Find the callable verb ...
                // Obj or flyweight?
                let object_location = match &verb_call.this.variant() {
                    Variant::Flyweight(f) => f.delegate().clone(),
                    Variant::Obj(o) => o.clone(),
                    _ => {
                        control_sender
                            .send((
                                self.task_id,
                                TaskControlMsg::TaskVerbNotFound(
                                    verb_call.this,
                                    verb_call.verb_name,
                                ),
                            ))
                            .expect("Could not send start response");
                        return false;
                    }
                };
                match world_state.find_method_verb_on(
                    &self.perms,
                    &object_location,
                    verb_call.verb_name,
                ) {
                    Err(WorldStateError::VerbNotFound(_, _)) => {
                        trace!(task_id = ?self.task_id, this = ?verb_call.this,
                              verb = ?verb_call.verb_name, "Verb not found");
                        control_sender
                            .send((
                                self.task_id,
                                TaskControlMsg::TaskVerbNotFound(
                                    verb_call.this,
                                    verb_call.verb_name,
                                ),
                            ))
                            .expect("Could not send start response");
                        return false;
                    }
                    Err(e) => {
                        error!(task_id = ?self.task_id, this = ?verb_call.this,
                               verb = ?verb_call.verb_name,
                               "World state error while resolving verb: {:?}", e);
                        panic!("Could not resolve verb: {:?}", e);
                    }
                    Ok(verb_info) => {
                        self.vm_host.start_call_method_verb(
                            self.task_id,
                            &self.perms,
                            verb_info,
                            verb_call,
                        );
                    }
                }
            }
            TaskStart::StartFork {
                fork_request,
                suspended,
            } => {
                trace!(task_id = ?self.task_id, suspended, "Setting up fork");
                self.vm_host
                    .start_fork(self.task_id, fork_request, *suspended);
            }
            TaskStart::StartEval { player, program } => {
                self.vm_host
                    .start_eval(self.task_id, player, program.clone(), world_state);
            }
            TaskStart::StartDoCommand { .. } => {
                panic!("StartDoCommand invocation should not happen on initial setup_task_start");
            }
        };
        true
    }

    fn start_command(
        &mut self,
        handler_object: &Obj,
        player: &Obj,
        command: &str,
        world_state: &mut dyn WorldState,
    ) -> Result<(), CommandError> {
        let perfc = sched_counters();
        let _t = PerfTimerGuard::new(&perfc.start_command);

        // Command execution is a multi-phase process:
        //   1. Lookup $do_command. If we have the verb, execute it.
        //   2. If it returns a boolean `true`, we're done, let scheduler know, otherwise:
        //   3. Call parse_command, looking for a verb to execute in the environment.
        //     a. If something, call that verb.
        //     b. If nothing, look for :huh. If we have it, execute it.
        //   4. On completion, let the scheduler know.

        // All of this should occur in the same task id, and in the same transaction, and
        //  forms a multi-part process with continuation back from the VM along the whole
        //  chain, which complicates things significantly.

        // First check to see if we have a $do_command at all, if yes, we're actually starting
        // that verb with the command as an argument. If that then fails (non-true return code)
        // we'll end up in the start_parse_command phase.
        let do_command =
            world_state.find_method_verb_on(&self.perms, &SYSTEM_OBJECT, Symbol::mk("do_command"));

        match do_command {
            Err(WorldStateError::VerbNotFound(_, _)) => {
                self.setup_start_parse_command(player, command, world_state)?;
            }
            Ok(verb_info) => {
                let arguments = parse_into_words(command);
                let args = List::from_iter(arguments.iter().map(|s| v_str(s)));
                let verb_call = VerbCall {
                    verb_name: Symbol::mk("do_command"),
                    location: v_obj(handler_object.clone()),
                    this: v_obj(handler_object.clone()),
                    player: player.clone(),
                    args,
                    argstr: command.to_string(),
                    caller: v_obj(handler_object.clone()),
                };
                self.vm_host.start_call_method_verb(
                    self.task_id,
                    &self.perms,
                    verb_info,
                    verb_call,
                );
                self.task_start = TaskStart::StartDoCommand {
                    handler_object: handler_object.clone(),
                    player: player.clone(),
                    command: command.to_string(),
                };
            }
            Err(e) => {
                panic!("Unable to start task due to error: {e:?}");
            }
        }
        Ok(())
    }

    fn setup_start_parse_command(
        &mut self,
        player: &Obj,
        command: &str,
        world_state: &mut dyn WorldState,
    ) -> Result<(), CommandError> {
        let (player_location, parsed_command) = {
            let perfc = sched_counters();
            let _t = PerfTimerGuard::new(&perfc.parse_command);

            // We need the player's location, and we'll just die if we can't get it.
            let player_location = match world_state.location_of(player, player) {
                Ok(loc) => loc,
                Err(WorldStateError::VerbPermissionDenied)
                | Err(WorldStateError::ObjectPermissionDenied)
                | Err(WorldStateError::PropertyPermissionDenied) => {
                    return Err(PermissionDenied);
                }
                Err(wse) => {
                    return Err(CommandError::DatabaseError(wse));
                }
            };

            // Parse the command in the current environment.
            let me = WsMatchEnv::new(world_state, player.clone());
            let matcher = DefaultObjectNameMatcher {
                env: me,
                player: player.clone(),
            };
            let command_parser = DefaultParseCommand::new();
            let parsed_command = match command_parser.parse_command(command, &matcher) {
                Ok(pc) => pc,
                Err(ParseCommandError::PermissionDenied) => {
                    return Err(PermissionDenied);
                }
                Err(_) => {
                    return Err(CommandError::CouldNotParseCommand);
                }
            };

            (player_location, parsed_command)
        };

        // Look for the verb...
        let parse_results =
            find_verb_for_command(player, &player_location, &parsed_command, world_state)?;
        let (verb_info, target) = match parse_results {
            // If we have a successful match, that's what we'll call into
            Some((verb_info, target)) => {
                trace!(
                    ?parsed_command,
                    ?player,
                    ?target,
                    ?verb_info,
                    "Starting command"
                );
                (verb_info, target)
            }
            // Otherwise, we want to try to call :huh, if it exists.
            None => {
                if player_location == NOTHING {
                    return Err(CommandError::NoCommandMatch);
                }
                // Try to find :huh. If it exists, we'll dispatch to that, instead.
                // If we don't find it, that's the end of the line.
                let Ok(verb_info) =
                    world_state.find_method_verb_on(&self.perms, &player_location, *HUH_SYM)
                else {
                    return Err(CommandError::NoCommandMatch);
                };
                let words = parse_into_words(command);
                trace!(?verb_info, ?player, ?player_location, args = ?words,
                            "Dispatching to :huh");

                (verb_info, player_location)
            }
        };
        let verb_call = VerbCall {
            verb_name: Symbol::mk_case_insensitive(parsed_command.verb.as_str()),
            location: v_obj(target.clone()),
            this: v_obj(target),
            player: player.clone(),
            args: List::mk_list(&parsed_command.args),
            argstr: parsed_command.argstr.clone(),
            caller: v_obj(player.clone()),
        };
        self.vm_host.start_call_command_verb(
            self.task_id,
            verb_info,
            verb_call,
            parsed_command,
            &self.perms,
        );
        Ok(())
    }
}

#[allow(clippy::type_complexity)]
fn find_verb_for_command(
    player: &Obj,
    player_location: &Obj,
    pc: &ParsedCommand,
    ws: &mut dyn WorldState,
) -> Result<Option<((ByteView, VerbDef), Obj)>, CommandError> {
    let perfc = sched_counters();
    let _t = PerfTimerGuard::new(&perfc.find_verb_for_command);
    let targets_to_search = vec![
        player.clone(),
        player_location.clone(),
        pc.dobj.clone().unwrap_or(NOTHING),
        pc.iobj.clone().unwrap_or(NOTHING),
    ];
    for target in targets_to_search {
        let match_result = ws.find_command_verb_on(
            player,
            &target,
            Symbol::mk_case_insensitive(pc.verb.as_str()),
            &pc.dobj.clone().unwrap_or(NOTHING),
            pc.prep,
            &pc.iobj.clone().unwrap_or(NOTHING),
        );
        let match_result = match match_result {
            Ok(m) => m,
            Err(WorldStateError::VerbPermissionDenied) => return Err(PermissionDenied),
            Err(WorldStateError::ObjectPermissionDenied) => {
                return Err(PermissionDenied);
            }
            Err(WorldStateError::PropertyPermissionDenied) => {
                return Err(PermissionDenied);
            }
            Err(wse) => return Err(CommandError::DatabaseError(wse)),
        };
        if let Some(vi) = match_result {
            return Ok(Some((vi, target)));
        }
    }
    Ok(None)
}

impl Encode for Task {
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        // We encode everything but the kill switch, which is transient and always decoded to 'true'
        self.task_id.encode(encoder)?;
        self.player.encode(encoder)?;
        self.task_start.encode(encoder)?;
        self.vm_host.encode(encoder)?;
        self.perms.encode(encoder)?;
        self.retries.encode(encoder)?;
        self.retry_state.encode(encoder)
    }
}

impl<C> Decode<C> for Task {
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let task_id = TaskId::decode(decoder)?;
        let player = Obj::decode(decoder)?;
        let task_start = TaskStart::decode(decoder)?;
        let vm_host = VmHost::decode(decoder)?;
        let perms = Obj::decode(decoder)?;
        let retries = u8::decode(decoder)?;
        let retry_state = VMExecState::decode(decoder)?;
        let kill_switch = Arc::new(AtomicBool::new(false));
        Ok(Task {
            task_id,
            player,
            task_start,
            vm_host,
            perms,
            kill_switch,
            retries,
            retry_state,
        })
    }
}

impl<'de, C> BorrowDecode<'de, C> for Task {
    fn borrow_decode<D: BorrowDecoder<'de>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let task_id = TaskId::borrow_decode(decoder)?;
        let player = Obj::borrow_decode(decoder)?;
        let task_start = TaskStart::decode(decoder)?;
        let vm_host = VmHost::borrow_decode(decoder)?;
        let perms = Obj::borrow_decode(decoder)?;
        let retries = u8::decode(decoder)?;
        let retry_state = VMExecState::decode(decoder)?;
        let kill_switch = Arc::new(AtomicBool::new(false));

        Ok(Task {
            task_id,
            player,
            task_start,
            vm_host,
            perms,
            kill_switch,
            retries,
            retry_state,
        })
    }
}

// TODO: a battery of unit tests here. Which will likely involve setting up a standalone VM running
//   a simple program.
#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    use crossbeam_channel::{Receiver, unbounded};

    use moor_common::model::{
        ArgSpec, BinaryType, PrepSpec, VerbArgsSpec, VerbFlag, WorldState, WorldStateSource,
    };
    use moor_common::tasks::{CommandError, Event, TaskId};
    use moor_common::util::BitEnum;
    use moor_compiler::{CompileOptions, Program, compile};
    use moor_db::{DatabaseConfig, TxDB};
    use moor_var::E_DIV;
    use moor_var::{AsByteBuffer, NOTHING, SYSTEM_OBJECT};
    use moor_var::{Symbol, v_obj};
    use moor_var::{v_int, v_str};

    use crate::config::Config;
    use crate::tasks::task::Task;
    use crate::tasks::task_scheduler_client::{TaskControlMsg, TaskSchedulerClient};
    use crate::tasks::{ServerOptions, TaskStart};
    use crate::vm::activation::Frame;
    use crate::vm::builtins::BuiltinRegistry;
    use moor_common::tasks::NoopClientSession;

    struct TestVerb {
        name: Symbol,
        program: Box<Program>,
        argspec: VerbArgsSpec,
    }

    #[allow(clippy::type_complexity)]
    fn setup_test_env(
        task_start: TaskStart,
        programs: &[TestVerb],
    ) -> (
        Arc<AtomicBool>,
        Task,
        TxDB,
        Box<dyn WorldState>,
        TaskSchedulerClient,
        Receiver<(TaskId, TaskControlMsg)>,
    ) {
        let (control_sender, control_receiver) = unbounded();
        let kill_switch = Arc::new(AtomicBool::new(false));
        let server_options = ServerOptions {
            bg_seconds: 5,
            bg_ticks: 50000,
            fg_seconds: 5,
            fg_ticks: 50000,
            max_stack_depth: 5,
        };
        let task_scheduler_client = TaskSchedulerClient::new(1, control_sender.clone());
        let mut task = Task::new(
            1,
            SYSTEM_OBJECT,
            task_start.clone(),
            SYSTEM_OBJECT,
            &server_options,
            kill_switch.clone(),
        );
        let (db, _) = TxDB::open(None, DatabaseConfig::default());
        let mut tx = db.new_world_state().unwrap();

        let sysobj = tx
            .create_object(&SYSTEM_OBJECT, &NOTHING, &SYSTEM_OBJECT, BitEnum::all())
            .unwrap();
        tx.update_property(
            &SYSTEM_OBJECT,
            &sysobj,
            Symbol::mk("name"),
            &v_str("system"),
        )
        .unwrap();
        tx.update_property(&SYSTEM_OBJECT, &sysobj, Symbol::mk("programmer"), &v_int(1))
            .unwrap();
        tx.update_property(&SYSTEM_OBJECT, &sysobj, Symbol::mk("wizard"), &v_int(1))
            .unwrap();

        for TestVerb {
            name,
            program,
            argspec,
        } in programs
        {
            let binary = program.make_copy_as_vec().unwrap();
            tx.add_verb(
                &SYSTEM_OBJECT,
                &SYSTEM_OBJECT,
                vec![*name],
                &SYSTEM_OBJECT,
                BitEnum::new_with(VerbFlag::Exec),
                *argspec,
                binary,
                BinaryType::LambdaMoo18X,
            )
            .unwrap();
        }
        task.setup_task_start(&control_sender, tx.as_mut());

        (
            kill_switch,
            task,
            db,
            tx,
            task_scheduler_client,
            control_receiver,
        )
    }

    /// Build a simple test environment with an Eval task (since that is simplest to setup)
    #[allow(clippy::type_complexity)]
    fn setup_test_env_eval(
        program: &str,
    ) -> (
        Arc<AtomicBool>,
        Task,
        TxDB,
        Box<dyn WorldState>,
        TaskSchedulerClient,
        Receiver<(TaskId, TaskControlMsg)>,
    ) {
        let program = compile(program, CompileOptions::default()).unwrap();
        let task_start = TaskStart::StartEval {
            player: SYSTEM_OBJECT,
            program,
        };
        setup_test_env(task_start, &[])
    }

    #[allow(clippy::type_complexity)]
    fn setup_test_env_command(
        command: &str,
        verbs: &[TestVerb],
    ) -> (
        Arc<AtomicBool>,
        Task,
        TxDB,
        Box<dyn WorldState>,
        TaskSchedulerClient,
        Receiver<(TaskId, TaskControlMsg)>,
    ) {
        let task_start = TaskStart::StartCommandVerb {
            handler_object: SYSTEM_OBJECT,
            player: SYSTEM_OBJECT,
            command: command.to_string(),
        };
        setup_test_env(task_start, verbs)
    }

    /// Test that we can start a task and run it to completion and it sends the right message with
    /// the result back to the scheduler.
    #[test]
    fn test_simple_run_return() {
        let (_kill_switch, task, _db, tx, task_scheduler_client, control_receiver) =
            setup_test_env_eval("return 1 + 1;");

        let session = Arc::new(NoopClientSession::new());
        Task::run_task_loop(
            task,
            &task_scheduler_client,
            session,
            tx,
            BuiltinRegistry::new(),
            Arc::new(Config::default()),
        );

        // Scheduler should have received a TaskSuccess message.
        let (task_id, msg) = control_receiver.recv().unwrap();
        assert_eq!(task_id, 1);
        let TaskControlMsg::TaskSuccess(result) = msg else {
            panic!("Expected TaskSuccess, got {:?}", msg);
        };
        assert_eq!(result, v_int(2));
    }

    /// Trigger a MOO VM exception, and verify it gets sent to scheduler
    #[test]
    fn test_simple_run_exception() {
        let (_kill_switch, task, _db, tx, task_scheduler_client, control_receiver) =
            setup_test_env_eval("return 1 / 0;");

        let session = Arc::new(NoopClientSession::new());
        Task::run_task_loop(
            task,
            &task_scheduler_client,
            session,
            tx,
            BuiltinRegistry::new(),
            Arc::new(Config::default()),
        );

        // Scheduler should have received a TaskException message.
        let (task_id, msg) = control_receiver.recv().unwrap();
        assert_eq!(task_id, 1);
        let TaskControlMsg::TaskException(exception) = msg else {
            panic!("Expected TaskException, got {:?}", msg);
        };
        assert_eq!(exception.error.err_type, E_DIV);
    }

    // notify() will dispatch to the scheduler
    #[test]
    fn test_notify_invocation() {
        let (_kill_switch, task, _db, tx, task_scheduler_client, control_receiver) =
            setup_test_env_eval(r#"notify(#0, "12345"); return 123;"#);

        let session = Arc::new(NoopClientSession::new());
        Task::run_task_loop(
            task,
            &task_scheduler_client,
            session,
            tx,
            BuiltinRegistry::new(),
            Arc::new(Config::default()),
        );

        // Scheduler should have received a TaskException message.
        let (task_id, msg) = control_receiver.recv().unwrap();
        assert_eq!(task_id, 1);
        let TaskControlMsg::Notify { player, event } = msg else {
            panic!("Expected Notify, got {:?}", msg);
        };
        assert_eq!(player, SYSTEM_OBJECT);
        assert_eq!(event.author(), &v_obj(SYSTEM_OBJECT));
        assert_eq!(event.event, Event::Notify(v_str("12345"), None));

        // Also scheduler should have received a TaskSuccess message.
        let (task_id, msg) = control_receiver.recv().unwrap();
        assert_eq!(task_id, 1);
        let TaskControlMsg::TaskSuccess(result) = msg else {
            panic!("Expected TaskSuccess, got {:?}", msg);
        };
        assert_eq!(result, v_int(123));
    }

    /// Trigger a task-suspend-resume
    #[test]
    fn test_simple_run_suspend() {
        let (_kill_switch, task, db, tx, task_scheduler_client, control_receiver) =
            setup_test_env_eval("suspend(1); return 123;");

        let session = Arc::new(NoopClientSession::new());
        Task::run_task_loop(
            task,
            &task_scheduler_client,
            session.clone(),
            tx,
            BuiltinRegistry::new(),
            Arc::new(Config::default()),
        );

        // Scheduler should have received a TaskSuspend message.
        let (task_id, msg) = control_receiver.recv().unwrap();
        assert_eq!(task_id, 1);
        let TaskControlMsg::TaskSuspend(_, mut resume_task) = msg else {
            panic!("Expected TaskSuspend, got {:?}", msg);
        };
        assert_eq!(resume_task.task_id, 1);

        // Now we can simulate resumption...
        resume_task.vm_host.resume_execution(v_int(0));

        let tx = db.new_world_state().unwrap();
        Task::run_task_loop(
            resume_task,
            &task_scheduler_client,
            session,
            tx,
            BuiltinRegistry::new(),
            Arc::new(Config::default()),
        );
        let (task_id, msg) = control_receiver.recv().unwrap();
        assert_eq!(task_id, 1);
        let TaskControlMsg::TaskSuccess(result) = msg else {
            panic!("Expected TaskSuccess, got {:?}", msg);
        };
        assert_eq!(result, v_int(123));
    }

    /// Trigger a simulated read()
    #[test]
    fn test_simple_run_read() {
        let (_kill_switch, task, db, tx, task_scheduler_client, control_receiver) =
            setup_test_env_eval("return read();");

        let session = Arc::new(NoopClientSession::new());
        Task::run_task_loop(
            task,
            &task_scheduler_client,
            session.clone(),
            tx,
            BuiltinRegistry::new(),
            Arc::new(Config::default()),
        );

        // Scheduler should have received a TaskRequestInput message, and it should contain the task.
        let (task_id, msg) = control_receiver.recv().unwrap();
        assert_eq!(task_id, 1);
        let TaskControlMsg::TaskRequestInput(mut resume_task) = msg else {
            panic!("Expected TaskRequestInput, got {:?}", msg);
        };
        assert_eq!(resume_task.task_id, 1);

        // Now we can simulate resumption...
        resume_task.vm_host.resume_execution(v_str("hello, world!"));

        // And run its task loop again, with a new transaction.
        let tx = db.new_world_state().unwrap();
        Task::run_task_loop(
            resume_task,
            &task_scheduler_client,
            session,
            tx,
            BuiltinRegistry::new(),
            Arc::new(Config::default()),
        );

        // Scheduler should have received a TaskSuccess message.
        let (task_id, msg) = control_receiver.recv().unwrap();
        assert_eq!(task_id, 1);
        let TaskControlMsg::TaskSuccess(result) = msg else {
            panic!("Expected TaskSuccess, got {:?}", msg);
        };
        assert_eq!(result, v_str("hello, world!"));
    }

    /// Trigger a task-fork
    #[test]
    fn test_simple_run_fork() {
        let (_kill_switch, task, db, tx, task_scheduler_client, control_receiver) =
            setup_test_env_eval("fork (1) return 1 + 1; endfork return 123;");
        tx.commit().unwrap();

        // Pull a copy of the program out for comparison later.
        let task_start = task.task_start.clone();
        let TaskStart::StartEval { program, .. } = &task_start else {
            panic!("Expected StartEval, got {:?}", task.task_start);
        };

        // This one needs to run in a thread because it's going to block waiting on a reply from
        // our fake scheduler.
        let jh = std::thread::spawn(move || {
            let tx = db.new_world_state().unwrap();
            let session = Arc::new(NoopClientSession::new());
            Task::run_task_loop(
                task,
                &task_scheduler_client,
                session,
                tx,
                BuiltinRegistry::new(),
                Arc::new(Config::default()),
            );
        });

        // Scheduler should have received a TaskRequestFork message.
        let (task_id, msg) = control_receiver.recv().unwrap();
        assert_eq!(task_id, 1);
        let TaskControlMsg::TaskRequestFork(fork_request, reply_channel) = msg else {
            panic!("Expected TaskRequestFork, got {:?}", msg);
        };
        assert_eq!(fork_request.task_id, None);
        assert_eq!(fork_request.parent_task_id, 1);

        let Frame::Moo(moo_frame) = &fork_request.activation.frame else {
            panic!(
                "Expected Moo frame, got {:?}",
                fork_request.activation.frame
            );
        };
        assert_eq!(moo_frame.program, *program);

        // Reply back with the new task id.
        reply_channel.send(2).unwrap();

        // Wait for the task to finish.
        jh.join().unwrap();

        // Scheduler should have received a TaskSuccess message.
        let (task_id, msg) = control_receiver.recv().unwrap();
        assert_eq!(task_id, 1);
        let TaskControlMsg::TaskSuccess(result) = msg else {
            panic!("Expected TaskSuccess, got {:?}", msg);
        };
        assert_eq!(result, v_int(123));
    }

    /// Verifies path through the command parser, and no match on verb
    #[test]
    fn test_command_no_match() {
        let (_kill_switch, task, _db, tx, task_scheduler_client, control_receiver) =
            setup_test_env_command("look here", &[]);

        let session = Arc::new(NoopClientSession::new());
        Task::run_task_loop(
            task,
            &task_scheduler_client,
            session,
            tx,
            BuiltinRegistry::new(),
            Arc::new(Config::default()),
        );

        // Scheduler should have received a NoCommandMatch
        let (task_id, msg) = control_receiver.recv().unwrap();
        assert_eq!(task_id, 1);
        let TaskControlMsg::TaskCommandError(CommandError::NoCommandMatch) = msg else {
            panic!("Expected NoCommandMatch, got {:?}", msg);
        };
    }

    /// Install a simple verb that will match and execute, without $do_command.
    #[test]
    fn test_command_match() {
        let look_this = TestVerb {
            name: Symbol::mk("look"),
            program: compile("return 1;", CompileOptions::default()).unwrap(),
            argspec: VerbArgsSpec {
                dobj: ArgSpec::This,
                prep: PrepSpec::None,
                iobj: ArgSpec::None,
            },
        };
        let (_kill_switch, task, _db, tx, task_scheduler_client, control_receiver) =
            setup_test_env_command("look #0", &[look_this]);

        let session = Arc::new(NoopClientSession::new());
        Task::run_task_loop(
            task,
            &task_scheduler_client,
            session,
            tx,
            BuiltinRegistry::new(),
            Arc::new(Config::default()),
        );

        // This should be a success, it got handled
        let (task_id, msg) = control_receiver.recv().unwrap();
        assert_eq!(task_id, 1);
        let TaskControlMsg::TaskSuccess(result) = msg else {
            panic!("Expected TaskSuccess, got {:?}", msg);
        };
        assert_eq!(result, v_int(1));
    }

    /// Install "do_command" that returns true, meaning the command was handled, and that's success.
    #[test]
    fn test_command_do_command() {
        let do_command_verb = TestVerb {
            name: Symbol::mk("do_command"),
            program: compile("return 1;", CompileOptions::default()).unwrap(),
            argspec: VerbArgsSpec::this_none_this(),
        };

        let (_kill_switch, task, _db, tx, task_scheduler_client, control_receiver) =
            setup_test_env_command("look here", &[do_command_verb]);

        let session = Arc::new(NoopClientSession::new());
        Task::run_task_loop(
            task,
            &task_scheduler_client,
            session,
            tx,
            BuiltinRegistry::new(),
            Arc::new(Config::default()),
        );

        // This should be a success, it got handled
        let (task_id, msg) = control_receiver.recv().unwrap();
        assert_eq!(task_id, 1);
        let TaskControlMsg::TaskSuccess(result) = msg else {
            panic!("Expected TaskSuccess, got {:?}", msg);
        };
        assert_eq!(result, v_int(1));
    }

    /// Install "do_command" that returns false, meaning the command needs to go to parsing and
    /// old school dispatch. But there will be nothing there to match, so we'll fail out.
    #[test]
    fn test_command_do_command_false_no_match() {
        let do_command_verb = TestVerb {
            name: Symbol::mk("do_command"),
            program: compile("return 0;", CompileOptions::default()).unwrap(),
            argspec: VerbArgsSpec::this_none_this(),
        };

        let (_kill_switch, task, _db, tx, task_scheduler_client, control_receiver) =
            setup_test_env_command("look here", &[do_command_verb]);

        let session = Arc::new(NoopClientSession::new());
        Task::run_task_loop(
            task,
            &task_scheduler_client,
            session,
            tx,
            BuiltinRegistry::new(),
            Arc::new(Config::default()),
        );

        // This should be a success, it got handled
        let (task_id, msg) = control_receiver.recv().unwrap();
        assert_eq!(task_id, 1);
        let TaskControlMsg::TaskCommandError(CommandError::NoCommandMatch) = msg else {
            panic!("Expected NoCommandMatch, got {:?}", msg);
        };
    }

    /// Install "do_command" that returns false, meaning the command needs to go to parsing and
    /// old school dispatch, and we will actually match on something.
    #[test]
    fn test_command_do_command_false_match() {
        let do_command_verb = TestVerb {
            name: Symbol::mk("do_command"),
            program: compile("return 0;", CompileOptions::default()).unwrap(),
            argspec: VerbArgsSpec::this_none_this(),
        };

        let look_this = TestVerb {
            name: Symbol::mk("look"),
            program: compile("return 1;", CompileOptions::default()).unwrap(),
            argspec: VerbArgsSpec {
                dobj: ArgSpec::This,
                prep: PrepSpec::None,
                iobj: ArgSpec::None,
            },
        };
        let (_kill_switch, task, _db, tx, task_scheduler_client, control_receiver) =
            setup_test_env_command("look #0", &[do_command_verb, look_this]);

        let session = Arc::new(NoopClientSession::new());
        Task::run_task_loop(
            task,
            &task_scheduler_client,
            session,
            tx,
            BuiltinRegistry::new(),
            Arc::new(Config::default()),
        );

        // This should be a success, it got handled
        let (task_id, msg) = control_receiver.recv().unwrap();
        assert_eq!(task_id, 1);
        let TaskControlMsg::TaskSuccess(result) = msg else {
            panic!("Expected TaskSuccess, got {:?}", msg);
        };
        assert_eq!(result, v_int(1));
    }
}
