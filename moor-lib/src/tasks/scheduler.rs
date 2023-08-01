use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::anyhow;
use dashmap::DashMap;
use fast_counter::ConcurrentCounter;
use thiserror::Error;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::RwLock;
use tracing::{debug, error, instrument, trace};

use crate::compiler::codegen::compile;
use crate::db::match_env::DBMatchEnvironment;
use crate::db::matching::world_environment_match_object;
use crate::db::state::WorldStateSource;
use crate::tasks::command_parse::{parse_command, ParsedCommand};
use crate::tasks::task::{Task, TaskControl, TaskControlMsg, TaskControlResponse};
use crate::tasks::{Sessions, TaskId};
use crate::values::objid::Objid;
use crate::values::var::Var;
use crate::vm::vm::VM;

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
            let mut ws = ss.new_world_state()?;
            let mut me = DBMatchEnvironment { ws: ws.as_mut() };
            let match_object_fn =
                |name: &str| world_environment_match_object(&mut me, player, name).unwrap();
            let pc = parse_command(command, match_object_fn);

            let loc = ws.location_of(player)?;

            match ws.find_command_verb_on(player, &pc)? {
                Some(vi) => (player, vi, pc),
                None => match ws.find_command_verb_on(loc, &pc)? {
                    Some(vi) => (loc, vi, pc),
                    None => match ws.find_command_verb_on(pc.dobj, &pc)? {
                        Some(vi) => (pc.dobj, vi, pc),
                        None => match ws.find_command_verb_on(pc.iobj, &pc)? {
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
                    "Task {:?} completed successfully with return value: {:?}",
                    task_id, value
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
        let state = {
            let mut state_source = state_source.write().await;
            state_source.new_world_state()?
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
            let vm = VM::new();
            let mut task = Task::new(
                task_id,
                rx_control,
                task_response_sender,
                player,
                vm,
                client_connection,
                state,
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use anyhow::Error;
    use async_trait::async_trait;
    use tokio::sync::RwLock;

    use crate::compiler::codegen::compile;
    use crate::db::mock_world_state::MockWorldStateSource;
    use crate::db::rocksdb::LoaderInterface;
    use crate::model::objects::{ObjAttrs, ObjFlag};
    use crate::model::r#match::{ArgSpec, PrepSpec, VerbArgsSpec};
    use crate::model::verbs::VerbFlag;
    use crate::tasks::scheduler::Scheduler;
    use crate::tasks::Sessions;
    use crate::util::bitenum::BitEnum;
    use crate::values::objid::{Objid, NOTHING};

    struct NoopClientConnection {}
    impl NoopClientConnection {
        fn new() -> Self {
            Self {}
        }
    }

    #[async_trait]
    impl Sessions for NoopClientConnection {
        async fn send_text(&mut self, _player: Objid, _msg: &str) -> Result<(), anyhow::Error> {
            Ok(())
        }

        async fn connected_players(&self) -> Result<Vec<Objid>, Error> {
            Ok(vec![])
        }
    }

    // Disabled until mock state is more full featured. This test used to use a full in-mem DB.
    // #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_scheduler_loop() {
        let src = MockWorldStateSource::new();

        let sys_obj = src
            .create_object(
                None,
                ObjAttrs::new()
                    .location(NOTHING)
                    .parent(NOTHING)
                    .name("System")
                    .flags(BitEnum::new_with(ObjFlag::Read)),
            )
            .unwrap();
        src.add_verb(
            sys_obj,
            vec!["test"],
            sys_obj,
            BitEnum::new_with(VerbFlag::Read),
            VerbArgsSpec {
                dobj: ArgSpec::This,
                prep: PrepSpec::None,
                iobj: ArgSpec::This,
            },
            compile("return {1,2,3,4};").unwrap(),
        )
        .unwrap();

        let mut sched = Scheduler::new(Arc::new(RwLock::new(src)));
        let task = sched
            .submit_verb_task(
                sys_obj,
                sys_obj,
                "test".to_string(),
                vec![],
                Arc::new(RwLock::new(NoopClientConnection::new())),
            )
            .await
            .expect("setup command task");
        assert_eq!(sched.tasks.len(), 1);

        sched.start_task(task).await.unwrap();

        assert_eq!(sched.tasks.len(), 1);

        while !sched.tasks.is_empty() {
            sched.do_process().await.unwrap();
        }

        assert_eq!(sched.tasks.len(), 0);
        assert_eq!(sched.num_started_tasks.sum(), 1);
        assert_eq!(sched.num_succeeded_tasks.sum(), 1);
        assert_eq!(sched.num_errored_tasks.sum(), 0);
        assert_eq!(sched.num_excepted_tasks.sum(), 0);
        assert_eq!(sched.num_aborted_tasks.sum(), 0);
    }
}
