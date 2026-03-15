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

use std::{
    fmt::{Debug, Formatter},
    time::Duration,
};

use tracing::{debug, error, warn};

use moor_common::{
    model::{ObjFlag, ResolvedVerb},
    tasks::{AbortLimitReason, TaskId},
    util::BitEnum,
};
use moor_compiler::{CompileOptions, Program, compile};
use moor_var::{E_MAXREC, List, Obj, Symbol, Var, v_none};
use moor_vm::ExecState;

use crate::{
    config::FeaturesConfig,
    task_context::with_current_transaction,
    tasks::task_program_cache::TaskProgramCache,
    vm::{
        FinallyReason, Fork, VMHostResponse,
        VMHostResponse::{AbortLimit, ContinueOk, DispatchFork, Suspend},
        builtins::BuiltinRegistry,
        kernel_host::KernelHost,
        vm_call::{ExecStateBuiltinExt, VmExecParams},
    },
};
use moor_common::{matching::ParsedCommand, tasks::Session};
use moor_var::program::{ProgramType, names::Name};
use moor_vm::{CallProgram, Frame, PhantomUnsync, VmHost as _, moo_frame_execute};

#[cfg(feature = "javascript")]
use moor_js_engine::{
    JsError, JsWorkerPool, TrampolineRequest, TrampolineResponse, WorkerInput,
};

pub(crate) use moor_vm::ExecutionResult;

/// Active trampoline channels for a single JS verb execution.
#[cfg(feature = "javascript")]
pub(crate) struct JsTrampolineState {
    pub trampoline_rx: flume::Receiver<TrampolineRequest>,
    pub worker_tx: flume::Sender<WorkerInput>,
    /// The resolver_id of a pending CallVerb, if any (VerbCallPending state).
    pub pending_resolver_id: Option<usize>,
}

/// A 'host' for running some kind of interpreter / virtual machine inside a running moor task.
pub struct VmHost {
    /// Where we store current execution state for this host. Includes all activations and the
    /// interpreter-specific frames inside them.
    pub(crate) vm_exec_state: ExecState,
    /// The maximum stack depth for this task
    pub(crate) max_stack_depth: usize,
    /// The amount of ticks (opcode executions) allotted to this task
    pub(crate) max_ticks: usize,
    /// The maximum amount of time allotted to this task
    pub(crate) max_time: Duration,
    pub(crate) running: bool,

    /// V8 worker pool for JavaScript verb execution.
    #[cfg(feature = "javascript")]
    pub(crate) js_worker: Option<std::sync::Arc<JsWorkerPool>>,

    /// Stack of active trampolines — one per nested JS verb execution.
    #[cfg(feature = "javascript")]
    pub(crate) js_trampolines: Vec<JsTrampolineState>,

    pub(crate) unsync: PhantomUnsync,
}

impl Debug for VmHost {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VmHost")
            .field("task_id", &self.vm_exec_state.task_id)
            .field("running", &self.running)
            .field("max_stack_depth", &self.max_stack_depth)
            .field("max_ticks", &self.max_ticks)
            .field("max_time", &self.max_time)
            .finish()
    }
}
impl VmHost {
    const TIME_CHECK_TICK_MASK: usize = 0x3f; // Check runtime limit every 64 ticks.

    pub fn new(
        task_id: TaskId,
        max_stack_depth: usize,
        max_ticks: usize,
        max_time: Duration,
    ) -> Self {
        let vm_exec_state = ExecState::new(task_id, max_ticks);

        // Created in an initial suspended state.
        Self {
            vm_exec_state,
            max_stack_depth,
            max_ticks,
            max_time,
            running: false,
            #[cfg(feature = "javascript")]
            js_worker: None,
            #[cfg(feature = "javascript")]
            js_trampolines: Vec::new(),
            unsync: Default::default(),
        }
    }

    /// Set the JS worker pool reference for this host.
    #[cfg(feature = "javascript")]
    pub fn set_js_worker(&mut self, pool: std::sync::Arc<JsWorkerPool>) {
        self.js_worker = Some(pool);
    }
}

