use std::sync::Arc;
use std::time::SystemTime;

use metrics_macros::increment_counter;
use tokio::select;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;
use tracing::{debug, error, info, instrument, trace, warn};

use moor_values::model::verb_info::VerbInfo;
use moor_values::model::world_state::WorldState;
use moor_values::model::CommandError::PermissionDenied;
use moor_values::model::{CommandError, CommitResult, WorldStateError};
use moor_values::var::objid::Objid;
use moor_values::var::variant::Variant;
use moor_values::var::{v_int, v_string};
use moor_values::NOTHING;

use crate::matching::match_env::MatchEnvironmentParseMatcher;
use crate::matching::ws_match_env::WsMatchEnv;
use crate::tasks::command_parse::{
    parse_command, parse_into_words, ParseCommandError, ParsedCommand,
};
use crate::tasks::moo_vm_host::MooVmHost;
use crate::tasks::scheduler::{AbortLimitReason, TaskDescription};
use crate::tasks::sessions::Session;
use crate::tasks::task_messages::{SchedulerControlMsg, TaskControlMsg};
use crate::tasks::vm_host::{VMHost, VMHostResponse};
use crate::tasks::{TaskId, VerbCall};

/// A task is a concurrent, transactionally isolated, thread of execution. It starts with the
/// execution of a 'verb' (or 'command verb' or 'eval' etc) and runs through to completion or
/// suspension or abort.
/// Within the task many verbs may be executed as subroutine calls from the root verb/command
/// Each task has its own VM host which is responsible for executing the program.
/// Each task has its own isolated transactional world state.
/// Each task is given a semi-isolated "session" object through which I/O is performed.
/// When a task fails, both the world state and I/O should be rolled back.
/// A task is generally tied 1:1 with a player connection, and usually come from one command, but
/// they can also be 'forked' from other tasks.
pub(crate) struct Task {
    /// My unique task id.
    pub(crate) task_id: TaskId,
    /// When this task will begin execution.
    /// For currently execution tasks this is when the task actually began running.
    /// For tasks in suspension, this is when they will wake up.
    /// If the task is in indefinite suspension, this is None.
    pub(crate) scheduled_start_time: Option<SystemTime>,
    /// The channel to send control messages to the scheduler.
    /// This sender is unique for our task, but is passed around all over the place down into the
    /// VM host and into the VM itself.
    pub(crate) scheduler_control_sender: UnboundedSender<(TaskId, SchedulerControlMsg)>,
    /// The 'player' this task is running as.
    pub(crate) player: Objid,
    /// The session object for connection mgmt and sending messages to players
    pub(crate) session: Arc<dyn Session>,
    /// The transactionaly isolated world state for this task.
    pub(crate) world_state: Box<dyn WorldState>,
    /// The permissions of the task -- the object on behalf of which all permissions are evaluated.
    pub(crate) perms: Objid,
    /// The actual VM host which is managing the execution of this task.
    pub(crate) vm_host: MooVmHost,
}

#[derive(Debug, PartialEq, Eq)]
enum VmContinue {
    Continue,
    Complete,
}

impl Task {
    #[instrument(skip(self), name = "task_run")]
    pub(crate) async fn run(
        mut self,
        mut task_control_receiver: UnboundedReceiver<TaskControlMsg>,
    ) {
        loop {
            // We have two potential sources of concurrent action here:
            //    * The VM host, which is running the VM and may need to suspend or abort or do
            //      something else.
            //    * The reception channel from the scheduler, which may be asking us to do
            //      something
            select! {
                // Run the dispatch loop for the virtual machine.
                vm_continuation = self.vm_dispatch() => {
                    trace!(task_id = ?self.task_id, ?vm_continuation, "VM dispatch");
                    if let Some(scheduler_msg) = vm_continuation.1 {
                         self.scheduler_control_sender
                                        .send((self.task_id, scheduler_msg))
                                        .expect("Could not send scheduler_msg");
                    }
                    if vm_continuation.0 == VmContinue::Complete {
                        debug!(task_id = ?self.task_id, "task execution complete");
                        break;
                    }
                }
                // And also handle any control messages from the scheduler.
                control_msg = task_control_receiver.recv() => {
                    match control_msg {
                        Some(control_msg) => {
                             if let Some(response) = self.handle_control_message(control_msg).await {
                                self.scheduler_control_sender
                                        .send((self.task_id, response))
                                        .expect("Could not send response");
                             }
                        }
                        None => {
                            debug!(task_id = ?self.task_id, "Channel closed");
                            return;
                        }
                    }
                }

            }
        }
    }

