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

use crate::tasks::command_parse::ParsedCommand;
use crate::tasks::scheduler::AbortLimitReason;
use crate::tasks::sessions::Session;
use crate::tasks::task_messages::SchedulerControlMsg;
use crate::tasks::vm_host::VMHostResponse::{AbortLimit, ContinueOk, DispatchFork, Suspend};
use crate::tasks::{TaskId, VerbCall};
use crate::vm::{ExecutionResult, Fork, VerbExecutionRequest, VM};
use crate::vm::{FinallyReason, VMExecState};
use crate::vm::{UncaughtException, VmExecParams};
use moor_compiler::Name;
use moor_compiler::Program;
use moor_values::model::verb_info::VerbInfo;
use moor_values::model::verbs::BinaryType;
use moor_values::model::world_state::WorldState;
use moor_values::util::slice_ref::SliceRef;
use moor_values::var::objid::Objid;
use moor_values::var::Var;
use moor_values::AsByteBuffer;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::mpsc::UnboundedSender;
use tracing::{trace, warn};

/// Return values from exec_interpreter back to the Task scheduler loop
pub enum VMHostResponse {
    /// Tell the task to just keep on letting us do what we're doing.
    ContinueOk,
    /// Tell the task to ask the scheduler to dispatch a fork request, and then resume execution.
    DispatchFork(Fork),
    /// Tell the task to suspend us.
    Suspend(Option<Duration>),
    /// Tell the task Johnny 5 needs input from the client (`read` invocation).
    SuspendNeedInput,
    /// Task timed out or exceeded ticks.
    AbortLimit(AbortLimitReason),
    /// Tell the task that execution has completed, and the task is successful.
    CompleteSuccess(Var),
    /// The VM aborted. (FinallyReason::Abort in MOO VM)
    CompleteAbort,
    /// The VM threw an exception. (FinallyReason::Uncaught in MOO VM)
    CompleteException(UncaughtException),
}

/// A 'host' for running the MOO virtual machine inside a task.
pub struct VmHost {
    /// The VM we're running for the current execution.
    // TODO: The VM itself holds no mutable state, so having our own copy here is maybe pointless.
    // TODO: we will hold a few of these, one for each runtime/language and flip between them
    //   depending on the verbdef.binary_type() of the verb we're executing.
    vm: VM,
    /// Where we store current execution state for this host.
    vm_exec_state: VMExecState,
    /// The maximum stack depth for this task
    max_stack_depth: usize,
    /// The amount of ticks (opcode executions) allotted to this task
    max_ticks: usize,
    /// The maximum amount of time allotted to this task
    max_time: Duration,
    sessions: Arc<dyn Session>,
    scheduler_control_sender: UnboundedSender<(TaskId, SchedulerControlMsg)>,
    run_watch_send: tokio::sync::watch::Sender<bool>,
    run_watch_recv: tokio::sync::watch::Receiver<bool>,
}

impl VmHost {
    pub fn new(
        max_stack_depth: usize,
        max_ticks: usize,
        max_time: Duration,
        sessions: Arc<dyn Session>,
        scheduler_control_sender: UnboundedSender<(TaskId, SchedulerControlMsg)>,
    ) -> Self {
        let vm = VM::new();
        let exec_state = VMExecState::new();
        let (run_watch_send, run_watch_recv) = tokio::sync::watch::channel(false);
        // Created in an initial suspended state.
        Self {
            vm,
            vm_exec_state: exec_state,
            max_stack_depth,
            max_ticks,
            max_time,
            sessions,
            scheduler_control_sender,
            run_watch_send,
            run_watch_recv,
        }
    }
}

impl VmHost {
    /// Setup for executing a method initiated from a command.
    pub async fn start_call_command_verb(
        &mut self,
        task_id: TaskId,
        vi: VerbInfo,
        verb_call: VerbCall,
        command: ParsedCommand,
        permissions: Objid,
    ) {
        let binary = Self::decode_program(vi.verbdef().binary_type(), vi.binary().as_slice());
        let call_request = VerbExecutionRequest {
            permissions,
            resolved_verb: vi,
            call: verb_call,
            command: Some(command),
            program: binary,
        };

        self.start_execution(task_id, call_request).await
    }

