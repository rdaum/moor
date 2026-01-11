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

use crate::{
    config::FeaturesConfig,
    task_context::with_current_transaction_mut,
    vm::{
        Fork,
        activation::{Activation, Frame},
        builtins::{BfCallState, BfErr, BfRet, BuiltinRegistry, bf_perf_counters},
        exec_state::VMExecState,
        vm_host::ExecutionResult,
        vm_unwind::FinallyReason,
    },
};

#[cfg(feature = "trace_events")]
use crate::{trace_builtin_begin, trace_builtin_end, trace_verb_begin};

use lazy_static::lazy_static;
use minstant::Instant;
use moor_common::{
    matching::ParsedCommand,
    model::{ObjFlag, VerbDef, WorldStateError},
    tasks::Session,
    util::BitEnum,
};
use moor_compiler::{BUILTINS, BuiltinId, Program, to_literal};
use moor_var::{
    E_INVIND, E_PERM, E_TYPE, E_VERBNF, Error, List, NOTHING, Obj, SYSTEM_OBJECT, Sequence, Symbol,
    Var,
    VarType::TYPE_NONE,
    Variant,
    program::{ProgramType, names::GlobalName},
    v_empty_str, v_int, v_obj, v_string,
};

lazy_static! {
    static ref LIST_PROTO_SYM: Symbol = Symbol::mk("list_proto");
    static ref MAP_PROTO_SYM: Symbol = Symbol::mk("map_proto");
    static ref STRING_PROTO_SYM: Symbol = Symbol::mk("str_proto");
    static ref INTEGER_PROTO_SYM: Symbol = Symbol::mk("int_proto");
    static ref FLOAT_PROTO_SYM: Symbol = Symbol::mk("float_proto");
    static ref ERROR_PROTO_SYM: Symbol = Symbol::mk("err_proto");
    static ref BOOL_PROTO_SYM: Symbol = Symbol::mk("bool_proto");
    static ref SYM_PROTO_SYM: Symbol = Symbol::mk("sym_proto");
}

/// The set of parameters for a scheduler-requested *resolved* verb method dispatch.
#[derive(Debug, Clone, PartialEq)]
pub struct VerbExecutionRequest {
    /// The applicable permissions.
    pub permissions: Obj,
    /// The resolved verb.
    pub resolved_verb: VerbDef,
    /// Verb name
    pub verb_name: Symbol,
    /// This object
    pub this: Var,
    /// Player
    pub player: Obj,
    /// Arguments
    pub args: List,
    /// Caller
    pub caller: Var,
    /// Argument string
    pub argstr: Var,
    /// The decoded MOO Binary that contains the verb to be executed.
    pub program: ProgramType,
}

/// The set of parameters for a command verb dispatch with full command environment.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandVerbExecutionRequest {
    /// The applicable permissions.
    pub permissions: Obj,
    /// The resolved verb.
    pub resolved_verb: VerbDef,
    /// Verb name
    pub verb_name: Symbol,
    /// This object
    pub this: Var,
    /// Player
    pub player: Obj,
    /// Arguments
    pub args: List,
    /// Caller
    pub caller: Var,
    /// Argument string
    pub argstr: Var,
    /// The parsed command with dobj, iobj, prep, etc.
    pub command: ParsedCommand,
    /// The decoded MOO Binary that contains the verb to be executed.
    pub program: ProgramType,
}

/// The set of parameters & utilities passed to the VM for execution of a given task.
pub struct VmExecParams<'a> {
    pub builtin_registry: &'a BuiltinRegistry,
    pub max_stack_depth: usize,
    pub config: &'a FeaturesConfig,
}

