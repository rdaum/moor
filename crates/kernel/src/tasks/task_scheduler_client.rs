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

use std::time::Duration;

use flume::Sender;

use crate::tasks::task::Task;
use crate::tasks::{TaskDescription, TaskStart};
use crate::vm::{Fork, TaskSuspend};
use moor_common::model::{Perms, WorldState};
use moor_common::tasks::{
    AbortLimitReason, CommandError, Exception, NarrativeEvent, SchedulerError, TaskId,
};
use moor_var::Symbol;
use moor_var::Var;
use moor_var::{Error, Obj};

/// Information about a worker type and its current state
#[derive(Debug, Clone)]
pub struct WorkerInfo {
    pub worker_type: Symbol,
    pub worker_count: usize,
    pub total_queue_size: usize,
    pub avg_response_time_ms: f64,
    pub last_ping_ago_secs: f64,
}

/// A handle for talking to the scheduler from within a task.
#[derive(Clone)]
pub struct TaskSchedulerClient {
    task_id: TaskId,
    scheduler_sender: Sender<(TaskId, TaskControlMsg)>,
}

impl TaskSchedulerClient {
    pub fn new(task_id: TaskId, scheduler_sender: Sender<(TaskId, TaskControlMsg)>) -> Self {
        Self {
            task_id,
            scheduler_sender,
        }
    }

    /// Send a message to the scheduler that the task has completed successfully, with the given
    /// return value.
    pub fn success(&self, var: Var) {
        self.scheduler_sender
            .send((self.task_id, TaskControlMsg::TaskSuccess(var)))
            .expect("Could not deliver client message -- scheduler shut down?");
    }

    /// Send a message to the scheduler that the task has hit a transaction conflict and needs to be
    /// retried from the beginning.
    pub fn conflict_retry(&self, task: Box<Task>) {
        self.scheduler_sender
            .send((self.task_id, TaskControlMsg::TaskConflictRetry(task)))
            .expect("Could not deliver client message -- scheduler shut down?");
    }

    /// Send a message to the scheduler that the task has failed to parse or match the command.
    pub fn command_error(&self, error: CommandError) {
        self.scheduler_sender
            .send((self.task_id, TaskControlMsg::TaskCommandError(error)))
            .expect("Could not deliver client message -- scheduler shut down?");
    }

    /// Send a message to the scheduler that the verb to be executed was not found.
    pub fn verb_not_found(&self, what: Var, verb: Symbol) {
        self.scheduler_sender
            .send((self.task_id, TaskControlMsg::TaskVerbNotFound(what, verb)))
            .expect("Could not deliver client message -- scheduler shut down?");
    }

    /// Send a message to the scheduler that an exception was thrown while executing the verb.
    pub fn exception(&self, exception: Box<Exception>) {
        self.scheduler_sender
            .send((self.task_id, TaskControlMsg::TaskException(exception)))
            .expect("Could not deliver client message -- scheduler shut down?");
    }

    /// Send a message to the scheduler that the task is requesting to fork itself.
    pub fn request_fork(&self, fork: Box<Fork>) -> TaskId {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send((self.task_id, TaskControlMsg::TaskRequestFork(fork, reply)))
            .expect("Could not deliver client message -- scheduler shut down?");
        receive
            .recv()
            .expect("Could not receive task id -- scheduler shut down?")
    }

    /// Send a message to the scheduler that the task has been cancelled.
    pub fn abort_cancelled(&self) {
        self.scheduler_sender
            .send((self.task_id, TaskControlMsg::TaskAbortCancelled))
            .expect("Could not deliver client message -- scheduler shut down?");
    }

    /// Send a message to the scheduler that the task has reached its abort limits.
    pub fn abort_limits_reached(
        &self,
        reason: AbortLimitReason,
        this: Var,
        verb_name: Symbol,
        line_number: usize,
    ) {
        self.scheduler_sender
            .send((
                self.task_id,
                TaskControlMsg::TaskAbortLimitsReached(reason, this, verb_name, line_number),
            ))
            .expect("Could not deliver client message -- scheduler shut down?");
    }

