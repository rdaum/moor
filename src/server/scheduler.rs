use std::sync::Arc;

use anyhow::{anyhow, Error};
use slotmap::{new_key_type, SlotMap};
use tokio::sync::Mutex;
use tracing::{debug, error, instrument, trace};

use crate::db::matching::{world_environment_match_object, MatchEnvironment};
use crate::db::state::{WorldState, WorldStateSource};
use crate::model::objects::ObjFlag;
use crate::model::var::Error::E_NONE;
use crate::model::var::{Objid, Var, NOTHING};
use crate::server::parse_cmd::{parse_command, ParsedCommand};
use crate::server::Sessions;
use crate::util::bitenum::BitEnum;
use crate::vm::execute::{ExecutionResult, VM};

new_key_type! { pub struct TaskId; }

pub struct Task {
    pub player: Objid,
    pub vm: Arc<Mutex<VM>>,
    pub sessions: Arc<Mutex<dyn Sessions + Send + Sync>>,
}

pub struct TaskState {
    tasks: Arc<Mutex<SlotMap<TaskId, Arc<Mutex<Task>>>>>,
}

pub struct Scheduler {
    state_source: Arc<Mutex<dyn WorldStateSource + Send + Sync>>,
    task_state: Arc<Mutex<TaskState>>,
}

struct DBMatchEnvironment<'a> {
    ws: &'a mut dyn WorldState,
}

impl<'a> MatchEnvironment for DBMatchEnvironment<'a> {
    fn obj_valid(&mut self, oid: Objid) -> Result<bool, Error> {
        self.ws.valid(oid).map_err(|e| anyhow!(e))
    }

    fn get_names(&mut self, oid: Objid) -> Result<Vec<String>, Error> {
        let mut names = self.ws.names_of(oid)?;
        let mut object_names = vec![names.0];
        object_names.append(&mut names.1);
        Ok(object_names)
    }

    fn get_surroundings(&mut self, player: Objid) -> Result<Vec<Objid>, Error> {
        let location = self.ws.location_of(player)?;
        let mut surroundings = self.ws.contents_of(location)?;
        surroundings.push(location);
        surroundings.push(player);

        Ok(surroundings)
    }

    fn location_of(&mut self, player: Objid) -> Result<Objid, Error> {
        Ok(self.ws.location_of(player)?)
    }
}

impl Scheduler {
    pub fn new(state_source: Arc<Mutex<dyn WorldStateSource + Sync + Send>>) -> Self {
        let sm: SlotMap<TaskId, Arc<Mutex<Task>>> = SlotMap::with_key();
        let task_state = Arc::new(Mutex::new(TaskState {
            tasks: Arc::new(Mutex::new(sm)),
        }));
        Self {
            state_source,
            task_state,
        }
    }

