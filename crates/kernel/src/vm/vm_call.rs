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

use std::sync::Arc;
use tracing::{debug, trace};

use moor_values::model::WorldState;
use moor_values::model::WorldStateError;
use moor_values::var::Error::{E_INVIND, E_PERM, E_VARNF, E_VERBNF};
use moor_values::var::Objid;
use moor_values::var::{v_int, Var};

use crate::builtins::bf_server::BF_SERVER_EVAL_TRAMPOLINE_RESUME;
use crate::builtins::{BfCallState, BfRet};
use crate::tasks::command_parse::ParsedCommand;
use crate::tasks::sessions::Session;
use crate::tasks::VerbCall;
use crate::vm::activation::Activation;
use crate::vm::vm_unwind::FinallyReason;
use crate::vm::{ExecutionResult, Fork, VM};
use crate::vm::{VMExecState, VmExecParams};
use moor_compiler::Program;
use moor_compiler::BUILTIN_DESCRIPTORS;
use moor_values::model::VerbInfo;

pub(crate) fn args_literal(args: &[Var]) -> String {
    args.iter()
        .map(|v| v.to_literal())
        .collect::<Vec<String>>()
        .join(", ")
}

/// The set of parameters for a scheduler-requested *resolved* verb method dispatch.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct VerbExecutionRequest {
    /// The applicable permissions.
    pub permissions: Objid,
    /// The resolved verb.
    pub resolved_verb: VerbInfo,
    /// The call parameters that were used to resolve the verb.
    pub call: VerbCall,
    /// The parsed user command that led to this verb dispatch, if any.
    pub command: Option<ParsedCommand>,
    /// The decoded MOO Binary that contains the verb to be executed.
    pub program: Program,
}

impl VM {
    /// Entry point for preparing a verb call for execution, invoked from the CallVerb opcode
    /// Seek the verb and prepare the call parameters.
    /// All parameters for player, caller, etc. are pulled off the stack.
    /// The call params will be returned back to the task in the scheduler, which will then dispatch
    /// back through to `do_method_call`
    pub(crate) fn prepare_call_verb(
        &self,
        vm_state: &mut VMExecState,
        world_state: &mut dyn WorldState,
        this: Objid,
        verb_name: &str,
        args: &[Var],
    ) -> ExecutionResult {
        let call = VerbCall {
            verb_name: verb_name.to_string(),
            location: this,
            this,
            player: vm_state.top().player,
            args: args.to_vec(),
            // caller her is current-activation 'this', not activation caller() ...
            // unless we're a builtin, in which case we're #-1.
            argstr: "".to_string(),
            caller: vm_state.caller(),
        };

        let self_valid = world_state
            .valid(this)
            .expect("Error checking object validity");
        if !self_valid {
            return self.push_error(vm_state, E_INVIND);
        }
        // Find the callable verb ...
        let verb_info =
            match world_state.find_method_verb_on(vm_state.top().permissions, this, verb_name) {
                Ok(vi) => vi,
                Err(WorldStateError::ObjectPermissionDenied) => {
                    return self.push_error(vm_state, E_PERM);
                }
                Err(WorldStateError::VerbPermissionDenied) => {
                    return self.push_error(vm_state, E_PERM);
                }
                Err(WorldStateError::VerbNotFound(_, _)) => {
                    return self.push_error_msg(
                        vm_state,
                        E_VERBNF,
                        format!("Verb \"{}\" not found", verb_name),
                    );
                }
                Err(e) => {
                    panic!("Unexpected error from find_method_verb_on: {:?}", e)
                }
            };

        // Permissions for the activation are the verb's owner.
        let permissions = verb_info.verbdef().owner();

        ExecutionResult::ContinueVerb {
            permissions,
            resolved_verb: verb_info,
            call,
            command: vm_state.top().command.clone(),
            trampoline: None,
            trampoline_arg: None,
        }
    }

    /// Setup the VM to execute the verb of the same current name, but using the parent's
    /// version.
    /// TODO this should be done up in task.rs instead. let's add a new ExecutionResult for it.
    pub(crate) fn prepare_pass_verb(
        &self,
        vm_state: &mut VMExecState,
        world_state: &mut dyn WorldState,
        args: &[Var],
    ) -> ExecutionResult {
        // get parent of verb definer object & current verb name.
        let definer = vm_state.top().verb_definer();
        let permissions = vm_state.top().permissions;
        let parent = world_state
            .parent_of(permissions, definer)
            .expect("unable to lookup parent");
        let verb = vm_state.top().verb_name.to_string();

        // call verb on parent, but with our current 'this'
        trace!(task_id = vm_state.task_id, verb, ?definer, ?parent);

        let Ok(vi) = world_state.find_method_verb_on(permissions, parent, verb.as_str()) else {
            return self.raise_error(vm_state, E_VERBNF);
        };

        let caller = vm_state.caller();
        let call = VerbCall {
            verb_name: verb,
            location: parent,
            this: vm_state.top().this,
            player: vm_state.top().player,
            args: args.to_vec(),
            argstr: "".to_string(),
            caller,
        };

        ExecutionResult::ContinueVerb {
            permissions,
            resolved_verb: vi,
            call,
            command: vm_state.top().command.clone(),
            trampoline: None,
            trampoline_arg: None,
        }
    }

