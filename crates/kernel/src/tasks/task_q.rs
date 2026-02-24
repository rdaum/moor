// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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
use flume::Sender;
use hierarchical_hash_wheel_timer::wheels::{
    Skip, TimerEntryWithDelay,
    quad_wheel::{PruneDecision, QuadWheelWithOverflow},
};
use minstant::Instant;
use rayon::ThreadPool;
use std::{
    collections::{HashMap, VecDeque},
    hash::BuildHasherDefault,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, SystemTime},
};
use tracing::{error, info, warn};
use uuid::Uuid;

use moor_var::{Obj, Var};

use crate::{tasks::task::Task, vm::extract_anonymous_refs_from_vm_exec_state};

/// Timer entry for the hash wheel timer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TimerEntry {
    task_id: TaskId,
    delay: Duration,
}

impl TimerEntryWithDelay for TimerEntry {
    fn delay(&self) -> Duration {
        self.delay
    }
}
use crate::tasks::task::TaskState;
use crate::tasks::{TaskDescription, TaskNotification, TaskStart, TasksDb};
use moor_common::tasks::{SchedulerError, Session, SessionFactory, TaskId};

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
    /// Thread pool for task execution
    pub(crate) thread_pool: ThreadPool,
    /// Inter-task message queues. Keyed by receiving task_id, shared across active and suspended.
    pub(crate) task_message_queues: HashMap<TaskId, VecDeque<Var>, BuildHasherDefault<AHasher>>,
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
    pub(crate) result_sender: Option<Sender<(TaskId, Result<TaskNotification, SchedulerError>)>>,
}

impl TaskQ {
    pub fn new(suspended: SuspensionQ) -> Self {
        let num_threads = std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(8);
        let thread_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .thread_name(|i| format!("moor-task-pool-{i}"))
            .build()
            .expect("Failed to create thread pool");

        Self {
            active: Default::default(),
            suspended,
            thread_pool,
            task_message_queues: HashMap::default(),
        }
    }

    /// Check if a task exists and return its owner (permissions).
    /// Checks both active and suspended tasks atomically.
    pub(crate) fn task_owner(&self, task_id: TaskId) -> Option<Obj> {
        // Check active tasks first
        if let Some(running) = self.active.get(&task_id) {
            return Some(running.player);
        }

        // Check suspended tasks
        self.suspended.task_owner(task_id)
    }

    /// Collect tasks that need to be woken up by timer, pull them from our suspended list, and
    /// return them. Other wake paths are event-driven through the immediate wake queue.
    pub(crate) fn collect_wake_tasks(&mut self) -> Option<Vec<SuspendedTask>> {
        let mut to_wake: Option<Vec<TaskId>> = None;

        // 1. Advance timer wheel based on elapsed time and collect expired timers
        // (Always advance the timer wheel to maintain accurate timing, even when no tasks are suspended)
        if let Some(expired_timers) = self.suspended.advance_timer_wheel() {
            to_wake
                .get_or_insert_with(Vec::new)
                .extend(expired_timers.into_iter().map(|e| e.task_id));
        }

        if self.suspended.tasks.is_empty() {
            return None;
        }
        let to_wake = to_wake?;
        let tasks: Vec<_> = to_wake
            .into_iter()
            .filter_map(|task_id| self.suspended.remove_task(task_id))
            .collect();

        if tasks.is_empty() { None } else { Some(tasks) }
    }

    /// Collect anonymous object references from all suspended tasks
    pub(crate) fn collect_anonymous_object_references(&self) -> std::collections::HashSet<Obj> {
        let mut refs = std::collections::HashSet::new();

        // Scan all suspended tasks
        for suspended_task in self.suspended.tasks.values() {
            // Scan the current VM state
            let current_vm_state = suspended_task.task.vm_host.vm_exec_state();
            extract_anonymous_refs_from_vm_exec_state(current_vm_state, &mut refs);

            // Scan the retry state
            extract_anonymous_refs_from_vm_exec_state(&suspended_task.task.retry_state, &mut refs);
        }

        refs
    }

