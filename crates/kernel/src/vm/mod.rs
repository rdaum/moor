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

use std::time::Duration;

use bincode::{Decode, Encode};
use moor_common::tasks::{AbortLimitReason, Exception, TaskId};
use moor_compiler::Offset;
pub use moor_var::program::ProgramType;
use moor_var::program::names::Name;
use moor_var::{List, Obj, Symbol, Var};
use moor_db::{SEQUENCE_MAX_OBJECT, SEQUENCE_MAX_UUOBJID};
pub use vm_call::VerbExecutionRequest;
pub use vm_unwind::FinallyReason;

use crate::vm::activation::Activation;

pub(crate) mod activation;
pub(crate) mod exec_state;
pub(crate) mod moo_execute;
pub(crate) mod scatter_assign;
pub(crate) mod vm_call;
pub(crate) mod vm_unwind;

pub mod builtins;
mod moo_frame;
pub mod vm_host;

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
    DispatchFork(Box<Fork>),
    /// Tell the task to suspend us.
    Suspend(Box<TaskSuspend>),
    /// Tell the task Johnny 5 needs input from the client (`read` invocation).
    SuspendNeedInput,
    /// Task timed out or exceeded ticks.
    AbortLimit(AbortLimitReason),
    /// Tell the task that execution has completed, and the task is successful.
    CompleteSuccess(Var),
    /// The VM aborted. (FinallyReason::Abort in MOO VM)
    CompleteAbort,
    /// The VM threw an exception. (FinallyReason::Uncaught in MOO VM)
    CompleteException(Box<Exception>),
    /// Finish the task with a DB rollback. Second argument is whether to commit the session.
    CompleteRollback(bool),
    /// A rollback-retry was requested.
    RollbackRetry,
}

/// Response back to our caller (scheduler) that we would like to be suspended in the manner described.
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
    WorkerRequest(Symbol, Vec<Var>, Option<Duration>),
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

#[cfg(test)]
mod tests {
    use crate::vm::VMHostResponse;

    #[test]
    fn test_width_structs_enums() {
        // This was insanely huge (>4k) so some boxing made it smaller, let's see if we can
        // keep it small.
        assert!(
            size_of::<VMHostResponse>() <= 24,
            "VMHostResponse is too big: {}",
            size_of::<VMHostResponse>()
        );
    }
}
