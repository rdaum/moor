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
use flume::Sender;
use gdt_cpus;
use minstant::Instant;
use rayon::ThreadPool;
use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{error, info, warn};
use uuid::Uuid;

use moor_var::Obj;

use crate::tasks::task::Task;
use crate::tasks::{TaskDescription, TaskResult, TaskStart, TasksDb};
use moor_common::tasks::{NoopClientSession, Session, SessionFactory};
use moor_common::tasks::{SchedulerError, TaskId};

/// The internal state of the task queue.
pub struct TaskQ {
    /// Information about the active, running tasks. The actual `Task` is owned by the task thread
    /// and this is just an information, and control record for communicating with it.
    pub(crate) active: HashMap<TaskId, RunningTask, BuildHasherDefault<AHasher>>,
    /// Tasks in various types of suspension:
    ///     Forked background tasks that will execute someday
    ///     Suspended foreground tasks that are either indefinitely suspended or will execute someday
    ///     Suspended tasks waiting for input from the player or a task id to complete
    pub(crate) suspended: SuspensionQ,
    /// Thread pool for task execution instead of creating new threads for each task
    pub(crate) thread_pool: ThreadPool,
}

/// Scheduler-side per-task record. Lives in the scheduler thread and owned by the scheduler and
/// not shared elsewhere.
/// The actual `Task` is owned by the task thread until it is suspended or completed.
/// (When suspended it is moved into a `SuspendedTask` in the `.suspended` list)
pub(crate) struct RunningTask {
    /// For which player this task is running on behalf of.
    pub(crate) player: Obj,
    /// What triggered this task to start.
    pub(crate) task_start: TaskStart,
    /// A kill switch to signal the task to stop. True means the VM execution thread should stop
    /// as soon as it can.
    pub(crate) kill_switch: Arc<AtomicBool>,
    /// The connection-session for this task.
    pub(crate) session: Arc<dyn Session>,
    /// A mailbox to deliver the result of the task to a waiting party with a subscription, if any.
    pub(crate) result_sender: Option<Sender<(TaskId, Result<TaskResult, SchedulerError>)>>,
}

fn none_or_push(vec: &mut Option<Vec<TaskId>>, task: TaskId) {
    if let Some(v) = vec {
        v.push(task);
    } else {
        *vec = Some(vec![task]);
    }
}

impl TaskQ {
    pub fn new(suspended: SuspensionQ) -> Self {
        let num_threads = (gdt_cpus::num_logical_cores().unwrap_or(8) * 2).min(64);
        let thread_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .thread_name(|i| format!("moor-task-pool-{i}"))
            .build()
            .expect("Failed to create thread pool");

        Self {
            active: Default::default(),
            suspended,
            thread_pool,
        }
    }

    /// Collect tasks that need to be woken up, pull them from our suspended list, and return them.
    pub(crate) fn collect_wake_tasks(&mut self) -> Option<Vec<SuspendedTask>> {
        if self.suspended.tasks.is_empty() {
            return None;
        }
        let now = Instant::now();
        let mut to_wake = None;
        for task in self.suspended.tasks.values() {
            match task.wake_condition {
                WakeCondition::Time(t) => {
                    if t <= now {
                        none_or_push(&mut to_wake, task.task.task_id);
                    }
                }
                WakeCondition::Task(task_id) => {
                    if !self.suspended.tasks.contains_key(&task_id)
                        && !self.active.contains_key(&task_id)
                    {
                        none_or_push(&mut to_wake, task.task.task_id);
                    }
                }
                WakeCondition::Immedate => {
                    none_or_push(&mut to_wake, task.task.task_id);
                }
                _ => {}
            }
        }
        let to_wake = to_wake?;
        let mut tasks = vec![];
        for task_id in to_wake {
            let sr = self.suspended.tasks.remove(&task_id).unwrap();
            tasks.push(sr);
        }
        Some(tasks)
    }
}

/// State a suspended task sits in inside the `suspended` side of the task queue.
/// When tasks are not running they are moved into these.
pub struct SuspendedTask {
    pub wake_condition: WakeCondition,
    pub task: Box<Task>,
    pub session: Arc<dyn Session>,
    pub result_sender: Option<Sender<(TaskId, Result<TaskResult, SchedulerError>)>>,
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
    /// Wake immediately. This is used for tasks that performed a commit().
    Immedate,
    /// Wake when a worker responds to this request id
    Worker(Uuid),
}

#[repr(u8)]
#[derive(Encode, Decode, Debug)]
pub enum WakeConditionType {
    Never = 0,
    Time = 1,
    Input = 2,
    Task = 3,
    Immediate = 4,
    Worker = 5,
}