    /// Deliver a message to a task's incoming queue. If the target task is suspended
    /// waiting for messages (WakeCondition::TaskMessage), trigger an immediate wake.
    pub(crate) fn deliver_message(&mut self, target_task_id: TaskId, value: Var) {
        self.task_message_queues
            .entry(target_task_id)
            .or_default()
            .push_back(value);

        // If the target is suspended and waiting for messages, wake it immediately
        if self
            .suspended
            .message_waiting_tasks
            .contains(&target_task_id)
        {
            self.suspended.enqueue_immediate_wake(target_task_id);
        }
    }

    /// Drain all messages from a task's queue, returning them.
    pub(crate) fn drain_messages(&mut self, task_id: TaskId) -> Vec<Var> {
        self.task_message_queues
            .remove(&task_id)
            .map(|q| q.into_iter().collect())
            .unwrap_or_default()
    }

    /// Return the current number of messages in a task's mailbox.
    pub(crate) fn mailbox_len(&self, task_id: TaskId) -> usize {
        self.task_message_queues
            .get(&task_id)
            .map_or(0, |q| q.len())
    }

    /// Remove a task's message queue (e.g., when task is killed/completed).
    pub(crate) fn remove_message_queue(&mut self, task_id: TaskId) {
        self.task_message_queues.remove(&task_id);
    }

    /// Trigger database compaction to reclaim space and reduce journal size.
    pub fn compact(&self) {
        self.suspended.compact();
    }
}

/// State a suspended task sits in inside the `suspended` side of the task queue.
/// When tasks are not running they are moved into these.
pub struct SuspendedTask {
    /// Timestamp when this task entered the suspended queue.
    pub enqueued_at: Instant,
    pub wake_condition: WakeCondition,
    pub task: Box<Task>,
    pub session: Arc<dyn Session>,
    pub result_sender: Option<Sender<(TaskId, Result<TaskNotification, SchedulerError>)>>,
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
    /// Wake immediately with optional return value. Some(val) for tasks that performed a commit(),
    /// None for brand new tasks that haven't executed yet.
    Immediate(Option<Var>),
    /// Wake when a worker responds to this request id
    Worker(Uuid),
    /// Wake when garbage collection completes
    GCComplete,
    /// Wake for retry after transaction conflict - includes backoff time
    Retry(Instant),
    /// Wake when a task message is delivered, or at deadline (whichever first)
    TaskMessage(Instant),
}

#[repr(u8)]
pub enum WakeConditionType {
    Never = 0,
    Time = 1,
    Input = 2,
    Task = 3,
    Immediate = 4,
    Worker = 5,
    GCComplete = 6,
    Retry = 7,
    TaskMessage = 8,
}

impl WakeCondition {
    pub fn condition_type(&self) -> WakeConditionType {
        match self {
            WakeCondition::Never => WakeConditionType::Never,
            WakeCondition::Time(_) => WakeConditionType::Time,
            WakeCondition::Input(_) => WakeConditionType::Input,
            WakeCondition::Task(_) => WakeConditionType::Task,
            WakeCondition::Immediate(_) => WakeConditionType::Immediate,
            WakeCondition::Worker(_) => WakeConditionType::Worker,
            WakeCondition::GCComplete => WakeConditionType::GCComplete,
            WakeCondition::Retry(_) => WakeConditionType::Retry,
            WakeCondition::TaskMessage(_) => WakeConditionType::TaskMessage,
        }
    }
}

/// Ties the local storage for suspended tasks in with a reference to the tasks DB, to allow for
/// keeping them in sync.
pub struct SuspensionQ {
    /// All suspended tasks - the master storage
    pub(crate) tasks: HashMap<TaskId, SuspendedTask, BuildHasherDefault<AHasher>>,

    /// Time-based tasks use a hash wheel timer (O(1) amortized)
    timer_wheel: QuadWheelWithOverflow<TimerEntry>,

    /// Last time we advanced the timer wheel (for tracking elapsed time)
    last_timer_advance: Option<Instant>,

    /// Queue for tasks that should wake immediately (O(1) push/pop)
    immediate_wake_queue: VecDeque<TaskId>,

    /// Tasks waiting for other tasks to complete (O(1) lookup by dependency)
    task_dependencies: HashMap<TaskId, Vec<TaskId>, BuildHasherDefault<AHasher>>,

    /// Tasks waiting for input by request ID (O(1) lookup)
    input_requests: HashMap<uuid::Uuid, TaskId, BuildHasherDefault<AHasher>>,

