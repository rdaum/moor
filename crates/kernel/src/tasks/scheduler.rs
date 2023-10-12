use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use bincode::{Decode, Encode};
use metrics_macros::{gauge, increment_counter};
use thiserror::Error;
use tokio::select;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::{oneshot, Mutex, RwLock};
use tracing::{debug, error, info, instrument, trace, warn};
use uuid::Uuid;

use moor_values::model::permissions::Perms;
use moor_values::model::world_state::WorldStateSource;
use moor_values::model::CommandError;
use moor_values::var::error::Error::{E_INVARG, E_PERM};
use moor_values::var::objid::Objid;
use moor_values::var::{v_err, v_int, v_none, v_string, Var};
use moor_values::SYSTEM_OBJECT;
use SchedulerError::{
    CommandExecutionError, CouldNotStartTask, EvalCompilationError, InputRequestNotFound,
    TaskAbortedCancelled, TaskAbortedError, TaskAbortedException, TaskAbortedLimit,
};

use crate::tasks::scheduler::SchedulerError::TaskNotFound;
use crate::tasks::sessions::Session;
use crate::tasks::task::Task;
use crate::tasks::task_messages::{SchedulerControlMsg, TaskControlMsg, TaskStart};
use crate::tasks::TaskId;
use crate::vm::vm_unwind::UncaughtException;
use crate::vm::Fork;
use moor_compiler::codegen::compile;
use moor_compiler::CompileError;

const SCHEDULER_TICK_TIME: Duration = Duration::from_millis(5);
const METRICS_POLLER_TICK_TIME: Duration = Duration::from_secs(5);

/// Responsible for the dispatching, control, and accounting of tasks in the system.
/// There should be only one scheduler per server.
#[derive(Clone)]
pub struct Scheduler {
    inner: Arc<RwLock<Inner>>,
    control_sender: UnboundedSender<(TaskId, SchedulerControlMsg)>,
    control_receiver: Arc<Mutex<UnboundedReceiver<(TaskId, SchedulerControlMsg)>>>,
}

struct Inner {
    running: bool,
    state_source: Arc<dyn WorldStateSource>,
    next_task_id: usize,
    tasks: HashMap<TaskId, TaskControl>,
    input_requests: HashMap<Uuid, TaskId>,
}

/// External interface description of a task, for purpose of e.g. the queued_tasks() builtin.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TaskDescription {
    pub task_id: TaskId,
    pub start_time: Option<SystemTime>,
    pub permissions: Objid,
    pub verb_name: String,
    pub verb_definer: Objid,
    pub line_number: usize,
    pub this: Objid,
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
    scheduler: Scheduler,
}

/// Scheduler-side per-task record. Lives in the scheduler thread and owned by the scheduler and
/// not shared elsewhere.
struct TaskControl {
    task_id: TaskId,
    player: Objid,
    /// Outbound mailbox for messages from the scheduler to the task.
    task_control_sender: UnboundedSender<TaskControlMsg>,
    state_source: Arc<dyn WorldStateSource>,
    session: Arc<dyn Session>,
    suspended: bool,
    waiting_input: Option<Uuid>,
    resume_time: Option<SystemTime>,
    // Self reference, used when forking tasks to pass them into the new task record. Not super
    // elegant and may need revisiting.
    scheduler: Scheduler,
    // One-shot subscribers for when the task is aborted, succeeded, etc.
    subscribers: Vec<oneshot::Sender<TaskWaiterResult>>,
}

#[derive(Debug, Error, Clone, Decode, Encode)]
pub enum SchedulerError {
    #[error("Task not found: {0:?}")]
    TaskNotFound(TaskId),
    #[error("Input request not found: {0:?}")]
    // TODO: using u128 here because Uuid is not bincode-able, but this is just a v4 uuid.
    InputRequestNotFound(u128),
    #[error("Could not start task (internal error)")]
    CouldNotStartTask,
    #[error("Eval compilation error")]
    EvalCompilationError(CompileError),
    #[error("Could not start command")]
    CommandExecutionError(CommandError),
    #[error("Task aborted due to limit: {0:?}")]
    TaskAbortedLimit(AbortLimitReason),
    #[error("Task aborted due to error.")]
    TaskAbortedError,
    #[error("Task aborted due to exception: {0:?}")]
    TaskAbortedException(UncaughtException),
    #[error("Task aborted due to cancellation.")]
    TaskAbortedCancelled,
}

