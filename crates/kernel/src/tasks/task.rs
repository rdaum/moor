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
use crossbeam_channel::Sender;

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use tracing::{debug, error, trace, warn};

use moor_values::model::CommandError::PermissionDenied;
use moor_values::model::VerbInfo;
use moor_values::model::WorldState;
use moor_values::model::{CommandError, CommitResult, WorldStateError};
use moor_values::util::parse_into_words;
use moor_values::var::v_int;
use moor_values::var::{List, Objid};
use moor_values::NOTHING;

use crate::matching::match_env::MatchEnvironmentParseMatcher;
use crate::matching::ws_match_env::WsMatchEnv;
use crate::tasks::command_parse::{parse_command, ParseCommandError, ParsedCommand};

use crate::tasks::sessions::Session;
use crate::tasks::task_messages::{SchedulerControlMsg, TaskStart};
use crate::tasks::vm_host::{VMHostResponse, VmHost};
use crate::tasks::{TaskId, VerbCall};

#[derive(Debug)]
pub struct Task {
    /// My unique task id.
    pub(crate) task_id: TaskId,
    /// What I was asked to do.
    pub(crate) task_start: Arc<TaskStart>,
    /// When this task will begin execution.
    /// For currently execution tasks this is when the task actually began running.
    /// For tasks in suspension, this is when they will wake up.
    /// If the task is in indefinite suspension, this is None.
    pub(crate) scheduled_start_time: Option<SystemTime>,
    /// The player on behalf of whom this task is running. Who owns this task.
    pub(crate) player: Objid,
    /// The permissions of the task -- the object on behalf of which all permissions are evaluated.
    pub(crate) perms: Objid,
    /// The actual VM host which is managing the execution of this task.
    pub(crate) vm_host: VmHost,
    /// True if the task should die.
    pub(crate) kill_switch: Arc<AtomicBool>,
}

// TODO Propagate default ticks, seconds values from global config / args properly.
//   Note these can be overridden in-core as well, server_options, will need caching, etc.
const DEFAULT_FG_TICKS: usize = 60_000;
const DEFAULT_BG_TICKS: usize = 30_000;
const DEFAULT_FG_SECONDS: u64 = 5;
const DEFAULT_BG_SECONDS: u64 = 3;
const DEFAULT_MAX_STACK_DEPTH: usize = 50;

fn max_vm_values(is_background: bool) -> (usize, u64, usize) {
    let (max_ticks, max_seconds, max_stack_depth) = if is_background {
        (
            DEFAULT_BG_TICKS,
            DEFAULT_BG_SECONDS,
            DEFAULT_MAX_STACK_DEPTH,
        )
    } else {
        (
            DEFAULT_FG_TICKS,
            DEFAULT_FG_SECONDS,
            DEFAULT_MAX_STACK_DEPTH,
        )
    };

    //
    // // Look up fg_ticks, fg_seconds, and max_stack_depth on $server_options.
    // // These are optional properties, and if they are not set, we use the defaults.
    // let wizperms = PermissionsContext::root_for(Objid(2), BitEnum::new_with(ObjFlag::Wizard));
    // if let Ok(server_options) = ws
    //     .retrieve_property(wizperms.clone(), Objid(0), "server_options")
    //
    // {
    //     if let Variant::Obj(server_options) = server_options.variant() {
    //         if let Ok(v) = ws
    //             .retrieve_property(wizperms.clone(), *server_options, "fg_ticks")
    //
    //         {
    //             if let Variant::Int(v) = v.variant() {
    //                 max_ticks = *v as usize;
    //             }
    //         }
    //         if let Ok(v) = ws
    //             .retrieve_property(wizperms.clone(), *server_options, "fg_seconds")
    //
    //         {
    //             if let Variant::Int(v) = v.variant() {
    //                 max_seconds = *v as u64;
    //             }
    //         }
    //         if let Ok(v) = ws
    //             .retrieve_property(wizperms, *server_options, "max_stack_depth")
    //
    //         {
    //             if let Variant::Int(v) = v.variant() {
    //                 max_stack_depth = *v as usize;
    //             }
    //         }
    //     }
    // }
    (max_ticks, max_seconds, max_stack_depth)
}