    /// Tasks waiting for worker responses by request ID (O(1) lookup)
    worker_requests: HashMap<uuid::Uuid, TaskId, BuildHasherDefault<AHasher>>,

    /// Tasks waiting for GC completion
    gc_waiting_tasks: Vec<TaskId>,

    /// Tasks waiting for retry after transaction conflict
    retry_tasks: Vec<TaskId>,

    /// Tasks waiting for inter-task messages (via task_recv with timeout)
    message_waiting_tasks: Vec<TaskId>,

    tasks_database: Box<dyn TasksDb>,
}

impl SuspensionQ {
    pub fn new(tasks_database: Box<dyn TasksDb>) -> Self {
        Self {
            tasks: Default::default(),
            // Create timer wheel with pruner that keeps all entries
            timer_wheel: QuadWheelWithOverflow::new(|_| PruneDecision::Keep),
            last_timer_advance: Some(Instant::now()),
            immediate_wake_queue: VecDeque::new(),
            task_dependencies: HashMap::default(),
            input_requests: HashMap::default(),
            worker_requests: HashMap::default(),
            gc_waiting_tasks: Vec::new(),
            retry_tasks: Vec::new(),
            message_waiting_tasks: Vec::new(),
            tasks_database,
        }
    }

    /// Check if a suspended task exists and return its owner (perms).
    pub(crate) fn task_owner(&self, task_id: TaskId) -> Option<Obj> {
        self.tasks.get(&task_id).map(|st| st.task.perms)
    }

    /// Queue a task for immediate wake.
    #[inline]
    pub(crate) fn enqueue_immediate_wake(&mut self, task_id: TaskId) {
        self.immediate_wake_queue.push_back(task_id);
    }

    /// Pop the next task queued for immediate wake.
    #[inline]
    pub(crate) fn pop_immediate_wake(&mut self) -> Option<TaskId> {
        self.immediate_wake_queue.pop_front()
    }

    /// Queue all tasks waiting on `dependency_task_id` for immediate wake.
    pub(crate) fn enqueue_dependents_for(&mut self, dependency_task_id: TaskId) {
        let Some(dependents) = self.task_dependencies.remove(&dependency_task_id) else {
            return;
        };
        for task_id in dependents {
            self.enqueue_immediate_wake(task_id);
        }
    }

    /// Queue all tasks waiting for GC completion for immediate wake.
    pub(crate) fn enqueue_gc_waiting_tasks(&mut self) {
        let waiting_tasks = std::mem::take(&mut self.gc_waiting_tasks);
        for task_id in waiting_tasks {
            self.enqueue_immediate_wake(task_id);
        }
    }

