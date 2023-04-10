use std::sync::Arc;
use std::sync::Mutex;

use slotmap::{new_key_type, SlotMap};
use tokio::task::spawn_local;

use crate::db::state::{WorldState, WorldStateSource};
use crate::model::objects::ObjFlag;
use crate::model::var::Objid;
use crate::server::parse_cmd::ParsedCommand;
use crate::util::bitenum::BitEnum;
use crate::vm::execute::{ExecutionResult, VM};

new_key_type! { pub struct TaskId; }

pub struct Task {
    pub player: Objid,
    pub vm: Arc<Mutex<VM>>,
}

pub struct TaskState {
    tasks: Arc<Mutex<SlotMap<TaskId, Arc<Mutex<Task>>>>>,
}

pub struct Scheduler {
    state_source: Arc<Mutex<dyn WorldStateSource>>,
    task_state: Arc<Mutex<TaskState>>,
}

impl Scheduler {
    pub fn new(state_source: Arc<Mutex<dyn WorldStateSource>>) -> Self {
        let sm: SlotMap<TaskId, Arc<Mutex<Task>>> = SlotMap::with_key();
        let task_state = Arc::new(Mutex::new(TaskState {
            tasks: Arc::new(Mutex::new(sm)),
        }));
        Self {
            state_source,
            task_state,
        }
    }

    pub fn setup_command_task(
        &mut self,
        player: Objid,
        command: ParsedCommand,
    ) -> Result<TaskId, anyhow::Error> {
        let mut ts = self.task_state.lock().unwrap();
        let task_id = ts.new_task(player, self.state_source.clone())?;

        let task_ref = ts.get_task(task_id).unwrap();
        let task_ref = task_ref.lock().unwrap();
        let player = task_ref.player;
        let mut vm = task_ref.vm.lock().unwrap();
        vm.do_method_verb(
            player,
            command.verb.as_str(),
            false,
            player,
            player,
            BitEnum::new_with(ObjFlag::Wizard),
            player,
            command.args,
        )
        .unwrap();

        Ok(task_id)
    }

    pub async fn start_task(&mut self, task_id: TaskId) -> Result<(), anyhow::Error> {
        let (vm, ts) = {
            let ts = self.task_state.lock().unwrap();
            let task_ref_guard = ts.get_task(task_id).unwrap();

            let task_ref_ref = task_ref_guard;
            let task_ref = task_ref_ref.lock().unwrap();
            let vm = task_ref.vm.clone();
            (vm, self.task_state.clone())
        };

        drop(self);
        // We use spawn_local because of the amount of bound variables (vm, state) here that
        // would be difficult to have as 'Send.
        spawn_local(async move {
            loop {
                let mut vm = vm.lock().unwrap();
                let result = vm.exec().await;
                match result {
                    Ok(ExecutionResult::More) => continue,
                    Ok(ExecutionResult::Complete(a)) => {
                        let mut ts = ts.lock().unwrap();
                        ts.commit_task(task_id).unwrap();

                        eprintln!("Task {} complete with result: {:?}", task_id.0.as_ffi(), a);
                        break;
                    }
                    Err(e) => {
                        let mut ts = ts.lock().unwrap();
                        ts.rollback_task(task_id).unwrap();
                        eprintln!("Task {} failed with error: {:?}", task_id.0.as_ffi(), e);
                        panic!("error during execution: {:?}", e)
                    }
                }
            }
        });

        Ok(())
    }

    // TODO:
    // Add concept of a 'connection' to the scheduler? Or is player sufficient?
    // Should be able to dispatch through:

    // - do_login_task: login, create player, etc.
    //      - need to think about what this means for us
    // - do_command_task: parse command, then execute verb
    //      - requires functionality in ODB to find/match command verb.  missing now

    // After configuration as above, a task would be created, and then the scheduler would
    // be able to spawn a new thread to execute the task. The execution would be invocation of
    // the VM in a loop until completion, at which time a commit or rollback would be invoked.
    // The task would be removed from the scheduler, and the thread would exit.

    // Note that each physical connection is not 1:1 to a thread.

    // Look into if tokio is a good fit here. Bad luck with it in the past, but this might be
    // an appropriate place for it.

    // Could just as easily just be a standard thread pool. async might be overkill, as there
    // would be little I/O blocking here, unless we rework the lower DB layer to be async as well.

    // Which would be major surgery and require piping async all the way up to the VM layer.
    // Might be worth it but ouch.

    // On the other hand most network I/O layers are async, and the websockets library likely will
    // be as well.

    // The following below is only provisional.
}

impl TaskState {
    pub fn new_task(
        &mut self,
        player: Objid,
        state_source: Arc<Mutex<dyn WorldStateSource>>,
    ) -> Result<TaskId, anyhow::Error> {
        let mut state_source = state_source.lock().unwrap();
        let state = state_source.new_transaction()?;
        let vm = Arc::new(Mutex::new(VM::new(state)));
        let tasks = self.tasks.clone();
        let mut tasks = tasks.lock().unwrap();
        let id = tasks.insert(Arc::new(Mutex::new(Task { player, vm })));

        Ok(id)
    }

    pub fn get_task(&self, id: TaskId) -> Option<Arc<Mutex<Task>>> {
        let mut tasks = self.tasks.lock().unwrap();
        tasks.get_mut(id).cloned()
    }

    pub fn commit_task(&mut self, id: TaskId) -> Result<(), anyhow::Error> {
        let task = self.get_task(id).ok_or(anyhow::anyhow!("Task not found"))?;
        let task = task.lock().unwrap();
        task.vm.lock().unwrap().commit()?;
        self.remove_task(id)?;
        Ok(())
    }

    pub fn rollback_task(&mut self, id: TaskId) -> Result<(), anyhow::Error> {
        let task = self.get_task(id).ok_or(anyhow::anyhow!("Task not found"))?;
        let task = task.lock().unwrap();
        task.vm.lock().unwrap().rollback()?;
        self.remove_task(id)?;
        Ok(())
    }

    fn remove_task(&mut self, id: TaskId) -> Result<(), anyhow::Error> {
        let mut tasks = self.tasks.lock().unwrap();
        tasks.remove(id).ok_or(anyhow::anyhow!("Task not found"))?;
        Ok(())
    }
}
