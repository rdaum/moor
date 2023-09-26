use crate::compiler::labels::Name;
use crate::tasks::command_parse::ParsedCommand;
use crate::tasks::scheduler::AbortLimitReason;
use crate::tasks::sessions::Session;
use crate::tasks::task_messages::SchedulerControlMsg;
use crate::tasks::vm_host::VMHostResponse::{AbortLimit, ContinueOk, DispatchFork, Suspend};
use crate::tasks::vm_host::{VMHost, VMHostResponse};
use crate::tasks::{TaskId, VerbCall};
use crate::vm::opcode::Program;
use crate::vm::vm_execute::VmExecParams;
use crate::vm::vm_unwind::FinallyReason;
use crate::vm::{ExecutionResult, ForkRequest, VerbExecutionRequest, VM};
use anyhow::bail;
use async_trait::async_trait;
use moor_values::model::verb_info::VerbInfo;
use moor_values::model::verbs::BinaryType;
use moor_values::model::world_state::WorldState;
use moor_values::util::slice_ref::SliceRef;
use moor_values::var::error::Error;
use moor_values::var::objid::Objid;
use moor_values::var::Var;
use moor_values::AsByteBuffer;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::Notify;
use tracing::{trace, warn};

/// A 'host' for running the MOO virtual machine inside a task.
pub struct MooVmHost {
    vm: VM,
    suspension_signal: SuspendSignal,
    running_method: bool,
    /// The maximum stack detph for this task
    max_stack_depth: usize,
    /// The amount of ticks (opcode executions) allotted to this task
    max_ticks: usize,
    /// The maximum amount of time allotted to this task
    max_time: Duration,
    sessions: Arc<dyn Session>,
    scheduler_control_sender: UnboundedSender<SchedulerControlMsg>,
}

impl MooVmHost {
    pub fn new(
        vm: VM,
        max_stack_depth: usize,
        max_ticks: usize,
        max_time: Duration,
        sessions: Arc<dyn Session>,
        scheduler_control_sender: UnboundedSender<SchedulerControlMsg>,
    ) -> Self {
        // Created in an initial suspended state.
        Self {
            vm,
            running_method: false,
            suspension_signal: SuspendSignal::suspended(),
            max_stack_depth,
            max_ticks,
            max_time,
            sessions,
            scheduler_control_sender,
        }
    }
}

