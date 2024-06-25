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

use std::collections::HashMap;
use std::fs::File;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use bincode::{Decode, Encode};
use crossbeam_channel::Sender;

use thiserror::Error;
use tracing::{debug, error, info, instrument, trace, warn};
use uuid::Uuid;

use crossbeam_channel::Receiver;
use std::sync::Mutex;
use std::thread::yield_now;

use moor_compiler::compile;
use moor_compiler::CompileError;
use moor_db::Database;
use moor_values::model::VerbProgramError;
use moor_values::model::{BinaryType, CommandError, HasUuid, VerbAttrs};
use moor_values::model::{CommitResult, Perms};
use moor_values::var::Error::{E_INVARG, E_PERM};
use moor_values::var::{v_err, v_int, v_string, List, Var};
use moor_values::var::{Objid, Variant};
use moor_values::{AsByteBuffer, SYSTEM_OBJECT};
use SchedulerError::{
    CommandExecutionError, CompilationError, InputRequestNotFound, TaskAbortedCancelled,
    TaskAbortedError, TaskAbortedException, TaskAbortedLimit,
};

use crate::config::Config;
use crate::matching::match_env::MatchEnvironmentParseMatcher;
use crate::matching::ws_match_env::WsMatchEnv;
use crate::tasks::command_parse::ParseMatcher;
use crate::tasks::scheduler::SchedulerError::{TaskNotFound, VerbProgramFailed};
use crate::tasks::sessions::Session;
use crate::tasks::task::Task;
use crate::tasks::task_messages::{SchedulerControlMsg, TaskStart};
use crate::tasks::{TaskDescription, TaskHandle, TaskId};
use crate::textdump::{make_textdump, TextdumpWriter};
use crate::vm::Fork;
use crate::vm::UncaughtException;

const SCHEDULER_TICK_TIME: Duration = Duration::from_millis(5);

/// Number of times to retry a program compilation transaction in case of conflict, before giving up.
const NUM_VERB_PROGRAM_ATTEMPTS: usize = 5;

/// Responsible for the dispatching, control, and accounting of tasks in the system.
/// There should be only one scheduler per server.
pub struct Scheduler {
    control_sender: Sender<(TaskId, SchedulerControlMsg)>,
    control_receiver: Receiver<(TaskId, SchedulerControlMsg)>,
    config: Arc<Config>,

    running: Arc<AtomicBool>,
    database: Arc<dyn Database + Send + Sync>,
    next_task_id: AtomicUsize,

    /// The internal task queue which holds our suspended tasks, and control records for actively
    /// running tasks.
    task_q: Arc<Mutex<TaskQ>>,
}

/// Scheduler-side per-task record. Lives in the scheduler thread and owned by the scheduler and
/// not shared elsewhere.
/// The actual `Task` is owned by the task thread until it is suspended or completed.
/// (When suspended it is moved into a `SuspendedTask` in the `.suspended` list)
struct RunningTaskControl {
    /// For which player this task is running on behalf of.
    player: Objid,
    /// A kill switch to signal the task to stop. True means the VM execution thread should stop
    /// as soon as it can.
    kill_switch: Arc<AtomicBool>,
    /// The connection-session for this task.
    session: Arc<dyn Session>,
    /// A mailbox to deliver the result of the task to a waiting party with a subscription, if any.
    result_sender: Option<oneshot::Sender<TaskResult>>,
}

/// State a suspended task sits in inside the `suspended` side of the task queue.
/// When tasks are not running they are moved into these.
struct SuspendedTask {
    wake_condition: WakeCondition,
    task: Task,
    session: Arc<dyn Session>,
    result_sender: Option<oneshot::Sender<TaskResult>>,
}

/// Possible conditions in which a suspended task can wake from suspension.
enum WakeCondition {
    /// This task will never wake up on its own, and must be manually woken with `bf_resume`
    Never,
    /// This task will wake up when the given time is reached.
    Time(Instant),
    /// This task will wake up when the given input request is fulfilled.
    Input(Uuid),
}

/// The internal state of the task queue.
struct TaskQ {
    tasks: HashMap<TaskId, RunningTaskControl>,
    suspended: HashMap<TaskId, SuspendedTask>,
}

/// Reasons a task might be aborted for a 'limit'
#[derive(Clone, Copy, Debug, Eq, PartialEq, Decode, Encode)]
pub enum AbortLimitReason {
    /// This task hit its allotted tick limit.
    Ticks(usize),
    /// This task hit its allotted time limit.
    Time(Duration),
}