/// Public facing interface for the scheduler.
impl Scheduler {
    pub fn new(state_source: Arc<dyn WorldStateSource>) -> Self {
        let (control_sender, control_receiver) = tokio::sync::mpsc::unbounded_channel();
        Self {
            inner: Arc::new(RwLock::new(Inner {
                running: Default::default(),
                state_source,
                next_task_id: Default::default(),
                tasks: HashMap::new(),
                input_requests: Default::default(),
            })),
            control_sender,
            control_receiver: Arc::new(Mutex::new(control_receiver)),
        }
    }

    pub async fn subscribe_to_task(
        &self,
        task_id: TaskId,
    ) -> Result<oneshot::Receiver<TaskWaiterResult>, SchedulerError> {
        let (sender, receiver) = oneshot::channel();
        let mut inner = self.inner.write().await;
        if let Some(task) = inner.tasks.get_mut(&task_id) {
            task.subscribers.push(sender);
        }
        Ok(receiver)
    }

    /// Execute the scheduler loop, run from the server process.
    pub async fn run(&self) {
        {
            let mut start_lock = self.inner.write().await;
            start_lock.running = true;
        }
        self.do_process().await;
        {
            let mut finish_lock = self.inner.write().await;
            finish_lock.running = false;
        }
        info!("Scheduler done.");
    }

    /// Submit a command to the scheduler for execution.
    #[instrument(skip(self, session))]
    pub async fn submit_command_task(
        &self,
        player: Objid,
        command: &str,
        session: Arc<dyn Session>,
    ) -> Result<TaskId, SchedulerError> {
        increment_counter!("scheduler.submit_command_task");

        let task_start = TaskStart::StartCommandVerb {
            player,
            command: command.to_string(),
        };

        let mut inner = self.inner.write().await;
        let task_id = inner
            .new_task(
                task_start,
                player,
                session,
                None,
                self.clone(),
                player,
                false,
            )
            .await?;

        Ok(task_id)
    }

    /// Receive input that the (suspended) task previously requested, using the given
    /// `input_request_id`.
    /// The request is identified by the `input_request_id`, and given the input and resumed under
    /// a new transaction.
    pub async fn submit_requested_input(
        &self,
        player: Objid,
        input_request_id: Uuid,
        input: String,
    ) -> Result<(), SchedulerError> {
        // Validate that the given input request is valid, and if so, resume the task, sending it
        // the given input, clearing the input request out.
        let mut inner = self.inner.write().await;

        let Some(task_id) = inner.input_requests.get(&input_request_id).cloned() else {
            return Err(InputRequestNotFound(input_request_id.as_u128()));
        };

        let Some(task) = inner.tasks.get_mut(&task_id) else {
            warn!(?task_id, ?input_request_id, "Input received for dead task");
            return Err(TaskNotFound(task_id));
        };

        // If the player doesn't match, we'll pretend we didn't even see it.
        if task.player != player {
            warn!(
                ?task_id,
                ?input_request_id,
                ?player,
                "Task input request received for wrong player"
            );
            return Err(TaskNotFound(task_id));
        }

        // Now we can resume the task with the given input
        let world_state = task.state_source.new_world_state().await.map_err(|e| {
            error!(
                ?e,
                ?task_id,
                ?input_request_id,
                ?player,
                "Could not create new world state when resuming task for player input"
            );
            CouldNotStartTask
        })?;
        task.task_control_sender
            .send(TaskControlMsg::ResumeReceiveInput(world_state, input))
            .map_err(|_| CouldNotStartTask)?;
        task.waiting_input = None;
        inner.input_requests.remove(&input_request_id);

        Ok(())
    }

    /// Submit a verb task to the scheduler for execution.
    /// (This path is really only used for the invocations from the serving processes like login,
    /// user_connected, or the do_command invocation which precedes an internal parser attempt.)
    #[instrument(skip(self, session))]
    pub async fn submit_verb_task(
        &self,
        player: Objid,
        vloc: Objid,
        verb: String,
        args: Vec<Var>,
        argstr: String,
        perms: Objid,
        session: Arc<dyn Session>,
    ) -> Result<TaskId, SchedulerError> {
        increment_counter!("scheduler.submit_verb_task");

        let mut inner = self.inner.write().await;

        let task_start = TaskStart::StartVerb {
            player,
            vloc,
            verb,
            args,
            argstr,
        };

        let task_id = inner
            .new_task(
                task_start,
                player,
                session,
                None,
                self.clone(),
                perms,
                false,
            )
            .await?;

        Ok(task_id)
    }

