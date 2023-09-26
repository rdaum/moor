use crate::tasks::scheduler::{AbortLimitReason, TaskDescription};
use crate::tasks::TaskId;
use crate::vm::vm_unwind::UncaughtException;
use crate::vm::ForkRequest;

use crate::vm::opcode::Program;
use moor_values::model::permissions::Perms;
use moor_values::model::world_state::WorldState;
use moor_values::model::CommandError;
use moor_values::var::objid::Objid;
use moor_values::var::Var;
use std::time::SystemTime;
use tokio::sync::oneshot;

/// Messages sent to tasks from the scheduler to tell the task to do things.
pub enum TaskControlMsg {
    /// The scheduler is telling the task to parse a command and execute whatever verbs are
    /// associated with it.
    StartCommandVerb { player: Objid, command: String },
    /// The scheduler is telling the task to run a (method) verb.
    StartVerb {
        player: Objid,
        vloc: Objid,
        verb: String,
        args: Vec<Var>,
        argstr: String,
    },
    /// The scheduler is telling the task to run a task that was forked from another task.
    /// ForkRequest contains the information on the fork vector and other information needed to
    /// set up execution.
    StartFork {
        task_id: TaskId,
        fork_request: ForkRequest,
        // If we're starting in a suspended state. If this is true, an explicit Resume from the
        // scheduler will be required to start the task.
        suspended: bool,
    },
    /// The scheduler is telling the task to evaluate a specific (MOO) program.
    /// TODO: remove the MOO-specificity of this.
    StartEval { player: Objid, program: Program },
    /// The scheduler is telling the task to resume execution. Use the given world state
    /// (transaction) and permissions when doing so.
    Resume(Box<dyn WorldState>, Var),
    /// The scheduler is asking the task to describe itself.
    /// TODO: This causes deadlock if the task _requesting_ the description is the task being
    ///   described, so I need to rethink this.
    Describe(oneshot::Sender<TaskDescription>),
    /// The scheduler is telling the task to abort itself.
    Abort,
}

/// The ad-hoc messages that can be sent from tasks (or VM) up to the scheduler.
#[derive(Debug)]
pub enum SchedulerControlMsg {
    /// Everything executed. The task is done.
    TaskSuccess(Var),
    /// A 'StartCommandVerb' type task failed to parse or match the command.
    TaskCommandError(CommandError),
    /// The verb to be executed was not found.
    TaskVerbNotFound(Objid, String),
    /// An execption was thrown while executing the verb.
    TaskException(UncaughtException),
    /// The task is requesting that it be forked.
    TaskRequestFork(ForkRequest, oneshot::Sender<TaskId>),
    /// The task is letting us know it was cancelled.
    TaskAbortCancelled,
    /// The task is letting us know that it has reached its abort limits.
    TaskAbortLimitsReached(AbortLimitReason),
    /// Tell the scheduler that the task in a suspended state, with a time to resume (if any)
    TaskSuspend(Option<SystemTime>),
    /// Task is requesting a list of all other tasks known to the scheduler.
    DescribeOtherTasks(oneshot::Sender<Vec<TaskDescription>>),
    /// Task is requesting that the scheduler abort another task.
    KillTask {
        victim_task_id: TaskId,
        sender_permissions: Perms,
        result_sender: oneshot::Sender<Var>,
    },
    /// Task is requesting that the scheduler resume another task.
    ResumeTask {
        queued_task_id: TaskId,
        sender_permissions: Perms,
        return_value: Var,
        result_sender: oneshot::Sender<Var>,
    },
    /// Task is requesting that the scheduler boot a player.
    BootPlayer {
        player: Objid,
        sender_permissions: Perms,
    },
}