    /// Setup for executing a method call in this VM.
    pub async fn start_call_method_verb(
        &mut self,
        task_id: TaskId,
        perms: Objid,
        verb_info: VerbInfo,
        verb_call: VerbCall,
    ) {
        let binary = Self::decode_program(
            verb_info.verbdef().binary_type(),
            verb_info.binary().as_slice(),
        );

        let call_request = VerbExecutionRequest {
            permissions: perms,
            resolved_verb: verb_info.clone(),
            call: verb_call,
            command: None,
            program: binary,
        };

        self.start_execution(task_id, call_request).await
    }

    /// Start execution of a fork request in the hosted VM.
    pub async fn start_fork(&mut self, task_id: TaskId, fork_request: Fork, suspended: bool) {
        self.vm_exec_state.tick_count = 0;
        self.vm
            .exec_fork_vector(&mut self.vm_exec_state, fork_request, task_id)
            .await;
        self.run_watch_send.send(!suspended).unwrap();
    }

    /// Start execution of a verb request.
    pub async fn start_execution(
        &mut self,
        task_id: TaskId,
        verb_execution_request: VerbExecutionRequest,
    ) {
        self.vm_exec_state.start_time = Some(SystemTime::now());
        self.vm_exec_state.tick_count = 0;
        self.vm
            .exec_call_request(&mut self.vm_exec_state, task_id, verb_execution_request)
            .await;
        self.run_watch_send.send(true).unwrap();
    }

    /// Start execution of an eval request.
    pub async fn start_eval(&mut self, task_id: TaskId, player: Objid, program: Program) {
        self.vm_exec_state.start_time = Some(SystemTime::now());
        self.vm_exec_state.tick_count = 0;
        self.vm
            .exec_eval_request(&mut self.vm_exec_state, task_id, player, player, program)
            .await;
        self.run_watch_send.send(true).unwrap();
    }