    #[instrument(skip(self, session))]
    pub async fn submit_out_of_band_task(
        &self,
        player: Objid,
        command: Vec<String>,
        argstr: String,
        session: Arc<dyn Session>,
    ) -> Result<TaskId, SchedulerError> {
        increment_counter!("scheduler.submit_out_of_band_task");

        let args = command.into_iter().map(v_string).collect::<Vec<Var>>();
        let task_start = TaskStart::StartVerb {
            player,
            vloc: SYSTEM_OBJECT,
            verb: "do_out_of_band_command".to_string(),
            args,
            argstr,
        };

        let mut inner = self.inner.write().await;

        let task_id = inner
            .new_task(
                task_start,
                player,
                session,
                None,
                self.clone(),
                player,
                false,
            )
            .await?;

        Ok(task_id)
    }

    /// Submit an eval task to the scheduler for execution.
    #[instrument(skip(self, sessions))]
    pub async fn submit_eval_task(
        &self,
        player: Objid,
        perms: Objid,
        code: String,
        sessions: Arc<dyn Session>,
    ) -> Result<TaskId, SchedulerError> {
        increment_counter!("scheduler.submit_eval_task");

        let mut inner = self.inner.write().await;

        // Compile the text into a verb.
        let binary = match compile(code.as_str()) {
            Ok(b) => b,
            Err(e) => return Err(EvalCompilationError(e)),
        };

        let task_start = TaskStart::StartEval {
            player,
            program: binary,
        };

        let task_id = inner
            .new_task(
                task_start,
                player,
                sessions,
                None,
                self.clone(),
                perms,
                false,
            )
            .await?;

        Ok(task_id)
    }

    pub async fn abort_player_tasks(&self, player: Objid) -> Result<(), SchedulerError> {
        let mut inner = self.inner.write().await;
        let mut to_abort = Vec::new();
        for (task_id, task_ref) in inner.tasks.iter() {
            if task_ref.player == player {
                to_abort.push(*task_id);
            }
        }
        for task_id in to_abort {
            if let Err(e) = inner
                .tasks
                .get_mut(&task_id)
                .expect("Corrupt task list")
                .task_control_sender
                .send(TaskControlMsg::Abort)
            {
                // TODO Unclear what to do here, because we really should be letting the main
                //   scheduler loop know it has to murder this. Though it's likely to find that out
                //   in the long run anyways...
                warn!(task_id, error = ?e, "Could not send abort for task. Dead?");
                continue;
            }
        }

        Ok(())
    }