impl VmHost {
    /// Setup for executing a method initiated from a command.
    #[allow(clippy::too_many_arguments)]
    pub fn start_call_command_verb(
        &mut self,
        task_id: TaskId,
        resolved_verb: ResolvedVerb,
        verb_name: Symbol,
        this: Var,
        player: Obj,
        caller: Var,
        command: ParsedCommand,
        permissions_flags: BitEnum<ObjFlag>,
        program: ProgramType,
    ) {
        self.vm_exec_state.mark_started_now();
        self.vm_exec_state.maximum_time = Some(self.max_time);
        self.vm_exec_state.tick_count = 0;
        self.vm_exec_state.task_id = task_id;
        self.vm_exec_state.exec_command_request(
            permissions_flags,
            resolved_verb,
            verb_name,
            this,
            player,
            caller,
            command,
            CallProgram::Materialized(program),
        );
        self.running = true;
    }

    /// Setup for executing a method call in this VM.
    #[allow(clippy::too_many_arguments)]
    pub fn start_call_method_verb(
        &mut self,
        task_id: TaskId,
        resolved_verb: ResolvedVerb,
        verb_name: Symbol,
        this: Var,
        player: Obj,
        args: List,
        caller: Var,
        argstr: Var,
        permissions_flags: BitEnum<ObjFlag>,
        program: ProgramType,
    ) {
        self.vm_exec_state.mark_started_now();
        self.vm_exec_state.maximum_time = Some(self.max_time);
        self.vm_exec_state.tick_count = 0;
        self.vm_exec_state.task_id = task_id;
        self.vm_exec_state.exec_call_request(
            permissions_flags,
            resolved_verb,
            verb_name,
            this,
            player,
            args,
            caller,
            argstr,
            CallProgram::Materialized(program),
        );
        self.running = true;
    }

    /// Start execution of a fork request in the hosted VM.
    pub fn start_fork(&mut self, task_id: TaskId, fork_request: &Fork, suspended: bool) {
        self.vm_exec_state.mark_started_now();
        self.vm_exec_state.maximum_time = Some(self.max_time);
        self.vm_exec_state.tick_count = 0;
        self.vm_exec_state.task_id = task_id;
        self.vm_exec_state.exec_fork_vector(fork_request.clone());
        self.running = !suspended;
    }

    /// Start execution of an eval request.
    pub fn start_eval(
        &mut self,
        task_id: TaskId,
        player: &Obj,
        program: Program,
        initial_env: Option<&[(Symbol, Var)]>,
    ) {
        let mut host = KernelHost;
        let is_programmer = host
            .flags_of(player)
            .inspect_err(|e| error!(?e, "Failed to read player flags"))
            .map(|flags| flags.contains(ObjFlag::Programmer))
            .unwrap_or(false);
        let program = if is_programmer {
            program
        } else {
            compile("return E_PERM;", CompileOptions::default()).unwrap()
        };

        self.vm_exec_state.mark_started_now();
        self.vm_exec_state.maximum_time = Some(self.max_time);
        self.vm_exec_state.tick_count = 0;
        self.vm_exec_state.task_id = task_id;
        self.vm_exec_state
            .exec_eval_request(&mut host, player, player, program, initial_env);
        self.running = true;
    }

