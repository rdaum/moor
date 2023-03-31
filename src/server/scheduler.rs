use crate::db::state::{WorldState, WorldStateSource};
use crate::model::ObjDB;
use crate::model::var::Objid;
use crate::vm::execute::VM;

pub struct Task<'a> {
    pub id: usize,
    pub player: Objid,
    pub vm: VM,
    pub state: Box<dyn WorldState + 'a>
}

pub struct Scheduler<'a> {
    state_source: Box<dyn WorldStateSource>,
    tasks: Vec<Task<'a>>,
}

impl<'a> Scheduler<'a> {
    pub fn new(state_source: Box<dyn WorldStateSource>) -> Self {
        Self {
            state_source,
            // TODO might want to use SlotMap etc here instead.
            tasks: Vec::new(),
        }
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

    pub fn new_task(&'a mut self, player: Objid) -> Result<usize, anyhow::Error> {
        let state = self.state_source.new_transaction()?;
        let vm = VM::new();
        let id = self.tasks.len();
        self.tasks.push(Task {
            id,
            player,
            vm,
            state
        });

        Ok(id)
    }

    pub fn get_task(&'a  mut self, id: usize) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| t.id == id)
    }

    pub fn get_task_mut(&'a mut self, id: usize) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| t.id == id)
    }

    pub fn get_tasks_for_player(&'a mut self, player: Objid) -> Vec<&mut Task> {
        self.tasks.iter_mut().filter(|t| t.player == player).collect()
    }

    pub fn commit_task(&mut self, id: usize) -> Result<(), anyhow::Error> {
        let mut task = self.remove_task(id).ok_or(anyhow::anyhow!("Task not found"))?;
        task.state.commit()?;
        Ok(())
    }

    pub fn rollback_task(&mut self, id: usize) -> Result<(), anyhow::Error> {
        let mut task = self.remove_task(id).ok_or(anyhow::anyhow!("Task not found"))?;
        task.state.rollback()?;
        Ok(())
    }

    fn remove_task(&mut self, id: usize) -> Option<Task> {
        let idx = self.tasks.iter().position(|t| t.id == id)?;
        Some(self.tasks.remove(idx))
    }
}