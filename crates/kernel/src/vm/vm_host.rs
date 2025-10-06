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

use std::{
    fmt::{Debug, Formatter},
    time::{Duration, SystemTime},
};

use tracing::{debug, error, warn};

#[cfg(feature = "trace_events")]
use crate::tracing_events::{TraceEventType, emit_trace_event};

use moor_common::{
    model::{ObjFlag, VerbDef},
    tasks::{AbortLimitReason, TaskId},
};
use moor_compiler::{BuiltinId, CompileOptions, Offset, Program, compile};
use moor_var::{E_MAXREC, Error, List, Obj, Symbol, Var, v_none};

use crate::{
    PhantomUnsync,
    config::FeaturesConfig,
    task_context::with_current_transaction,
    vm::{
        FinallyReason, Fork, TaskSuspend, VMHostResponse,
        VMHostResponse::{AbortLimit, ContinueOk, DispatchFork, Suspend},
        VerbCall, VerbExecutionRequest,
        activation::Frame,
        builtins::BuiltinRegistry,
        exec_state::VMExecState,
        moo_execute::moo_frame_execute,
        vm_call::VmExecParams,
    },
};
use moor_common::{matching::ParsedCommand, tasks::Session};
use moor_var::program::{ProgramType, names::Name};

/// Possible outcomes from VM execution inner loop, which are used to determine what to do next.
#[derive(Debug, Clone)]
pub(crate) enum ExecutionResult {
    /// All is well. The task should let the VM continue executing.
    More,
    /// Execution of this stack frame is complete with a return value.
    Complete(Var),
    /// An error occurred during execution, that we might need to push to the stack and
    /// potentially resume or unwind, depending on the context.
    PushError(Error),
    /// An error occurred during execution, that should definitely be treated as a proper "raise"
    /// and unwind event unless there's a catch handler in place
    RaiseError(Error),
    /// An explicit stack unwind (for a reason other than a return.)
    Unwind(FinallyReason),
    /// Explicit return, unwind stack
    Return(Var),
    /// An exception was raised during execution.
    Exception(FinallyReason),
    /// Create the frames necessary to perform a `pass` up the inheritance chain.
    DispatchVerbPass(List),
    /// Begin preparing to call a verb, by looking up the verb and preparing the dispatch.
    PrepareVerbDispatch {
        this: Var,
        verb_name: Symbol,
        args: List,
    },
    /// Perform the verb dispatch, building the stack frame and executing it.
    DispatchVerb(Box<VerbExecutionRequest>),
    /// Request `eval` execution, which is a kind of special activation creation where we've already
    /// been given the program to execute instead of having to look it up.
    DispatchEval {
        /// The permissions context for the eval.
        permissions: Obj,
        /// The player who is performing the eval.
        player: Obj,
        /// The program to execute.
        program: Program,
    },
    /// Request dispatch of a builtin function with the given arguments.
    DispatchBuiltin { builtin: BuiltinId, arguments: List },
    /// Request dispatch of a lambda function with the given arguments.
    DispatchLambda {
        lambda: moor_var::Lambda,
        arguments: List,
    },
    /// Request start of a new task as a fork, at a given offset into the fork vector of the
    /// current program. If the duration is None, the task should be started immediately, otherwise
    /// it should be scheduled to start after the given delay.
    /// If a Name is provided, the task ID of the new task should be stored in the variable with
    /// that in the parent activation.
    TaskStartFork(Option<Duration>, Option<Name>, Offset),
    /// Request that this task be suspended for a duration of time.
    /// This leads to the task performing a commit, being suspended for a delay, and then being
    /// resumed under a new transaction.
    /// If the duration is None, then the task is suspended indefinitely, until it is killed or
    /// resumed using `resume()` or `kill_task()`.
    TaskSuspend(TaskSuspend),
    /// Request input from the client.
    TaskNeedInput,
    /// Rollback the current transaction and restart the task in a new transaction.
    /// This can happen when a conflict occurs during execution, independent of a commit.
    TaskRollbackRestart,
    /// Just rollback and die. Kills all task DB mutations. Output (Session) is optionally committed.
    TaskRollback(bool),
}

