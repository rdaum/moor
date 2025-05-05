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

use crate::builtins::{BfCallState, BfErr, BfRet, BuiltinRegistry, bf_perf_counters};
use crate::config::FeaturesConfig;
use crate::tasks::VerbCall;
use crate::tasks::sessions::Session;
use crate::tasks::task_scheduler_client::TaskSchedulerClient;
use crate::vm::VMExecState;
use crate::vm::activation::{Activation, Frame};
use crate::vm::exec_state::vm_counters;
use crate::vm::vm_unwind::FinallyReason;
use crate::vm::{ExecutionResult, Fork};
use lazy_static::lazy_static;
use moor_common::matching::ParsedCommand;
use moor_common::model::VerbDef;
use moor_common::model::WorldState;
use moor_common::model::WorldStateError;
use moor_common::util::PerfTimerGuard;
use moor_compiler::{BUILTINS, BuiltinId, Program};
use moor_var::Error::{E_INVIND, E_PERM, E_TYPE, E_VERBNF};
use moor_var::{Error, SYSTEM_OBJECT, Sequence, Symbol, Variant};
use moor_var::{List, Obj};
use moor_var::{Var, v_int, v_obj};
use std::sync::Arc;
use std::time::Instant;

lazy_static! {
    static ref LIST_SYM: Symbol = Symbol::mk("list");
    static ref MAP_SYM: Symbol = Symbol::mk("map");
    static ref STRING_SYM: Symbol = Symbol::mk("string");
    static ref INTEGER_SYM: Symbol = Symbol::mk("integer");
    static ref FLOAT_SYM: Symbol = Symbol::mk("float");
    static ref ERROR_SYM: Symbol = Symbol::mk("error");
    static ref BOOL_SYM: Symbol = Symbol::mk("boolean");
    static ref SYM_SYM: Symbol = Symbol::mk("symbol");
    static ref FLYWEIGHT_SYM: Symbol = Symbol::mk("flyweight");
}

/// The set of parameters for a scheduler-requested *resolved* verb method dispatch.
#[derive(Debug, Clone, PartialEq)]
pub struct VerbExecutionRequest {
    /// The applicable permissions.
    pub permissions: Obj,
    /// The resolved verb.
    pub resolved_verb: VerbDef,
    /// The call parameters that were used to resolve the verb.
    pub call: VerbCall,
    /// The parsed user command that led to this verb dispatch, if any.
    pub command: Option<ParsedCommand>,
    /// The decoded MOO Binary that contains the verb to be executed.
    pub program: VerbProgram,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VerbProgram {
    Moo(Box<Program>),
}

/// The set of parameters & utilities passed to the VM for execution of a given task.
pub struct VmExecParams {
    pub task_scheduler_client: TaskSchedulerClient,
    pub builtin_registry: Arc<BuiltinRegistry>,
    pub max_stack_depth: usize,
    pub config: FeaturesConfig,
}

impl VMExecState {
    /// Entry point for dispatching a verb (method) call.
    /// Called from the VM execution loop for CallVerb opcodes.
    pub(crate) fn verb_dispatch(
        &mut self,
        exec_params: &VmExecParams,
        world_state: &mut dyn WorldState,
        target: Var,
        verb: Symbol,
        args: List,
    ) -> Result<ExecutionResult, Error> {
        let vm_counters = vm_counters();
        let _t = PerfTimerGuard::new(&vm_counters.prepare_verb_dispatch);
        let (args, this, location) = match target.variant() {
            Variant::Obj(o) => (args, target.clone(), o.clone()),
            Variant::Flyweight(f) => (args, target.clone(), f.delegate().clone()),
            non_obj => {
                if !exec_params.config.type_dispatch {
                    return Err(E_TYPE);
                }
                // If the object is not an object or frob, it's a primitive.
                // For primitives, we look at its type, and look for a
                // sysprop that corresponds, then dispatch to that, with the object as the
                // first argument.
                // e.g. "blah":reverse() becomes $string:reverse("blah")
                let sysprop_sym = match non_obj {
                    Variant::Int(_) => *INTEGER_SYM,
                    Variant::Float(_) => *FLOAT_SYM,
                    Variant::Str(_) => *STRING_SYM,
                    Variant::List(_) => *LIST_SYM,
                    Variant::Map(_) => *MAP_SYM,
                    Variant::Err(_) => *ERROR_SYM,
                    Variant::Flyweight(_) => *FLYWEIGHT_SYM,
                    Variant::Sym(_) => *SYM_SYM,
                    Variant::Bool(_) => *BOOL_SYM,
                    _ => {
                        return Err(E_TYPE);
                    }
                };
                let perms = self.top().permissions.clone();
                let prop_val =
                    match world_state.retrieve_property(&perms, &SYSTEM_OBJECT, sysprop_sym) {
                        Ok(prop_val) => prop_val,
                        Err(e) => {
                            return Err(e.to_error_code());
                        }
                    };
                let Variant::Obj(prop_val) = prop_val.variant() else {
                    return Err(E_TYPE);
                };
                let arguments = args
                    .insert(0, &target)
                    .expect("Failed to insert object for dispatch");
                let Variant::List(arguments) = arguments.variant() else {
                    return Err(E_TYPE);
                };
                (arguments.clone(), v_obj(prop_val.clone()), prop_val.clone())
            }
        };
        Ok(self.prepare_call_verb(world_state, location, this, verb, args.clone()))
    }

