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

//! Executable task state and the worker execution loop.
//!
//! A task owns its VM, retry snapshot, authority, and program cache. The worker supplies its
//! transaction and session through task context. The scheduler retains active metadata separately.
//!
//! Startup resolves commands and verbs in `start`. The loop calls `Task::vm_dispatch` in `dispatch`
//! for each VM response. `transaction` owns commit arbitration and retry snapshots. The loop
//! consumes decisions from `outcome` to transfer execution or finalize it through the scheduler.
//! Cache lifetime remains tied to this executable task and its retry snapshot.

mod dispatch;
mod outcome;
mod start;
mod transaction;

use outcome::ExecutionOutcome;

use crate::{
    config::Config,
    task_context::with_current_transaction,
    tasks::{
        ServerOptions, TaskStart,
        task_control::{CommittedBoundary, TaskControl},
        task_program_cache::TaskProgramCache,
        task_scheduler_client::TaskSchedulerClient,
    },
    trace_task_start,
    vm::{builtins::BuiltinRegistry, vm_host::VmHost},
};
#[cfg(feature = "trace_events")]
use crate::{
    trace_task_create_command, trace_task_create_eval, trace_task_create_exception_handler,
    trace_task_create_fork, trace_task_create_verb,
};
use ahash::AHasher;
use moor_common::{
    model::{ObjFlag, TaskPermissions},
    tasks::{Exception, Session, TaskId},
    util::{BitEnum, Instant},
};
use moor_compiler::to_literal;
#[cfg(feature = "trace_events")]
use moor_var::v_str;
use moor_var::{Error, ErrorCode, Obj};
use moor_vm::{ExecState, Frame};
use std::{collections::HashSet, sync::Arc, time::Duration};
use tracing::error;

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
    player: Obj,
    /// The object on behalf of which task permissions are evaluated.
    authority_principal: Obj,
    /// Cached flags for the authority principal.
    authority_principal_flags: BitEnum<ObjFlag>,
    /// The actual VM host which is managing the execution of this task.
    pub(crate) vm_host: VmHost,
    /// Arbitration between cancellation and transaction commit.
    pub(crate) control: Arc<TaskControl>,
    /// Committed boundary carried only between an owned outcome and scheduler handoff.
    /// This capability is never serialized or restored.
    committed_boundary: Option<CommittedBoundary>,
    /// The number of retries this process has undergone.
    pub(crate) retries: u8,
    /// A copy of the VM state at the time the task was created or last committed/suspended.
    /// For restoring on retry.
    pub(crate) retry_state: ExecState,
    /// True if we're currently handling an uncaught error to prevent infinite recursion.
    pub(crate) handling_uncaught_error: bool,
    /// The original exception when calling handle_uncaught_error, in case it returns false.
    pub(crate) pending_exception: Option<Exception>,
    /// Program storage for this task's frames, with transaction-scoped executable lookups.
    pub(crate) program_cache: TaskProgramCache,
}

impl Task {
    // Yes yes I know it's a lot of arguments, but wrapper object here is redundant.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        task_id: TaskId,
        player: Obj,
        authority_principal: Obj,
        task_start: TaskStart,
        server_options: &ServerOptions,
        control: Arc<TaskControl>,
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
                TaskStart::StartScheduled { verb, .. } => {
                    trace_task_create_verb!(
                        task_id,
                        &player,
                        &verb.as_string(),
                        &v_str("scheduled")
                    );
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
            authority_principal,
            authority_principal_flags: BitEnum::new(),
            control,
            committed_boundary: None,
            retries: 0,
            retry_state,
            handling_uncaught_error: false,
            pending_exception: None,
            program_cache: TaskProgramCache::default(),
        })
    }

    /// Carry the existing commit proof through the public Box<Task> handoff interface.
    pub(crate) fn with_committed_boundary(
        mut self: Box<Self>,
        boundary: CommittedBoundary,
    ) -> Box<Self> {
        assert!(
            self.committed_boundary.is_none(),
            "task already owns a boundary"
        );
        assert!(
            boundary.belongs_to(&self.control),
            "boundary belongs to another dispatch"
        );
        self.committed_boundary = Some(boundary);
        self
    }

    pub(crate) fn take_committed_boundary(&mut self) -> Option<CommittedBoundary> {
        self.committed_boundary.take()
    }

    #[inline]
    pub(crate) fn player(&self) -> Obj {
        self.player
    }

    #[inline]
    pub(crate) fn authority_principal(&self) -> Obj {
        self.authority_principal
    }

    #[inline]
    fn task_permissions(&self) -> TaskPermissions {
        TaskPermissions::new(self.authority_principal, self.authority_principal_flags)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_restored(
        task_id: TaskId,
        creation_time: Instant,
        player: Obj,
        state: TaskState,
        vm_host: VmHost,
        authority_principal: Obj,
        authority_principal_flags: BitEnum<ObjFlag>,
        control: Arc<TaskControl>,
        retries: u8,
        retry_state: ExecState,
        handling_uncaught_error: bool,
        pending_exception: Option<Exception>,
        program_cache: TaskProgramCache,
    ) -> Self {
        Self {
            task_id,
            creation_time,
            player,
            state,
            vm_host,
            authority_principal,
            authority_principal_flags,
            control,
            committed_boundary: None,
            retries,
            retry_state,
            handling_uncaught_error,
            pending_exception,
            program_cache,
        }
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
            let outcome = if task.control.is_cancelled() {
                task.cancelled_outcome()
            } else {
                task.vm_dispatch(
                    task_scheduler_client,
                    session.as_ref(),
                    &builtin_registry,
                    config.features.as_ref(),
                )
            };
            match outcome {
                ExecutionOutcome::Continue(continuation) => task = continuation,
                ExecutionOutcome::Retry(request) => {
                    request.handoff(task_scheduler_client);
                    break;
                }
                ExecutionOutcome::Suspend(request) => {
                    request.handoff(task_scheduler_client);
                    break;
                }
                ExecutionOutcome::Finish(request) => {
                    request.finish(task_scheduler_client);
                    break;
                }
            }
        }

        // Transaction is automatically cleaned up by _tx_guard drop
    }

    fn sync_authority_from_vm(&mut self) {
        let authority_principal = self.vm_host.vm_exec_state().task_authority_principal();
        if !authority_principal.is_nothing() {
            self.authority_principal = authority_principal;
            self.refresh_authority_principal_flags();
        }
    }

    fn refresh_authority_principal_flags(&mut self) {
        self.authority_principal_flags =
            with_current_transaction(|ws| ws.flags_of(&self.authority_principal))
                .unwrap_or_default();
    }

    fn collect_live_program_ptrs_from_state(
        state: &ExecState,
        live_ptrs: &mut HashSet<usize, std::hash::BuildHasherDefault<AHasher>>,
    ) {
        for activation in &state.stack {
            let Frame::Moo(frame) = &activation.frame else {
                continue;
            };
            if let Some(ptr) = frame.cached_program_ptr() {
                live_ptrs.insert(ptr.addr());
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
            && let Some(ptr) = frame.cached_program_ptr()
        {
            live_ptrs.insert(ptr.addr());
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

        let stack = ExecState::make_stack_list(&vm_state.stack);
        let panic_error = Error::new(ErrorCode::E_MAXREC, Some("Task panicked".to_string()), None);
        let backtrace = ExecState::make_backtrace(&vm_state.stack, &panic_error);
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
            .args()
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

// Tests use the real Scheduler with TxDB — tasks are submitted via SchedulerClient
// and results are observed through TaskHandle receivers.
#[cfg(test)]
mod tests;
