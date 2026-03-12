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

use std::time::{Duration, SystemTime};

use flume::Sender;

use crate::{
    tasks::{TaskDescription, TaskStart, sched_counters, task::Task},
    vm::{Fork, TaskSuspend},
};
use moor_common::{
    model::{Perms, WorldState},
    tasks::{
        AbortLimitReason, CommandError, EventLogPurgeResult, EventLogStats, Exception,
        ListenerInfo, NarrativeEvent, SchedulerError, TaskId,
    },
    util::PerfTimerGuard,
};
use moor_var::{Error, Obj, Symbol, Var};

use crate::tasks::scheduler::Scheduler;

/// Information for invoking a timeout handler verb on #0.
/// This contains the traceback data that should be passed to $handle_task_timeout.
/// The abort reason is provided separately in TaskAbortLimitsReached.
/// Structured similarly to Exception for consistency.
#[derive(Debug, Clone)]
pub struct TimeoutHandlerInfo {
    /// Stack trace as Vars
    pub stack: Vec<Var>,
    /// Formatted backtrace strings
    pub backtrace: Vec<Var>,
}

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
    backend: BackendShared,
}

/// Shared backend (Clone via Arc for Live, Clone via Sender for Channel).
#[derive(Clone)]
enum BackendShared {
    Live(Scheduler),
    Channel(Sender<(TaskId, TaskControlMsg)>),
}

impl TaskSchedulerClient {
    /// Create a new TaskSchedulerClient backed by a Scheduler (production path).
    pub fn new(task_id: TaskId, scheduler: Scheduler) -> Self {
        Self {
            task_id,
            backend: BackendShared::Live(scheduler),
        }
    }

    /// Create a new TaskSchedulerClient backed by a flume channel (test path).
    pub fn new_channel(task_id: TaskId, sender: Sender<(TaskId, TaskControlMsg)>) -> Self {
        Self {
            task_id,
            backend: BackendShared::Channel(sender),
        }
    }