    /// Entry point from scheduler for actually beginning the dispatch of a method execution
    /// (non-command) in this VM.
    /// Actually creates the activation record and puts it on the stack.
    pub fn exec_call_request(
        &self,
        vm_state: &mut VMExecState,
        call_request: VerbExecutionRequest,
    ) {
        let a = Activation::for_call(call_request);
        vm_state.stack.push(a);
    }

    pub fn exec_eval_request(
        &self,
        vm_state: &mut VMExecState,
        permissions: Objid,
        player: Objid,
        program: Program,
    ) {
        if !vm_state.stack.is_empty() {
            // We need to set up a trampoline to return back into `bf_eval`
            vm_state.top_mut().bf_trampoline_arg = None;
            vm_state.top_mut().bf_trampoline = Some(BF_SERVER_EVAL_TRAMPOLINE_RESUME);
        }

        let a = Activation::for_eval(permissions, player, program);

        vm_state.stack.push(a);
    }

    /// Prepare a new stack & call hierarchy for invocation of a forked task.
    /// Called (ultimately) from the scheduler as the result of a fork() call.
    /// We get an activation record which is a copy of where it was borked from, and a new Program
    /// which is the new task's code, derived from a fork vector in the original task.
    pub(crate) fn exec_fork_vector(&self, vm_state: &mut VMExecState, fork_request: Fork) {
        // Set the activation up with the new task ID, and the new code.
        let mut a = fork_request.activation;
        a.frame.program.main_vector =
            Arc::new(a.frame.program.fork_vectors[fork_request.fork_vector_offset.0 as usize].clone());
        a.frame.pc = 0;
        if let Some(task_id_name) = fork_request.task_id {
            a.frame.set_var_offset(&task_id_name, v_int(vm_state.task_id as i64))
                .expect("Unable to set task_id in activation frame");
        }

        // TODO how to set the task_id in the parent activation, as we no longer have a reference
        // to it?
        vm_state.stack = vec![a];
    }

    /// Call into a builtin function.
    pub(crate) fn call_builtin_function(
        &self,
        vm_state: &mut VMExecState,
        bf_func_num: usize,
        args: &[Var],
        exec_args: &VmExecParams,
        world_state: &mut dyn WorldState,
        session: Arc<dyn Session>,
    ) -> ExecutionResult {
        if bf_func_num >= self.builtins.len() {
            return self.raise_error(vm_state, E_VARNF);
        }
        let bf = self.builtins[bf_func_num].clone();

        debug!(
            "Calling builtin: {}({}) caller_perms: {}",
            BUILTIN_DESCRIPTORS[bf_func_num].name,
            args_literal(args),
            vm_state.top().permissions
        );
        let args = args.to_vec();

        // Push an activation frame for the builtin function.
        let flags = vm_state.top().verb_info.verbdef().flags();
        vm_state.stack.push(Activation::for_bf_call(
            bf_func_num,
            BUILTIN_DESCRIPTORS[bf_func_num].name.as_str(),
            args.clone(),
            // We copy the flags from the calling verb, that will determine error handling 'd'
            // behaviour below.
            flags,
            vm_state.top().player,
        ));
        let mut bf_args = BfCallState {
            exec_state: vm_state,
            name: BUILTIN_DESCRIPTORS[bf_func_num].name.clone(),
            world_state,
            session: session.clone(),
            args,
            scheduler_sender: exec_args.scheduler_sender.clone(),
        };

        let call_results = match bf.call(&mut bf_args) {
            Ok(BfRet::Ret(result)) => {
                self.unwind_stack(vm_state, FinallyReason::Return(result.clone()))
            }
            Err(e) => self.push_bf_error(vm_state, e),
            Ok(BfRet::VmInstr(vmi)) => vmi,
        };

        trace!(?call_results, "Builtin function call complete");
        call_results
    }

    /// We're returning into a builtin function, which is all set up at the top of the stack.
    pub(crate) fn reenter_builtin_function(
        &self,
        vm_state: &mut VMExecState,
        exec_args: &VmExecParams,
        world_state: &mut dyn WorldState,
        session: Arc<dyn Session>,
    ) -> ExecutionResult {
        trace!(
            bf_index = vm_state.top().bf_index,
            "Reentering builtin function"
        );
        // Functions that did not set a trampoline are assumed to be complete, so we just unwind.
        // Note: If there was an error that required unwinding, we'll have already done that, so
        // we can assume a *value* here not, an error.
        let Some(_) = vm_state.top_mut().bf_trampoline else {
            let return_value = vm_state.top_mut().frame.pop();

            return self.unwind_stack(vm_state, FinallyReason::Return(return_value));
        };

        let bf = self.builtins[vm_state.top().bf_index.unwrap()].clone();
        let verb_name = vm_state.top().verb_name.clone();
        let sessions = session.clone();
        let args = vm_state.top().args.clone();
        let mut bf_args = BfCallState {
            exec_state: vm_state,
            name: verb_name,
            world_state,
            session: sessions,
            args,
            scheduler_sender: exec_args.scheduler_sender.clone(),
        };

        match bf.call(&mut bf_args) {
            Ok(BfRet::Ret(result)) => {
                self.unwind_stack(vm_state, FinallyReason::Return(result.clone()))
            }
            Err(e) => self.push_bf_error(vm_state, e),
            Ok(BfRet::VmInstr(vmi)) => vmi,
        }
    }
}
