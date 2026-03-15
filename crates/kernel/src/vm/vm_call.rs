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
    vm::builtins::{BfCallState, BfErr, BfRet, BuiltinRegistry, bf_perf_counters},
};
use moor_vm::{Activation, ExecState, FinallyReason, Frame};

#[cfg(feature = "trace_events")]
use crate::{trace_builtin_begin, trace_builtin_end};

use moor_common::{tasks::Session, util::PerfIntensity};
use moor_compiler::{BUILTINS, BuiltinId};
use moor_var::{List, VarType::TYPE_NONE, v_int};

use crate::vm::vm_host::ExecutionResult;

/// The set of parameters & utilities passed to the VM for execution of a given task.
pub struct VmExecParams<'a> {
    pub builtin_registry: &'a BuiltinRegistry,
    pub max_stack_depth: usize,
    pub config: &'a FeaturesConfig,
}

/// Extension trait for ExecState methods that require kernel-level builtin registry access.
pub(crate) trait ExecStateBuiltinExt {
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

impl ExecStateBuiltinExt for ExecState {
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
        let mut host = crate::vm::kernel_host::KernelHost;
        if let Some(proxy_result) =
            self.maybe_invoke_bf_proxy(&mut host, bf_desc.bf_override_name, &args)
        {
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