    /// The VM dispatch loop. If we're actively running, we'll dispatch to the VM host to execute
    /// the next instruction. If we're suspended, we'll wait for a Resume message from the
    /// scheduler.
    /// Returns a tuple of (VmContinue, Option<SchedulerControlMsg>), where VmContinue indicates
    /// whether the VM should continue running, and the SchedulerControlMsg is a message to send
    /// back to the scheduler, if any.
    async fn vm_dispatch(&mut self) -> (VmContinue, Option<SchedulerControlMsg>) {
        let vm_exec_result = self
            .vm_host
            .exec_interpreter(self.task_id, self.world_state.as_mut())
            .await;
        match vm_exec_result {
            VMHostResponse::DispatchFork(fork_request) => {
                trace!(task_id = self.task_id, ?fork_request, "Task fork");
                // To fork a new task, we need to get the scheduler to do some work for us. So we'll
                // send a message back asking it to fork the task and return the new task id on a
                // reply channel.
                // We will then take the new task id and send it back to the caller.
                let (send, reply) = oneshot::channel();
                let task_id_var = fork_request.task_id;
                self.scheduler_control_sender
                    .send((
                        self.task_id,
                        SchedulerControlMsg::TaskRequestFork(fork_request, send),
                    ))
                    .expect("Could not send fork request");
                let task_id = reply.await.expect("Could not get fork reply");
                if let Some(task_id_var) = task_id_var {
                    self.vm_host
                        .set_variable(task_id_var, v_int(task_id as i64));
                }
                (VmContinue::Continue, None)
            }
            VMHostResponse::Suspend(delay) => {
                trace!(task_id = self.task_id, delay = ?delay, "Task suspend");

                // VMHost is now suspended for execution, and we'll be waiting for a Resume

                // Attempt commit...
                // TODO: what to do on conflict? The whole thing needs to be retried, but we have
                // not implemented that at any other level yet, so we'll just abort for now.
                let commit_result = self
                    .world_state
                    .commit()
                    .await
                    .expect("Could not commit world state before suspend");
                if let CommitResult::ConflictRetry = commit_result {
                    error!("Conflict during commit before suspend");
                    return (
                        VmContinue::Complete,
                        Some(SchedulerControlMsg::TaskAbortCancelled),
                    );
                }

                // let new_session = self
                //     .session
                //     .clone()
                //     .fork()
                //     .await
                //     .expect("Could not fork session for suspension");
                // Request the session flush itself out, as it's now committed, and we'll replace
                // with the new session when we resume.
                self.session
                    .commit()
                    .await
                    .expect("Could not commit session before suspend");

                // TODO: here and below, fork (for at least RpcSession) and replacing into here
                //   seems to not produce any output. Likely because the session is not actually
                //   replaced everywhere?  Or the RpcSession is invalid somehow.  Luckily, it's
                //   harmless to just not fork here, commit() flushes and then allows additional
                //   appends.
                // self.session = new_session;

                self.vm_host.stop().await;

                // Let the scheduler know about our suspension, which can be of the form:
                //      * Indefinite, wake-able only with Resume
                //      * Scheduled, a duration is given, and we'll wake up after that duration
                // In both cases we'll rely on the scheduler to wake us up in its processing loop
                // rather than sleep here, which would make this thread unresponsive to other
                // messages.
                let resume_time = delay.map(|delay| SystemTime::now() + delay);
                (
                    VmContinue::Continue,
                    Some(SchedulerControlMsg::TaskSuspend(resume_time)),
                )
            }
            VMHostResponse::SuspendNeedInput => {
                trace!(task_id = self.task_id, "Task suspend need input");

                // VMHost is now suspended for input, and we'll be waiting for a ResumeReceiveInput

                // Attempt commit... See comments/notes on Suspend above.
                let commit_result = self
                    .world_state
                    .commit()
                    .await
                    .expect("Could not commit world state before suspend");
                if let CommitResult::ConflictRetry = commit_result {
                    error!("Conflict during commit before suspend");
                    return (
                        VmContinue::Complete,
                        Some(SchedulerControlMsg::TaskAbortCancelled),
                    );
                }

                // let new_session = self
                //     .session
                //     .clone()
                //     .fork()
                //     .await
                //     .expect("Could not fork session for suspension");
                self.session
                    .commit()
                    .await
                    .expect("Could not commit session before suspend");
                // self.session = new_session;

                self.vm_host.stop().await;

                (
                    VmContinue::Continue,
                    Some(SchedulerControlMsg::TaskRequestInput),
                )
            }
            VMHostResponse::ContinueOk => (VmContinue::Continue, None),
            VMHostResponse::CompleteSuccess(result) => {
                trace!(task_id = self.task_id, result = ?result, "Task complete, success");

                // TODO: restart the whole task on conflict.
                let CommitResult::Success = self
                    .world_state
                    .commit()
                    .await
                    .expect("Could not attempt commit")
                else {
                    unimplemented!("Task restart on conflict")
                };

                self.session
                    .commit()
                    .await
                    .expect("Could not commit session...");

                (
                    VmContinue::Complete,
                    Some(SchedulerControlMsg::TaskSuccess(result)),
                )
            }
            VMHostResponse::CompleteAbort => {
                error!(task_id = self.task_id, "Task aborted");
                if let Err(send_error) = self
                    .session
                    .send_system_msg(self.player, "Aborted.".to_string().as_str())
                    .await
                {
                    warn!("Could not send abort message to player: {:?}", send_error);
                };

                self.world_state
                    .rollback()
                    .await
                    .expect("Could not rollback world state transaction");
                self.session
                    .rollback()
                    .await
                    .expect("Could not rollback connection...");

                (
                    VmContinue::Complete,
                    Some(SchedulerControlMsg::TaskAbortCancelled),
                )
            }
            VMHostResponse::CompleteException(exception) => {
                // Compose a string out of the backtrace
                let mut traceback = vec![];
                for frame in exception.backtrace.iter() {
                    let Variant::Str(s) = frame.variant() else {
                        continue;
                    };
                    traceback.push(format!("{:}\n", s));
                }

                for l in traceback.iter() {
                    if let Err(send_error) =
                        self.session.send_system_msg(self.player, l.as_str()).await
                    {
                        warn!("Could not send traceback to player: {:?}", send_error);
                    }
                }

                // Commands that end in exceptions are still expected to be committed, to
                // conform with MOO's expectations.
                // We may revisit this later.
                self.world_state.commit().await.expect("Could not commit");
                self.session
                    .commit()
                    .await
                    .expect("Could not commit connection output");

                (
                    VmContinue::Complete,
                    Some(SchedulerControlMsg::TaskException(exception)),
                )
            }
            VMHostResponse::AbortLimit(reason) => {
                let abort_reason_text = match reason {
                    AbortLimitReason::Ticks(ticks) => {
                        format!("Abort: Task exceeded ticks limit of {}", ticks)
                    }
                    AbortLimitReason::Time(time) => {
                        format!("Abort: Task exceeded time limit of {:?}", time)
                    }
                };
                self.session
                    .send_system_msg(self.player, abort_reason_text.as_str())
                    .await
                    .expect("Could not send abort message to player");
                self.world_state
                    .rollback()
                    .await
                    .expect("Could not rollback world state");
                self.session
                    .rollback()
                    .await
                    .expect("Could not rollback connection output");
                (
                    VmContinue::Complete,
                    Some(SchedulerControlMsg::TaskAbortLimitsReached(reason)),
                )
            }
        }
    }