/// A 'host' for running some kind of interpreter / virtual machine inside a running moor task.
pub struct VmHost {
    /// Where we store current execution state for this host. Includes all activations and the
    /// interpreter-specific frames inside them.
    pub(crate) vm_exec_state: VMExecState,
    /// The maximum stack depth for this task
    pub(crate) max_stack_depth: usize,
    /// The amount of ticks (opcode executions) allotted to this task
    pub(crate) max_ticks: usize,
    /// The maximum amount of time allotted to this task
    pub(crate) max_time: Duration,
    pub(crate) running: bool,

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
    pub fn new(
        task_id: TaskId,
        max_stack_depth: usize,
        max_ticks: usize,
        max_time: Duration,
    ) -> Self {
        let vm_exec_state = VMExecState::new(task_id, max_ticks);

        // Created in an initial suspended state.
        Self {
            vm_exec_state,
            max_stack_depth,
            max_ticks,
            max_time,
            running: false,
            unsync: Default::default(),
        }
    }
}

impl VmHost {
    /// Setup for executing a method initiated from a command.
    pub fn start_call_command_verb(
        &mut self,
        task_id: TaskId,
        verb: (ProgramType, VerbDef),
        verb_call: VerbCall,
        command: ParsedCommand,
        permissions: &Obj,
    ) {
        self.vm_exec_state.start_time = Some(SystemTime::now());
        self.vm_exec_state.maximum_time = Some(self.max_time);
        self.vm_exec_state.tick_count = 0;
        self.vm_exec_state.task_id = task_id;
        self.vm_exec_state.exec_command_request(
            *permissions,
            verb.1,
            Box::new(verb_call),
            &command,
            verb.0,
        );
        self.running = true;
    }

    /// Setup for executing a method call in this VM.
    pub fn start_call_method_verb(
        &mut self,
        task_id: TaskId,
        perms: &Obj,
        verb_info: (ProgramType, VerbDef),
        verb_call: VerbCall,
    ) {
        self.start_execution(
            task_id,
            *perms,
            verb_info.1,
            Box::new(verb_call),
            verb_info.0,
        )
    }

    /// Start execution of a fork request in the hosted VM.
    pub fn start_fork(&mut self, task_id: TaskId, fork_request: &Fork, suspended: bool) {
        self.vm_exec_state.start_time = Some(SystemTime::now());
        self.vm_exec_state.maximum_time = Some(self.max_time);
        self.vm_exec_state.tick_count = 0;
        self.vm_exec_state.task_id = task_id;
        self.vm_exec_state.exec_fork_vector(fork_request.clone());
        self.running = !suspended;
    }

    /// Start execution of a verb request.
    pub fn start_execution(
        &mut self,
        task_id: TaskId,
        permissions: Obj,
        resolved_verb: VerbDef,
        call: Box<VerbCall>,
        program: ProgramType,
    ) {
        self.vm_exec_state.start_time = Some(SystemTime::now());
        self.vm_exec_state.maximum_time = Some(self.max_time);
        self.vm_exec_state.tick_count = 0;
        self.vm_exec_state.task_id = task_id;
        self.vm_exec_state
            .exec_call_request(permissions, resolved_verb, call, program);
        self.running = true;
    }

    /// Start execution of an eval request.
    pub fn start_eval(&mut self, task_id: TaskId, player: &Obj, program: Program) {
        let is_programmer = with_current_transaction(|world_state| {
            world_state
                .flags_of(player)
                .inspect_err(|e| error!(?e, "Failed to read player flags"))
                .map(|flags| flags.contains(ObjFlag::Programmer))
                .unwrap_or(false)
        });
        let program = if is_programmer {
            program
        } else {
            compile("return E_PERM;", CompileOptions::default()).unwrap()
        };

        self.vm_exec_state.start_time = Some(SystemTime::now());
        self.vm_exec_state.maximum_time = Some(self.max_time);
        self.vm_exec_state.tick_count = 0;
        self.vm_exec_state.task_id = task_id;
        self.vm_exec_state
            .exec_eval_request(player, player, program);
        self.running = true;
    }

