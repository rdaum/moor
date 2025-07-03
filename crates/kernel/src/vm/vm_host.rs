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

use std::fmt::{Debug, Formatter};
use std::time::{Duration, SystemTime};

use bincode::de::{BorrowDecoder, Decoder};
use bincode::enc::Encoder;
use bincode::error::{DecodeError, EncodeError};
use bincode::{BorrowDecode, Decode, Encode};
use tracing::{debug, error, warn};

use moor_common::model::ObjFlag;
use moor_common::model::{VerbDef, WorldState};
use moor_common::tasks::{AbortLimitReason, TaskId};
use moor_compiler::Program;
use moor_compiler::{BuiltinId, Offset};
use moor_compiler::{CompileOptions, compile};
use moor_var::List;
use moor_var::Obj;
use moor_var::Var;
use moor_var::{E_MAXREC, Error};
use moor_var::{Symbol, v_none};

use crate::PhantomUnsync;
use crate::config::FeaturesConfig;
use crate::tasks::task_scheduler_client::TaskSchedulerClient;
use crate::vm::FinallyReason;
use crate::vm::VMHostResponse::{AbortLimit, ContinueOk, DispatchFork, Suspend};
use crate::vm::activation::Frame;
use crate::vm::builtins::BuiltinRegistry;
use crate::vm::exec_state::VMExecState;
use crate::vm::moo_execute::moo_frame_execute;
use crate::vm::vm_call::VmExecParams;
use crate::vm::{Fork, VMHostResponse, VerbExecutionRequest};
use crate::vm::{TaskSuspend, VerbCall};
use moor_common::matching::ParsedCommand;
use moor_common::tasks::Session;
use moor_var::program::ProgramType;
use moor_var::program::names::Name;

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
    vm_exec_state: VMExecState,
    /// The maximum stack depth for this task
    max_stack_depth: usize,
    /// The amount of ticks (opcode executions) allotted to this task
    max_ticks: usize,
    /// The maximum amount of time allotted to this task
    max_time: Duration,
    running: bool,

    unsync: PhantomUnsync,
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
        let call_request = Box::new(VerbExecutionRequest {
            permissions: *permissions,
            resolved_verb: verb.1,
            call: Box::new(verb_call),
            command: Some(Box::new(command)),
            program: verb.0,
        });

        self.start_execution(task_id, call_request)
    }

    /// Setup for executing a method call in this VM.
    pub fn start_call_method_verb(
        &mut self,
        task_id: TaskId,
        perms: &Obj,
        verb_info: (ProgramType, VerbDef),
        verb_call: VerbCall,
    ) {
        let call_request = Box::new(VerbExecutionRequest {
            permissions: *perms,
            resolved_verb: verb_info.1,
            call: Box::new(verb_call),
            command: None,
            program: verb_info.0,
        });

        self.start_execution(task_id, call_request)
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
        verb_execution_request: Box<VerbExecutionRequest>,
    ) {
        self.vm_exec_state.start_time = Some(SystemTime::now());
        self.vm_exec_state.maximum_time = Some(self.max_time);
        self.vm_exec_state.tick_count = 0;
        self.vm_exec_state.task_id = task_id;
        self.vm_exec_state.exec_call_request(verb_execution_request);
        self.running = true;
    }

    /// Start execution of an eval request.
    pub fn start_eval(
        &mut self,
        task_id: TaskId,
        player: &Obj,
        program: Program,
        world_state: &dyn WorldState,
    ) {
        let is_programmer = world_state
            .flags_of(player)
            .inspect_err(|e| error!(?e, "Failed to read player flags"))
            .map(|flags| flags.contains(ObjFlag::Programmer))
            .unwrap_or(false);
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
        world_state: &mut dyn WorldState,
        task_scheduler_client: &TaskSchedulerClient,
        session: &dyn Session,
        builtin_registry: &BuiltinRegistry,
        config: &FeaturesConfig,
    ) -> VMHostResponse {
        self.vm_exec_state.task_id = task_id;

        let exec_params = VmExecParams {
            task_scheduler_client,
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

        // Actually invoke the VM, asking it to loop until it's ready to yield back to us.
        let mut result = self.run_interpreter(&exec_params, world_state, session);
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
                    result = self
                        .vm_exec_state
                        .prepare_pass_verb(world_state, &pass_args);
                    continue;
                }
                ExecutionResult::PrepareVerbDispatch {
                    this,
                    verb_name,
                    args,
                } => {
                    result = self
                        .vm_exec_state
                        .verb_dispatch(&exec_params, world_state, this, verb_name, args)
                        .unwrap_or_else(ExecutionResult::PushError);
                    continue;
                }
                ExecutionResult::DispatchVerb(exec_request) => {
                    self.vm_exec_state.exec_call_request(exec_request);
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
                        world_state,
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
                            result = self.run_interpreter(&exec_params, world_state, session);
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
        world_state: &mut dyn WorldState,
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
                    world_state,
                    vm_exec_params.config,
                );
                (result, tick_count)
            }
            Frame::Bf(_) => {
                let result = self.vm_exec_state.reenter_builtin_function(
                    vm_exec_params,
                    world_state,
                    session,
                );
                (result, tick_count)
            }
        };
        self.vm_exec_state.tick_count = new_tick_count;

        result
    }

    /// Resume what you were doing after suspension.
    pub fn resume_execution(&mut self, value: Var) {
        self.vm_exec_state.start_time = Some(SystemTime::now());
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

    /// Get a copy of the current VM state, for later restoration.
    pub(crate) fn snapshot_state(&self) -> VMExecState {
        self.vm_exec_state.clone()
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

impl Encode for VmHost {
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        // The VM is not something we need to serialize.
        self.vm_exec_state.encode(encoder)?;
        self.max_stack_depth.encode(encoder)?;
        self.max_ticks.encode(encoder)?;
        self.max_time.as_secs().encode(encoder)?;

        // 'running' is a transient state, so we don't encode it, it will always be `true`
        // when we decode
        Ok(())
    }
}

impl<C> Decode<C> for VmHost {
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let vm_exec_state = VMExecState::decode(decoder)?;
        let max_stack_depth = Decode::decode(decoder)?;
        let max_ticks = Decode::decode(decoder)?;
        let max_time = Duration::from_secs(Decode::decode(decoder)?);

        Ok(Self {
            vm_exec_state,
            max_stack_depth,
            max_ticks,
            max_time,
            running: true,
            unsync: Default::default(),
        })
    }
}

impl<'de, C> BorrowDecode<'de, C> for VmHost {
    fn borrow_decode<D: BorrowDecoder<'de>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let vm_exec_state = VMExecState::borrow_decode(decoder)?;
        let max_stack_depth = BorrowDecode::borrow_decode(decoder)?;
        let max_ticks = BorrowDecode::borrow_decode(decoder)?;
        let max_time = Duration::from_secs(BorrowDecode::borrow_decode(decoder)?);

        Ok(Self {
            vm_exec_state,
            max_stack_depth,
            max_ticks,
            max_time,
            running: true,
            unsync: Default::default(),
        })
    }
}
