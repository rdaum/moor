use std::time::Duration;

use crate::compiler::labels::Name;
use crate::tasks::command_parse::ParsedCommand;
use crate::tasks::scheduler::AbortLimitReason;
use crate::tasks::{TaskId, VerbCall};
use crate::vm::vm_unwind::UncaughtException;
use crate::vm::{ForkRequest, VerbExecutionRequest};
use async_trait::async_trait;
use moor_value::model::verb_info::VerbInfo;
use moor_value::model::verbs::BinaryType;
use moor_value::model::world_state::WorldState;
use moor_value::var::error::Error;
use moor_value::var::objid::Objid;
use moor_value::var::Var;

/// Return values from exec_interpreter back to the Task scheduler loop
pub enum VMHostResponse {
    /// Tell the task to just keep on letting us do what we're doing.
    ContinueOk,
    /// Tell the task to ask the scheduler to dispatch a fork request, and then resume execution.
    DispatchFork(ForkRequest),
    /// Tell the task to suspend us.
    Suspend(Option<Duration>),
    /// Task timed out or exceeded ticks.
    AbortLimit(AbortLimitReason),
    /// Tell the task that execution has completed, and the task is successful.
    CompleteSuccess(Var),
    /// The VM aborted. (FinallyReason::Abort in MOO VM)
    CompleteAbort,
    /// The VM threw an exception. (FinallyReason::Uncaught in MOO VM)
    CompleteException(UncaughtException),
}

/// A "VM Host" is the interface between the Task scheduler and a virtual machine runtime.
/// Defining the level of abstraction for executing programmes which run in tasks against shared
/// virtual state.
#[async_trait]
pub trait VMHost<ProgramType> {
    /// Setup for executing a method call in this VM.
    async fn start_call_command_verb(
        &mut self,
        task_id: TaskId,
        vi: VerbInfo,
        verb_call: VerbCall,
        command: ParsedCommand,
        permissions: Objid,
    ) -> Result<(), anyhow::Error>;

    /// Setup for executing a method call in this VM.
    async fn start_call_method_verb(
        &mut self,
        task_id: TaskId,
        perms: Objid,
        verb_info: VerbInfo,
        verb_call: VerbCall,
    ) -> Result<(), anyhow::Error>;

    /// Setup for dispatching into a fork request.
    async fn start_fork(
        &mut self,
        task_id: TaskId,
        fork_request: ForkRequest,
        suspended: bool,
    ) -> Result<(), anyhow::Error>;

    /// Signal the need to start execution of a verb request.
    async fn start_execution(
        &mut self,
        task_id: TaskId,
        verb_execution_request: VerbExecutionRequest,
    ) -> Result<(), anyhow::Error>;

    /// Setup for executing a free-standing evaluation of `program`.
    async fn start_eval(
        &mut self,
        task_id: TaskId,
        player: Objid,
        program: ProgramType,
    ) -> Result<(), Error>;

    /// The meat of the VM host: this is invoked repeatedly by the task scheduler loop to drive the
    /// VM. The responses from this function are used to determine what the task/scheduler should do
    /// next with this VM.
    async fn exec_interpreter(
        &mut self,
        task_id: TaskId,
        world_state: &mut dyn WorldState,
    ) -> Result<VMHostResponse, anyhow::Error>;

    /// Ask the host to resume what it was doing after suspension.
    async fn resume_execution(&mut self, value: Var) -> Result<(), anyhow::Error>;

    /// Return true if the VM is currently running.
    fn is_running(&self) -> bool;

    /// Stop a running VM.
    fn stop(&mut self);

    /// Decodes a binary into opcodes that this kind of VM can execute.
    fn decode_program(
        binary_type: BinaryType,
        binary_bytes: &[u8],
    ) -> Result<ProgramType, anyhow::Error>;

    /// Attempt to set a variable inside the VM's current top stack frame.
    /// The sole use of this is to set the task id variable for forked tasks or resumed tasks.
    // TODO: a bit of an abstraction break, might require some better thought.
    fn set_variable(&mut self, task_id_var: Name, value: Var);

    /// Return the operating user permissions in place.
    fn permissions(&self) -> Objid;

    /// Return the name of the 'verb' (method) being executed by this VM.
    fn verb_name(&self) -> String;

    /// Return who is the responsible 'definer' of the verb being executed by this VM.
    fn verb_definer(&self) -> Objid;

    /// Return the object id of the object being operated on by this VM.
    fn this(&self) -> Objid;

    /// Return the current source line number being executed by this VM.
    fn line_number(&self) -> usize;

    /// Return the arguments to the verb being executed by this VM.
    fn args(&self) -> Vec<Var>;
}