impl WakeCondition {
    pub fn condition_type(&self) -> WakeConditionType {
        match self {
            WakeCondition::Never => WakeConditionType::Never,
            WakeCondition::Time(_) => WakeConditionType::Time,
            WakeCondition::Input(_) => WakeConditionType::Input,
            WakeCondition::Task(_) => WakeConditionType::Task,
            WakeCondition::Immedate => WakeConditionType::Immediate,
            WakeCondition::Worker(_) => WakeConditionType::Worker,
        }
    }
}

/// Ties the local storage for suspended tasks in with a reference to the tasks DB, to allow for
/// keeping them in sync.
pub struct SuspensionQ {
    pub(crate) tasks: HashMap<TaskId, SuspendedTask, BuildHasherDefault<AHasher>>,
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
        task: Box<Task>,
        session: Arc<dyn Session>,
        result_sender: Option<Sender<(TaskId, Result<TaskResult, SchedulerError>)>>,
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

    /// Pull a task from the suspended list that is waiting for input, for the given player.
    pub(crate) fn pull_task_for_input(
        &mut self,
        input_request_id: Uuid,
        player: &Obj,
    ) -> Option<SuspendedTask> {
        let (task_id, perms) = self.tasks.iter().find_map(|(task_id, sr)| {
            if let WakeCondition::Input(request_id) = &sr.wake_condition {
                if *request_id == input_request_id {
                    Some((*task_id, sr.task.perms))
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

    pub(crate) fn pull_task_for_worker(
        &mut self,
        worker_request_id: Uuid,
    ) -> Option<SuspendedTask> {
        let (task_id, _) = self.tasks.iter().find_map(|(task_id, sr)| {
            if let WakeCondition::Worker(request_id) = &sr.wake_condition {
                if *request_id == worker_request_id {
                    Some((*task_id, sr.task.perms))
                } else {
                    None
                }
            } else {
                None
            }
        })?;

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
                permissions: sr.task.perms,
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
        Some(sr.task.perms)
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
    // Convert stored epoch micros back to SystemTime
    let stored_system_time =
        UNIX_EPOCH + Duration::from_micros(time_since_epoch_micros.min(u64::MAX as u128) as u64);

    // Calculate how far in the future (or past) this time is relative to now
    let now_system = SystemTime::now();
    let now_instant = Instant::now();

    match stored_system_time.cmp(&now_system) {
        std::cmp::Ordering::Greater => {
            // Future time - add the difference
            let time_diff = stored_system_time
                .duration_since(now_system)
                .unwrap_or(Duration::ZERO);
            now_instant + time_diff
        }
        std::cmp::Ordering::Less => {
            // Past time - subtract the difference (but don't go negative)
            let time_diff = now_system
                .duration_since(stored_system_time)
                .unwrap_or(Duration::ZERO);
            now_instant.checked_sub(time_diff).unwrap_or(now_instant)
        }
        std::cmp::Ordering::Equal => {
            // Same time
            now_instant
        }
    }
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
        let task = Box::new(Task::decode(decoder)?);
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
        let task = Box::new(Task::borrow_decode(decoder)?);
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
                // Convert Instant to absolute epoch time for storage
                let now_system = SystemTime::now();
                let now_instant = Instant::now();

                let epoch_time = if *t >= now_instant {
                    // Future time - add the difference
                    let time_diff = t.duration_since(now_instant);
                    now_system + time_diff
                } else {
                    // Past time - subtract the difference (but don't go before epoch)
                    let time_diff = now_instant.duration_since(*t);
                    now_system.checked_sub(time_diff).unwrap_or(UNIX_EPOCH)
                };

                let time_since_epoch = epoch_time
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or(Duration::ZERO);
                time_since_epoch.as_micros().encode(encoder)
            }
            WakeCondition::Input(uuid) => uuid.as_u128().encode(encoder),
            WakeCondition::Task(task_id) => task_id.encode(encoder),
            WakeCondition::Worker(worker_request_id) => worker_request_id.as_u128().encode(encoder),
            WakeCondition::Immedate => Ok(()),
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
            WakeConditionType::Worker => {
                let worker_request_id = Uuid::from_u128(Decode::decode(decoder)?);
                Ok(WakeCondition::Worker(worker_request_id))
            }
            WakeConditionType::Immediate => Ok(WakeCondition::Immedate),
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
            WakeConditionType::Worker => {
                let worker_request_id = Uuid::from_u128(Decode::decode(decoder)?);
                Ok(WakeCondition::Worker(worker_request_id))
            }
            WakeConditionType::Immediate => Ok(WakeCondition::Immedate),
        }
    }
}
