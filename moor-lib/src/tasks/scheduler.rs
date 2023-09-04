use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, Error};
use metrics_macros::{gauge, increment_counter};
use thiserror::Error;
use tokio::select;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::{oneshot, RwLock};
use tracing::{debug, error, info, instrument, span, trace, warn, Level};

use moor_value::var::error::Error::{E_INVARG, E_PERM};
use moor_value::var::objid::Objid;

use moor_value::var::{v_err, v_int, v_none, Var};

use crate::compiler::codegen::compile;
use crate::db::match_env::DBMatchEnvironment;
use crate::db::matching::MatchEnvironmentParseMatcher;
use crate::tasks::command_parse::{parse_command, ParsedCommand};
use crate::tasks::task::{Task, TaskControlMsg};
use crate::tasks::{Sessions, TaskId};
use crate::vm::vm_unwind::FinallyReason;
use crate::vm::{ForkRequest, VM};

use crate::tasks::scheduler::SchedulerError::{CouldNotParseCommand, DatabaseError, TaskNotFound};
use moor_value::model::permissions::Perms;
use moor_value::model::world_state::{WorldState, WorldStateSource};
use moor_value::model::WorldStateError;

const SCHEDULER_TICK_TIME: Duration = Duration::from_millis(5);
const METRICS_POLLER_TICK_TIME: Duration = Duration::from_secs(1);

// TODO allow these to be set by command line arguments, as well.
// Note these can be overriden in-core.
const DEFAULT_FG_TICKS: usize = 30_000;
const DEFAULT_BG_TICKS: usize = 15_000;
const DEFAULT_FG_SECONDS: u64 = 5;
const DEFAULT_BG_SECONDS: u64 = 3;
const DEFAULT_MAX_STACK_DEPTH: usize = 50;

/// Responsible for the dispatching, control, and accounting of tasks in the system.
/// There should be only one scheduler per server.
#[derive(Clone)]
pub struct Scheduler {
    inner: Arc<RwLock<Inner>>,
}