impl VMExecState {
    /// Entry point for dispatching a verb (method) call.
    /// Called from the VM execution loop for CallVerb opcodes.
    pub(crate) fn verb_dispatch(
        &mut self,
        exec_params: &VmExecParams,
        target: Var,
        verb: Symbol,
        args: List,
    ) -> Result<ExecutionResult, Error> {
        // Fast path: Obj is by far the most common case for verb dispatch
        if let Some(o) = target.as_object() {
            return Ok(self.prepare_call_verb(o, target, verb, args));
        }

        // Flyweight dispatches to its delegate
        if let Some(f) = target.as_flyweight() {
            return Ok(self.prepare_call_verb(*f.delegate(), target, verb, args));
        }

        // Primitive dispatch (int, string, float are most common)
        if !exec_params.config.type_dispatch {
            return Err(E_TYPE.with_msg(|| {
                format!("Invalid target {:?} for verb dispatch", target.type_code())
            }));
        }

        // For primitives, look at type and dispatch to corresponding sysprop
        // e.g. "blah":reverse() becomes $string:reverse("blah")
        // Check common types first with direct accessors
        let sysprop_sym = if target.is_int() {
            *INTEGER_PROTO_SYM
        } else if target.is_string() {
            *STRING_PROTO_SYM
        } else if target.is_float() {
            *FLOAT_PROTO_SYM
        } else if target.is_list() {
            *LIST_PROTO_SYM
        } else {
            // Less common types - use variant()
            match target.variant() {
                Variant::Map(_) => *MAP_PROTO_SYM,
                Variant::Err(_) => *ERROR_PROTO_SYM,
                Variant::Sym(_) => *SYM_PROTO_SYM,
                Variant::Bool(_) => *BOOL_PROTO_SYM,
                _ => {
                    return Err(E_TYPE.with_msg(|| {
                        format!(
                            "Invalid target for verb dispatch: {}",
                            target.type_code().to_literal()
                        )
                    }));
                }
            }
        };
        let perms = self.top().permissions;
        let prop_val = with_current_transaction_mut(|world_state| {
            match world_state.retrieve_property(&perms, &SYSTEM_OBJECT, sysprop_sym) {
                Ok(prop_val) => Ok(prop_val),
                Err(e) => Err(e.to_error()),
            }
        })?;
        let Some(prop_val) = prop_val.as_object() else {
            return Err(E_TYPE.with_msg(|| {
                format!(
                    "Invalid target for verb dispatch: {}",
                    prop_val.type_code().to_literal()
                )
            }));
        };
        let arguments = args
            .insert(0, &target)
            .expect("Failed to insert object for dispatch");
        let Some(arguments) = arguments.as_list() else {
            return Err(E_TYPE.with_msg(|| {
                format!(
                    "Invalid arguments for verb dispatch: {}",
                    arguments.type_code().to_literal()
                )
            }));
        };
        Ok(self.prepare_call_verb(prop_val, v_obj(prop_val), verb, arguments.clone()))
    }

    fn prepare_call_verb(
        &mut self,
        location: Obj,
        this: Var,
        verb_name: Symbol,
        args: List,
    ) -> ExecutionResult {
        let caller = self.caller();

        // Only wizards can propagate a modified player value to called verbs.
        let activation_player = self.top().player;
        let player = if let Frame::Moo(frame) = &self.top().frame {
            frame
                .get_gvar(GlobalName::player)
                .and_then(|v| v.as_object())
                .filter(|fp| fp != &activation_player)
                .map_or(activation_player, |fp| {
                    let is_wiz = self.task_perms_flags().contains(ObjFlag::Wizard);
                    if is_wiz { fp } else { activation_player }
                })
        } else {
            activation_player
        };

        let self_valid = with_current_transaction_mut(|world_state| world_state.valid(&location))
            .expect("Error checking object validity");
        if !self_valid {
            return self.push_error(
                E_INVIND.with_msg(|| format!("Invalid object ({location}) for verb dispatch")),
            );
        }
        // Find the callable verb ...
        let verb_result = with_current_transaction_mut(|world_state| {
            world_state.find_method_verb_on(&self.top().permissions, &location, verb_name)
        });

        let (program, resolved_verb) = match verb_result {
            Ok(vi) => vi,
            Err(WorldStateError::ObjectPermissionDenied) => {
                return self.push_error(E_PERM.into());
            }
            Err(WorldStateError::RollbackRetry) => {
                return ExecutionResult::TaskRollbackRestart;
            }
            Err(WorldStateError::VerbPermissionDenied) => {
                return self.push_error(E_PERM.into());
            }
            Err(WorldStateError::VerbNotFound(_, _)) => {
                return self.push_error(E_VERBNF.with_msg(|| {
                    format!(
                        "Verb {}:{} not found",
                        to_literal(&v_obj(location)),
                        verb_name,
                    )
                }));
            }
            Err(e) => {
                panic!("Unexpected error from find_method_verb_on: {e:?}")
            }
        };

        // Permissions for the activation are the verb's owner.
        let permissions_flags =
            with_current_transaction_mut(|ws| ws.flags_of(&resolved_verb.owner()))
                .unwrap_or_default();
        self.exec_call_request(
            permissions_flags,
            resolved_verb,
            verb_name,
            this,
            player,
            args,
            caller,
            v_empty_str(),
            program,
        );
        ExecutionResult::More
    }

