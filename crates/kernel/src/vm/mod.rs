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

use moor_common::tasks::{AbortLimitReason, Exception, TaskId};
use moor_compiler::Offset;
pub use moor_var::program::ProgramType;
use moor_var::{List, Obj, Symbol, Var, program::names::Name};

use crate::vm::{
    activation::{Activation, Frame},
    moo_frame::{MooStackFrame, ScopeType},
};
pub use vm_call::VerbExecutionRequest;
pub use vm_unwind::FinallyReason;

pub(crate) mod activation;
pub(crate) mod exec_state;
pub(crate) mod moo_execute;
pub(crate) mod scatter_assign;
pub(crate) mod vm_call;
pub(crate) mod vm_unwind;

pub mod builtins;
pub(crate) mod moo_frame;
pub mod vm_host;

/// The set of parameters for a VM-requested fork.
#[derive(Debug, Clone)]
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

/// Extract anonymous object references from a variable
fn extract_anonymous_refs_from_var(var: &Var, refs: &mut std::collections::HashSet<Obj>) {
    match var.variant() {
        moor_var::Variant::Obj(obj) => {
            if obj.is_anonymous() {
                refs.insert(*obj);
            }
        }
        moor_var::Variant::List(list) => {
            for item in list.iter() {
                extract_anonymous_refs_from_var(&item, refs);
            }
        }
        moor_var::Variant::Map(map) => {
            for (key, value) in map.iter() {
                extract_anonymous_refs_from_var(&key, refs);
                extract_anonymous_refs_from_var(&value, refs);
            }
        }
        moor_var::Variant::Flyweight(flyweight) => {
            // Check delegate
            let delegate = flyweight.delegate();
            if delegate.is_anonymous() {
                refs.insert(*delegate);
            }

            // Check slots (Symbol -> Var pairs)
            for (_symbol, slot_value) in flyweight.slots().iter() {
                extract_anonymous_refs_from_var(slot_value, refs);
            }

            // Check contents (List)
            for item in flyweight.contents().iter() {
                extract_anonymous_refs_from_var(&item, refs);
            }
        }
        moor_var::Variant::Err(error) => {
            // Check the error's optional value field
            if let Some(error_value) = &error.value {
                extract_anonymous_refs_from_var(error_value, refs);
            }
        }
        moor_var::Variant::Lambda(lambda) => {
            // Check captured environment (stack frames)
            for frame in lambda.0.captured_env.iter() {
                for var in frame.iter() {
                    extract_anonymous_refs_from_var(var, refs);
                }
            }
        }
        _ => {} // Other types (None, Bool, Int, Float, Str, Sym, Binary) don't contain object references
    }
}

/// Extract anonymous object references from a MOO stack frame
fn extract_anonymous_refs_from_moo_frame(
    frame: &MooStackFrame,
    refs: &mut std::collections::HashSet<Obj>,
) {
    // 1. Scan all variables in the environment stack
    for env_level in &frame.environment {
        for var in env_level.iter().flatten() {
            extract_anonymous_refs_from_var(var, refs);
        }
    }

    // 2. Scan all values on the value stack
    for var in &frame.valstack {
        extract_anonymous_refs_from_var(var, refs);
    }

    // 3. Scan temp variable
    extract_anonymous_refs_from_var(&frame.temp, refs);

    // 4. Scan scope stack for any stored variables
    for scope in &frame.scope_stack {
        match &scope.scope_type {
            ScopeType::ForSequence {
                sequence,
                value_bind: _,
                key_bind: _,
                current_index: _,
                end_label: _,
            } => {
                extract_anonymous_refs_from_var(sequence, refs);
            }
            ScopeType::ForRange {
                current_value,
                end_value,
                loop_variable: _,
                end_label: _,
            } => {
                extract_anonymous_refs_from_var(current_value, refs);
                extract_anonymous_refs_from_var(end_value, refs);
            }
            _ => {} // Other scope types don't store variables we can scan
        }
    }

    // 5. Scan capture stack
    for (_name, var) in &frame.capture_stack {
        extract_anonymous_refs_from_var(var, refs);
    }
}

/// Extract anonymous object references from an activation frame
fn extract_anonymous_refs_from_activation(
    activation: &Activation,
    refs: &mut std::collections::HashSet<Obj>,
) {
    // 1. Scan the frame contents
    match &activation.frame {
        Frame::Moo(moo_frame) => {
            extract_anonymous_refs_from_moo_frame(moo_frame, refs);
        }
        Frame::Bf(bf_frame) => {
            // Check bf trampoline argument
            if let Some(trampoline_arg) = &bf_frame.bf_trampoline_arg {
                extract_anonymous_refs_from_var(trampoline_arg, refs);
            }
            // Check return value
            if let Some(return_value) = &bf_frame.return_value {
                extract_anonymous_refs_from_var(return_value, refs);
            }
        }
    }

    // 2. Scan activation-level variables
    // Check 'this' object
    extract_anonymous_refs_from_var(&activation.this, refs);

    // Check player (already an Obj, so check directly)
    if activation.player.is_anonymous() {
        refs.insert(activation.player);
    }

    // Check permissions (already an Obj, so check directly)
    if activation.permissions.is_anonymous() {
        refs.insert(activation.permissions);
    }

    // Scan arguments
    for arg in activation.args.iter() {
        extract_anonymous_refs_from_var(&arg, refs);
    }

    // 3. Check verbdef fields for anonymous object references
    if activation.verbdef.location().is_anonymous() {
        refs.insert(activation.verbdef.location());
    }
    if activation.verbdef.owner().is_anonymous() {
        refs.insert(activation.verbdef.owner());
    }
}

/// Extract anonymous object references from VM execution state
pub(crate) fn extract_anonymous_refs_from_vm_exec_state(
    vm_state: &exec_state::VMExecState,
    refs: &mut std::collections::HashSet<Obj>,
) {
    // Scan all activations in the call stack
    for activation in &vm_state.stack {
        extract_anonymous_refs_from_activation(activation, refs);
    }
}

#[cfg(test)]
mod tests {
    use crate::vm::VMHostResponse;
    use std::mem::size_of;

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
