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

use crate::{
    config::FeaturesConfig,
    task_context::with_current_transaction,
    vm::{
        builtins::{BfCallState, BfErr, BfRet, BuiltinRegistry, bf_perf_counters},
        vm_host::ExecutionResult,
    },
};
use moor_vm::{Activation, FinallyReason, Frame, VMExecState};

#[cfg(feature = "trace_events")]
use crate::{trace_builtin_begin, trace_builtin_end, trace_verb_begin};

use moor_common::{
    model::{DispatchFlagsSource, ObjFlag, VerbDispatch, VerbLookup, WorldStateError},
    tasks::Session,
    util::PerfIntensity,
};
use moor_compiler::{BUILTINS, BuiltinId, Program, to_literal};
use moor_var::{
    E_INVIND, E_PERM, E_TYPE, E_VERBNF, Error, List, Obj, SYSTEM_OBJECT, Sequence, Symbol, Var,
    VarType::TYPE_NONE, Variant, program::names::GlobalName, v_empty_str, v_int, v_obj,
};
use std::sync::LazyLock;

static LIST_PROTO_SYM: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("list_proto"));
static MAP_PROTO_SYM: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("map_proto"));
static STRING_PROTO_SYM: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("str_proto"));
static INTEGER_PROTO_SYM: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("int_proto"));
static FLOAT_PROTO_SYM: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("float_proto"));
static ERROR_PROTO_SYM: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("err_proto"));
static BOOL_PROTO_SYM: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("bool_proto"));
static SYM_PROTO_SYM: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("sym_proto"));

pub use moor_vm::execute::VerbExecutionRequest;

/// The set of parameters & utilities passed to the VM for execution of a given task.
pub struct VmExecParams<'a> {
    pub builtin_registry: &'a BuiltinRegistry,
    pub max_stack_depth: usize,
    pub config: &'a FeaturesConfig,
}

/// Extension trait for VMExecState methods that require kernel-level TLS access
/// (e.g., `with_current_transaction`).
pub(crate) trait VMExecStateKernelExt {
    fn verb_dispatch(
        &mut self,
        exec_params: &VmExecParams,
        target: Var,
        verb: Symbol,
        args: List,
    ) -> Result<ExecutionResult, Error>;

    fn prepare_call_verb(
        &mut self,
        location: Obj,
        this: Var,
        verb_name: Symbol,
        args: List,
    ) -> ExecutionResult;

    fn prepare_pass_verb(&mut self, args: &List) -> ExecutionResult;

    fn exec_eval_request(
        &mut self,
        permissions: &Obj,
        player: &Obj,
        program: Program,
        initial_env: Option<&[(Symbol, Var)]>,
    );

    fn maybe_invoke_bf_proxy(
        &mut self,
        bf_override_name: Symbol,
        args: &List,
    ) -> Option<ExecutionResult>;

    fn call_builtin_function(
        &mut self,
        bf_id: BuiltinId,
        args: List,
        exec_args: &VmExecParams,
        session: &dyn Session,
    ) -> ExecutionResult;

    fn reenter_builtin_function(
        &mut self,
        exec_args: &VmExecParams,
        session: &dyn Session,
    ) -> ExecutionResult;
}