    /// Run the hosted VM.
    pub fn exec_interpreter(
        &mut self,
        task_id: TaskId,
        session: &dyn Session,
        builtin_registry: &BuiltinRegistry,
        config: &FeaturesConfig,
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
        if let Some(start_time) = self.vm_exec_state.start_time {
            let elapsed = start_time.elapsed().expect("Could not get elapsed time");
            if elapsed > self.max_time {
                return AbortLimit(AbortLimitReason::Time(elapsed));
            }
        };

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
                    result = self.vm_exec_state.prepare_pass_verb(&pass_args);
                    continue;
                }
                ExecutionResult::PrepareVerbDispatch {
                    this,
                    verb_name,
                    args,
                } => {
                    result = self
                        .vm_exec_state
                        .verb_dispatch(&exec_params, this, verb_name, args)
                        .unwrap_or_else(ExecutionResult::PushError);
                    continue;
                }
                ExecutionResult::DispatchVerb(exec_request) => {
                    self.vm_exec_state.exec_call_request(
                        exec_request.permissions,
                        exec_request.resolved_verb,
                        exec_request.call,
                        exec_request.program,
                    );
                    return ContinueOk;
                }
                ExecutionResult::DispatchEval {
                    permissions,
                    player,
                    program,
                } => {
                    self.vm_exec_state
                        .exec_eval_request(&permissions, &player, program);
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
                    let new_activation = a.clone();
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
                ExecutionResult::TaskNeedInput => {
                    return VMHostResponse::SuspendNeedInput;
                }
                ExecutionResult::Complete(a) => {
                    tracing::info!("exec_interpreter: ExecutionResult::Complete with value: {:?}", a);
                    return VMHostResponse::CompleteSuccess(a);
                }
                ExecutionResult::Exception(fr) => {
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

        // Pick the right kind of execution flow depending on the activation -- builtin or MOO?
        let mut tick_count = self.vm_exec_state.tick_count;
        let tick_slice = self.vm_exec_state.tick_slice;
        let activation = self.vm_exec_state.top_mut();

        let (result, new_tick_count) = match &mut activation.frame {
            Frame::Moo(fr) => {
                let result = moo_frame_execute(
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
            Frame::JavaScript(js_fr) => {
                // Execute JavaScript using the V8 engine
                use crate::vm::js_execute::execute_js_frame;
                let result = execute_js_frame(js_fr, &activation.this, activation.player, tick_slice);
                tracing::info!("run_interpreter: JavaScript frame returned: {:?}", result);
                (result, tick_count)
            }
        };
        self.vm_exec_state.tick_count = new_tick_count;

        tracing::info!("run_interpreter: Returning result: {:?}", result);
        result
    }

    /// Resume what you were doing after suspension.
    pub fn resume_execution(&mut self, value: Var) {
        self.vm_exec_state.start_time = Some(SystemTime::now());
        self.vm_exec_state.tick_count = 0;
        self.running = true;

        // Emit task resume trace event
        #[cfg(feature = "trace_events")]
        emit_trace_event(TraceEventType::TaskResume {
            task_id: self.vm_exec_state.task_id,
        });

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
        self.vm_exec_state.start_time = Some(SystemTime::now());
        self.vm_exec_state.tick_count = 0;
        self.running = true;

        // Emit task resume trace event
        #[cfg(feature = "trace_events")]
        emit_trace_event(TraceEventType::TaskResume {
            task_id: self.vm_exec_state.task_id,
        });

        // Set pending error to be raised when execution starts
        self.vm_exec_state.pending_raise_error = Some(error);

        debug!(
            task_id = self.vm_exec_state.task_id,
            "Resuming VMHost with error"
        );
    }

    /// Get a copy of the current VM state, for later restoration.
    pub(crate) fn snapshot_state(&self) -> VMExecState {
        self.vm_exec_state.clone()
    }

    /// Get a reference to the current VM execution state for read-only access.
    pub(crate) fn vm_exec_state(&self) -> &VMExecState {
        &self.vm_exec_state
    }

    /// Restore from a snapshot.
    pub(crate) fn restore_state(&mut self, state: &VMExecState) {
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
    pub fn verb_name(&self) -> Symbol {
        self.vm_exec_state.top().verb_name
    }
    pub fn verb_definer(&self) -> Obj {
        self.vm_exec_state.top().verb_definer()
    }
    pub fn this(&self) -> Var {
        self.vm_exec_state.top().this.clone()
    }
    pub fn line_number(&self) -> usize {
        self.vm_exec_state.top().frame.find_line_no().unwrap_or(0)
    }

    /// Get the current traceback and formatted backtrace
    pub fn get_traceback(&self) -> (Vec<Var>, Vec<Var>) {
        use crate::vm::exec_state::VMExecState;
        let stack = VMExecState::make_stack_list(&self.vm_exec_state.stack);
        // For timeouts, we don't have an Error, so create a simple timeout "error" for formatting
        let timeout_error = moor_var::Error::new(
            moor_var::ErrorCode::E_MAXREC, // Use a generic error code
            Some("Task timeout".to_string()),
            None,
        );
        let backtrace = VMExecState::make_backtrace(&self.vm_exec_state.stack, &timeout_error);
        (stack, backtrace)
    }

    pub fn reset_ticks(&mut self) {
        self.vm_exec_state.tick_count = 0;
    }
    pub fn tick_count(&self) -> usize {
        self.vm_exec_state.tick_count
    }
    pub fn reset_time(&mut self) {
        self.vm_exec_state.start_time = Some(SystemTime::now());
    }
    pub fn args(&self) -> &List {
        &self.vm_exec_state.top().args
    }
}