    /// Setup the VM to execute the verb of the same current name, but using the parent's
    /// version.
    /// TODO this should be done up in task.rs instead. let's add a new ExecutionResult for it.
    pub(crate) fn prepare_pass_verb(&mut self, args: &List) -> ExecutionResult {
        // get parent of verb definer object & current verb name.
        let definer = self.top().verb_definer();
        let permissions = &self.top().permissions;

        let parent_result = with_current_transaction_mut(|world_state| {
            world_state.parent_of(permissions, &definer)
        });
        let parent = match parent_result {
            Ok(parent) => parent,
            Err(WorldStateError::RollbackRetry) => {
                return ExecutionResult::TaskRollbackRestart;
            }
            Err(e) => return self.raise_error(e.to_error()),
        };
        let verb = self.top().verb_name;

        // if `parent` is not a valid object, raise E_INVIND
        let parent_valid = with_current_transaction_mut(|world_state| world_state.valid(&parent))
            .expect("Error checking object validity");
        if !parent_valid {
            return self.push_error(E_INVIND.msg("Invalid object for pass() verb dispatch"));
        }

        // call verb on parent, but with our current 'this'
        let verb_result = with_current_transaction_mut(|world_state| {
            world_state.find_method_verb_on(permissions, &parent, verb)
        });
        let (program, resolved_verb) = match verb_result {
            Ok(vi) => vi,
            Err(WorldStateError::RollbackRetry) => {
                return ExecutionResult::TaskRollbackRestart;
            }
            Err(e) => return self.raise_error(e.to_error()),
        };

        let caller = self.caller();
        let this = self.top().this.clone();
        let player = self.top().player;
        let args_list = args.iter().collect();

        ExecutionResult::DispatchVerb(Box::new(VerbExecutionRequest {
            permissions: *permissions,
            resolved_verb,
            verb_name: verb,
            this,
            player,
            args: args_list,
            caller,
            argstr: v_empty_str(),
            program,
        }))
    }

