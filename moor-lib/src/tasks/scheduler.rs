use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::anyhow;
use dashmap::DashMap;
use fast_counter::ConcurrentCounter;
use thiserror::Error;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::RwLock;
use tracing::{debug, error, instrument, span, trace, Level};

use crate::compiler::codegen::compile;
use crate::db::match_env::DBMatchEnvironment;
use crate::db::matching::MatchEnvironmentParseMatcher;
use crate::model::world_state::WorldStateSource;
use crate::tasks::command_parse::{parse_command, ParsedCommand};
use crate::tasks::task::{Task, TaskControl, TaskControlMsg, TaskControlResponse};
use crate::tasks::{Sessions, TaskId};
use crate::vm::VM;
use moor_value::var::objid::Objid;
use moor_value::var::Var;

pub struct Scheduler {
    running: AtomicBool,
    state_source: Arc<RwLock<dyn WorldStateSource + Send + Sync>>,
    next_task_id: AtomicUsize,
    tasks: DashMap<TaskId, TaskControl>,
    response_sender: UnboundedSender<(TaskId, TaskControlResponse)>,
    response_receiver: UnboundedReceiver<(TaskId, TaskControlResponse)>,

    num_started_tasks: ConcurrentCounter,
    num_succeeded_tasks: ConcurrentCounter,
    num_aborted_tasks: ConcurrentCounter,
    num_errored_tasks: ConcurrentCounter,
    num_excepted_tasks: ConcurrentCounter,
}

#[derive(Debug, Eq, PartialEq, Error)]
pub enum SchedulerError {
    #[error("Could not find match for command '{0}': {1:?}")]
    NoCommandMatch(String, ParsedCommand),
}

impl Scheduler {
    pub fn new(state_source: Arc<RwLock<dyn WorldStateSource + Sync + Send>>) -> Self {
        let (response_sender, response_receiver) = tokio::sync::mpsc::unbounded_channel();
        Self {
            running: Default::default(),
            state_source,
            next_task_id: Default::default(),
            tasks: DashMap::new(),
            response_sender,
            response_receiver,
            num_started_tasks: ConcurrentCounter::new(0),
            num_succeeded_tasks: ConcurrentCounter::new(0),
            num_aborted_tasks: ConcurrentCounter::new(0),
            num_errored_tasks: ConcurrentCounter::new(0),
            num_excepted_tasks: ConcurrentCounter::new(0),
        }
    }