    /// Send a message to the scheduler that the task should be suspended.
    pub fn suspend(&self, resume_condition: TaskSuspend, task: Box<Task>) {
        self.scheduler_sender
            .send((
                self.task_id,
                TaskControlMsg::TaskSuspend(resume_condition, task),
            ))
            .expect("Could not deliver client message -- scheduler shut down?");
    }

    /// Send a message to the scheduler that the task is requesting input from the client.
    /// Moves this task into the suspension queue until the client provides input.
    pub fn request_input(&self, task: Box<Task>) {
        self.scheduler_sender
            .send((self.task_id, TaskControlMsg::TaskRequestInput(task)))
            .expect("Could not deliver client message -- scheduler shut down?");
    }

    /// Ask the scheduler for a list of all background/suspended tasks known to it.
    pub fn task_list(&self) -> Vec<TaskDescription> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send((self.task_id, TaskControlMsg::RequestTasks(reply)))
            .expect("Could not deliver client message -- scheduler shut down?");
        receive
            .recv()
            .expect("Could not receive queued tasks -- scheduler shut down?")
    }

    /// Request that the scheduler abort another task.
    pub fn kill_task(&self, victim_task_id: TaskId, sender_permissions: Perms) -> Var {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send((
                self.task_id,
                TaskControlMsg::KillTask {
                    victim_task_id,
                    sender_permissions,
                    result_sender: reply,
                },
            ))
            .expect("Could not deliver client message -- scheduler shut down?");
        receive
            .recv()
            .expect("Could not receive result -- scheduler shut down?")
    }

    /// Request that the scheduler force-resume another, suspended, task.
    pub fn resume_task(
        &self,
        queued_task_id: TaskId,
        sender_permissions: Perms,
        return_value: Var,
    ) -> Var {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send((
                self.task_id,
                TaskControlMsg::ResumeTask {
                    queued_task_id,
                    sender_permissions,
                    return_value,
                    result_sender: reply,
                },
            ))
            .expect("Could not deliver client message -- scheduler shut down?");
        receive
            .recv()
            .expect("Could not receive result -- scheduler shut down?")
    }

    /// Request that the scheduler boot a player.
    pub fn boot_player(&self, player: Obj) {
        self.scheduler_sender
            .send((self.task_id, TaskControlMsg::BootPlayer { player }))
            .expect("Could not deliver client message -- scheduler shut down?");
    }

    /// Request that the scheduler write a textdump checkpoint.
    pub fn checkpoint(&self) {
        self.scheduler_sender
            .send((self.task_id, TaskControlMsg::Checkpoint(None)))
            .expect("Could not deliver client message -- scheduler shut down?");
    }

    /// Request that the scheduler write a textdump checkpoint with optional blocking.
    /// If `blocking` is true, waits for textdump generation to complete.
    /// Returns an error if the checkpoint fails or times out.
    pub fn checkpoint_with_blocking(&self, blocking: bool) -> Result<(), SchedulerError> {
        if blocking {
            let (reply, receive) = oneshot::channel();
            self.scheduler_sender
                .send((self.task_id, TaskControlMsg::Checkpoint(Some(reply))))
                .expect("Could not deliver client message -- scheduler shut down?");

            receive
                .recv_timeout(Duration::from_secs(600)) // 10 minutes for large textdumps
                .map_err(|_| SchedulerError::SchedulerNotResponding)?
        } else {
            self.scheduler_sender
                .send((self.task_id, TaskControlMsg::Checkpoint(None)))
                .expect("Could not deliver client message -- scheduler shut down?");
            Ok(())
        }
    }

    /// Ask the scheduler to dispatch a session notification to a player.
    pub fn notify(&self, player: Obj, event: Box<NarrativeEvent>) {
        self.scheduler_sender
            .send((self.task_id, TaskControlMsg::Notify { player, event }))
            .expect("Could not deliver client message -- scheduler shut down?");
    }

    pub fn listen(
        &self,
        handler_object: Obj,
        host_type: String,
        port: u16,
        print_messages: bool,
    ) -> Option<Error> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send((
                self.task_id,
                TaskControlMsg::Listen {
                    reply,
                    handler_object,
                    host_type,
                    port,
                    print_messages,
                },
            ))
            .expect("Unable to send listen message to scheduler");

        receive
            .recv_timeout(Duration::from_secs(5))
            .expect("Listen message timed out")
    }

    pub fn listeners(&self) -> Vec<(Obj, String, u16, bool)> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send((self.task_id, TaskControlMsg::GetListeners(reply)))
            .expect("Could not deliver client message -- scheduler shut down?");
        receive
            .recv()
            .expect("Could not receive listeners -- scheduler shut down?")
    }

    pub fn unlisten(&self, host_type: String, port: u16) -> Option<Error> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send((
                self.task_id,
                TaskControlMsg::Unlisten {
                    host_type,
                    port,
                    reply,
                },
            ))
            .expect("Could not deliver client message -- scheduler shut down?");
        receive
            .recv()
            .expect("Could not receive unlisten reply -- scheduler shut down?")
    }

    /// Request that the server refresh its set of information off $server_options
    pub fn refresh_server_options(&self) {
        self.scheduler_sender
            .send((self.task_id, TaskControlMsg::RefreshServerOptions))
            .expect("Could not deliver client message -- scheduler shut down?");
    }

    /// Request that the system shut down.
    pub fn shutdown(&self, msg: Option<String>) {
        self.scheduler_sender
            .send((self.task_id, TaskControlMsg::Shutdown(msg)))
            .expect("Could not deliver client message -- scheduler shut down?");
    }

    pub fn force_input(&self, who: Obj, line: String) -> Result<TaskId, Error> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send((
                self.task_id,
                TaskControlMsg::ForceInput { who, line, reply },
            ))
            .expect("Could not deliver client message -- scheduler shut down?");
        receive
            .recv()
            .expect("Could not receive task id -- scheduler shut down?")
    }

    pub fn active_tasks(&self) -> Result<ActiveTaskDescriptions, Error> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send((self.task_id, TaskControlMsg::ActiveTasks { reply }))
            .expect("Could not deliver client message -- scheduler shut down?");
        receive
            .recv()
            .expect("Could not receive active tasks -- scheduler shut down?")
    }

    pub fn switch_player(&self, new_player: Obj) -> Result<(), Error> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send((
                self.task_id,
                TaskControlMsg::SwitchPlayer { new_player, reply },
            ))
            .expect("Could not deliver client message -- scheduler shut down?");
        receive
            .recv()
            .expect("Could not receive switch player reply -- scheduler shut down?")
    }

    pub fn dump_object(&self, obj: Obj) -> Result<Vec<String>, Error> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send((self.task_id, TaskControlMsg::DumpObject { obj, reply }))
            .expect("Could not deliver client message -- scheduler shut down?");
        receive
            .recv()
            .expect("Could not receive dump object reply -- scheduler shut down?")
    }

    pub fn workers_info(&self) -> Vec<WorkerInfo> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send((self.task_id, TaskControlMsg::GetWorkersInfo { reply }))
            .expect("Could not deliver client message -- scheduler shut down?");
        receive
            .recv()
            .expect("Could not receive workers info -- scheduler shut down?")
    }

    /// Request a new database transaction for immediate task continuation.
    /// This is used for optimizing suspend(0) and commit-only suspensions
    /// to avoid the full suspend/resume cycle through the scheduler.
    pub fn begin_new_transaction(&self) -> Result<Box<dyn WorldState>, SchedulerError> {
        let (reply, receive) = oneshot::channel();
        self.scheduler_sender
            .send((self.task_id, TaskControlMsg::RequestNewTransaction(reply)))
            .expect("Could not deliver client message -- scheduler shut down?");

        // Short timeout since this should be very fast - just a database call
        receive
            .recv_timeout(Duration::from_millis(100))
            .map_err(|_| SchedulerError::SchedulerNotResponding)?
    }
}