    /// Entry point from scheduler for beginning the dispatch of an initial command verb execution.
    /// This sets up the initial activation with parsing variables from the parsed command.
    #[allow(clippy::too_many_arguments)]
    pub fn exec_command_request(
        &mut self,
        permissions_flags: BitEnum<ObjFlag>,
        resolved_verb: VerbDef,
        verb_name: Symbol,
        this: Var,
        player: Obj,
        args: List,
        caller: Var,
        argstr: Var,
        command: &ParsedCommand,
        program: ProgramType,
    ) {
        // Initial command activation - no parent to inherit from
        let mut a = Activation::for_call(
            resolved_verb,
            permissions_flags,
            verb_name,
            this,
            player,
            args,
            caller,
            argstr,
            None,
            program,
        );

        // Set parsing variables from the parsed command
        a.frame
            .set_global_variable(GlobalName::argstr, v_string(command.argstr.clone()));
        a.frame
            .set_global_variable(GlobalName::dobj, v_obj(command.dobj.unwrap_or(NOTHING)));
        a.frame.set_global_variable(
            GlobalName::dobjstr,
            command
                .dobjstr
                .as_ref()
                .map_or_else(v_empty_str, |s| v_string(s.clone())),
        );
        a.frame.set_global_variable(
            GlobalName::prepstr,
            command
                .prepstr
                .as_ref()
                .map_or_else(v_empty_str, |s| v_string(s.clone())),
        );
        a.frame
            .set_global_variable(GlobalName::iobj, v_obj(command.iobj.unwrap_or(NOTHING)));
        a.frame.set_global_variable(
            GlobalName::iobjstr,
            command
                .iobjstr
                .as_ref()
                .map_or_else(v_empty_str, |s| v_string(s.clone())),
        );

        self.stack.push(a);

        // Emit VerbBegin trace event if this is a MOO verb
        #[cfg(feature = "trace_events")]
        if let Frame::Moo(_) = self.top().frame {
            // No calling line number for initial command activation
            trace_verb_begin!(
                self.task_id,
                &self.top().verb_name.as_string(),
                &self.top().this,
                &self.top().verb_definer(),
                None,
                &self.top().args
            );
        }
    }

    /// Entry point from scheduler for actually beginning the dispatch of a method execution
    /// (verb-to-verb call) in this VM.
    /// Actually creates the activation record and puts it on the stack.
    #[allow(clippy::too_many_arguments)]
    pub fn exec_call_request(
        &mut self,
        permissions_flags: BitEnum<ObjFlag>,
        resolved_verb: VerbDef,
        verb_name: Symbol,
        this: Var,
        player: Obj,
        args: List,
        caller: Var,
        argstr: Var,
        program: ProgramType,
    ) {
        // Get current activation to inherit global variables from, if any.
        let current_activation = self.stack.last();

        let a = Activation::for_call(
            resolved_verb,
            permissions_flags,
            verb_name,
            this,
            player,
            args,
            caller,
            argstr,
            current_activation,
            program,
        );
        self.stack.push(a);

        // Emit VerbBegin trace event if this is a MOO verb
        #[cfg(feature = "trace_events")]
        if let Frame::Moo(_) = self.top().frame {
            // Capture calling line number from the previous frame (before this push)
            let calling_line = if self.stack.len() > 1 {
                self.stack[self.stack.len() - 2].frame.find_line_no()
            } else {
                None
            };

            trace_verb_begin!(
                self.task_id,
                &self.top().verb_name.as_string(),
                &self.top().this,
                &self.top().verb_definer(),
                calling_line,
                &self.top().args
            );
        }
    }

    pub fn exec_eval_request(
        &mut self,
        permissions: &Obj,
        player: &Obj,
        program: Program,
        initial_env: Option<&[(Symbol, Var)]>,
    ) {
        let permissions_flags =
            with_current_transaction_mut(|ws| ws.flags_of(permissions)).unwrap_or_default();
        let a = Activation::for_eval(
            *permissions,
            permissions_flags,
            player,
            program,
            initial_env,
        );
        self.stack.push(a);

        // Emit VerbBegin trace event if this is a MOO eval
        #[cfg(feature = "trace_events")]
        if let Frame::Moo(_) = self.top().frame {
            // Capture calling line number from the previous frame (before this push)
            let calling_line = if self.stack.len() > 1 {
                self.stack[self.stack.len() - 2].frame.find_line_no()
            } else {
                None
            };

            trace_verb_begin!(
                self.task_id,
                &self.top().verb_name.as_string(),
                &self.top().this,
                &self.top().verb_definer(),
                calling_line,
                &self.top().args
            );
        }
    }

