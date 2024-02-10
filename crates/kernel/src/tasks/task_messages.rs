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

use crate::tasks::scheduler::AbortLimitReason;
use crate::tasks::{TaskDescription, TaskId};
use crate::vm::vm_unwind::UncaughtException;
use crate::vm::Fork;
use std::sync::Arc;

use kanal::OneshotSender;
use moor_compiler::Program;

use moor_values::model::{CommandError, NarrativeEvent};
use moor_values::model::{Perms, WorldStateSource};
use moor_values::var::Objid;
use moor_values::var::Var;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub enum TaskStart {
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
        fork_request: Fork,
        // If we're starting in a suspended state. If this is true, an explicit Resume from the
        // scheduler will be required to start the task.
        suspended: bool,
    },
    /// The scheduler is telling the task to evaluate a specific (MOO) program.
    StartEval { player: Objid, program: Program },
}

/// Messages sent to tasks from the scheduler to tell the task to do things.
pub enum TaskControlMsg {
    /// The scheduler is telling the task to restart itself in a new transaction.
    Restart(Arc<dyn WorldStateSource>),
    /// The scheduler is telling the task to resume execution. Use the given world state
    /// (transaction) and permissions when doing so.
    Resume(Arc<dyn WorldStateSource>, Var),
    /// The scheduler is giving the task the input it requested from the client, and is asking it
    /// to resume execution, using the given world state (transaction) to do so.
    ResumeReceiveInput(Arc<dyn WorldStateSource>, String),
    /// The scheduler is asking the task to describe itself.
    /// TODO: Rethink task 'description' mechanism.
    ///   Causes deadlock if the task _requesting_ the description is the task being
    ///   described, so I need to rethink this. Right now this is prevented by the
    ///   runtime, but it's not a good design.
    Describe(OneshotSender<TaskDescription>),
    /// The scheduler is telling the task to abort itself.
    Abort,
}

/// The ad-hoc messages that can be sent from tasks (or VM) up to the scheduler.
#[derive(Debug)]
pub enum SchedulerControlMsg {
    /// Everything executed. The task is done.
    TaskSuccess(Var),
    /// The task hit an unresolvable transaction serialization conflict, and needs to be restarted
    /// in a new transaction.
    TaskConflictRetry,
    /// A 'StartCommandVerb' type task failed to parse or match the command.
    TaskCommandError(CommandError),
    /// The verb to be executed was not found.
    TaskVerbNotFound(Objid, String),
    /// An execption was thrown while executing the verb.
    TaskException(UncaughtException),
    /// The task is requesting that it be forked.
    TaskRequestFork(Fork, OneshotSender<TaskId>),
    /// The task is letting us know it was cancelled.
    TaskAbortCancelled,
    /// The task is letting us know that it has reached its abort limits.
    TaskAbortLimitsReached(AbortLimitReason),
    /// Tell the scheduler that the task in a suspended state, with a time to resume (if any)
    TaskSuspend(Option<SystemTime>),
    /// Tell the scheduler we're suspending until we get input from the client.
    TaskRequestInput,
    /// Task is requesting a list of all other tasks known to the scheduler.
    DescribeOtherTasks(OneshotSender<Vec<TaskDescription>>),
    /// Task is requesting that the scheduler abort another task.
    KillTask {
        victim_task_id: TaskId,
        sender_permissions: Perms,
        result_sender: OneshotSender<Var>,
    },
    /// Task is requesting that the scheduler resume another task.
    ResumeTask {
        queued_task_id: TaskId,
        sender_permissions: Perms,
        return_value: Var,
        result_sender: OneshotSender<Var>,
    },
    /// Task is requesting that the scheduler boot a player.
    BootPlayer {
        player: Objid,
        sender_permissions: Perms,
    },
    /// Task is requesting that a textdump checkpoint happen, to the configured file.
    Checkpoint,
    Notify {
        player: Objid,
        event: NarrativeEvent,
    },
    /// Task requesting shutdown
    Shutdown(Option<String>),
}
