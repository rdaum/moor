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

//! A LambdaMOO 1.8.x compatibl(ish) virtual machine.
//! Executes opcodes which are essentially 1:1 with LambdaMOO's.
//! Aims to be semantically identical, so as to be able to run existing LambdaMOO compatible cores
//! without blocking issues.

use std::sync::Arc;
use std::time::Duration;

use bincode::{Decode, Encode};
use bytes::Bytes;
pub use exec_state::VMExecState;
use moor_compiler::{BuiltinId, Name};
use moor_compiler::{Offset, Program};
use moor_values::matching::command_parse::ParsedCommand;
use moor_values::model::VerbDef;
use moor_values::{Obj, Var};
pub use vm_call::VerbExecutionRequest;
pub use vm_unwind::FinallyReason;

// Exports to the rest of the kernel
use crate::builtins::BuiltinRegistry;
use crate::config::Config;
use crate::tasks::task_scheduler_client::TaskSchedulerClient;
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

/// Represents the set of parameters passed to the VM for execution.
pub struct VmExecParams {
    pub task_scheduler_client: TaskSchedulerClient,
    pub builtin_registry: Arc<BuiltinRegistry>,
    pub max_stack_depth: usize,
    pub config: Arc<Config>,
}

#[derive(Debug, Clone)]
pub enum ExecutionResult {
    /// Execution of this call stack is complete.
    Complete(Var),
    /// All is well. The task should let the VM continue executing.
    More,
    /// An exception was raised during execution.
    Exception(FinallyReason),
    /// Request dispatch to another verb
    ContinueVerb {
        /// The applicable permissions context.
        permissions: Obj,
        /// The requested verb.
        resolved_verb: VerbDef,
        /// And its binary
        binary: Bytes,
        /// The call parameters that were used to resolve the verb.
        call: VerbCall,
        /// The parsed user command that led to this verb dispatch, if any.
        command: Option<ParsedCommand>,
    },
    /// Request dispatch of a new task as a fork
    DispatchFork(Fork),
    /// Request dispatch of a builtin function with the given arguments.
    ContinueBuiltin {
        builtin: BuiltinId,
        arguments: Vec<Var>,
    },
    /// Request that this task be suspended for a duration of time.
    /// This leads to the task performing a commit, being suspended for a delay, and then being
    /// resumed under a new transaction.
    /// If the duration is None, then the task is suspended indefinitely, until it is killed or
    /// resumed using `resume()` or `kill_task()`.
    Suspend(Option<Duration>),
    /// Request input from the client.
    NeedInput,
    /// Request `eval` execution, which is a kind of special activation creation where we've already
    /// been given the program to execute instead of having to look it up.
    PerformEval {
        /// The permissions context for the eval.
        permissions: Obj,
        /// The player who is performing the eval.
        player: Obj,
        /// The program to execute.
        program: Program,
    },
    /// Rollback the current transaction and restart the task in a new transaction.
    /// This can happen when a conflict occurs during execution, independent of a commit.
    RollbackRestart,
}
