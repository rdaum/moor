// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

use std::{fmt::Debug, time::SystemTime};

use flume::Receiver;
use lazy_static::lazy_static;
use moor_compiler::{Program, to_literal};
use moor_var::{List, Obj, Symbol, Var};

pub use crate::tasks::tasks_db::{NoopTasksDb, TasksDb, TasksDbError};
use crate::vm::Fork;
use moor_common::{
    tasks::{Exception, SchedulerError, TaskId},
    util::PerfCounter,
};

pub mod scheduler;

pub(crate) mod checkpoint;
pub mod convert_task;
pub mod nursery;
pub(crate) mod gc_thread;
pub(crate) mod scheduler_client;
pub(crate) mod task;
pub(crate) mod task_q;
pub mod task_scheduler_client;
mod tasks_db;
pub mod workers;
pub(crate) mod world_state_action;
pub(crate) mod world_state_executor;

pub const DEFAULT_FG_TICKS: usize = 60_000;
pub const DEFAULT_BG_TICKS: usize = 30_000;
pub const DEFAULT_FG_SECONDS: u64 = 5;
pub const DEFAULT_BG_SECONDS: u64 = 3;
pub const DEFAULT_MAX_STACK_DEPTH: usize = 50;
pub const DEFAULT_GC_INTERVAL_SECONDS: u64 = 30;
pub const DEFAULT_MAX_TASK_RETRIES: u8 = 10;
pub const DEFAULT_MAX_TASK_MAILBOX: usize = 1000;
/// Interval for tasks DB compaction (independent of GC)
pub const DEFAULT_COMPACT_INTERVAL_SECONDS: u64 = 300;

lazy_static! {
    static ref SCHED_COUNTERS: SchedulerPerfCounters = SchedulerPerfCounters::new();
}

thread_local! {
    static SCHED_COUNTERS_TLS: &'static SchedulerPerfCounters = &SCHED_COUNTERS;
}

pub fn sched_counters() -> &'static SchedulerPerfCounters {
    SCHED_COUNTERS_TLS.with(|c| *c)
}

/// Just a handle to a task, with a receiver for the result.
pub struct TaskHandle(
    TaskId,
    Receiver<(TaskId, Result<TaskNotification, SchedulerError>)>,
);

// Results from a task which are either a value or a notification that the underlying task handle
// was replaced at the whim of the scheduler.
pub enum TaskNotification {
    /// Task is completed, and here are its results.
    Result(Var),
    /// Task has transitioned into a suspended/background state.
    Suspended,
}

impl Debug for TaskHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskHandle")
            .field("task_id", &self.0)
            .finish()
    }
}

impl TaskHandle {
    pub fn task_id(&self) -> TaskId {
        self.0
    }

    /// Dissolve the handle into a receiver for the result.
    pub fn into_receiver(self) -> Receiver<(TaskId, Result<TaskNotification, SchedulerError>)> {
        self.1
    }

    pub fn receiver(&self) -> &Receiver<(TaskId, Result<TaskNotification, SchedulerError>)> {
        &self.1
    }

    /// Create a new TaskHandle (for testing/mocking purposes)
    pub fn new_mock(
        task_id: TaskId,
        receiver: Receiver<(TaskId, Result<TaskNotification, SchedulerError>)>,
    ) -> Self {
        Self(task_id, receiver)
    }
}

/// External interface description of a task, for purpose of e.g. the queued_tasks() builtin.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TaskDescription {
    pub task_id: TaskId,
    pub start_time: Option<SystemTime>,
    pub permissions: Obj,
    pub verb_name: Symbol,
    pub verb_definer: Obj,
    pub line_number: usize,
    pub this: Var,
}

/// The set of options that can be configured for the server via core $server_options.
/// bf_load_server_options refreshes the server options from the database.
#[derive(Debug, Clone)]
pub struct ServerOptions {
    /// The number of seconds allotted to background tasks.
    pub bg_seconds: u64,
    /// The number of ticks allotted to background tasks.
    pub bg_ticks: usize,
    /// The number of seconds allotted to foreground tasks.
    pub fg_seconds: u64,
    /// The number of ticks allotted to foreground tasks.
    pub fg_ticks: usize,
    /// The maximum number of levels of nested verb calls.
    pub max_stack_depth: usize,
    /// The interval in seconds for automatic database checkpoints.
    pub dump_interval: Option<u64>,
    /// The interval in seconds for automatic garbage collection.
    pub gc_interval: Option<u64>,
    /// Maximum number of times a task can be retried on transaction conflict before aborting.
    pub max_task_retries: u8,
    /// Maximum number of messages allowed in a task's mailbox (for task_send/task_recv).
    pub max_task_mailbox: usize,
}

