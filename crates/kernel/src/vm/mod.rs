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

use moor_common::tasks::{AbortLimitReason, Exception};
use moor_var::{Obj, Symbol, Var};

// Re-export types from moor-vm that kernel code uses.
pub use moor_vm::FinallyReason;
pub use moor_vm::Fork;
pub(crate) use moor_vm::{Activation, Frame, MooStackFrame, ScopeType};
pub use moor_vm::{CommandVerbExecutionRequest, VerbExecutionRequest};

pub(crate) mod kernel_host;
pub(crate) mod vm_call;

pub mod builtins;
pub mod vm_host;

/// Return common from exec_interpreter back to the Task scheduler loop
pub enum VMHostResponse {
    /// Tell the task to just keep on letting us do what we're doing.
    ContinueOk,
    /// Tell the task to ask the scheduler to dispatch a fork request, and then resume execution.
    DispatchFork(Box<Fork>),
    /// Tell the task to suspend us.
    Suspend(Box<TaskSuspend>),
    /// Tell the task Johnny 5 needs input from the client (`read` invocation).
    /// Optional metadata provides UI hints for rich input prompts.
    SuspendNeedInput(Option<Vec<(Symbol, Var)>>),
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

pub use moor_vm::TaskSuspend;

/// Extract anonymous object references from a variable
fn extract_anonymous_refs_from_var(var: &Var, refs: &mut std::collections::HashSet<Obj>) {
    match var.variant() {
        moor_var::Variant::Obj(obj) => {
            if obj.is_anonymous() {
                refs.insert(obj);
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
            for (_symbol, slot_value) in flyweight.slots_storage().iter() {
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
    for scope in frame.environment.iter_scopes() {
        for var in scope.iter().filter(|v| !v.is_none()) {
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
                current_key,
                end_label: _,
            } => {
                extract_anonymous_refs_from_var(sequence, refs);
                if let Some(k) = current_key {
                    extract_anonymous_refs_from_var(k, refs);
                }
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
        Frame::Js(js_frame) => {
            if let Some(return_value) = &js_frame.return_value {
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
    vm_state: &moor_vm::ExecState,
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
    use moor_vm::{Activation, BuiltinFrame, Frame, MooStackFrame};
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

    #[test]
    fn test_frame_sizes() {
        println!("Size of MooStackFrame: {}", size_of::<MooStackFrame>());
        println!("Size of BuiltinFrame: {}", size_of::<BuiltinFrame>());
        println!("Size of Frame (unboxed): {}", size_of::<Frame>());
        println!("Size of Activation: {}", size_of::<Activation>());

        // Frame is an enum with MooStackFrame and BuiltinFrame (unboxed)
        // Size should be max(MooStackFrame, BuiltinFrame) + discriminant
        let expected_size = size_of::<MooStackFrame>().max(size_of::<BuiltinFrame>()) + 8;
        println!("Expected Frame size: {expected_size}");
        println!("Total activation size: {} bytes", size_of::<Activation>());
    }

    #[test]
    fn test_list_box_overhead() {
        use moor_var::{List, Var};

        // List is defined as: List(Box<im::Vector<Var>>)
        println!("Size of List (just the Box pointer): {}", size_of::<List>());
        println!("Size of Var: {}", size_of::<Var>());
        println!("Size of Box pointer: {}", size_of::<Box<()>>());

        // Create a list and clone it
        let list1 = List::mk_list(&[Var::mk_integer(1), Var::mk_integer(2), Var::mk_integer(3)]);
        let _list2 = list1.clone();

        // When we clone List(Box<im::Vector<Var>>):
        // Box::clone() does:
        //   1. Allocates new heap memory for Box wrapper (malloc)
        //   2. Calls imbl::Vector::clone() on the interior
        //   3. imbl::Vector::clone() bumps Arc refcount (cheap!)
        //
        // The Box wrapper adds malloc overhead to every List::clone()
        // But imbl::Vector::clone() itself is still cheap (just Arc refcount)

        println!("\nCloning behavior:");
        println!("- Box::clone() allocates heap memory (malloc overhead)");
        println!("- imbl::Vector::clone() bumps Arc refcount (nearly free)");
        println!(
            "\nSo List::clone has malloc overhead from Box, but structural sharing from imbl::Vector"
        );
    }
}