pub struct Inner {
    running: bool,
    state_source: Arc<RwLock<dyn WorldStateSource + Send + Sync>>,
    next_task_id: usize,
    tasks: HashMap<TaskId, TaskControl>,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AbortLimitReason {
    Ticks(usize),
    Time(Duration),
}

/// The messages that can be sent from tasks (or VM) to the scheduler.
pub enum SchedulerControlMsg {
    TaskSuccess(Var),
    TaskException(FinallyReason),
    TaskAbortError(Error),
    TaskRequestFork(ForkRequest, oneshot::Sender<TaskId>),
    TaskAbortCancelled,
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
    ResumeTask {
        queued_task_id: TaskId,
        sender_permissions: Perms,
        return_value: Var,
        result_sender: oneshot::Sender<Var>,
    },
    BootPlayer {
        player: Objid,
        sender_permissions: Perms,
    },
}

// A subset of the messages above, for use by subscribers on tasks (e.g. the websocket connection)
// TODO consider consolidation here
#[derive(Clone)]
pub enum TaskWaiterResult {
    Success(Var),
    Exception(FinallyReason),
    AbortTimeout(AbortLimitReason),
    AbortCancelled,
    AbortError,
}

/// Scheduler-side per-task record. Lives in the scheduler thread and owned by the scheduler and
/// not shared elsewhere.
struct TaskControl {
    task_id: TaskId,
    player: Objid,
    /// Outbound mailbox for messages from the scheduler to the task.
    task_control_sender: UnboundedSender<TaskControlMsg>,
    /// (Per-task) receiver for messages from the task to the scheduler.
    scheduler_control_receiver: UnboundedReceiver<SchedulerControlMsg>,
    state_source: Arc<RwLock<dyn WorldStateSource + Send + Sync>>,
    sessions: Arc<RwLock<dyn Sessions>>,
    suspended: bool,
    resume_time: Option<SystemTime>,
    // Self reference, used when forking tasks to pass them into the new task record. Not super
    // elegant and may need revisiting.
    scheduler: Scheduler,
    // One-shot subscribers for when the task is aborted, succeeded, etc.
    subscribers: Vec<oneshot::Sender<TaskWaiterResult>>,
}

#[derive(Debug, Error)]
pub enum SchedulerError {
    #[error("Could not parse command: {0}")]
    CouldNotParseCommand(Error),
    #[error("Could not find match for command '{0}': {1:?}")]
    NoCommandMatch(String, ParsedCommand),
    #[error("Could not start transaction due to database error: {0}")]
    DatabaseError(WorldStateError),
    #[error("Permission denied")]
    PermissionDenied,
    #[error("Task not found: {0:?}")]
    TaskNotFound(TaskId),
    #[error("Could not start task: {0}")]
    CouldNotStartTask(Error),
}

// TODO cache
async fn max_vm_values(_ws: &mut dyn WorldState, is_background: bool) -> (usize, u64, usize) {
    let (max_ticks, max_seconds, max_stack_depth) = if is_background {
        (
            DEFAULT_BG_TICKS,
            DEFAULT_BG_SECONDS,
            DEFAULT_MAX_STACK_DEPTH,
        )
    } else {
        (
            DEFAULT_FG_TICKS,
            DEFAULT_FG_SECONDS,
            DEFAULT_MAX_STACK_DEPTH,
        )
    };

    // TODO: revisit this -- we need a way to look up and cache these without having to fake wizard
    //   permissions to get them, which probably means not going through worldstate. I don't want to
    //   have to guess what $wizard is, and some cores may not have this even defined.
    //   I think the scheduler will need a handle on some access to the DB that bypasses perms?

    //
    // // Look up fg_ticks, fg_seconds, and max_stack_depth on $server_options.
    // // These are optional properties, and if they are not set, we use the defaults.
    // let wizperms = PermissionsContext::root_for(Objid(2), BitEnum::new_with(ObjFlag::Wizard));
    // if let Ok(server_options) = ws
    //     .retrieve_property(wizperms.clone(), Objid(0), "server_options")
    //     .await
    // {
    //     if let Variant::Obj(server_options) = server_options.variant() {
    //         if let Ok(v) = ws
    //             .retrieve_property(wizperms.clone(), *server_options, "fg_ticks")
    //             .await
    //         {
    //             if let Variant::Int(v) = v.variant() {
    //                 max_ticks = *v as usize;
    //             }
    //         }
    //         if let Ok(v) = ws
    //             .retrieve_property(wizperms.clone(), *server_options, "fg_seconds")
    //             .await
    //         {
    //             if let Variant::Int(v) = v.variant() {
    //                 max_seconds = *v as u64;
    //             }
    //         }
    //         if let Ok(v) = ws
    //             .retrieve_property(wizperms, *server_options, "max_stack_depth")
    //             .await
    //         {
    //             if let Variant::Int(v) = v.variant() {
    //                 max_stack_depth = *v as usize;
    //             }
    //         }
    //     }
    // }
    (max_ticks, max_seconds, max_stack_depth)
}

/// Public facing interface for the scheduler.
impl Scheduler {
    pub fn new(state_source: Arc<RwLock<dyn WorldStateSource + Sync + Send>>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(Inner {
                running: Default::default(),
                state_source,
                next_task_id: Default::default(),
                tasks: HashMap::new(),
            })),
        }
    }

    pub async fn subscribe_to_task(
        &self,
        task_id: TaskId,
    ) -> Result<oneshot::Receiver<TaskWaiterResult>, anyhow::Error> {
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
        let mut scheduler_interval = tokio::time::interval(SCHEDULER_TICK_TIME);
        let mut metrics_poller_interval = tokio::time::interval(METRICS_POLLER_TICK_TIME);
        loop {
            {
                select! {
                    _ = metrics_poller_interval.tick() => {
                        let inner = self.inner.read().await;
                        gauge!("scheduler.tasks", inner.tasks.len() as f64);
                    }
                    _ = scheduler_interval.tick() => {
                        let mut inner = self.inner.write().await;
                        if !inner.running {
                            break;
                        }
                        if let Err(e) = inner.do_process().await {
                            error!(error = ?e, "Error processing scheduler loop");
                        }
                    }
                }
            }
        }
        {
            let mut start_lock = self.inner.write().await;
            start_lock.running = false;
        }
        info!("Scheduler done.");
    }

    /// Submit a command to the scheduler for execution.
    #[instrument(skip(self, sessions))]
    pub async fn submit_command_task(
        &self,
        player: Objid,
        command: &str,
        sessions: Arc<RwLock<dyn Sessions>>,
    ) -> Result<TaskId, SchedulerError> {
        increment_counter!("scheduler.submit_command_task");

        let mut inner = self.inner.write().await;

        let (vloc, vi, command) = {
            let mut ss = inner.state_source.write().await;
            let mut ws = ss.new_world_state().await.map_err(|e| DatabaseError(e))?;

            // Get perms for environment search. Player's perms.
            let me = DBMatchEnvironment {
                ws: ws.as_mut(),
                perms: player,
            };
            let matcher = MatchEnvironmentParseMatcher { env: me, player };
            let pc = parse_command(command, matcher)
                .await
                .map_err(|e| CouldNotParseCommand(e))?;
            let loc = match ws.location_of(player, player).await {
                Ok(loc) => loc,
                Err(e) => return Err(DatabaseError(e)),
            };

            let targets_to_search = vec![player, loc, pc.dobj, pc.iobj];
            let mut found = None;
            for target in targets_to_search {
                let match_result = ws
                    .find_command_verb_on(
                        player,
                        target,
                        pc.verb.as_str(),
                        pc.dobj,
                        pc.prep,
                        pc.iobj,
                    )
                    .await;
                let match_result = match match_result {
                    Ok(m) => m,
                    Err(e) => return Err(DatabaseError(e)),
                };
                if let Some(vi) = match_result {
                    found = Some((target, vi, pc.clone()));
                    break;
                }
            }
            let Some((target, vi, pc)) = found else {
                return Err(SchedulerError::NoCommandMatch(command.to_string(), pc));
            };
            (target, vi, pc)
        };
        let state_source = inner.state_source.clone();
        let task_id = inner
            .new_task(
                player,
                state_source,
                sessions,
                None,
                self.clone(),
                player,
                false,
            )
            .await?;

        let Some(task_ref) = inner.tasks.get_mut(&task_id) else {
            return Err(TaskNotFound(task_id));
        };

        trace!(
            "Set up command task {:?} for {:?}, sending StartCommandVerb...",
            task_id,
            command
        );
        // This gets enqueued as the first thing the task sees when it is started.
        task_ref
            .task_control_sender
            .send(TaskControlMsg::StartCommandVerb {
                player,
                vloc,
                verbinfo: vi,
                command,
            })
            .map_err(|e| SchedulerError::CouldNotStartTask(anyhow!(e)))?;

        Ok(task_id)
    }

    /// Submit a verb task to the scheduler for execution.
    #[instrument(skip(self, sessions))]
    pub async fn submit_verb_task(
        &self,
        player: Objid,
        vloc: Objid,
        verb: String,
        args: Vec<Var>,
        perms: Objid,
        sessions: Arc<RwLock<dyn Sessions>>,
    ) -> Result<TaskId, anyhow::Error> {
        increment_counter!("scheduler.submit_verb_task");

        let mut inner = self.inner.write().await;

        let state_source = inner.state_source.clone();
        let task_id = inner
            .new_task(
                player,
                state_source,
                sessions,
                None,
                self.clone(),
                perms,
                false,
            )
            .await?;

        let Some(task_ref) = inner.tasks.get_mut(&task_id) else {
            return Err(anyhow!("Could not find task with id {:?}", task_id));
        };

        // This gets enqueued as the first thing the task sees when it is started.
        task_ref
            .task_control_sender
            .send(TaskControlMsg::StartVerb {
                player,
                vloc,
                verb,
                args,
            })?;

        Ok(task_id)
    }

    /// Submit an eval task to the scheduler for execution.
    #[instrument(skip(self, sessions))]
    pub async fn submit_eval_task(
        &self,
        player: Objid,
        perms: Objid,
        code: String,
        sessions: Arc<RwLock<dyn Sessions>>,
    ) -> Result<TaskId, anyhow::Error> {
        increment_counter!("scheduler.submit_eval_task");

        let mut inner = self.inner.write().await;

        // Compile the text into a verb.
        let binary = compile(code.as_str())?;

        let state_source = inner.state_source.clone();
        let task_id = inner
            .new_task(
                player,
                state_source,
                sessions,
                None,
                self.clone(),
                perms,
                false,
            )
            .await?;

        let Some(task_ref) = inner.tasks.get_mut(&task_id) else {
            return Err(anyhow!("Could not find task with id {:?}", task_id));
        };

        // This gets enqueued as the first thing the task sees when it is started.
        task_ref
            .task_control_sender
            .send(TaskControlMsg::StartEval {
                player,
                program: binary,
            })?;

        Ok(task_id)
    }

    pub async fn abort_player_tasks(&self, player: Objid) -> Result<(), anyhow::Error> {
        let mut inner = self.inner.write().await;
        let mut to_abort = Vec::new();
        for (task_id, task_ref) in inner.tasks.iter() {
            if task_ref.player == player {
                to_abort.push(*task_id);
            }
        }
        for task_id in to_abort {
            inner
                .tasks
                .get_mut(&task_id)
                .unwrap()
                .task_control_sender
                .send(TaskControlMsg::Abort)?;
        }

        Ok(())
    }

    /// Stop the scheduler run loop.
    pub async fn stop(&self) -> Result<(), anyhow::Error> {
        let mut scheduler = self.inner.write().await;
        // Send shut down to all the tasks.
        for task in scheduler.tasks.values() {
            task.task_control_sender.send(TaskControlMsg::Abort)?;
        }
        // Then spin until they're all done.
        while !scheduler.tasks.is_empty() {}
        scheduler.running = false;
        Ok(())
    }
}

