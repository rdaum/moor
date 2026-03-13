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

pub(crate) mod lifecycle;
mod scheduler_config;
mod scheduler_gc;
mod scheduler_ops;
mod scheduler_submit;
mod scheduler_task_callbacks;
mod task_q_ops;

use crate::{
    task_context::TaskGuard,
    tasks::checkpoint::{CheckpointMode, start_checkpoint},
};
use flume::{Receiver, Sender};
use moor_common::util::{Deadline, Instant};
use rand::Rng;
use std::{
    sync::{
        Arc, Condvar, LazyLock, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::yield_now,
    time::{Duration, SystemTime},
};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use moor_common::model::{CommitResult, Perms, WorldState};
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
        task::Task,
        task_q::{RunningTask, SuspendedTask, SuspensionQ, TaskQ, WakeCondition},
        task_scheduler_client::{TaskSchedulerClient, WorkerInfo},
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

use self::lifecycle::TaskLifecycle;

/// Action to take when resuming a suspended task
#[derive(Debug, Clone)]
pub enum ResumeAction {
    /// Resume with a return value (normal case)
    Return(Var),
    /// Resume and immediately raise an error
    Raise(Error),
}

/// Responsible for the dispatching, control, and accounting of tasks in the system.
/// Cheaply cloneable handle — replaces both SchedulerClient and TaskSchedulerClient.
#[derive(Clone)]
pub struct Scheduler {
    /// All mutable lifecycle state, protected by a single Mutex.
    pub(crate) lifecycle: Arc<Mutex<TaskLifecycle>>,

    /// Database access (thread-safe, lock-free reads).
    pub(crate) database: Arc<dyn Database>,

    /// Runtime configuration.
    pub(crate) config: Arc<Config>,

    /// Host/connection management.
    pub(crate) system_control: Arc<dyn SystemControl>,

    /// Builtin function registry.
    pub(crate) builtin_registry: BuiltinRegistry,

    /// Server version.
    pub(crate) version: semver::Version,

    /// Tracks whether a checkpoint operation is currently in progress.
    pub(crate) checkpoint_in_progress: Arc<AtomicBool>,

    /// Channel for sending requests TO workers.
    pub(crate) worker_request_send: Option<Sender<WorkerRequest>>,

    /// Worker response receiver — taken once when starting the worker response thread.
    worker_response_recv: Arc<Mutex<Option<Receiver<WorkerResponse>>>>,

    /// Condvar to wake the timer thread when a new earlier timer is inserted.
    timer_notify: Arc<(Mutex<bool>, Condvar)>,
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

        let database: Arc<dyn Database> = Arc::from(database);

        let lifecycle = TaskLifecycle {
            task_q,
            pending_task_sends: HashMap::new(),
            next_task_id: 0,
            gc_collection_in_progress: false,
            gc_mark_in_progress: false,
            gc_sweep_in_progress: false,
            gc_force_collect: false,
            gc_cycle_count: 0,
            gc_last_cycle_time: std::time::Instant::now(),
            last_mutation_timestamp: None,
            server_options: default_server_options,
            running: false,
            last_compact_time: std::time::Instant::now(),
        };

        let mut timing_policy = perf_timing_policy();
        if let Some(enabled) = config.runtime.perf_timing_enabled {
            timing_policy.enabled = enabled;
        }
        if let Some(shift) = config.runtime.perf_timing_hot_path_shift {
            timing_policy.hot_path_shift = shift;
        }
        if let Some(shift) = config.runtime.perf_timing_medium_path_shift {
            timing_policy.medium_path_shift = shift;
        }
        set_perf_timing_policy(timing_policy);

        let s = Self {
            lifecycle: Arc::new(Mutex::new(lifecycle)),
            database,
            config,
            builtin_registry,
            system_control,
            version,
            checkpoint_in_progress: Arc::new(AtomicBool::new(false)),
            worker_request_send,
            worker_response_recv: Arc::new(Mutex::new(worker_request_recv)),
            timer_notify: Arc::new((Mutex::new(false), Condvar::new())),
        };

        s.reload_server_options();
        s
    }

    /// Start the scheduler: rehydrate tasks, spawn timer and worker-response threads.
    /// Returns join handle for the timer thread (join it at shutdown).
    pub fn start(
        &self,
        bg_session_factory: Arc<dyn SessionFactory>,
    ) -> std::thread::JoinHandle<()> {
        // Rehydrate suspended tasks.
        {
            let mut lc = self.lifecycle.lock().unwrap();
            lc.task_q.suspended.load_tasks(bg_session_factory);
            lc.running = true;
        }

        self.reload_server_options();

        // Start worker response thread if we have a worker receiver.
        if let Some(recv) = self.worker_response_recv.lock().unwrap().take() {
            let scheduler = self.clone();
            spawn_perf("moor-worker-recv", move || {
                scheduler.worker_response_loop(recv);
            })
            .expect("Could not spawn worker response thread");
        }

        // Start timer thread.
        let scheduler = self.clone();
        let timer_jh = spawn_perf("moor-timer", move || {
            set_current_thread_background_priority().ok();
            scheduler.timer_loop();
        })
        .expect("Could not spawn timer thread");

        info!("Scheduler started");
        timer_jh
    }

    /// The timer loop replaces the old run() main loop.
    /// Handles: timer expirations, GC checks, compaction, immediate wakes.
    fn timer_loop(&self) {
        loop {
            {
                let lc = self.lifecycle.lock().unwrap();
                if !lc.running {
                    break;
                }
            }

            // Check GC conditions
            {
                let mut lc = self.lifecycle.lock().unwrap();
                if self.config.features.anonymous_objects
                    && !lc.gc_collection_in_progress
                    && !lc.gc_mark_in_progress
                    && self.should_run_gc(&lc)
                {
                    self.run_gc_cycle(&mut lc);
                }

                // Periodic tasks DB compaction
                if lc.last_compact_time.elapsed()
                    >= Duration::from_secs(DEFAULT_COMPACT_INTERVAL_SECONDS)
                {
                    debug!("Triggering periodic tasks database compaction");
                    lc.task_q.compact();
                    lc.last_compact_time = std::time::Instant::now();
                }
            }

            // Drain immediate wakes
            self.drain_immediate_wakes();

            // Collect timer-based wakes
            self.collect_and_wake_expired_tasks();

            // Sleep until next timer expiry or notification
            let tick_duration = self
                .config
                .runtime
                .scheduler_tick_duration
                .unwrap_or(Duration::from_millis(10));

            let (lock, cvar) = &*self.timer_notify;
            let mut notified = lock.lock().unwrap();
            *notified = false;
            let _ = cvar.wait_timeout(notified, tick_duration);
        }

        // Write out all the suspended tasks to the database.
        info!("Timer loop done; saving suspended tasks");
        let lc = self.lifecycle.lock().unwrap();
        lc.task_q.suspended.save_tasks();
        info!("Saved.");
    }

    /// Wake the timer thread to recompute its sleep duration.
    pub(crate) fn wake_timer_thread(&self) {
        let (lock, cvar) = &*self.timer_notify;
        let mut notified = lock.lock().unwrap();
        *notified = true;
        cvar.notify_one();
    }

    /// Dedicated thread for receiving worker responses.
    fn worker_response_loop(&self, recv: Receiver<WorkerResponse>) {
        while let Ok(response) = recv.recv() {
            self.handle_worker_response(response);
        }
        debug!("Worker response loop exited");
    }

    /// Collect expired timer tasks and wake them.
    fn collect_and_wake_expired_tasks(&self) {
        let mut lc = self.lifecycle.lock().unwrap();

        let to_wake = match lc.task_q.collect_wake_tasks() {
            Some(tasks) => tasks,
            None => return,
        };

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
                lc.task_q.wake_retry_suspended_task(
                    sr,
                    self,
                    self.database.as_ref(),
                    self.builtin_registry.clone(),
                    self.config.clone(),
                );
            } else {
                let resume_value = match &sr.wake_condition {
                    WakeCondition::TaskMessage(_) => {
                        let messages = lc.task_q.drain_messages(task_id);
                        List::from_iter(messages).into()
                    }
                    WakeCondition::Immediate(val) => {
                        val.clone().unwrap_or_else(|| v_int(0))
                    }
                    _ => v_int(0),
                };
                if let Err(e) = lc.task_q.wake_suspended_task(
                    sr,
                    ResumeAction::Return(resume_value),
                    self,
                    self.database.as_ref(),
                    self.builtin_registry.clone(),
                    self.config.clone(),
                ) {
                    error!(?task_id, ?e, "Error resuming task");
                }
            }
        }
    }

    /// Submit a new task and wake it immediately if needed.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn submit_task(
        &self,
        lc: &mut TaskLifecycle,
        task_id: TaskId,
        player: &Obj,
        perms: &Obj,
        task_start: TaskStart,
        delay_start: Option<Duration>,
        session: Arc<dyn Session>,
    ) -> Result<TaskHandle, SchedulerError> {
        let gc_in_progress = self.config.features.anonymous_objects
            && (lc.gc_sweep_in_progress || lc.gc_force_collect);

        match lc.task_q.submit_new_task(
            task_id,
            player,
            perms,
            task_start,
            delay_start,
            session,
            &lc.server_options,
            gc_in_progress,
        ) {
            task_q_ops::TaskSubmission::Suspended(handle) => Ok(handle),
            task_q_ops::TaskSubmission::NeedsWake {
                handle,
                task,
                session,
                result_sender,
            } => {
                lc.task_q.wake_task_thread(
                    task,
                    ResumeAction::Return(v_int(0)),
                    session,
                    result_sender,
                    self,
                    self.database.as_ref(),
                    self.builtin_registry.clone(),
                    self.config.clone(),
                )?;
                Ok(handle)
            }
        }
    }

    /// Legacy compatibility: returns a SchedulerClient wrapping this Scheduler.
    pub fn client(&self) -> Result<crate::tasks::scheduler_client::SchedulerClient, SchedulerError> {
        Ok(crate::tasks::scheduler_client::SchedulerClient::new(self.clone()))
    }
}
