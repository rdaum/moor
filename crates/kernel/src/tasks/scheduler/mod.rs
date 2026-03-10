// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

mod scheduler_client_msg;
mod scheduler_config;
mod scheduler_gc;
mod scheduler_task_msg;
mod scheduler_task_ops;
mod task_q_ops;

use crate::{
    task_context::TaskGuard,
    tasks::checkpoint::{CheckpointMode, start_checkpoint},
};
use flume::{Receiver, Sender};
use moor_common::util::{Deadline, Instant, Timestamp};
use rand::Rng;
use std::{
    sync::{
        Arc, LazyLock,
        atomic::{AtomicBool, Ordering},
    },
    thread::yield_now,
    time::{Duration, SystemTime},
};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use moor_common::model::{CommitResult, ObjectRef, Perms, WorldState};
use moor_compiler::to_literal;
use moor_db::Database;

use crate::{
    config::Config,
    tasks::{
        DEFAULT_BG_SECONDS, DEFAULT_BG_TICKS, DEFAULT_COMPACT_INTERVAL_SECONDS, DEFAULT_FG_SECONDS,
        DEFAULT_FG_TICKS, DEFAULT_GC_INTERVAL_SECONDS, DEFAULT_MAX_STACK_DEPTH,
        DEFAULT_MAX_TASK_MAILBOX, DEFAULT_MAX_TASK_RETRIES, ServerOptions, TaskHandle,
        TaskNotification, TaskStart,
        gc_thread::spawn_gc_mark_phase,
        sched_counters,
        scheduler_client::{SchedulerClient, SchedulerClientMsg},
        task::Task,
        task_q::{RunningTask, SuspendedTask, SuspensionQ, TaskQ, WakeCondition},
        task_scheduler_client::{TaskControlMsg, TaskSchedulerClient, WorkerInfo},
        tasks_db::TasksDb,
        workers::{WorkerRequest, WorkerResponse},
        world_state_action::{WorldStateAction, WorldStateResponse},
        world_state_executor::{WorldStateActionExecutor, match_object_ref},
    },
    trace_task_create_command, trace_task_create_eval, trace_task_create_verb,
    vm::{Fork, TaskSuspend, builtins::BuiltinRegistry},
};

#[cfg(feature = "trace_events")]
use crate::trace_task_resume;

use moor_common::{
    tasks::{
        AbortLimitReason, CommandError, Event, NarrativeEvent, SchedulerError,
        SchedulerError::{
            CommandExecutionError, InputRequestNotFound, TaskAbortedCancelled, TaskAbortedError,
            TaskAbortedException, TaskAbortedLimit,
        },
        Session, SessionFactory, SystemControl, TaskId, WorkerError,
    },
    threading::{
        TaskPoolAffinityConfig, set_current_thread_background_priority,
        set_task_pool_affinity_config, spawn_perf,
    },
    util::{
        PerfCounter, PerfIntensity, PerfTimerGuard, perf_timing_policy, set_perf_timing_policy,
    },
};
use moor_objdef::{collect_object, collect_object_definitions, dump_object, extract_index_names};
use moor_var::{
    E_EXEC, E_INVARG, E_INVIND, E_PERM, E_QUOTA, E_TYPE, Error, List, NOTHING, Obj, SYSTEM_OBJECT,
    Symbol, Var, v_bool_int, v_empty_str, v_err, v_error, v_int, v_obj, v_str,
};
use std::collections::HashMap;

/// Action to take when resuming a suspended task
#[derive(Debug, Clone)]
pub enum ResumeAction {
    /// Resume with a return value (normal case)
    Return(Var),
    /// Resume and immediately raise an error
    Raise(Error),
}

/// Responsible for the dispatching, control, and accounting of tasks in the system.
/// There should be only one scheduler per server.
pub struct Scheduler {
    pub(super) version: semver::Version,

    pub(super) task_control_sender: Sender<(TaskId, TaskControlMsg)>,
    pub(super) task_control_receiver: Receiver<(TaskId, TaskControlMsg)>,

    pub(super) scheduler_sender: Sender<SchedulerClientMsg>,
    pub(super) scheduler_receiver: Receiver<SchedulerClientMsg>,

    pub(super) config: Arc<Config>,

    pub(super) running: bool,
    pub(super) database: Box<dyn Database>,
    pub(super) next_task_id: usize,

    pub(super) server_options: ServerOptions,

    pub(super) builtin_registry: BuiltinRegistry,