    /// Execute a lambda call by creating a new lambda activation
    pub fn exec_lambda_request(
        &mut self,
        lambda: moor_var::Lambda,
        args: List,
    ) -> Result<(), Error> {
        // Get current activation before borrowing self immutably
        let current_activation = self.top();
        let a = Activation::for_lambda_call(&lambda, current_activation, args.iter().collect())?;
        self.stack.push(a);

        // Emit VerbBegin trace event if this is a MOO lambda
        #[cfg(feature = "trace_events")]
        if let Frame::Moo(_) = self.top().frame {
            // Capture calling line number from the previous frame (before this push)
            let calling_line = if self.stack.len() > 1 {
                self.stack[self.stack.len() - 2].frame.find_line_no()
            } else {
                None
            };

            trace_verb_begin!(
                self.task_id,
                &self.top().verb_name.as_string(),
                &self.top().this,
                &self.top().verb_definer(),
                calling_line,
                &self.top().args
            );
        }
        Ok(())
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
        let Frame::Moo(ref mut frame) = a.frame else {
            panic!("Attempt to fork a non-MOO frame");
        };

        frame.switch_to_fork_vector(fork_request.fork_vector_offset);
        if let Some(task_id_name) = fork_request.task_id {
            frame.set_variable(&task_id_name, v_int(self.task_id as i64));
        }

        // TODO how to set the task_id in the parent activation, as we no longer have a reference
        //  to it?
        self.stack = vec![a];

        // Emit VerbBegin trace event for forked MOO verb
        #[cfg(feature = "trace_events")]
        {
            // For forked tasks, we capture the line number from the original activation
            // before it was modified by switch_to_fork_vector
            let calling_line = self.top().frame.find_line_no();

            trace_verb_begin!(
                self.task_id,
                &self.top().verb_name.as_string(),
                &self.top().this,
                &self.top().verb_definer(),
                calling_line,
                &self.top().args
            );
        }
    }

    /// If a bf_<xxx> wrapper function is present on #0, invoke that instead.
    fn maybe_invoke_bf_proxy(
        &mut self,
        bf_override_name: Symbol,
        args: &List,
    ) -> Option<ExecutionResult> {
        // Reject invocations of maybe-wrapper functions if the caller is #0.
        // This prevents recursion through them.
        // TODO: This is a copy of LambdaMOO's logic, and is maybe a bit over-zealous as it will prevent
        //  one wrapped builtin from calling the wrapper on another builtin. We can revist later.

        if self.caller() == v_obj(SYSTEM_OBJECT) {
            return None;
        }

        // Look for it...
        let (program, resolved_verb) = with_current_transaction_mut(|world_state| {
            world_state.find_method_verb_on(
                &self.top().permissions,
                &SYSTEM_OBJECT,
                bf_override_name,
            )
        })
        .ok()?;

        let player = self.top().player;
        let caller = self.caller();
        let args_list = args.iter().collect();

        Some(ExecutionResult::DispatchVerb(Box::new(
            VerbExecutionRequest {
                permissions: self.top().permissions,
                resolved_verb,
                verb_name: bf_override_name,
                this: v_obj(SYSTEM_OBJECT),
                player,
                args: args_list,
                caller,
                argstr: v_empty_str(),
                program,
            },
        )))
    }

    /// Call into a builtin function.
    pub(crate) fn call_builtin_function(
        &mut self,
        bf_id: BuiltinId,
        args: List,
        exec_args: &VmExecParams,
        _session: &dyn Session,
    ) -> ExecutionResult {
        let bf = exec_args.builtin_registry.builtin_for(&bf_id);
        let start = Instant::now();
        let bf_desc = BUILTINS.description_for(bf_id).expect("Builtin not found");
        let bf_name = bf_desc.name;

        // Emit builtin begin trace event
        #[cfg(feature = "trace_events")]
        {
            // Capture calling line number before pushing builtin activation
            let calling_line = if !self.stack.is_empty() {
                self.top().frame.find_line_no()
            } else {
                None
            };
            trace_builtin_begin!(self.task_id, bf_name, calling_line, &args);
        }

        // TODO: check for $server_options.protect_[func]
        // Check for builtin override at #0.
        if let Some(proxy_result) = self.maybe_invoke_bf_proxy(bf_desc.bf_override_name, &args) {
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
            self.top().player,
        ));
        let mut bf_args = BfCallState {
            exec_state: self,
            name: bf_name,
            args: &args,
            config: exec_args.config,
        };
        let bf_counters = bf_perf_counters();
        bf_counters.counter_for(bf_id).invocations().add(1);