    /// Run the hosted VM.
    pub fn exec_interpreter(
        &mut self,
        task_id: TaskId,
        session: &dyn Session,
        builtin_registry: &BuiltinRegistry,
        config: &FeaturesConfig,
        program_cache: &mut TaskProgramCache,
    ) -> VMHostResponse {
        self.vm_exec_state.task_id = task_id;

        let exec_params = VmExecParams {
            builtin_registry,
            max_stack_depth: self.max_stack_depth,
            config,
        };

        // Check existing ticks and seconds, and abort the task if we've exceeded the limits.
        if self.vm_exec_state.tick_count >= self.max_ticks {
            return AbortLimit(AbortLimitReason::Ticks(self.vm_exec_state.tick_count));
        }
        if (self.vm_exec_state.tick_count & Self::TIME_CHECK_TICK_MASK) == 0
            && let Some(elapsed) = self.vm_exec_state.elapsed_runtime()
            && elapsed > self.max_time
        {
            return AbortLimit(AbortLimitReason::Time(elapsed));
        }

        // Grant the loop its next tick slice.
        self.vm_exec_state.tick_slice = self.max_ticks - self.vm_exec_state.tick_count;

        // Check if we have a pending error to raise (from worker error handling)
        let mut result = if let Some(pending_error) = self.vm_exec_state.pending_raise_error.take()
        {
            self.vm_exec_state.raise_error(pending_error)
        } else {
            // Actually invoke the VM, asking it to loop until it's ready to yield back to us.
            self.run_interpreter(&exec_params, session)
        };
        while self.is_running() {
            match result {
                ExecutionResult::More => return ContinueOk,
                ExecutionResult::PushError(e) => {
                    result = self.vm_exec_state.push_error(e);
                    continue;
                }
                ExecutionResult::RaiseError(e) => {
                    result = self.vm_exec_state.raise_error(e);
                    continue;
                }
                ExecutionResult::Return(value) => {
                    result = self
                        .vm_exec_state
                        .unwind_stack(FinallyReason::Return(value));
                    continue;
                }
                ExecutionResult::Unwind(fr) => {
                    result = self.vm_exec_state.unwind_stack(fr);
                    continue;
                }
                ExecutionResult::DispatchVerbPass(pass_args) => {
                    let mut host = KernelHost;
                    result = self.vm_exec_state.prepare_pass_verb(&mut host, &pass_args);
                    continue;
                }
                ExecutionResult::PrepareVerbDispatch {
                    this,
                    verb_name,
                    args,
                } => {
                    let mut host = KernelHost;
                    result = self
                        .vm_exec_state
                        .verb_dispatch(
                            &mut host,
                            exec_params.config.type_dispatch,
                            this,
                            verb_name,
                            args,
                        )
                        .unwrap_or_else(ExecutionResult::PushError);
                    continue;
                }
                ExecutionResult::DispatchVerb(exec_request) => {
                    // Check if this is a JavaScript verb before hitting the MooR cache.
                    #[cfg(feature = "javascript")]
                    {
                        let maybe_js = with_current_transaction(|ws| {
                            ws.retrieve_verb(
                                &exec_request.permissions,
                                &exec_request.program_key.verb_definer,
                                exec_request.program_key.verb_uuid,
                            )
                        });
                        match maybe_js {
                            Ok((ProgramType::JavaScript(source), _)) => {
                                result = self.dispatch_js_verb(&exec_request, source);
                                continue;
                            }
                            Ok((ProgramType::MooR(_), _)) => {
                                // Fall through to normal MooR dispatch below.
                            }
                            Err(moor_common::model::WorldStateError::RollbackRetry) => {
                                return VMHostResponse::RollbackRetry;
                            }
                            Err(e) => {
                                result = ExecutionResult::PushError(e.to_error());
                                continue;
                            }
                        }
                    }

                    let resolved = match with_current_transaction(|ws| {
                        program_cache.resolve_verb_slot(
                            ws,
                            &exec_request.permissions,
                            &exec_request.program_key.verb_definer,
                            exec_request.program_key.verb_uuid,
                        )
                    }) {
                        Ok(program_slot) => program_slot,
                        Err(moor_common::model::WorldStateError::RollbackRetry) => {
                            return VMHostResponse::RollbackRetry;
                        }
                        Err(e) => {
                            result = ExecutionResult::PushError(e.to_error());
                            continue;
                        }
                    };
                    if resolved.cache_hit {
                        self.vm_exec_state.program_cache_stats.hits += 1;
                    } else {
                        self.vm_exec_state.program_cache_stats.misses += 1;
                    }
                    if resolved.inserted {
                        self.vm_exec_state.program_cache_stats.inserts += 1;
                    }
                    self.vm_exec_state.exec_call_request(
                        exec_request.permissions_flags,
                        exec_request.resolved_verb,
                        exec_request.verb_name,
                        exec_request.this,
                        exec_request.player,
                        exec_request.args,
                        exec_request.caller,
                        exec_request.argstr,
                        CallProgram::CachedSlot(resolved.slot),
                    );
                    return ContinueOk;
                }
                ExecutionResult::DispatchCommandVerb(exec_request) => {
                    let resolved = match with_current_transaction(|ws| {
                        program_cache.resolve_verb_slot(
                            ws,
                            &exec_request.permissions,
                            &exec_request.program_key.verb_definer,
                            exec_request.program_key.verb_uuid,
                        )
                    }) {
                        Ok(program_slot) => program_slot,
                        Err(moor_common::model::WorldStateError::RollbackRetry) => {
                            return VMHostResponse::RollbackRetry;
                        }
                        Err(e) => {
                            result = ExecutionResult::PushError(e.to_error());
                            continue;
                        }
                    };
                    if resolved.cache_hit {
                        self.vm_exec_state.program_cache_stats.hits += 1;
                    } else {
                        self.vm_exec_state.program_cache_stats.misses += 1;
                    }
                    if resolved.inserted {
                        self.vm_exec_state.program_cache_stats.inserts += 1;
                    }
                    self.vm_exec_state.exec_command_request(
                        exec_request.permissions_flags,
                        exec_request.resolved_verb,
                        exec_request.verb_name,
                        exec_request.this,
                        exec_request.player,
                        exec_request.caller,
                        exec_request.command,
                        CallProgram::CachedSlot(resolved.slot),
                    );
                    return ContinueOk;
                }
                ExecutionResult::DispatchEval {
                    permissions,
                    player,
                    program,
                    initial_env,
                } => {
                    let mut host = KernelHost;
                    self.vm_exec_state.exec_eval_request(
                        &mut host,
                        &permissions,
                        &player,
                        program,
                        initial_env.as_deref(),
                    );
                    return ContinueOk;
                }
                ExecutionResult::DispatchBuiltin {
                    builtin: bf_offset,
                    arguments: args,
                } => {
                    // Ask the VM to execute the builtin function.
                    // This will push the result onto the stack.
                    // After this we will loop around and check the result.
                    result = self.vm_exec_state.call_builtin_function(
                        bf_offset,
                        args,
                        &exec_params,
                        session,
                    );
                    continue;
                }
                ExecutionResult::DispatchLambda {
                    lambda,
                    arguments: args,
                } => {
                    // Handle lambda execution by pushing a new lambda activation
                    match self.vm_exec_state.exec_lambda_request(lambda, args) {
                        Ok(_) => {
                            result = self.run_interpreter(&exec_params, session);
                            continue;
                        }
                        Err(e) => {
                            result = ExecutionResult::PushError(e);
                            continue;
                        }
                    }
                }
                ExecutionResult::TaskStartFork(delay, task_id, fv_offset) => {
                    let a = self.vm_exec_state.top().clone();
                    let parent_task_id = self.vm_exec_state.task_id;
                    let mut new_activation = a.clone();
                    if let Frame::Moo(ref mut frame) = new_activation.frame {
                        frame.materialize_program_for_handoff();
                    }
                    let fork_request = Box::new(Fork {
                        player: a.player,
                        progr: a.permissions,
                        parent_task_id,
                        delay,
                        activation: new_activation,
                        fork_vector_offset: fv_offset,
                        task_id,
                    });
                    return DispatchFork(fork_request);
                }
                ExecutionResult::TaskSuspend(delay) => {
                    return Suspend(Box::new(delay));
                }
                ExecutionResult::TaskNeedInput(metadata) => {
                    return VMHostResponse::SuspendNeedInput(metadata);
                }
                ExecutionResult::Complete(a) => {
                    #[cfg(feature = "javascript")]
                    self.cleanup_js_trampoline(None);
                    return VMHostResponse::CompleteSuccess(a);
                }
                ExecutionResult::Exception(fr) => {
                    #[cfg(feature = "javascript")]
                    if let FinallyReason::Raise(ref exception) = fr {
                        self.cleanup_js_trampoline(Some(&exception.error));
                    } else {
                        self.cleanup_js_trampoline(None);
                    }
                    return match &fr {
                        FinallyReason::Abort => VMHostResponse::CompleteAbort,
                        FinallyReason::Raise(exception) => {
                            VMHostResponse::CompleteException(exception.clone())
                        }
                        _ => {
                            unreachable!(
                                "Invalid FinallyReason {:?} reached for task {} in scheduler",
                                fr, task_id
                            );
                        }
                    };
                }
                ExecutionResult::TaskRollbackRestart => {
                    return VMHostResponse::RollbackRetry;
                }
                ExecutionResult::TaskRollback(commit_session) => {
                    return VMHostResponse::CompleteRollback(commit_session);
                }
            }
        }

        // We're not running and we didn't get a completion response from the VM - we must have been
        // asked to stop by the scheduler.
        warn!(task_id, "VM host stopped by task");
        VMHostResponse::CompleteAbort
    }