    pub(super) system_control: Arc<dyn SystemControl>,

    pub(super) worker_request_send: Option<Sender<WorkerRequest>>,
    pub(super) worker_request_recv: Option<Receiver<WorkerResponse>>,

    /// The internal task queue which holds our suspended tasks, and control records for actively
    /// running tasks.
    /// This is in a lock to allow interior mutability for the scheduler loop, but is only ever
    /// accessed by the scheduler thread.
    pub(super) task_q: TaskQ,

    /// Anonymous object garbage collection flag
    pub(super) gc_collection_in_progress: bool,
    /// Flag indicating concurrent GC mark phase is in progress
    pub(super) gc_mark_in_progress: bool,
    /// Flag indicating GC sweep phase is in progress (blocks new tasks)
    pub(super) gc_sweep_in_progress: bool,
    /// Flag to force GC on next opportunity (set by gc_collect() builtin)
    pub(super) gc_force_collect: bool,
    /// Counter tracking the number of GC cycles completed
    pub(super) gc_cycle_count: u64,
    /// Time of last GC cycle (for interval-based collection)
    pub(super) gc_last_cycle_time: std::time::Instant,
    /// Transaction timestamp (monotonically incrementing) of the last mutating task/transaction
    pub(super) last_mutation_timestamp: Option<u64>,

    /// Tracks whether a checkpoint operation is currently in progress to prevent overlapping checkpoints
    pub(super) checkpoint_in_progress: Arc<AtomicBool>,

    /// Time of last tasks DB compaction (independent of GC)
    pub(super) last_compact_time: std::time::Instant,

    /// Buffered inter-task messages awaiting commit. Keyed by sending task_id.
    /// Delivered to target queues when the sending task commits; discarded on abort/conflict.
    pub(super) pending_task_sends: HashMap<TaskId, Vec<(TaskId, Var)>>,
}

impl Scheduler {
    pub fn new(
        version: semver::Version,
        database: Box<dyn Database>,
        tasks_database: Box<dyn TasksDb>,
        config: Arc<Config>,
        system_control: Arc<dyn SystemControl>,
        worker_request_send: Option<Sender<WorkerRequest>>,
        worker_request_recv: Option<Receiver<WorkerResponse>>,
    ) -> Self {
        let mut affinity_config = TaskPoolAffinityConfig::default();
        if let Some(pinning_mode) = config.runtime.task_pool_pinning {
            affinity_config.pinning_mode = pinning_mode;
        }
        affinity_config.service_perf_cores = config.runtime.service_perf_cores;
        set_task_pool_affinity_config(affinity_config);

        let (task_control_sender, task_control_receiver) = flume::unbounded();
        let (scheduler_sender, scheduler_receiver) = flume::unbounded();
        let suspension_q = SuspensionQ::new(tasks_database);
        let task_q = TaskQ::new(suspension_q);
        let default_server_options = ServerOptions {
            bg_seconds: DEFAULT_BG_SECONDS,
            bg_ticks: DEFAULT_BG_TICKS,
            fg_seconds: DEFAULT_FG_SECONDS,
            fg_ticks: DEFAULT_FG_TICKS,
            max_stack_depth: DEFAULT_MAX_STACK_DEPTH,
            dump_interval: None,
            gc_interval: None,
            max_task_retries: DEFAULT_MAX_TASK_RETRIES,
            max_task_mailbox: DEFAULT_MAX_TASK_MAILBOX,
        };
        let builtin_registry = BuiltinRegistry::new();

        let mut s = Self {
            version,
            running: false,
            database,
            next_task_id: Default::default(),
            task_q,
            config,
            task_control_sender,
            task_control_receiver,
            scheduler_sender,
            scheduler_receiver,
            builtin_registry,
            server_options: default_server_options,
            system_control,
            worker_request_send,
            worker_request_recv,
            gc_collection_in_progress: false,
            gc_mark_in_progress: false,
            gc_sweep_in_progress: false,
            gc_force_collect: false,
            gc_cycle_count: 0,
            gc_last_cycle_time: std::time::Instant::now(),
            last_mutation_timestamp: None,
            checkpoint_in_progress: Arc::new(AtomicBool::new(false)),
            last_compact_time: std::time::Instant::now(),
            pending_task_sends: HashMap::new(),
        };

        let mut timing_policy = perf_timing_policy();
        if let Some(enabled) = s.config.runtime.perf_timing_enabled {
            timing_policy.enabled = enabled;
        }
        if let Some(shift) = s.config.runtime.perf_timing_hot_path_shift {
            timing_policy.hot_path_shift = shift;
        }
        if let Some(shift) = s.config.runtime.perf_timing_medium_path_shift {
            timing_policy.medium_path_shift = shift;
        }
        set_perf_timing_policy(timing_policy);

        s.reload_server_options();
        s
    }