    /// Run the hosted VM.
    pub async fn exec_interpreter(
        &mut self,
        task_id: TaskId,
        world_state: &mut dyn WorldState,
    ) -> VMHostResponse {
        self.run_watch_recv
            .wait_for(|running| *running)
            .await
            .unwrap();

        // Check ticks and seconds, and abort the task if we've exceeded the limits.
        let time_left = match self.vm_exec_state.start_time {
            Some(start_time) => {
                let elapsed = start_time.elapsed().expect("Could not get elapsed time");
                if elapsed > self.max_time {
                    return AbortLimit(AbortLimitReason::Time(elapsed));
                }
                Some(self.max_time - elapsed)
            }
            None => None,
        };
        if self.vm_exec_state.tick_count >= self.max_ticks {
            return AbortLimit(AbortLimitReason::Ticks(self.vm_exec_state.tick_count));
        }
        let exec_params = VmExecParams {
            scheduler_sender: self.scheduler_control_sender.clone(),
            max_stack_depth: self.max_stack_depth,
            ticks_left: self.max_ticks - self.vm_exec_state.tick_count,
            time_left,
        };
        let pre_exec_tick_count = self.vm_exec_state.tick_count;

        // Actually invoke the VM, asking it to loop until it's ready to yield back to us.
        let mut result = self
            .vm
            .exec(
                &exec_params,
                &mut self.vm_exec_state,
                world_state,
                self.sessions.clone(),
                self.max_ticks,
            )
            .await;

        let post_exec_tick_count = self.vm_exec_state.tick_count;
        trace!(
            task_id,
            executed_ticks = post_exec_tick_count - pre_exec_tick_count,
            ?result,
            "Executed ticks",
        );
        while self.is_running() {
            match result {
                ExecutionResult::More => return ContinueOk,
                ExecutionResult::ContinueVerb {
                    permissions,
                    resolved_verb,
                    call,
                    command,
                    trampoline,
                    trampoline_arg,
                } => {
                    trace!(task_id, call = ?call, "Task continue, call into verb");

                    self.vm_exec_state.top_mut().bf_trampoline_arg = trampoline_arg;
                    self.vm_exec_state.top_mut().bf_trampoline = trampoline;

                    let program = Self::decode_program(
                        resolved_verb.verbdef().binary_type(),
                        resolved_verb.binary().as_slice(),
                    );

                    let call_request = VerbExecutionRequest {
                        permissions,
                        resolved_verb,
                        call,
                        command,
                        program,
                    };

                    self.vm
                        .exec_call_request(&mut self.vm_exec_state, task_id, call_request)
                        .await;
                    return ContinueOk;
                }
                ExecutionResult::PerformEval {
                    permissions,
                    player,
                    program,
                } => {
                    self.vm
                        .exec_eval_request(&mut self.vm_exec_state, 0, permissions, player, program)
                        .await;
                    return ContinueOk;
                }
                ExecutionResult::ContinueBuiltin {
                    bf_func_num: bf_offset,
                    arguments: args,
                } => {
                    let exec_params = VmExecParams {
                        max_stack_depth: self.max_stack_depth,
                        scheduler_sender: self.scheduler_control_sender.clone(),
                        ticks_left: self.max_ticks - self.vm_exec_state.tick_count,
                        time_left,
                    };
                    // Ask the VM to execute the builtin function.
                    // This will push the result onto the stack.
                    // After this we will loop around and check the result.
                    result = self
                        .vm
                        .call_builtin_function(
                            &mut self.vm_exec_state,
                            bf_offset,
                            &args,
                            &exec_params,
                            world_state,
                            self.sessions.clone(),
                        )
                        .await;
                    continue;
                }
                ExecutionResult::DispatchFork(fork_request) => {
                    return DispatchFork(fork_request);
                }
                ExecutionResult::Suspend(delay) => {
                    self.run_watch_send.send(false).unwrap();
                    return Suspend(delay);
                }
                ExecutionResult::NeedInput => {
                    self.run_watch_send.send(false).unwrap();
                    return VMHostResponse::SuspendNeedInput;
                }
                ExecutionResult::Complete(a) => {
                    return VMHostResponse::CompleteSuccess(a);
                }
                ExecutionResult::Exception(fr) => {
                    trace!(task_id, result = ?fr, "Task exception");

                    return match &fr {
                        FinallyReason::Abort => VMHostResponse::CompleteAbort,
                        FinallyReason::Uncaught(exception) => {
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
            }
        }

        // We're not running and we didn't get a completion signal from the VM - we must have been
        // asked to stop by the scheduler.
        warn!(task_id, "VM host stopped by task");
        VMHostResponse::CompleteAbort
    }

    /// Resume what you were doing after suspension.
    pub async fn resume_execution(&mut self, value: Var) {
        // coming back from suspend, we need a return value to feed back to `bf_suspend`
        self.vm_exec_state.top_mut().push(value);
        self.vm_exec_state.start_time = Some(SystemTime::now());
        self.vm_exec_state.tick_count = 0;
        self.run_watch_send.send(true).unwrap();
    }
    pub fn is_running(&self) -> bool {
        *self.run_watch_recv.borrow()
    }
    pub async fn stop(&mut self) {
        self.run_watch_send.send(false).unwrap();
    }
    pub fn decode_program(binary_type: BinaryType, binary_bytes: &[u8]) -> Program {
        match binary_type {
            BinaryType::LambdaMoo18X => Program::from_sliceref(SliceRef::from_bytes(binary_bytes)),
            _ => panic!("Unsupported binary type {:?}", binary_type),
        }
    }
    pub fn set_variable(&mut self, task_id_var: Name, value: Var) {
        self.vm_exec_state
            .top_mut()
            .set_var_offset(task_id_var, value)
            .expect("Could not set forked task id");
    }
    pub fn permissions(&self) -> Objid {
        self.vm_exec_state.top().permissions
    }
    pub fn verb_name(&self) -> String {
        self.vm_exec_state.top().verb_name.clone()
    }
    pub fn verb_definer(&self) -> Objid {
        self.vm_exec_state.top().verb_definer()
    }
    pub fn this(&self) -> Objid {
        self.vm_exec_state.top().this
    }
    pub fn line_number(&self) -> usize {
        self.vm_exec_state
            .top()
            .find_line_no(self.vm_exec_state.top().pc)
            .unwrap_or(0)
    }

    pub fn reset_ticks(&mut self) {
        self.vm_exec_state.tick_count = 0;
    }
    pub fn reset_time(&mut self) {
        self.vm_exec_state.start_time = Some(SystemTime::now());
    }
    pub fn args(&self) -> Vec<Var> {
        self.vm_exec_state.top().args.clone()
    }
}