    /// Request information on all tasks known to the scheduler.
    pub async fn tasks(&self) -> Result<Vec<TaskDescription>, SchedulerError> {
        let inner = self.inner.read().await;
        let mut tasks = Vec::new();
        for (task_id, task) in inner.tasks.iter() {
            trace!(task_id, "Requesting task description");
            let (t_send, t_reply) = oneshot::channel();
            if let Err(e) = task
                .task_control_sender
                .send(TaskControlMsg::Describe(t_send))
            {
                // TODO: again, we probably want to prune here, and signal back to the scheduler
                //   to do so.  Or a generic liveness check collects these.
                warn!(task_id, error = ?e, "Could not request task description for task. Dead?");
                continue;
            }
            let Ok(task_desc) = t_reply.await else {
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
    pub async fn stop(&self) -> Result<(), SchedulerError> {
        let mut scheduler = self.inner.write().await;
        // Send shut down to all the tasks.
        for task in scheduler.tasks.values() {
            if let Err(e) = task.task_control_sender.send(TaskControlMsg::Abort) {
                warn!(task_id = task.task_id, error = ?e, "Could not send abort for task. Already dead?");
                continue;
            }
        }
        // Then spin until they're all done.
        while !scheduler.tasks.is_empty() {}
        scheduler.running = false;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn do_process(&self) {
        let mut metrics_poller_interval = tokio::time::interval(METRICS_POLLER_TICK_TIME);
        let mut task_poller_interval = tokio::time::interval(SCHEDULER_TICK_TIME);

        // control receiver is on its own lock so we don't have to hold 'inner' while we wait for
        // messages, but is still Arc< so the scheduler handle can be cloned.
        let mut receiver = self.control_receiver.lock().await;

        loop {
            select! {
                // Track active tasks in metrics.
                _ = metrics_poller_interval.tick() => {
                    let inner = self.inner.read().await;
                    let mut number_suspended_tasks = 0;
                    let mut number_readblocked_tasks = 0;
                    let number_tasks =  inner.tasks.len();

                    for (_, task) in &inner.tasks {
                        if task.suspended {
                            number_suspended_tasks += 1;
                        }
                        if task.waiting_input.is_some() {
                            number_readblocked_tasks += 1;
                        }
                    }
                    gauge!("scheduler.tasks", number_tasks as f64);
                    gauge!("scheduler.suspended_tasks", number_suspended_tasks as f64);
                    debug!(number_readblocked_tasks, number_tasks, number_suspended_tasks, "...");
                }
                // Look for tasks that need to be woken (have hit their wakeup-time), and wake them.
                // TODO: we might be able to use a vector of delay-futures for this instead, and just poll
                //  those using some futures_util magic.
                _ = task_poller_interval.tick() => {
                    let mut inner = self.inner.write().await;
                    let mut to_wake = Vec::new();
                    for (task_id, task) in &inner.tasks {
                        if task.suspended {
                            if let Some(delay) = task.resume_time {
                                if delay <= SystemTime::now() {
                                    to_wake.push(*task_id);
                                }
                            }
                        }
                    }
                    inner.process_wake_ups(to_wake).await;
                }
                // Receive messages from any tasks that have sent us messages.
                msg = receiver.recv() => {
                    match msg {
                        Some((task_id, msg)) => {
                            self.handle_task_control_msg(task_id, msg).await;
                        },
                        None => {
                            error!("Scheduler control channel closed");
                            return;
                        }
                    }
                }
            }
        }
    }

    /// Handle task-origin'd scheduler control messages.
    async fn handle_task_control_msg(&self, task_id: TaskId, msg: SchedulerControlMsg) {
        // The following are queues of work items to process rather than doing them inline.
        // TODO: This may be overkill at this point as it is a product of the old scheduler where
        //  these messages were processed from multiple tasks, in a loop.  Now they're processed
        //  from a single task, so we could probably most of this inline.
        let mut to_notify = Vec::new();
        let mut to_remove = Vec::new();
        let mut fork_requests = Vec::new();
        let mut desc_requests = Vec::new();
        let mut kill_requests = Vec::new();
        let mut resume_requests = Vec::new();
        let mut to_disconnect = Vec::new();
        let mut to_retry = Vec::new();

        match msg {
            SchedulerControlMsg::TaskSuccess(value) => {
                increment_counter!("scheduler.task_succeeded");
                debug!(?task_id, result = ?value, "Task succeeded");
                to_notify.push((task_id, TaskWaiterResult::Success(value)));
                to_remove.push(task_id);
            }
            SchedulerControlMsg::TaskConflictRetry => {
                increment_counter!("scheduler.task_conflict_retry");
                debug!(?task_id, "Task retrying due to conflict");

                // Ask the task to restart itself, using its stashed original start info, but with
                // a brand new transaction.
                to_retry.push(task_id);
            }
            SchedulerControlMsg::TaskVerbNotFound(this, verb) => {
                increment_counter!("scheduler.verb_not_found");

                // I'd make this 'warn' but `do_command` gets invoked for every command and
                // many cores don't have it at all. So it would just be way too spammy.
                debug!(this = ?this, verb, ?task_id, "Verb not found, task cancelled");

                to_notify.push((task_id, TaskWaiterResult::Error(TaskAbortedError)));
                to_remove.push(task_id);
            }
            SchedulerControlMsg::TaskCommandError(parse_command_error) => {
                increment_counter!("scheduler.command_error");

                // This is a common occurrence, so we don't want to log it at warn level.
                trace!(?task_id, error = ?parse_command_error, "command parse error");
                to_notify.push((
                    task_id,
                    TaskWaiterResult::Error(CommandExecutionError(parse_command_error)),
                ));
                to_remove.push(task_id);
            }
            SchedulerControlMsg::TaskAbortCancelled => {
                increment_counter!("scheduler.aborted_cancelled");

                warn!(?task_id, "Task cancelled");

                to_notify.push((task_id, TaskWaiterResult::Error(TaskAbortedCancelled)));
                to_remove.push(task_id);
            }
            SchedulerControlMsg::TaskAbortLimitsReached(limit_reason) => {
                match limit_reason {
                    AbortLimitReason::Ticks(t) => {
                        increment_counter!("scheduler.aborted_ticks");
                        warn!(?task_id, ticks = t, "Task aborted, ticks exceeded");
                    }
                    AbortLimitReason::Time(t) => {
                        increment_counter!("scheduler.aborted_time");
                        warn!(?task_id, time = ?t, "Task aborted, time exceeded");
                    }
                }
                increment_counter!("scheduler.aborted_limits");
                to_notify.push((
                    task_id,
                    TaskWaiterResult::Error(TaskAbortedLimit(limit_reason)),
                ));
                to_remove.push(task_id);
            }
            SchedulerControlMsg::TaskException(exception) => {
                increment_counter!("scheduler.task_exception");

                warn!(?task_id, finally_reason = ?exception, "Task threw exception");
                to_notify.push((
                    task_id,
                    TaskWaiterResult::Error(TaskAbortedException(exception)),
                ));
                to_remove.push(task_id);
            }
            SchedulerControlMsg::TaskRequestFork(fork_request, reply) => {
                trace!(?task_id,  delay=?fork_request.delay, "Task requesting fork");
                increment_counter!("scheduler.fork_task");
                // Task has requested a fork. Dispatch it and reply with the new task id.
                // Gotta dump this out til we exit the loop tho, since self.tasks is already
                // borrowed here.
                let mut inner = self.inner.write().await;
                let Some(task) = inner.tasks.get_mut(&task_id) else {
                    warn!(task_id, "Task not found for fork request");
                    return;
                };
                fork_requests.push(ForkRequest {
                    fork_request,
                    reply,
                    session: task.session.clone(),
                    scheduler: task.scheduler.clone(),
                });
            }
            SchedulerControlMsg::TaskSuspend(resume_time) => {
                increment_counter!("scheduler.suspend_task");
                // Task is suspended. The resume time (if any) is the system time at which
                // the scheduler should try to wake us up.
                let mut inner = self.inner.write().await;
                let Some(task) = inner.tasks.get_mut(&task_id) else {
                    warn!(task_id, "Task not found for suspend request");
                    return;
                };
                task.suspended = true;
                task.resume_time = resume_time;
            }
            SchedulerControlMsg::TaskRequestInput => {
                increment_counter!("scheduler.request_input");
                // Task has gone into suspension waiting for input from the client.
                // Create a unique ID for this request, and we'll wake the task when the
                // session receives input.
                let mut inner = self.inner.write().await;
                let input_request_id = Uuid::new_v4();
                {
                    let Some(task) = inner.tasks.get_mut(&task_id) else {
                        warn!(task_id, "Task not found for input request");
                        return;
                    };
                    let Ok(()) = task
                        .session
                        .request_input(task.player, input_request_id)
                        .await
                    else {
                        warn!("Could not request input from session; aborting task");
                        to_notify.push((task_id, TaskWaiterResult::Error(TaskAbortedError)));
                        return;
                    };
                    task.waiting_input = Some(input_request_id);
                }
                inner.input_requests.insert(input_request_id, task_id);
                debug!(?task_id, "Task suspended waiting for input");
                return;
            }
            SchedulerControlMsg::DescribeOtherTasks(reply) => {
                increment_counter!("scheduler.describe_tasks");
                // Task is asking for a description of all other tasks.
                desc_requests.push((task_id, reply));
            }
            SchedulerControlMsg::KillTask {
                victim_task_id,
                sender_permissions,
                result_sender,
            } => {
                increment_counter!("scheduler.kill_task");
                // Task is asking to kill another task.
                kill_requests.push(KillRequest {
                    requesting_task_id: task_id,
                    victim_task_id,
                    sender_permissions,
                    result_sender,
                });
            }
            SchedulerControlMsg::ResumeTask {
                queued_task_id,
                sender_permissions,
                return_value,
                result_sender,
            } => {
                increment_counter!("scheduler.resume_task");
                resume_requests.push(ResumeRequest {
                    requesting_task_id: task_id,
                    queued_task_id,
                    sender_permissions,
                    return_value,
                    result_sender,
                });
            }
            SchedulerControlMsg::BootPlayer {
                player,
                sender_permissions: _,
            } => {
                increment_counter!("scheduler.boot_player");
                // Task is asking to boot a player.
                to_disconnect.push((task_id, player));
            }
        }

        // Now apply task mutation actions that shook out of this.
        let mut inner = self.inner.write().await;

        // Send notifications. These are oneshot and consumed.
        to_remove.append(&mut inner.process_notifications(to_notify).await);

        // Service fork requests
        to_remove.append(&mut inner.process_fork_requests(fork_requests).await);

        // Service describe requests.
        to_remove.append(&mut inner.process_describe_requests(desc_requests).await);

        // Service kill requests, removing any that were non-responsive (returned from function)
        to_remove.append(&mut inner.process_kill_requests(kill_requests).await);

        // Service resume requests, removing any that were non-responsive (returned from function)
        to_remove.append(&mut inner.process_resume_requests(resume_requests).await);

        // Service retry requests, removing any that were non-responsive (returned from function)
        to_remove.append(&mut inner.process_retry_requests(to_retry).await);

        inner.process_disconnect_tasks(to_disconnect).await;

        // Prune any completed/dead tasks
        inner.prune_dead_tasks();

        // Service task removals. This is done last because other queues above might contributed to
        // this list.
        inner.process_task_removals(to_remove);
    }
}

impl Inner {
    async fn submit_fork_task(
        &mut self,
        fork: Fork,
        session: Arc<dyn Session>,
        scheduler_ref: Scheduler,
    ) -> Result<TaskId, SchedulerError> {
        increment_counter!("scheduler.forked_tasks");

        let suspended = fork.delay.is_some();

        let player = fork.player;
        let delay = fork.delay;
        let progr = fork.progr;
        let task_id = self
            .new_task(
                TaskStart::StartFork {
                    fork_request: fork,
                    suspended,
                },
                player,
                session,
                delay,
                scheduler_ref,
                progr,
                false,
            )
            .await?;

        let Some(task_ref) = self.tasks.get_mut(&task_id) else {
            return Err(TaskNotFound(task_id));
        };

        // If there's a delay on the fork, we will mark it in suspended state and put in the
        // delay time.
        if let Some(delay) = delay {
            task_ref.suspended = true;
            task_ref.resume_time = Some(SystemTime::now() + delay);
        }

        increment_counter!("scheduler.forked_tasks");

        Ok(task_id)
    }

    async fn process_notifications(
        &mut self,
        to_notify: Vec<(TaskId, TaskWaiterResult)>,
    ) -> Vec<TaskId> {
        let mut to_remove = vec![];

        for (task_id, result) in to_notify {
            let Some(task) = self.tasks.get_mut(&task_id) else {
                // Missing task, must have ended already. This is odd though? So we'll warn.
                warn!(task_id, "Task not found for notification, ignoring");
                continue;
            };
            for subscriber in task.subscribers.drain(..) {
                if subscriber.send(result.clone()).is_err() {
                    to_remove.push(task_id);
                    error!("Notify to subscriber on task {} failed", task_id);
                }
            }
        }
        to_remove
    }

    async fn process_wake_ups(&mut self, to_wake: Vec<TaskId>) -> Vec<TaskId> {
        let mut to_remove = vec![];

        for task_id in to_wake {
            let task = self.tasks.get_mut(&task_id).unwrap();
            task.suspended = false;

            let world_state = self
                .state_source
                .new_world_state()
                .await
                // This is a rather drastic system problem if it happens, and it's best to just die.
                .expect("Unable to start transaction for resumed task. Panic.");

            if let Err(e) = task
                .task_control_sender
                .send(TaskControlMsg::Resume(world_state, v_int(0)))
            {
                error!(?task_id, error = ?e, "Could not send message resume task. Task being removed.");
                to_remove.push(task.task_id);
            }
        }
        to_remove
    }

    async fn process_fork_requests(&mut self, fork_requests: Vec<ForkRequest>) -> Vec<TaskId> {
        let mut to_remove = vec![];
        for ForkRequest {
            fork_request,
            reply,
            session,
            scheduler,
        } in fork_requests
        {
            // Fork the session.
            let forked_session = session.clone();
            let task_id = self
                .submit_fork_task(fork_request, forked_session, scheduler)
                .await
                .unwrap_or_else(|e| panic!("Could not fork task: {:?}", e));
            if let Err(e) = reply.send(task_id) {
                error!(task = task_id, error = ?e, "Could not send fork reply. Parent task gone?  Remove.");
                to_remove.push(task_id);
            }
        }
        to_remove
    }
    async fn process_describe_requests(
        &mut self,
        desc_requests: Vec<(TaskId, oneshot::Sender<Vec<TaskDescription>>)>,
    ) -> Vec<TaskId> {
        let mut to_remove = vec![];

        // Note these could be done in parallel and joined instead of single file, to avoid blocking
        // the loop on one uncooperative thread, and could be done in a separate thread as well?
        // The challenge being the borrow semantics of the 'tasks' list.
        // And we should have a timeout here to boot.
        // For now, just iterate blocking.
        for (requesting_task_id, reply) in desc_requests {
            let mut tasks = Vec::new();
            trace!(
                task = requesting_task_id,
                "Task requesting task descriptions"
            );
            for (task_id, task) in self.tasks.iter() {
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
                    if let Err(e) = task
                        .task_control_sender
                        .send(TaskControlMsg::Describe(t_send))
                    {
                        error!(?task_id, error = ?e,
                            "Could not send describe request to task. Task being removed.");
                        to_remove.push(task.task_id);
                        continue;
                    }
                    let Ok(task_desc) = t_reply.await else {
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
        }
        to_remove
    }

    async fn process_kill_requests(&mut self, kill_requests: Vec<KillRequest>) -> Vec<TaskId> {
        let mut to_remove = vec![];
        // Service kill requests
        for KillRequest {
            requesting_task_id,
            victim_task_id,
            sender_permissions,
            result_sender,
        } in kill_requests
        {
            // If the task somehow is requesting a kill on itself, that would lead to deadlock,
            // because we could never send the result back. So we reject that outright. bf_kill_task
            // should be handling this upfront.
            if requesting_task_id == victim_task_id {
                error!(
                    task = requesting_task_id,
                    "Task requested to kill itself. Ignoring"
                );
                continue;
            }

            let victim_task = match self.tasks.get(&victim_task_id) {
                Some(victim_task) => victim_task,
                None => {
                    result_sender
                        .send(v_err(E_INVARG))
                        .expect("Could not send kill result");
                    continue;
                }
            };

            // We reject this outright if the sender permissions are not sufficient:
            //   The either have to be the owner of the task (task.programmer == sender_permissions.task_perms)
            //   Or they have to be a wizard.
            // TODO: Will have to verify that it's enough that .player on task control can
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
                continue;
            }

            if let Err(e) = victim_task.task_control_sender.send(TaskControlMsg::Abort) {
                error!(task = victim_task_id, error = ?e, "Could not send kill request to task. Task being removed.");
                to_remove.push(victim_task_id);
            }

            if let Err(e) = result_sender.send(v_none()) {
                error!(task = requesting_task_id, error = ?e, "Could not send kill result to requesting task. Requesting task being removed.");
            }
        }
        to_remove
    }

    async fn process_resume_requests(
        &mut self,
        resume_requests: Vec<ResumeRequest>,
    ) -> Vec<TaskId> {
        let mut to_remove = vec![];

        // Service resume requests
        for ResumeRequest {
            requesting_task_id,
            queued_task_id,
            sender_permissions,
            return_value,
            result_sender,
        } in resume_requests
        {
            // Task can't resume itself, it couldn't be queued. Builtin should not have sent this
            // request.
            if requesting_task_id == queued_task_id {
                error!(
                    task = requesting_task_id,
                    "Task requested to resume itself. Ignoring"
                );
                continue;
            }

            // Task does not exist.
            let queued_task = match self.tasks.get_mut(&queued_task_id) {
                Some(queued_task) => queued_task,
                None => {
                    result_sender
                        .send(v_err(E_INVARG))
                        .expect("Could not send resume result");
                    continue;
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
                continue;
            }
            // Task is not suspended.
            if !queued_task.suspended {
                result_sender
                    .send(v_err(E_INVARG))
                    .expect("Could not send resume result");
                continue;
            }

            // Follow the usual task resume logic.
            let world_state = self
                .state_source
                .new_world_state()
                .await
                .expect("Could not start transaction for resumed task. Panic.");

            queued_task.suspended = false;
            if let Err(e) = queued_task
                .task_control_sender
                .send(TaskControlMsg::Resume(world_state, return_value))
            {
                error!(task = queued_task_id, error = ?e,
                    "Could not send resume request to task. Task being removed.");
                to_remove.push(queued_task_id);
            }

            if let Err(e) = result_sender.send(v_none()) {
                error!(task = requesting_task_id, error = ?e,
                    "Could not send resume result to requesting task. Requesting task being removed.");
                to_remove.push(requesting_task_id);
            }
        }
        to_remove
    }

    async fn process_retry_requests(&mut self, to_retry: Vec<TaskId>) -> Vec<TaskId> {
        let mut to_remove = vec![];
        for task_id in to_retry {
            let Some(task) = self.tasks.get_mut(&task_id) else {
                warn!(task = task_id, "Retrying task not found");
                continue;
            };

            // Create a new transaction.
            let world_state = self
                .state_source
                .new_world_state()
                .await
                .expect("Could not start transaction for resumed task. Panic.");

            task.suspended = false;
            if let Err(e) = task
                .task_control_sender
                .send(TaskControlMsg::Restart(world_state))
            {
                error!(task = task_id, error = ?e,
                    "Could not send resume request to task. Task being removed.");
                to_remove.push(task_id);
            }
        }
        to_remove
    }
    async fn process_disconnect_tasks(&mut self, to_disconnect: Vec<(TaskId, Objid)>) {
        for (disconnect_task_id, player) in to_disconnect {
            {
                let Some(task) = self.tasks.get_mut(&disconnect_task_id) else {
                    warn!(task = disconnect_task_id, "Disconnecting task not found");
                    continue;
                };
                // First disconnect the player...
                warn!(?player, ?disconnect_task_id, "Disconnecting player");
                if let Err(e) = task.session.disconnect(player).await {
                    warn!(?player, ?disconnect_task_id, error = ?e, "Could not disconnect player's session");
                    continue;
                }
            }

            // Then abort all of their still-living forked tasks (that weren't the disconnect
            // task, we need to let that run to completion for sanity's sake.)
            for (task_id, task) in self.tasks.iter() {
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
                let Ok(_) = task.task_control_sender.send(TaskControlMsg::Abort) else {
                    trace!(?player, task_id, "Task already dead");
                    continue;
                };
            }
        }
    }

    fn prune_dead_tasks(&mut self) {
        let dead_tasks: Vec<_> = self
            .tasks
            .iter()
            .filter_map(|(task_id, task)| task.task_control_sender.is_closed().then_some(*task_id))
            .collect();
        for task in dead_tasks {
            self.tasks.remove(&task);
        }
    }
    fn process_task_removals(&mut self, to_remove: Vec<TaskId>) {
        for task_id in to_remove {
            trace!(task = task_id, "Task removed");
            self.tasks.remove(&task_id);
        }
    }

    async fn new_task(
        &mut self,
        task_start: TaskStart,
        player: Objid,
        session: Arc<dyn Session>,
        delay_start: Option<Duration>,
        scheduler_ref: Scheduler,
        perms: Objid,
        is_background: bool,
    ) -> Result<TaskId, SchedulerError> {
        increment_counter!("scheduler.new_task");

        let task_id = self.next_task_id;
        self.next_task_id += 1;
        let (task_control_sender, task_control_receiver) = tokio::sync::mpsc::unbounded_channel();

        let state_source = self.state_source.clone();
        let task_control = TaskControl {
            task_id,
            player,
            task_control_sender,
            state_source: state_source.clone(),
            session: session.clone(),
            suspended: false,
            waiting_input: None,
            resume_time: None,
            scheduler: scheduler_ref.clone(),
            subscribers: vec![],
        };
        self.tasks.insert(task_id, task_control);

        // TODO: support a queue-size on concurrent executing tasks and allow them to sit in an
        //   initially suspended state without spawning a worker thread, until the queue has space.
        // Spawn the task's thread.
        let state_source = self.state_source.clone();
        tokio::spawn(async move {
            debug!(?task_id, ?task_start, "Starting up task");
            Task::run(
                task_id,
                task_start,
                player,
                perms,
                delay_start,
                state_source,
                is_background,
                session.clone(),
                task_control_receiver,
                scheduler_ref.control_sender.clone(),
            )
            .await;
            debug!(?task_id, "Completed task");
        });

        increment_counter!("scheduler.created_tasks");
        gauge!("scheduler.active_tasks", self.tasks.len() as f64);

        Ok(task_id)
    }

    #[instrument(skip(self))]
    pub async fn abort_task(&mut self, id: TaskId) -> Result<(), SchedulerError> {
        let task = self.tasks.get_mut(&id).ok_or(TaskNotFound(id))?;
        if let Err(e) = task.task_control_sender.send(TaskControlMsg::Abort) {
            error!(error = ?e, "Could not send abort message to task on its channel.  Already dead?");
        }
        Ok(())
    }

    #[instrument(skip(self))]
    async fn remove_task(&mut self, id: TaskId) -> Result<(), SchedulerError> {
        self.tasks.remove(&id).ok_or(TaskNotFound(id))?;
        Ok(())
    }
}