    /// Execute the scheduler loop, run from the server process.
    pub fn run(mut self, bg_session_factory: Arc<dyn SessionFactory>) {
        // Rehydrate suspended tasks.
        self.task_q.suspended.load_tasks(bg_session_factory);

        set_current_thread_background_priority().ok();

        self.running = true;
        info!("Starting scheduler loop");

        // Set up receivers for listening to various message types
        let task_receiver = self.task_control_receiver.clone();
        let scheduler_receiver = self.scheduler_receiver.clone();
        let worker_receiver = self.worker_request_recv.clone();

        self.reload_server_options();
        while self.running {
            // Check if we should run GC (and no GC is already in progress)
            // Only run GC if anonymous objects are enabled
            if self.config.features.anonymous_objects
                && !self.gc_collection_in_progress
                && !self.gc_mark_in_progress
                && self.should_run_gc()
            {
                self.run_gc_cycle();
            }

            // Periodic tasks DB compaction (independent of GC)
            if self.last_compact_time.elapsed()
                >= Duration::from_secs(DEFAULT_COMPACT_INTERVAL_SECONDS)
            {
                debug!("Triggering periodic tasks database compaction");
                self.task_q.compact();
                self.last_compact_time = std::time::Instant::now();
            }

            // Skip task processing only if GC sweep is in progress
            // (mark phase allows concurrent task processing)
            if self.gc_sweep_in_progress {
                let tick_duration = self
                    .config
                    .runtime
                    .scheduler_tick_duration
                    .unwrap_or(Duration::from_millis(50));
                std::thread::sleep(tick_duration);
                continue;
            }

            // Define an enum to handle different message types
            enum SchedulerMessage {
                Task(TaskId, TaskControlMsg),
                Scheduler(SchedulerClientMsg),
                Worker(WorkerResponse),
            }

            // Immediate wakes are produced by scheduler-owned state transitions.
            // Drain them first to avoid waiting for channel activity.
            self.drain_immediate_wakes();

            // Use flume's Selector to properly select across channels with different types
            let selector = flume::Selector::new();

            // Add task receiver
            let selector = selector.recv(&task_receiver, |result| match result {
                Ok((task_id, msg)) => Some(SchedulerMessage::Task(task_id, msg)),
                Err(_) => None,
            });

            // Add scheduler receiver
            let selector = selector.recv(&scheduler_receiver, |result| match result {
                Ok(msg) => Some(SchedulerMessage::Scheduler(msg)),
                Err(_) => None,
            });

            // Add worker receiver if present
            let selector = if let Some(ref wr) = worker_receiver {
                selector.recv(wr, |result| match result {
                    Ok(response) => Some(SchedulerMessage::Worker(response)),
                    Err(_) => None,
                })
            } else {
                selector
            };

            let tick_duration = self
                .config
                .runtime
                .scheduler_tick_duration
                .unwrap_or(Duration::from_millis(10));

            // Process first message from selector (blocking with timeout)
            match selector.wait_timeout(tick_duration) {
                Ok(Some(SchedulerMessage::Task(task_id, msg))) => {
                    self.handle_task_msg(task_id, msg);
                }
                Ok(Some(SchedulerMessage::Scheduler(msg))) => {
                    self.handle_scheduler_msg(msg);
                }
                Ok(Some(SchedulerMessage::Worker(response))) => {
                    self.handle_worker_response(response);
                }
                Ok(None) | Err(_) => {
                    // Timeout or channel disconnected, continue
                }
            }

            // Drain any additional ready messages (non-blocking) to process them in batch
            loop {
                let mut found_message = false;

                // Check task receiver
                if let Ok((task_id, msg)) = task_receiver.try_recv() {
                    self.handle_task_msg(task_id, msg);
                    found_message = true;
                }

                // Check scheduler receiver
                if let Ok(msg) = scheduler_receiver.try_recv() {
                    self.handle_scheduler_msg(msg);
                    found_message = true;
                }

                // Check worker receiver if present
                if let Some(ref wr) = worker_receiver
                    && let Ok(response) = wr.try_recv()
                {
                    self.handle_worker_response(response);
                    found_message = true;
                }

                // If no messages were found, exit the drain loop
                if !found_message {
                    break;
                }
            }

            self.drain_immediate_wakes();

            // Check for tasks that need to be woken (timer wheel handles timing internally)
            if let Some(to_wake) = self.task_q.collect_wake_tasks() {
                for sr in to_wake {
                    let task_id = sr.task.task_id;
                    let is_retry = matches!(sr.wake_condition, WakeCondition::Retry(_));

                    #[cfg(feature = "trace_events")]
                    {
                        let max_ticks = sr.task.vm_host.max_ticks;
                        let tick_count = sr.task.vm_host.tick_count();

                        let (wake_condition, wake_reason) = match &sr.wake_condition {
                            WakeCondition::Time(_) => ("Time", "Timer expired"),
                            WakeCondition::Input(_) => ("Input", "Input request fulfilled"),
                            WakeCondition::Task(_) => ("Task", "Dependency task completed"),
                            WakeCondition::Immediate(_) => ("Immediate", "Immediate wake"),
                            WakeCondition::Worker(_) => ("Worker", "Worker response received"),
                            WakeCondition::GCComplete => {
                                ("GCComplete", "Garbage collection completed")
                            }
                            WakeCondition::Never => ("Never", "Manual wake"),
                            WakeCondition::Retry(_) => ("Retry", "Transaction retry backoff"),
                            WakeCondition::TaskMessage(_) => {
                                ("TaskMessage", "Message received or timeout")
                            }
                        };

                        trace_task_resume!(
                            task_id,
                            wake_condition,
                            wake_reason,
                            to_literal(&v_int(0)),
                            max_ticks,
                            tick_count
                        );
                    }

                    if is_retry {
                        // Retry tasks need special handling - restore state and restart
                        self.task_q.wake_retry_suspended_task(
                            sr,
                            &self.task_control_sender,
                            self.database.as_ref(),
                            self.builtin_registry.clone(),
                            self.config.clone(),
                        );
                    } else {
                        // Determine resume value based on wake condition
                        let resume_value = match &sr.wake_condition {
                            WakeCondition::TaskMessage(_) => {
                                // Drain message queue and return as list
                                let messages = self.task_q.drain_messages(task_id);
                                List::from_iter(messages).into()
                            }
                            WakeCondition::Immediate(val) => {
                                val.clone().unwrap_or_else(|| v_int(0))
                            }
                            _ => v_int(0),
                        };
                        if let Err(e) = self.task_q.wake_suspended_task(
                            sr,
                            ResumeAction::Return(resume_value),
                            &self.task_control_sender,
                            self.database.as_ref(),
                            self.builtin_registry.clone(),
                            self.config.clone(),
                        ) {
                            error!(?task_id, ?e, "Error resuming task");
                        }
                    }
                }
            }
        }

        // Write out all the suspended tasks to the database.
        info!("Scheduler done; saving suspended tasks");
        self.task_q.suspended.save_tasks();
        info!("Saved.");
    }