    pub(crate) fn run_interpreter(
        &mut self,
        vm_exec_params: &VmExecParams,
        session: &dyn Session,
    ) -> ExecutionResult {
        // No activations? Nothing to do.
        if self.vm_exec_state.stack.is_empty() {
            return ExecutionResult::Complete(v_none());
        }

        // Before executing, check stack depth...
        if self.vm_exec_state.stack.len() >= vm_exec_params.max_stack_depth {
            // Absolutely raise-unwind an error here instead of just offering it as a potential
            // return value if this is a non-d verb. At least I think this the right thing to do?
            return self.vm_exec_state.throw_error(E_MAXREC.into());
        }

        // JS frames are handled via the trampoline, not the normal interpreter path.
        if matches!(self.vm_exec_state.top().frame, Frame::Js(_)) {
            return self.handle_js_trampoline();
        }

        // Pick the right kind of execution flow depending on the activation — builtin or MOO?
        let mut tick_count = self.vm_exec_state.tick_count;
        let tick_slice = self.vm_exec_state.tick_slice;
        let activation = self.vm_exec_state.top_mut();

        let (result, new_tick_count) = match &mut activation.frame {
            Frame::Moo(fr) => {
                let mut host = KernelHost;
                let result = moo_frame_execute(
                    &mut host,
                    tick_slice,
                    &mut tick_count,
                    activation.permissions,
                    fr,
                    vm_exec_params.config,
                );
                (result, tick_count)
            }
            Frame::Bf(_) => {
                let result = self
                    .vm_exec_state
                    .reenter_builtin_function(vm_exec_params, session);
                (result, tick_count)
            }
            Frame::Js(_) => unreachable!("Js case handled above"),
        };
        self.vm_exec_state.tick_count = new_tick_count;

        result
    }

