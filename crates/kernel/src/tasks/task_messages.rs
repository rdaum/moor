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

use crate::tasks::scheduler::{AbortLimitReason, SchedulerError};
use crate::tasks::{TaskDescription, TaskHandle, TaskId};
use crate::vm::vm_unwind::UncaughtException;
use crate::vm::Fork;
use std::sync::Arc;

use moor_compiler::Program;

use crate::tasks::sessions::Session;
use crate::tasks::task::Task;
use moor_values::model::Perms;
use moor_values::model::{CommandError, NarrativeEvent};
use moor_values::var::Var;
use moor_values::var::{List, Objid};
use std::time::Instant;
use uuid::Uuid;

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
        args: List,
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

pub enum SchedulerMsg {
    /// Submit a command to be executed by the player.
    SubmitCommandTask {
        player: Objid,
        command: String,
        session: Arc<dyn Session>,
        reply: oneshot::Sender<Result<TaskHandle, SchedulerError>>,
    },
    /// Submit a top-level verb (method) invocation to be executed on behalf of the player.
    SubmitVerbTask {
        player: Objid,
        vloc: Objid,
        verb: String,
        args: Vec<Var>,
        argstr: String,
        perms: Objid,
        session: Arc<dyn Session>,
        reply: oneshot::Sender<Result<TaskHandle, SchedulerError>>,
    },
    /// Submit input to a task that is waiting for it.
    SubmitTaskInput {
        player: Objid,
        input_request_id: Uuid,
        input: String,
        reply: oneshot::Sender<Result<(), SchedulerError>>,
    },
    /// Submit an out-of-band task to be executed
    SubmitOobTask {
        player: Objid,
        command: Vec<String>,
        argstr: String,
        session: Arc<dyn Session>,
        reply: oneshot::Sender<Result<TaskHandle, SchedulerError>>,
    },
    /// Submit an eval task
    SubmitEvalTask {
        player: Objid,
        perms: Objid,
        program: Program,
        sessions: Arc<dyn Session>,
        reply: oneshot::Sender<Result<TaskHandle, SchedulerError>>,
    },
    /// Submit a (non-task specific) request to shutdown the scheduler
    Shutdown(String, oneshot::Sender<Result<(), SchedulerError>>),
}

/// The ad-hoc messages that can be sent from tasks (or VM) up to the scheduler.
#[derive(Debug)]
pub enum TaskControlMsg {
    /// Everything executed. The task is done.
    TaskSuccess(Var),
    /// The task hit an unresolvable transaction serialization conflict, and needs to be restarted
    /// in a new transaction.
    TaskConflictRetry(Task),
    /// A 'StartCommandVerb' type task failed to parse or match the command.
    TaskCommandError(CommandError),
    /// The verb to be executed was not found.
    TaskVerbNotFound(Objid, String),
    /// An exception was thrown while executing the verb.
    TaskException(UncaughtException),
    /// The task is requesting that it be forked.
    TaskRequestFork(Fork, oneshot::Sender<TaskId>),
    /// The task is letting us know it was cancelled.
    TaskAbortCancelled,
    /// The task is letting us know that it has reached its abort limits.
    TaskAbortLimitsReached(AbortLimitReason),
    /// Tell the scheduler that the task in a suspended state, with a time to resume (if any)
    TaskSuspend(Option<Instant>, Task),
    /// Tell the scheduler we're suspending until we get input from the client.
    TaskRequestInput(Task),
    /// Task is requesting a list of all other tasks known to the scheduler.
    RequestQueuedTasks(oneshot::Sender<Vec<TaskDescription>>),
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
    /// Task is requesting that a textdump checkpoint happen, to the configured file.
    Checkpoint,
    Notify {
        player: Objid,
        event: NarrativeEvent,
    },
    /// Request that the server refresh its set of information off $server_options
    RefreshServerOptions { player: Objid },
    /// Task requesting shutdown
    Shutdown(Option<String>),
}