    pub fn client(&self) -> Result<SchedulerClient, SchedulerError> {
        Ok(SchedulerClient::new(self.scheduler_sender.clone()))
    }
}

impl Scheduler {
    /// Submit a new task and wake it immediately if needed.
    /// This is the main entry point for starting new tasks.
    #[allow(clippy::too_many_arguments)]
    fn submit_task(
        &mut self,
        task_id: TaskId,
        player: &Obj,
        perms: &Obj,
        task_start: TaskStart,
        delay_start: Option<Duration>,
        session: Arc<dyn Session>,
    ) -> Result<TaskHandle, SchedulerError> {
        let gc_in_progress = self.config.features.anonymous_objects
            && (self.gc_sweep_in_progress || self.gc_force_collect);

        match self.task_q.submit_new_task(
            task_id,
            player,
            perms,
            task_start,
            delay_start,
            session,
            &self.server_options,
            gc_in_progress,
        ) {
            task_q_ops::TaskSubmission::Suspended(handle) => Ok(handle),
            task_q_ops::TaskSubmission::NeedsWake {
                handle,
                task,
                session,
                result_sender,
            } => {
                self.task_q.wake_task_thread(
                    task,
                    ResumeAction::Return(v_int(0)),
                    session,
                    result_sender,
                    &self.task_control_sender,
                    self.database.as_ref(),
                    self.builtin_registry.clone(),
                    self.config.clone(),
                )?;
                Ok(handle)
            }
        }
    }
}