#[async_trait]
impl VMHost<Program> for MooVmHost {
    /// Setup for executing a method initited from a command.
    async fn start_call_command_verb(
        &mut self,
        task_id: TaskId,
        vi: VerbInfo,
        verb_call: VerbCall,
        command: ParsedCommand,
        permissions: Objid,
    ) -> Result<(), anyhow::Error> {
        let binary = Self::decode_program(vi.verbdef().binary_type(), vi.binary().as_slice())?;
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
    async fn start_call_method_verb(
        &mut self,
        task_id: TaskId,
        perms: Objid,
        verb_info: VerbInfo,
        verb_call: VerbCall,
    ) -> Result<(), anyhow::Error> {
        let binary = Self::decode_program(
            verb_info.verbdef().binary_type(),
            verb_info.binary().as_slice(),
        )?;

        let call_request = VerbExecutionRequest {
            permissions: perms,
            resolved_verb: verb_info.clone(),
            call: verb_call,
            command: None,
            program: binary,
        };

        self.start_execution(task_id, call_request).await
    }
    async fn start_fork(
        &mut self,
        task_id: TaskId,
        fork_request: ForkRequest,
        suspended: bool,
    ) -> Result<(), anyhow::Error> {
        self.vm.tick_count = 0;
        self.vm.exec_fork_vector(fork_request, task_id).await;
        self.running_method = !suspended;
        if !suspended {
            self.suspension_signal.start().await;
        } else {
            self.suspension_signal.suspend().await;
        }
        Ok(())
    }
    /// Start execution of a verb request.
    async fn start_execution(
        &mut self,
        task_id: TaskId,
        verb_execution_request: VerbExecutionRequest,
    ) -> Result<(), anyhow::Error> {
        self.vm.start_time = Some(SystemTime::now());
        self.vm.tick_count = 0;
        self.vm
            .exec_call_request(task_id, verb_execution_request)
            .await;
        self.running_method = true;
        self.suspension_signal.start().await;
        Ok(())
    }
    async fn start_eval(
        &mut self,
        task_id: TaskId,
        player: Objid,
        program: Program,
    ) -> Result<(), Error> {
        self.vm.start_time = Some(SystemTime::now());
        self.vm.tick_count = 0;
        self.vm
            .exec_eval_request(task_id, player, player, program)
            .await;
        self.running_method = true;
        self.suspension_signal.start().await;
        Ok(())
    }
    async fn exec_interpreter(
        &mut self,
        task_id: TaskId,
        world_state: &mut dyn WorldState,
    ) -> Result<VMHostResponse, anyhow::Error> {
        if !self.running_method {
            self.suspension_signal.wait_for_start().await;
            assert!(self.running_method);
        }

        // Check ticks and seconds, and abort the task if we've exceeded the limits.
        let time_left = match self.vm.start_time {
            Some(start_time) => {
                let elapsed = start_time.elapsed()?;
                if elapsed > self.max_time {
                    return Ok(AbortLimit(AbortLimitReason::Time(elapsed)));
                }
                Some(self.max_time - elapsed)
            }
            None => None,
        };
        if self.vm.tick_count >= self.max_ticks {
            return Ok(AbortLimit(AbortLimitReason::Ticks(self.vm.tick_count)));
        }
        let exec_params = VmExecParams {
            world_state,
            session: self.sessions.clone(),
            scheduler_sender: self.scheduler_control_sender.clone(),
            max_stack_depth: self.max_stack_depth,
            ticks_left: self.max_ticks - self.vm.tick_count,
            time_left,
        };
        let pre_exec_tick_count = self.vm.tick_count;

        // Actually invoke the VM, asking it to loop until it's ready to yield back to us.
        let mut result = self.vm.exec(exec_params, self.max_ticks).await;

        let post_exec_tick_count = self.vm.tick_count;
        trace!(
            task_id,
            executed_ticks = post_exec_tick_count - pre_exec_tick_count,
            ?result,
            "Executed ticks",
        );
        while self.running_method {
            match result {
                ExecutionResult::More => return Ok(ContinueOk),
                ExecutionResult::ContinueVerb {
                    permissions,
                    resolved_verb,
                    call,
                    command,
                    trampoline,
                    trampoline_arg,
                } => {
                    trace!(task_id, call = ?call, "Task continue, call into verb");

                    self.vm.top_mut().bf_trampoline_arg = trampoline_arg;
                    self.vm.top_mut().bf_trampoline = trampoline;

                    let program = Self::decode_program(
                        resolved_verb.verbdef().binary_type(),
                        resolved_verb.binary().as_slice(),
                    )?;

                    let call_request = VerbExecutionRequest {
                        permissions,
                        resolved_verb,
                        call,
                        command,
                        program,
                    };

                    self.vm.exec_call_request(task_id, call_request).await;
                    return Ok(ContinueOk);
                }
                ExecutionResult::PerformEval {
                    permissions,
                    player,
                    program,
                } => {
                    self.vm
                        .exec_eval_request(0, permissions, player, program)
                        .await;
                    return Ok(ContinueOk);
                }
                ExecutionResult::ContinueBuiltin {
                    bf_func_num: bf_offset,
                    arguments: args,
                } => {
                    let mut exec_params = VmExecParams {
                        world_state,
                        session: self.sessions.clone(),
                        max_stack_depth: self.max_stack_depth,
                        scheduler_sender: self.scheduler_control_sender.clone(),
                        ticks_left: self.max_ticks - self.vm.tick_count,
                        time_left,
                    };
                    // Ask the VM to execute the builtin function.
                    // This will push the result onto the stack.
                    // After this we will loop around and check the result.
                    result = self
                        .vm
                        .call_builtin_function(bf_offset, &args, &mut exec_params)
                        .await;
                    continue;
                }
                ExecutionResult::DispatchFork(fork_request) => {
                    return Ok(DispatchFork(fork_request));
                }
                ExecutionResult::Suspend(delay) => {
                    self.running_method = false;
                    self.suspension_signal.suspend().await;
                    return Ok(Suspend(delay));
                }
                ExecutionResult::Complete(a) => {
                    return Ok(VMHostResponse::CompleteSuccess(a));
                }
                ExecutionResult::Exception(fr) => {
                    trace!(task_id, result = ?fr, "Task exception");

                    return match &fr {
                        FinallyReason::Abort => Ok(VMHostResponse::CompleteAbort),
                        FinallyReason::Uncaught(exception) => {
                            Ok(VMHostResponse::CompleteException(exception.clone()))
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
        Ok(VMHostResponse::CompleteAbort)
    }
    /// Resume what you were doing after suspension.
    async fn resume_execution(&mut self, value: Var) -> Result<(), anyhow::Error> {
        // coming back from suspend, we need a return value to feed back to `bf_suspend`
        self.vm.top_mut().push(value);
        self.vm.start_time = Some(SystemTime::now());
        self.vm.tick_count = 0;
        assert!(!self.running_method);
        self.running_method = true;
        self.suspension_signal.start().await;
        Ok(())
    }
    fn is_running(&self) -> bool {
        self.running_method
    }
    async fn stop(&mut self) {
        self.running_method = false;
        self.suspension_signal.suspend().await;
    }
    fn decode_program(
        binary_type: BinaryType,
        binary_bytes: &[u8],
    ) -> Result<Program, anyhow::Error> {
        match binary_type {
            BinaryType::LambdaMoo18X => {
                Ok(Program::from_sliceref(SliceRef::from_bytes(binary_bytes)))
            }
            _ => bail!("Unsupported binary type {:?}", binary_type),
        }
    }
    fn set_variable(&mut self, task_id_var: Name, value: Var) {
        self.vm
            .top_mut()
            .set_var_offset(task_id_var, value)
            .expect("Could not set forked task id");
    }
    fn permissions(&self) -> Objid {
        self.vm.top().permissions
    }
    fn verb_name(&self) -> String {
        self.vm.top().verb_name.clone()
    }
    fn verb_definer(&self) -> Objid {
        self.vm.top().verb_definer()
    }
    fn this(&self) -> Objid {
        self.vm.top().this
    }
    fn line_number(&self) -> usize {
        // self.vm.top().line_number
        // TODO: implement line number tracking
        0
    }

    fn args(&self) -> Vec<Var> {
        self.vm.top().args.clone()
    }
}

/// A little utility to do a kind of condition variable to represent our internal suspended state,
/// to allow us to do a blocking async wait for wake-ups.
/// TODO: It's quite possible this thing might have issues on cancellations...
struct SuspendSignal(Option<Notify>);

impl SuspendSignal {
    fn suspended() -> Self {
        Self(Some(Notify::new()))
    }

    async fn suspend(&mut self) {
        if self.0.is_none() {
            self.0 = Some(Notify::new());
        }
    }
    async fn start(&mut self) {
        if let Some(notify) = self.0.take() {
            notify.notify_waiters()
        }
    }
    async fn wait_for_start(&mut self) {
        if let Some(notify) = self.0.take() {
            notify.notified().await;
        }
    }
}
