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

//! A LambdaMOO 1.8.x compatibl(ish) virtual machine.
//! Executes opcodes which are essentially 1:1 with LambdaMOO's.
//! Aims to be semantically identical, so as to be able to run existing LambdaMOO compatible cores
//! without blocking issues.

use std::time::Duration;

use bincode::{Decode, Encode};
use byteview::ByteView;
pub use exec_state::VMExecState;
pub use exec_state::vm_counters;
use moor_common::matching::ParsedCommand;
use moor_common::model::VerbDef;
use moor_common::tasks::{AbortLimitReason, Exception, TaskId};
use moor_compiler::{BuiltinId, Name};
use moor_compiler::{Offset, Program};
use moor_var::{Error, List, Obj, Symbol, Var};
pub use vm_call::VerbExecutionRequest;
pub use vm_unwind::FinallyReason;

// Exports to the rest of the kernel
use crate::tasks::VerbCall;
use crate::vm::activation::Activation;

pub(crate) mod activation;
pub(crate) mod exec_state;
pub(crate) mod moo_execute;
pub(crate) mod vm_call;
pub(crate) mod vm_unwind;

mod moo_frame;
#[cfg(test)]
mod vm_test;

#[derive(Debug, Clone)]
pub enum TaskSuspend {
    /// Suspend forever.
    Never,
    /// Suspend for a given duration.
    Timed(Duration),
    /// Suspend until another task completes (or never exists)
    WaitTask(TaskId),
    /// Just commit and resume immediately.
    Commit,
    /// Ask the scheduler to ask a worker to do some work, suspend us, and then resume us when
    /// the work is done.
    WorkerRequest(Symbol, Vec<Var>),
}

/// Possible outcomes from VM execution inner loop, which are used to determine what to do next.
#[derive(Debug, Clone)]
pub enum ExecutionResult {
    /// All is well. The task should let the VM continue executing.
    More,
    /// Execution of this stack frame is complete with a return value.
    Complete(Var),
    /// An error occurred during execution, that we might need to push to the stack and
    /// potentially resume or unwind, depending on the context.
    PushError(Error),
    /// As above, but with extra meta-data.
    PushErrorPack(Error, String, Var),
    /// An error occurred during execution, that should definitely be treated as a proper "raise"
    /// and unwind event unless there's a catch handler in place
    RaiseError(Error),
    /// An explicit stack unwind (for a reason other than a return.)
    Unwind(FinallyReason),
    /// Explicit return, unwind stack
    Return(Var),
    /// An exception was raised during execution.
    Exception(FinallyReason),
    /// Create the frames necessary to perform a `pass` up the inheritance chain.
    DispatchVerbPass(List),
    /// Begin preparing to call a verb, by looking up the verb and preparing the dispatch.
    PrepareVerbDispatch {
        this: Var,
        verb_name: Symbol,
        args: List,
    },
    /// Perform the verb dispatch, building the stack frame and executing it.
    DispatchVerb {
        /// The applicable permissions context.
        permissions: Obj,
        /// The requested verb.
        resolved_verb: VerbDef,
        /// And its binary
        binary: ByteView,
        /// The call parameters that were used to resolve the verb.
        call: VerbCall,
        /// The parsed user command that led to this verb dispatch, if any.
        command: Option<ParsedCommand>,
    },
    /// Request `eval` execution, which is a kind of special activation creation where we've already
    /// been given the program to execute instead of having to look it up.
    DispatchEval {
        /// The permissions context for the eval.
        permissions: Obj,
        /// The player who is performing the eval.
        player: Obj,
        /// The program to execute.
        program: Program,
    },
    /// Request dispatch of a builtin function with the given arguments.
    DispatchBuiltin { builtin: BuiltinId, arguments: List },
    /// Request start of a new task as a fork, at a given offset into the fork vector of the
    /// current program. If the duration is None, the task should be started immediately, otherwise
    /// it should be scheduled to start after the given delay.
    /// If a Name is provided, the task ID of the new task should be stored in the variable with
    /// that in the parent activation.
    TaskStartFork(Option<Duration>, Option<Name>, Offset),
    /// Request that this task be suspended for a duration of time.
    /// This leads to the task performing a commit, being suspended for a delay, and then being
    /// resumed under a new transaction.
    /// If the duration is None, then the task is suspended indefinitely, until it is killed or
    /// resumed using `resume()` or `kill_task()`.
    TaskSuspend(TaskSuspend),
    /// Request input from the client.
    TaskNeedInput,
    /// Rollback the current transaction and restart the task in a new transaction.
    /// This can happen when a conflict occurs during execution, independent of a commit.
    TaskRollbackRestart,
    /// Just rollback and die. Kills all task DB mutations. Output (Session) is optionally committed.
    TaskRollback(bool),
}

/// The set of parameters for a VM-requested fork.
#[derive(Debug, Clone, Encode, Decode)]
pub struct Fork {
    /// The player. This is in the activation as well, but it's nicer to have it up here and
    /// explicit
    pub(crate) player: Obj,
    /// The permissions context for the forked task.
    pub(crate) progr: Obj,
    /// The task ID of the task that forked us
    pub(crate) parent_task_id: usize,
    /// The time to delay before starting the forked task, if any.
    pub(crate) delay: Option<Duration>,
    /// A copy of the activation record from the task that forked us.
    pub(crate) activation: Activation,
    /// The unique fork vector offset into the fork vector for the executing binary held in the
    /// activation record.  This is copied into the main vector and execution proceeds from there,
    /// instead.
    pub(crate) fork_vector_offset: Offset,
    /// The (optional) variable label where the task ID of the new task should be stored, in both
    /// the parent activation and the new task's activation.
    pub task_id: Option<Name>,
}

/// Return common from exec_interpreter back to the Task scheduler loop
pub enum VMHostResponse {
    /// Tell the task to just keep on letting us do what we're doing.
    ContinueOk,
    /// Tell the task to ask the scheduler to dispatch a fork request, and then resume execution.
    DispatchFork(Fork),
    /// Tell the task to suspend us.
    Suspend(TaskSuspend),
    /// Tell the task Johnny 5 needs input from the client (`read` invocation).
    SuspendNeedInput,
    /// Task timed out or exceeded ticks.
    AbortLimit(AbortLimitReason),
    /// Tell the task that execution has completed, and the task is successful.
    CompleteSuccess(Var),
    /// The VM aborted. (FinallyReason::Abort in MOO VM)
    CompleteAbort,
    /// The VM threw an exception. (FinallyReason::Uncaught in MOO VM)
    CompleteException(Exception),
    /// Finish the task with a DB rollback. Second argument is whether to commit the session.
    CompleteRollback(bool),
    /// A rollback-retry was requested.
    RollbackRetry,
}
