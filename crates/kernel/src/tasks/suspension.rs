// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use ahash::AHasher;
use bincode::de::{BorrowDecoder, Decoder};
use bincode::enc::Encoder;
use bincode::error::{DecodeError, EncodeError};
use bincode::{BorrowDecode, Decode, Encode};
use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use moor_var::Obj;

use crate::tasks::sessions::{NoopClientSession, Session, SessionFactory};
use crate::tasks::task::Task;
use crate::tasks::{TaskDescription, TaskResult, TasksDb};
use moor_common::tasks::{SchedulerError, TaskId};

/// State a suspended task sits in inside the `suspended` side of the task queue.
/// When tasks are not running they are moved into these.
pub struct SuspendedTask {
    pub wake_condition: WakeCondition,
    pub task: Task,
    pub session: Arc<dyn Session>,
    pub result_sender: Option<oneshot::Sender<Result<TaskResult, SchedulerError>>>,
}

/// Possible conditions in which a suspended task can wake from suspension.
#[derive(Debug)]
pub enum WakeCondition {
    /// This task will never wake up on its own, and must be manually woken with `bf_resume`
    Never,
    /// This task will wake up when the given time is reached.
    Time(Instant),
    /// This task will wake up when the given input request is fulfilled.
    Input(Uuid),
    /// This task will wake up when the given task is completed.
    Task(TaskId),
}

#[repr(u8)]
#[derive(Encode, Decode, Debug)]
pub enum WakeConditionType {
    Never = 0,
    Time = 1,
    Input = 2,
    Task = 3,
}

impl WakeCondition {
    pub fn condition_type(&self) -> WakeConditionType {
        match self {
            WakeCondition::Never => WakeConditionType::Never,
            WakeCondition::Time(_) => WakeConditionType::Time,
            WakeCondition::Input(_) => WakeConditionType::Input,
            WakeCondition::Task(_) => WakeConditionType::Task,
        }
    }
}

/// Ties the local storage for suspended tasks in with a reference to the tasks DB, to allow for
/// keeping them in sync.
pub struct SuspensionQ {
    tasks: HashMap<TaskId, SuspendedTask, BuildHasherDefault<AHasher>>,
    tasks_database: Box<dyn TasksDb>,
}

impl SuspensionQ {
    pub fn new(tasks_database: Box<dyn TasksDb>) -> Self {
        Self {
            tasks: Default::default(),
            tasks_database,
        }
    }

    /// Load all tasks from the tasks database. Called on startup to reconstitute the task list
    /// from the database.
    pub(crate) fn load_tasks(&mut self, bg_session_factory: Arc<dyn SessionFactory>) {
        // LambdaMOO doesn't do anything special to filter out tasks that are too old, or tasks that
        // are related to disconnected players, or anything like that.
        // We'll just start them all up and let the scheduler handle them.
        // This could in theory lead to a sudden glut of starting tasks firing up when the server
        // restarts, but we'll just have to live with that for now.
        let tasks = self
            .tasks_database
            .load_tasks()
            .expect("Unable to reconstitute tasks from tasks database");
        let num_tasks = tasks.len();
        for mut task in tasks {
            debug!(wake_condition = ?task.wake_condition, task_id = task.task.task_id,
                start = ?task.task.task_start , "Loaded suspended task from tasks database");
            task.session = bg_session_factory
                .clone()
                .mk_background_session(&task.task.player)
                .expect("Unable to create new background session for suspended task");
            self.tasks.insert(task.task.task_id, task);
        }
        // Now delete them from the database.
        if let Err(e) = self.tasks_database.delete_all_tasks() {
            error!(?e, "Could not delete suspended tasks from tasks database");
        }
        info!(?num_tasks, "Loaded suspended tasks from tasks database")
    }

    /// Add a task to the set of suspended tasks.
    pub(crate) fn add_task(
        &mut self,
        wake_condition: WakeCondition,
        task: Task,
        session: Arc<dyn Session>,
        result_sender: Option<oneshot::Sender<Result<TaskResult, SchedulerError>>>,
    ) {
        let task_id = task.task_id;
        let sr = SuspendedTask {
            wake_condition,
            task,
            session,
            result_sender,
        };
        if let Err(e) = self.tasks_database.save_task(&sr) {
            error!(?e, "Could not save suspended task");
        }
        self.tasks.insert(task_id, sr);
    }

