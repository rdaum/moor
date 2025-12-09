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
use std::{
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

use crate::task_context::{
    commit_current_transaction, rollback_current_transaction, with_current_transaction,
    with_current_transaction_mut, with_new_transaction,
};

use flume::Sender;
use lazy_static::lazy_static;
use rand::Rng;
use tracing::{error, warn};

use crate::{
    trace_task_abort, trace_task_complete, trace_task_start, trace_task_suspend,
    trace_task_suspend_with_delay,
};

#[cfg(feature = "trace_events")]
use crate::{
    trace_abort_limit_reached, trace_task_create_command, trace_task_create_eval,
    trace_task_create_exception_handler, trace_task_create_fork, trace_task_create_verb,
};

#[cfg(feature = "trace_events")]
use moor_common::tasks::AbortLimitReason;
use moor_common::{
    model::{CommitResult, VerbDef, WorldState, WorldStateError},
    tasks::{CommandError, CommandError::PermissionDenied, Exception, TaskId},
    util::{PerfTimerGuard, parse_into_words},
};
use moor_var::{
    List, NOTHING, Obj, SYSTEM_OBJECT, Symbol, Variant, v_empty_str, v_err, v_int, v_obj, v_str,
    v_string,
};

use crate::{
    config::{Config, FeaturesConfig},
    tasks::{
        ServerOptions, TaskStart, sched_counters,
        task_scheduler_client::{TaskControlMsg, TaskSchedulerClient, TimeoutHandlerInfo},
    },
    vm::{
        TaskSuspend, VMHostResponse, builtins::BuiltinRegistry, exec_state::VMExecState,
        vm_host::VmHost,
    },
};
use moor_common::{
    matching::{
        CommandParser, ComplexObjectNameMatcher, DefaultParseCommand, ParseCommandError,
        ParsedCommand, WsMatchEnv,
    },
    tasks::Session,
};
use moor_var::program::ProgramType;

lazy_static! {
    static ref HUH_SYM: Symbol = Symbol::mk("huh");
    static ref HANDLE_UNCAUGHT_ERROR_SYM: Symbol = Symbol::mk("handle_uncaught_error");
    static ref HANDLE_TASK_TIMEOUT_SYM: Symbol = Symbol::mk("handle_task_timeout");
    static ref DO_COMMAND_SYM: Symbol = Symbol::mk("do_command");
}

/// Tracks the lifecycle state of a task
#[derive(Debug, Clone)]
pub enum TaskState {
    /// Task pending execution, their host and activation frames are not yet set up, and is not
    /// prepared for execution yet.
    Pending(TaskStart),
    /// Task has had its state set up and ready to go.
    Prepared(TaskStart),
}

impl TaskState {
    pub fn task_start(&self) -> &TaskStart {
        match self {
            TaskState::Pending(start) => start,
            TaskState::Prepared(start) => start,
        }
    }

    pub fn is_background(&self) -> bool {
        self.task_start().is_background()
    }
}

#[derive(Debug)]
pub struct Task {
    /// My unique task id.
    pub task_id: TaskId,
    /// When I was first instantiated (not necessarily) started
    pub creation_time: minstant::Instant,
    /// What I was asked to do and current lifecycle state.
    pub(crate) state: TaskState,
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
    /// True if we're currently handling an uncaught error to prevent infinite recursion.
    pub(crate) handling_uncaught_error: bool,
    /// The original exception when calling handle_uncaught_error, in case it returns false.
    pub(crate) pending_exception: Option<Exception>,
}

impl Task {
    // Yes yes I know it's a lot of arguments, but wrapper object here is redundant.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        task_id: TaskId,
        player: Obj,
        perms: Obj,
        task_start: TaskStart,
        server_options: &ServerOptions,
        kill_switch: Arc<AtomicBool>,
    ) -> Box<Self> {
        let is_background = task_start.is_background();
        let state = TaskState::Pending(task_start.clone());

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

        // Emit task creation trace event based on task start type
        #[cfg(feature = "trace_events")]
        {
            match &task_start {
                TaskStart::StartCommandVerb {
                    command,
                    handler_object,
                    ..
                } => {
                    trace_task_create_command!(task_id, &player, command, handler_object);
                }
                TaskStart::StartDoCommand {
                    command,
                    handler_object,
                    ..
                } => {
                    trace_task_create_command!(task_id, &player, command, handler_object);
                }
                TaskStart::StartVerb { verb, vloc, .. } => {
                    trace_task_create_verb!(task_id, &player, &verb.as_string(), vloc);
                }
                TaskStart::StartFork { .. } => {
                    trace_task_create_fork!(task_id, &player);
                }
                TaskStart::StartEval { .. } => {
                    trace_task_create_eval!(task_id, &player);
                }
                TaskStart::StartExceptionHandler { .. } => {
                    trace_task_create_exception_handler!(task_id, &player);
                }
            }
        }

        let creation_time = minstant::Instant::now();
        Box::new(Self {
            task_id,
            creation_time,
            player,
            state,
            vm_host,
            perms,
            kill_switch,
            retries: 0,
            retry_state,
            handling_uncaught_error: false,
            pending_exception: None,
        })
    }

    pub fn run_task_loop(
        mut task: Box<Task>,
        task_scheduler_client: &TaskSchedulerClient,
        session: Arc<dyn Session>,
        builtin_registry: BuiltinRegistry,
        config: Arc<Config>,
    ) {
        // Transaction context is already set up by the caller

        trace_task_start!(task.task_id);

        // Try to pick a high thread priority for user tasks.
        gdt_cpus::set_thread_priority(gdt_cpus::ThreadPriority::AboveNormal).ok();

        while task.vm_host.is_running() {
            // Check kill switch.
            if task.kill_switch.load(std::sync::atomic::Ordering::Relaxed) {
                task_scheduler_client.abort_cancelled();
                break;
            }

            if let Some(continuation_task) = task.vm_dispatch(
                task_scheduler_client,
                session.as_ref(),
                &builtin_registry,
                config.features.as_ref(),
            ) {
                task = continuation_task;
            } else {
                break;
            }
        }

        // Transaction is automatically cleaned up by _tx_guard drop
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
        mut self: Box<Self>,
        task_scheduler_client: &TaskSchedulerClient,
        session: &dyn Session,
        builtin_registry: &BuiltinRegistry,
        config: &FeaturesConfig,
    ) -> Option<Box<Self>> {
        // Call the VM using transaction context
        let vm_exec_result =
            self.vm_host
                .exec_interpreter(self.task_id, session, builtin_registry, config);

        // Having done that, what should we now do?
        match vm_exec_result {
            VMHostResponse::DispatchFork(fork_request) => {
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
                Some(self)
            }
            VMHostResponse::Suspend(delay) => {
                // Check for immediate wake conditions to avoid scheduler round-trip
                let (is_immediate, resume_value) = match delay.as_ref() {
                    TaskSuspend::Commit(val) => (true, val.clone()),
                    TaskSuspend::Timed(d) if d.is_zero() => (true, v_int(0)),
                    _ => (false, v_int(0)),
                };

                if is_immediate {
                    // Fast path: get new transaction and continue immediately
                    match with_new_transaction(|| {
                        let new_world_state =
                            task_scheduler_client.begin_new_transaction().map_err(|e| {
                                WorldStateError::DatabaseError(format!("Scheduler error: {e:?}"))
                            })?;
                        Ok((new_world_state, ()))
                    }) {
                        Ok((CommitResult::Success { .. }, _)) => {
                            self.retry_state = self.vm_host.snapshot_state();
                            self.vm_host.resume_execution(resume_value);
                            return Some(self);
                        }
                        Ok((CommitResult::ConflictRetry, _)) => {
                            warn!("Conflict during immediate resume transaction");
                            session.rollback().unwrap();
                            task_scheduler_client.conflict_retry(self);
                            return None;
                        }
                        Err(e) => {
                            error!(
                                "Failed to begin new transaction for immediate resume: {:?}",
                                e
                            );
                            // Fall back to normal suspend path
                        }
                    }
                }

                // VMHost is now suspended for execution, and we'll be waiting for a Resume
                let commit_result = commit_current_transaction()
                    .expect("Could not commit world state before suspend");

                if let CommitResult::ConflictRetry = commit_result {
                    warn!("Conflict during commit before suspend");
                    session.rollback().unwrap();
                    task_scheduler_client.conflict_retry(self);
                    return None;
                }

                self.retry_state = self.vm_host.snapshot_state();
                self.vm_host.stop();

                trace_task_suspend_with_delay!(self.task_id, delay.as_ref());

                // Let the scheduler know about our suspension, which can be of the form:
                //      * Indefinite, wake-able only with Resume
                //      * Scheduled, a duration is given, and we'll wake up after that duration
                // In both cases we'll rely on the scheduler to wake us up in its processing loop
                // rather than sleep here, which would make this thread unresponsive to other
                // messages.
                task_scheduler_client.suspend(delay.as_ref().clone(), self);
                None
            }
            VMHostResponse::SuspendNeedInput(metadata) => {
                // VMHost is now suspended for input, and we'll be waiting for a ResumeReceiveInput

                // Attempt commit... See comments/notes on Suspend above.
                let commit_result = commit_current_transaction()
                    .expect("Could not commit world state before suspend");

                if let CommitResult::ConflictRetry = commit_result {
                    warn!("Conflict during commit before suspend");
                    session.rollback().unwrap();
                    task_scheduler_client.conflict_retry(self);
                    return None;
                }

                self.retry_state = self.vm_host.snapshot_state();
                self.vm_host.stop();

                trace_task_suspend!(self.task_id, "Waiting for input");

                // Consume us, passing back to the scheduler that we're waiting for input.
                task_scheduler_client.request_input(self, metadata);
                None
            }
            VMHostResponse::ContinueOk => Some(self),

            VMHostResponse::CompleteSuccess(result) => {
                // Special case: in case of return from $do_command @ top-level, we need to look at the results:
                //      non-true value? => parse_command and restart (in same transaction)
                //      true value? => commit and return success.
                if let TaskStart::StartDoCommand {
                    handler_object,
                    player,
                    command,
                } = self.state.task_start()
                {
                    let (player, command) = (*player, command.clone());
                    if !result.is_true() {
                        // Intercept and rewrite us back to StartVerbCommand and do old school parse.
                        self.state = TaskState::Prepared(TaskStart::StartCommandVerb {
                            handler_object: *handler_object,
                            player,
                            command: command.clone(),
                        });

                        if let Err(e) = with_current_transaction_mut(|world_state| {
                            self.setup_start_parse_command(&player, &command, world_state)
                        }) {
                            task_scheduler_client.command_error(e);
                        }
                        return Some(self);
                    }
                }

                // Special case: if we're returning from $handle_uncaught_error, check the result
                if self.handling_uncaught_error {
                    self.handling_uncaught_error = false;

                    // If handler returned false, proceed with normal exception handling
                    if !result.is_true() {
                        let Some(original_exception) = self.pending_exception.take() else {
                            warn!(
                                task_id = self.task_id,
                                "handle_uncaught_error returned false, but original exception lost"
                            );
                            return None; // Can't restore exception, abort task
                        };

                        // Restore the original exception and handle it normally
                        let commit_result =
                            commit_current_transaction().expect("Could not attempt commit");

                        let CommitResult::Success { .. } = commit_result else {
                            error!(
                                "Conflict during commit before exception handling, asking scheduler to retry task ({})",
                                self.task_id
                            );
                            session.rollback().unwrap();
                            task_scheduler_client.conflict_retry(self);
                            return None;
                        };

                        // Debug level - this is normal when handle_uncaught_error doesn't exist or returns false
                        // The exception itself is already being handled and reported to the client
                        tracing::debug!(
                            task_id = self.task_id,
                            ?original_exception,
                            "Task exception (handle_uncaught_error returned false)"
                        );
                        self.vm_host.stop();

                        trace_task_abort!(
                            self.task_id,
                            &format!("Exception: {}", original_exception.error.err_type)
                        );

                        task_scheduler_client.exception(Box::new(original_exception));
                        return None;
                    }

                    // Handler returned true, clear pending exception and continue with success
                    self.pending_exception = None;
                }

                let commit_result = commit_current_transaction().expect("Could not attempt commit");

                let CommitResult::Success {
                    mutations_made,
                    timestamp,
                } = commit_result
                else {
                    warn!(
                        "Conflict during commit before complete, asking scheduler to retry task for task_id: {}, player {}, retry # {}, task_start: {}",
                        self.task_id,
                        self.player,
                        self.retries,
                        self.state.task_start().diagnostic(),
                    );
                    session.rollback().unwrap();

                    // Add randomized backoff to prevent retry storms. Base delay is 10-50ms, multiplied by retry count.
                    let mut rng = rand::rng();
                    let base_delay_ms = rng.random_range(10..=50);
                    let delay_ms = base_delay_ms * (self.retries + 1) as u64; // +1 since retries will be incremented in scheduler
                    std::thread::sleep(std::time::Duration::from_millis(delay_ms));

                    task_scheduler_client.conflict_retry(self);
                    return None;
                };

                self.vm_host.stop();

                trace_task_complete!(self.task_id, &format!("{result:?}"));

                task_scheduler_client.success(result, mutations_made, timestamp);
                None
            }
            VMHostResponse::CompleteAbort => {
                error!(task_id = self.task_id, "Task aborted");

                rollback_current_transaction().expect("Could not rollback world state transaction");

                self.vm_host.stop();

                trace_task_abort!(self.task_id, "Task aborted");

                task_scheduler_client.abort_cancelled();
                None
            }
            VMHostResponse::CompleteException(exception) => {
                // Check if we're already handling an uncaught error (prevent infinite recursion)
                if self.handling_uncaught_error {
                    // We're in the handler and it threw an exception.
                    // Fall through to normal exception reporting below.
                } else if let TaskState::Prepared(TaskStart::StartExceptionHandler { .. })
                | TaskState::Pending(TaskStart::StartExceptionHandler { .. }) =
                    &self.state
                {
                    // Current task IS the exception handler and it threw.
                    // Fall through to normal exception reporting.
                } else {
                    // Try to find and invoke $handle_uncaught_error on #0 (SYSTEM_OBJECT)
                    let verb_lookup = with_current_transaction(|world_state| {
                        world_state.find_method_verb_on(
                            &self.perms,
                            &SYSTEM_OBJECT,
                            *HANDLE_UNCAUGHT_ERROR_SYM,
                        )
                    });

                    if let Ok((program, verbdef)) = verb_lookup {
                        // Handler exists - prepare to invoke it
                        // Prepare arguments: {code, msg, value, stack, traceback}
                        let code = v_err(exception.error.err_type);
                        let msg = match &exception.error.msg {
                            Some(m) => v_string(m.to_string()),
                            None => v_str(""),
                        };
                        let value = exception
                            .error
                            .value
                            .as_deref()
                            .cloned()
                            .unwrap_or(v_int(0));
                        let stack = List::from_iter(exception.stack.clone());
                        let traceback = List::from_iter(exception.backtrace.clone());

                        let args =
                            List::from_iter(vec![code, msg, value, stack.into(), traceback.into()]);

                        // Store the original exception and mark that we're handling uncaught error
                        self.pending_exception = Some((*exception).clone());
                        self.handling_uncaught_error = true;

                        // Set up the handler as a method call on SYSTEM_OBJECT
                        self.vm_host.start_call_method_verb(
                            self.task_id,
                            self.perms,
                            verbdef,
                            *HANDLE_UNCAUGHT_ERROR_SYM,
                            v_obj(SYSTEM_OBJECT),
                            self.player,
                            args,
                            v_obj(self.player),
                            v_empty_str(),
                            program,
                        );

                        // Continue execution - the handler will now run
                        return Some(self);
                    }

                    // No handler exists or error looking it up
                    match verb_lookup.unwrap_err() {
                        WorldStateError::VerbNotFound(_, _) => {
                            // No handler exists, proceed with normal exception reporting
                        }
                        e => {
                            error!(task_id = ?self.task_id, "Error looking up handle_uncaught_error: {:?}", e);
                            // Proceed with normal exception reporting
                        }
                    }
                }

                // Normal exception reporting (either no handler found, or handler itself threw)
                // Commands that end in exceptions are still expected to be committed, to
                // conform with MOO's expectations.
                let commit_result = commit_current_transaction().expect("Could not attempt commit");

                let CommitResult::Success { .. } = commit_result else {
                    warn!(
                        "Conflict during commit before exception, asking scheduler to retry task ({})",
                        self.task_id
                    );
                    session.rollback().unwrap();
                    task_scheduler_client.conflict_retry(self);
                    return None;
                };

                // Format the backtrace for logging
                let backtrace_str: String = exception
                    .backtrace
                    .iter()
                    .filter_map(|v| v.as_string())
                    .map(|s| format!("        {}", s))
                    .collect::<Vec<_>>()
                    .join("\n");

                error!(
                    task_id = self.task_id,
                    player_id = self.player.to_literal(),
                    perms = self.perms.to_literal(),
                    error = %exception.error,
                    "Task exception:\n{}",
                    backtrace_str
                );
                self.vm_host.stop();

                trace_task_abort!(
                    self.task_id,
                    &format!("Exception: {}", exception.error.err_type)
                );

                task_scheduler_client.exception(exception);
                None
            }
            VMHostResponse::CompleteRollback(commit_session) => {
                // Rollback the transaction
                rollback_current_transaction().expect("Could not rollback world state transaction");

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

                // Inside a running task, stack should never be empty - if it is, that's a critical bug
                let this = self
                    .vm_host
                    .this()
                    .expect("Task has empty activation stack during abort - critical bug");
                let verb_name = self
                    .vm_host
                    .verb_name()
                    .expect("Task has empty activation stack during abort - critical bug");
                let line_number = self
                    .vm_host
                    .line_number()
                    .expect("Task has empty activation stack during abort - critical bug");

                // Emit trace event for abort limit
                #[cfg(feature = "trace_events")]
                {
                    let (limit_type, limit_value) = match &reason {
                        AbortLimitReason::Ticks(ticks) => {
                            ("Ticks".to_string(), format!("{}", ticks))
                        }
                        AbortLimitReason::Time(duration) => (
                            "Time".to_string(),
                            format!("{:.3}s", duration.as_secs_f64()),
                        ),
                    };

                    trace_abort_limit_reached!(
                        self.task_id,
                        &limit_type,
                        limit_value,
                        self.vm_host.max_ticks,
                        self.vm_host.tick_count(),
                        verb_name,
                        this.clone(),
                        line_number
                    );
                }

                // Collect traceback information for the handler
                let (stack_list, backtrace_list) = self.vm_host.get_traceback();
                let handler_info = TimeoutHandlerInfo {
                    stack: stack_list,
                    backtrace: backtrace_list,
                };

                // Stop execution, rollback transaction, and abort the task.
                // The scheduler will handle invoking $handle_task_timeout as a separate task.
                self.vm_host.stop();
                rollback_current_transaction().expect("Could not rollback world state");
                task_scheduler_client.abort_limits_reached(
                    reason,
                    this,
                    verb_name,
                    line_number,
                    handler_info,
                );
                None
            }
            VMHostResponse::RollbackRetry => {
                warn!(task_id = self.task_id, "Task rollback requested, retrying");

                self.vm_host.stop();
                rollback_current_transaction().expect("Could not rollback world state");

                session.rollback().unwrap();
                task_scheduler_client.conflict_retry(self);
                None
            }
        }
    }

    /// Set the task up to start executing, based on the task start configuration.
    pub(crate) fn setup_task_start(
        &mut self,
        control_sender: &Sender<(TaskId, TaskControlMsg)>,
    ) -> bool {
        let perfc = sched_counters();
        let _t = PerfTimerGuard::new(&perfc.setup_task);
        match self.state.task_start() {
            // We've been asked to start a command.
            // We need to set up the VM and then execute it.
            TaskStart::StartCommandVerb {
                handler_object,
                player,
                command,
            } => {
                let (handler_object, player, command) = (*handler_object, *player, command.clone());
                if let Err(e) = with_current_transaction_mut(|world_state| {
                    self.start_command(&handler_object, &player, command.as_str(), world_state)
                }) {
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
                let verb_name = *verb;
                let this = vloc.clone();
                let player = *player;
                let args_val = args.clone();
                let argstr_val = argstr.clone();
                let caller = v_obj(player);

                // Find the callable verb ...
                // Obj or flyweight?
                let object_location = match &this.variant() {
                    Variant::Flyweight(f) => *f.delegate(),
                    Variant::Obj(o) => *o,
                    _ => {
                        control_sender
                            .send((
                                self.task_id,
                                TaskControlMsg::TaskVerbNotFound(this, verb_name),
                            ))
                            .expect("Could not send start response");
                        return false;
                    }
                };
                match with_current_transaction(|world_state| {
                    world_state.find_method_verb_on(&self.perms, &object_location, verb_name)
                }) {
                    Err(WorldStateError::VerbNotFound(_, _)) => {
                        control_sender
                            .send((
                                self.task_id,
                                TaskControlMsg::TaskVerbNotFound(this, verb_name),
                            ))
                            .expect("Could not send start response");
                        return false;
                    }
                    Err(e) => {
                        error!(task_id = ?self.task_id, this = ?this,
                               verb = ?verb_name,
                               "World state error while resolving verb: {:?}", e);
                        panic!("Could not resolve verb: {e:?}");
                    }
                    Ok((program, verbdef)) => {
                        self.vm_host.start_call_method_verb(
                            self.task_id,
                            self.perms,
                            verbdef,
                            verb_name,
                            this,
                            player,
                            args_val,
                            caller,
                            argstr_val,
                            program,
                        );
                    }
                }
            }
            TaskStart::StartFork {
                fork_request,
                suspended: _,
            } => {
                // When setup_task_start is called, the task is being woken/started, so we always
                // pass suspended=false to ensure vm_host.running is set to true
                self.vm_host.start_fork(self.task_id, fork_request, false);
            }
            TaskStart::StartEval { player, program } => {
                self.vm_host
                    .start_eval(self.task_id, player, program.clone());
            }
            TaskStart::StartDoCommand { .. } => {
                panic!("StartDoCommand invocation should not happen on initial setup_task_start");
            }
            TaskStart::StartExceptionHandler { player, args, .. } => {
                // Start $handle_uncaught_error on the system object with the exception args
                // Find and set up the handler verb
                match with_current_transaction(|world_state| {
                    world_state.find_method_verb_on(
                        &self.perms,
                        &SYSTEM_OBJECT,
                        *HANDLE_UNCAUGHT_ERROR_SYM,
                    )
                }) {
                    Err(WorldStateError::VerbNotFound(_, _)) => {
                        // No handler defined - this shouldn't happen in setup, we would have checked before
                        warn!("handle_uncaught_error verb not found during setup");
                        return false;
                    }
                    Err(e) => {
                        error!(task_id = ?self.task_id, "Error resolving handle_uncaught_error: {e:?}");
                        return false;
                    }
                    Ok((program, verbdef)) => {
                        self.vm_host.start_call_method_verb(
                            self.task_id,
                            self.perms,
                            verbdef,
                            *HANDLE_UNCAUGHT_ERROR_SYM,
                            v_obj(SYSTEM_OBJECT),
                            *player,
                            args.clone(),
                            v_obj(*player),
                            v_empty_str(),
                            program,
                        );
                    }
                }
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
            world_state.find_method_verb_on(&self.perms, &SYSTEM_OBJECT, *DO_COMMAND_SYM);

        match do_command {
            Err(WorldStateError::VerbNotFound(_, _)) => {
                self.setup_start_parse_command(player, command, world_state)?;
            }
            Ok((program, verbdef)) => {
                let arguments = parse_into_words(command);
                let args = List::from_iter(arguments.iter().map(|s| v_str(s)));
                self.vm_host.start_call_method_verb(
                    self.task_id,
                    self.perms,
                    verbdef,
                    *DO_COMMAND_SYM,
                    v_obj(*handler_object),
                    *player,
                    args,
                    v_obj(*handler_object),
                    v_str(command),
                    program,
                );
                self.state = TaskState::Prepared(TaskStart::StartDoCommand {
                    handler_object: *handler_object,
                    player: *player,
                    command: command.to_string(),
                });
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
            let me = WsMatchEnv::new(world_state, *player);
            let matcher = ComplexObjectNameMatcher {
                env: me,
                player: *player,
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
        let ((program, verbdef), target) = match parse_results {
            // If we have a successful match, that's what we'll call into
            Some((verb_info, target)) => (verb_info, target),
            // Otherwise, we want to try to call :huh, if it exists.
            None => {
                if player_location == NOTHING {
                    return Err(CommandError::NoCommandMatch);
                }
                // Try to find :huh. If it exists, we'll dispatch to that, instead.
                // If we don't find it, that's the end of the line.
                let Ok((program, verbdef)) =
                    world_state.find_method_verb_on(&self.perms, &player_location, *HUH_SYM)
                else {
                    return Err(CommandError::NoCommandMatch);
                };
                ((program, verbdef), player_location)
            }
        };
        let verb_owner = verbdef.owner();
        self.vm_host.start_call_command_verb(
            self.task_id,
            verbdef,
            parsed_command.verb,
            v_obj(target),
            *player,
            List::mk_list(&parsed_command.args),
            v_obj(*player),
            v_string(parsed_command.argstr.clone()),
            parsed_command,
            verb_owner,
            program,
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
) -> Result<Option<((ProgramType, VerbDef), Obj)>, CommandError> {
    let perfc = sched_counters();
    let _t = PerfTimerGuard::new(&perfc.find_verb_for_command);
    let targets_to_search = vec![
        *player,
        *player_location,
        pc.dobj.unwrap_or(NOTHING),
        pc.iobj.unwrap_or(NOTHING),
    ];
    for target in targets_to_search {
        let match_result = ws.find_command_verb_on(
            player,
            &target,
            pc.verb,
            &pc.dobj.unwrap_or(NOTHING),
            pc.prep,
            &pc.iobj.unwrap_or(NOTHING),
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
#[cfg(test)]
mod tests {
    use std::sync::{Arc, atomic::AtomicBool};

    use crate::{task_context::TaskGuard, testing::vm_test_utils::setup_task_context};
    use flume::{Receiver, unbounded};

    use moor_common::{
        model::{
            ArgSpec, ObjectKind, PrepSpec, VerbArgsSpec, VerbFlag, WorldState, WorldStateSource,
        },
        tasks::{CommandError, Event, TaskId},
        util::BitEnum,
    };
    use moor_compiler::{CompileOptions, Program, compile};
    use moor_db::{DatabaseConfig, TxDB};
    use moor_var::{
        E_DIV, NOTHING, SYSTEM_OBJECT, Symbol, program::ProgramType, v_int, v_obj, v_str,
    };

    use crate::tasks::DEFAULT_MAX_TASK_RETRIES;
    use crate::{
        config::Config,
        tasks::{
            ServerOptions, TaskStart,
            task::Task,
            task_scheduler_client::{TaskControlMsg, TaskSchedulerClient},
        },
        vm::{activation::Frame, builtins::BuiltinRegistry},
    };
    use moor_common::tasks::NoopClientSession;

    struct TestVerb {
        name: Symbol,
        program: Program,
        argspec: VerbArgsSpec,
    }

    #[allow(clippy::type_complexity)]
    fn setup_test_env(
        task_start: TaskStart,
        programs: &[TestVerb],
    ) -> (
        Arc<AtomicBool>,
        Box<Task>,
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
            dump_interval: None,
            gc_interval: None,
            max_task_retries: DEFAULT_MAX_TASK_RETRIES,
        };
        let task_scheduler_client = TaskSchedulerClient::new(1, control_sender.clone());
        let task = Task::new(
            1,
            SYSTEM_OBJECT,
            SYSTEM_OBJECT,
            task_start.clone(),
            &server_options,
            kill_switch.clone(),
        );
        let (db, _) = TxDB::open(None, DatabaseConfig::default());
        let mut tx = db.new_world_state().unwrap();

        let sysobj = tx
            .create_object(
                &SYSTEM_OBJECT,
                &NOTHING,
                &SYSTEM_OBJECT,
                BitEnum::all(),
                ObjectKind::NextObjid,
            )
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
            tx.add_verb(
                &SYSTEM_OBJECT,
                &SYSTEM_OBJECT,
                vec![*name],
                &SYSTEM_OBJECT,
                BitEnum::new_with(VerbFlag::Exec),
                *argspec,
                ProgramType::MooR(program.clone()),
            )
            .unwrap();
        }

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
        Box<Task>,
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
        Box<Task>,
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
        let (_kill_switch, mut task, _db, tx, task_scheduler_client, control_receiver) =
            setup_test_env_eval("return 1 + 1;");

        let session = Arc::new(NoopClientSession::new());
        {
            let _tx_guard = setup_task_context(tx);
            task.setup_task_start(task_scheduler_client.control_sender());
            Task::run_task_loop(
                task,
                &task_scheduler_client,
                session,
                BuiltinRegistry::new(),
                Arc::new(Config::default()),
            );
        }

        // Scheduler should have received a TaskSuccess message.
        let (task_id, msg) = control_receiver.recv().unwrap();
        assert_eq!(task_id, 1);
        let TaskControlMsg::TaskSuccess(result, _mutations, _timestamp) = msg else {
            panic!("Expected TaskSuccess, got different message type");
        };
        assert_eq!(result, v_int(2));
    }

    /// Trigger a MOO VM exception, and verify it gets sent to scheduler
    #[test]
    fn test_simple_run_exception() {
        let (_kill_switch, mut task, _db, tx, task_scheduler_client, control_receiver) =
            setup_test_env_eval("return 1 / 0;");

        let session = Arc::new(NoopClientSession::new());
        {
            let _tx_guard = setup_task_context(tx);
            task.setup_task_start(task_scheduler_client.control_sender());
            Task::run_task_loop(
                task,
                &task_scheduler_client,
                session,
                BuiltinRegistry::new(),
                Arc::new(Config::default()),
            );
        }

        // Scheduler should have received a TaskException message.
        let (task_id, msg) = control_receiver.recv().unwrap();
        assert_eq!(task_id, 1);
        let TaskControlMsg::TaskException(exception) = msg else {
            panic!("Expected TaskException, got different message type");
        };
        assert_eq!(exception.error.err_type, E_DIV);
    }

    // notify() will dispatch to the scheduler
    #[test]
    fn test_notify_invocation() {
        let (_kill_switch, mut task, _db, tx, task_scheduler_client, control_receiver) =
            setup_test_env_eval(r#"notify(#0, "12345"); return 123;"#);

        let session = Arc::new(NoopClientSession::new());
        {
            let _tx_guard = TaskGuard::new(
                tx,
                task_scheduler_client.clone(),
                task.task_id,
                task.player,
                session.clone(),
            );
            task.setup_task_start(task_scheduler_client.control_sender());
            Task::run_task_loop(
                task,
                &task_scheduler_client,
                session,
                BuiltinRegistry::new(),
                Arc::new(Config::default()),
            );
        }

        // Scheduler should have received a TaskException message.
        let (task_id, msg) = control_receiver.recv().unwrap();
        assert_eq!(task_id, 1);
        let TaskControlMsg::Notify { player, event } = msg else {
            panic!("Expected Notify, got different message type");
        };
        assert_eq!(player, SYSTEM_OBJECT);
        assert_eq!(event.author(), &v_obj(SYSTEM_OBJECT));
        assert_eq!(
            event.event,
            Event::Notify {
                value: v_str("12345"),
                content_type: None,
                no_flush: false,
                no_newline: false,
                metadata: None,
            }
        );

        // Also scheduler should have received a TaskSuccess message.
        let (task_id, msg) = control_receiver.recv().unwrap();
        assert_eq!(task_id, 1);
        let TaskControlMsg::TaskSuccess(result, _mutations, _timestamp) = msg else {
            panic!("Expected TaskSuccess, got different message type");
        };
        assert_eq!(result, v_int(123));
    }

    /// Trigger a task-suspend-resume
    #[test]
    fn test_simple_run_suspend() {
        let (_kill_switch, mut task, db, tx, task_scheduler_client, control_receiver) =
            setup_test_env_eval("suspend(1); return 123;");

        let session = Arc::new(NoopClientSession::new());
        {
            let _tx_guard = setup_task_context(tx);
            task.setup_task_start(task_scheduler_client.control_sender());
            Task::run_task_loop(
                task,
                &task_scheduler_client,
                session.clone(),
                BuiltinRegistry::new(),
                Arc::new(Config::default()),
            );
        }

        // Scheduler should have received a TaskSuspend message.
        let (task_id, msg) = control_receiver.recv().unwrap();
        assert_eq!(task_id, 1);
        let TaskControlMsg::TaskSuspend(_, mut resume_task) = msg else {
            panic!("Expected TaskSuspend, got different message type");
        };
        assert_eq!(resume_task.task_id, 1);

        // Now we can simulate resumption...
        resume_task.vm_host.resume_execution(v_int(0));

        let tx = db.new_world_state().unwrap();
        {
            let _tx_guard = setup_task_context(tx);
            Task::run_task_loop(
                resume_task,
                &task_scheduler_client,
                session,
                BuiltinRegistry::new(),
                Arc::new(Config::default()),
            );
        }
        let (task_id, msg) = control_receiver.recv().unwrap();
        assert_eq!(task_id, 1);
        let TaskControlMsg::TaskSuccess(result, _mutations, _timestamp) = msg else {
            panic!("Expected TaskSuccess, got different message type");
        };
        assert_eq!(result, v_int(123));
    }

    /// Trigger a simulated read()
    #[test]
    fn test_simple_run_read() {
        let (_kill_switch, mut task, db, tx, task_scheduler_client, control_receiver) =
            setup_test_env_eval("return read();");

        let session = Arc::new(NoopClientSession::new());
        {
            let _tx_guard = setup_task_context(tx);
            task.setup_task_start(task_scheduler_client.control_sender());
            Task::run_task_loop(
                task,
                &task_scheduler_client,
                session.clone(),
                BuiltinRegistry::new(),
                Arc::new(Config::default()),
            );
        }

        // Scheduler should have received a TaskRequestInput message, and it should contain the task.
        let (task_id, msg) = control_receiver.recv().unwrap();
        assert_eq!(task_id, 1);
        let TaskControlMsg::TaskRequestInput(mut resume_task, _metadata) = msg else {
            panic!("Expected TaskRequestInput, got different message type");
        };
        assert_eq!(resume_task.task_id, 1);

        // Now we can simulate resumption...
        resume_task.vm_host.resume_execution(v_str("hello, world!"));

        // And run its task loop again, with a new transaction.
        let tx = db.new_world_state().unwrap();
        {
            let _tx_guard = setup_task_context(tx);
            Task::run_task_loop(
                resume_task,
                &task_scheduler_client,
                session,
                BuiltinRegistry::new(),
                Arc::new(Config::default()),
            );
        }

        // Scheduler should have received a TaskSuccess message.
        let (task_id, msg) = control_receiver.recv().unwrap();
        assert_eq!(task_id, 1);
        let TaskControlMsg::TaskSuccess(result, _mutations, _timestamp) = msg else {
            panic!("Expected TaskSuccess, got different message type");
        };
        assert_eq!(result, v_str("hello, world!"));
    }

    /// Trigger a task-fork
    #[test]
    fn test_simple_run_fork() {
        let (_kill_switch, mut task, db, tx, task_scheduler_client, control_receiver) =
            setup_test_env_eval("fork (1) return 1 + 1; endfork return 123;");
        tx.commit().unwrap();

        // Pull a copy of the program out for comparison later.
        let task_start = task.state.task_start().clone();
        let TaskStart::StartEval { program, .. } = &task_start else {
            panic!("Expected StartEval, got {:?}", task.state.task_start());
        };

        // This one needs to run in a thread because it's going to block waiting on a reply from
        // our fake scheduler.
        let jh = std::thread::spawn(move || {
            let tx = db.new_world_state().unwrap();
            let session = Arc::new(NoopClientSession::new());
            {
                let _tx_guard = setup_task_context(tx);
                task.setup_task_start(task_scheduler_client.control_sender());
                Task::run_task_loop(
                    task,
                    &task_scheduler_client,
                    session,
                    BuiltinRegistry::new(),
                    Arc::new(Config::default()),
                );
            }
        });

        // Scheduler should have received a TaskRequestFork message.
        let (task_id, msg) = control_receiver.recv().unwrap();
        assert_eq!(task_id, 1);
        let TaskControlMsg::TaskRequestFork(fork_request, reply_channel) = msg else {
            panic!("Expected TaskRequestFork, got different message type");
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
        let TaskControlMsg::TaskSuccess(result, _mutations, _timestamp) = msg else {
            panic!("Expected TaskSuccess, got different message type");
        };
        assert_eq!(result, v_int(123));
    }

    /// Verifies path through the command parser, and no match on verb
    #[test]
    fn test_command_no_match() {
        let (_kill_switch, mut task, _db, tx, task_scheduler_client, control_receiver) =
            setup_test_env_command("look here", &[]);

        let session = Arc::new(NoopClientSession::new());
        {
            let _tx_guard = setup_task_context(tx);
            task.setup_task_start(task_scheduler_client.control_sender());
            Task::run_task_loop(
                task,
                &task_scheduler_client,
                session,
                BuiltinRegistry::new(),
                Arc::new(Config::default()),
            );
        }

        // Scheduler should have received a NoCommandMatch
        let (task_id, msg) = control_receiver.recv().unwrap();
        assert_eq!(task_id, 1);
        let TaskControlMsg::TaskCommandError(CommandError::NoCommandMatch) = msg else {
            panic!("Expected NoCommandMatch, got different message type");
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
        let (_kill_switch, mut task, _db, tx, task_scheduler_client, control_receiver) =
            setup_test_env_command("look #0", &[look_this]);

        let session = Arc::new(NoopClientSession::new());
        {
            let _tx_guard = setup_task_context(tx);
            task.setup_task_start(task_scheduler_client.control_sender());
            Task::run_task_loop(
                task,
                &task_scheduler_client,
                session,
                BuiltinRegistry::new(),
                Arc::new(Config::default()),
            );
        }

        // This should be a success, it got handled
        let (task_id, msg) = control_receiver.recv().unwrap();
        assert_eq!(task_id, 1);
        let TaskControlMsg::TaskSuccess(result, _mutations, _timestamp) = msg else {
            panic!("Expected TaskSuccess, got different message type");
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

        let (_kill_switch, mut task, _db, tx, task_scheduler_client, control_receiver) =
            setup_test_env_command("look here", &[do_command_verb]);

        let session = Arc::new(NoopClientSession::new());
        {
            let _tx_guard = setup_task_context(tx);
            task.setup_task_start(task_scheduler_client.control_sender());
            Task::run_task_loop(
                task,
                &task_scheduler_client,
                session,
                BuiltinRegistry::new(),
                Arc::new(Config::default()),
            );
        }

        // This should be a success, it got handled
        let (task_id, msg) = control_receiver.recv().unwrap();
        assert_eq!(task_id, 1);
        let TaskControlMsg::TaskSuccess(result, _mutations, _timestamp) = msg else {
            panic!("Expected TaskSuccess, got different message type");
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

        let (_kill_switch, mut task, _db, tx, task_scheduler_client, control_receiver) =
            setup_test_env_command("look here", &[do_command_verb]);

        let session = Arc::new(NoopClientSession::new());
        {
            let _tx_guard = setup_task_context(tx);
            task.setup_task_start(task_scheduler_client.control_sender());
            Task::run_task_loop(
                task,
                &task_scheduler_client,
                session,
                BuiltinRegistry::new(),
                Arc::new(Config::default()),
            );
        }

        // This should be a success, it got handled
        let (task_id, msg) = control_receiver.recv().unwrap();
        assert_eq!(task_id, 1);
        let TaskControlMsg::TaskCommandError(CommandError::NoCommandMatch) = msg else {
            panic!("Expected NoCommandMatch, got different message type");
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
        let (_kill_switch, mut task, _db, tx, task_scheduler_client, control_receiver) =
            setup_test_env_command("look #0", &[do_command_verb, look_this]);

        let session = Arc::new(NoopClientSession::new());
        {
            let _tx_guard = setup_task_context(tx);
            task.setup_task_start(task_scheduler_client.control_sender());
            Task::run_task_loop(
                task,
                &task_scheduler_client,
                session,
                BuiltinRegistry::new(),
                Arc::new(Config::default()),
            );
        }

        // This should be a success, it got handled
        let (task_id, msg) = control_receiver.recv().unwrap();
        assert_eq!(task_id, 1);
        let TaskControlMsg::TaskSuccess(result, _mutations, _timestamp) = msg else {
            panic!("Expected TaskSuccess, got different message type");
        };
        assert_eq!(result, v_int(1));
    }
}