    #[instrument(skip(self, sessions), name = "start_task")]
    pub async fn setup_parse_command_task(
        &mut self,
        player: Objid,
        command: &str,
        sessions: Arc<Mutex<dyn Sessions + Send + Sync>>,
    ) -> Result<TaskId, anyhow::Error> {
        let (vloc, pc) = {
            let mut ss = self.state_source.lock().await;
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

        self.setup_command_task(player, vloc, pc, sessions).await
    }

    async fn setup_command_task(
        &mut self,
        player: Objid,
        vloc: Objid,
        command: ParsedCommand,
        client_connection: Arc<Mutex<dyn Sessions + Sync + Send>>,
    ) -> Result<TaskId, anyhow::Error> {
        let mut ts = self.task_state.lock().await;
        let task_id = ts
            .new_task(player, self.state_source.clone(), client_connection)
            .await?;

        let task_ref = ts.get_task(task_id).await.unwrap();
        let task_ref = task_ref.lock().await;
        let player = task_ref.player;
        let mut vm = task_ref.vm.lock().await;
        let result = vm.do_method_verb(
            vloc,
            command.verb.as_str(),
            false,
            vloc,
            player,
            BitEnum::new_with(ObjFlag::Wizard),
            player,
            command.args,
        )?;
        if result != Var::Err(E_NONE) {
            return Err(anyhow!("exception while setting up VM: {:?}", result));
        }
        Ok(task_id)
    }

    #[instrument(skip(self), name="scheduler_start_task", fields(task_id = task_id.0.as_ffi()))]
    pub async fn start_task(&mut self, task_id: TaskId) -> Result<(), anyhow::Error> {
        let ts = self.task_state.lock().await;
        let task_ref = ts.get_task(task_id).await.unwrap();

        tokio::spawn(async move {
            debug!("Starting up task: {:?}", task_id);
            let mut task_ref = task_ref.lock().await;

            task_ref.run(task_id).await;

            debug!("Completed task: {:?}", task_id);
        })
        .await?;

        Ok(())
    }
}

impl Task {
    #[instrument(skip(self), name="task_run", fields(task_id = task_id.0.as_ffi()))]
    pub async fn run(&mut self, task_id: TaskId) {
        trace!("Entering task loop...");
        let mut vm = self.vm.lock().await;
        loop {
            let result = vm.exec(self.sessions.clone()).await;
            match result {
                Ok(ExecutionResult::More) => {}
                Ok(ExecutionResult::Complete(a)) => {
                    vm.commit().unwrap();

                    debug!("Task {} complete with result: {:?}", task_id.0.as_ffi(), a);
                    return;
                }
                Ok(ExecutionResult::Exception(e)) => {
                    vm.rollback().unwrap();

                    debug!("Task finished with exception {:?}", e);
                    self.sessions
                        .lock()
                        .await
                        .send_text(self.player, format!("Exception: {:?}", e).to_string())
                        .await
                        .unwrap();

                    return;
                }
                Err(e) => {
                    vm.rollback().unwrap();
                    error!("Task {} failed with error: {:?}", task_id.0.as_ffi(), e);
                    return;
                }
            }
        }
    }
}

impl TaskState {
    pub async fn new_task(
        &mut self,
        player: Objid,
        state_source: Arc<Mutex<dyn WorldStateSource + Send + Sync>>,
        client_connection: Arc<Mutex<dyn Sessions + Send + Sync>>,
    ) -> Result<TaskId, anyhow::Error> {
        let mut state_source = state_source.lock().await;
        let state = state_source.new_world_state()?;
        let vm = Arc::new(Mutex::new(VM::new(state)));
        let tasks = self.tasks.clone();
        let mut tasks = tasks.lock().await;
        let id = tasks.insert(Arc::new(Mutex::new(Task {
            player,
            vm,
            sessions: client_connection,
        })));

        Ok(id)
    }

    pub async fn get_task(&self, id: TaskId) -> Option<Arc<Mutex<Task>>> {
        let mut tasks = self.tasks.lock().await;
        tasks.get_mut(id).cloned()
    }

    pub async fn commit_task(&mut self, id: TaskId) -> Result<(), anyhow::Error> {
        let task = self
            .get_task(id)
            .await
            .ok_or(anyhow::anyhow!("Task not found"))?;
        let task = task.lock().await;
        task.vm.lock().await.commit()?;
        self.remove_task(id).await?;
        Ok(())
    }

    pub async fn rollback_task(&mut self, id: TaskId) -> Result<(), anyhow::Error> {
        let task = self
            .get_task(id)
            .await
            .ok_or(anyhow::anyhow!("Task not found"))?;
        let task = task.lock().await;
        task.vm.lock().await.rollback()?;
        self.remove_task(id).await?;
        Ok(())
    }

    async fn remove_task(&mut self, id: TaskId) -> Result<(), anyhow::Error> {
        let mut tasks = self.tasks.lock().await;
        tasks.remove(id).ok_or(anyhow::anyhow!("Task not found"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use anyhow::Error;
    use async_trait::async_trait;
    use tokio::sync::Mutex;
    use tracing::info;

    use crate::compiler::codegen::compile;
    use crate::db::inmem_db::ImDB;
    use crate::db::inmem_db_worldstate::ImDbWorldStateSource;
    use crate::model::objects::{ObjAttrs, ObjFlag};
    use crate::model::r#match::{ArgSpec, PrepSpec, VerbArgsSpec};
    use crate::model::var::{Objid, NOTHING};
    use crate::model::verbs::VerbFlag;
    use crate::server::parse_cmd::ParsedCommand;
    use crate::server::scheduler::Scheduler;
    use crate::server::Sessions;
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

        async fn connected_players(&mut self) -> Result<Vec<Objid>, Error> {
            Ok(vec![])
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn setup() {
        let mut db = ImDB::new();

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

        let src = ImDbWorldStateSource::new(db);

        let mut sched = Scheduler::new(Arc::new(Mutex::new(src)));
        let task = sched
            .setup_command_task(
                sys_obj,
                sys_obj,
                ParsedCommand {
                    verb: "test".to_string(),
                    argstr: "".to_string(),
                    args: vec![],
                    dobjstr: "".to_string(),
                    dobj: NOTHING,
                    prepstr: "".to_string(),
                    prep: PrepSpec::Any,
                    iobjstr: "".to_string(),
                    iobj: NOTHING,
                },
                Arc::new(Mutex::new(NoopClientConnection::new())),
            )
            .await
            .expect("setup command task");

        sched.start_task(task).await.unwrap();

        info!("Done");
    }
}
