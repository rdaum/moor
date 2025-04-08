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

use std::fmt::Debug;
use std::sync::Arc;
use std::time::SystemTime;

use bincode::{Decode, Encode};
use lazy_static::lazy_static;
use moor_compiler::Program;
use moor_var::{List, Obj};
use moor_var::{Symbol, Var};

pub use crate::tasks::tasks_db::{NoopTasksDb, TasksDb, TasksDbError};
use crate::vm::Fork;
use moor_common::tasks::{SchedulerError, TaskId};
use moor_common::util::PerfCounter;

pub mod scheduler;
pub mod sessions;

pub(crate) mod scheduler_client;
pub(crate) mod suspension;
pub(crate) mod task;
pub mod task_scheduler_client;
mod tasks_db;
pub mod vm_host;
pub mod workers;

pub const DEFAULT_FG_TICKS: usize = 60_000;
pub const DEFAULT_BG_TICKS: usize = 30_000;
pub const DEFAULT_FG_SECONDS: u64 = 5;
pub const DEFAULT_BG_SECONDS: u64 = 3;
pub const DEFAULT_MAX_STACK_DEPTH: usize = 50;

lazy_static! {
    static ref SCHED_COUNTERS: Arc<SchedulerPerfCounters> = Arc::new(SchedulerPerfCounters::new());
}

pub fn sched_counters() -> Arc<SchedulerPerfCounters> {
    SCHED_COUNTERS.clone()
}

/// Just a handle to a task, with a receiver for the result.
pub struct TaskHandle(
    TaskId,
    oneshot::Receiver<Result<TaskResult, SchedulerError>>,
);

// Results from a task which are either a value or a notification that the underlying task handle
// was replaced at the whim of the scheduler.
pub enum TaskResult {
    Result(Var),
    // TODO: this is no longer used, 'twas used in previous implementation of task restarting on
    //  conflict. But the facility could come in handy in the future, so leaving it in for now,
    //  rather than gut all the code I wrote to handle it on the other side.
    Replaced(TaskHandle),
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
    pub fn into_receiver(self) -> oneshot::Receiver<Result<TaskResult, SchedulerError>> {
        self.1
    }

    pub fn receiver(&self) -> &oneshot::Receiver<Result<TaskResult, SchedulerError>> {
        &self.1
    }
}

/// The minimum set of information needed to make a *resolution* call for a verb.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct VerbCall {
    pub verb_name: Symbol,
    pub location: Var,
    pub this: Var,
    pub player: Obj,
    pub args: List,
    pub argstr: String,
    pub caller: Var,
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
#[derive(Debug, Clone, Encode, Decode)]
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
    vm_dispatch: PerfCounter,
    post_vm_dispatch: PerfCounter,
    setup_task: PerfCounter,
    start_command: PerfCounter,
    parse_command: PerfCounter,
    find_verb_for_command: PerfCounter,
    task_conflict_retry: PerfCounter,
    task_abort_cancelled: PerfCounter,
    task_abort_limits: PerfCounter,
    fork_task: PerfCounter,
    suspend_task: PerfCounter,
    task_exception: PerfCounter,
    handle_scheduler_msg: PerfCounter,
    handle_task_msg: PerfCounter,
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
            vm_dispatch: PerfCounter::new("vm_dispatch"),
            post_vm_dispatch: PerfCounter::new("post_vm_dispatch"),
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
        }
    }

    pub fn all_counters(&self) -> Vec<&PerfCounter> {
        vec![
            &self.resume_task,
            &self.start_task,
            &self.retry_task,
            &self.kill_task,
            &self.vm_dispatch,
            &self.post_vm_dispatch,
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
        ]
    }
}

pub mod vm_test_utils {
    use std::sync::Arc;
    use std::time::Duration;

    use moor_common::model::WorldState;
    use moor_compiler::Program;
    use moor_var::{List, SYSTEM_OBJECT};
    use moor_var::{Obj, Var};
    use moor_var::{Symbol, v_obj};

    use crate::builtins::BuiltinRegistry;
    use crate::config::FeaturesConfig;
    use crate::tasks::VerbCall;
    use crate::tasks::sessions::Session;
    use crate::tasks::vm_host::VmHost;
    use crate::vm::VMHostResponse;
    use moor_common::tasks::Exception;

    pub type ExecResult = Result<Var, Exception>;