pub type ActiveTaskDescriptions = Vec<(TaskId, Obj, TaskStart)>;

/// The ad-hoc messages that can be sent from tasks (or VM) up to the scheduler.
pub enum TaskControlMsg {
    /// Everything executed. The task is done.
    TaskSuccess(Var),
    /// The task hit an unresolvable transaction serialization conflict, and needs to be restarted
    /// in a new transaction.
    TaskConflictRetry(Box<Task>),
    /// A 'StartCommandVerb' type task failed to parse or match the command.
    TaskCommandError(CommandError),
    /// The verb to be executed was not found.
    TaskVerbNotFound(Var, Symbol),
    /// An exception was thrown while executing the verb.
    TaskException(Box<Exception>),
    /// The task is requesting that it be forked.
    TaskRequestFork(Box<Fork>, oneshot::Sender<TaskId>),
    /// The task is letting us know it was cancelled.
    TaskAbortCancelled,
    /// The task is letting us know that it has reached its abort limits.
    TaskAbortLimitsReached(AbortLimitReason, Var, Symbol, usize),
    /// Tell the scheduler that the task in a suspended state, with a time to resume (if any)
    TaskSuspend(TaskSuspend, Box<Task>),
    /// Tell the scheduler we're suspending until we get input from the client.
    TaskRequestInput(Box<Task>),
    /// Task is requesting a list of all other tasks known to the scheduler.
    RequestTasks(oneshot::Sender<Vec<TaskDescription>>),
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
        player: Obj,
    },
    /// Task is requesting that a textdump checkpoint happen, to the configured file.
    /// If reply channel is provided, waits for textdump generation to complete (blocking).
    /// If None, returns immediately after initiating checkpoint (non-blocking).
    Checkpoint(Option<oneshot::Sender<Result<(), SchedulerError>>>),
    Notify {
        player: Obj,
        event: Box<NarrativeEvent>,
    },
    GetListeners(oneshot::Sender<Vec<(Obj, String, u16, bool)>>),
    /// Ask hosts to listen for connections on `port` and send them to `handler_object`
    /// `print_messages` is a flag to enable or disable printing of connected etc strings
    /// `host_type` is a string identifying the type of host
    Listen {
        handler_object: Obj,
        host_type: String,
        port: u16,
        print_messages: bool,
        reply: oneshot::Sender<Option<Error>>,
    },
    /// Ask hosts of type `host_type` to stop listening on `port`
    Unlisten {
        host_type: String,
        port: u16,
        reply: oneshot::Sender<Option<Error>>,
    },
    /// Request that the server refresh its set of information off $server_options
    RefreshServerOptions,
    /// Task requesting shutdown
    Shutdown(Option<String>),
    /// Ask the scheduler to force input from the client.
    ForceInput {
        who: Obj,
        line: String,
        reply: oneshot::Sender<Result<TaskId, Error>>,
    },
    /// Ask the scheduler to return information of all active tasks (non-suspended)
    ActiveTasks {
        reply: oneshot::Sender<Result<ActiveTaskDescriptions, Error>>,
    },
    /// Request to switch the current session to a different player
    SwitchPlayer {
        new_player: Obj,
        reply: oneshot::Sender<Result<(), Error>>,
    },
    /// Request to dump an object to objdef format
    DumpObject {
        obj: Obj,
        reply: oneshot::Sender<Result<Vec<String>, Error>>,
    },
    /// Request information about all workers
    GetWorkersInfo {
        reply: oneshot::Sender<Vec<WorkerInfo>>,
    },
    /// Request a new database transaction for immediate task continuation
    RequestNewTransaction(oneshot::Sender<Result<Box<dyn WorldState>, SchedulerError>>),
}

impl TaskSchedulerClient {
    /// Get the control sender for testing purposes
    pub fn control_sender(&self) -> &Sender<(TaskId, TaskControlMsg)> {
        &self.scheduler_sender
    }
}

#[cfg(test)]
mod tests {
    /// Measure size of TaskControlMsg
    use super::*;
    #[test]
    fn test_task_control_msg_size() {
        use std::mem::size_of;
        assert!(
            size_of::<TaskControlMsg>() <= 64,
            "TaskControlMsg is too large: {} bytes",
            size_of::<TaskControlMsg>()
        );
    }
}