    /// Remove a task from the set of suspended tasks.
    pub(crate) fn remove_task(&mut self, task_id: TaskId) -> Option<SuspendedTask> {
        let task = self.tasks.remove(&task_id);
        if task.is_some() {
            if let Err(e) = self.tasks_database.delete_task(task_id) {
                error!(?e, "Could not delete suspended task from tasks database");
            }
        }
        task
    }

    /// Synchronize the suspended tasks with the tasks database. Called on shutdown.
    pub(crate) fn save_tasks(&self) {
        for (_, st) in self.tasks.iter() {
            if let Err(e) = self.tasks_database.save_task(st) {
                error!(?e, "Could not save suspended task");
            }
        }
    }

    /// Collect tasks that need to be woken up, pull them from our suspended list, and return them.
    pub(crate) fn collect_wake_tasks(&mut self) -> Vec<SuspendedTask> {
        let now = Instant::now();
        let mut to_wake = vec![];
        for task in self.tasks.values() {
            match task.wake_condition {
                WakeCondition::Time(t) => {
                    if t <= now {
                        to_wake.push(task.task.task_id);
                    }
                }
                WakeCondition::Task(task_id) => {
                    if !self.tasks.contains_key(&task_id) {
                        to_wake.push(task.task.task_id);
                    }
                }
                _ => {}
            }
        }
        let mut tasks = vec![];
        for task_id in to_wake {
            let sr = self.tasks.remove(&task_id).unwrap();
            tasks.push(sr);
        }
        tasks
    }

    /// Pull a task from the suspended list that is waiting for input, for the given player.
    pub(crate) fn pull_task_for_input(
        &mut self,
        input_request_id: Uuid,
        player: &Obj,
    ) -> Option<SuspendedTask> {
        let (task_id, perms) = self.tasks.iter().find_map(|(task_id, sr)| {
            if let WakeCondition::Input(request_id) = &sr.wake_condition {
                if *request_id == input_request_id {
                    Some((*task_id, sr.task.perms.clone()))
                } else {
                    None
                }
            } else {
                None
            }
        })?;

        // If the player doesn't match, we'll pretend we didn't even see it.
        if perms.ne(player) {
            warn!(
                ?task_id,
                ?input_request_id,
                ?player,
                "Task input request received for wrong player"
            );
            return None;
        };

        let sr = self.remove_task(task_id).expect("Corrupt task list");
        Some(sr)
    }

    /// Get a nice friendly list of all tasks in suspension state.
    pub(crate) fn tasks(&self) -> Vec<TaskDescription> {
        let mut tasks = Vec::new();

        // Suspended tasks.
        for (_, sr) in self.tasks.iter() {
            let start_time = match sr.wake_condition {
                WakeCondition::Time(t) => {
                    let distance_from_now = t.duration_since(Instant::now());
                    Some(SystemTime::now() + distance_from_now)
                }
                WakeCondition::Task(task_id) => {
                    if self.tasks.contains_key(&task_id) {
                        Some(SystemTime::now() + Duration::from_secs(1000000000))
                    } else {
                        None
                    }
                }
                _ => None,
            };
            tasks.push(TaskDescription {
                task_id: sr.task.task_id,
                start_time,
                permissions: sr.task.perms.clone(),
                verb_name: sr.task.vm_host.verb_name(),
                verb_definer: sr.task.vm_host.verb_definer(),
                line_number: sr.task.vm_host.line_number(),
                this: sr.task.vm_host.this(),
            });
        }
        tasks
    }

    /// Check if the task is suspended, and if so, return its permissions.
    /// If `filter_input` is true, filter out WaitingInput tasks.
    pub(crate) fn perms_check(&self, task_id: TaskId, filter_input: bool) -> Option<Obj> {
        let sr = self.tasks.get(&task_id)?;
        if filter_input {
            if let WakeCondition::Input(_) = sr.wake_condition {
                return None;
            }
        }
        Some(sr.task.perms.clone())
    }