    #[instrument(skip(self, sessions))]
    pub async fn submit_command_task(
        &mut self,
        player: Objid,
        command: &str,
        sessions: Arc<RwLock<dyn Sessions>>,
    ) -> Result<TaskId, anyhow::Error> {
        let (vloc, vi, command) = {
            let mut ss = self.state_source.write().await;
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
        let task_id = self
            .new_task(player, self.state_source.clone(), sessions)
            .await?;

        let Some(task_ref) = self.tasks.get_mut(&task_id) else {
            return Err(anyhow!("Could not find task with id {:?}", task_id));
        };

        trace!(
            "Set up command task {:?} for {:?}, sending StartCommandVerb...",
            task_id,
            command
        );
        // This gets enqueued as the first thing the task sees when it is started.
        task_ref
            .control_sender
            .send(TaskControlMsg::StartCommandVerb {
                player,
                vloc,
                verbinfo: vi,
                command,
            })?;

        Ok(task_id)
    }

    #[instrument(skip(self, sessions))]
    pub async fn submit_verb_task(
        &mut self,
        player: Objid,
        vloc: Objid,
        verb: String,
        args: Vec<Var>,
        sessions: Arc<RwLock<dyn Sessions>>,
    ) -> Result<TaskId, anyhow::Error> {
        let task_id = self
            .new_task(player, self.state_source.clone(), sessions)
            .await?;

        let Some(task_ref) = self.tasks.get_mut(&task_id) else {
            return Err(anyhow!("Could not find task with id {:?}", task_id));
        };

        // This gets enqueued as the first thing the task sees when it is started.
        task_ref.control_sender.send(TaskControlMsg::StartVerb {
            player,
            vloc,
            verb,
            args,
        })?;

        Ok(task_id)
    }

    #[instrument(skip(self, sessions))]
    pub async fn submit_eval_task(
        &mut self,
        player: Objid,
        code: String,
        sessions: Arc<RwLock<dyn Sessions>>,
    ) -> Result<TaskId, anyhow::Error> {
        // Compile the text into a verb.
        let binary = compile(code.as_str())?;

        let task_id = self
            .new_task(player, self.state_source.clone(), sessions)
            .await?;

        let Some(task_ref) = self.tasks.get_mut(&task_id) else {
            return Err(anyhow!("Could not find task with id {:?}", task_id));
        };

        // This gets enqueued as the first thing the task sees when it is started.
        task_ref
            .control_sender
            .send(TaskControlMsg::StartEval { player, binary })?;

        Ok(task_id)
    }

    #[instrument(skip(self))]
    pub async fn do_process(&mut self) -> Result<(), anyhow::Error> {
        let msg = match self.response_receiver.try_recv() {
            Ok(msg) => msg,
            Err(TryRecvError::Empty) => return Ok(()),
            Err(e) => {
                return Err(anyhow!(e));
            }
        };
        match msg {
            (task_id, TaskControlResponse::AbortCancelled) => {
                self.num_aborted_tasks.add(1);

                debug!("Cleaning up cancelled task {:?}", task_id);
                self.remove_task(task_id)
                    .await
                    .expect("Could not remove task");
            }
            (task_id, TaskControlResponse::AbortError(e)) => {
                self.num_errored_tasks.add(1);

                error!("Error in task {:?}: {:?}", task_id, e);
                self.remove_task(task_id)
                    .await
                    .expect("Could not remove task");
            }
            (task_id, TaskControlResponse::Exception(finally_reason)) => {
                self.num_excepted_tasks.add(1);

                error!("Exception in task {:?}: {:?}", task_id, finally_reason);
                self.remove_task(task_id)
                    .await
                    .expect("Could not remove task");
            }
            (task_id, TaskControlResponse::Success(value)) => {
                self.num_succeeded_tasks.add(1);
                debug!(
                    "Task {:?} completed successfully with return value: {}",
                    task_id,
                    value.to_literal()
                );
                self.remove_task(task_id)
                    .await
                    .expect("Could not remove task");
            }
        }
        Ok(())
    }

    pub async fn stop(scheduler: Arc<RwLock<Self>>) -> Result<(), anyhow::Error> {
        let scheduler = scheduler.write().await;
        scheduler.running.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn new_task(
        &mut self,
        player: Objid,
        state_source: Arc<RwLock<dyn WorldStateSource + Send + Sync>>,
        client_connection: Arc<RwLock<dyn Sessions>>,
    ) -> Result<TaskId, anyhow::Error> {
        let (state, perms) = {
            let mut state_source = state_source.write().await;
            state_source.new_world_state(player).await?
        };

        let (tx_control, rx_control) = tokio::sync::mpsc::unbounded_channel();

        let task_id = self.next_task_id.fetch_add(1, Ordering::SeqCst);

        let task_control = TaskControl {
            control_sender: tx_control,
        };

        self.tasks.insert(task_id, task_control);

        let task_response_sender = self.response_sender.clone();

        // Spawn the task's thread.
        tokio::spawn(async move {
            span!(
                Level::DEBUG,
                "spawn_task",
                task_id = task_id,
                player = player.to_literal()
            );

            let vm = VM::new();
            let mut task = Task::new(
                task_id,
                rx_control,
                task_response_sender,
                player,
                vm,
                client_connection,
                state,
                perms,
            );

            debug!("Starting up task: {:?}", task_id);
            task.run(task_id).await;
            debug!("Completed task: {:?}", task_id);
        });

        self.num_started_tasks.add(1);
        Ok(task_id)
    }

    #[instrument(skip(self))]
    pub async fn abort_task(&mut self, id: TaskId) -> Result<(), anyhow::Error> {
        let task = self
            .tasks
            .get_mut(&id)
            .ok_or(anyhow::anyhow!("Task not found"))?;
        task.control_sender.send(TaskControlMsg::Abort)?;
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