    /// Advance the timer wheel based on elapsed time and return expired entries.
    fn advance_timer_wheel(&mut self) -> Option<Vec<TimerEntry>> {
        let now = Instant::now();
        let last_advance = self.last_timer_advance.unwrap_or(now);

        if now <= last_advance {
            return None;
        }

        let elapsed_millis = now.duration_since(last_advance).as_millis() as u32;
        let mut millis_remaining = elapsed_millis;

        let mut expired_entries = None;

        while millis_remaining > 0 {
            match self.timer_wheel.can_skip() {
                Skip::Empty => {
                    // Wheel is empty - no timers, nothing to tick
                    self.timer_wheel.skip(millis_remaining);
                    break;
                }
                Skip::Millis(skippable) => {
                    let to_skip = skippable.min(millis_remaining);
                    self.timer_wheel.skip(to_skip);
                    millis_remaining -= to_skip;
                }
                Skip::None => {
                    // Next tick has expiring timers, must tick
                    expired_entries
                        .get_or_insert_with(Vec::new)
                        .extend(self.timer_wheel.tick());
                    millis_remaining -= 1;
                }
            }
        }

        self.last_timer_advance = Some(last_advance + Duration::from_millis(elapsed_millis as u64));

        expired_entries
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

            let task_id = task.task.task_id;
            match &task.wake_condition {
                WakeCondition::Time(wake_time) => {
                    let now = Instant::now();
                    let inserted = *wake_time > now && {
                        let delay = wake_time.duration_since(now);
                        let timer_entry = TimerEntry { task_id, delay };
                        self.timer_wheel
                            .insert_with_delay(timer_entry, delay)
                            .is_ok()
                    };
                    if !inserted {
                        // Past deadline or timer expired - wake immediately
                        self.enqueue_immediate_wake(task_id);
                    }
                }
                WakeCondition::Immediate(_) => {
                    self.enqueue_immediate_wake(task_id);
                }
                WakeCondition::Task(dependency_task_id) => {
                    self.task_dependencies
                        .entry(*dependency_task_id)
                        .or_default()
                        .push(task_id);
                }
                WakeCondition::Input(input_request_id) => {
                    self.input_requests.insert(*input_request_id, task_id);
                }
                WakeCondition::Worker(worker_request_id) => {
                    self.worker_requests.insert(*worker_request_id, task_id);
                }
                WakeCondition::Never => {
                    //
                }
                WakeCondition::GCComplete => {
                    self.gc_waiting_tasks.push(task_id);
                }
                WakeCondition::Retry(wake_time) => {
                    // Retry tasks shouldn't be persisted, but handle gracefully if loaded
                    self.retry_tasks.push(task_id);
                    let now = Instant::now();
                    let inserted = *wake_time > now && {
                        let delay = wake_time.duration_since(now);
                        let timer_entry = TimerEntry { task_id, delay };
                        self.timer_wheel
                            .insert_with_delay(timer_entry, delay)
                            .is_ok()
                    };
                    if !inserted {
                        self.enqueue_immediate_wake(task_id);
                    }
                }
                WakeCondition::TaskMessage(wake_time) => {
                    self.message_waiting_tasks.push(task_id);
                    let now = Instant::now();
                    let inserted = *wake_time > now && {
                        let delay = wake_time.duration_since(now);
                        let timer_entry = TimerEntry { task_id, delay };
                        self.timer_wheel
                            .insert_with_delay(timer_entry, delay)
                            .is_ok()
                    };
                    if !inserted {
                        self.enqueue_immediate_wake(task_id);
                    }
                }
            }

            self.tasks.insert(task_id, task);
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
        result_sender: Option<Sender<(TaskId, Result<TaskNotification, SchedulerError>)>>,
    ) {
        let task_id = task.task_id;
        let now = Instant::now();

        // Add to appropriate storage based on wake condition
        let should_persist = match &wake_condition {
            WakeCondition::Time(wake_time) => {
                let inserted = *wake_time > now && {
                    let delay = wake_time.duration_since(now);
                    let timer_entry = TimerEntry { task_id, delay };
                    self.timer_wheel
                        .insert_with_delay(timer_entry, delay)
                        .is_ok()
                };
                if !inserted {
                    // Past deadline or timer expired - wake immediately
                    self.enqueue_immediate_wake(task_id);
                }
                inserted // Persist only if successfully inserted into timer wheel
            }
            WakeCondition::Immediate(_) => {
                self.enqueue_immediate_wake(task_id);
                false // Skip database persistence for immediate wake
            }
            WakeCondition::Task(dependency_task_id) => {
                self.task_dependencies
                    .entry(*dependency_task_id)
                    .or_default()
                    .push(task_id);
                true
            }
            WakeCondition::Input(input_request_id) => {
                self.input_requests.insert(*input_request_id, task_id);
                // TODO No point in saving, because we'll probably never get the input, I think. But we
                //  could re-evaluate this
                false
            }
            WakeCondition::Worker(worker_request_id) => {
                self.worker_requests.insert(*worker_request_id, task_id);
                true
            }
            WakeCondition::Never => true,
            WakeCondition::GCComplete => true,
            WakeCondition::Retry(wake_time) => {
                self.retry_tasks.push(task_id);
                let inserted = *wake_time > now && {
                    let delay = wake_time.duration_since(now);
                    let timer_entry = TimerEntry { task_id, delay };
                    self.timer_wheel
                        .insert_with_delay(timer_entry, delay)
                        .is_ok()
                };
                if !inserted {
                    // Past deadline - wake immediately for retry
                    self.enqueue_immediate_wake(task_id);
                }
                false // Don't persist retry tasks - they're transient
            }
            WakeCondition::TaskMessage(wake_time) => {
                self.message_waiting_tasks.push(task_id);
                let inserted = *wake_time > now && {
                    let delay = wake_time.duration_since(now);
                    let timer_entry = TimerEntry { task_id, delay };
                    self.timer_wheel
                        .insert_with_delay(timer_entry, delay)
                        .is_ok()
                };
                if !inserted {
                    // Past deadline - wake immediately
                    self.enqueue_immediate_wake(task_id);
                }
                true // Persist - message queue state should survive restarts
            }
        };

        let sr = SuspendedTask {
            enqueued_at: now,
            wake_condition,
            task,
            session,
            result_sender,
        };

        if should_persist && let Err(e) = self.tasks_database.save_task(&sr) {
            error!(?e, "Could not save suspended task");
        }

        self.tasks.insert(task_id, sr);
    }

