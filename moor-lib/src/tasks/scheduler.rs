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

use moor_value::var::objid::Objid;
use moor_value::var::Var;

use crate::compiler::codegen::compile;
use crate::db::match_env::DBMatchEnvironment;
use crate::db::matching::MatchEnvironmentParseMatcher;
use crate::model::permissions::PermissionsContext;
use crate::model::world_state::WorldStateSource;
use crate::tasks::command_parse::{parse_command, ParsedCommand};
use crate::tasks::task::{Task, TaskControlMsg};
use crate::tasks::{Sessions, TaskId};
use crate::vm::vm_unwind::FinallyReason;
use crate::vm::{ForkRequest, VM};

const SCHEDULER_TICK_TIME: Duration = Duration::from_millis(5);

/// Responsible for the dispatching, control, and accounting of tasks in the system.
/// There should be only one scheduler per server.
#[derive(Clone)]
pub struct Scheduler {
    inner: Arc<RwLock<Inner>>,
}

// Scheduler is a just a handle which points to an inner send/sync thing. It can be passed around at
// will between threads.
unsafe impl Send for Scheduler {}
unsafe impl Sync for Scheduler {}

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
    pub permissions: PermissionsContext,
    pub verb_name: String,
    pub verb_definer: Objid,
    pub line_number: usize,
    pub this: Objid,
}

/// The messages that can be sent from tasks to the scheduler.
pub enum SchedulerControlMsg {
    TaskSuccess(Var),
    TaskException(FinallyReason),
    TaskAbortError(Error),
    TaskRequestFork(ForkRequest, oneshot::Sender<TaskId>),
    TaskAbortCancelled,
    /// Tell the scheduler that the task in a suspended state, with a time to resume (if any)
    TaskSuspend(Option<SystemTime>),
    /// Task is requesting a list of all other tasks known to the scheduler.
    DescribeOtherTasks(oneshot::Sender<Vec<TaskDescription>>),
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
}

#[derive(Debug, Eq, PartialEq, Error)]
pub enum SchedulerError {
    #[error("Could not find match for command '{0}': {1:?}")]
    NoCommandMatch(String, ParsedCommand),
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