impl ServerOptions {
    pub fn max_vm_values(&self, is_background: bool) -> (u64, usize, usize) {
        if is_background {
            (self.bg_seconds, self.bg_ticks, self.max_stack_depth)
        } else {
            (self.fg_seconds, self.fg_ticks, self.max_stack_depth)
        }
    }
}

pub struct SchedulerPerfCounters {
    resume_task: PerfCounter,
    start_task: PerfCounter,
    retry_task: PerfCounter,
    kill_task: PerfCounter,
    pub setup_task: PerfCounter,
    start_command: PerfCounter,
    parse_command: PerfCounter,
    find_verb_for_command: PerfCounter,
    task_conflict_retry: PerfCounter,
    task_abort_cancelled: PerfCounter,
    task_abort_limits: PerfCounter,
    fork_task: PerfCounter,
    suspend_task: PerfCounter,
    task_exception: PerfCounter,
    pub handle_scheduler_msg: PerfCounter,
    handle_task_msg: PerfCounter,
    gc_mark_phase: PerfCounter,
    gc_sweep_phase: PerfCounter,

    // SchedulerClient latency counters (end-to-end from send to reply)
    pub submit_command_task_latency: PerfCounter,
    pub submit_verb_task_latency: PerfCounter,
    pub submit_eval_task_latency: PerfCounter,
    pub submit_oob_task_latency: PerfCounter,
    pub submit_system_handler_task_latency: PerfCounter,
    pub checkpoint_latency: PerfCounter,
    pub load_object_latency: PerfCounter,
    pub reload_object_latency: PerfCounter,

    // TaskSchedulerClient latency counters (from running tasks)
    pub task_request_fork_latency: PerfCounter,
    pub task_kill_task_latency: PerfCounter,
    pub task_resume_task_latency: PerfCounter,
    pub task_checkpoint_latency: PerfCounter,
    pub task_active_tasks_latency: PerfCounter,
    pub task_begin_transaction_latency: PerfCounter,

    // Task lifecycle latency counters
    pub task_wakeup_latency: PerfCounter,
}

impl Default for SchedulerPerfCounters {
    fn default() -> Self {
        Self::new()
    }
}

impl SchedulerPerfCounters {
    pub fn new() -> Self {
        Self {
            resume_task: PerfCounter::new("resume_task"),
            start_task: PerfCounter::new("start_task"),
            retry_task: PerfCounter::new("retry_task"),
            kill_task: PerfCounter::new("kill_task"),
            setup_task: PerfCounter::new("setup_task"),
            start_command: PerfCounter::new("start_command"),
            parse_command: PerfCounter::new("parse_command"),
            find_verb_for_command: PerfCounter::new("find_verb_for_command"),
            task_conflict_retry: PerfCounter::new("task_conflict_retry"),
            task_abort_cancelled: PerfCounter::new("task_abort_cancelled"),
            task_abort_limits: PerfCounter::new("task_abort_limits"),
            fork_task: PerfCounter::new("fork_task"),
            suspend_task: PerfCounter::new("suspend_task"),
            task_exception: PerfCounter::new("task_exception"),
            handle_scheduler_msg: PerfCounter::new("handle_scheduler_msg"),
            handle_task_msg: PerfCounter::new("handle_task_msg"),
            gc_mark_phase: PerfCounter::new("gc_mark_phase"),
            gc_sweep_phase: PerfCounter::new("gc_sweep_phase"),

            submit_command_task_latency: PerfCounter::new("submit_command_task_latency"),
            submit_verb_task_latency: PerfCounter::new("submit_verb_task_latency"),
            submit_eval_task_latency: PerfCounter::new("submit_eval_task_latency"),
            submit_oob_task_latency: PerfCounter::new("submit_oob_task_latency"),
            submit_system_handler_task_latency: PerfCounter::new(
                "submit_system_handler_task_latency",
            ),
            checkpoint_latency: PerfCounter::new("checkpoint_latency"),
            load_object_latency: PerfCounter::new("load_object_latency"),
            reload_object_latency: PerfCounter::new("reload_object_latency"),

            task_request_fork_latency: PerfCounter::new("task_request_fork_latency"),
            task_kill_task_latency: PerfCounter::new("task_kill_task_latency"),
            task_resume_task_latency: PerfCounter::new("task_resume_task_latency"),
            task_checkpoint_latency: PerfCounter::new("task_checkpoint_latency"),
            task_active_tasks_latency: PerfCounter::new("task_active_tasks_latency"),
            task_begin_transaction_latency: PerfCounter::new("task_begin_transaction_latency"),

            task_wakeup_latency: PerfCounter::new("task_wakeup_latency"),
        }
    }