    /// Handle an inbound control message from the scheduler, and return a response message to send
    ///  back, if any.
    async fn handle_control_message(&mut self, msg: TaskControlMsg) -> Option<SchedulerControlMsg> {
        match msg {
            // We've been asked to start a command.
            // We need to set up the VM and then execute it.
            TaskControlMsg::StartCommandVerb { player, command } => {
                increment_counter!("task.start_command");

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

                // TODO First try to match $do_command. And execute that, scheduling a callback into
                //   this stage again, if that fails. For now though, we rely on the daemon having
                //   done this work for us.

                // Next, try parsing the command.

                // We need the player's location, and we'll just die if we can't get it.
                let player_location = match self.world_state.location_of(player, player).await {
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
                    ws: self.world_state.as_mut(),
                    perms: player,
                };
                let matcher = MatchEnvironmentParseMatcher { env: me, player };
                let parsed_command = match parse_command(&command, matcher).await {
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
                let parse_results = match find_verb_for_command(
                    player,
                    player_location,
                    &parsed_command,
                    self.world_state.as_mut(),
                )
                .await
                {
                    Ok(results) => results,
                    Err(e) => return Some(SchedulerControlMsg::TaskCommandError(e)),
                };
                let (verb_info, target) = match parse_results {
                    // If we have a successul match, that's what we'll call into
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
                        let Ok(verb_info) = self
                            .world_state
                            .find_method_verb_on(self.perms, player_location, "huh")
                            .await
                        else {
                            return Some(SchedulerControlMsg::TaskCommandError(
                                CommandError::NoCommandMatch,
                            ));
                        };
                        let words = parse_into_words(&command);
                        info!(?verb_info, ?player, ?player_location, args = ?words, "Dispatching to :huh");

                        (verb_info, player_location)
                    }
                };
                let verb_call = VerbCall {
                    verb_name: parsed_command.verb.clone(),
                    location: target,
                    this: target,
                    player,
                    args: parsed_command.args.clone(),
                    argstr: parsed_command.argstr.clone(),
                    caller: player,
                };
                self.vm_host
                    .start_call_command_verb(
                        self.task_id,
                        verb_info,
                        verb_call,
                        parsed_command,
                        self.perms,
                    )
                    .await;
            }

            TaskControlMsg::StartVerb {
                player,
                vloc,
                verb,
                args,
                argstr,
            } => {
                increment_counter!("task.start_verb");
                // We should never be asked to start a command while we're already running one.
                trace!(?verb, ?player, ?vloc, ?args, "Starting verb");

                let verb_call = VerbCall {
                    verb_name: verb,
                    location: vloc,
                    this: vloc,
                    player,
                    args,
                    argstr,
                    caller: NOTHING,
                };
                // Find the callable verb ...
                let Ok(verb_info) = self
                    .world_state
                    .find_method_verb_on(self.perms, verb_call.this, verb_call.verb_name.as_str())
                    .await
                else {
                    return Some(SchedulerControlMsg::TaskVerbNotFound(
                        verb_call.this,
                        verb_call.verb_name,
                    ));
                };

                self.vm_host
                    .start_call_method_verb(self.task_id, self.perms, verb_info, verb_call)
                    .await;
            }
            TaskControlMsg::StartFork {
                task_id,
                fork_request,
                suspended,
            } => {
                trace!(?task_id, suspended, "Setting up fork");

                self.vm_host
                    .start_fork(task_id, fork_request, suspended)
                    .await;
            }
            TaskControlMsg::StartEval { player, program } => {
                increment_counter!("task.start_eval");

                self.scheduled_start_time = None;
                self.vm_host.start_eval(self.task_id, player, program).await;
            }
            TaskControlMsg::Resume(world_state, value) => {
                increment_counter!("task.resume");

                // We're back.
                debug!(
                    task_id = self.task_id,
                    "Resuming task, with new transaction"
                );
                self.world_state = world_state;
                self.scheduled_start_time = None;
                self.vm_host.resume_execution(value).await;
                return None;
            }
            TaskControlMsg::ResumeReceiveInput(world_state, input) => {
                increment_counter!("task.resume_receive_input");

                // We're back.
                debug!(
                    task_id = self.task_id,
                    ?input,
                    "Resuming task, with new transaction and input"
                );
                assert!(!self.vm_host.is_running());
                self.world_state = world_state;
                self.scheduled_start_time = None;
                self.vm_host.resume_execution(v_string(input)).await;
                return None;
            }
            TaskControlMsg::Abort => {
                // We've been asked to die. Go tell the VM host to abort, and roll back the
                // transaction.
                increment_counter!("task.abort");
                trace!(task_id = self.task_id, "Aborting task");
                self.vm_host.stop().await;

                // Failure to rollback is a panic, something is fundamentally wrong, and we're best
                //   to just restart.
                self.world_state
                    .rollback()
                    .await
                    .expect("Could not rollback transaction. Panic.");

                // And now tell the scheduler we're done, as we exit.
                return Some(SchedulerControlMsg::TaskAbortCancelled);
            }
            TaskControlMsg::Describe(reply_sender) => {
                increment_counter!("task.describe");

                let description = TaskDescription {
                    task_id: self.task_id,
                    start_time: self.scheduled_start_time,
                    permissions: self.vm_host.permissions(),
                    verb_name: self.vm_host.verb_name(),
                    verb_definer: self.vm_host.verb_definer(),
                    line_number: self.vm_host.line_number(),
                    this: self.vm_host.this(),
                };
                reply_sender
                    .send(description)
                    .expect("Could not send task description");
                return None;
            }
        }
        None
    }
}

async fn find_verb_for_command(
    player: Objid,
    player_location: Objid,
    pc: &ParsedCommand,
    ws: &mut dyn WorldState,
) -> Result<Option<(VerbInfo, Objid)>, CommandError> {
    let targets_to_search = vec![player, player_location, pc.dobj, pc.iobj];
    for target in targets_to_search {
        let match_result = ws
            .find_command_verb_on(player, target, pc.verb.as_str(), pc.dobj, pc.prep, pc.iobj)
            .await;
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