    /// Remove a task from the set of suspended tasks.
    pub(crate) fn remove_task(&mut self, task_id: TaskId) -> Option<SuspendedTask> {
        let task = self.tasks.remove(&task_id);
        if let Some(ref suspended_task) = task {
            // Clean up from all data structures based on wake condition
            match &suspended_task.wake_condition {
                WakeCondition::Time(_) => {
                    // Timer wheel handles removal automatically when tasks expire
                    // No manual cleanup needed
                }
                WakeCondition::Immediate(_) => {
                    // No cleanup needed for queue - scheduler will ignore stale task ids
                    // if task is no longer in the suspended tasks map.
                }
                WakeCondition::Task(dependency_task_id) => {
                    // Remove from task dependencies
                    if let Some(dependents) = self.task_dependencies.get_mut(dependency_task_id) {
                        dependents.retain(|&id| id != task_id);
                        if dependents.is_empty() {
                            self.task_dependencies.remove(dependency_task_id);
                        }
                    }
                }
                WakeCondition::Input(input_request_id) => {
                    self.input_requests.remove(input_request_id);
                }
                WakeCondition::Worker(worker_request_id) => {
                    self.worker_requests.remove(worker_request_id);
                }
                WakeCondition::Never => {
                    //
                }
                WakeCondition::GCComplete => {
                    self.gc_waiting_tasks.retain(|&id| id != task_id);
                }
                WakeCondition::Retry(_) => {
                    self.retry_tasks.retain(|&id| id != task_id);
                }
                WakeCondition::TaskMessage(_) => {
                    self.message_waiting_tasks.retain(|&id| id != task_id);
                }
            }

            // Try to delete from database - will be a no-op for tasks that were never persisted
            let _ = self.tasks_database.delete_task(task_id);
        }
        task
    }

    /// Remove a task permanently from suspension and wake tasks depending on it.
    pub(crate) fn remove_task_terminal(&mut self, task_id: TaskId) -> Option<SuspendedTask> {
        let task = self.remove_task(task_id);
        if task.is_some() {
            self.enqueue_dependents_for(task_id);
        }
        task
    }

    /// Synchronize the suspended tasks with the tasks database. Called on shutdown.
    pub(crate) fn save_tasks(&self) {
        for (_, st) in self.tasks.iter() {
            // Skip retry tasks - they're transient and their transaction context
            // would be invalid after restart anyway
            if matches!(st.wake_condition, WakeCondition::Retry(_)) {
                continue;
            }
            if let Err(e) = self.tasks_database.save_task(st) {
                error!(?e, "Could not save suspended task");
            }
        }
    }

    /// Pull a task from the suspended list that is waiting for input, for the given player.
    /// Uses O(1) lookup instead of O(n) linear scan.
    pub(crate) fn pull_task_for_input(
        &mut self,
        input_request_id: Uuid,
        player: &Obj,
    ) -> Option<SuspendedTask> {
        // O(1) lookup by input request ID
        let &task_id = self.input_requests.get(&input_request_id)?;

        // Verify the task still exists and check permissions
        let suspended_task = self.tasks.get(&task_id)?;
        if suspended_task.task.perms.ne(player) {
            warn!(
                ?task_id,
                ?input_request_id,
                ?player,
                "Task input request received for wrong player"
            );
            return None;
        }

        // Remove and return the task
        self.remove_task(task_id)
    }