    pub fn all_counters(&self) -> Vec<&PerfCounter> {
        vec![
            &self.resume_task,
            &self.start_task,
            &self.retry_task,
            &self.kill_task,
            &self.setup_task,
            &self.start_command,
            &self.parse_command,
            &self.find_verb_for_command,
            &self.task_conflict_retry,
            &self.task_abort_cancelled,
            &self.task_abort_limits,
            &self.fork_task,
            &self.suspend_task,
            &self.task_exception,
            &self.handle_scheduler_msg,
            &self.handle_task_msg,
            &self.gc_mark_phase,
            &self.gc_sweep_phase,
            &self.submit_command_task_latency,
            &self.submit_verb_task_latency,
            &self.submit_eval_task_latency,
            &self.submit_oob_task_latency,
            &self.submit_system_handler_task_latency,
            &self.checkpoint_latency,
            &self.load_object_latency,
            &self.reload_object_latency,
            &self.task_request_fork_latency,
            &self.task_kill_task_latency,
            &self.task_resume_task_latency,
            &self.task_checkpoint_latency,
            &self.task_active_tasks_latency,
            &self.task_begin_transaction_latency,
            &self.task_wakeup_latency,
        ]
    }
}

#[derive(Debug, Clone)]
pub enum TaskStart {
    /// The scheduler is telling the task to parse a command and execute whatever verbs are
    /// associated with it.
    StartCommandVerb {
        /// The object that will handle the command, usually #0 (the system object), but can
        /// be a connection handler passed from `listen()`.
        handler_object: Obj,
        player: Obj,
        command: String,
    },
    /// The task start has been turned into an invocation to $do_command, which is a verb on the
    /// system object that is called when a player types a command. If it returns true, all is
    /// well and we just return. If it returns false, we intercept and turn it back into a
    /// StartCommandVerb and dispatch it as an old school parsed command.
    StartDoCommand {
        /// The object that will handle the command, usually #0 (the system object), but can
        /// be a connection handler passed from `listen()`.
        handler_object: Obj,
        player: Obj,
        command: String,
    },
    /// The scheduler is telling the task to run a (method) verb.
    StartVerb {
        player: Obj,
        vloc: Var,
        verb: Symbol,
        args: List,
        argstr: Var,
    },
    /// The scheduler is telling the task to run a task that was forked from another task.
    /// ForkRequest contains the information on the fork vector and other information needed to
    /// set up execution.
    StartFork {
        fork_request: Box<Fork>,
        // If we're starting in a suspended state. If this is true, an explicit Resume from the
        // scheduler will be required to start the task.
        suspended: bool,
    },
    /// The scheduler is telling the task to evaluate a specific (MOO) program.
    StartEval {
        player: Obj,
        program: Program,
        /// Optional initial variable bindings to inject into the eval's environment.
        initial_env: Option<Vec<(Symbol, Var)>>,
    },
    /// The task is executing $handle_uncaught_error to handle an exception.
    /// The original exception is stored so if the handler returns false, we can re-raise it.
    StartExceptionHandler {
        player: Obj,
        args: List,
        original_exception: Box<Exception>,
    },
}

impl TaskStart {
    pub fn is_background(&self) -> bool {
        matches!(self, TaskStart::StartFork { .. })
    }

    pub fn diagnostic(&self) -> String {
        match self {
            TaskStart::StartCommandVerb {
                player, command, ..
            } => {
                format!("CommandVerb(player: {player}, command: {command:?})")
            }
            TaskStart::StartDoCommand {
                player, command, ..
            } => {
                format!("DoCommand(player: {player}, command: {command:?})")
            }
            TaskStart::StartVerb {
                player, verb, vloc, ..
            } => {
                format!(
                    "Verb(player: {}, verb: {}, vloc: {})",
                    player,
                    verb,
                    to_literal(vloc)
                )
            }
            TaskStart::StartFork {
                suspended,
                fork_request,
            } => {
                format!(
                    "Fork(suspended: {}) for verb: {}:{} (defined on {}) @ line {:?}, parent task {}",
                    suspended,
                    to_literal(&fork_request.activation.this),
                    fork_request.activation.verb_name,
                    fork_request.activation.verb_definer(),
                    fork_request.activation.frame.find_line_no(),
                    fork_request.parent_task_id,
                )
            }
            TaskStart::StartEval { player, .. } => {
                format!("Eval(player: {player})")
            }
            TaskStart::StartExceptionHandler { player, .. } => {
                format!("ExceptionHandler(player: {player})")
            }
        }
    }
}