    fn execute<F>(
        world_state: &mut dyn WorldState,
        session: Arc<dyn Session>,
        builtins: Arc<BuiltinRegistry>,
        fun: F,
    ) -> ExecResult
    where
        F: FnOnce(&mut dyn WorldState, &mut VmHost),
    {
        let (scs_tx, _scs_rx) = crossbeam_channel::unbounded();
        let task_scheduler_client =
            crate::tasks::task_scheduler_client::TaskSchedulerClient::new(0, scs_tx);
        let mut vm_host = VmHost::new(0, 20, 90_000, Duration::from_secs(5));

        fun(world_state, &mut vm_host);

        let config = FeaturesConfig::default();

        // Call repeatedly into exec until we ge either an error or Complete.
        loop {
            match vm_host.exec_interpreter(
                0,
                world_state,
                task_scheduler_client.clone(),
                session.clone(),
                builtins.clone(),
                config.clone(),
            ) {
                VMHostResponse::ContinueOk => {
                    continue;
                }
                VMHostResponse::DispatchFork(f) => {
                    panic!("Unexpected fork: {:?}", f);
                }
                VMHostResponse::AbortLimit(a) => {
                    panic!("Unexpected abort: {:?}", a);
                }
                VMHostResponse::CompleteException(e) => {
                    return Err(e);
                }
                VMHostResponse::CompleteSuccess(v) => {
                    return Ok(v);
                }
                VMHostResponse::CompleteAbort => {
                    panic!("Unexpected abort");
                }
                VMHostResponse::Suspend(_) => {
                    panic!("Unexpected suspend");
                }
                VMHostResponse::SuspendNeedInput => {
                    panic!("Unexpected suspend need input");
                }
                VMHostResponse::RollbackRetry => {
                    panic!("Unexpected rollback retry");
                }
                VMHostResponse::CompleteRollback(_) => {
                    panic!("Unexpected rollback");
                }
            }
        }
    }

    pub fn call_verb(
        world_state: &mut dyn WorldState,
        session: Arc<dyn Session>,
        builtins: Arc<BuiltinRegistry>,
        verb_name: &str,
        args: List,
    ) -> ExecResult {
        execute(world_state, session, builtins, |world_state, vm_host| {
            let verb_name = Symbol::mk_case_insensitive(verb_name);
            let vi = world_state
                .find_method_verb_on(&SYSTEM_OBJECT, &SYSTEM_OBJECT, verb_name)
                .unwrap();
            vm_host.start_call_method_verb(
                0,
                &SYSTEM_OBJECT,
                vi,
                VerbCall {
                    verb_name,
                    location: v_obj(SYSTEM_OBJECT),
                    this: v_obj(SYSTEM_OBJECT),
                    player: SYSTEM_OBJECT,
                    args,
                    argstr: "".to_string(),
                    caller: v_obj(SYSTEM_OBJECT),
                },
            );
        })
    }

    pub fn call_eval_builtin(
        world_state: &mut dyn WorldState,
        session: Arc<dyn Session>,
        builtins: Arc<BuiltinRegistry>,

        player: Obj,
        program: Program,
    ) -> ExecResult {
        execute(world_state, session, builtins, |world_state, vm_host| {
            vm_host.start_eval(0, &player, program, world_state);
        })
    }
}

pub mod scheduler_test_utils {
    use std::sync::Arc;
    use std::time::Duration;

    use moor_common::tasks::{CommandError, SchedulerError};
    use moor_var::{Error::E_VERBNF, Obj, SYSTEM_OBJECT, Var};

    use super::{TaskHandle, TaskResult};
    use crate::config::FeaturesConfig;
    use crate::tasks::scheduler_client::SchedulerClient;
    use crate::tasks::sessions::Session;
    use moor_common::tasks::Exception;
    use moor_common::tasks::SchedulerError::{CommandExecutionError, TaskAbortedException};

    pub type ExecResult = Result<Var, Exception>;

    fn execute<F>(fun: F) -> Result<Var, SchedulerError>
    where
        F: FnOnce() -> Result<TaskHandle, SchedulerError>,
    {
        let task_handle = fun()?;
        match task_handle
            .1
            .recv_timeout(Duration::from_secs(1))
            .inspect_err(|e| {
                eprintln!(
                    "subscriber.recv_timeout() failed for task {}: {e}",
                    task_handle.task_id(),
                )
            })
            .unwrap()
        {
            // Some errors can be represented as a MOO `Var`; translate those to a `Var`, so that
            // `moot` tests can match against them.
            Err(TaskAbortedException(Exception { code, .. })) => Ok(code.into()),
            Err(CommandExecutionError(CommandError::NoCommandMatch)) => Ok(E_VERBNF.into()),
            Err(err) => Err(err),
            Ok(TaskResult::Result(var)) => Ok(var),
            Ok(TaskResult::Replaced(_)) => panic!("Unexpected task restart"),
        }
    }

    pub fn call_command(
        scheduler: SchedulerClient,
        session: Arc<dyn Session>,
        player: &Obj,
        command: &str,
    ) -> Result<Var, SchedulerError> {
        execute(|| scheduler.submit_command_task(&SYSTEM_OBJECT, player, command, session))
    }

    pub fn call_eval(
        scheduler: SchedulerClient,
        session: Arc<dyn Session>,
        player: &Obj,
        code: String,
    ) -> Result<Var, SchedulerError> {
        execute(|| {
            scheduler.submit_eval_task(player, player, code, session, FeaturesConfig::default())
        })
    }
}

#[derive(Debug, Clone, Encode, Decode)]
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
        argstr: String,
    },
    /// The scheduler is telling the task to run a task that was forked from another task.
    /// ForkRequest contains the information on the fork vector and other information needed to
    /// set up execution.
    StartFork {
        fork_request: Fork,
        // If we're starting in a suspended state. If this is true, an explicit Resume from the
        // scheduler will be required to start the task.
        suspended: bool,
    },
    /// The scheduler is telling the task to evaluate a specific (MOO) program.
    StartEval { player: Obj, program: Program },
}

impl TaskStart {
    pub fn is_background(&self) -> bool {
        matches!(self, TaskStart::StartFork { .. })
    }
}