/// Possible results returned to waiters on tasks to which they've subscribed.
#[derive(Clone, Debug)]
pub enum TaskResult {
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
    #[error("Compilation error")]
    CompilationError(#[source] CompileError),
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
    #[error("Unable to program verb {0}")]
    VerbProgramFailed(VerbProgramError),
}

impl Scheduler {
    pub fn new(database: Arc<dyn Database + Send + Sync>, config: Config) -> Self {
        let config = Arc::new(config);
        let (control_sender, control_receiver) = crossbeam_channel::unbounded();
        let inner = TaskQ {
            tasks: Default::default(),
            suspended: Default::default(),
        };
        Self {
            running: Arc::new(AtomicBool::new(false)),
            database,
            next_task_id: Default::default(),
            task_q: Arc::new(Mutex::new(inner)),
            config,
            control_sender,
            control_receiver,
        }
    }

    /// Execute the scheduler loop, run from the server process.
    #[instrument(skip(self))]
    pub fn run(self: Arc<Self>) {
        self.running.store(true, Ordering::SeqCst);
        info!("Starting scheduler loop");
        loop {
            let is_running = self.running.load(Ordering::SeqCst);
            if !is_running {
                warn!("Scheduler stopping");
                break;
            }
            {
                // Look for tasks that need to be woken (have hit their wakeup-time), and wake them.
                let mut inner = self.task_q.lock().unwrap();
                let now = Instant::now();

                // We need to take the tasks that need waking out of the suspended list, and then
                // rehydrate them.
                let to_wake = inner
                    .suspended
                    .iter()
                    .filter_map(|(task_id, sr)| match &sr.wake_condition {
                        WakeCondition::Time(t) => (*t <= now).then_some(*task_id),
                        _ => None,
                    })
                    .collect::<Vec<_>>();

                for task_id in to_wake {
                    let sr = inner.suspended.remove(&task_id).unwrap();
                    if let Err(e) = inner.resume_task_thread(
                        sr.task,
                        v_int(0),
                        sr.session,
                        sr.result_sender,
                        &self.control_sender,
                        self.database.clone(),
                    ) {
                        error!(?task_id, ?e, "Error resuming task");
                    }
                }
            }
            if let Ok((task_id, msg)) = self.control_receiver.recv_timeout(SCHEDULER_TICK_TIME) {
                self.handle_task_control_msg(task_id, msg);
            }
        }
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

        let task_start = Arc::new(TaskStart::StartCommandVerb {
            player,
            command: command.to_string(),
        });

        self.new_task(task_start, player, session, None, player, false)
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
        trace!(?input_request_id, ?input, "Received input for task");

        let mut inner = self.task_q.lock().unwrap();

        // Find the task that requested this input, if any
        let Some((task_id, perms)) = inner.suspended.iter().find_map(|(task_id, sr)| {
            if let WakeCondition::Input(request_id) = &sr.wake_condition {
                if *request_id == input_request_id {
                    Some((*task_id, sr.task.perms))
                } else {
                    None
                }
            } else {
                None
            }
        }) else {
            warn!(?input_request_id, "Input request not found");
            return Err(InputRequestNotFound(input_request_id.as_u128()));
        };

        // If the player doesn't match, we'll pretend we didn't even see it.
        if perms != player {
            warn!(
                ?task_id,
                ?input_request_id,
                ?player,
                "Task input request received for wrong player"
            );
            return Err(TaskNotFound(task_id));
        }

        let sr = inner.suspended.remove(&task_id).expect("Corrupt task list");

        // Wake and bake.
        inner.resume_task_thread(
            sr.task,
            v_string(input),
            sr.session,
            sr.result_sender,
            &self.control_sender,
            self.database.clone(),
        )
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
        let task_start = Arc::new(TaskStart::StartVerb {
            player,
            vloc,
            verb,
            args: List::from_slice(&args),
            argstr,
        });

        self.new_task(task_start, player, session, None, perms, false)
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
        let task_start = Arc::new(TaskStart::StartVerb {
            player,
            vloc: SYSTEM_OBJECT,
            verb: "do_out_of_band_command".to_string(),
            args: List::from_slice(&args),
            argstr,
        });

        self.new_task(task_start, player, session, None, player, false)
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
            Err(e) => return Err(CompilationError(e)),
        };

        let task_start = Arc::new(TaskStart::StartEval {
            player,
            program: binary,
        });