impl Inner {
    async fn submit_fork_task(
        &mut self,
        fork_request: ForkRequest,
        state_source: Arc<RwLock<dyn WorldStateSource + Sync + Send>>,
        sessions: Arc<RwLock<dyn Sessions>>,
        scheduler_ref: Scheduler,
    ) -> Result<TaskId, anyhow::Error> {
        increment_counter!("scheduler.forked_tasks");
        let task_id = self
            .new_task(
                fork_request.player,
                state_source,
                sessions,
                fork_request.delay,
                scheduler_ref,
                fork_request.progr,
                false,
            )
            .await?;

        let Some(task_ref) = self.tasks.get_mut(&task_id) else {
            return Err(anyhow!("Could not find task with id {:?}", task_id));
        };

        // If there's a delay on the fork, we will mark it in suspended state and put in the
        // delay time.
        let mut suspended = false;
        if let Some(delay) = fork_request.delay {
            task_ref.suspended = true;
            task_ref.resume_time = Some(SystemTime::now() + delay);
            suspended = true;
        }

        // This gets enqueued as the first thing the task sees when it is started.
        task_ref
            .task_control_sender
            .send(TaskControlMsg::StartFork {
                task_id,
                fork_request,
                suspended,
            })?;

        increment_counter!("scheduler.forked_tasks");

        Ok(task_id)
    }
    /// This is expected to be run on a loop, and will process the first task response it sees.
    #[instrument(skip(self))]
    async fn do_process(&mut self) -> Result<(), anyhow::Error> {
        // Would have preferred a futures::select_all here, but it doesn't seem to be possible to
        // do this without consuming the futures, which we don't want to do.
        let mut to_notify = Vec::new();
        let mut to_remove = Vec::new();
        let mut fork_requests = Vec::new();
        let mut desc_requests = Vec::new();
        let mut kill_requests = Vec::new();
        let mut resume_requests = Vec::new();
        let mut to_disconnect = Vec::new();
        let mut to_wake = Vec::new();
        for (task_id, task) in self.tasks.iter_mut() {
            // Look for any tasks in suspension whose wake-up time has passed.
            if task.suspended {
                if let Some(delay) = task.resume_time {
                    if delay <= SystemTime::now() {
                        to_wake.push(*task_id);
                    }
                }
            }

            match task.scheduler_control_receiver.try_recv() {
                Ok(msg) => match msg {
                    SchedulerControlMsg::TaskAbortCancelled => {
                        increment_counter!("scheduler.aborted_cancelled");

                        warn!(task = task.task_id, "Task cancelled");

                        to_notify.push((*task_id, TaskWaiterResult::AbortCancelled));
                        to_remove.push(*task_id);
                    }
                    SchedulerControlMsg::TaskAbortError(e) => {
                        increment_counter!("scheduler.aborted_error");

                        warn!(task = task.task_id, error = ?e, "Task aborted");

                        to_notify.push((*task_id, TaskWaiterResult::AbortError));
                        to_remove.push(*task_id);
                    }
                    SchedulerControlMsg::TaskAbortLimitsReached(limit_reason) => {
                        match limit_reason {
                            AbortLimitReason::Ticks(t) => {
                                increment_counter!("scheduler.aborted_ticks");
                                warn!(
                                    task = task.task_id,
                                    ticks = t,
                                    "Task aborted, ticks exceeded"
                                );
                            }
                            AbortLimitReason::Time(t) => {
                                increment_counter!("scheduler.aborted_time");
                                warn!(task = task.task_id, time = ?t, "Task aborted, time exceeded");
                            }
                        }
                        increment_counter!("scheduler.aborted_limits");
                        to_notify.push((*task_id, TaskWaiterResult::AbortTimeout(limit_reason)));
                        to_remove.push(*task_id);
                    }
                    SchedulerControlMsg::TaskException(finally_reason) => {
                        increment_counter!("scheduler.task_exception");

                        warn!(task = task.task_id, finally_reason = ?finally_reason, "Task threw exception");
                        to_notify.push((*task_id, TaskWaiterResult::Exception(finally_reason)));
                        to_remove.push(*task_id);
                    }
                    SchedulerControlMsg::TaskSuccess(value) => {
                        increment_counter!("scheduler.task_succeeded");
                        debug!(task = task.task_id, result = ?value, "Task succeeded");
                        to_notify.push((*task_id, TaskWaiterResult::Success(value)));
                        to_remove.push(*task_id);
                    }
                    SchedulerControlMsg::TaskRequestFork(fork_request, reply) => {
                        increment_counter!("scheduler.fork_task");
                        // Task has requested a fork. Dispatch it and reply with the new task id.
                        // Gotta dump this out til we exit the loop tho, since self.tasks is already
                        // borrowed here.
                        fork_requests.push((
                            fork_request,
                            reply,
                            task.state_source.clone(),
                            task.sessions.clone(),
                            task.scheduler.clone(),
                        ));
                    }
                    SchedulerControlMsg::TaskSuspend(resume_time) => {
                        increment_counter!("scheduler.suspend_task");
                        // Task is suspended. The resume time (if any) is the system time at which
                        // the scheduler should try to wake us up.
                        task.suspended = true;
                        task.resume_time = resume_time;
                    }
                    SchedulerControlMsg::DescribeOtherTasks(reply) => {
                        increment_counter!("scheduler.describe_tasks");
                        // Task is asking for a description of all other tasks.
                        desc_requests.push((task.task_id, reply));
                    }
                    SchedulerControlMsg::KillTask {
                        victim_task_id,
                        sender_permissions,
                        result_sender,
                    } => {
                        increment_counter!("scheduler.kill_task");
                        // Task is asking to kill another task.
                        kill_requests.push((
                            task.task_id,
                            victim_task_id,
                            sender_permissions,
                            result_sender,
                        ));
                    }
                    SchedulerControlMsg::ResumeTask {
                        queued_task_id,
                        sender_permissions,
                        return_value,
                        result_sender,
                    } => {
                        increment_counter!("scheduler.resume_task");
                        resume_requests.push((
                            task.task_id,
                            queued_task_id,
                            sender_permissions,
                            return_value,
                            result_sender,
                        ));
                    }
                    SchedulerControlMsg::BootPlayer {
                        player,
                        sender_permissions: _,
                    } => {
                        increment_counter!("scheduler.boot_player");
                        // Task is asking to boot a player.
                        to_disconnect.push((task.task_id, player));
                    }
                },
                Err(TryRecvError::Empty) => {}
                Err(e) => {
                    warn!(task = task.task_id, error = ?e, "Task sys-errored");
                    to_remove.push(*task_id);
                    continue;
                }
            }
        }

        // Send notifications. These are oneshot and consumed.
        for (task_id, result) in to_notify {
            let task = self.tasks.get_mut(&task_id).unwrap();
            for subscriber in task.subscribers.drain(..) {
                if subscriber.send(result.clone()).is_err() {
                    error!("Notify to subscriber on task {} failed", task_id);
                }
            }
        }

        // Service wake-ups
        for task_id in to_wake {
            let task = self.tasks.get_mut(&task_id).unwrap();
            task.suspended = false;

            let world_state = self.state_source.write().await.new_world_state().await?;

            task.task_control_sender
                .send(TaskControlMsg::Resume(world_state, v_int(0)))?;
        }

        // Service fork requests
        for (fork_request, reply, state_source, sessions, scheduler) in fork_requests {
            let task_id = self
                .submit_fork_task(fork_request, state_source, sessions, scheduler)
                .await?;
            reply.send(task_id).expect("Could not send fork reply");
        }

        // Service task removals
        for task_id in to_remove {
            trace!(task = task_id, "Task removed");
            self.tasks.remove(&task_id);
        }

        // Service describe requests.
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
                    task.task_control_sender
                        .send(TaskControlMsg::Describe(t_send))?;
                    let task_desc = t_reply.await?;
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

        // Service kill requests
        for (requesting_task_id, victim_task_id, sender_permissions, result_sender) in kill_requests
        {
            // If the task somehow is reuesting a kill on itself, that would lead to deadlock,
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

            victim_task
                .task_control_sender
                .send(TaskControlMsg::Abort)?;

            result_sender
                .send(v_none())
                .expect("Could not send kill result");
        }

        // Service resume requests
        for (requesting_task_id, queued_task_id, sender_permissions, return_value, result_sender) in
            resume_requests
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
            let world_state = self.state_source.write().await.new_world_state().await?;

            queued_task.suspended = false;
            queued_task
                .task_control_sender
                .send(TaskControlMsg::Resume(world_state, return_value))?;
            result_sender
                .send(v_none())
                .expect("Could not send resume result");
        }

