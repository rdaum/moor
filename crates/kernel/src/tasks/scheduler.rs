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

use std::fs::File;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use bincode::{Decode, Encode};
use crossbeam_channel::Sender;
use dashmap::DashMap;

use thiserror::Error;
use tracing::{error, info, instrument, trace, warn};
use uuid::Uuid;

use crossbeam_channel::internal::SelectHandle;
use crossbeam_channel::Receiver;
use std::sync::Mutex;
use std::thread::yield_now;

use moor_compiler::compile;
use moor_compiler::CompileError;
use moor_db::Database;
use moor_values::model::CommandError;
use moor_values::model::Perms;
use moor_values::model::WorldStateSource;
use moor_values::var::Error::{E_INVARG, E_PERM};
use moor_values::var::{v_err, v_int, v_none, v_string, Var};
use moor_values::var::{Objid, Variant};
use moor_values::SYSTEM_OBJECT;
use SchedulerError::{
    CommandExecutionError, CouldNotStartTask, EvalCompilationError, InputRequestNotFound,
    TaskAbortedCancelled, TaskAbortedError, TaskAbortedException, TaskAbortedLimit,
};

use crate::config::Config;
use crate::tasks::scheduler::SchedulerError::TaskNotFound;
use crate::tasks::sessions::Session;
use crate::tasks::task::Task;
use crate::tasks::task_messages::{SchedulerControlMsg, TaskControlMsg, TaskStart};
use crate::tasks::{TaskDescription, TaskHandle, TaskId};
use crate::textdump::{make_textdump, TextdumpWriter};
use crate::vm::Fork;
use crate::vm::UncaughtException;

const SCHEDULER_TICK_TIME: Duration = Duration::from_millis(5);

/// Responsible for the dispatching, control, and accounting of tasks in the system.
/// There should be only one scheduler per server.
pub struct Scheduler {
    control_sender: Sender<(TaskId, SchedulerControlMsg)>,
    control_receiver: Receiver<(TaskId, SchedulerControlMsg)>,
    config: Arc<Config>,

    running: Arc<AtomicBool>,
    database: Arc<dyn Database + Send + Sync>,
    next_task_id: AtomicUsize,
    tasks: DashMap<TaskId, TaskControl>,
    input_requests: DashMap<Uuid, TaskId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Decode, Encode)]
pub enum AbortLimitReason {
    Ticks(usize),
    Time(Duration),
}

/// Results returned to waiters on tasks during subscription.
#[derive(Clone, Debug)]
pub enum TaskWaiterResult {
    Success(Var),
    Error(SchedulerError),
}