    fn prepare_call_verb(
        &mut self,
        world_state: &mut dyn WorldState,
        location: Obj,
        this: Var,
        verb_name: Symbol,
        args: List,
    ) -> ExecutionResult {
        let call = VerbCall {
            verb_name,
            location: v_obj(location.clone()),
            this: this.clone(),
            player: self.top().player.clone(),
            args: args.iter().collect(),
            // caller her is current-activation 'this', not activation caller() ...
            // unless we're a builtin, in which case we're #-1.
            argstr: "".to_string(),
            caller: self.caller(),
        };

        let self_valid = world_state
            .valid(&location)
            .expect("Error checking object validity");
        if !self_valid {
            return self.push_error(E_INVIND);
        }
        // Find the callable verb ...
        let (binary, resolved_verb) =
            match world_state.find_method_verb_on(&self.top().permissions, &location, verb_name) {
                Ok(vi) => vi,
                Err(WorldStateError::ObjectPermissionDenied) => {
                    return self.push_error(E_PERM);
                }
                Err(WorldStateError::RollbackRetry) => {
                    return ExecutionResult::TaskRollbackRestart;
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
        let permissions = resolved_verb.owner();

        ExecutionResult::DispatchVerb {
            permissions,
            resolved_verb,
            binary,
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
        let vm_counters = vm_counters();
        let _t = PerfTimerGuard::new(&vm_counters.prepare_pass_verb);
        // get parent of verb definer object & current verb name.
        let definer = self.top().verb_definer();
        let permissions = &self.top().permissions;

        let parent = match world_state.parent_of(permissions, &definer) {
            Ok(parent) => parent,
            Err(WorldStateError::RollbackRetry) => {
                return ExecutionResult::TaskRollbackRestart;
            }
            Err(e) => return self.raise_error(e.to_error_code()),
        };
        let verb = self.top().verb_name;

        // if `parent` is not a valid object, raise E_INVIND
        if !world_state
            .valid(&parent)
            .expect("Error checking object validity")
        {
            return self.push_error(E_INVIND);
        }

        // call verb on parent, but with our current 'this'
        let (binary, resolved_verb) =
            match world_state.find_method_verb_on(permissions, &parent, verb) {
                Ok(vi) => vi,
                Err(WorldStateError::RollbackRetry) => {
                    return ExecutionResult::TaskRollbackRestart;
                }
                Err(e) => return self.raise_error(e.to_error_code()),
            };

        let caller = self.caller();
        let call = VerbCall {
            verb_name: verb,
            location: v_obj(parent),
            this: self.top().this.clone(),
            player: self.top().player.clone(),
            args: args.iter().collect(),
            argstr: "".to_string(),
            caller,
        };

        ExecutionResult::DispatchVerb {
            permissions: permissions.clone(),
            resolved_verb,
            binary,
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

    pub fn exec_eval_request(&mut self, permissions: &Obj, player: &Obj, program: Box<Program>) {
        let a = Activation::for_eval(permissions.clone(), player, program);

        self.stack.push(a);
    }

    /// Prepare a new stack & call hierarchy for invocation of a forked task.
    /// Called (ultimately) from the scheduler as the result of a fork() call.
    /// We get an activation record which is a copy of where it was borked from, and a new Program
    /// which is the new task's code, derived from a fork vector in the original task.
    pub(crate) fn exec_fork_vector(&mut self, fork_request: Fork) {
        let vm_counters = vm_counters();
        let _t = PerfTimerGuard::new(&vm_counters.prepare_exec_fork_vector);
        // Set the activation up with the new task ID, and the new code.
        let mut a = fork_request.activation;

        // This makes sense only for a MOO stack frame, and could only be initiated from there,
        // so anything else is a legit panic, we shouldn't have gotten here.
        let Frame::Moo(ref mut frame) = a.frame else {
            panic!("Attempt to fork a non-MOO frame");
        };

        frame.program.main_vector = Arc::new(
            frame.program.fork_vectors[fork_request.fork_vector_offset.0 as usize].clone(),
        );
        frame.pc = 0;
        if let Some(task_id_name) = fork_request.task_id {
            frame.set_variable(&task_id_name, v_int(self.task_id as i64));
        }

        // TODO how to set the task_id in the parent activation, as we no longer have a reference
        // to it?
        self.stack = vec![a];
    }

    /// If a bf_<xxx> wrapper function is present on #0, invoke that instead.
    fn maybe_invoke_bf_proxy(
        &mut self,
        bf_name: Symbol,
        args: &List,
        world_state: &mut dyn WorldState,
    ) -> Option<ExecutionResult> {
        // Reject invocations of maybe-wrapper functions if the caller is #0.
        // This prevents recursion through them.
        // TODO: This is a copy of LambdaMOO's logic, and is maybe a bit over-zealous as it will prevent
        //  one wrapped builtin from calling the wrapper on another builtin. We can revist later.

        if self.caller() == v_obj(SYSTEM_OBJECT) {
            return None;
        }

        let bf_override_name = Symbol::mk(&format!("bf_{}", bf_name));

        // Look for it...
        let (binary, resolved_verb) = world_state
            .find_method_verb_on(&self.top().permissions, &SYSTEM_OBJECT, bf_override_name)
            .ok()?;

        let call = VerbCall {
            verb_name: bf_override_name,
            location: v_obj(resolved_verb.location()),
            this: v_obj(SYSTEM_OBJECT),
            player: self.top().player.clone(),
            args: args.iter().collect(),
            argstr: "".to_string(),
            caller: self.caller(),
        };
        Some(ExecutionResult::DispatchVerb {
            permissions: self.top().permissions.clone(),
            resolved_verb,
            binary,
            call,
            command: self.top().command.clone(),
        })
    }

    /// Call into a builtin function.
    pub(crate) fn call_builtin_function(
        &mut self,
        bf_id: BuiltinId,
        args: List,
        exec_args: &VmExecParams,
        world_state: &mut dyn WorldState,
        session: Arc<dyn Session>,
    ) -> ExecutionResult {
        let bf = exec_args.builtin_registry.builtin_for(&bf_id);
        let start = Instant::now();
        let bf_desc = BUILTINS.description_for(bf_id).expect("Builtin not found");
        let bf_name = bf_desc.name;

        // TODO: check for $server_options.protect_[func]
        // Check for builtin override at #0.
        if let Some(proxy_result) = self.maybe_invoke_bf_proxy(bf_name, &args, world_state) {
            return proxy_result;
        }

        // Push an activation frame for the builtin function.
        let flags = self.top().verbdef.flags();
        self.stack.push(Activation::for_bf_call(
            bf_id,
            bf_name,
            args.clone(),
            // We copy the flags from the calling verb, that will determine error handling 'd'
            // behaviour below.
            flags,
            self.top().player.clone(),
        ));
        let vm_counters = vm_counters();
        let mut bf_args = BfCallState {
            exec_state: self,
            name: bf_name,
            world_state,
            session: session.clone(),
            args,
            task_scheduler_client: exec_args.task_scheduler_client.clone(),
            config: exec_args.config.clone(),
        };
        let bf_counters = bf_perf_counters();
        bf_counters.counter_for(bf_id).invocations.add(1);
        let elapsed_nanos = start.elapsed().as_nanos();
        vm_counters.prepare_builtin_function.invocations.add(1);
        vm_counters
            .prepare_builtin_function
            .cumulative_duration_nanos
            .add(elapsed_nanos as isize);

        let result = bf.call(&mut bf_args);
        let elapsed_nanos = start.elapsed().as_nanos();
        bf_counters
            .counter_for(bf_id)
            .cumulative_duration_nanos
            .add(elapsed_nanos as isize);
        match result {
            Ok(BfRet::Ret(result)) => self.unwind_stack(FinallyReason::Return(result.clone())),
            Err(BfErr::Code(e)) => self.push_bf_error(e, None, None),
            Err(BfErr::Raise(e, msg, value)) => self.push_bf_error(e, msg, value),
            Err(BfErr::Rollback) => ExecutionResult::TaskRollbackRestart,
            Ok(BfRet::VmInstr(vmi)) => vmi,
        }
    }

    /// We're returning into a builtin function, which is all set up at the top of the stack.
    pub(crate) fn reenter_builtin_function(
        &mut self,
        exec_args: &VmExecParams,
        world_state: &mut dyn WorldState,
        session: Arc<dyn Session>,
    ) -> ExecutionResult {
        let start = Instant::now();
        let bf_frame = match self.top().frame {
            Frame::Bf(ref frame) => frame,
            _ => panic!("Expected a BF frame at the top of the stack"),
        };

        // Functions that did not set a trampoline are assumed to be complete, so we just unwind.
        // Note: If there was an error that required unwinding, we'll have already done that, so
        // we can assume a *value* here not, an error.
        let Some(_) = bf_frame.bf_trampoline else {
            let return_value = self.top_mut().frame.return_value();

            return self.unwind_stack(FinallyReason::Return(return_value));
        };

        let bf_id = bf_frame.bf_id;
        let bf = exec_args.builtin_registry.builtin_for(&bf_id);
        let verb_name = self.top().verb_name;
        let sessions = session.clone();
        let args = self.top().args.clone();
        let mut bf_args = BfCallState {
            exec_state: self,
            name: verb_name,
            world_state,
            session: sessions,
            // TODO: avoid copy here by using List inside BfCallState
            args,
            task_scheduler_client: exec_args.task_scheduler_client.clone(),
            config: exec_args.config.clone(),
        };

        let elapsed_nanos = start.elapsed().as_nanos();
        vm_counters()
            .prepare_reenter_builtin_function
            .invocations
            .add(1);
        vm_counters()
            .prepare_reenter_builtin_function
            .cumulative_duration_nanos
            .add(elapsed_nanos as isize);

        let result = bf.call(&mut bf_args);
        let elapsed_nanos = start.elapsed().as_nanos();
        let bf_counters = bf_perf_counters();
        bf_counters
            .counter_for(bf_id)
            .cumulative_duration_nanos
            .add(elapsed_nanos as isize);
        match result {
            Ok(BfRet::Ret(result)) => self.unwind_stack(FinallyReason::Return(result.clone())),
            Err(BfErr::Code(e)) => self.push_bf_error(e, None, None),
            Err(BfErr::Raise(e, msg, value)) => self.push_bf_error(e, msg, value),

            Err(BfErr::Rollback) => ExecutionResult::TaskRollbackRestart,
            Ok(BfRet::VmInstr(vmi)) => vmi,
        }
    }
}