        for (task_id, player) in to_disconnect {
            {
                let task = match self.tasks.get_mut(&task_id) {
                    Some(task) => task,
                    None => {
                        error!(task = task_id, "Task w/ disconnect (and session) not found");
                        continue;
                    }
                };
                // First disconnect the player...
                warn!(?player, "Disconnecting player ...");
                task.sessions.write().await.disconnect(player).await?;
            }

            // Then abort *all* of their still-living tasks.
            for (task_id, task) in self.tasks.iter() {
                if task.player != player {
                    continue;
                }
                warn!(?player, task_id, "Aborting task ...");
                // This is fire and forget, we cannot assume that the task is still alive.
                let Ok(_) = task.task_control_sender.send(TaskControlMsg::Abort) else {
                    trace!(?player, task_id, "Task already dead");
                    continue;
                };
            }
        }

        // Prune any completed/dead tasks
        let dead_tasks: Vec<_> = self
            .tasks
            .iter()
            .filter_map(|(task_id, task)| task.task_control_sender.is_closed().then_some(*task_id))
            .collect();
        for task in dead_tasks {
            self.tasks.remove(&task);
        }
        Ok(())
    }

    async fn new_task(
        &mut self,
        player: Objid,
        state_source: Arc<RwLock<dyn WorldStateSource + Send + Sync>>,
        sessions: Arc<RwLock<dyn Sessions>>,
        delay_start: Option<Duration>,
        scheduler_ref: Scheduler,
        perms: Objid,
        background: bool,
    ) -> Result<TaskId, SchedulerError> {
        increment_counter!("scheduler.new_task");
        let mut world_state = {
            let mut state_source = state_source.write().await;
            state_source
                .new_world_state()
                .await
                .map_err(|e| DatabaseError(e))?
        };

        // Find out max ticks, etc. for this task. These are either pulled from server constants in
        // the DB or from default constants.
        let (max_ticks, max_seconds, max_stack_depth) =
            max_vm_values(world_state.as_mut(), background).await;

        let (task_control_sender, task_control_receiver) = tokio::sync::mpsc::unbounded_channel();
        let (scheduler_control_sender, scheduler_control_receiver) =
            tokio::sync::mpsc::unbounded_channel();

        let task_id = self.next_task_id;
        self.next_task_id += 1;

        let task_control = TaskControl {
            task_id,
            player,
            task_control_sender,
            scheduler_control_receiver,
            state_source: state_source.clone(),
            sessions: sessions.clone(),
            suspended: false,
            resume_time: None,
            scheduler: scheduler_ref.clone(),
            subscribers: vec![],
        };
        self.tasks.insert(task_id, task_control);

        // TODO: support a queue-size on concurrent executing tasks and allow them to sit in an
        // initially suspended state without spawning a worker thread, until the queue has space.
        // Spawn the task's thread.
        tokio::spawn(async move {
            if let Some(delay) = delay_start {
                tokio::time::sleep(delay).await;
            }
            span!(
                Level::DEBUG,
                "spawn_fork",
                task_id = task_id,
                player = player.to_literal()
            );

            let vm = VM::new();

            let task = Task {
                task_id,
                scheduled_start_time: None,
                task_control_receiver,
                scheduler_control_sender,
                player,
                vm,
                sessions: sessions.clone(),
                world_state,
                perms,
                running_method: false,
                max_stack_depth,
                max_ticks,
                max_time: Duration::from_secs(max_seconds),
            };
            debug!("Starting up task: {:?}", task_id);
            task.run().await;
            debug!("Completed task: {:?}", task_id);
        });

        increment_counter!("scheduler.created_tasks");
        gauge!("scheduler.active_tasks", self.tasks.len() as f64);

        Ok(task_id)
    }

    #[instrument(skip(self))]
    pub async fn abort_task(&mut self, id: TaskId) -> Result<(), anyhow::Error> {
        let task = self
            .tasks
            .get_mut(&id)
            .ok_or(anyhow::anyhow!("Task not found"))?;
        task.task_control_sender.send(TaskControlMsg::Abort)?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn remove_task(&mut self, id: TaskId) -> Result<(), anyhow::Error> {
        self.tasks
            .remove(&id)
            .ok_or(anyhow::anyhow!("Task not found"))?;
        Ok(())
    }
}
