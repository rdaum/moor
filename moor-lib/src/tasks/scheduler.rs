use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use anyhow::{anyhow, Error};
use dashmap::DashMap;
use fast_counter::ConcurrentCounter;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::RwLock;
use tracing::{debug, error, instrument, trace};

use crate::db::match_env::DBMatchEnvironment;
use crate::db::matching::world_environment_match_object;
use crate::db::state::{WorldState, WorldStateSource};
use crate::model::objects::ObjFlag;
use crate::model::var::{NOTHING, Objid, Var, Variant};
use crate::tasks::parse_cmd::{parse_command, ParsedCommand};
use crate::tasks::Sessions;
use crate::util::bitenum::BitEnum;
use crate::vm::execute::{ExecutionResult, FinallyReason, VM};

pub type TaskId = usize;

#[derive(Debug)]
enum TaskControlMsg {
    StartCommandVerb {
        player: Objid,
        vloc: Objid,
        command: ParsedCommand,
    },
    StartVerb {
        player: Objid,
        vloc: Objid,
        verb: String,
        args: Vec<Var>,
    },
    Abort,
}

#[derive(Debug)]
enum TaskControlResponse {
    Success(Var),
    Exception(FinallyReason),
    AbortError(Error),
    AbortCancelled,
}

pub struct Task {
    task_id: TaskId,
    control_receiver: UnboundedReceiver<TaskControlMsg>,
    response_sender: UnboundedSender<(TaskId, TaskControlResponse)>,
    player: Objid,
    vm: VM,
    sessions: Arc<RwLock<dyn Sessions + Send + Sync>>,
    state: Box<dyn WorldState>,
}

struct TaskControl {
    pub task: Arc<RwLock<Task>>,
    pub control_sender: UnboundedSender<TaskControlMsg>,
}

pub struct Scheduler {
    running: AtomicBool,
    state_source: Arc<RwLock<dyn WorldStateSource + Send + Sync>>,
    next_task_id: AtomicUsize,
    tasks: DashMap<TaskId, TaskControl>,
    response_sender: UnboundedSender<(TaskId, TaskControlResponse)>,
    response_receiver: UnboundedReceiver<(TaskId, TaskControlResponse)>,