impl Task {
    // Yes yes I know it's a lot of arguments, but wrapper object here is redundant.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        task_id: TaskId,
        player: Objid,
        task_start: Arc<TaskStart>,
        perms: Objid,
        is_background: bool,
        session: Arc<dyn Session>,
        control_sender: &Sender<(TaskId, SchedulerControlMsg)>,
        kill_switch: Arc<AtomicBool>,
    ) -> Self {
        // Find out max ticks, etc. for this task. These are either pulled from server constants in
        // the DB or from default constants.
        let (max_ticks, max_seconds, max_stack_depth) = max_vm_values(is_background);

        let scheduler_control_sender = control_sender.clone();
        let vm_host = VmHost::new(
            task_id,
            max_stack_depth,
            max_ticks,
            Duration::from_secs(max_seconds),
            session.clone(),
            scheduler_control_sender.clone(),
        );

        Task {
            task_id,
            player,
            task_start,
            scheduled_start_time: None,
            vm_host,
            perms,
            kill_switch,
        }
    }

    pub fn run_task_loop(
        mut task: Task,
        control_sender: Sender<(TaskId, SchedulerControlMsg)>,
        mut world_state: Box<dyn WorldState>,
    ) {
        let task_id = task.task_id;
        debug!(task_id, "Task started");
        while task.vm_host.is_running() {
            // Check kill switch.
            if task.kill_switch.load(std::sync::atomic::Ordering::Relaxed) {
                trace!(task_id = ?task.task_id, "Task killed");
                control_sender
                    .send((task.task_id, SchedulerControlMsg::TaskAbortCancelled))
                    .expect("Could not send kill message");
                break;
            }
            if let Some(continuation_task) = task.vm_dispatch(&control_sender, world_state.as_mut())
            {
                task = continuation_task;
            } else {
                break;
            }
        }
        debug!(task_id, "Task finished");
    }

    /// Set the task up to start executing, based on the task start configuration.
    pub(crate) fn setup_task_start(
        &mut self,
        control_sender: &Sender<(TaskId, SchedulerControlMsg)>,
        world_state: &mut dyn WorldState,
    ) -> bool {
        match self.task_start.clone().as_ref() {
            // We've been asked to start a command.
            // We need to set up the VM and then execute it.
            TaskStart::StartCommandVerb { player, command } => {
                if let Some(msg) = self.start_command(*player, command.as_str(), world_state) {
                    control_sender
                        .send((self.task_id, msg))
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
                // We should never be asked to start a command while we're already running one.
                trace!(?verb, ?player, ?vloc, ?args, "Starting verb");

                let verb_call = VerbCall {
                    verb_name: verb.clone(),
                    location: *vloc,
                    this: *vloc,
                    player: *player,
                    args: args.clone(),
                    argstr: argstr.clone(),
                    caller: NOTHING,
                };
                // Find the callable verb ...
                match world_state.find_method_verb_on(
                    self.perms,
                    verb_call.this,
                    verb_call.verb_name.as_str(),
                ) {
                    Err(WorldStateError::VerbNotFound(_, _)) => {
                        debug!(task_id = ?self.task_id, this = ?verb_call.this,
                              verb = verb_call.verb_name, "Verb not found");
                        control_sender
                            .send((
                                self.task_id,
                                SchedulerControlMsg::TaskVerbNotFound(
                                    verb_call.this,
                                    verb_call.verb_name,
                                ),
                            ))
                            .expect("Could not send start response");
                        return false;
                    }
                    Err(e) => {
                        error!(task_id = ?self.task_id, this = ?verb_call.this,
                               verb = verb_call.verb_name,
                               "World state error while resolving verb: {:?}", e);
                        panic!("Could not resolve verb: {:?}", e);
                    }
                    Ok(verb_info) => {
                        self.vm_host.start_call_method_verb(
                            self.task_id,
                            self.perms,
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
                self.scheduled_start_time = None;
                self.vm_host
                    .start_eval(self.task_id, *player, program.clone(), world_state);
            }
        };
        true
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
        control_sender: &Sender<(TaskId, SchedulerControlMsg)>,
        world_state: &mut dyn WorldState,
    ) -> Option<Self> {
        // Call the VM
        let vm_exec_result = self.vm_host.exec_interpreter(self.task_id, world_state);

        // Having done that, what should we now do?
        match vm_exec_result {
            VMHostResponse::DispatchFork(fork_request) => {
                trace!(task_id = self.task_id, ?fork_request, "Task fork");
                // To fork a new task, we need to get the scheduler to do some work for us. So we'll
                // send a message back asking it to fork the task and return the new task id on a
                // reply channel.
                // We will then take the new task id and send it back to the caller.
                let (send, reply) = oneshot::channel();
                let task_id_var = fork_request.task_id;
                control_sender
                    .send((
                        self.task_id,
                        SchedulerControlMsg::TaskRequestFork(fork_request, send),
                    ))
                    .expect("Could not send fork request");
                let task_id = reply.recv().expect("Could not get fork reply");
                if let Some(task_id_var) = task_id_var {
                    self.vm_host
                        .set_variable(&task_id_var, v_int(task_id as i64));
                }
                Some(self)
            }
            VMHostResponse::Suspend(delay) => {
                trace!(task_id = self.task_id, delay = ?delay, "Task suspend");

                // VMHost is now suspended for execution, and we'll be waiting for a Resume
                let commit_result = world_state
                    .commit()
                    .expect("Could not commit world state before suspend");
                if let CommitResult::ConflictRetry = commit_result {
                    warn!("Conflict during commit before suspend");
                    control_sender
                        .send((self.task_id, SchedulerControlMsg::TaskConflictRetry(self)))
                        .expect("Could not send conflict retry");
                    return None;
                }

                trace!(task_id = self.task_id, "Task suspended");
                self.vm_host.stop();

                // Let the scheduler know about our suspension, which can be of the form:
                //      * Indefinite, wake-able only with Resume
                //      * Scheduled, a duration is given, and we'll wake up after that duration
                // In both cases we'll rely on the scheduler to wake us up in its processing loop
                // rather than sleep here, which would make this thread unresponsive to other
                // messages.
                let resume_time = delay.map(|delay| Instant::now() + delay);

                control_sender
                    .send((
                        self.task_id,
                        SchedulerControlMsg::TaskSuspend(resume_time, self),
                    ))
                    .expect("Could not send suspend message");
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
                    control_sender
                        .send((self.task_id, SchedulerControlMsg::TaskConflictRetry(self)))
                        .expect("Could not send conflict retry");
                    return None;
                }

                trace!(task_id = self.task_id, "Task suspended for input");
                self.vm_host.stop();

                // Consume us, passing back to the scheduler that we're waiting for input.
                control_sender
                    .send((self.task_id, SchedulerControlMsg::TaskRequestInput(self)))
                    .expect("Could not send suspend message");
                None
            }
            VMHostResponse::ContinueOk => Some(self),
            VMHostResponse::CompleteSuccess(result) => {
                trace!(task_id = self.task_id, result = ?result, "Task complete, success");

                let CommitResult::Success = world_state.commit().expect("Could not attempt commit")
                else {
                    warn!("Conflict during commit before complete, asking scheduler to retry task");
                    control_sender
                        .send((self.task_id, SchedulerControlMsg::TaskConflictRetry(self)))
                        .expect("Could not send conflict retry");
                    return None;
                };

                self.vm_host.stop();

                control_sender
                    .send((self.task_id, SchedulerControlMsg::TaskSuccess(result)))
                    .expect("Could not send success message");
                Some(self)
            }
            VMHostResponse::CompleteAbort => {
                error!(task_id = self.task_id, "Task aborted");

                world_state
                    .rollback()
                    .expect("Could not rollback world state transaction");

                self.vm_host.stop();

                control_sender
                    .send((self.task_id, SchedulerControlMsg::TaskAbortCancelled))
                    .expect("Could not send abort message");
                Some(self)
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

                warn!(task_id = self.task_id, "Task exception");
                self.vm_host.stop();

                control_sender
                    .send((
                        self.task_id,
                        SchedulerControlMsg::TaskException(exception.clone()),
                    ))
                    .expect("Could not send exception message");
                Some(self)
            }
            VMHostResponse::AbortLimit(reason) => {
                warn!(task_id = self.task_id, "Task abort limit reached");

                self.vm_host.stop();
                world_state
                    .rollback()
                    .expect("Could not rollback world state");
                control_sender
                    .send((
                        self.task_id,
                        SchedulerControlMsg::TaskAbortLimitsReached(reason),
                    ))
                    .expect("Could not send abort limit message");
                Some(self)
            }
            VMHostResponse::RollbackRetry => {
                warn!(task_id = self.task_id, "Task rollback requested, retrying");

                self.vm_host.stop();
                world_state
                    .rollback()
                    .expect("Could not rollback world state");
                control_sender
                    .send((self.task_id, SchedulerControlMsg::TaskConflictRetry(self)))
                    .expect("Could not send rollback retry message");
                None
            }
        }
    }

    fn start_command(
        &mut self,
        player: Objid,
        command: &str,
        world_state: &mut dyn WorldState,
    ) -> Option<SchedulerControlMsg> {
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

        // TODO Move $do_command handling into task/scheduler
        //   Right now this is done in the daemon, but it should be moved into the task/scheduler
        //   First try to match $do_command. And execute that, scheduling a callback into
        //   this stage again, if that fails. For now though, we rely on the daemon having
        //   done this work for us.

        // Next, try parsing the command.

        // We need the player's location, and we'll just die if we can't get it.
        let player_location = match world_state.location_of(player, player) {
            Ok(loc) => loc,
            Err(WorldStateError::VerbPermissionDenied)
            | Err(WorldStateError::ObjectPermissionDenied)
            | Err(WorldStateError::PropertyPermissionDenied) => {
                return Some(SchedulerControlMsg::TaskCommandError(PermissionDenied));
            }
            Err(wse) => {
                return Some(SchedulerControlMsg::TaskCommandError(
                    CommandError::DatabaseError(wse),
                ));
            }
        };

        // Parse the command in the current environment.
        let me = WsMatchEnv {
            ws: world_state,
            perms: player,
        };
        let matcher = MatchEnvironmentParseMatcher { env: me, player };
        let parsed_command = match parse_command(command, matcher) {
            Ok(pc) => pc,
            Err(ParseCommandError::PermissionDenied) => {
                return Some(SchedulerControlMsg::TaskCommandError(PermissionDenied));
            }
            Err(_) => {
                return Some(SchedulerControlMsg::TaskCommandError(
                    CommandError::CouldNotParseCommand,
                ));
            }
        };

        // Look for the verb...
        let parse_results =
            match find_verb_for_command(player, player_location, &parsed_command, world_state) {
                Ok(results) => results,
                Err(e) => {
                    return Some(SchedulerControlMsg::TaskCommandError(e));
                }
            };
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
                    return Some(SchedulerControlMsg::TaskCommandError(
                        CommandError::NoCommandMatch,
                    ));
                }

                // Try to find :huh. If it exists, we'll dispatch to that, instead.
                // If we don't find it, that's the end of the line.
                let Ok(verb_info) =
                    world_state.find_method_verb_on(self.perms, player_location, "huh")
                else {
                    return Some(SchedulerControlMsg::TaskCommandError(
                        CommandError::NoCommandMatch,
                    ));
                };
                let words = parse_into_words(command);
                trace!(?verb_info, ?player, ?player_location, args = ?words,
                            "Dispatching to :huh");

                (verb_info, player_location)
            }
        };
        let verb_call = VerbCall {
            verb_name: parsed_command.verb.clone(),
            location: target,
            this: target,
            player,
            args: List::from_slice(&parsed_command.args),
            argstr: parsed_command.argstr.clone(),
            caller: player,
        };
        self.vm_host.start_call_command_verb(
            self.task_id,
            verb_info,
            verb_call,
            parsed_command,
            self.perms,
        );
        None
    }
}

fn find_verb_for_command(
    player: Objid,
    player_location: Objid,
    pc: &ParsedCommand,
    ws: &mut dyn WorldState,
) -> Result<Option<(VerbInfo, Objid)>, CommandError> {
    let targets_to_search = vec![
        player,
        player_location,
        pc.dobj.unwrap_or(NOTHING),
        pc.iobj.unwrap_or(NOTHING),
    ];
    for target in targets_to_search {
        let match_result = ws.find_command_verb_on(
            player,
            target,
            pc.verb.as_str(),
            pc.dobj.unwrap_or(NOTHING),
            pc.prep,
            pc.iobj.unwrap_or(NOTHING),
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

// TODO: a battery of unit tests here. Which will likely involve setting up a standalone VM running
//   a simple program.