        self.new_task(task_start, player, sessions, None, perms, false)
    }

    #[instrument(skip(self))]
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

    /// Start a transaction, match the object name and verb name, and if it exists and the
    /// permissions are correct, program the verb with the given code.
    // TODO: this probably doesn't belong on scheduler
    #[instrument(skip(self))]
    pub fn program_verb(
        &self,
        player: Objid,
        perms: Objid,
        object_name: String,
        verb_name: String,
        code: Vec<String>,
    ) -> Result<(Objid, String), SchedulerError> {
        let db = self.database.clone().world_state_source().unwrap();
        for _ in 0..NUM_VERB_PROGRAM_ATTEMPTS {
            let mut tx = db.new_world_state().unwrap();

            let match_env = WsMatchEnv {
                ws: tx.as_mut(),
                perms,
            };
            let matcher = MatchEnvironmentParseMatcher {
                env: match_env,
                player,
            };
            let Ok(Some(o)) = matcher.match_object(&object_name) else {
                let _ = tx.rollback();
                return Err(CommandExecutionError(CommandError::NoObjectMatch));
            };

            let vi = tx
                .find_method_verb_on(perms, o, &verb_name)
                .map_err(|_| VerbProgramFailed(VerbProgramError::NoVerbToProgram))?;

            if vi.verbdef().location() != o {
                let _ = tx.rollback();
                return Err(VerbProgramFailed(VerbProgramError::NoVerbToProgram));
            }

            let program = compile(code.join("\n").as_str()).map_err(|e| {
                VerbProgramFailed(VerbProgramError::CompilationError(vec![format!("{:?}", e)]))
            })?;

            // Now we have a program, we need to encode it.
            let binary = program
                .with_byte_buffer(|d| Vec::from(d))
                .expect("Failed to encode program byte stream");
            // Now we can update the verb.
            let update_attrs = VerbAttrs {
                definer: None,
                owner: None,
                names: None,
                flags: None,
                args_spec: None,
                binary_type: Some(BinaryType::LambdaMoo18X),
                binary: Some(binary),
            };
            tx.update_verb_with_id(perms, o, vi.verbdef().uuid(), update_attrs)
                .map_err(|_| VerbProgramFailed(VerbProgramError::NoVerbToProgram))?;

            let commit_result = tx.commit().unwrap();
            if commit_result == CommitResult::Success {
                return Ok((o, verb_name));
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        error!("Could not commit transaction after {NUM_VERB_PROGRAM_ATTEMPTS} tries.");
        Err(VerbProgramFailed(VerbProgramError::DatabaseError))
    }

    /// Request information on all (suspended) tasks known to the scheduler.
    pub fn tasks(&self) -> Vec<TaskDescription> {
        let inner = self.task_q.lock().unwrap();
        let mut tasks = Vec::new();

        // Suspended tasks.
        for (_, sr) in inner.suspended.iter() {
            let start_time = match sr.wake_condition {
                WakeCondition::Time(t) => {
                    let distance_from_now = t.duration_since(Instant::now());
                    Some(SystemTime::now() + distance_from_now)
                }
                _ => None,
            };
            tasks.push(TaskDescription {
                task_id: sr.task.task_id,
                start_time,
                permissions: sr.task.perms,
                verb_name: sr.task.vm_host.verb_name().clone(),
                verb_definer: sr.task.vm_host.verb_definer(),
                line_number: sr.task.vm_host.line_number(),
                this: sr.task.vm_host.this(),
            });
        }
        tasks
    }

    /// Stop the scheduler run loop.
    pub fn stop(&self) -> Result<(), SchedulerError> {
        warn!("Issuing clean shutdown...");
        {
            // Send shut down to all the tasks.
            let mut inner = self.task_q.lock().unwrap();
            for (_, task) in inner.tasks.drain() {
                task.kill_switch.store(true, Ordering::SeqCst);
            }
        }
        warn!("Waiting for tasks to finish...");

        // Then spin until they're all done.
        loop {
            {
                let inner = self.task_q.lock().unwrap();
                if inner.tasks.is_empty() {
                    break;
                }
            }
            yield_now();
        }

        warn!("All tasks finished.  Stopping scheduler.");
        self.running.store(false, Ordering::SeqCst);

        Ok(())
    }
}

impl Scheduler {
    /// Handle scheduler control messages inbound from tasks.
    /// Note: this function should never be allowed to panic, as it is called from the scheduler main loop.
    #[instrument(skip(self))]
    fn handle_task_control_msg(&self, task_id: TaskId, msg: SchedulerControlMsg) {
        match msg {
            SchedulerControlMsg::TaskSuccess(value) => {
                // Commit the session.
                let mut inner = self.task_q.lock().unwrap();
                let Some(task) = inner.tasks.get_mut(&task_id) else {
                    warn!(task_id, "Task not found for success");
                    return;
                };
                let Ok(()) = task.session.commit() else {
                    warn!("Could not commit session; aborting task");
                    return inner.send_task_result(task_id, TaskResult::Error(TaskAbortedError));
                };
                trace!(?task_id, result = ?value, "Task succeeded");
                return inner.send_task_result(task_id, TaskResult::Success(value));
            }
            SchedulerControlMsg::TaskConflictRetry(task) => {
                trace!(?task_id, "Task retrying due to conflict");

                // Ask the task to restart itself, using its stashed original start info, but with
                // a brand new transaction.
                let mut inner = self.task_q.lock().unwrap();
                inner.retry_task(task, &self.control_sender, self.database.clone());
            }
            SchedulerControlMsg::TaskVerbNotFound(this, verb) => {
                // I'd make this 'warn' but `do_command` gets invoked for every command and
                // many cores don't have it at all. So it would just be way too spammy.
                trace!(this = ?this, verb, ?task_id, "Verb not found, task cancelled");
                let mut inner = self.task_q.lock().unwrap();
                inner.send_task_result(task_id, TaskResult::Error(TaskAbortedError));
            }
            SchedulerControlMsg::TaskCommandError(parse_command_error) => {
                // This is a common occurrence, so we don't want to log it at warn level.
                trace!(?task_id, error = ?parse_command_error, "command parse error");
                let mut inner = self.task_q.lock().unwrap();
                inner.send_task_result(
                    task_id,
                    TaskResult::Error(CommandExecutionError(parse_command_error)),
                );
            }
            SchedulerControlMsg::TaskAbortCancelled => {
                warn!(?task_id, "Task cancelled");

                // Rollback the session.
                let mut inner = self.task_q.lock().unwrap();
                let Some(task) = inner.tasks.get_mut(&task_id) else {
                    warn!(task_id, "Task not found for abort");
                    return;
                };
                if let Err(send_error) = task
                    .session
                    .send_system_msg(task.player, "Aborted.".to_string().as_str())
                {
                    warn!("Could not send abort message to player: {:?}", send_error);
                };

                let Ok(()) = task.session.rollback() else {
                    warn!("Could not rollback session; aborting task");
                    return inner.send_task_result(task_id, TaskResult::Error(TaskAbortedError));
                };
                inner.send_task_result(task_id, TaskResult::Error(TaskAbortedCancelled));
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

                // Commit the session
                let mut inner = self.task_q.lock().unwrap();
                let Some(task) = inner.tasks.get_mut(&task_id) else {
                    warn!(task_id, "Task not found for abort");
                    return;
                };

                task.session
                    .send_system_msg(task.player, &abort_reason_text)
                    .expect("Could not send abort message to player");

                let _ = task.session.commit();

                inner.send_task_result(task_id, TaskResult::Error(TaskAbortedLimit(limit_reason)));
            }
            SchedulerControlMsg::TaskException(exception) => {
                warn!(?task_id, finally_reason = ?exception, "Task threw exception");

                let mut inner = self.task_q.lock().unwrap();
                let Some(task) = inner.tasks.get_mut(&task_id) else {
                    warn!(task_id, "Task not found for abort");
                    return;
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

                inner.send_task_result(task_id, TaskResult::Error(TaskAbortedException(exception)));
            }
            SchedulerControlMsg::TaskRequestFork(fork_request, reply) => {
                trace!(?task_id,  delay=?fork_request.delay, "Task requesting fork");

                // Task has requested a fork. Dispatch it and reply with the new task id.
                // Gotta dump this out til we exit the loop tho, since self.tasks is already
                // borrowed here.
                let new_session = {
                    let mut inner = self.task_q.lock().unwrap();

                    let Some(task) = inner.tasks.get_mut(&task_id) else {
                        warn!(task_id, "Task not found for fork request");
                        return;
                    };
                    task.session.clone()
                };
                self.process_fork_request(fork_request, reply, new_session);
            }
            SchedulerControlMsg::TaskSuspend(resume_time, task) => {
                debug!(task_id, "Handling task suspension until {:?}", resume_time);
                // Task is suspended. The resume time (if any) is the system time at which
                // the scheduler should try to wake us up.

                let mut inner = self.task_q.lock().unwrap();

                // Remove from the local task control...
                let Some(tc) = inner.tasks.remove(&task_id) else {
                    warn!(task_id, "Task not found for suspend request");
                    return;
                };

                // Commit the session.
                let Ok(()) = tc.session.commit() else {
                    warn!("Could not commit session; aborting task");
                    return inner.send_task_result(task_id, TaskResult::Error(TaskAbortedError));
                };

                // And insert into the suspended list.
                let wake_condition = match resume_time {
                    Some(t) => WakeCondition::Time(t),
                    None => WakeCondition::Never,
                };

                inner.suspended.insert(
                    task_id,
                    SuspendedTask {
                        wake_condition,
                        task,
                        session: tc.session,
                        result_sender: tc.result_sender,
                    },
                );

                debug!(task_id, "Task suspended");
            }
            SchedulerControlMsg::TaskRequestInput(task) => {
                // Task has gone into suspension waiting for input from the client.
                // Create a unique ID for this request, and we'll wake the task when the
                // session receives input.

                let input_request_id = Uuid::new_v4();
                let mut inner = self.task_q.lock().unwrap();
                let Some(tc) = inner.tasks.remove(&task_id) else {
                    warn!(task_id, "Task not found for input request");
                    return;
                };
                // Commit the session (not DB transaction) to make sure current output is
                // flushed up to the prompt point.
                let Ok(()) = tc.session.commit() else {
                    warn!("Could not commit session; aborting task");
                    return inner.send_task_result(task_id, TaskResult::Error(TaskAbortedError));
                };

                let Ok(()) = tc.session.request_input(tc.player, input_request_id) else {
                    warn!("Could not request input from session; aborting task");
                    return inner.send_task_result(task_id, TaskResult::Error(TaskAbortedError));
                };
                inner.suspended.insert(
                    task_id,
                    SuspendedTask {
                        wake_condition: WakeCondition::Input(input_request_id),
                        task,
                        session: tc.session,
                        result_sender: tc.result_sender,
                    },
                );

                trace!(?task_id, "Task suspended waiting for input");
            }
            SchedulerControlMsg::RequestQueuedTasks(reply) => {
                // Task is asking for a description of all other tasks.
                if let Err(e) = reply.send(self.tasks()) {
                    error!(?e, "Could not send task description to requester");
                    // TODO: murder this errant task
                }
            }
            SchedulerControlMsg::KillTask {
                victim_task_id,
                sender_permissions,
                result_sender,
            } => {
                // Task is asking to kill another task.
                let mut inner = self.task_q.lock().unwrap();
                inner.kill_task(victim_task_id, sender_permissions, result_sender);
            }
            SchedulerControlMsg::ResumeTask {
                queued_task_id,
                sender_permissions,
                return_value,
                result_sender,
            } => {
                let mut inner = self.task_q.lock().unwrap();
                inner.resume_task(
                    task_id,
                    queued_task_id,
                    sender_permissions,
                    return_value,
                    result_sender,
                    &self.control_sender,
                    self.database.clone(),
                );
            }
            SchedulerControlMsg::BootPlayer {
                player,
                sender_permissions: _,
            } => {
                // Task is asking to boot a player.
                let mut inner = self.task_q.lock().unwrap();
                inner.disconnect_task(task_id, player);
            }
            SchedulerControlMsg::Notify { player, event } => {
                // Task is asking to notify a player of an event.
                let mut inner = self.task_q.lock().unwrap();
                let Some(task) = inner.tasks.get_mut(&task_id) else {
                    warn!(task_id, "Task not found for notify request");
                    return;
                };
                let Ok(()) = task.session.send_event(player, event) else {
                    warn!("Could not notify player; aborting task");
                    return inner.send_task_result(task_id, TaskResult::Error(TaskAbortedError));
                };
            }
            SchedulerControlMsg::Shutdown(msg) => {
                info!("Shutting down scheduler. Reason: {msg:?}");
                let result_mst = match self.stop() {
                    Ok(_) => v_string("Scheduler stopping.".to_string()),
                    Err(e) => v_string(format!("Shutdown failed: {e}")),
                };
                let mut inner = self.task_q.lock().unwrap();
                let Some(task) = inner.tasks.get_mut(&task_id) else {
                    warn!(task_id, "Task not found for notify request");
                    return;
                };
                match task.session.shutdown(msg) {
                    Ok(_) => inner.send_task_result(task_id, TaskResult::Success(result_mst)),
                    Err(e) => {
                        warn!(?e, "Could not notify player; aborting task");
                        inner.send_task_result(task_id, TaskResult::Error(TaskAbortedError));
                    }
                }
            }
            SchedulerControlMsg::Checkpoint => {
                let Some(textdump_path) = self.config.textdump_output.clone() else {
                    error!("Cannot textdump as textdump_file not configured");
                    return;
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
            }
        }
    }

    #[instrument(skip(self, session))]
    fn process_fork_request(
        &self,
        fork_request: Fork,
        reply: oneshot::Sender<TaskId>,
        session: Arc<dyn Session>,
    ) {
        let mut to_remove = vec![];

        // Fork the session.
        let forked_session = session.clone();

        let suspended = fork_request.delay.is_some();
        let player = fork_request.player;
        let delay = fork_request.delay;
        let progr = fork_request.progr;

        let task_handle = match self.new_task(
            Arc::new(TaskStart::StartFork {
                fork_request,
                suspended,
            }),
            player,
            forked_session,
            delay,
            progr,
            true,
        ) {
            Ok(th) => th,
            Err(e) => {
                error!(?e, "Could not fork task");
                return;
            }
        };

        let task_id = task_handle.task_id();

        let reply = reply;
        if let Err(e) = reply.send(task_id) {
            error!(task = task_id, error = ?e, "Could not send fork reply. Parent task gone?  Remove.");
            to_remove.push(task_id);
        }
    }

    #[instrument(skip(self, session))]
    #[allow(clippy::too_many_arguments)]
    fn new_task(
        &self,
        task_start: Arc<TaskStart>,
        player: Objid,
        session: Arc<dyn Session>,
        delay_start: Option<Duration>,
        perms: Objid,
        is_background: bool,
    ) -> Result<TaskHandle, SchedulerError> {
        // TODO: support a queue-size on concurrent executing tasks and allow them to sit in an
        //   initially suspended state without spawning a worker thread, until the queue has space.
        let task_id = self.next_task_id.fetch_add(1, Ordering::SeqCst);
        let mut inner = self.task_q.lock().unwrap();
        inner.start_task_thread(
            task_id,
            task_start,
            player,
            session,
            delay_start,
            perms,
            is_background,
            &self.control_sender,
            self.database.clone(),
        )
    }
}

impl TaskQ {
    #[allow(clippy::too_many_arguments)]
    #[instrument(skip(self, control_sender, database, session))]
    fn start_task_thread(
        &mut self,
        task_id: TaskId,
        task_start: Arc<TaskStart>,
        player: Objid,
        session: Arc<dyn Session>,
        delay_start: Option<Duration>,
        perms: Objid,
        is_background: bool,
        control_sender: &Sender<(TaskId, SchedulerControlMsg)>,
        database: Arc<dyn Database>,
    ) -> Result<TaskHandle, SchedulerError> {
        let state_source = database
            .world_state_source()
            .expect("Unable to instantiate database");

        let (sender, receiver) = oneshot::channel();

        let kill_switch = Arc::new(AtomicBool::new(false));
        let mut task = Task::new(
            task_id,
            player,
            task_start,
            perms,
            is_background,
            session.clone(),
            control_sender,
            kill_switch.clone(),
        );

        // If this task is delayed, stick it into suspension state immediately.
        if let Some(delay) = delay_start {
            // However we'll need the task to be in a resumable state, which means executing
            //  setup_task_start in a transaction.
            let mut world_state = match state_source.new_world_state() {
                Ok(ws) => ws,
                Err(e) => {
                    error!(error = ?e, "Could not start transaction for delayed task");
                    return Err(SchedulerError::CouldNotStartTask);
                }
            };

            if !task.setup_task_start(control_sender, world_state.as_mut()) {
                error!(task_id, "Could not setup task start");
                return Err(SchedulerError::CouldNotStartTask);
            }

            match world_state.commit() {
                Ok(CommitResult::Success) => {}
                // TODO: perform a retry here in a modest loop.
                Ok(CommitResult::ConflictRetry) => {
                    error!(task_id, "Conflict during task start");
                    return Err(SchedulerError::CouldNotStartTask);
                }
                Err(e) => {
                    error!(task_id, error = ?e, "Error committing task start");
                    return Err(SchedulerError::CouldNotStartTask);
                }
            }
            let wake_condition = WakeCondition::Time(Instant::now() + delay);
            self.suspended.insert(
                task_id,
                SuspendedTask {
                    // (suspend_until, task, session)
                    wake_condition,
                    task,
                    session,
                    result_sender: Some(sender),
                },
            );
            return Ok(TaskHandle(task_id, receiver));
        }

        // Otherwise, we create a task control record and fire up a thread.
        let task_control = RunningTaskControl {
            player,
            kill_switch,
            session,
            result_sender: Some(sender),
        };

        // Footgun warning: ALWAYS `self.tasks.insert` before spawning the task thread!
        self.tasks.insert(task_id, task_control);

        let thread_name = format!("moor-task-{}-player-{}", task_id, player);
        let control_sender = control_sender.clone();
        std::thread::Builder::new()
            .name(thread_name)
            .spawn(move || {
                trace!(?task_id, "Starting up task");
                // Start the db transaction, which will initially be used to resolve the verb before the task
                // starts executing.
                let mut world_state = match state_source.new_world_state() {
                    Ok(ws) => ws,
                    Err(e) => {
                        error!(error = ?e, "Could not start transaction for task");
                        return;
                    }
                };

                if !task.setup_task_start(&control_sender, world_state.as_mut()) {
                    error!(task_id, "Could not setup task start");
                    return;
                }

                Task::run_task_loop(task, control_sender, world_state);
                trace!(?task_id, "Completed task");
            })
            .expect("Could not spawn task thread");

        Ok(TaskHandle(task_id, receiver))
    }

    #[allow(clippy::too_many_arguments)]
    #[instrument(skip(self, result_sender, control_sender, database, session))]
    fn resume_task_thread(
        &mut self,
        mut task: Task,
        resume_val: Var,
        session: Arc<dyn Session>,
        result_sender: Option<oneshot::Sender<TaskResult>>,
        control_sender: &Sender<(TaskId, SchedulerControlMsg)>,
        database: Arc<dyn Database>,
    ) -> Result<(), SchedulerError> {
        // Take a task out of a suspended state and start running it again.
        // Means:
        //   Start a new transaction
        //   Create a new control record
        //   Push resume-value into the task

        let state_source = database
            .world_state_source()
            .expect("Unable to instantiate database");

        let task_id = task.task_id;
        let player = task.perms;
        let kill_switch = task.kill_switch.clone();
        let task_control = RunningTaskControl {
            player,
            kill_switch,
            session,
            result_sender,
        };

        self.tasks.insert(task_id, task_control);
        task.vm_host.resume_execution(resume_val);
        let thread_name = format!("moor-task-{}-player-{}", task_id, player);
        let control_sender = control_sender.clone();
        std::thread::Builder::new()
            .name(thread_name)
            .spawn(move || {
                // Start its new transaction...
                let world_state = match state_source.new_world_state() {
                    Ok(ws) => ws,
                    Err(e) => {
                        error!(error = ?e, "Could not start transaction for task resumption");
                        return;
                    }
                };

                Task::run_task_loop(task, control_sender, world_state);
                trace!(?task_id, "Completed task");
            })
            .expect("Could not spawn task thread");

        Ok(())
    }

    fn send_task_result(&mut self, task_id: TaskId, result: TaskResult) {
        let Some(mut task_control) = self.tasks.remove(&task_id) else {
            // Missing task, must have ended already or gone into suspension?
            // This is odd though? So we'll warn.
            warn!(task_id, "Task not found for notification, ignoring");
            return;
        };
        let result_sender = task_control.result_sender.take();
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

    #[instrument(skip(self, control_sender, database))]
    fn retry_task(
        &mut self,
        task: Task,
        control_sender: &Sender<(TaskId, SchedulerControlMsg)>,
        database: Arc<dyn Database>,
    ) {
        // Make sure the old thread is dead.
        task.kill_switch.store(false, Ordering::SeqCst);

        // Remove this from the running tasks.
        // By definition we can't respond to a retry for a suspended task, so if it's not in the
        // running tasks there's something very wrong.
        let old_tc = self
            .tasks
            .remove(&task.task_id)
            .expect("Task not found for retry");

        // Grab the "task start" record from the (now dead) task, and submit this again with the same
        // task_id.
        let task_start = task.task_start.clone();
        if let Err(e) = self.start_task_thread(
            task.task_id,
            task_start,
            old_tc.player,
            old_tc.session,
            None,
            task.perms,
            false,
            control_sender,
            database,
        ) {
            error!(?e, "Could not restart task");
        }
    }

    #[instrument(skip(self, result_sender))]
    fn kill_task(
        &mut self,
        victim_task_id: TaskId,
        sender_permissions: Perms,
        result_sender: oneshot::Sender<Var>,
    ) {
        // We need to do perms check first, which means checking both running and suspended tasks,
        // and getting their permissions. And may as well remember whether it was in suspended or
        // active at the same time.
        let (perms, is_suspended) = match self.suspended.get(&victim_task_id) {
            Some(sr) => (sr.task.perms, true),
            None => match self.tasks.get(&victim_task_id) {
                Some(task) => (task.player, false),
                None => {
                    result_sender
                        .send(v_err(E_INVARG))
                        .expect("Could not send kill result");
                    return;
                }
            },
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
            && sender_permissions.who != perms
        {
            result_sender
                .send(v_err(E_PERM))
                .expect("Could not send kill result");
            return;
        }

        // If suspended we can just remove completely and move on.
        if is_suspended {
            if self.suspended.remove(&victim_task_id).is_none() {
                error!(
                    task = victim_task_id,
                    "Task not found in suspended list for kill request"
                );
            }
            return;
        }

        // Otherwise we have to check if the task is running, remove its control record, and flip
        // its kill switch.
        let victim_task = match self.tasks.remove(&victim_task_id) {
            Some(victim_task) => victim_task,
            None => {
                result_sender
                    .send(v_err(E_INVARG))
                    .expect("Could not send kill result");
                return;
            }
        };
        victim_task.kill_switch.store(true, Ordering::SeqCst);
    }

    #[allow(clippy::too_many_arguments)]
    #[instrument(skip(self, result_sender, control_sender, database))]
    fn resume_task(
        &mut self,
        requesting_task_id: TaskId,
        queued_task_id: TaskId,
        sender_permissions: Perms,
        return_value: Var,
        result_sender: oneshot::Sender<Var>,
        control_sender: &Sender<(TaskId, SchedulerControlMsg)>,
        database: Arc<dyn Database>,
    ) {
        // Task can't resume itself, it couldn't be queued. Builtin should not have sent this
        // request.
        if requesting_task_id == queued_task_id {
            error!(
                task = requesting_task_id,
                "Task requested to resume itself. Ignoring"
            );
            result_sender.send(v_err(E_INVARG)).ok();
            return;
        }

        let perms = match self.suspended.get(&queued_task_id) {
            Some(SuspendedTask {
                wake_condition: WakeCondition::Never,
                task,
                ..
            }) => task.perms,
            Some(SuspendedTask {
                wake_condition: WakeCondition::Time(_),
                task,
                ..
            }) => task.perms,
            _ => {
                result_sender.send(v_err(E_INVARG)).ok();
                return;
            }
        };
        // No permissions.
        if !sender_permissions
            .check_is_wizard()
            .expect("Could not check wizard status for resume request")
            && sender_permissions.who != perms
        {
            result_sender
                .send(v_err(E_PERM))
                .expect("Could not send resume result");
            return;
        }

        let sr = self.suspended.remove(&queued_task_id).unwrap();

        if self
            .resume_task_thread(
                sr.task,
                return_value,
                sr.session,
                sr.result_sender,
                control_sender,
                database,
            )
            .is_err()
        {
            error!(task = queued_task_id, "Could not resume task");
            result_sender.send(v_err(E_INVARG)).ok();
        }
    }

    #[instrument(skip(self))]
    fn disconnect_task(&mut self, disconnect_task_id: TaskId, player: Objid) {
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
        for (task_id, tc) in self.tasks.iter() {
            if *task_id == disconnect_task_id {
                continue;
            }
            if tc.player != player {
                continue;
            }
            warn!(
                ?player,
                task_id, "Aborting task from disconnected player..."
            );
            tc.kill_switch.store(true, Ordering::SeqCst);
        }
        // Likewise, suspended tasks.
        let to_remove = self
            .suspended
            .iter()
            .filter_map(|(task_id, sr)| {
                if sr.task.player != player {
                    return None;
                }
                Some(*task_id)
            })
            .collect::<Vec<_>>();
        for task_id in to_remove {
            self.suspended.remove(&task_id);
        }
    }
}