    /// Execute the scheduler loop, run from the server process.
    pub async fn run(&mut self) {
        {
            let mut start_lock = self.inner.write().await;
            start_lock.running = true;
        }
        let mut interval = tokio::time::interval(SCHEDULER_TICK_TIME);
        loop {
            {
                select! {
                    _ = interval.tick() => {
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
        &mut self,
        player: Objid,
        command: &str,
        sessions: Arc<RwLock<dyn Sessions>>,
    ) -> Result<TaskId, anyhow::Error> {
        let mut inner = self.inner.write().await;

        let (vloc, vi, command) = {
            let mut ss = inner.state_source.write().await;
            let (mut ws, perms) = ss.new_world_state(player).await?;
            let me = DBMatchEnvironment {
                ws: ws.as_mut(),
                perms: perms.clone(),
            };
            let matcher = MatchEnvironmentParseMatcher { env: me, player };
            let pc = parse_command(command, matcher).await?;
            let loc = ws.location_of(perms.clone(), player).await?;

            match ws.find_command_verb_on(perms.clone(), player, &pc).await? {
                Some(vi) => (player, vi, pc),
                None => match ws.find_command_verb_on(perms.clone(), loc, &pc).await? {
                    Some(vi) => (loc, vi, pc),
                    None => match ws.find_command_verb_on(perms.clone(), pc.dobj, &pc).await? {
                        Some(vi) => (pc.dobj, vi, pc),
                        None => match ws.find_command_verb_on(perms.clone(), pc.iobj, &pc).await? {
                            Some(vi) => (pc.iobj, vi, pc),
                            None => {
                                return Err(anyhow!(SchedulerError::NoCommandMatch(
                                    command.to_string(),
                                    pc
                                )));
                            }
                        },
                    },
                },
            }
        };
        let state_source = inner.state_source.clone();
        let task_id = inner
            .new_task(player, state_source, sessions, None, self.clone())
            .await?;

        let Some(task_ref) = inner.tasks.get_mut(&task_id) else {
            return Err(anyhow!("Could not find task with id {:?}", task_id));
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
            })?;

        Ok(task_id)
    }

    /// Submit a verb task to the scheduler for execution.
    #[instrument(skip(self, sessions))]
    pub async fn submit_verb_task(
        &mut self,
        player: Objid,
        vloc: Objid,
        verb: String,
        args: Vec<Var>,
        sessions: Arc<RwLock<dyn Sessions>>,
    ) -> Result<TaskId, anyhow::Error> {
        let mut inner = self.inner.write().await;

        let state_source = inner.state_source.clone();
        let task_id = inner
            .new_task(player, state_source, sessions, None, self.clone())
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
        &mut self,
        player: Objid,
        code: String,
        sessions: Arc<RwLock<dyn Sessions>>,
    ) -> Result<TaskId, anyhow::Error> {
        let mut inner = self.inner.write().await;

        // Compile the text into a verb.
        let binary = compile(code.as_str())?;

        let state_source = inner.state_source.clone();
        let task_id = inner
            .new_task(player, state_source, sessions, None, self.clone())
            .await?;

        let Some(task_ref) = inner.tasks.get_mut(&task_id) else {
            return Err(anyhow!("Could not find task with id {:?}", task_id));
        };

        // This gets enqueued as the first thing the task sees when it is started.
        task_ref
            .task_control_sender
            .send(TaskControlMsg::StartEval { player, binary })?;

        Ok(task_id)
    }

    /// Stop the scheduler run loop.
    pub async fn stop(&mut self) -> Result<(), anyhow::Error> {
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
        let task_id = self
            .new_task(
                fork_request.player,
                state_source,
                sessions,
                fork_request.delay,
                scheduler_ref,
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

        increment_counter!("moo.scheduler.forked_tasks");

        Ok(task_id)
    }
    /// This is expected to be run on a loop, and will process the first task response it sees.
    #[instrument(skip(self))]
    async fn do_process(&mut self) -> Result<(), anyhow::Error> {
        // Would have preferred a futures::select_all here, but it doesn't seem to be possible to
        // do this without consuming the futures, which we don't want to do.
        let mut to_remove = Vec::new();
        let mut fork_requests = Vec::new();
        let mut desc_requests = Vec::new();
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
                        increment_counter!("moo.scheduler.aborted_cancelled");

                        warn!(task = task.task_id, "Task cancelled");
                        to_remove.push(*task_id);
                    }
                    SchedulerControlMsg::TaskAbortError(e) => {
                        increment_counter!("moo.scheduler.aborted_error");

                        warn!(task = task.task_id, error = ?e, "Task aborted");
                        to_remove.push(*task_id);
                    }
                    SchedulerControlMsg::TaskException(finally_reason) => {
                        increment_counter!("moo.scheduler.exception");

                        warn!(task = task.task_id, finally_reason = ?finally_reason, "Task threw exception");
                        to_remove.push(*task_id);
                    }
                    SchedulerControlMsg::TaskSuccess(value) => {
                        increment_counter!("moo.scheduler.succeeded");
                        debug!(task = task.task_id, result = ?value, "Task succeeded");
                        to_remove.push(*task_id);
                    }
                    SchedulerControlMsg::TaskRequestFork(fork_request, reply) => {
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
                        // Task is suspended. The resume time (if any) is the system time at which
                        // the scheduler should try to wake us up.
                        task.suspended = true;
                        task.resume_time = resume_time;
                    }
                    SchedulerControlMsg::DescribeOtherTasks(reply) => {
                        // Task is asking for a description of all other tasks.
                        desc_requests.push((task.task_id, reply));
                    }
                },
                Err(TryRecvError::Empty) => {}
                Err(e) => {
                    error!(task = task.task_id, error = ?e, "Task sys-errored");
                    to_remove.push(*task_id);
                    continue;
                }
            }
        }
        // Service wake-ups
        for task_id in to_wake {
            let task = self.tasks.get_mut(&task_id).unwrap();
            task.suspended = false;

            let (world_state, permissions) = self
                .state_source
                .write()
                .await
                .new_world_state(task.player)
                .await?;

            task.task_control_sender
                .send(TaskControlMsg::Resume(world_state, permissions))?;
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

        Ok(())
    }

    async fn new_task(
        &mut self,
        player: Objid,
        state_source: Arc<RwLock<dyn WorldStateSource + Send + Sync>>,
        sessions: Arc<RwLock<dyn Sessions>>,
        delay_start: Option<Duration>,
        scheduler_ref: Scheduler,
    ) -> Result<TaskId, anyhow::Error> {
        let (world_state, perms) = {
            let mut state_source = state_source.write().await;
            state_source.new_world_state(player).await?
        };

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
                start_time: None,
                task_control_receiver,
                scheduler_control_sender,
                player,
                vm,
                sessions: sessions.clone(),
                world_state,
                perms,
                running_method: false,
                tmp_verb: None,
            };
            debug!("Starting up task: {:?}", task_id);
            task.run().await;
            debug!("Completed task: {:?}", task_id);
        });

        increment_counter!("moo.scheduler.created_tasks");
        gauge!("moo.scheduler.active_tasks", self.tasks.len() as f64);

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