    /// Remove all non-background tasks for the given player.
    pub(crate) fn prune_foreground_tasks(&mut self, player: &Obj) {
        let to_remove = self
            .tasks
            .iter()
            .filter_map(|(task_id, sr)| {
                (!sr.task.task_start.is_background() && sr.task.player.eq(player))
                    .then_some(*task_id)
            })
            .collect::<Vec<_>>();
        for task_id in to_remove {
            self.remove_task(task_id);
        }
    }
}

fn from_epoch_micros_to_instant(time_since_epoch_micros: u128) -> Instant {
    let time_since_epoch_systime = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

    let time_since_epoch_duration = Duration::from_micros(time_since_epoch_micros as u64);
    let time_since_epoch_instant = Instant::now() - time_since_epoch_systime;
    time_since_epoch_instant + time_since_epoch_duration
}

impl Encode for SuspendedTask {
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        // We only care about the task and wake condition. The session & result sender are not
        // encoded, as they are transient and re-constituted as no-op versions.

        self.wake_condition.encode(encoder)?;
        self.task.encode(encoder)?;
        Ok(())
    }
}

impl<C> Decode<C> for SuspendedTask {
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let wake_condition = WakeCondition::decode(decoder)?;
        let task = Task::decode(decoder)?;
        Ok(SuspendedTask {
            wake_condition,
            task,
            session: Arc::new(NoopClientSession::new()),
            result_sender: None,
        })
    }
}

impl<'de, C> BorrowDecode<'de, C> for SuspendedTask {
    fn borrow_decode<D: BorrowDecoder<'de>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let wake_condition = WakeCondition::borrow_decode(decoder)?;
        let task = Task::borrow_decode(decoder)?;
        Ok(SuspendedTask {
            wake_condition,
            task,
            session: Arc::new(NoopClientSession::new()),
            result_sender: None,
        })
    }
}

impl Encode for WakeCondition {
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        let type_code = self.condition_type();
        type_code.encode(encoder)?;
        match self {
            WakeCondition::Never => Ok(()),
            WakeCondition::Time(t) => {
                // Convert to a time since epoch and encode as micros.
                let time_since_epoch_systime =
                    SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
                let from_now_instant = t.duration_since(Instant::now());
                let time_to_wake = time_since_epoch_systime + from_now_instant;
                time_to_wake.as_micros().encode(encoder)
            }
            WakeCondition::Input(uuid) => uuid.as_u128().encode(encoder),
            WakeCondition::Task(task_id) => task_id.encode(encoder),
        }
    }
}

impl<C> Decode<C> for WakeCondition {
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let type_code: WakeConditionType = Decode::decode(decoder)?;
        match type_code {
            WakeConditionType::Never => Ok(WakeCondition::Never),
            WakeConditionType::Time => {
                let time_since_epoch_micros: u128 = Decode::decode(decoder)?;
                let wake_time = from_epoch_micros_to_instant(time_since_epoch_micros);
                Ok(WakeCondition::Time(wake_time))
            }
            WakeConditionType::Input => {
                let uuid = Uuid::from_u128(Decode::decode(decoder)?);
                Ok(WakeCondition::Input(uuid))
            }
            WakeConditionType::Task => {
                let task_id = TaskId::decode(decoder)?;
                Ok(WakeCondition::Task(task_id))
            }
        }
    }
}

impl<'de, C> BorrowDecode<'de, C> for WakeCondition {
    fn borrow_decode<D: BorrowDecoder<'de>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let type_code: WakeConditionType = Decode::decode(decoder)?;
        match type_code {
            WakeConditionType::Never => Ok(WakeCondition::Never),
            WakeConditionType::Time => {
                let time_since_epoch_micros: u128 = Decode::decode(decoder)?;
                let wake_time = from_epoch_micros_to_instant(time_since_epoch_micros);
                Ok(WakeCondition::Time(wake_time))
            }
            WakeConditionType::Input => {
                let uuid = Uuid::from_u128(Decode::decode(decoder)?);
                Ok(WakeCondition::Input(uuid))
            }
            WakeConditionType::Task => {
                let task_id = TaskId::borrow_decode(decoder)?;
                Ok(WakeCondition::Task(task_id))
            }
        }
    }
}