#[derive(Debug, Error, Clone, Decode, Encode, PartialEq)]
pub enum SchedulerError {
    #[error("Task not found: {0:?}")]
    TaskNotFound(TaskId),
    #[error("Input request not found: {0:?}")]
    // Using u128 here because Uuid is not bincode-able, but this is just a v4 uuid.
    InputRequestNotFound(u128),
    #[error("Could not start task (internal error)")]
    CouldNotStartTask,
    #[error("Eval compilation error")]
    EvalCompilationError(#[source] CompileError),
    #[error("Could not start command")]
    CommandExecutionError(#[source] CommandError),
    #[error("Task aborted due to limit: {0:?}")]
    TaskAbortedLimit(AbortLimitReason),
    #[error("Task aborted due to error.")]
    TaskAbortedError,
    #[error("Task aborted due to exception")]
    TaskAbortedException(#[source] UncaughtException),
    #[error("Task aborted due to cancellation.")]
    TaskAbortedCancelled,
}

struct KillRequest {
    requesting_task_id: TaskId,
    victim_task_id: TaskId,
    sender_permissions: Perms,
    result_sender: oneshot::Sender<Var>,
}

struct ResumeRequest {
    requesting_task_id: TaskId,
    queued_task_id: TaskId,
    sender_permissions: Perms,
    return_value: Var,
    result_sender: oneshot::Sender<Var>,
}

struct ForkRequest {
    fork_request: Fork,
    reply: oneshot::Sender<TaskId>,
    session: Arc<dyn Session>,
}

/// Scheduler-side per-task record. Lives in the scheduler thread and owned by the scheduler and
/// not shared elsewhere.
struct TaskControl {
    task_id: TaskId,
    player: Objid,
    /// Outbound mailbox for messages from the scheduler to the task.
    task_control_sender: Sender<TaskControlMsg>,
    state_source: Arc<dyn WorldStateSource>,
    session: Arc<dyn Session>,
    suspended: bool,
    waiting_input: Option<Uuid>,
    resume_time: Option<SystemTime>,
    // TODO: find a way for this not to be in a mutex.
    result_sender: Mutex<Option<oneshot::Sender<TaskWaiterResult>>>,
}

/// The set of actions that the scheduler needs to take in response to a task control message.
enum TaskHandleResult {
    Notify(TaskId, TaskWaiterResult),
    Fork(ForkRequest),
    Describe(TaskId, oneshot::Sender<Vec<TaskDescription>>),
    Kill(KillRequest),
    Resume(ResumeRequest),
    Disconnect(TaskId, Objid),
    Retry(TaskId),
}

/// Public facing interface for the scheduler.
impl Scheduler {
    pub fn new(database: Arc<dyn Database + Send + Sync>, config: Config) -> Self {
        let config = Arc::new(config);
        let (control_sender, control_receiver) = crossbeam_channel::unbounded();
        Self {
            running: Arc::new(AtomicBool::new(false)),
            database,
            next_task_id: Default::default(),
            tasks: DashMap::new(),
            input_requests: Default::default(),
            config,
            control_sender,
            control_receiver,
        }
    }

    /// Execute the scheduler loop, run from the server process.
    pub fn run(self: Arc<Self>) {
        self.running.store(true, Ordering::SeqCst);
        self.clone().do_process();
        info!("Scheduler done.");
    }

    /// Submit a command to the scheduler for execution.
    #[instrument(skip(self, session))]
    pub fn submit_command_task(
        &self,
        player: Objid,
        command: &str,
        session: Arc<dyn Session>,
    ) -> Result<TaskHandle, SchedulerError> {
        trace!(?player, ?command, "Command submitting");

        let task_start = TaskStart::StartCommandVerb {
            player,
            command: command.to_string(),
        };

        self.new_task(
            task_start,
            player,
            session,
            None,
            self.control_sender.clone(),
            player,
            false,
        )
    }

    /// Receive input that the (suspended) task previously requested, using the given
    /// `input_request_id`.
    /// The request is identified by the `input_request_id`, and given the input and resumed under
    /// a new transaction.
    pub fn submit_requested_input(
        &self,
        player: Objid,
        input_request_id: Uuid,
        input: String,
    ) -> Result<(), SchedulerError> {
        // Validate that the given input request is valid, and if so, resume the task, sending it
        // the given input, clearing the input request out.

        let Some(task_ref) = self.input_requests.get(&input_request_id) else {
            return Err(InputRequestNotFound(input_request_id.as_u128()));
        };

        let (_uuid, task_id) = (task_ref.key(), task_ref.value());
        let Some(mut task) = self.tasks.get_mut(task_id) else {
            warn!(?task_id, ?input_request_id, "Input received for dead task");
            return Err(TaskNotFound(*task_id));
        };

        // If the player doesn't match, we'll pretend we didn't even see it.
        if task.player != player {
            warn!(
                ?task_id,
                ?input_request_id,
                ?player,
                "Task input request received for wrong player"
            );
            return Err(TaskNotFound(*task_id));
        }

        // Now we can resume the task with the given input
        let tcs = task.task_control_sender.clone();
        tcs.send(TaskControlMsg::ResumeReceiveInput(
            task.state_source.clone(),
            input,
        ))
        .map_err(|_| CouldNotStartTask)?;
        task.waiting_input = None;
        self.input_requests.remove(&input_request_id);

        Ok(())
    }

    /// Submit a verb task to the scheduler for execution.
    /// (This path is really only used for the invocations from the serving processes like login,
    /// user_connected, or the do_command invocation which precedes an internal parser attempt.)
    #[instrument(skip(self, session))]
    // Yes yes I know it's a lot of arguments, but wrapper object here is redundant.
    #[allow(clippy::too_many_arguments)]
    pub fn submit_verb_task(
        &self,
        player: Objid,
        vloc: Objid,
        verb: String,
        args: Vec<Var>,
        argstr: String,
        perms: Objid,
        session: Arc<dyn Session>,
    ) -> Result<TaskHandle, SchedulerError> {
        let task_start = TaskStart::StartVerb {
            player,
            vloc,
            verb,
            args,
            argstr,
        };

        self.new_task(
            task_start,
            player,
            session,
            None,
            self.control_sender.clone(),
            perms,
            false,
        )
    }

    #[instrument(skip(self, session))]
    pub fn submit_out_of_band_task(
        &self,
        player: Objid,
        command: Vec<String>,
        argstr: String,
        session: Arc<dyn Session>,
    ) -> Result<TaskHandle, SchedulerError> {
        let args = command.into_iter().map(v_string).collect::<Vec<Var>>();
        let task_start = TaskStart::StartVerb {
            player,
            vloc: SYSTEM_OBJECT,
            verb: "do_out_of_band_command".to_string(),
            args,
            argstr,
        };

        self.new_task(
            task_start,
            player,
            session,
            None,
            self.control_sender.clone(),
            player,
            false,
        )
    }

    /// Submit an eval task to the scheduler for execution.
    #[instrument(skip(self, sessions))]
    pub fn submit_eval_task(
        &self,
        player: Objid,
        perms: Objid,
        code: String,
        sessions: Arc<dyn Session>,
    ) -> Result<TaskHandle, SchedulerError> {
        // Compile the text into a verb.
        let binary = match compile(code.as_str()) {
            Ok(b) => b,
            Err(e) => return Err(EvalCompilationError(e)),
        };

        let task_start = TaskStart::StartEval {
            player,
            program: binary,
        };

        self.new_task(
            task_start,
            player,
            sessions,
            None,
            self.control_sender.clone(),
            perms,
            false,
        )
    }

    pub fn submit_shutdown(
        &self,
        task: TaskId,
        reason: Option<String>,
    ) -> Result<(), SchedulerError> {
        // If we can't deliver a shutdown message, that's really a cause for panic!
        self.control_sender
            .send((task, SchedulerControlMsg::Shutdown(reason)))
            .expect("could not send clean shutdown message");
        Ok(())
    }

    pub fn abort_player_tasks(&self, player: Objid) -> Result<(), SchedulerError> {
        let mut to_abort = Vec::new();
        for t in self.tasks.iter() {
            let (task_id, task_ref) = (t.key(), t.value());
            if task_ref.player == player {
                to_abort.push(*task_id);
            }
        }
        for task_id in to_abort {
            let task = self.tasks.get_mut(&task_id).expect("Corrupt task list");
            let tcs = task.task_control_sender.clone();
            if let Err(e) = tcs.send(TaskControlMsg::Abort) {
                warn!(task_id, error = ?e, "Could not send abort for task. Dead?");
                continue;
            }
        }

        Ok(())
    }

    /// Request information on all tasks known to the scheduler.
    pub fn tasks(&self) -> Result<Vec<TaskDescription>, SchedulerError> {
        let mut tasks = Vec::new();
        for t in self.tasks.iter() {
            let (task_id, task) = (t.key(), t.value());
            trace!(task_id, "Requesting task description");
            let (t_send, t_reply) = oneshot::channel();
            let tcs = task.task_control_sender.clone();
            if let Err(e) = tcs.send(TaskControlMsg::Describe(t_send)) {
                warn!(task_id, error = ?e, "Could not request task description for task. Dead?");
                continue;
            }
            let Ok(task_desc) = t_reply.recv() else {
                warn!(
                    task_id,
                    "Could not request task description for task. Dead?"
                );
                continue;
            };
            trace!(task_id, "Got task description");
            tasks.push(task_desc);
        }
        Ok(tasks)
    }

    /// Stop the scheduler run loop.
    pub fn stop(&self) -> Result<(), SchedulerError> {
        warn!("Issuing clean shutdown...");
        // Send shut down to all the tasks.
        for t in self.tasks.iter() {
            let task = t.value();
            let tcs = task.task_control_sender.clone();
            if let Err(e) = tcs.send(TaskControlMsg::Abort) {
                warn!(task_id = task.task_id, error = ?e, "Could not send abort for task. Already dead?");
                continue;
            }
        }
        warn!("Waiting for tasks to finish...");

        // Then spin until they're all done.
        while !self.tasks.is_empty() {
            yield_now();
        }

        warn!("All tasks finished.  Stopping scheduler.");
        self.running.store(false, Ordering::SeqCst);

        Ok(())
    }

    pub fn abort_task(&self, id: TaskId) -> Result<(), SchedulerError> {
        let task = self.tasks.get_mut(&id).ok_or(TaskNotFound(id))?;
        let tcs = task.task_control_sender.clone();
        if let Err(e) = tcs.send(TaskControlMsg::Abort) {
            error!(error = ?e, "Could not send abort message to task on its channel.  Already dead?");
        }
        Ok(())
    }
}

impl Scheduler {
    fn do_process(&self) {
        // TODO: Improve scheduler "tick" and "prune" logic.  It's a bit of a mess.
        //  we might be able to use a vector of delay-futures for this instead, and just poll
        //  those using some futures_util magic.
        info!("Starting scheduler loop");
        loop {
            let is_running = self.running.load(Ordering::SeqCst);
            if !is_running {
                warn!("Scheduler stopping");
                break;
            }

            // Look for tasks that need to be woken (have hit their wakeup-time), and wake them.
            // Or tasks that need pruning.
            let mut to_wake = Vec::new();
            let mut to_prune = Vec::new();
            for t in self.tasks.iter() {
                let (task_id, task) = (t.key(), t.value());
                if !task.task_control_sender.is_ready() {
                    warn!(
                        task_id,
                        "Task is present but its channel is invalid.  Pruning."
                    );
                    to_prune.push(*task_id);
                    continue;
                }

                if !task.suspended {
                    continue;
                }
                let Some(delay) = task.resume_time else {
                    continue;
                };
                if delay <= SystemTime::now() {
                    to_wake.push(*task_id);
                }
            }
            if !to_wake.is_empty() {
                self.process_wake_ups(&to_wake);
            }
            if !to_prune.is_empty() {
                self.process_task_removals(&to_prune);
            }
            if let Ok(msg) = self.control_receiver.recv_timeout(SCHEDULER_TICK_TIME) {
                let (task_id, msg) = msg;
                if let Some(action) = self.handle_task_control_msg(task_id, msg) {
                    self.process_task_action(action);
                }
            }
        }
        info!("Done.");
    }

    /// Handle scheduler control messages inbound from tasks.
    /// Note: this function should never be allowed to panic, as it is called from the scheduler main loop.
    fn handle_task_control_msg(
        &self,
        task_id: TaskId,
        msg: SchedulerControlMsg,
    ) -> Option<TaskHandleResult> {
        match msg {
            SchedulerControlMsg::TaskSuccess(value) => {
                // Commit the session.
                let Some(task) = self.tasks.get_mut(&task_id) else {
                    warn!(task_id, "Task not found for success");
                    return None;
                };
                let Ok(()) = task.session.commit() else {
                    warn!("Could not commit session; aborting task");
                    return Some(TaskHandleResult::Notify(
                        task_id,
                        TaskWaiterResult::Error(TaskAbortedError),
                    ));
                };
                trace!(?task_id, result = ?value, "Task succeeded");
                Some(TaskHandleResult::Notify(
                    task_id,
                    TaskWaiterResult::Success(value),
                ))
            }
            SchedulerControlMsg::TaskConflictRetry => {
                trace!(?task_id, "Task retrying due to conflict");

                // Ask the task to restart itself, using its stashed original start info, but with
                // a brand new transaction.
                Some(TaskHandleResult::Retry(task_id))
            }
            SchedulerControlMsg::TaskVerbNotFound(this, verb) => {
                // I'd make this 'warn' but `do_command` gets invoked for every command and
                // many cores don't have it at all. So it would just be way too spammy.
                trace!(this = ?this, verb, ?task_id, "Verb not found, task cancelled");

                Some(TaskHandleResult::Notify(
                    task_id,
                    TaskWaiterResult::Error(TaskAbortedError),
                ))
            }
            SchedulerControlMsg::TaskCommandError(parse_command_error) => {
                // This is a common occurrence, so we don't want to log it at warn level.
                trace!(?task_id, error = ?parse_command_error, "command parse error");

                Some(TaskHandleResult::Notify(
                    task_id,
                    TaskWaiterResult::Error(CommandExecutionError(parse_command_error)),
                ))
            }
            SchedulerControlMsg::TaskAbortCancelled => {
                warn!(?task_id, "Task cancelled");

                // Rollback the session.
                let Some(task) = self.tasks.get_mut(&task_id) else {
                    warn!(task_id, "Task not found for abort");
                    return None;
                };
                if let Err(send_error) = task
                    .session
                    .send_system_msg(task.player, "Aborted.".to_string().as_str())
                {
                    warn!("Could not send abort message to player: {:?}", send_error);
                };

                let Ok(()) = task.session.rollback() else {
                    warn!("Could not rollback session; aborting task");
                    return Some(TaskHandleResult::Notify(
                        task_id,
                        TaskWaiterResult::Error(TaskAbortedError),
                    ));
                };
                Some(TaskHandleResult::Notify(
                    task_id,
                    TaskWaiterResult::Error(TaskAbortedCancelled),
                ))
            }
            SchedulerControlMsg::TaskAbortLimitsReached(limit_reason) => {
                let abort_reason_text = match limit_reason {
                    AbortLimitReason::Ticks(t) => {
                        warn!(?task_id, ticks = t, "Task aborted, ticks exceeded");
                        format!("Abort: Task exceeded ticks limit of {}", t)
                    }
                    AbortLimitReason::Time(t) => {
                        warn!(?task_id, time = ?t, "Task aborted, time exceeded");
                        format!("Abort: Task exceeded time limit of {:?}", t)
                    }
                };

                // Commit the session.
                let Some(task) = self.tasks.get_mut(&task_id) else {
                    warn!(task_id, "Task not found for abort");
                    return None;
                };

                task.session
                    .send_system_msg(task.player, &abort_reason_text)
                    .expect("Could not send abort message to player");

                let _ = task.session.commit();

                Some(TaskHandleResult::Notify(
                    task_id,
                    TaskWaiterResult::Error(TaskAbortedLimit(limit_reason)),
                ))
            }
            SchedulerControlMsg::TaskException(exception) => {
                warn!(?task_id, finally_reason = ?exception, "Task threw exception");

                let Some(task) = self.tasks.get_mut(&task_id) else {
                    warn!(task_id, "Task not found for abort");
                    return None;
                };

                // Compose a string out of the backtrace
                let mut traceback = vec![];
                for frame in exception.backtrace.iter() {
                    let Variant::Str(s) = frame.variant() else {
                        continue;
                    };
                    traceback.push(format!("{:}\n", s));
                }

                for l in traceback.iter() {
                    if let Err(send_error) = task.session.send_system_msg(task.player, l.as_str()) {
                        warn!("Could not send traceback to player: {:?}", send_error);
                    }
                }

                let _ = task.session.commit();

                Some(TaskHandleResult::Notify(
                    task_id,
                    TaskWaiterResult::Error(TaskAbortedException(exception)),
                ))
            }
            SchedulerControlMsg::TaskRequestFork(fork_request, reply) => {
                trace!(?task_id,  delay=?fork_request.delay, "Task requesting fork");

                // Task has requested a fork. Dispatch it and reply with the new task id.
                // Gotta dump this out til we exit the loop tho, since self.tasks is already
                // borrowed here.
                let Some(task) = self.tasks.get_mut(&task_id) else {
                    warn!(task_id, "Task not found for fork request");
                    return None;
                };
                Some(TaskHandleResult::Fork(ForkRequest {
                    fork_request,
                    reply,
                    session: task.session.clone(),
                }))
            }
            SchedulerControlMsg::TaskSuspend(resume_time) => {
                trace!(task_id, "Handling task suspension until {:?}", resume_time);
                // Task is suspended. The resume time (if any) is the system time at which
                // the scheduler should try to wake us up.

                let Some(mut task) = self.tasks.get_mut(&task_id) else {
                    warn!(task_id, "Task not found for suspend request");
                    return None;
                };

                // Commit the session.
                let Ok(()) = task.session.commit() else {
                    warn!("Could not commit session; aborting task");
                    return Some(TaskHandleResult::Notify(
                        task_id,
                        TaskWaiterResult::Error(TaskAbortedError),
                    ));
                };
                task.suspended = true;
                task.resume_time = resume_time;

                trace!(task_id, resume_time = ?task.resume_time, "Task suspended");
                None
            }
            SchedulerControlMsg::TaskRequestInput => {
                // Task has gone into suspension waiting for input from the client.
                // Create a unique ID for this request, and we'll wake the task when the
                // session receives input.

                let input_request_id = Uuid::new_v4();
                {
                    let Some(mut task) = self.tasks.get_mut(&task_id) else {
                        warn!(task_id, "Task not found for input request");
                        return None;
                    };
                    let Ok(()) = task.session.request_input(task.player, input_request_id) else {
                        warn!("Could not request input from session; aborting task");
                        return Some(TaskHandleResult::Notify(
                            task_id,
                            TaskWaiterResult::Error(TaskAbortedError),
                        ));
                    };
                    task.waiting_input = Some(input_request_id);
                }
                self.input_requests.insert(input_request_id, task_id);
                trace!(?task_id, "Task suspended waiting for input");
                None
            }
            SchedulerControlMsg::DescribeOtherTasks(reply) => {
                // Task is asking for a description of all other tasks.
                Some(TaskHandleResult::Describe(task_id, reply))
            }
            SchedulerControlMsg::KillTask {
                victim_task_id,
                sender_permissions,
                result_sender,
            } => {
                // Task is asking to kill another task.
                Some(TaskHandleResult::Kill(KillRequest {
                    requesting_task_id: task_id,
                    victim_task_id,
                    sender_permissions,
                    result_sender,
                }))
            }
            SchedulerControlMsg::ResumeTask {
                queued_task_id,
                sender_permissions,
                return_value,
                result_sender,
            } => Some(TaskHandleResult::Resume(ResumeRequest {
                requesting_task_id: task_id,
                queued_task_id,
                sender_permissions,
                return_value,
                result_sender,
            })),
            SchedulerControlMsg::BootPlayer {
                player,
                sender_permissions: _,
            } => {
                // Task is asking to boot a player.
                Some(TaskHandleResult::Disconnect(task_id, player))
            }
            SchedulerControlMsg::Notify { player, event } => {
                // Task is asking to notify a player.

                let Some(task) = self.tasks.get_mut(&task_id) else {
                    warn!(task_id, "Task not found for notify request");
                    return None;
                };
                let Ok(()) = task.session.send_event(player, event) else {
                    warn!("Could not notify player; aborting task");
                    return Some(TaskHandleResult::Notify(
                        task_id,
                        TaskWaiterResult::Error(TaskAbortedError),
                    ));
                };
                None
            }
            SchedulerControlMsg::Shutdown(msg) => {
                info!("Shutting down scheduler. Reason: {msg:?}");
                let result_mst = match self.stop() {
                    Ok(_) => v_string("Scheduler stopping.".to_string()),
                    Err(e) => v_string(format!("Shutdown failed: {e}")),
                };
                let Some(task) = self.tasks.get_mut(&task_id) else {
                    warn!(task_id, "Task not found for notify request");
                    return None;
                };
                match task.session.shutdown(msg) {
                    Ok(_) => Some(TaskHandleResult::Notify(
                        task_id,
                        TaskWaiterResult::Success(result_mst),
                    )),
                    Err(e) => {
                        warn!(?e, "Could not notify player; aborting task");
                        Some(TaskHandleResult::Notify(
                            task_id,
                            TaskWaiterResult::Error(TaskAbortedError),
                        ))
                    }
                }
            }
            SchedulerControlMsg::Checkpoint => {
                let Some(textdump_path) = self.config.textdump_output.clone() else {
                    error!("Cannot textdump as textdump_file not configured");
                    return None;
                };

                let db = self.database.clone();
                let tr = std::thread::Builder::new()
                    .name("textdump-thread".to_string())
                    .spawn(move || {
                        let loader_client = {
                            match db.loader_client() {
                                Ok(tx) => tx,
                                Err(e) => {
                                    error!(?e, "Could not start transaction for checkpoint");
                                    return;
                                }
                            }
                        };

                        let Ok(mut output) = File::create(&textdump_path) else {
                            error!("Could not open textdump file for writing");
                            return;
                        };

                        info!("Creating textdump...");
                        let textdump = make_textdump(
                            loader_client.as_ref(),
                            // just to be compatible with LambdaMOO import for now, hopefully.
                            Some("** LambdaMOO Database, Format Version 4 **"),
                        );

                        info!("Writing textdump to {}", textdump_path.display());

                        let mut writer = TextdumpWriter::new(&mut output);
                        if let Err(e) = writer.write_textdump(&textdump) {
                            error!(?e, "Could not write textdump");
                            return;
                        }
                        info!("Textdump written to {}", textdump_path.display());
                    });
                if let Err(e) = tr {
                    error!(?e, "Could not start textdump thread");
                }
                None
            }
        }
    }

    fn submit_fork_task(
        &self,
        fork: Fork,
        session: Arc<dyn Session>,
    ) -> Result<TaskId, SchedulerError> {
        let suspended = fork.delay.is_some();

        let player = fork.player;
        let delay = fork.delay;
        let progr = fork.progr;
        let task_handle = self.new_task(
            TaskStart::StartFork {
                fork_request: fork,
                suspended,
            },
            player,
            session,
            delay,
            self.control_sender.clone(),
            progr,
            false,
        )?;

        let task_id = task_handle.task_id();
        let Some(mut task_ref) = self.tasks.get_mut(&task_id) else {
            return Err(TaskNotFound(task_id));
        };

        // If there's a delay on the fork, we will mark it in suspended state and put in the
        // delay time.
        if let Some(delay) = delay {
            task_ref.suspended = true;
            task_ref.resume_time = Some(SystemTime::now() + delay);
        }

        Ok(task_id)
    }

    fn process_task_action(&self, task_action: TaskHandleResult) {
        let mut to_remove = vec![];
        match task_action {
            TaskHandleResult::Notify(task_id, result) => self.process_notification(task_id, result),
            TaskHandleResult::Fork(fork_request) => {
                self.process_fork_request(fork_request);
            }
            TaskHandleResult::Describe(task_id, reply) => {
                to_remove.extend(self.process_describe_request(task_id, reply))
            }
            TaskHandleResult::Kill(kill_request) => {
                to_remove.extend(self.process_kill_request(kill_request))
            }
            TaskHandleResult::Resume(resume_request) => {
                to_remove.extend(self.process_resume_request(resume_request))
            }
            TaskHandleResult::Disconnect(task_id, player) => {
                self.process_disconnect(task_id, player);
            }
            TaskHandleResult::Retry(task_id) => {
                to_remove.extend(self.process_retry_request(task_id));
            }
        }
        self.process_task_removals(&to_remove);
    }

    fn process_notification(&self, task_id: TaskId, result: TaskWaiterResult) {
        let Some((task_id, task_control)) = self.tasks.remove(&task_id) else {
            // Missing task, must have ended already. This is odd though? So we'll warn.
            warn!(task_id, "Task not found for notification, ignoring");
            return;
        };
        let result_sender = {
            let mut result_sender_lock = task_control.result_sender.lock().unwrap();
            result_sender_lock.take()
        };
        let Some(result_sender) = result_sender else {
            return;
        };
        // There's no guarantee that the other side didn't just go away and drop the Receiver
        // because it's not interested in subscriptions.
        if result_sender.is_closed() {
            return;
        }
        if result_sender.send(result.clone()).is_err() {
            error!("Notify to task {} failed", task_id);
        }
    }

    fn process_wake_ups(&self, to_wake: &[TaskId]) -> Vec<TaskId> {
        let mut to_remove = vec![];

        trace!(?to_wake, "Waking up tasks...");

        for task_id in to_wake {
            let mut task = self.tasks.get_mut(task_id).unwrap();
            task.suspended = false;

            let world_state_source = self
                .database
                .clone()
                .world_state_source()
                .expect("Could not get world state source");

            let tcs = task.task_control_sender.clone();
            if let Err(e) = tcs.send(TaskControlMsg::Resume(world_state_source, v_int(0))) {
                error!(?task_id, error = ?e, "Could not send message resume task. Task being removed.");
                to_remove.push(task.task_id);
            }
        }
        to_remove
    }

    fn process_fork_request(
        &self,
        ForkRequest {
            fork_request,
            reply,
            session,
        }: ForkRequest,
    ) -> Vec<TaskId> {
        let mut to_remove = vec![];
        // Fork the session.
        let forked_session = session.clone();
        let task_id = self
            .submit_fork_task(fork_request, forked_session)
            .unwrap_or_else(|e| panic!("Could not fork task: {:?}", e));

        let reply = reply;
        if let Err(e) = reply.send(task_id) {
            error!(task = task_id, error = ?e, "Could not send fork reply. Parent task gone?  Remove.");
            to_remove.push(task_id);
        }
        to_remove
    }

    fn process_describe_request(
        &self,
        requesting_task_id: TaskId,
        reply: oneshot::Sender<Vec<TaskDescription>>,
    ) -> Vec<TaskId> {
        let mut to_remove = vec![];

        let reply = reply;

        // Note these could be done in parallel and joined instead of single file, to avoid blocking
        // the loop on one uncooperative thread, and could be done in a separate thread as well?
        // The challenge being the borrow semantics of the 'tasks' list.
        // And we should have a timeout here to boot.
        // For now, just iterate blocking.
        let mut tasks = Vec::new();
        trace!(
            task = requesting_task_id,
            "Task requesting task descriptions"
        );
        for t_r in self.tasks.iter() {
            let (task_id, task) = (t_r.key(), t_r.value());
            // Tasks not in suspended state shouldn't be added.
            if !task.suspended {
                continue;
            }
            if *task_id != requesting_task_id {
                trace!(
                    requesting_task_id = requesting_task_id,
                    other_task = task_id,
                    "Requesting task description"
                );
                let (t_send, t_reply) = oneshot::channel();
                let tcs = task.task_control_sender.clone();
                if let Err(e) = tcs.send(TaskControlMsg::Describe(t_send)) {
                    error!(?task_id, error = ?e,
                            "Could not send describe request to task. Task being removed.");
                    to_remove.push(task.task_id);
                    continue;
                }
                let Ok(task_desc) = t_reply.recv() else {
                    error!(?task_id, "Could not get task description");
                    to_remove.push(task.task_id);
                    continue;
                };
                trace!(
                    requesting_task_id = requesting_task_id,
                    other_task = task_id,
                    "Got task description"
                );
                tasks.push(task_desc);
            }
        }
        trace!(
            task = requesting_task_id,
            "Sending task descriptions back..."
        );
        reply.send(tasks).expect("Could not send task description");
        trace!(task = requesting_task_id, "Sent task descriptions back");
        to_remove
    }

    fn process_kill_request(
        &self,
        KillRequest {
            requesting_task_id,
            victim_task_id,
            sender_permissions,
            result_sender,
        }: KillRequest,
    ) -> Vec<TaskId> {
        let mut to_remove = vec![];

        // If the task somehow is requesting a kill on itself, that would lead to deadlock,
        // because we could never send the result back. So we reject that outright. bf_kill_task
        // should be handling this upfront.
        if requesting_task_id == victim_task_id {
            error!(
                task = requesting_task_id,
                "Task requested to kill itself. Ignoring"
            );
            return vec![];
        }

        let victim_task = match self.tasks.get(&victim_task_id) {
            Some(victim_task) => victim_task,
            None => {
                result_sender
                    .send(v_err(E_INVARG))
                    .expect("Could not send kill result");
                return vec![];
            }
        };

        // We reject this outright if the sender permissions are not sufficient:
        //   The either have to be the owner of the task (task.programmer == sender_permissions.task_perms)
        //   Or they have to be a wizard.
        // TODO: Verify kill task permissions is right
        //   Will have to verify that it's enough that .player on task control can
        //   be considered "owner" of the task, or there needs to be some more
        //   elaborate consideration here?
        if !sender_permissions
            .check_is_wizard()
            .expect("Could not check wizard status for kill request")
            && sender_permissions.who != victim_task.player
        {
            result_sender
                .send(v_err(E_PERM))
                .expect("Could not send kill result");
            return vec![];
        }

        let tcs = victim_task.task_control_sender.clone();
        if let Err(e) = tcs.send(TaskControlMsg::Abort) {
            error!(task = victim_task_id, error = ?e, "Could not send kill request to task. Task being removed.");
            to_remove.push(victim_task_id);
        }

        if let Err(e) = result_sender.send(v_none()) {
            error!(task = requesting_task_id, error = ?e, "Could not send kill result to requesting task. Requesting task being removed.");
        }
        to_remove
    }

    fn process_resume_request(
        &self,
        ResumeRequest {
            requesting_task_id,
            queued_task_id,
            sender_permissions,
            return_value,
            result_sender,
        }: ResumeRequest,
    ) -> Option<TaskId> {
        // Task can't resume itself, it couldn't be queued. Builtin should not have sent this
        // request.
        if requesting_task_id == queued_task_id {
            error!(
                task = requesting_task_id,
                "Task requested to resume itself. Ignoring"
            );
            return None;
        }

        // Task does not exist.
        let mut queued_task = match self.tasks.get_mut(&queued_task_id) {
            Some(queued_task) => queued_task,
            None => {
                result_sender
                    .send(v_err(E_INVARG))
                    .expect("Could not send resume result");
                return None;
            }
        };

        // No permissions.
        if !sender_permissions
            .check_is_wizard()
            .expect("Could not check wizard status for resume request")
            && sender_permissions.who != queued_task.player
        {
            result_sender
                .send(v_err(E_PERM))
                .expect("Could not send resume result");
            return None;
        }
        // Task is not suspended.
        if !queued_task.suspended {
            result_sender
                .send(v_err(E_INVARG))
                .expect("Could not send resume result");
            return None;
        }

        // Follow the usual task resume logic.
        let state_source = self
            .database
            .clone()
            .world_state_source()
            .expect("Unable to create world state source from database");

        queued_task.suspended = false;

        let tcs = queued_task.task_control_sender.clone();
        if let Err(e) = tcs.send(TaskControlMsg::Resume(state_source, return_value)) {
            error!(task = queued_task_id, error = ?e,
                    "Could not send resume request to task. Task being removed.");
            return Some(queued_task_id);
        }

        if let Err(e) = result_sender.send(v_none()) {
            error!(task = requesting_task_id, error = ?e,
                    "Could not send resume result to requesting task. Requesting task being removed.");
            return Some(requesting_task_id);
        }

        None
    }

    fn process_retry_request(&self, task_id: TaskId) -> Option<TaskId> {
        let Some(mut task) = self.tasks.get_mut(&task_id) else {
            warn!(task = task_id, "Retrying task not found");
            return None;
        };

        // Create a new transaction.
        let state_source = self
            .database
            .clone()
            .world_state_source()
            .expect("Unable to get world source from database");

        task.suspended = false;

        let tcs = task.task_control_sender.clone();
        if let Err(e) = tcs.send(TaskControlMsg::Restart(state_source)) {
            error!(task = task_id, error = ?e,
                    "Could not send resume request to task. Task being removed.");
            return Some(task_id);
        }
        None
    }

    fn process_disconnect(&self, disconnect_task_id: TaskId, player: Objid) {
        let Some(task) = self.tasks.get_mut(&disconnect_task_id) else {
            warn!(task = disconnect_task_id, "Disconnecting task not found");
            return;
        };
        // First disconnect the player...
        warn!(?player, ?disconnect_task_id, "Disconnecting player");
        if let Err(e) = task.session.disconnect(player) {
            warn!(?player, ?disconnect_task_id, error = ?e, "Could not disconnect player's session");
            return;
        }

        // Then abort all of their still-living forked tasks (that weren't the disconnect
        // task, we need to let that run to completion for sanity's sake.)
        for t in self.tasks.iter() {
            let (task_id, task) = (t.key(), t.value());
            if *task_id == disconnect_task_id {
                continue;
            }
            if task.player != player {
                continue;
            }
            warn!(
                ?player,
                task_id, "Aborting task from disconnected player..."
            );
            // This is fire and forget, we cannot assume that the task is still alive.
            let tcs = task.task_control_sender.clone();
            let Ok(_) = tcs.send(TaskControlMsg::Abort) else {
                trace!(?player, task_id, "Task already dead");
                continue;
            };
        }
    }

    fn process_task_removals(&self, to_remove: &[TaskId]) {
        for task_id in to_remove {
            trace!(task = task_id, "Task removed");
            self.tasks.remove(task_id);
        }
    }

    // Yes yes I know it's a lot of arguments, but wrapper object here is redundant.
    #[allow(clippy::too_many_arguments)]
    fn new_task(
        &self,
        task_start: TaskStart,
        player: Objid,
        session: Arc<dyn Session>,
        delay_start: Option<Duration>,
        control_sender: Sender<(TaskId, SchedulerControlMsg)>,
        perms: Objid,
        is_background: bool,
    ) -> Result<TaskHandle, SchedulerError> {
        let task_id = self.next_task_id.fetch_add(1, Ordering::SeqCst);
        let (task_control_sender, task_control_receiver) = crossbeam_channel::unbounded();

        let state_source = self
            .database
            .clone()
            .world_state_source()
            .expect("Unable to instantiate database");

        // TODO: support a queue-size on concurrent executing tasks and allow them to sit in an
        //   initially suspended state without spawning a worker thread, until the queue has space.
        // Spawn the task's thread.
        let task_state_source = state_source.clone();
        let task_session = session.clone();

        let (sender, receiver) = oneshot::channel();
        let name = format!("moor-task-{}-player-{}", task_id, player);
        let task_control = TaskControl {
            task_id,
            player,
            task_control_sender,
            state_source,
            session,
            suspended: false,
            waiting_input: None,
            resume_time: None,
            result_sender: Mutex::new(Some(sender)),
        };
        self.tasks.insert(task_id, task_control);

        // Footgun warning: ALWAYS `self.tasks.insert` before spawning the task thread!

        std::thread::Builder::new()
            .name(name)
            .spawn(move || {
                trace!(?task_id, ?task_start, "Starting up task");
                Task::run(
                    task_id,
                    task_start,
                    perms,
                    delay_start,
                    task_state_source,
                    is_background,
                    task_session,
                    task_control_receiver,
                    control_sender,
                );
                trace!(?task_id, "Completed task");
            })
            .expect("Could not spawn task thread");

        Ok(TaskHandle(task_id, receiver))
    }
}