        let bf_result = bf(&mut bf_args);
        let elapsed_nanos = start.elapsed().as_nanos();
        bf_counters
            .counter_for(bf_id)
            .cumulative_duration_nanos()
            .add(elapsed_nanos as isize);

        match bf_result {
            Ok(BfRet::Ret(result)) => {
                debug_assert_ne!(
                    result.type_code(),
                    TYPE_NONE,
                    "Builtin {bf_name} returned TYPE_NONE"
                );
                self.unwind_stack(FinallyReason::Return(result))
            }
            Ok(BfRet::RetNil) => self.unwind_stack(FinallyReason::Return(v_int(0))),
            Err(BfErr::ErrValue(e)) => self.push_error(e),
            Err(BfErr::Code(c)) => self.push_error(c.into()),
            Err(BfErr::Raise(e)) => self.push_error(e),
            Err(BfErr::Rollback) => ExecutionResult::TaskRollbackRestart,
            Ok(BfRet::VmInstr(vmi)) => vmi,
        }
    }

    /// We're returning into a builtin function, which is all set up at the top of the stack.
    pub(crate) fn reenter_builtin_function(
        &mut self,
        exec_args: &VmExecParams,
        _session: &dyn Session,
    ) -> ExecutionResult {
        let start = Instant::now();
        let bf_id = match self.top().frame {
            Frame::Bf(ref frame) => frame.bf_id,
            _ => panic!("Expected a BF frame at the top of the stack"),
        };

        // Functions that did not set a trampoline are assumed to be complete, so we just unwind.
        // Note: If there was an error that required unwinding, we'll have already done that, so
        // we can assume a *value* here not, an error.
        let has_trampoline = match self.top().frame {
            Frame::Bf(ref frame) => frame.bf_trampoline.is_some(),
            _ => false,
        };

        if !has_trampoline {
            let return_value = self.top_mut().frame.return_value();

            // Emit builtin end trace event for non-trampoline builtins
            #[cfg(feature = "trace_events")]
            {
                let bf_desc = BUILTINS.description_for(bf_id).expect("Builtin not found");
                trace_builtin_end!(self.task_id, bf_desc.name);
            }

            return self.unwind_stack(FinallyReason::Return(return_value));
        }

        let bf = exec_args.builtin_registry.builtin_for(&bf_id);
        let verb_name = self.top().verb_name;
        let args = self.top().args.clone();

        let mut bf_args = BfCallState {
            exec_state: self,
            name: verb_name,
            // TODO: avoid copy here by using List inside BfCallState
            args: &args,
            config: exec_args.config,
        };

        let bf_result = bf(&mut bf_args);
        let elapsed_nanos = start.elapsed().as_nanos();
        let bf_counters = bf_perf_counters();
        bf_counters
            .counter_for(bf_id)
            .cumulative_duration_nanos()
            .add(elapsed_nanos as isize);

        // Emit builtin end trace event
        #[cfg(feature = "trace_events")]
        {
            let bf_desc = BUILTINS.description_for(bf_id).expect("Builtin not found");
            trace_builtin_end!(self.task_id, bf_desc.name);
        }

        match bf_result {
            Ok(BfRet::Ret(result)) => self.unwind_stack(FinallyReason::Return(result.clone())),
            Ok(BfRet::RetNil) => self.unwind_stack(FinallyReason::Return(v_int(0))),
            Err(BfErr::Code(c)) => self.push_error(c.into()),
            Err(BfErr::ErrValue(e)) => self.push_error(e),
            Err(BfErr::Raise(e)) => self.push_error(e),
            Err(BfErr::Rollback) => ExecutionResult::TaskRollbackRestart,
            Ok(BfRet::VmInstr(vmi)) => vmi,
        }
    }
}
