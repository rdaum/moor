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
    collections::HashSet,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

use crate::task_context::{
    commit_current_transaction, rollback_current_transaction, with_current_transaction,
    with_current_transaction_mut, with_new_transaction,
};
use ahash::AHasher;

use moor_compiler::to_literal;
use std::sync::LazyLock;
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
    model::{
        CommitResult, DispatchFlagsSource, ObjFlag, ResolvedVerb, VerbDispatch, VerbLookup,
        WorldState, WorldStateError, command_verb_argspec,
    },
    tasks::{CommandError, CommandError::PermissionDenied, Exception, TaskId},
    util::{BitEnum, Instant, PerfTimerGuard, parse_into_words},
};
use moor_var::{
    Error, ErrorCode, List, NOTHING, Obj, SYSTEM_OBJECT, Symbol, Variant, v_empty_str, v_err,
    v_int, v_obj, v_str, v_string,
};

use crate::{
    config::{Config, FeaturesConfig},
    tasks::{
        ServerOptions, TaskStart, sched_counters,
        task_program_cache::TaskProgramCache,
        task_scheduler_client::{TaskSchedulerClient, TimeoutHandlerInfo},
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
use moor_vm::Frame;

static HUH_SYM: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("huh"));
static HANDLE_UNCAUGHT_ERROR_SYM: LazyLock<Symbol> =
    LazyLock::new(|| Symbol::mk("handle_uncaught_error"));
static DO_COMMAND_SYM: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("do_command"));

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
    pub creation_time: Instant,
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
    /// Transaction-lifetime verb program cache for this task.
    pub(crate) program_cache: TaskProgramCache,
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
            Duration::from_secs_f64(max_seconds),
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
                TaskStart::StartBatchWorldState { .. } => {
                    // No specific trace event for batch world state tasks yet
                }
            }
        }

        let creation_time = Instant::now();
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
            program_cache: TaskProgramCache::default(),
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

    #[inline]
    fn refresh_retry_state(&mut self) {
        let snapshot = self.vm_host.snapshot_state();
        self.vm_host.restore_state(&snapshot);
        self.retry_state = snapshot;
    }

    fn collect_live_program_ptrs_from_state(
        state: &VMExecState,
        live_ptrs: &mut HashSet<usize, std::hash::BuildHasherDefault<AHasher>>,
    ) {
        for activation in &state.stack {
            let Frame::Moo(frame) = &activation.frame else {
                continue;
            };
            if let Some(ptr) = frame.program_ptr {
                live_ptrs.insert(ptr);
            }
        }
    }

    pub(crate) fn reclaim_program_cache(&mut self) {
        let mut live_ptrs =
            HashSet::with_hasher(std::hash::BuildHasherDefault::<AHasher>::default());

        Self::collect_live_program_ptrs_from_state(self.vm_host.vm_exec_state(), &mut live_ptrs);
        Self::collect_live_program_ptrs_from_state(&self.retry_state, &mut live_ptrs);

        if let TaskStart::StartFork { fork_request, .. } = self.state.task_start()
            && let Frame::Moo(frame) = &fork_request.activation.frame
            && let Some(ptr) = frame.program_ptr
        {
            live_ptrs.insert(ptr);
        }

        let reclaimed = self.program_cache.reclaim_unreferenced(&live_ptrs);
        if reclaimed > 0 {
            let reclaimed_i = reclaimed as i64;
            self.vm_host
                .vm_exec_state_mut()
                .program_cache_stats
                .reclaimed += reclaimed_i;
            self.retry_state.program_cache_stats.reclaimed += reclaimed_i;
        }

        let total_slots = self.program_cache.total_slot_count();
        let live_slots = self.program_cache.live_slot_count();
        let key_count = self.program_cache.key_count();
        self.vm_host
            .set_program_cache_sizes(total_slots, live_slots, key_count);
        self.retry_state.program_cache_total_slots = total_slots;
        self.retry_state.program_cache_live_slots = live_slots;
        self.retry_state.program_cache_key_count = key_count;
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
        let vm_exec_result = self.vm_host.exec_interpreter(
            self.task_id,
            session,
            builtin_registry,
            config,
            &mut self.program_cache,
        );
        self.vm_host.set_program_cache_sizes(
            self.program_cache.total_slot_count(),
            self.program_cache.live_slot_count(),
            self.program_cache.key_count(),
        );

        // Having done that, what should we now do?
        match vm_exec_result {
            VMHostResponse::DispatchFork(fork_request) => {
                // Commit current transaction, dispatch fork, then resume in a new transaction.
                let task_id_var = fork_request.task_id;
                let fork_request = fork_request;

                match with_new_transaction(|| {
                    let new_world_state =
                        task_scheduler_client.begin_new_transaction().map_err(|e| {
                            WorldStateError::DatabaseError(format!("Scheduler error: {e:?}"))
                        })?;
                    let task_id = task_scheduler_client.request_fork(fork_request);
                    Ok((new_world_state, task_id))
                }) {
                    Ok((CommitResult::Success { .. }, Some(task_id))) => {
                        if let Some(task_id_var) = task_id_var {
                            self.vm_host
                                .set_variable(&task_id_var, v_int(task_id as i64));
                        }
                        Some(self)
                    }
                    Ok((CommitResult::ConflictRetry { .. }, _)) => {
                        warn!("Conflict during commit before fork dispatch");
                        session.rollback().unwrap();
                        task_scheduler_client.conflict_retry(self);
                        None
                    }
                    Ok((CommitResult::Success { .. }, None)) => {
                        error!("Fork dispatch did not return a new task id");
                        session.rollback().unwrap();
                        task_scheduler_client.conflict_retry(self);
                        None
                    }
                    Err(e) => {
                        error!("Failed to commit before fork dispatch: {:?}", e);
                        session.rollback().unwrap();
                        task_scheduler_client.conflict_retry(self);
                        None
                    }
                }
            }
            VMHostResponse::Suspend(delay) => {
                // Fast path for RecvMessages(None): commit, drain messages, resume immediately
                if matches!(delay.as_ref(), TaskSuspend::RecvMessages(None)) {
                    let perfc = sched_counters();
                    let _t = PerfTimerGuard::new(&perfc.task_recv_immediate_resume_latency);
                    match with_new_transaction(|| {
                        let new_world_state =
                            task_scheduler_client.begin_new_transaction().map_err(|e| {
                                WorldStateError::DatabaseError(format!("Scheduler error: {e:?}"))
                            })?;
                        Ok((new_world_state, ()))
                    }) {
                        Ok((CommitResult::Success { .. }, _)) => {
                            let messages = task_scheduler_client.task_recv();
                            let resume_value = List::from_iter(messages).into();
                            self.vm_host.resume_execution(resume_value);
                            self.refresh_retry_state();
                            return Some(self);
                        }
                        Ok((CommitResult::ConflictRetry { .. }, _)) => {
                            warn!("Conflict during task_recv immediate resume");
                            session.rollback().unwrap();
                            task_scheduler_client.conflict_retry(self);
                            return None;
                        }
                        Err(e) => {
                            error!("Failed to begin new transaction for task_recv: {:?}", e);
                            // Fall back to normal suspend path
                        }
                    }
                }

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
                            // Resume first (which resets start_time), then snapshot
                            // so retry_state has fresh timing if we need to restore
                            self.vm_host.resume_execution(resume_value);
                            self.refresh_retry_state();
                            return Some(self);
                        }
                        Ok((CommitResult::ConflictRetry { .. }, _)) => {
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

                if let CommitResult::ConflictRetry { .. } = commit_result {
                    warn!("Conflict during commit before suspend");
                    session.rollback().unwrap();
                    task_scheduler_client.conflict_retry(self);
                    return None;
                }

                self.refresh_retry_state();
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

                if let CommitResult::ConflictRetry { .. } = commit_result {
                    warn!("Conflict during commit before suspend");
                    session.rollback().unwrap();
                    task_scheduler_client.conflict_retry(self);
                    return None;
                }

                self.refresh_retry_state();
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

                    // Backoff is handled by the scheduler via suspension-based retry
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
                        world_state.dispatch_verb(
                            &self.perms,
                            VerbDispatch::new(
                                VerbLookup::method(&SYSTEM_OBJECT, *HANDLE_UNCAUGHT_ERROR_SYM),
                                DispatchFlagsSource::Permissions,
                            ),
                        )
                    });

                    if let Ok(Some(verb_result)) = verb_lookup {
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
                            verb_result.verbdef,
                            *HANDLE_UNCAUGHT_ERROR_SYM,
                            v_obj(SYSTEM_OBJECT),
                            self.player,
                            args,
                            v_obj(self.player),
                            v_empty_str(),
                            verb_result.permissions_flags,
                            match with_current_transaction(|ws| {
                                ws.retrieve_verb(
                                    &self.perms,
                                    &verb_result.program_key.verb_definer,
                                    verb_result.program_key.verb_uuid,
                                )
                            }) {
                                Ok((program, _)) => program,
                                Err(e) => {
                                    error!(
                                        task_id = ?self.task_id,
                                        "Error resolving handler program: {e:?}"
                                    );
                                    return None;
                                }
                            },
                        );

                        // Continue execution - the handler will now run
                        return Some(self);
                    }

                    // No handler exists or error looking it up
                    if let Err(e) = verb_lookup {
                        error!(task_id = ?self.task_id, "Error looking up handle_uncaught_error: {:?}", e);
                        // Proceed with normal exception reporting
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
    pub(crate) fn setup_task_start(&mut self, tsc: &TaskSchedulerClient, config: &Config) -> bool {
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
                    tsc.command_error(e);
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
                        tsc.verb_not_found(this, verb_name);
                        return false;
                    }
                };
                match with_current_transaction(|world_state| {
                    world_state.dispatch_verb(
                        &self.perms,
                        VerbDispatch::new(
                            VerbLookup::method(&object_location, verb_name),
                            DispatchFlagsSource::Permissions,
                        ),
                    )
                }) {
                    Ok(None) => {
                        tsc.verb_not_found(this, verb_name);
                        return false;
                    }
                    Err(WorldStateError::VerbNotFound(_, _)) => {
                        panic!("dispatch_verb() should return Ok(None), not VerbNotFound");
                    }
                    Err(e) => {
                        error!(task_id = ?self.task_id, this = ?this,
                               verb = ?verb_name,
                               "World state error while resolving verb: {:?}", e);
                        panic!("Could not resolve verb: {e:?}");
                    }
                    Ok(Some(verb_result)) => {
                        self.vm_host.start_call_method_verb(
                            self.task_id,
                            verb_result.verbdef,
                            verb_name,
                            this,
                            player,
                            args_val,
                            caller,
                            argstr_val,
                            verb_result.permissions_flags,
                            match with_current_transaction(|ws| {
                                ws.retrieve_verb(
                                    &self.perms,
                                    &verb_result.program_key.verb_definer,
                                    verb_result.program_key.verb_uuid,
                                )
                            }) {
                                Ok((program, _)) => program,
                                Err(e) => {
                                    error!(
                                        task_id = ?self.task_id,
                                        "Error resolving startup verb program: {e:?}"
                                    );
                                    return false;
                                }
                            },
                        );
                    }
                }
            }
            TaskStart::StartFork {
                fork_request,
                suspended: _,
            } => {
                let mut prepared_fork = (**fork_request).clone();
                if let Frame::Moo(ref mut frame) = prepared_fork.activation.frame {
                    frame.materialize_program_for_handoff();
                }
                // When setup_task_start is called, the task is being woken/started, so we always
                // pass suspended=false to ensure vm_host.running is set to true
                self.vm_host.start_fork(self.task_id, &prepared_fork, false);
            }
            TaskStart::StartEval {
                player,
                program,
                initial_env,
            } => {
                self.vm_host.start_eval(
                    self.task_id,
                    player,
                    program.clone(),
                    initial_env.as_deref(),
                );
            }
            TaskStart::StartDoCommand { .. } => {
                panic!("StartDoCommand invocation should not happen on initial setup_task_start");
            }
            TaskStart::StartBatchWorldState {
                actions,
                rollback,
                result_sink,
                ..
            } => {
                let actions = actions.clone();
                let rollback = *rollback;
                let result_sink = result_sink.clone();

                // Execute the batch directly against the task's transaction.
                let batch_result = with_current_transaction_mut(|world_state| {
                    crate::tasks::world_state_executor::execute_world_state_actions(
                        world_state,
                        config,
                        actions,
                    )
                });

                // Store the result in the shared sink for the caller to retrieve.
                *result_sink.lock().unwrap() = Some(batch_result.clone());

                // Handle commit/rollback and notify the scheduler.
                match batch_result {
                    Ok(_) => {
                        if rollback {
                            let _ = rollback_current_transaction();
                        } else {
                            match commit_current_transaction() {
                                Ok(CommitResult::Success { .. }) => {}
                                Ok(CommitResult::ConflictRetry { conflict_info }) => {
                                    let msg = match conflict_info {
                                        Some(info) => format!("Transaction conflict: {info}"),
                                        None => "Transaction conflict".to_string(),
                                    };
                                    *result_sink.lock().unwrap() = Some(Err(
                                        moor_common::tasks::SchedulerError::CommandExecutionError(
                                            CommandError::DatabaseError(
                                                moor_common::model::WorldStateError::DatabaseError(
                                                    msg,
                                                ),
                                            ),
                                        ),
                                    ));
                                    tsc.command_error(CommandError::DatabaseError(
                                        moor_common::model::WorldStateError::DatabaseError(
                                            "Transaction conflict".to_string(),
                                        ),
                                    ));
                                    return false;
                                }
                                Err(e) => {
                                    *result_sink.lock().unwrap() = Some(Err(
                                        moor_common::tasks::SchedulerError::CommandExecutionError(
                                            CommandError::DatabaseError(e),
                                        ),
                                    ));
                                    tsc.command_error(CommandError::DatabaseError(
                                        moor_common::model::WorldStateError::DatabaseError(
                                            "Commit failed".to_string(),
                                        ),
                                    ));
                                    return false;
                                }
                            }
                        }
                        tsc.success(v_int(0), !rollback, 0);
                        return false; // No VM loop needed
                    }
                    Err(ref e) => {
                        tsc.command_error(CommandError::DatabaseError(
                            moor_common::model::WorldStateError::DatabaseError(e.to_string()),
                        ));
                        return false;
                    }
                }
            }
            TaskStart::StartExceptionHandler { player, args, .. } => {
                // Start $handle_uncaught_error on the system object with the exception args
                // Find and set up the handler verb
                match with_current_transaction(|world_state| {
                    world_state.dispatch_verb(
                        &self.perms,
                        VerbDispatch::new(
                            VerbLookup::method(&SYSTEM_OBJECT, *HANDLE_UNCAUGHT_ERROR_SYM),
                            DispatchFlagsSource::Permissions,
                        ),
                    )
                }) {
                    Ok(None) => {
                        warn!("handle_uncaught_error verb not found during setup");
                        return false;
                    }
                    Err(e) => {
                        error!(task_id = ?self.task_id, "Error resolving handle_uncaught_error: {e:?}");
                        return false;
                    }
                    Ok(Some(verb_result)) => {
                        self.vm_host.start_call_method_verb(
                            self.task_id,
                            verb_result.verbdef,
                            *HANDLE_UNCAUGHT_ERROR_SYM,
                            v_obj(SYSTEM_OBJECT),
                            *player,
                            args.clone(),
                            v_obj(*player),
                            v_empty_str(),
                            verb_result.permissions_flags,
                            match with_current_transaction(|ws| {
                                ws.retrieve_verb(
                                    &self.perms,
                                    &verb_result.program_key.verb_definer,
                                    verb_result.program_key.verb_uuid,
                                )
                            }) {
                                Ok((program, _)) => program,
                                Err(e) => {
                                    error!(
                                        task_id = ?self.task_id,
                                        "Error resolving exception-handler program: {e:?}"
                                    );
                                    return false;
                                }
                            },
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
        let do_command = world_state.dispatch_verb(
            &self.perms,
            VerbDispatch::new(
                VerbLookup::method(&SYSTEM_OBJECT, *DO_COMMAND_SYM),
                DispatchFlagsSource::Permissions,
            ),
        );

        match do_command {
            Ok(None) => {
                self.setup_start_parse_command(player, command, world_state)?;
            }
            Ok(Some(verb_result)) => {
                let arguments = parse_into_words(command);
                let args = List::from_iter(arguments.iter().map(|s| v_str(s)));
                self.vm_host.start_call_method_verb(
                    self.task_id,
                    verb_result.verbdef,
                    *DO_COMMAND_SYM,
                    v_obj(*handler_object),
                    *player,
                    args,
                    v_obj(*handler_object),
                    v_str(command),
                    verb_result.permissions_flags,
                    world_state
                        .retrieve_verb(
                            &self.perms,
                            &verb_result.program_key.verb_definer,
                            verb_result.program_key.verb_uuid,
                        )
                        .map_err(CommandError::DatabaseError)?
                        .0,
                );
                self.state = TaskState::Prepared(TaskStart::StartDoCommand {
                    handler_object: *handler_object,
                    player: *player,
                    command: command.to_string(),
                });
            }
            Err(WorldStateError::VerbNotFound(_, _)) => {
                panic!("dispatch_verb() should return Ok(None), not VerbNotFound");
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
                fuzzy_threshold: 0.5,
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
        let ((program, verbdef, permissions_flags), target) = match parse_results {
            // If we have a successful match, that's what we'll call into
            Some((verb_info, target)) => (verb_info, target),
            // Otherwise, we want to try to call :huh, if it exists.
            None => {
                if player_location == NOTHING {
                    return Err(CommandError::NoCommandMatch);
                }
                // Try to find :huh. If it exists, we'll dispatch to that, instead.
                // If we don't find it, that's the end of the line.
                let Ok(Some(verb_result)) = world_state.dispatch_verb(
                    &self.perms,
                    VerbDispatch::new(
                        VerbLookup::method(&player_location, *HUH_SYM),
                        DispatchFlagsSource::VerbOwner,
                    ),
                ) else {
                    return Err(CommandError::NoCommandMatch);
                };
                (
                    (
                        world_state
                            .retrieve_verb(
                                player,
                                &verb_result.program_key.verb_definer,
                                verb_result.program_key.verb_uuid,
                            )
                            .map_err(CommandError::DatabaseError)?
                            .0,
                        verb_result.verbdef,
                        verb_result.permissions_flags,
                    ),
                    player_location,
                )
            }
        };
        self.vm_host.start_call_command_verb(
            self.task_id,
            verbdef,
            parsed_command.verb,
            v_obj(target),
            *player,
            v_obj(*player),
            parsed_command,
            permissions_flags,
            program,
        );
        Ok(())
    }
}

impl Drop for Task {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            return;
        }

        let task_start = self.state.task_start().diagnostic();
        let vm_state = self.vm_host.vm_exec_state();
        let Some(activation) = vm_state.try_top() else {
            error!(
                task_id = self.task_id,
                player = %self.player,
                task_start = %task_start,
                "Task panicked with empty activation stack"
            );
            return;
        };

        let stack = VMExecState::make_stack_list(&vm_state.stack);
        let panic_error = Error::new(ErrorCode::E_MAXREC, Some("Task panicked".to_string()), None);
        let backtrace = VMExecState::make_backtrace(&vm_state.stack, &panic_error);
        let stack_literals = stack.iter().map(to_literal).collect::<Vec<_>>();
        let backtrace_lines = backtrace
            .iter()
            .map(|entry| {
                entry
                    .as_string()
                    .map(str::to_string)
                    .unwrap_or_else(|| to_literal(entry))
            })
            .collect::<Vec<_>>();
        let args = activation
            .args
            .iter()
            .map(|arg| to_literal(&arg))
            .collect::<Vec<_>>();
        let this_literal = to_literal(&activation.this);
        let line_number = activation.frame.find_line_no();
        let definer = activation.verb_definer();

        error!(
            task_id = self.task_id,
            player = %self.player,
            task_start = %task_start,
            this = %this_literal,
            verb = %activation.verb_name,
            definer = %definer,
            line_number = ?line_number,
            args = ?args,
            stack = ?stack_literals,
            backtrace = ?backtrace_lines,
            "Task panicked at top activation"
        );
    }
}

#[allow(clippy::type_complexity)]
fn find_verb_for_command(
    player: &Obj,
    player_location: &Obj,
    pc: &ParsedCommand,
    ws: &mut dyn WorldState,
) -> Result<Option<((ProgramType, ResolvedVerb, BitEnum<ObjFlag>), Obj)>, CommandError> {
    let perfc = sched_counters();
    let _t = PerfTimerGuard::new(&perfc.find_verb_for_command);
    let targets_to_search = vec![
        *player,
        *player_location,
        pc.dobj.unwrap_or(NOTHING),
        pc.iobj.unwrap_or(NOTHING),
    ];
    let dobj = pc.dobj.unwrap_or(NOTHING);
    let iobj = pc.iobj.unwrap_or(NOTHING);
    for target in targets_to_search {
        let argspec = command_verb_argspec(&target, &dobj, pc.prep, &iobj);
        let match_result = ws.dispatch_verb(
            player,
            VerbDispatch::new(
                VerbLookup::command(&target, pc.verb, argspec),
                DispatchFlagsSource::VerbOwner,
            ),
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
        if let Some(verb_result) = match_result {
            return Ok(Some((
                (
                    ws.retrieve_verb(
                        player,
                        &verb_result.program_key.verb_definer,
                        verb_result.program_key.verb_uuid,
                    )
                    .map_err(CommandError::DatabaseError)?
                    .0,
                    verb_result.verbdef,
                    verb_result.permissions_flags,
                ),
                target,
            )));
        }
    }
    Ok(None)
}

// Tests use the real Scheduler with TxDB — tasks are submitted via SchedulerClient
// and results are observed through TaskHandle receivers.
#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use moor_common::{
        model::{ArgSpec, ObjFlag, ObjectKind, PrepSpec, VerbArgsSpec, VerbFlag, WorldStateSource},
        tasks::{
            CommandError, NoopClientSession, NoopSystemControl, SchedulerError, SessionError,
            SessionFactory,
        },
        util::BitEnum,
    };
    use moor_compiler::{CompileOptions, Program, compile};
    use moor_db::{DatabaseConfig, TxDB};
    use moor_var::{
        E_DIV, NOTHING, Obj, SYSTEM_OBJECT, Symbol, program::ProgramType, v_int, v_str,
    };

    use crate::{
        config::{Config, FeaturesConfig},
        tasks::{
            NoopTasksDb, TaskHandle, TaskNotification, scheduler::Scheduler,
            scheduler_client::SchedulerClient,
        },
    };

    struct TestVerb {
        name: Symbol,
        program: Program,
        argspec: VerbArgsSpec,
    }

    struct NoopSessionFactory;
    impl SessionFactory for NoopSessionFactory {
        fn mk_background_session(
            self: Arc<Self>,
            _player: &Obj,
        ) -> Result<Arc<dyn moor_common::tasks::Session>, SessionError> {
            Ok(Arc::new(NoopClientSession::new()))
        }
    }

    /// Create a TxDB, populate it with a system object (wizard/programmer),
    /// optionally add verbs, commit, then create a Scheduler + SchedulerClient.
    fn setup_scheduler(verbs: &[TestVerb]) -> (SchedulerClient, Scheduler) {
        let (db, _) = TxDB::open(None, DatabaseConfig::default());
        let mut tx = db.new_world_state().unwrap();

        let sysobj = tx
            .create_object(
                &SYSTEM_OBJECT,
                &NOTHING,
                &SYSTEM_OBJECT,
                ObjFlag::all_flags(),
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
        } in verbs
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
        tx.commit().unwrap();

        let scheduler = Scheduler::new(
            semver::Version::new(0, 0, 0),
            Box::new(db),
            Box::new(NoopTasksDb {}),
            Arc::new(Config::default()),
            Arc::new(NoopSystemControl::default()),
            None,
            None,
        );
        let _timer_jh = scheduler.start(Arc::new(NoopSessionFactory));
        let client = scheduler.client().unwrap();
        (client, scheduler)
    }

    /// Wait for a task result, handling suspended notifications.
    fn wait_result(handle: &TaskHandle) -> Result<moor_var::Var, SchedulerError> {
        loop {
            match handle
                .receiver()
                .recv_timeout(Duration::from_secs(5))
                .expect("Task result timed out")
            {
                (_, Ok(TaskNotification::Result(v))) => return Ok(v),
                (_, Ok(TaskNotification::Suspended)) => continue,
                (_, Err(e)) => return Err(e),
            }
        }
    }

    /// Test that we can start a task and run it to completion.
    #[test]
    fn test_simple_run_return() {
        let (client, _sched) = setup_scheduler(&[]);
        let session = Arc::new(NoopClientSession::new());
        let handle = client
            .submit_eval_task(
                &SYSTEM_OBJECT,
                &SYSTEM_OBJECT,
                "return 1 + 1;".to_string(),
                None,
                session,
                Arc::new(FeaturesConfig::default()),
            )
            .unwrap();
        let result = wait_result(&handle).unwrap();
        assert_eq!(result, v_int(2));
    }

    /// Trigger a MOO VM exception
    #[test]
    fn test_simple_run_exception() {
        let (client, _sched) = setup_scheduler(&[]);
        let session = Arc::new(NoopClientSession::new());
        let handle = client
            .submit_eval_task(
                &SYSTEM_OBJECT,
                &SYSTEM_OBJECT,
                "return 1 / 0;".to_string(),
                None,
                session,
                Arc::new(FeaturesConfig::default()),
            )
            .unwrap();
        let err = wait_result(&handle).unwrap_err();
        match err {
            SchedulerError::TaskAbortedException(ex) => {
                assert_eq!(ex.error.err_type, E_DIV);
            }
            other => panic!("Expected TaskAbortedException, got {other:?}"),
        }
    }

    /// notify() dispatches to the scheduler (no crash, returns successfully)
    #[test]
    fn test_notify_invocation() {
        let (client, _sched) = setup_scheduler(&[]);
        let session = Arc::new(NoopClientSession::new());
        let handle = client
            .submit_eval_task(
                &SYSTEM_OBJECT,
                &SYSTEM_OBJECT,
                r#"notify(#0, "12345"); return 123;"#.to_string(),
                None,
                session,
                Arc::new(FeaturesConfig::default()),
            )
            .unwrap();
        let result = wait_result(&handle).unwrap();
        assert_eq!(result, v_int(123));
    }

    /// Trigger a task-suspend-resume via suspend(0) (commit-and-continue)
    #[test]
    fn test_simple_run_suspend() {
        let (client, _sched) = setup_scheduler(&[]);
        let session = Arc::new(NoopClientSession::new());
        let handle = client
            .submit_eval_task(
                &SYSTEM_OBJECT,
                &SYSTEM_OBJECT,
                "suspend(0); return 123;".to_string(),
                None,
                session,
                Arc::new(FeaturesConfig::default()),
            )
            .unwrap();
        let result = wait_result(&handle).unwrap();
        assert_eq!(result, v_int(123));
    }

    /// Trigger a task-fork — fork spawns a child, parent returns its own value
    #[test]
    fn test_simple_run_fork() {
        let (client, _sched) = setup_scheduler(&[]);
        let session = Arc::new(NoopClientSession::new());
        let handle = client
            .submit_eval_task(
                &SYSTEM_OBJECT,
                &SYSTEM_OBJECT,
                "fork (0) endfork return 123;".to_string(),
                None,
                session,
                Arc::new(FeaturesConfig::default()),
            )
            .unwrap();
        let result = wait_result(&handle).unwrap();
        assert_eq!(result, v_int(123));
    }

    /// Verifies path through the command parser, and no match on verb
    #[test]
    fn test_command_no_match() {
        let (client, _sched) = setup_scheduler(&[]);
        let session = Arc::new(NoopClientSession::new());
        let handle = client
            .submit_command_task(&SYSTEM_OBJECT, &SYSTEM_OBJECT, "look here", session)
            .unwrap();
        let err = wait_result(&handle).unwrap_err();
        assert!(
            matches!(
                err,
                SchedulerError::CommandExecutionError(CommandError::NoCommandMatch)
            ),
            "Expected NoCommandMatch, got {err:?}"
        );
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
        let (client, _sched) = setup_scheduler(&[look_this]);
        let session = Arc::new(NoopClientSession::new());
        let handle = client
            .submit_command_task(&SYSTEM_OBJECT, &SYSTEM_OBJECT, "look #0", session)
            .unwrap();
        let result = wait_result(&handle).unwrap();
        assert_eq!(result, v_int(1));
    }

    /// Install "do_command" that returns true — command was handled.
    #[test]
    fn test_command_do_command() {
        let do_command_verb = TestVerb {
            name: Symbol::mk("do_command"),
            program: compile("return 1;", CompileOptions::default()).unwrap(),
            argspec: VerbArgsSpec::this_none_this(),
        };
        let (client, _sched) = setup_scheduler(&[do_command_verb]);
        let session = Arc::new(NoopClientSession::new());
        let handle = client
            .submit_command_task(&SYSTEM_OBJECT, &SYSTEM_OBJECT, "look here", session)
            .unwrap();
        let result = wait_result(&handle).unwrap();
        assert_eq!(result, v_int(1));
    }

    /// Install "do_command" that returns false — falls through to verb dispatch, no match.
    #[test]
    fn test_command_do_command_false_no_match() {
        let do_command_verb = TestVerb {
            name: Symbol::mk("do_command"),
            program: compile("return 0;", CompileOptions::default()).unwrap(),
            argspec: VerbArgsSpec::this_none_this(),
        };
        let (client, _sched) = setup_scheduler(&[do_command_verb]);
        let session = Arc::new(NoopClientSession::new());
        let handle = client
            .submit_command_task(&SYSTEM_OBJECT, &SYSTEM_OBJECT, "look here", session)
            .unwrap();
        let err = wait_result(&handle).unwrap_err();
        assert!(
            matches!(
                err,
                SchedulerError::CommandExecutionError(CommandError::NoCommandMatch)
            ),
            "Expected NoCommandMatch, got {err:?}"
        );
    }

    /// Install "do_command" that returns false + a matching verb — falls through and matches.
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
        let (client, _sched) = setup_scheduler(&[do_command_verb, look_this]);
        let session = Arc::new(NoopClientSession::new());
        let handle = client
            .submit_command_task(&SYSTEM_OBJECT, &SYSTEM_OBJECT, "look #0", session)
            .unwrap();
        let result = wait_result(&handle).unwrap();
        assert_eq!(result, v_int(1));
    }

    // =========================================================================
    // Batch World State Task Tests
    // =========================================================================

    #[test]
    fn test_batch_world_state_empty() {
        let (client, _sched) = setup_scheduler(&[]);
        let session = Arc::new(NoopClientSession::new());
        let (handle, result_sink) = client
            .submit_batch_world_state_task(&SYSTEM_OBJECT, &SYSTEM_OBJECT, vec![], false, session)
            .unwrap();
        let result = wait_result(&handle).unwrap();
        assert_eq!(result, v_int(0));

        let sink = result_sink.lock().unwrap();
        let results = sink.as_ref().unwrap().as_ref().unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_batch_world_state_read_property() {
        use crate::tasks::world_state_action::{WorldStateAction, WorldStateResult};
        use moor_common::model::ObjectRef;

        let actions = vec![WorldStateAction::RequestSystemProperty {
            player: SYSTEM_OBJECT,
            obj: ObjectRef::Id(SYSTEM_OBJECT),
            property: Symbol::mk("name"),
        }];

        let (client, _sched) = setup_scheduler(&[]);
        let session = Arc::new(NoopClientSession::new());
        let (handle, result_sink) = client
            .submit_batch_world_state_task(&SYSTEM_OBJECT, &SYSTEM_OBJECT, actions, false, session)
            .unwrap();
        wait_result(&handle).unwrap();

        let sink = result_sink.lock().unwrap();
        let results = sink.as_ref().unwrap().as_ref().unwrap();
        assert_eq!(results.len(), 1);
        match &results[0] {
            WorldStateResult::SystemProperty(v) => assert_eq!(*v, v_str("system")),
            other => panic!("Expected SystemProperty, got {other:?}"),
        }
    }

    #[test]
    fn test_batch_world_state_rollback() {
        use crate::tasks::world_state_action::{WorldStateAction, WorldStateResult};
        use moor_common::model::ObjectRef;

        let actions = vec![
            WorldStateAction::UpdateProperty {
                player: SYSTEM_OBJECT,
                perms: SYSTEM_OBJECT,
                obj: ObjectRef::Id(SYSTEM_OBJECT),
                property: Symbol::mk("name"),
                value: v_str("modified"),
            },
            WorldStateAction::RequestSystemProperty {
                player: SYSTEM_OBJECT,
                obj: ObjectRef::Id(SYSTEM_OBJECT),
                property: Symbol::mk("name"),
            },
        ];

        let (client, _sched) = setup_scheduler(&[]);
        let session = Arc::new(NoopClientSession::new());
        let (handle, result_sink) = client
            .submit_batch_world_state_task(&SYSTEM_OBJECT, &SYSTEM_OBJECT, actions, true, session)
            .unwrap();
        wait_result(&handle).unwrap();

        let sink = result_sink.lock().unwrap();
        let results = sink.as_ref().unwrap().as_ref().unwrap();
        assert_eq!(results.len(), 2);
        match &results[0] {
            WorldStateResult::PropertyUpdated => {}
            other => panic!("Expected PropertyUpdated, got {other:?}"),
        }
        match &results[1] {
            WorldStateResult::SystemProperty(v) => assert_eq!(*v, v_str("modified")),
            other => panic!("Expected SystemProperty, got {other:?}"),
        }
    }

    #[test]
    fn test_batch_world_state_multiple_reads() {
        use crate::tasks::world_state_action::{WorldStateAction, WorldStateResult};
        use moor_common::model::ObjectRef;

        let actions = vec![
            WorldStateAction::RequestSystemProperty {
                player: SYSTEM_OBJECT,
                obj: ObjectRef::Id(SYSTEM_OBJECT),
                property: Symbol::mk("name"),
            },
            WorldStateAction::GetObjectFlags { obj: SYSTEM_OBJECT },
            WorldStateAction::RequestAllObjects {
                player: SYSTEM_OBJECT,
            },
            WorldStateAction::ResolveObject {
                player: SYSTEM_OBJECT,
                obj: ObjectRef::Id(SYSTEM_OBJECT),
            },
        ];

        let (client, _sched) = setup_scheduler(&[]);
        let session = Arc::new(NoopClientSession::new());
        let (handle, result_sink) = client
            .submit_batch_world_state_task(&SYSTEM_OBJECT, &SYSTEM_OBJECT, actions, false, session)
            .unwrap();
        wait_result(&handle).unwrap();

        let sink = result_sink.lock().unwrap();
        let results = sink.as_ref().unwrap().as_ref().unwrap();
        assert_eq!(results.len(), 4);

        assert!(matches!(&results[0], WorldStateResult::SystemProperty(_)));
        assert!(matches!(&results[1], WorldStateResult::ObjectFlags(_)));
        assert!(matches!(&results[2], WorldStateResult::AllObjects(_)));
        assert!(matches!(&results[3], WorldStateResult::ResolvedObject(_)));
    }
}