    /// Resume what you were doing after suspension.
    pub fn resume_execution(&mut self, value: Var) {
        self.vm_exec_state.mark_started_now();
        self.vm_exec_state.tick_count = 0;
        self.running = true;

        // If there's no activations at all, that means we're a Fork, not returning to something.
        if !self.vm_exec_state.stack.is_empty() {
            // coming back from any suspend, we need a return value to feed back to `bf_suspend` or
            // `bf_read()`
            self.vm_exec_state.set_return_value(value);
        }

        debug!(task_id = self.vm_exec_state.task_id, "Resuming VMHost");
    }

    /// Resume execution by raising an error (for worker error conditions)
    pub fn resume_with_error(&mut self, error: moor_var::Error) {
        self.vm_exec_state.mark_started_now();
        self.vm_exec_state.tick_count = 0;
        self.running = true;

        // Set pending error to be raised when execution starts
        self.vm_exec_state.pending_raise_error = Some(error);

        debug!(
            task_id = self.vm_exec_state.task_id,
            "Resuming VMHost with error"
        );
    }

    /// Get a copy of the current VM state, for later restoration.
    pub(crate) fn snapshot_state(&self) -> ExecState {
        let mut snapshot = self.vm_exec_state.clone();
        snapshot.materialize_frame_programs();
        snapshot
    }

    /// Get a reference to the current VM execution state for read-only access.
    pub(crate) fn vm_exec_state(&self) -> &ExecState {
        &self.vm_exec_state
    }

    pub(crate) fn vm_exec_state_mut(&mut self) -> &mut ExecState {
        &mut self.vm_exec_state
    }