    /// Send a message to the scheduler that the task has completed successfully, with the given
    /// return value, mutations flag, and commit timestamp.
    pub fn success(&self, var: Var, mutations: bool, timestamp: u64) {
        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_task_success(self.task_id, var, mutations, timestamp);
            }
            BackendShared::Channel(sender) => {
                sender
                    .send((
                        self.task_id,
                        TaskControlMsg::TaskSuccess(var, mutations, timestamp),
                    ))
                    .expect("Could not deliver client message -- scheduler shut down?");
            }
        }
    }

    /// Send a message to the scheduler that the task has hit a transaction conflict and needs to be
    /// retried from the beginning.
    pub fn conflict_retry(&self, task: Box<Task>) {
        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_task_conflict_retry(self.task_id, task);
            }
            BackendShared::Channel(sender) => {
                sender
                    .send((self.task_id, TaskControlMsg::TaskConflictRetry(task)))
                    .expect("Could not deliver client message -- scheduler shut down?");
            }
        }
    }

    /// Send a message to the scheduler that the task has failed to parse or match the command.
    pub fn command_error(&self, error: CommandError) {
        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_task_command_error(self.task_id, error);
            }
            BackendShared::Channel(sender) => {
                sender
                    .send((self.task_id, TaskControlMsg::TaskCommandError(error)))
                    .expect("Could not deliver client message -- scheduler shut down?");
            }
        }
    }

    /// Send a message to the scheduler that the verb to be executed was not found.
    pub fn verb_not_found(&self, what: Var, verb: Symbol) {
        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_task_verb_not_found(self.task_id, what, verb);
            }
            BackendShared::Channel(sender) => {
                sender
                    .send((self.task_id, TaskControlMsg::TaskVerbNotFound(what, verb)))
                    .expect("Could not deliver client message -- scheduler shut down?");
            }
        }
    }

    /// Send a message to the scheduler that an exception was thrown while executing the verb.
    pub fn exception(&self, exception: Box<Exception>) {
        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_task_exception(self.task_id, exception);
            }
            BackendShared::Channel(sender) => {
                sender
                    .send((self.task_id, TaskControlMsg::TaskException(exception)))
                    .expect("Could not deliver client message -- scheduler shut down?");
            }
        }
    }

    /// Send a message to the scheduler that the task is requesting to fork itself.
    pub fn request_fork(&self, fork: Box<Fork>) -> TaskId {
        let _timer = PerfTimerGuard::new(&sched_counters().task_request_fork_latency);

        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_task_request_fork(self.task_id, fork)
            }
            BackendShared::Channel(sender) => {
                let (reply, receive) = oneshot::channel();
                sender
                    .send((self.task_id, TaskControlMsg::TaskRequestFork(fork, reply)))
                    .expect("Could not deliver client message -- scheduler shut down?");
                receive
                    .recv()
                    .expect("Could not receive task id -- scheduler shut down?")
            }
        }
    }

    /// Send a message to the scheduler that the task has been cancelled.
    pub fn abort_cancelled(&self) {
        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_task_abort_cancelled(self.task_id);
            }
            BackendShared::Channel(sender) => {
                sender
                    .send((self.task_id, TaskControlMsg::TaskAbortCancelled))
                    .expect("Could not deliver client message -- scheduler shut down?");
            }
        }
    }

    /// Send a message to the scheduler that the task has reached its abort limits.
    pub fn abort_limits_reached(
        &self,
        reason: AbortLimitReason,
        this: Var,
        verb_name: Symbol,
        line_number: usize,
        handler_info: TimeoutHandlerInfo,
    ) {
        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_task_abort_limits_reached(
                    self.task_id,
                    reason,
                    this,
                    verb_name,
                    line_number,
                    Box::new(handler_info),
                );
            }
            BackendShared::Channel(sender) => {
                sender
                    .send((
                        self.task_id,
                        TaskControlMsg::TaskAbortLimitsReached(
                            reason,
                            this,
                            verb_name,
                            line_number,
                            Box::new(handler_info),
                        ),
                    ))
                    .expect("Could not deliver client message -- scheduler shut down?");
            }
        }
    }

    /// Send a message to the scheduler that the task should be suspended.
    pub fn suspend(&self, resume_condition: TaskSuspend, task: Box<Task>) {
        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_task_suspend(self.task_id, resume_condition, task);
            }
            BackendShared::Channel(sender) => {
                sender
                    .send((
                        self.task_id,
                        TaskControlMsg::TaskSuspend(resume_condition, task),
                    ))
                    .expect("Could not deliver client message -- scheduler shut down?");
            }
        }
    }

    /// Send a message to the scheduler that the task is requesting input from the client.
    /// Moves this task into the suspension queue until the client provides input.
    pub fn request_input(&self, task: Box<Task>, metadata: Option<Vec<(Symbol, Var)>>) {
        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_task_request_input(self.task_id, task, metadata);
            }
            BackendShared::Channel(sender) => {
                sender
                    .send((
                        self.task_id,
                        TaskControlMsg::TaskRequestInput(task, metadata),
                    ))
                    .expect("Could not deliver client message -- scheduler shut down?");
            }
        }
    }

    /// Ask the scheduler for a list of all background/suspended tasks known to it.
    pub fn task_list(&self) -> Vec<TaskDescription> {
        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_request_tasks(self.task_id)
            }
            BackendShared::Channel(sender) => {
                let (reply, receive) = oneshot::channel();
                sender
                    .send((self.task_id, TaskControlMsg::RequestTasks(reply)))
                    .expect("Could not deliver client message -- scheduler shut down?");
                receive
                    .recv()
                    .expect("Could not receive queued tasks -- scheduler shut down?")
            }
        }
    }

    /// Check if a task exists (suspended or active) atomically.
    /// Returns Some(owner) if the task exists, None otherwise.
    pub fn task_exists(&self, task_id: TaskId) -> Option<Obj> {
        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_task_exists(task_id)
            }
            BackendShared::Channel(sender) => {
                let (reply, receive) = oneshot::channel();
                sender
                    .send((
                        self.task_id,
                        TaskControlMsg::TaskExists {
                            task_id,
                            result_sender: reply,
                        },
                    ))
                    .expect("Could not deliver client message -- scheduler shut down?");
                receive
                    .recv()
                    .expect("Could not receive task exists result -- scheduler shut down?")
            }
        }
    }

    /// Request that the scheduler abort another task.
    pub fn kill_task(&self, victim_task_id: TaskId, sender_permissions: Perms) -> Var {
        let _timer = PerfTimerGuard::new(&sched_counters().task_kill_task_latency);

        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_kill_task(self.task_id, victim_task_id, sender_permissions)
            }
            BackendShared::Channel(sender) => {
                let (reply, receive) = oneshot::channel();
                sender
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
        }
    }

    /// Request that the scheduler force-resume another, suspended, task.
    pub fn resume_task(
        &self,
        queued_task_id: TaskId,
        sender_permissions: Perms,
        return_value: Var,
    ) -> Var {
        let _timer = PerfTimerGuard::new(&sched_counters().task_resume_task_latency);

        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_resume_task(
                    self.task_id,
                    queued_task_id,
                    sender_permissions,
                    return_value,
                )
            }
            BackendShared::Channel(sender) => {
                let (reply, receive) = oneshot::channel();
                sender
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
        }
    }

    /// Request that the scheduler boot a player.
    pub fn boot_player(&self, player: Obj) {
        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_boot_player(self.task_id, player);
            }
            BackendShared::Channel(sender) => {
                sender
                    .send((self.task_id, TaskControlMsg::BootPlayer { player }))
                    .expect("Could not deliver client message -- scheduler shut down?");
            }
        }
    }

    /// Request that the scheduler write a textdump checkpoint.
    pub fn checkpoint(&self) {
        match &self.backend {
            BackendShared::Live(scheduler) => {
                let _ = scheduler.handle_checkpoint_from_task(self.task_id, false);
            }
            BackendShared::Channel(sender) => {
                sender
                    .send((self.task_id, TaskControlMsg::Checkpoint(None)))
                    .expect("Could not deliver client message -- scheduler shut down?");
            }
        }
    }

    /// Request that the scheduler write a textdump checkpoint with optional blocking.
    /// If `blocking` is true, waits for textdump generation to complete.
    /// Returns an error if the checkpoint fails or times out.
    pub fn checkpoint_with_blocking(&self, blocking: bool) -> Result<(), SchedulerError> {
        let _timer = PerfTimerGuard::new(&sched_counters().task_checkpoint_latency);

        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_checkpoint_from_task(self.task_id, blocking)
            }
            BackendShared::Channel(sender) => {
                if blocking {
                    let (reply, receive) = oneshot::channel();
                    sender
                        .send((self.task_id, TaskControlMsg::Checkpoint(Some(reply))))
                        .expect("Could not deliver client message -- scheduler shut down?");

                    receive
                        .recv_timeout(Duration::from_secs(600))
                        .map_err(|_| SchedulerError::SchedulerNotResponding)?
                } else {
                    sender
                        .send((self.task_id, TaskControlMsg::Checkpoint(None)))
                        .expect("Could not deliver client message -- scheduler shut down?");
                    Ok(())
                }
            }
        }
    }

    /// Ask the scheduler to dispatch a session notification to a player.
    pub fn notify(&self, player: Obj, event: Box<NarrativeEvent>) {
        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_notify(self.task_id, player, event);
            }
            BackendShared::Channel(sender) => {
                sender
                    .send((self.task_id, TaskControlMsg::Notify { player, event }))
                    .expect("Could not deliver client message -- scheduler shut down?");
            }
        }
    }

    /// Ask the scheduler to log an event to a player's event log without broadcasting.
    pub fn log_event(&self, player: Obj, event: Box<NarrativeEvent>) {
        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_log_event(self.task_id, player, event);
            }
            BackendShared::Channel(sender) => {
                sender
                    .send((self.task_id, TaskControlMsg::LogEvent { player, event }))
                    .expect("Could not deliver client message -- scheduler shut down?");
            }
        }
    }

    pub fn listen(
        &self,
        handler_object: Obj,
        host_type: String,
        port: u16,
        options: Vec<(Symbol, Var)>,
    ) -> Option<Error> {
        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_listen(
                    self.task_id,
                    handler_object,
                    host_type,
                    port,
                    Box::new(options),
                )
            }
            BackendShared::Channel(sender) => {
                let (reply, receive) = oneshot::channel();
                sender
                    .send((
                        self.task_id,
                        TaskControlMsg::Listen {
                            reply,
                            handler_object,
                            host_type,
                            port,
                            options: Box::new(options),
                        },
                    ))
                    .expect("Unable to send listen message to scheduler");

                receive
                    .recv_timeout(Duration::from_secs(5))
                    .expect("Listen message timed out")
            }
        }
    }

    pub fn listeners(&self) -> Vec<ListenerInfo> {
        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_get_listeners()
            }
            BackendShared::Channel(sender) => {
                let (reply, receive) = oneshot::channel();
                sender
                    .send((self.task_id, TaskControlMsg::GetListeners(reply)))
                    .expect("Could not deliver client message -- scheduler shut down?");
                receive
                    .recv()
                    .expect("Could not receive listeners -- scheduler shut down?")
            }
        }
    }

    pub fn unlisten(&self, host_type: String, port: u16) -> Option<Error> {
        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_unlisten(self.task_id, host_type, port)
            }
            BackendShared::Channel(sender) => {
                let (reply, receive) = oneshot::channel();
                sender
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
        }
    }

    /// Request that the server refresh its set of information off $server_options
    pub fn refresh_server_options(&self) {
        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_refresh_server_options();
            }
            BackendShared::Channel(sender) => {
                sender
                    .send((self.task_id, TaskControlMsg::RefreshServerOptions))
                    .expect("Could not deliver client message -- scheduler shut down?");
            }
        }
    }

    /// Request that the system shut down.
    pub fn shutdown(&self, msg: Option<String>) {
        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_shutdown(msg);
            }
            BackendShared::Channel(sender) => {
                sender
                    .send((self.task_id, TaskControlMsg::Shutdown(msg)))
                    .expect("Could not deliver client message -- scheduler shut down?");
            }
        }
    }

    /// Request that the enrollment token be rotated, returning the new token.
    pub fn rotate_enrollment_token(&self) -> Result<String, Error> {
        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_rotate_enrollment_token()
            }
            BackendShared::Channel(sender) => {
                let (reply, receive) = oneshot::channel();
                sender
                    .send((
                        self.task_id,
                        TaskControlMsg::RotateEnrollmentToken { reply },
                    ))
                    .expect("Could not deliver client message -- scheduler shut down?");
                receive
                    .recv()
                    .expect("Could not receive rotate enrollment token reply -- scheduler shut down?")
            }
        }
    }

    pub fn player_event_log_stats(
        &self,
        player: Obj,
        since: Option<SystemTime>,
        until: Option<SystemTime>,
    ) -> Result<EventLogStats, Error> {
        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_player_event_log_stats(player, since, until)
            }
            BackendShared::Channel(sender) => {
                let (reply, receive) = oneshot::channel();
                sender
                    .send((
                        self.task_id,
                        TaskControlMsg::PlayerEventLogStats {
                            player,
                            since,
                            until,
                            reply,
                        },
                    ))
                    .expect("Could not deliver client message -- scheduler shut down?");
                receive
                    .recv()
                    .expect("Could not receive event log stats reply -- scheduler shut down?")
            }
        }
    }

    pub fn purge_player_event_log(
        &self,
        player: Obj,
        before: Option<SystemTime>,
        drop_pubkey: bool,
    ) -> Result<EventLogPurgeResult, Error> {
        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_purge_player_event_log(player, before, drop_pubkey)
            }
            BackendShared::Channel(sender) => {
                let (reply, receive) = oneshot::channel();
                sender
                    .send((
                        self.task_id,
                        TaskControlMsg::PurgePlayerEventLog {
                            player,
                            before,
                            drop_pubkey,
                            reply,
                        },
                    ))
                    .expect("Could not deliver client message -- scheduler shut down?");
                receive
                    .recv()
                    .expect("Could not receive event log purge reply -- scheduler shut down?")
            }
        }
    }

    pub fn force_input(&self, who: Obj, line: String) -> Result<TaskId, Error> {
        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_force_input(self.task_id, who, line)
            }
            BackendShared::Channel(sender) => {
                let (reply, receive) = oneshot::channel();
                sender
                    .send((
                        self.task_id,
                        TaskControlMsg::ForceInput { who, line, reply },
                    ))
                    .expect("Could not deliver client message -- scheduler shut down?");
                receive
                    .recv()
                    .expect("Could not receive task id -- scheduler shut down?")
            }
        }
    }

    pub fn active_tasks(&self) -> Result<ActiveTaskDescriptions, Error> {
        let _timer = PerfTimerGuard::new(&sched_counters().task_active_tasks_latency);

        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_active_tasks(self.task_id)
            }
            BackendShared::Channel(sender) => {
                let (reply, receive) = oneshot::channel();
                sender
                    .send((self.task_id, TaskControlMsg::ActiveTasks { reply }))
                    .expect("Could not deliver client message -- scheduler shut down?");
                receive
                    .recv()
                    .expect("Could not receive active tasks -- scheduler shut down?")
            }
        }
    }

    pub fn switch_player(&self, new_player: Obj) -> Result<(), Error> {
        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_switch_player(self.task_id, new_player)
            }
            BackendShared::Channel(sender) => {
                let (reply, receive) = oneshot::channel();
                sender
                    .send((
                        self.task_id,
                        TaskControlMsg::SwitchPlayer { new_player, reply },
                    ))
                    .expect("Could not deliver client message -- scheduler shut down?");
                receive
                    .recv()
                    .expect("Could not receive switch player reply -- scheduler shut down?")
            }
        }
    }

    pub fn dump_object(&self, obj: Obj, use_constants: bool) -> Result<Vec<String>, Error> {
        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_dump_object_from_task(obj, use_constants)
            }
            BackendShared::Channel(sender) => {
                let (reply, receive) = oneshot::channel();
                sender
                    .send((
                        self.task_id,
                        TaskControlMsg::DumpObject {
                            obj,
                            use_constants,
                            reply,
                        },
                    ))
                    .expect("Could not deliver client message -- scheduler shut down?");
                receive
                    .recv()
                    .expect("Could not receive dump object reply -- scheduler shut down?")
            }
        }
    }

    pub fn workers_info(&self) -> Vec<WorkerInfo> {
        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_get_workers_info_from_task()
            }
            BackendShared::Channel(sender) => {
                let (reply, receive) = oneshot::channel();
                sender
                    .send((self.task_id, TaskControlMsg::GetWorkersInfo { reply }))
                    .expect("Could not deliver client message -- scheduler shut down?");
                receive
                    .recv()
                    .expect("Could not receive workers info -- scheduler shut down?")
            }
        }
    }

    /// Request a new database transaction for immediate task continuation.
    /// This is used for optimizing suspend(0) and commit-only suspensions
    /// to avoid the full suspend/resume cycle through the scheduler.
    pub fn begin_new_transaction(&self) -> Result<Box<dyn WorldState>, SchedulerError> {
        let _timer = PerfTimerGuard::new(&sched_counters().task_begin_transaction_latency);

        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_request_new_transaction(self.task_id)
            }
            BackendShared::Channel(sender) => {
                let (reply, receive) = oneshot::channel();
                sender
                    .send((self.task_id, TaskControlMsg::RequestNewTransaction(reply)))
                    .expect("Could not deliver client message -- scheduler shut down?");

                receive
                    .recv_timeout(Duration::from_millis(100))
                    .map_err(|_| SchedulerError::SchedulerNotResponding)?
            }
        }
    }

    /// Buffer a message for delivery to another task at commit time.
    /// Validates target existence and permissions eagerly.
    pub fn task_send(&self, target_task_id: TaskId, value: Var, sender_permissions: Perms) -> Var {
        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_task_send(self.task_id, target_task_id, value, sender_permissions)
            }
            BackendShared::Channel(sender) => {
                let (reply, receive) = oneshot::channel();
                sender
                    .send((
                        self.task_id,
                        TaskControlMsg::TaskSend {
                            target_task_id,
                            value,
                            sender_permissions,
                            result_sender: reply,
                        },
                    ))
                    .expect("Could not deliver client message -- scheduler shut down?");
                receive
                    .recv()
                    .expect("Could not receive task send result -- scheduler shut down?")
            }
        }
    }

    /// Drain all messages from this task's message queue.
    pub fn task_recv(&self) -> Vec<Var> {
        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_task_recv(self.task_id)
            }
            BackendShared::Channel(sender) => {
                let (reply, receive) = oneshot::channel();
                sender
                    .send((
                        self.task_id,
                        TaskControlMsg::TaskRecv {
                            result_sender: reply,
                        },
                    ))
                    .expect("Could not deliver client message -- scheduler shut down?");
                receive
                    .recv()
                    .expect("Could not receive task recv result -- scheduler shut down?")
            }
        }
    }

    pub fn force_gc(&self) {
        match &self.backend {
            BackendShared::Live(scheduler) => {
                scheduler.handle_force_gc();
            }
            BackendShared::Channel(sender) => {
                sender
                    .send((self.task_id, TaskControlMsg::ForceGC))
                    .expect("Could not deliver client message -- scheduler shut down?");
            }
        }
    }
}