    num_scheduled_tasks: ConcurrentCounter,
    num_started_tasks: ConcurrentCounter,
    num_succeeded_tasks: ConcurrentCounter,
    num_aborted_tasks: ConcurrentCounter,
    num_errored_tasks: ConcurrentCounter,
    num_excepted_tasks: ConcurrentCounter,
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
            num_scheduled_tasks: ConcurrentCounter::new(0),
            num_started_tasks: ConcurrentCounter::new(0),
            num_succeeded_tasks: ConcurrentCounter::new(0),
            num_aborted_tasks: ConcurrentCounter::new(0),
            num_errored_tasks: ConcurrentCounter::new(0),
            num_excepted_tasks: ConcurrentCounter::new(0),
        }
    }

    #[instrument(skip(self, sessions))]
    pub async fn setup_command_task(
        &mut self,
        player: Objid,
        command: &str,
        sessions: Arc<RwLock<dyn Sessions + Send + Sync>>,
    ) -> Result<TaskId, anyhow::Error> {
        let (vloc, command) = {
            let mut ss = self.state_source.write().await;
            let mut ws = ss.new_world_state().unwrap();
            let mut me = DBMatchEnvironment { ws: ws.as_mut() };
            let match_object_fn =
                |name: &str| world_environment_match_object(&mut me, player, name).unwrap();
            let pc = parse_command(command, match_object_fn);

            let loc = ws.location_of(player)?;
            let mut vloc = NOTHING;
            if let Some(_vh) = ws.find_command_verb_on(player, &pc)? {
                vloc = player;
            } else if let Some(_vh) = ws.find_command_verb_on(loc, &pc)? {
                vloc = loc;
            } else if let Some(_vh) = ws.find_command_verb_on(pc.dobj, &pc)? {
                vloc = pc.dobj;
            } else if let Some(_vh) = ws.find_command_verb_on(pc.iobj, &pc)? {
                vloc = pc.iobj;
            }

            if vloc == NOTHING {
                return Err(anyhow!("Could not parse command: {:?}", pc));
            }

            (vloc, pc)
        };
        let task_id = self
            .new_task(player, self.state_source.clone(), sessions)
            .await?;

        let Some(task_ref) = self.tasks.get_mut(&task_id) else {
            return Err(anyhow!("Could not find task with id {:?}", task_id));
        };

        // This gets enqueued as the first thing the task sees when it is started.
        task_ref
            .control_sender
            .send(TaskControlMsg::StartCommandVerb {
                player,
                vloc,
                command,
            })?;

        Ok(task_id)
    }

    #[instrument(skip(self, sessions))]
    pub async fn setup_verb_task(
        &mut self,
        player: Objid,
        vloc: Objid,
        verb: String,
        args: Vec<Var>,
        sessions: Arc<RwLock<dyn Sessions + Send + Sync>>,
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
        client_connection: Arc<RwLock<dyn Sessions + Send + Sync>>,
    ) -> Result<TaskId, anyhow::Error> {
        let mut state_source = state_source.write().await;
        let state = state_source.new_world_state()?;
        let vm = VM::new();

        let (tx_control, rx_control) = tokio::sync::mpsc::unbounded_channel();

        let task_id = self.next_task_id.fetch_add(1, Ordering::SeqCst);
        let task = Task {
            task_id,
            control_receiver: rx_control,
            response_sender: self.response_sender.clone(),
            player,
            vm,
            sessions: client_connection,
            state,
        };
        let task_info = TaskControl {
            task: Arc::new(RwLock::new(task)),
            control_sender: tx_control,
        };

        self.num_scheduled_tasks.add(1);

        self.tasks.insert(task_id, task_info);

        Ok(task_id)
    }

    #[instrument(skip(self), name="scheduler_start_task", fields(task_id = task_id))]
    pub async fn start_task(&mut self, task_id: TaskId) -> Result<(), anyhow::Error> {
        let task = {
            let Some(task_ref) = self.tasks.get_mut(&task_id) else {
                return Err(anyhow!("Could not find task with id {:?}", task_id));
            };
            task_ref.task.clone()
        };

        // Spawn the task's thread.
        tokio::spawn(async move {
            debug!("Starting up task: {:?}", task_id);
            task.write().await.run(task_id).await;

            debug!("Completed task: {:?}", task_id);
        })
        .await?;

        self.num_started_tasks.add(1);
        Ok(())
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

impl Task {
    #[instrument(skip(self), name="task_run", fields(task_id = task_id))]
    pub async fn run(&mut self, task_id: TaskId) {
        trace!("Entering task loop...");
        let mut running_method = false;
        loop {
            let msg = if running_method {
                match self.control_receiver.try_recv() {
                    Ok(msg) => Some(msg),
                    Err(TryRecvError::Empty) => None,
                    Err(_) => panic!("Task control channel closed"),
                }
            } else {
                self.control_receiver.recv().await
            };
            // Check for control messages.
            match msg {
                // We've been asked to start a command.
                // We need to set up the VM and then execute it.
                Some(TaskControlMsg::StartCommandVerb {
                    player,
                    vloc,
                    command,
                }) => {
                    // We should never be asked to start a command while we're already running one.
                    assert!(!running_method);
                    self.vm
                        .do_method_verb(
                            self.task_id,
                            self.state.as_mut(),
                            vloc,
                            command.verb.as_str(),
                            false,
                            vloc,
                            player,
                            BitEnum::new_with(ObjFlag::Wizard),
                            player,
                            &command.args,
                        )
                        .expect("Could not set up VM for command execution");
                    running_method = true;
                }

                Some(TaskControlMsg::StartVerb {
                    player,
                    vloc,
                    verb,
                    args,
                }) => {
                    // We should never be asked to start a command while we're already running one.
                    assert!(!running_method);
                    self.vm
                        .do_method_verb(
                            self.task_id,
                            self.state.as_mut(),
                            vloc,
                            verb.as_str(),
                            false,
                            vloc,
                            player,
                            BitEnum::new_with(ObjFlag::Wizard),
                            player,
                            &args,
                        )
                        .expect("Could not set up VM for command execution");
                    running_method = true;
                }
                // We've been asked to die.
                Some(TaskControlMsg::Abort) => {
                    self.state.rollback().unwrap();

                    self.response_sender
                        .send((self.task_id, TaskControlResponse::AbortCancelled))
                        .expect("Could not send abort response");
                    return;
                }
                _ => {}
            }

            if !running_method {
                continue;
            }
            let result = self
                .vm
                .exec(self.state.as_mut(), self.sessions.clone())
                .await;
            match result {
                Ok(ExecutionResult::More) => {}
                Ok(ExecutionResult::Complete(a)) => {
                    self.state.commit().unwrap();

                    debug!("Task {} complete with result: {:?}", task_id, a);

                    self.response_sender
                        .send((self.task_id, TaskControlResponse::Success(a)))
                        .expect("Could not send success response");
                    return;
                }
                Ok(ExecutionResult::Exception(fr)) => {
                    self.state.rollback().unwrap();

                    match &fr {
                        FinallyReason::Abort => {
                            error!("Task {} aborted", task_id);
                            self.sessions
                                .write()
                                .await
                                .send_text(self.player, format!("Aborted: {:?}", fr).to_string())
                                .await
                                .unwrap();

                            self.response_sender
                                .send((self.task_id, TaskControlResponse::AbortCancelled))
                                .expect("Could not send exception response");
                        }
                        FinallyReason::Uncaught {
                            code: _,
                            msg: _,
                            value: _,
                            stack: _,
                            backtrace,
                        } => {
                            // Compose a string out of the backtrace
                            let mut traceback = vec![];
                            for frame in backtrace.iter() {
                                let Variant::Str(s) = frame.v() else {
                                    continue;
                                };
                                traceback.push(format!("{:}\n", s));
                            }

                            for l in traceback.iter() {
                                self.sessions
                                    .write()
                                    .await
                                    .send_text(self.player, l.to_string())
                                    .await
                                    .unwrap();
                            }

                            self.response_sender
                                .send((self.task_id, TaskControlResponse::Exception(fr)))
                                .expect("Could not send exception response");
                        }
                        _ => {
                            self.response_sender
                                .send((self.task_id, TaskControlResponse::Exception(fr.clone())))
                                .expect("Could not send exception response");
                            unreachable!(
                                "Invalid FinallyReason {:?} reached for task {} in scheduler",
                                fr, task_id
                            )
                        }
                    }

                    return;
                }
                Err(e) => {
                    self.state.rollback().unwrap();
                    error!("Task {} failed with error: {:?}", task_id, e);

                    self.response_sender
                        .send((self.task_id, TaskControlResponse::AbortError(e)))
                        .expect("Could not send error response");
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use anyhow::Error;
    use async_trait::async_trait;
    use tokio::sync::RwLock;

    use crate::compiler::codegen::compile;
    use crate::db::moor_db::MoorDB;
    use crate::db::moor_db_worldstate::MoorDbWorldStateSource;
    use crate::model::objects::{ObjAttrs, ObjFlag};
    use crate::model::r#match::{ArgSpec, PrepSpec, VerbArgsSpec};
    use crate::model::var::{NOTHING, Objid};
    use crate::model::verbs::VerbFlag;
    use crate::tasks::scheduler::Scheduler;
    use crate::tasks::Sessions;
    use crate::util::bitenum::BitEnum;

    struct NoopClientConnection {}
    impl NoopClientConnection {
        pub fn new() -> Self {
            Self {}
        }
    }

    #[async_trait]
    impl Sessions for NoopClientConnection {
        async fn send_text(&mut self, _player: Objid, _msg: String) -> Result<(), anyhow::Error> {
            Ok(())
        }

        async fn connected_players(&self) -> Result<Vec<Objid>, Error> {
            Ok(vec![])
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_scheduler_loop() {
        let mut db = MoorDB::new();

        let mut tx = db.do_begin_tx().unwrap();
        let sys_obj = db
            .create_object(
                &mut tx,
                None,
                ObjAttrs::new()
                    .location(NOTHING)
                    .parent(NOTHING)
                    .name("System")
                    .flags(BitEnum::new_with(ObjFlag::Read)),
            )
            .unwrap();
        db.add_verb(
            &mut tx,
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

        db.do_commit_tx(&mut tx).expect("Commit of test data");

        let src = MoorDbWorldStateSource::new(db);

        let mut sched = Scheduler::new(Arc::new(RwLock::new(src)));
        let task = sched
            .setup_verb_task(
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