    /// Restore from a snapshot.
    pub(crate) fn restore_state(&mut self, state: &ExecState) {
        self.vm_exec_state = state.clone();
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn stop(&mut self) {
        self.running = false;
    }

    pub fn set_variable(&mut self, task_id_var: &Name, value: Var) {
        self.vm_exec_state
            .top_mut()
            .frame
            .set_variable(task_id_var, value)
            .expect("Could not set forked task id");
    }

    pub fn permissions(&self) -> Obj {
        self.vm_exec_state.top().permissions
    }

    pub fn set_program_cache_sizes(
        &mut self,
        total_slots: usize,
        live_slots: usize,
        key_count: usize,
    ) {
        self.vm_exec_state.program_cache_total_slots = total_slots;
        self.vm_exec_state.program_cache_live_slots = live_slots;
        self.vm_exec_state.program_cache_key_count = key_count;
    }

    /// Try to get the verb name of the current activation.
    /// Returns None if the activation stack is empty (e.g., task not yet initialized).
    pub fn verb_name(&self) -> Option<Symbol> {
        self.vm_exec_state.try_top().map(|a| a.verb_name)
    }

    /// Try to get the verb definer of the current activation.
    /// Returns None if the activation stack is empty (e.g., task not yet initialized).
    pub fn verb_definer(&self) -> Option<Obj> {
        self.vm_exec_state.try_top().map(|a| a.verb_definer())
    }

    /// Try to get the 'this' value of the current activation.
    /// Returns None if the activation stack is empty (e.g., task not yet initialized).
    pub fn this(&self) -> Option<Var> {
        self.vm_exec_state.try_top().map(|a| a.this.clone())
    }

    /// Try to get the line number of the current activation.
    /// Returns None if the activation stack is empty (e.g., task not yet initialized).
    pub fn line_number(&self) -> Option<usize> {
        self.vm_exec_state
            .try_top()
            .map(|a| a.frame.find_line_no().unwrap_or(0))
    }

    /// Get the current traceback and formatted backtrace
    pub fn get_traceback(&self) -> (Vec<Var>, Vec<Var>) {
        let stack = ExecState::make_stack_list(&self.vm_exec_state.stack);
        // For timeouts, we don't have an Error, so create a simple timeout "error" for formatting
        let timeout_error = moor_var::Error::new(
            moor_var::ErrorCode::E_MAXREC, // Use a generic error code
            Some("Task timeout".to_string()),
            None,
        );
        let backtrace = ExecState::make_backtrace(&self.vm_exec_state.stack, &timeout_error);
        (stack, backtrace)
    }

    /// Push a JsFrame activation, send the DockRequest to the V8 worker, and
    /// store the trampoline channels. Returns More so the exec_interpreter loop
    /// comes back around and hits run_interpreter → handle_js_trampoline.
    #[cfg(feature = "javascript")]
    fn dispatch_js_verb(
        &mut self,
        exec_request: &moor_vm::VerbExecutionRequest,
        source: std::sync::Arc<str>,
    ) -> ExecutionResult {
        let Some(ref js_pool) = self.js_worker else {
            return ExecutionResult::PushError(
                moor_var::E_INVARG.msg("JavaScript execution not available"),
            );
        };

        let activation = moor_vm::Activation::for_js_call(
            exec_request.resolved_verb,
            exec_request.permissions_flags,
            exec_request.verb_name,
            exec_request.this.clone(),
            exec_request.player,
            exec_request.args.clone(),
        );
        self.vm_exec_state.stack.push(activation);

        let this_obj = exec_request
            .this
            .as_object()
            .unwrap_or(moor_var::NOTHING);
        let args: Vec<moor_var::Var> = exec_request.args.iter().collect();

        let (trampoline_rx, worker_tx) =
            js_pool.submit(source, this_obj, exec_request.player, args);

        self.js_trampolines.push(JsTrampolineState {
            trampoline_rx,
            worker_tx,
            pending_resolver_id: None,
        });

        ExecutionResult::More
    }

    /// Process the JS trampoline when a JsFrame is at the top of stack.
    /// Called from run_interpreter.
    fn handle_js_trampoline(&mut self) -> ExecutionResult {
        #[cfg(feature = "javascript")]
        {
            use moor_vm::JsFrameState;

            let Some(trampoline) = self.js_trampolines.last_mut() else {
                self.vm_exec_state.stack.pop();
                return ExecutionResult::PushError(
                    moor_var::E_INVARG.msg("JS frame without active trampoline"),
                );
            };

            // If a verb call just returned, send its result back to V8.
            {
                let activation = self.vm_exec_state.top_mut();
                let Frame::Js(ref mut js_frame) = activation.frame else {
                    unreachable!()
                };
                if js_frame.state == JsFrameState::VerbCallPending {
                    let val = js_frame.return_value.take().unwrap_or_else(moor_var::v_none);
                    let resolver_id = trampoline.pending_resolver_id.take().unwrap_or(0);
                    let _ = trampoline.worker_tx.send(WorkerInput::Response {
                        resolver_id,
                        response: TrampolineResponse::Value(val),
                    });
                    js_frame.state = JsFrameState::AwaitingRequest;
                }
            }

            // Block waiting for the next request from V8.
            let request = match trampoline.trampoline_rx.recv() {
                Ok(r) => r,
                Err(_) => {
                    self.js_trampolines.pop();
                    self.vm_exec_state.stack.pop();
                    return ExecutionResult::PushError(
                        moor_var::E_INVARG.msg("JS worker channel closed"),
                    );
                }
            };

            match request {
                TrampolineRequest::Complete(Ok(value)) => {
                    self.js_trampolines.pop();
                    self.vm_exec_state.stack.pop();
                    if self.vm_exec_state.stack.is_empty() {
                        return ExecutionResult::Complete(value);
                    }
                    self.vm_exec_state.set_return_value(value);
                    ExecutionResult::More
                }
                TrampolineRequest::Complete(Err(js_err)) => {
                    self.js_trampolines.pop();
                    self.vm_exec_state.stack.pop();
                    ExecutionResult::PushError(moor_var::E_INVARG.with_msg(|| {
                        format!("JavaScript error: {}", js_err.message)
                    }))
                }
                TrampolineRequest::GetProp {
                    resolver_id,
                    obj,
                    prop,
                } => {
                    let mut host = KernelHost;
                    let perms = self.vm_exec_state.top().permissions;
                    let response = match host.retrieve_property(&perms, &obj, prop) {
                        Ok(val) => TrampolineResponse::Value(val),
                        Err(e) => TrampolineResponse::Error(JsError {
                            message: format!("{e:?}"),
                        }),
                    };
                    // Re-borrow after KernelHost is done.
                    let trampoline = self.js_trampolines.last().unwrap();
                    let _ = trampoline.worker_tx.send(WorkerInput::Response {
                        resolver_id,
                        response,
                    });
                    ExecutionResult::More
                }
                TrampolineRequest::SetProp {
                    resolver_id,
                    obj,
                    prop,
                    value,
                } => {
                    let mut host = KernelHost;
                    let perms = self.vm_exec_state.top().permissions;
                    let response = match host.update_property(&perms, &obj, prop, &value) {
                        Ok(()) => TrampolineResponse::Value(moor_var::v_none()),
                        Err(e) => TrampolineResponse::Error(JsError {
                            message: format!("{e:?}"),
                        }),
                    };
                    let trampoline = self.js_trampolines.last().unwrap();
                    let _ = trampoline.worker_tx.send(WorkerInput::Response {
                        resolver_id,
                        response,
                    });
                    ExecutionResult::More
                }
                TrampolineRequest::CallVerb {
                    resolver_id,
                    this,
                    verb,
                    args,
                } => {
                    // Store the resolver_id so we can send the response when
                    // the verb returns.
                    let trampoline = self.js_trampolines.last_mut().unwrap();
                    trampoline.pending_resolver_id = Some(resolver_id);

                    // Mark the JS frame as waiting for a verb return.
                    let activation = self.vm_exec_state.top_mut();
                    let Frame::Js(ref mut js_frame) = activation.frame else {
                        unreachable!()
                    };
                    js_frame.state = JsFrameState::VerbCallPending;

                    let args_list: moor_var::List = args.into_iter().collect();
                    ExecutionResult::PrepareVerbDispatch {
                        this: moor_var::v_obj(this),
                        verb_name: verb,
                        args: args_list,
                    }
                }
            }
        }

        #[cfg(not(feature = "javascript"))]
        {
            self.vm_exec_state.stack.pop();
            ExecutionResult::PushError(moor_var::E_INVARG.msg("JavaScript not enabled"))
        }
    }

    /// Clean up all JS trampoline state, notifying V8 of the error if needed.
    #[cfg(feature = "javascript")]
    fn cleanup_js_trampoline(&mut self, error: Option<&moor_var::Error>) {
        for trampoline in self.js_trampolines.drain(..) {
            if let Some(err) = error {
                // Send error so the V8 worker doesn't hang waiting for a response.
                if let Some(resolver_id) = trampoline.pending_resolver_id {
                    let _ = trampoline.worker_tx.send(WorkerInput::Response {
                        resolver_id,
                        response: TrampolineResponse::Error(JsError {
                            message: format!("{err}"),
                        }),
                    });
                }
            }
        }
    }

    pub fn reset_ticks(&mut self) {
        self.vm_exec_state.tick_count = 0;
    }
    pub fn tick_count(&self) -> usize {
        self.vm_exec_state.tick_count
    }
    pub fn reset_time(&mut self) {
        self.vm_exec_state.mark_started_now();
    }
    pub fn args(&self) -> &List {
        &self.vm_exec_state.top().args
    }
}
