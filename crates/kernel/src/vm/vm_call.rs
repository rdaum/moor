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
use tracing::trace;

use moor_values::model::WorldState;
use moor_values::model::WorldStateError;
use moor_values::var::v_int;
use moor_values::var::Error::{E_INVIND, E_PERM, E_VARNF, E_VERBNF};
use moor_values::var::{List, Objid};

use crate::builtins::{BfCallState, BfErr, BfRet};
use crate::tasks::command_parse::ParsedCommand;
use crate::tasks::sessions::Session;
use crate::tasks::VerbCall;
use crate::vm::activation::{Activation, VmStackFrame};
use crate::vm::vm_unwind::FinallyReason;
use crate::vm::{ExecutionResult, Fork};
use crate::vm::{VMExecState, VmExecParams};
use moor_compiler::Program;
use moor_compiler::BUILTIN_DESCRIPTORS;
use moor_values::model::VerbInfo;
use moor_values::var::Symbol;

pub(crate) fn args_literal(args: &List) -> String {
    args.iter()
        .map(|v| v.to_literal())
        .collect::<Vec<String>>()
        .join(", ")
}

/// The set of parameters for a scheduler-requested *resolved* verb method dispatch.
#[derive(Debug, Clone, PartialEq)]
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

impl VMExecState {
    /// Entry point for preparing a verb call for execution, invoked from the CallVerb opcode
    /// Seek the verb and prepare the call parameters.
    /// All parameters for player, caller, etc. are pulled off the stack.
    /// The call params will be returned back to the task in the scheduler, which will then dispatch
    /// back through to `do_method_call`
    pub(crate) fn prepare_call_verb(
        &mut self,
        world_state: &mut dyn WorldState,
        this: Objid,
        verb_name: Symbol,
        args: List,
    ) -> ExecutionResult {
        let call = VerbCall {
            verb_name,
            location: this,
            this,
            player: self.top().player,
            args,
            // caller her is current-activation 'this', not activation caller() ...
            // unless we're a builtin, in which case we're #-1.
            argstr: "".to_string(),
            caller: self.caller(),
        };

        let self_valid = world_state
            .valid(this)
            .expect("Error checking object validity");
        if !self_valid {
            return self.push_error(E_INVIND);
        }
        // Find the callable verb ...
        let verb_info =
            match world_state.find_method_verb_on(self.top().permissions, this, verb_name) {
                Ok(vi) => vi,
                Err(WorldStateError::ObjectPermissionDenied) => {
                    return self.push_error(E_PERM);
                }
                Err(WorldStateError::RollbackRetry) => {
                    return ExecutionResult::RollbackRestart;
                }
                Err(WorldStateError::VerbPermissionDenied) => {
                    return self.push_error(E_PERM);
                }
                Err(WorldStateError::VerbNotFound(_, _)) => {
                    return self
                        .push_error_msg(E_VERBNF, format!("Verb \"{}\" not found", verb_name));
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
            command: self.top().command.clone(),
        }
    }

    /// Setup the VM to execute the verb of the same current name, but using the parent's
    /// version.
    /// TODO this should be done up in task.rs instead. let's add a new ExecutionResult for it.
    pub(crate) fn prepare_pass_verb(
        &mut self,
        world_state: &mut dyn WorldState,
        args: &List,
    ) -> ExecutionResult {
        // get parent of verb definer object & current verb name.
        let definer = self.top().verb_definer();
        let permissions = self.top().permissions;

        let parent = match world_state.parent_of(permissions, definer) {
            Ok(parent) => parent,
            Err(WorldStateError::RollbackRetry) => {
                return ExecutionResult::RollbackRestart;
            }
            Err(e) => return self.raise_error(e.to_error_code()),
        };
        let verb = self.top().verb_name;

        // call verb on parent, but with our current 'this'
        trace!(task_id = self.task_id, ?verb, ?definer, ?parent);

        let vi = match world_state.find_method_verb_on(permissions, parent, verb) {
            Ok(vi) => vi,
            Err(WorldStateError::RollbackRetry) => {
                return ExecutionResult::RollbackRestart;
            }
            Err(e) => return self.raise_error(e.to_error_code()),
        };

        let caller = self.caller();
        let call = VerbCall {
            verb_name: verb,
            location: parent,
            this: self.top().this,
            player: self.top().player,
            args: args.clone(),
            argstr: "".to_string(),
            caller,
        };

        ExecutionResult::ContinueVerb {
            permissions,
            resolved_verb: vi,
            call,
            command: self.top().command.clone(),
        }
    }

    /// Entry point from scheduler for actually beginning the dispatch of a method execution
    /// (non-command) in this VM.
    /// Actually creates the activation record and puts it on the stack.
    pub fn exec_call_request(&mut self, call_request: VerbExecutionRequest) {
        let a = Activation::for_call(call_request);
        self.stack.push(a);
    }

    pub fn exec_eval_request(&mut self, permissions: Objid, player: Objid, program: Program) {
        let a = Activation::for_eval(permissions, player, program);

        self.stack.push(a);
    }

    /// Prepare a new stack & call hierarchy for invocation of a forked task.
    /// Called (ultimately) from the scheduler as the result of a fork() call.
    /// We get an activation record which is a copy of where it was borked from, and a new Program
    /// which is the new task's code, derived from a fork vector in the original task.
    pub(crate) fn exec_fork_vector(&mut self, fork_request: Fork) {
        // Set the activation up with the new task ID, and the new code.
        let mut a = fork_request.activation;

        // This makes sense only for a MOO stack frame, and could only be initiated from there,
        // so anything else is a legit panic, we shouldn't have gotten here.
        let VmStackFrame::Moo(ref mut frame) = a.frame else {
            panic!("Attempt to fork a non-MOO frame");
        };

        frame.program.main_vector = Arc::new(
            frame.program.fork_vectors[fork_request.fork_vector_offset.0 as usize].clone(),
        );
        frame.pc = 0;
        if let Some(task_id_name) = fork_request.task_id {
            frame
                .set_var_offset(&task_id_name, v_int(self.task_id as i64))
                .expect("Unable to set task_id in activation frame");
        }

        // TODO how to set the task_id in the parent activation, as we no longer have a reference
        // to it?
        self.stack = vec![a];
    }

    /// Call into a builtin function.
    pub(crate) fn call_builtin_function(
        &mut self,
        bf_func_num: usize,
        args: List,
        exec_args: &VmExecParams,
        world_state: &mut dyn WorldState,
        session: Arc<dyn Session>,
    ) -> ExecutionResult {
        if bf_func_num >= exec_args.builtin_registry.builtins.len() {
            return self.raise_error(E_VARNF);
        }
        let bf = exec_args.builtin_registry.builtins[bf_func_num].clone();

        trace!(
            "Calling builtin: {}({}) caller_perms: {}",
            BUILTIN_DESCRIPTORS[bf_func_num].name,
            args_literal(&args),
            self.top().permissions
        );

        // Push an activation frame for the builtin function.
        let flags = self.top().verb_info.verbdef().flags();
        self.stack.push(Activation::for_bf_call(
            bf_func_num,
            BUILTIN_DESCRIPTORS[bf_func_num].name,
            args.clone(),
            // We copy the flags from the calling verb, that will determine error handling 'd'
            // behaviour below.
            flags,
            self.top().player,
        ));
        let mut bf_args = BfCallState {
            exec_state: self,
            name: BUILTIN_DESCRIPTORS[bf_func_num].name,
            world_state,
            session: session.clone(),
            // TODO: avoid copy here by using List inside BfCallState
            args: args.iter().collect(),
            task_scheduler_client: exec_args.task_scheduler_client.clone(),
        };

        let call_results = match bf.call(&mut bf_args) {
            Ok(BfRet::Ret(result)) => self.unwind_stack(FinallyReason::Return(result.clone())),
            Err(BfErr::Code(e)) => self.push_bf_error(e, None, None),
            Err(BfErr::Raise(e, msg, value)) => self.push_bf_error(e, msg, value),
            Err(BfErr::Rollback) => ExecutionResult::RollbackRestart,
            Ok(BfRet::VmInstr(vmi)) => vmi,
        };

        trace!(?call_results, "Builtin function call complete");
        call_results
    }

    /// We're returning into a builtin function, which is all set up at the top of the stack.
    pub(crate) fn reenter_builtin_function(
        &mut self,
        exec_args: &VmExecParams,
        world_state: &mut dyn WorldState,
        session: Arc<dyn Session>,
    ) -> ExecutionResult {
        let bf_frame = match self.top().frame {
            VmStackFrame::Bf(ref frame) => frame,
            _ => panic!("Expected a BF frame at the top of the stack"),
        };

        trace!(bf_index = bf_frame.bf_index, "Reentering builtin function");
        // Functions that did not set a trampoline are assumed to be complete, so we just unwind.
        // Note: If there was an error that required unwinding, we'll have already done that, so
        // we can assume a *value* here not, an error.
        let Some(_) = bf_frame.bf_trampoline else {
            let return_value = self.top_mut().frame.return_value();

            return self.unwind_stack(FinallyReason::Return(return_value));
        };

        let bf = exec_args.builtin_registry.builtins[bf_frame.bf_index].clone();
        let verb_name = self.top().verb_name;
        let sessions = session.clone();
        let args = self.top().args.clone();
        let mut bf_args = BfCallState {
            exec_state: self,
            name: verb_name,
            world_state,
            session: sessions,
            // TODO: avoid copy here by using List inside BfCallState
            args: args.iter().collect(),
            task_scheduler_client: exec_args.task_scheduler_client.clone(),
        };

        match bf.call(&mut bf_args) {
            Ok(BfRet::Ret(result)) => self.unwind_stack(FinallyReason::Return(result.clone())),
            Err(BfErr::Code(e)) => self.push_bf_error(e, None, None),
            Err(BfErr::Raise(e, msg, value)) => self.push_bf_error(e, msg, value),

            Err(BfErr::Rollback) => ExecutionResult::RollbackRestart,
            Ok(BfRet::VmInstr(vmi)) => vmi,
        }
    }
}