    /// Pull a task from the suspended list that is waiting for a worker response.
    /// Uses O(1) lookup instead of O(n) linear scan.
    pub(crate) fn pull_task_for_worker(
        &mut self,
        worker_request_id: Uuid,
    ) -> Option<SuspendedTask> {
        // O(1) lookup by worker request ID
        let &task_id = self.worker_requests.get(&worker_request_id)?;

        // Remove and return the task
        self.remove_task(task_id)
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
            // For tasks in Created state (not yet started), we need to extract info from TaskStart
            // because the vm_host stack is still empty (setup_task_start hasn't been called yet)
            let (verb_name, verb_definer, line_number, this) = match &sr.task.state {
                TaskState::Pending(task_start) => {
                    // Extract info from the TaskStart since vm_host isn't initialized yet
                    match task_start {
                        TaskStart::StartFork { fork_request, .. } => {
                            let activation = &fork_request.activation;
                            (
                                activation.verb_name,
                                activation.verb_definer(),
                                activation.frame.find_line_no().unwrap_or(0),
                                activation.this.clone(),
                            )
                        }
                        _ => {
                            // For other task types in Created state, we can't get this info yet
                            // Use placeholder values
                            (
                                moor_var::Symbol::mk(""),
                                moor_var::NOTHING,
                                0,
                                moor_var::v_none(),
                            )
                        }
                    }
                }
                TaskState::Prepared(_) => {
                    // Prefer the top non-builtin frame so builtins like suspend() don't mask
                    // the calling verb in queued_tasks().
                    let activation = sr
                        .task
                        .vm_host
                        .vm_exec_state()
                        .stack
                        .iter()
                        .rev()
                        .find(|a| !a.is_builtin_frame());
                    if let Some(activation) = activation {
                        let line_number = activation.frame.find_line_no().unwrap_or(0);
                        (
                            activation.verb_name,
                            activation.verb_definer(),
                            line_number,
                            activation.this.clone(),
                        )
                    } else {
                        // For prepared tasks, vm_host stack MUST be non-empty
                        let Some(((verb_name, verb_definer), (line_number, this))) = sr
                            .task
                            .vm_host
                            .verb_name()
                            .zip(sr.task.vm_host.verb_definer())
                            .zip(sr.task.vm_host.line_number().zip(sr.task.vm_host.this()))
                        else {
                            error!(
                                task_id = sr.task.task_id,
                                "Prepared task has empty activation stack - skipping"
                            );
                            continue;
                        };
                        (verb_name, verb_definer, line_number, this)
                    }
                }
            };

            tasks.push(TaskDescription {
                task_id: sr.task.task_id,
                start_time,
                permissions: sr.task.perms,
                verb_name,
                verb_definer,
                line_number,
                this,
            });
        }
        tasks
    }

    /// Check if the given sender has permission to operate on the suspended task.
    /// Uses LambdaMOO's dual permission model:
    /// - I/O tasks (waiting for input): sender must match task.player
    /// - Computational tasks: sender must match task.perms
    ///   If `filter_input` is true, filter out Input-waiting tasks.
    pub(crate) fn perms_check(&self, task_id: TaskId, sender: Obj, filter_input: bool) -> bool {
        let Some(sr) = self.tasks.get(&task_id) else {
            return false;
        };

        if filter_input && matches!(sr.wake_condition, WakeCondition::Input(_)) {
            return false;
        }

        // LambdaMOO dual model: I/O tasks use player, computational tasks use perms
        let required_perm = if matches!(sr.wake_condition, WakeCondition::Input(_)) {
            sr.task.player // Session owner for I/O
        } else {
            sr.task.perms // Programmer for computational
        };

        sender == required_perm
    }

    /// Remove all non-background tasks for the given player.
    pub(crate) fn prune_foreground_tasks(&mut self, player: &Obj) {
        let to_remove = self
            .tasks
            .iter()
            .filter_map(|(task_id, sr)| {
                (!sr.task.state.is_background() && sr.task.player.eq(player)).then_some(*task_id)
            })
            .collect::<Vec<_>>();
        for task_id in to_remove {
            self.remove_task_terminal(task_id);
        }
    }

    /// Trigger database compaction to reclaim space and reduce journal size.
    pub(crate) fn compact(&self) {
        self.tasks_database.compact();
    }
}