pub type ActiveTaskDescriptions = Vec<(TaskId, Obj, TaskStart)>;

/// The ad-hoc messages that can be sent from tasks (or VM) up to the scheduler.
/// Retained for test compatibility (Channel mode) -- production code uses direct method calls.
pub enum TaskControlMsg {
    /// Everything executed. The task is done.
    /// Contains: (return_value, mutations_occurred, commit_timestamp)
    TaskSuccess(Var, bool, u64),
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
    /// The task thread panicked with the given message.
    TaskAbortPanicked(String, Box<std::backtrace::Backtrace>),
    /// The task is letting us know that it has reached its abort limits.
    /// Handler info contains traceback data for $handle_task_timeout.
    TaskAbortLimitsReached(
        AbortLimitReason,
        Var,
        Symbol,
        usize,
        Box<TimeoutHandlerInfo>,
    ),
    /// Tell the scheduler that the task in a suspended state, with a time to resume (if any)
    TaskSuspend(TaskSuspend, Box<Task>),
    /// Tell the scheduler we're suspending until we get input from the client.
    /// Optional metadata provides UI hints for rich input prompts.
    TaskRequestInput(Box<Task>, Option<Vec<(Symbol, Var)>>),
    /// Task is requesting a list of all other tasks known to the scheduler.
    RequestTasks(oneshot::Sender<Vec<TaskDescription>>),
    /// Task is requesting to check if a task exists (suspended or active).
    /// Returns Some(owner) if task exists, None otherwise.
    TaskExists {
        task_id: TaskId,
        result_sender: oneshot::Sender<Option<Obj>>,
    },
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
    /// Log an event to the player's event log without broadcasting to connections.
    LogEvent {
        player: Obj,
        event: Box<NarrativeEvent>,
    },
    GetListeners(oneshot::Sender<Vec<ListenerInfo>>),
    /// Ask hosts to listen for connections on `port` and send them to `handler_object`
    Listen {
        handler_object: Obj,
        host_type: String,
        port: u16,
        options: Box<Vec<(Symbol, Var)>>,
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
        use_constants: bool,
        reply: oneshot::Sender<Result<Vec<String>, Error>>,
    },
    /// Request information about all workers
    GetWorkersInfo {
        reply: oneshot::Sender<Vec<WorkerInfo>>,
    },
    /// Request a new database transaction for immediate task continuation
    RequestNewTransaction(oneshot::Sender<Result<Box<dyn WorldState>, SchedulerError>>),
    /// Request that the scheduler force a garbage collection cycle
    ForceGC,
    /// Request that the scheduler rotate the enrollment token
    RotateEnrollmentToken {
        reply: oneshot::Sender<Result<String, Error>>,
    },
    /// Request event log statistics for a player
    PlayerEventLogStats {
        player: Obj,
        since: Option<SystemTime>,
        until: Option<SystemTime>,
        reply: oneshot::Sender<Result<EventLogStats, Error>>,
    },
    /// Request that part or all of a player's event log be purged
    PurgePlayerEventLog {
        player: Obj,
        before: Option<SystemTime>,
        drop_pubkey: bool,
        reply: oneshot::Sender<Result<EventLogPurgeResult, Error>>,
    },
    /// Buffer a task message for delivery at commit time.
    TaskSend {
        target_task_id: TaskId,
        value: Var,
        sender_permissions: Perms,
        result_sender: oneshot::Sender<Var>,
    },
    /// Drain all messages from the calling task's message queue.
    TaskRecv {
        result_sender: oneshot::Sender<Vec<Var>>,
    },
}

/// Get the control sender for testing purposes (only available in Channel mode).
/// Panics if called on a Live backend.
impl TaskSchedulerClient {
    pub fn control_sender(&self) -> &Sender<(TaskId, TaskControlMsg)> {
        match &self.backend {
            BackendShared::Channel(sender) => sender,
            BackendShared::Live(_) => panic!("control_sender() is only available in Channel mode (tests)"),
        }
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