impl VMExecStateKernelExt for VMExecState {
    /// Entry point for dispatching a verb (method) call.
    /// Called from the VM execution loop for CallVerb opcodes.
    fn verb_dispatch(
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
        let prop_val = with_current_transaction(|world_state| {
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

        let lookup = with_current_transaction(|world_state| {
            let self_valid = world_state
                .valid(&location)
                .expect("Error checking object validity");
            if !self_valid {
                return None;
            }

            let verb_result = world_state.dispatch_verb(
                &self.top().permissions,
                VerbDispatch::new(
                    VerbLookup::method(&location, verb_name),
                    DispatchFlagsSource::VerbOwner,
                ),
            );
            Some(verb_result)
        });

        let Some(verb_result) = lookup else {
            return self.push_error(
                E_INVIND.with_msg(|| format!("Invalid object ({location}) for verb dispatch")),
            );
        };

        let (program_key, resolved_verb, permissions_flags) = match verb_result {
            Ok(Some(vi)) => (vi.program_key, vi.verbdef, vi.permissions_flags),
            Ok(None) => {
                return self.push_error(E_VERBNF.with_msg(|| {
                    format!(
                        "Verb {}:{} not found",
                        to_literal(&v_obj(location)),
                        verb_name,
                    )
                }));
            }
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
                panic!("dispatch_verb() should return Ok(None), not VerbNotFound");
            }
            Err(e) => {
                panic!("Unexpected error from dispatch_verb: {e:?}")
            }
        };

        // Defer program materialization/slot resolution to VmHost so it can source programs
        // from the task-owned cache.
        ExecutionResult::DispatchVerb(Box::new(VerbExecutionRequest {
            permissions: self.top().permissions,
            permissions_flags,
            resolved_verb,
            verb_name,
            this,
            player,
            args,
            caller,
            argstr: v_empty_str(),
            program_key,
        }))
    }

    /// Setup the VM to execute the verb of the same current name, but using the parent's
    /// version.
    /// TODO this should be done up in task.rs instead. let's add a new ExecutionResult for it.
    fn prepare_pass_verb(&mut self, args: &List) -> ExecutionResult {
        // get parent of verb definer object & current verb name.
        let definer = self.top().verb_definer();
        let permissions = self.top().permissions;
        let verb = self.top().verb_name;

        let lookup = with_current_transaction(|world_state| {
            let parent = world_state.parent_of(&permissions, &definer)?;
            let parent_valid = world_state
                .valid(&parent)
                .expect("Error checking object validity");
            if !parent_valid {
                return Ok(None);
            }

            let verb_result = world_state.dispatch_verb(
                &permissions,
                VerbDispatch::new(
                    VerbLookup::method(&parent, verb),
                    DispatchFlagsSource::Permissions,
                ),
            );
            Ok(Some(verb_result))
        });

        let lookup = match lookup {
            Ok(lookup) => lookup,
            Err(WorldStateError::RollbackRetry) => {
                return ExecutionResult::TaskRollbackRestart;
            }
            Err(e) => return self.raise_error(e.to_error()),
        };
        let Some(verb_result) = lookup else {
            return self.push_error(E_INVIND.msg("Invalid object for pass() verb dispatch"));
        };
        let (program_key, resolved_verb, permissions_flags) = match verb_result {
            Ok(Some(vi)) => (vi.program_key, vi.verbdef, vi.permissions_flags),
            Ok(None) => {
                return self.push_error(E_VERBNF.msg("Verb not found for pass() dispatch"));
            }
            Err(WorldStateError::RollbackRetry) => {
                return ExecutionResult::TaskRollbackRestart;
            }
            Err(e) => return self.raise_error(e.to_error()),
        };

        let caller = self.caller();
        let this = self.top().this.clone();
        let player = self.top().player;
        let args_list = args.clone();
        ExecutionResult::DispatchVerb(Box::new(VerbExecutionRequest {
            permissions,
            permissions_flags,
            resolved_verb,
            verb_name: verb,
            this,
            player,
            args: args_list,
            caller,
            argstr: v_empty_str(),
            program_key,
        }))
    }

    fn exec_eval_request(
        &mut self,
        permissions: &Obj,
        player: &Obj,
        program: Program,
        initial_env: Option<&[(Symbol, Var)]>,
    ) {
        let permissions_flags =
            with_current_transaction(|ws| ws.flags_of(permissions)).unwrap_or_default();
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
        let verb_result = with_current_transaction(|world_state| {
            world_state.dispatch_verb(
                &self.top().permissions,
                VerbDispatch::new(
                    VerbLookup::method(&SYSTEM_OBJECT, bf_override_name),
                    DispatchFlagsSource::Permissions,
                ),
            )
        })
        .ok()?;
        let verb_result = verb_result?;
        let program_key = verb_result.program_key;
        let resolved_verb = verb_result.verbdef;
        let permissions_flags = verb_result.permissions_flags;

        let player = self.top().player;
        let caller = self.caller();
        let args_list = args.clone();
        let permissions = self.top().permissions;
        Some(ExecutionResult::DispatchVerb(Box::new(
            VerbExecutionRequest {
                permissions,
                permissions_flags,
                resolved_verb,
                verb_name: bf_override_name,
                this: v_obj(SYSTEM_OBJECT),
                player,
                args: args_list,
                caller,
                argstr: v_empty_str(),
                program_key,
            },
        )))
    }

    /// Call into a builtin function.
    fn call_builtin_function(
        &mut self,
        bf_id: BuiltinId,
        args: List,
        exec_args: &VmExecParams,
        _session: &dyn Session,
    ) -> ExecutionResult {
        let bf = exec_args.builtin_registry.builtin_for(&bf_id);
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
        let bf_counter = bf_counters.counter_for(bf_id);
        bf_counter.invocations().add(1);
        let sampled_start = bf_counter.sampled_start_with_intensity(PerfIntensity::HotPath);

        let bf_result = bf(&mut bf_args);
        bf_counter.add_elapsed_sample(sampled_start);

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
    fn reenter_builtin_function(
        &mut self,
        exec_args: &VmExecParams,
        _session: &dyn Session,
    ) -> ExecutionResult {
        let bf_id = match self.top().frame {
            Frame::Bf(ref frame) => frame.bf_id,
            _ => panic!("Expected a BF frame at the top of the stack"),
        };
        let bf_counter = bf_perf_counters().counter_for(bf_id);
        let sampled_start = bf_counter.sampled_start_with_intensity(PerfIntensity::HotPath);

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
        bf_counter.add_elapsed_sample(sampled_start);

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
