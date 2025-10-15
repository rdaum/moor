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

use crate::task_context::TaskGuard;
use flume::{Receiver, Sender};
use lazy_static::lazy_static;
use minstant::Instant;
use std::{
    fs::File,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::yield_now,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use moor_common::model::{CommitResult, ObjectRef, Perms, WorldState};
use moor_compiler::to_literal;
use moor_db::Database;

use crate::{
    config::{Config, ImportExportFormat},
    tasks::{
        DEFAULT_BG_SECONDS, DEFAULT_BG_TICKS, DEFAULT_FG_SECONDS, DEFAULT_FG_TICKS,
        DEFAULT_GC_INTERVAL_SECONDS, DEFAULT_MAX_STACK_DEPTH, ServerOptions, TaskHandle,
        TaskResult, TaskStart,
        gc_thread::spawn_gc_mark_phase,
        sched_counters,
        scheduler_client::{SchedulerClient, SchedulerClientMsg},
        task::Task,
        task_q::{RunningTask, SuspensionQ, TaskQ, WakeCondition},
        task_scheduler_client::{TaskControlMsg, TaskSchedulerClient, WorkerInfo},
        tasks_db::TasksDb,
        workers::{WorkerRequest, WorkerResponse},
        world_state_action::{WorldStateAction, WorldStateResponse},
        world_state_executor::{WorldStateActionExecutor, match_object_ref},
    },
    trace_task_create_command, trace_task_create_eval, trace_task_create_oob,
    trace_task_create_verb,
    vm::{Fork, TaskSuspend, builtins::BuiltinRegistry},
};
use moor_common::{
    tasks::{
        AbortLimitReason, CommandError, Event, NarrativeEvent, SchedulerError,
        SchedulerError::{
            CommandExecutionError, InputRequestNotFound, TaskAbortedCancelled, TaskAbortedError,
            TaskAbortedException, TaskAbortedLimit,
        },
        Session, SessionFactory, SystemControl, TaskId, WorkerError,
    },
    util::PerfTimerGuard,
};
use moor_objdef::{
    collect_object, collect_object_definitions, dump_object, dump_object_definitions,
};
use moor_textdump::{TextdumpWriter, make_textdump};
use moor_var::{
    E_EXEC, E_INVARG, E_INVIND, E_PERM, E_QUOTA, E_TYPE, Error, List, Obj, SYSTEM_OBJECT, Symbol,
    Var, Variant, v_bool_int, v_err, v_int, v_obj, v_string,
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

/// If a task is retried more than N number of times (due to commit conflict) we choose to abort.
// TODO: we could also look into some exponential-ish backoff
const MAX_TASK_RETRIES: u8 = 10;

lazy_static! {
    static ref SERVER_OPTIONS: Symbol = Symbol::mk("server_options");
    static ref BG_SECONDS: Symbol = Symbol::mk("bg_seconds");
    static ref BG_TICKS: Symbol = Symbol::mk("bg_ticks");
    static ref FG_SECONDS: Symbol = Symbol::mk("fg_seconds");
    static ref FG_TICKS: Symbol = Symbol::mk("fg_ticks");
    static ref MAX_STACK_DEPTH: Symbol = Symbol::mk("max_stack_depth");
    static ref DUMP_INTERVAL: Symbol = Symbol::mk("dump_interval");
    static ref GC_INTERVAL: Symbol = Symbol::mk("gc_interval");
    static ref DO_OUT_OF_BAND_COMMAND: Symbol = Symbol::mk("do_out_of_band_command");
}
/// Responsible for the dispatching, control, and accounting of tasks in the system.
/// There should be only one scheduler per server.
pub struct Scheduler {
    version: semver::Version,

    task_control_sender: Sender<(TaskId, TaskControlMsg)>,
    task_control_receiver: Receiver<(TaskId, TaskControlMsg)>,

    scheduler_sender: Sender<SchedulerClientMsg>,
    scheduler_receiver: Receiver<SchedulerClientMsg>,

    config: Arc<Config>,

    running: bool,
    database: Box<dyn Database>,
    next_task_id: usize,

    server_options: ServerOptions,

    builtin_registry: BuiltinRegistry,

    system_control: Arc<dyn SystemControl>,

    worker_request_send: Option<Sender<WorkerRequest>>,
    worker_request_recv: Option<Receiver<WorkerResponse>>,

    /// The internal task queue which holds our suspended tasks, and control records for actively
    /// running tasks.
    /// This is in a lock to allow interior mutability for the scheduler loop, but is only ever
    /// accessed by the scheduler thread.
    task_q: TaskQ,

    /// Anonymous object garbage collection flag
    gc_collection_in_progress: bool,
    /// Flag indicating concurrent GC mark phase is in progress
    gc_mark_in_progress: bool,
    /// Flag indicating GC sweep phase is in progress (blocks new tasks)
    gc_sweep_in_progress: bool,
    /// Flag to force GC on next opportunity (set by gc_collect() builtin)
    gc_force_collect: bool,
    /// Counter tracking the number of GC cycles completed
    gc_cycle_count: u64,
    /// Time of last GC cycle (for interval-based collection)
    gc_last_cycle_time: std::time::Instant,
    /// Transaction timestamp (monotonically incrementing) of the last mutating task/transaction
    last_mutation_timestamp: Option<u64>,

    /// Tracks whether a checkpoint operation is currently in progress to prevent overlapping checkpoints
    checkpoint_in_progress: Arc<AtomicBool>,
}

fn load_int_sysprop(server_options_obj: &Obj, name: Symbol, tx: &dyn WorldState) -> Option<u64> {
    let Ok(value) = tx.retrieve_property(&SYSTEM_OBJECT, server_options_obj, name) else {
        return None;
    };
    match value.variant() {
        Variant::Int(i) if *i >= 0 => Some(*i as u64),
        _ => {
            warn!("$bg_seconds is not a positive integer");
            None
        }
    }
}

fn gc_error(context: &str, e: impl std::fmt::Debug) -> SchedulerError {
    SchedulerError::GarbageCollectionFailed(format!("{context}: {e:?}"))
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
        };
        s.reload_server_options();
        s
    }

    /// Execute the scheduler loop, run from the server process.
    pub fn run(mut self, bg_session_factory: Arc<dyn SessionFactory>) {
        // Rehydrate suspended tasks.
        self.task_q.suspended.load_tasks(bg_session_factory);

        gdt_cpus::set_thread_priority(gdt_cpus::ThreadPriority::Background).ok();

        self.running = true;
        info!("Starting scheduler loop");

        // Set up receivers for listening to various message types
        let task_receiver = self.task_control_receiver.clone();
        let scheduler_receiver = self.scheduler_receiver.clone();
        let worker_receiver = self.worker_request_recv.clone();
        let immediate_wake_receiver = self.task_q.suspended.immediate_wake_receiver().clone();

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

            // Skip task processing only if GC sweep is in progress
            // (mark phase allows concurrent task processing)
            if self.gc_sweep_in_progress {
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }

            // Check for tasks that need to be woken (timer wheel handles timing internally)
            if let Some(to_wake) = self.task_q.collect_wake_tasks(self.gc_sweep_in_progress) {
                for sr in to_wake {
                    let task_id = sr.task.task_id;
                    if let Err(e) = self.task_q.resume_task_thread(
                        sr.task,
                        ResumeAction::Return(v_int(0)),
                        sr.session,
                        sr.result_sender,
                        &self.task_control_sender,
                        self.database.as_ref(),
                        self.builtin_registry.clone(),
                        self.config.clone(),
                    ) {
                        error!(?task_id, ?e, "Error resuming task");
                    }
                }
            }

            // Define an enum to handle different message types
            enum SchedulerMessage {
                Task(TaskId, TaskControlMsg),
                Scheduler(SchedulerClientMsg),
                Worker(WorkerResponse),
                ImmediateWake(TaskId),
            }

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

            // Add immediate wake receiver
            let selector = selector.recv(&immediate_wake_receiver, |result| match result {
                Ok(task_id) => Some(SchedulerMessage::ImmediateWake(task_id)),
                Err(_) => None,
            });

            match selector.wait_timeout(Duration::from_millis(10)) {
                Ok(Some(SchedulerMessage::Task(task_id, msg))) => {
                    self.handle_task_msg(task_id, msg);
                }
                Ok(Some(SchedulerMessage::Scheduler(msg))) => {
                    self.handle_scheduler_msg(msg);
                }
                Ok(Some(SchedulerMessage::Worker(response))) => {
                    self.handle_worker_response(response);
                }
                Ok(Some(SchedulerMessage::ImmediateWake(task_id))) => {
                    self.handle_immediate_wake(task_id);
                }
                Ok(None) | Err(_) => {
                    // Timeout or channel disconnected, continue
                }
            }
        }

        // Write out all the suspended tasks to the database.
        info!("Scheduler done; saving suspended tasks");
        self.task_q.suspended.save_tasks();
        info!("Saved.");
    }

    pub fn reload_server_options(&mut self) {
        // Load the server options from the database, if possible.
        let tx = self
            .database
            .new_world_state()
            .expect("Could not open transaction to read server properties");

        let mut so = self.server_options.clone();

        let Ok(server_options_obj) =
            tx.retrieve_property(&SYSTEM_OBJECT, &SYSTEM_OBJECT, *SERVER_OPTIONS)
        else {
            info!("No server options object found; using defaults");
            tx.rollback().unwrap();
            return;
        };
        let Some(server_options_obj) = server_options_obj.as_object() else {
            info!("Server options property is not an object; using defaults");
            tx.rollback().unwrap();
            return;
        };
        info!("Found server options object: {}", server_options_obj);

        if let Some(bg_seconds) = load_int_sysprop(&server_options_obj, *BG_SECONDS, tx.as_ref()) {
            so.bg_seconds = bg_seconds;
        }
        if let Some(bg_ticks) = load_int_sysprop(&server_options_obj, *BG_TICKS, tx.as_ref()) {
            so.bg_ticks = bg_ticks as usize;
        }
        if let Some(fg_seconds) = load_int_sysprop(&server_options_obj, *FG_SECONDS, tx.as_ref()) {
            so.fg_seconds = fg_seconds;
        }
        if let Some(fg_ticks) = load_int_sysprop(&server_options_obj, *FG_TICKS, tx.as_ref()) {
            so.fg_ticks = fg_ticks as usize;
        }
        if let Some(max_stack_depth) =
            load_int_sysprop(&server_options_obj, *MAX_STACK_DEPTH, tx.as_ref())
        {
            so.max_stack_depth = max_stack_depth as usize;
        }
        if let Some(dump_interval) = load_int_sysprop(&SYSTEM_OBJECT, *DUMP_INTERVAL, tx.as_ref()) {
            info!(
                "Loaded dump_interval from database: {} seconds",
                dump_interval
            );
            so.dump_interval = Some(dump_interval);
        } else {
            info!("No dump_interval found on #0");
        }
        if let Some(gc_interval) = load_int_sysprop(&SYSTEM_OBJECT, *GC_INTERVAL, tx.as_ref()) {
            info!("Loaded gc_interval from database: {} seconds", gc_interval);
            so.gc_interval = Some(gc_interval);
        } else {
            // Check if we have a config override before falling back to default
            if self.config.runtime.gc_interval.is_some() {
                info!("No gc_interval found on #0, will use config override");
                so.gc_interval = None; // Config will take precedence
            } else {
                info!(
                    "No gc_interval found on #0, using default of {} seconds",
                    DEFAULT_GC_INTERVAL_SECONDS
                );
                so.gc_interval = Some(DEFAULT_GC_INTERVAL_SECONDS);
            }
        }
        tx.rollback().unwrap();

        self.server_options = so;

        info!("Server options refreshed.");
    }

    pub fn client(&self) -> Result<SchedulerClient, SchedulerError> {
        Ok(SchedulerClient::new(self.scheduler_sender.clone()))
    }

    pub fn get_checkpoint_interval(
        &mut self,
        cli_checkpoint_interval: Option<Duration>,
    ) -> Option<Duration> {
        // Reload server options to get fresh dump_interval from database
        self.reload_server_options();

        // Determine the checkpoint interval using the proper precedence:
        // 1. Command-line config overrides all
        // 2. Database dump_interval setting
        // 3. Default disabled
        if let Some(cli_interval) = cli_checkpoint_interval {
            info!(
                "Using checkpoint_interval from command-line config: {:?}",
                cli_interval
            );
            Some(cli_interval)
        } else if let Some(db_secs) = self.server_options.dump_interval {
            let db_interval = Duration::from_secs(db_secs);
            info!("Using dump_interval from database: {:?}", db_interval);
            Some(db_interval)
        } else {
            None
        }
    }

    pub fn get_gc_interval(&mut self) -> Option<Duration> {
        // Reload server options to get fresh gc_interval from database
        self.reload_server_options();

        // Determine the GC interval using the proper precedence:
        // 1. Command-line config overrides all
        // 2. Database gc_interval setting
        // 3. Default of 30 seconds
        if let Some(config_interval) = self.config.runtime.gc_interval {
            info!(
                "Using gc_interval from command-line config: {:?}",
                config_interval
            );
            Some(config_interval)
        } else if let Some(db_secs) = self.server_options.gc_interval {
            let db_interval = Duration::from_secs(db_secs);
            info!("Using gc_interval from database: {:?}", db_interval);
            Some(db_interval)
        } else {
            // Default to 30 seconds if not configured
            let default_interval = Duration::from_secs(DEFAULT_GC_INTERVAL_SECONDS);
            info!("Using default gc_interval: {:?}", default_interval);
            Some(default_interval)
        }
    }
}

impl Scheduler {
    fn handle_scheduler_msg(&mut self, msg: SchedulerClientMsg) {
        let counters = sched_counters();
        let _t = PerfTimerGuard::new(&counters.handle_scheduler_msg);
        let task_q = &mut self.task_q;
        match msg {
            SchedulerClientMsg::SubmitCommandTask {
                handler_object,
                player,
                command,
                session,
                reply,
            } => {
                let task_start = TaskStart::StartCommandVerb {
                    handler_object,
                    player,
                    command: command.to_string(),
                };

                let task_id = self.next_task_id;
                self.next_task_id += 1;

                trace_task_create_command!(task_id, &player, &command, &handler_object);

                let result = task_q.start_task_thread(
                    task_id,
                    task_start,
                    &player,
                    session,
                    None,
                    &player,
                    &self.server_options,
                    &self.task_control_sender,
                    self.database.as_ref(),
                    self.builtin_registry.clone(),
                    self.config.clone(),
                    self.config.features.anonymous_objects
                        && (self.gc_sweep_in_progress || self.gc_force_collect),
                );

                reply
                    .send(result)
                    .expect("Could not send task handle reply");
            }
            SchedulerClientMsg::SubmitVerbTask {
                player,
                vloc,
                verb,
                args,
                argstr,
                perms,
                session,
                reply,
            } => {
                // We need to translate Vloc and any of of the arguments into valid references
                // before we can start the task.
                // If they're all just plain object references, we can just use them as-is, without
                // starting a transaction. Otherwise, we need to start a transaction to resolve them.
                let need_tx_oref = !matches!(vloc, ObjectRef::Id(_));
                let vloc = if need_tx_oref {
                    let mut tx = self.database.new_world_state().unwrap();
                    let Ok(vloc) = match_object_ref(&player, &perms, &vloc, tx.as_mut()) else {
                        reply
                            .send(Err(CommandExecutionError(CommandError::NoObjectMatch)))
                            .expect("Could not send task handle reply");
                        return;
                    };
                    v_obj(vloc)
                } else {
                    match vloc {
                        ObjectRef::Id(id) => v_obj(id),
                        _ => panic!("Unexpected object reference in vloc"),
                    }
                };

                let task_id = self.next_task_id;
                self.next_task_id += 1;

                trace_task_create_verb!(task_id, &player, &verb.as_string(), &vloc);

                let task_start = TaskStart::StartVerb {
                    player,
                    vloc,
                    verb,
                    args,
                    argstr,
                };

                let result = task_q.start_task_thread(
                    task_id,
                    task_start,
                    &player,
                    session,
                    None,
                    &perms,
                    &self.server_options,
                    &self.task_control_sender,
                    self.database.as_ref(),
                    self.builtin_registry.clone(),
                    self.config.clone(),
                    self.config.features.anonymous_objects
                        && (self.gc_sweep_in_progress || self.gc_force_collect),
                );
                reply
                    .send(result)
                    .expect("Could not send task handle reply");
            }
            SchedulerClientMsg::SubmitTaskInput {
                player,
                input_request_id,
                input,
                reply,
            } => {
                // Validate that the given input request is valid, and if so, resume the task, sending it
                // the given input, clearing the input request out.

                // Find the task that requested this input, if any
                let Some(sr) = task_q
                    .suspended
                    .pull_task_for_input(input_request_id, &player)
                else {
                    warn!(?input_request_id, "Input request not found");
                    reply
                        .send(Err(InputRequestNotFound(input_request_id.as_u128())))
                        .expect("Could not send input request not found reply");
                    return;
                };

                // Wake and bake.
                let response = task_q.resume_task_thread(
                    sr.task,
                    ResumeAction::Return(input),
                    sr.session,
                    sr.result_sender,
                    &self.task_control_sender,
                    self.database.as_ref(),
                    self.builtin_registry.clone(),
                    self.config.clone(),
                );
                reply.send(response).expect("Could not send input reply");
            }
            SchedulerClientMsg::SubmitOobTask {
                handler_object,
                player,
                command,
                argstr,
                session,
                reply,
            } => {
                let task_id = self.next_task_id;
                self.next_task_id += 1;

                trace_task_create_oob!(task_id, &player, &command);

                let args = command.into_iter().map(v_string);
                let args = List::from_iter(args);
                let task_start = TaskStart::StartVerb {
                    player,
                    vloc: v_obj(handler_object),
                    verb: *DO_OUT_OF_BAND_COMMAND,
                    args,
                    argstr,
                };

                let result = task_q.start_task_thread(
                    task_id,
                    task_start,
                    &player,
                    session,
                    None,
                    &player,
                    &self.server_options,
                    &self.task_control_sender,
                    self.database.as_ref(),
                    self.builtin_registry.clone(),
                    self.config.clone(),
                    self.config.features.anonymous_objects
                        && (self.gc_sweep_in_progress || self.gc_force_collect),
                );
                reply
                    .send(result)
                    .expect("Could not send task handle reply");
            }
            SchedulerClientMsg::SubmitEvalTask {
                player,
                perms,
                program,
                sessions,
                reply,
            } => {
                let task_id = self.next_task_id;
                self.next_task_id += 1;

                trace_task_create_eval!(task_id, &player);

                let task_start = TaskStart::StartEval { player, program };

                let result = task_q.start_task_thread(
                    task_id,
                    task_start,
                    &player,
                    sessions,
                    None,
                    &perms,
                    &self.server_options,
                    &self.task_control_sender,
                    self.database.as_ref(),
                    self.builtin_registry.clone(),
                    self.config.clone(),
                    self.config.features.anonymous_objects
                        && (self.gc_sweep_in_progress || self.gc_force_collect),
                );
                reply
                    .send(result)
                    .expect("Could not send task handle reply");
            }
            SchedulerClientMsg::Shutdown(msg, reply) => {
                // Send shutdown notifications to all live tasks.

                let result = self.stop(Some(msg));
                reply.send(result).expect("Could not send shutdown reply");
            }
            SchedulerClientMsg::Checkpoint(blocking, reply) => {
                let result = if blocking {
                    self.checkpoint_blocking()
                } else {
                    self.checkpoint()
                };
                if reply.send(result).is_err() {
                    error!("Could not send checkpoint reply (client likely timed out)");
                }
            }
            SchedulerClientMsg::CheckStatus(reply) => {
                // Lightweight status check - just confirm we're alive and responding
                reply.send(Ok(())).expect("Could not send status reply");
            }
            SchedulerClientMsg::GetGCStats(reply) => {
                use crate::tasks::scheduler_client::GCStats;
                let stats = GCStats {
                    cycle_count: self.gc_cycle_count,
                };
                reply
                    .send(Ok(stats))
                    .expect("Could not send GC stats reply");
            }
            SchedulerClientMsg::RequestGC(reply) => {
                debug!("Direct GC request received via scheduler client");

                // Check if anonymous objects are enabled first
                if !self.config.features.anonymous_objects {
                    warn!("GC requested but anonymous objects are disabled, ignoring request");
                    reply.send(Ok(())).expect("Could not send GC request reply");
                } else if self.gc_collection_in_progress {
                    info!(
                        "GC already in progress, request acknowledged but no additional cycle started"
                    );
                    reply.send(Ok(())).expect("Could not send GC request reply");
                } else if self.task_q.active.is_empty() {
                    // Can run GC immediately since no active tasks
                    self.run_gc_cycle();
                    reply.send(Ok(())).expect("Could not send GC request reply");
                } else {
                    // Set flag for GC to run when tasks complete
                    self.gc_force_collect = true;
                    debug!("GC requested but tasks are active, will run when tasks complete");
                    reply.send(Ok(())).expect("Could not send GC request reply");
                }
            }
            SchedulerClientMsg::LoadObject {
                object_definition,
                options,
                return_conflicts,
                reply,
            } => {
                let result = self.handle_load_object(object_definition, options, return_conflicts);
                if let Err(e) = reply.send(result) {
                    error!(?e, "Could not send load_object reply to requester");
                }
            }
            SchedulerClientMsg::GCMarkPhaseComplete {
                unreachable_objects,
                mutation_timestamp_before_mark,
            } => {
                // Clear the concurrent GC flag
                self.gc_mark_in_progress = false;

                info!(
                    "GC mark phase completed, received {} unreachable objects",
                    unreachable_objects.len()
                );

                // Check if mutations happened during mark phase
                if mutation_timestamp_before_mark != self.last_mutation_timestamp {
                    info!(
                        "Minor GC cycle #{}: mark phase invalidated by mutation during marking (before: {:?}, after: {:?}), skipping sweep phase",
                        self.gc_cycle_count,
                        mutation_timestamp_before_mark,
                        self.last_mutation_timestamp
                    );
                    return;
                }

                // Check if there's work to do
                if unreachable_objects.is_empty() {
                    info!(
                        "Minor GC cycle #{}: mark phase found no objects to collect, skipping sweep phase",
                        self.gc_cycle_count
                    );
                    return;
                }

                // Start blocking sweep phase
                let _ = self.run_blocking_sweep_phase(unreachable_objects);
            }
            SchedulerClientMsg::ExecuteWorldStateActions {
                actions,
                rollback,
                reply,
            } => {
                // Create transaction in scheduler thread
                let tx = match self.database.new_world_state() {
                    Ok(tx) => tx,
                    Err(e) => {
                        reply
                            .send(Err(CommandExecutionError(CommandError::DatabaseError(e))))
                            .expect("Could not send batch execution reply");
                        return;
                    }
                };

                // Extract just the actions from the requests
                let action_vec: Vec<WorldStateAction> =
                    actions.iter().map(|req| req.action.clone()).collect();
                let config = self.config.clone();

                // Spawn thread to execute actions, moving transaction into the thread
                std::thread::Builder::new()
                    .name("ws-actions".to_string())
                    .spawn(move || {
                        let executor = WorldStateActionExecutor::new(tx, config);

                        match executor.execute_batch(action_vec, rollback) {
                            Ok(results) => {
                                // Build responses with the original request IDs
                                let responses: Vec<WorldStateResponse> = actions
                                    .into_iter()
                                    .zip(results)
                                    .map(|(request, result)| WorldStateResponse::Success {
                                        id: request.id,
                                        result,
                                    })
                                    .collect();

                                reply
                                    .send(Ok(responses))
                                    .expect("Could not send batch execution reply");
                            }
                            Err(error) => {
                                reply
                                    .send(Err(error))
                                    .expect("Could not send batch execution reply");
                            }
                        }
                    })
                    .expect("Could not spawn WorldStateAction execution thread");
            }
        }
    }

    /// Handle task control messages inbound from tasks.
    /// Note: this function should never be allowed to panic, as it is called from the scheduler main loop.
    fn handle_task_msg(&mut self, task_id: TaskId, msg: TaskControlMsg) {
        let counters = sched_counters();
        let _t = PerfTimerGuard::new(&counters.handle_task_msg);

        let task_q = &mut self.task_q;
        match msg {
            TaskControlMsg::TaskSuccess(value, mutations_made, timestamp) => {
                // Record that this is the transaction to have last mutated the world.
                // Used by e.g. concurrent GC algorithm.
                if mutations_made {
                    self.last_mutation_timestamp = Some(timestamp);
                }

                // Commit the session.
                let Some(task) = task_q.active.get_mut(&task_id) else {
                    warn!(task_id, "Task not found for success");
                    return;
                };
                let Ok(()) = task.session.commit() else {
                    warn!("Could not commit session; aborting task");
                    return task_q.send_task_result(task_id, Err(TaskAbortedError));
                };
                task_q.send_task_result(task_id, Ok(value))
            }
            TaskControlMsg::TaskConflictRetry(task) => {
                let perfc = sched_counters();
                let _t = PerfTimerGuard::new(&perfc.task_conflict_retry);

                // Ask the task to restart itself, using its stashed original start info, but with
                // a brand new transaction.
                task_q.retry_task(
                    task,
                    &self.task_control_sender,
                    self.database.as_ref(),
                    self.builtin_registry.clone(),
                    self.config.clone(),
                );
            }
            TaskControlMsg::TaskVerbNotFound(..) => {
                task_q.send_task_result(task_id, Err(TaskAbortedError));
            }
            TaskControlMsg::TaskCommandError(parse_command_error) => {
                // This is a common occurrence, so we don't want to log it at warn level.
                task_q.send_task_result(task_id, Err(CommandExecutionError(parse_command_error)));
            }
            TaskControlMsg::TaskAbortCancelled => {
                let perfc = sched_counters();
                let _t = PerfTimerGuard::new(&perfc.task_abort_cancelled);

                warn!(?task_id, "Task cancelled");

                // Rollback the session.
                let Some(task) = task_q.active.get_mut(&task_id) else {
                    warn!(task_id, "Task not found for abort");
                    return;
                };
                if let Err(send_error) = task
                    .session
                    .send_system_msg(task.player, "Aborted.".to_string().as_str())
                {
                    warn!("Could not send abort message to player: {:?}", send_error);
                };

                let Ok(()) = task.session.commit() else {
                    warn!("Could not commit aborted session; aborting task");
                    return task_q.send_task_result(task_id, Err(TaskAbortedError));
                };
                task_q.send_task_result(task_id, Err(TaskAbortedCancelled));
            }
            TaskControlMsg::TaskAbortLimitsReached(limit_reason, this, verb, line_number) => {
                let perfc = sched_counters();
                let _t = PerfTimerGuard::new(&perfc.task_abort_limits);
                let abort_reason_text = match limit_reason {
                    AbortLimitReason::Ticks(t) => {
                        warn!(?task_id, ticks = t, "Task aborted, ticks exceeded");
                        format!(
                            "Abort: Task exceeded ticks limit of {t} @ {}:{verb}:{line_number}",
                            to_literal(&this)
                        )
                    }
                    AbortLimitReason::Time(t) => {
                        warn!(?task_id, time = ?t, "Task aborted, time exceeded");
                        format!("Abort: Task exceeded time limit of {t:?}")
                    }
                };

                // Commit the session
                let Some(task) = task_q.active.get_mut(&task_id) else {
                    warn!(task_id, "Task not found for abort");

                    return;
                };

                if let Err(e) = task
                    .session
                    .send_system_msg(task.player, &abort_reason_text)
                {
                    warn!("Could not send abort message to player: {e:?}");
                }

                let _ = task.session.commit();

                task_q.send_task_result(task_id, Err(TaskAbortedLimit(limit_reason)));
            }
            TaskControlMsg::TaskException(exception) => {
                let perfc = sched_counters();
                let _t = PerfTimerGuard::new(&perfc.task_exception);

                let Some(task) = task_q.active.get_mut(&task_id) else {
                    warn!(task_id, "Task not found for abort");
                    return;
                };

                // Compose a string out of the backtrace
                if let Err(send_error) = task.session.send_event(
                    task.player,
                    Box::new(NarrativeEvent {
                        event_id: Uuid::now_v7(),
                        timestamp: SystemTime::now(),
                        author: v_obj(task.player),
                        event: Event::Traceback(exception.as_ref().clone()),
                    }),
                ) {
                    warn!("Could not send traceback to player: {:?}", send_error);
                }

                let _ = task.session.commit();

                task_q.send_task_result(
                    task_id,
                    Err(TaskAbortedException(exception.as_ref().clone())),
                );
            }
            TaskControlMsg::TaskRequestFork(fork_request, reply) => {
                let perfc = sched_counters();
                let _t = PerfTimerGuard::new(&perfc.fork_task);

                // Task has requested a fork. Dispatch it and reply with the new task id.
                // Gotta dump this out til we exit the loop tho, since self.tasks is already
                // borrowed here.
                let new_session = {
                    let Some(task) = task_q.active.get_mut(&task_id) else {
                        warn!(task_id, "Task not found for fork request");
                        return;
                    };
                    task.session.clone()
                };
                self.process_fork_request(fork_request, reply, new_session);
            }
            TaskControlMsg::TaskSuspend(wake_condition, task) => {
                let perfc = sched_counters();
                let _t = PerfTimerGuard::new(&perfc.suspend_task);
                // Task is suspended. The resume time (if any) is the system time at which
                // the scheduler should try to wake us up.

                // Remove from the local task control...
                let Some(tc) = task_q.active.remove(&task_id) else {
                    warn!(task_id, "Task not found for suspend request");
                    return;
                };

                // Commit the session.
                let Ok(()) = tc.session.commit() else {
                    warn!("Could not commit session; aborting task");
                    return task_q.send_task_result(task_id, Err(TaskAbortedError));
                };

                // And insert into the suspended list.
                let wake_condition = match wake_condition {
                    TaskSuspend::Never => WakeCondition::Never,
                    TaskSuspend::Timed(t) => WakeCondition::Time(Instant::now() + t),
                    TaskSuspend::WaitTask(task_id) => WakeCondition::Task(task_id),
                    TaskSuspend::Commit(return_value) => WakeCondition::Immediate(return_value),
                    TaskSuspend::WorkerRequest(worker_type, args, timeout) => {
                        let worker_request_id = Uuid::new_v4();
                        // Send out a message over the workers channel.
                        // If we're not set up to do workers, just abort the task.
                        let Some(workers_sender) = self.worker_request_send.as_ref() else {
                            warn!("No workers configured for scheduler; aborting task");
                            return task_q.send_task_result(task_id, Err(TaskAbortedError));
                        };

                        if let Err(e) = workers_sender.send(WorkerRequest::Request {
                            request_id: worker_request_id,
                            request_type: worker_type,
                            perms: task.perms,
                            request: args,
                            timeout,
                        }) {
                            error!(?e, "Could not send worker request; aborting task");
                            return task_q.send_task_result(task_id, Err(TaskAbortedError));
                        }

                        WakeCondition::Worker(worker_request_id)
                    }
                };

                task_q
                    .suspended
                    .add_task(wake_condition, task, tc.session, tc.result_sender);
            }
            TaskControlMsg::TaskRequestInput(task) => {
                // Task has gone into suspension waiting for input from the client.
                // Create a unique ID for this request, and we'll wake the task when the
                // session receives input.

                let input_request_id = Uuid::new_v4();
                let Some(tc) = task_q.active.remove(&task_id) else {
                    warn!(task_id, "Task not found for input request");
                    return;
                };
                // Commit the session (not DB transaction) to make sure current output is
                // flushed up to the prompt point.
                let Ok(()) = tc.session.commit() else {
                    warn!("Could not commit session; aborting task");
                    return task_q.send_task_result(task_id, Err(TaskAbortedError));
                };

                let Ok(()) = tc.session.request_input(tc.player, input_request_id) else {
                    warn!("Could not request input from session; aborting task");
                    return task_q.send_task_result(task_id, Err(TaskAbortedError));
                };
                task_q.suspended.add_task(
                    WakeCondition::Input(input_request_id),
                    task,
                    tc.session,
                    tc.result_sender,
                );
            }

            TaskControlMsg::RequestTasks(reply) => {
                let tasks = self.task_q.suspended.tasks();
                if let Err(e) = reply.send(tasks) {
                    error!(?e, "Could not send task description to requester");
                    // TODO: murder this errant task
                }
                // TODO: add non-queued tasks.
            }
            TaskControlMsg::KillTask {
                victim_task_id,
                sender_permissions,
                result_sender,
            } => {
                // Task is asking to kill another task.
                let kr = task_q.kill_task(victim_task_id, sender_permissions);
                if let Err(e) = result_sender.send(kr) {
                    error!(?e, "Could not send kill task result to requester");
                }
            }
            TaskControlMsg::ResumeTask {
                queued_task_id,
                sender_permissions,
                return_value,
                result_sender,
            } => {
                let rr = task_q.resume_task(
                    task_id,
                    queued_task_id,
                    sender_permissions,
                    return_value,
                    &self.task_control_sender,
                    self.database.as_ref(),
                    self.builtin_registry.clone(),
                    self.config.clone(),
                );
                if let Err(e) = result_sender.send(rr) {
                    error!(?e, "Could not send resume task result to requester");
                }
            }
            TaskControlMsg::BootPlayer { player } => {
                // Task is asking to boot a player.
                task_q.disconnect_task(task_id, &player);
            }
            TaskControlMsg::Notify { player, event } => {
                // Task is asking to notify a player of an event.
                let Some(task) = task_q.active.get_mut(&task_id) else {
                    warn!(task_id, "Task not found for notify request");
                    return;
                };
                let Ok(()) = task.session.send_event(player, event) else {
                    warn!("Could not notify player; aborting task");
                    return task_q.send_task_result(task_id, Err(TaskAbortedError));
                };
            }
            TaskControlMsg::GetListeners(reply) => {
                let listeners = self
                    .system_control
                    .listeners()
                    .expect("Could not get listeners");
                if let Err(e) = reply.send(listeners) {
                    error!(?e, "Could not send listeners to requester");
                }
            }
            TaskControlMsg::Listen {
                handler_object,
                host_type,
                port,
                print_messages,
                reply,
            } => {
                let Some(_task) = task_q.active.get_mut(&task_id) else {
                    warn!(task_id, "Task not found for listen request");
                    return;
                };
                let result = self
                    .system_control
                    .listen(handler_object, &host_type, port, print_messages)
                    .err();
                reply.send(result).expect("Could not send listen reply");
            }
            TaskControlMsg::Unlisten {
                host_type,
                port,
                reply,
            } => {
                let Some(_task) = task_q.active.get_mut(&task_id) else {
                    warn!(task_id, "Task not found for unlisten request");
                    return;
                };
                let result = match self.system_control.unlisten(port, &host_type) {
                    Ok(_) => None,
                    Err(_) => Some(E_PERM.msg("Permission denied on unlisten")),
                };
                reply.send(result).expect("Could not send unlisten reply");
            }
            TaskControlMsg::Shutdown(msg) => {
                info!("Shutting down scheduler. Reason: {msg:?}");
                self.stop(msg)
                    .expect("Could not shutdown scheduler cleanly");
            }
            TaskControlMsg::ForceInput { who, line, reply } => {
                let Some(task) = task_q.active.get_mut(&task_id) else {
                    warn!(task_id, "Task not found for force input request");

                    reply.send(Err(E_INVIND.msg("Task not found"))).ok();
                    return;
                };
                let new_session = task.session.clone().fork().unwrap();
                let task_start = TaskStart::StartCommandVerb {
                    handler_object: SYSTEM_OBJECT,
                    player: who,
                    command: line,
                };

                let task_id = self.next_task_id;
                self.next_task_id += 1;
                let result = task_q.start_task_thread(
                    task_id,
                    task_start,
                    &who,
                    new_session,
                    None,
                    &who,
                    &self.server_options,
                    &self.task_control_sender,
                    self.database.as_ref(),
                    self.builtin_registry.clone(),
                    self.config.clone(),
                    self.config.features.anonymous_objects
                        && (self.gc_sweep_in_progress || self.gc_force_collect),
                );
                match result {
                    Err(e) => {
                        error!(?e, "Could not start task thread");
                        reply
                            .send(Err(E_INVIND.with_msg(|| {
                                format!("Could not start thread for force_input: {e:?}")
                            })))
                            .ok();
                    }
                    Ok(th) => {
                        reply.send(Ok(th.0)).ok();
                    }
                }
            }
            TaskControlMsg::Checkpoint(reply) => {
                let result = if reply.is_some() {
                    self.checkpoint_blocking()
                } else {
                    self.checkpoint()
                };

                if let Some(reply) = reply {
                    let _ = reply.send(result);
                } else if let Err(e) = result {
                    error!(?e, "Could not checkpoint");
                }
            }
            TaskControlMsg::RefreshServerOptions => {
                self.reload_server_options();
            }
            TaskControlMsg::ActiveTasks { reply } => {
                let mut results = vec![];
                for (task_id, tc) in self.task_q.active.iter() {
                    results.push((*task_id, tc.player, tc.task_start.clone()));
                }
                if let Err(e) = reply.send(Ok(results)) {
                    error!(?e, "Could not send active tasks to requester");
                }
            }
            TaskControlMsg::SwitchPlayer { new_player, reply } => {
                let result = self.handle_switch_player(task_id, new_player);
                if let Err(e) = reply.send(result) {
                    error!(?e, "Could not send switch player reply to requester");
                }
            }
            TaskControlMsg::DumpObject { obj, reply } => {
                let result = self.handle_dump_object(obj);
                if let Err(e) = reply.send(result) {
                    error!(?e, "Could not send dump object reply to requester");
                }
            }
            TaskControlMsg::GetWorkersInfo { reply } => {
                let result = self.handle_get_workers_info();
                if let Err(e) = reply.send(result) {
                    error!(?e, "Could not send workers info reply to requester");
                }
            }
            TaskControlMsg::RequestNewTransaction(reply) => {
                let result = self
                    .database
                    .new_world_state()
                    .map_err(|_| SchedulerError::CouldNotStartTask);
                if let Err(e) = reply.send(result) {
                    error!(?e, "Could not send new transaction reply to requester");
                }
            }
            TaskControlMsg::ForceGC => {
                info!("Forcing garbage collection via gc_collect() builtin");
                if !self.config.features.anonymous_objects {
                    warn!(
                        "GC force requested but anonymous objects are disabled, ignoring request"
                    );
                } else {
                    self.gc_force_collect = true;
                }
            }
        }
    }

    fn handle_switch_player(&mut self, task_id: TaskId, new_player: Obj) -> Result<(), Error> {
        // Get the current task to access its session
        let Some(task) = self.task_q.active.get_mut(&task_id) else {
            return Err(E_INVARG.with_msg(|| "Task not found for switch_player".to_string()));
        };

        // Get the connection details for the current session (player=None means "this session")
        let connection_details = task.session.connection_details(None).map_err(|e| {
            E_INVARG
                .with_msg(|| format!("Failed to get connection details for current session: {e:?}"))
        })?;

        // There should be exactly one connection for the current session
        let connection_obj = connection_details
            .first()
            .ok_or_else(|| {
                E_INVARG.with_msg(|| "No connection found for current session".to_string())
            })?
            .connection_obj;

        // Switch the player through the system control (which handles connection registry and host notification)
        self.system_control
            .switch_player(connection_obj, new_player)?;

        // Update the task's player
        task.player = new_player;

        Ok(())
    }

    fn handle_dump_object(&self, obj: Obj) -> Result<Vec<String>, Error> {
        // Create a snapshot to avoid blocking ongoing operations
        let snapshot = self.database.create_snapshot().map_err(|e| {
            E_INVARG.with_msg(|| format!("Failed to create database snapshot: {e:?}"))
        })?;

        // Collect the object definition
        let (_, _, _, object_def) = collect_object(snapshot.as_ref(), &obj)
            .map_err(|e| E_INVARG.with_msg(|| format!("Failed to collect object {obj}: {e:?}")))?;

        // Dump to objdef format
        let index_names = HashMap::new(); // Empty index for now, keeping simple
        let lines = dump_object(&index_names, &object_def)
            .map_err(|e| E_INVARG.with_msg(|| format!("Failed to dump object {obj}: {e:?}")))?;

        Ok(lines)
    }

    fn handle_load_object(
        &self,
        object_definition: String,
        options: moor_objdef::ObjDefLoaderOptions,
        _return_conflicts: bool,
    ) -> Result<moor_objdef::ObjDefLoaderResults, SchedulerError> {
        use moor_objdef::ObjectDefinitionLoader;

        // Create a new world state for loading
        let world_state = self
            .database
            .new_world_state()
            .map_err(|_| SchedulerError::CouldNotStartTask)?;

        let mut loader = Box::new(world_state)
            .as_loader_interface()
            .map_err(|_| SchedulerError::CouldNotStartTask)?;

        let mut object_loader = ObjectDefinitionLoader::new(loader.as_mut());

        // Load the object with the provided options
        let compile_options = self.config.features.compile_options();
        let target_obj = options.target_object;
        let constants = options.constants.clone();

        let result = object_loader
            .load_single_object(
                &object_definition,
                compile_options,
                target_obj,
                constants,
                options,
            )
            .map_err(|_| SchedulerError::CouldNotStartTask)?;

        // Commit the transaction if the result says we should
        if result.commit {
            loader
                .commit()
                .map_err(|_| SchedulerError::CouldNotStartTask)?;
        }

        Ok(result)
    }

    fn handle_get_workers_info(&self) -> Vec<WorkerInfo> {
        let Some(workers_sender) = self.worker_request_send.as_ref() else {
            warn!("No workers configured for scheduler; returning empty worker list");
            return vec![];
        };

        let request_id = Uuid::new_v4();

        // Send the workers info request
        if let Err(e) = workers_sender.send(WorkerRequest::GetWorkersInfo { request_id }) {
            error!("Failed to send workers info request: {e}");
            return vec![];
        }

        // Wait for the response (with timeout)
        let Some(worker_recv) = self.worker_request_recv.as_ref() else {
            error!("No worker response channel configured");
            return vec![];
        };

        match worker_recv.recv_timeout(Duration::from_secs(5)) {
            Ok(WorkerResponse::WorkersInfo {
                request_id: resp_id,
                workers_info,
            }) => {
                if resp_id == request_id {
                    workers_info
                } else {
                    warn!("Received workers info response with mismatched ID");
                    vec![]
                }
            }
            Ok(other_response) => {
                warn!("Received unexpected response type: {:?}", other_response);
                vec![]
            }
            Err(e) => {
                warn!("Timeout or error waiting for workers info response: {e}");
                vec![]
            }
        }
    }

    fn handle_immediate_wake(&mut self, task_id: TaskId) {
        // Handle immediate wake task from channel
        // Check if task still exists in suspended state and wake it
        if let Some(sr) = self.task_q.suspended.remove_task(task_id) {
            // Extract the return value from the wake condition
            let return_value = match sr.wake_condition {
                WakeCondition::Immediate(val) => val,
                _ => {
                    error!(
                        ?task_id,
                        "Immediate wake task has non-immediate wake condition"
                    );
                    v_int(0)
                }
            };

            if let Err(e) = self.task_q.resume_task_thread(
                sr.task,
                ResumeAction::Return(return_value),
                sr.session,
                sr.result_sender,
                &self.task_control_sender,
                self.database.as_ref(),
                self.builtin_registry.clone(),
                self.config.clone(),
            ) {
                error!(?task_id, ?e, "Error resuming immediate wake task");
            }
        } else {
            // Task was already removed (e.g., killed), ignore
        }
    }

    fn handle_worker_response(&mut self, worker_response: WorkerResponse) {
        let (request_id, resume_action) = match worker_response {
            WorkerResponse::Error { request_id, error } => {
                let err_msg = error.to_string();
                let err = match error {
                    WorkerError::PermissionDenied(_) => E_PERM.msg(err_msg),
                    WorkerError::NoWorkerAvailable(_) => E_TYPE.msg(err_msg),
                    WorkerError::InvalidRequest(_) => E_INVARG.msg(err_msg),
                    WorkerError::InternalError(_) => E_EXEC.msg(err_msg),
                    WorkerError::RequestTimedOut(_) => E_QUOTA.msg(err_msg),
                    WorkerError::RequestError(_) => E_INVARG.msg(err_msg),
                    WorkerError::WorkerDetached(_) => E_EXEC.msg(err_msg),
                };
                (request_id, ResumeAction::Raise(err))
            }
            WorkerResponse::Response {
                request_id,
                response,
            } => (request_id, ResumeAction::Return(response)),
            WorkerResponse::WorkersInfo {
                request_id: _,
                workers_info: _,
            } => {
                // Workers info responses are handled synchronously in handle_get_workers_info
                // This shouldn't happen in the normal worker response flow
                warn!("Received unexpected WorkersInfo response in handle_worker_response");
                return;
            }
        };

        // Find the suspended task for this request.
        let task = self.task_q.suspended.pull_task_for_worker(request_id);

        // Find the task that requested this input, if any
        let Some(sr) = task else {
            warn!(?request_id, "Task for worker request not found; expired?");
            return;
        };

        if let Err(e) = self.task_q.resume_task_thread(
            sr.task,
            resume_action,
            sr.session,
            sr.result_sender,
            &self.task_control_sender,
            self.database.as_ref(),
            self.builtin_registry.clone(),
            self.config.clone(),
        ) {
            error!("Failure to resume task after worker response: {:?}", e);
        }
    }

    fn checkpoint(&self) -> Result<(), SchedulerError> {
        // Check if a checkpoint is already in progress
        if self
            .checkpoint_in_progress
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            )
            .is_err()
        {
            warn!("Checkpoint already in progress, skipping duplicate request");
            return Ok(());
        }

        let Some(textdump_path) = self.config.import_export.output_path.clone() else {
            self.checkpoint_in_progress
                .store(false, std::sync::atomic::Ordering::SeqCst);
            error!("Cannot textdump as output directory not configured");
            return Err(SchedulerError::CouldNotStartTask);
        };

        // Verify the directory exists / create it
        if let Err(e) = std::fs::create_dir_all(&textdump_path) {
            self.checkpoint_in_progress
                .store(false, std::sync::atomic::Ordering::SeqCst);
            error!(?e, "Could not create textdump directory");
            return Err(SchedulerError::CouldNotStartTask);
        }

        // Output file should be suffixed with an incrementing number, to avoid overwriting
        // existing dumps, so we can do rolling backups.
        // We should be able to just use seconds since epoch for this.
        let textdump_path = textdump_path.join(format!(
            "textdump-{}.in-progress",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_secs()
        ));

        let encoding_mode = self.config.import_export.output_encoding;
        let version_string = self
            .config
            .import_export
            .version_string(&self.version, &self.config.features);
        let dirdump = self.config.import_export.export_format == ImportExportFormat::Objdef;

        // Clone the checkpoint flag to pass to the callback
        let checkpoint_flag = self.checkpoint_in_progress.clone();

        // Use async snapshot creation - the blocking and export work happens in the callback
        let result = self
            .database
            .create_snapshot_async(Box::new(move |snapshot_result| {
                let loader_client = match snapshot_result {
                    Ok(loader_client) => loader_client,
                    Err(e) => {
                        error!(?e, "Could not create snapshot for checkpoint");
                        checkpoint_flag.store(false, std::sync::atomic::Ordering::SeqCst);
                        return Ok(());
                    }
                };

                if dirdump {
                    info!("Collecting objects for dump...");
                    let objects = match collect_object_definitions(loader_client.as_ref()) {
                        Ok(objects) => objects,
                        Err(e) => {
                            error!(?e, "Failed to collect objects for dump");
                            checkpoint_flag.store(false, std::sync::atomic::Ordering::SeqCst);
                            return Ok(());
                        }
                    };
                    info!("Dumping objects to {textdump_path:?}");
                    if let Err(e) = dump_object_definitions(&objects, &textdump_path) {
                        error!(?e, "Failed to dump objects");
                        checkpoint_flag.store(false, std::sync::atomic::Ordering::SeqCst);
                        return Ok(());
                    }
                    // Now that the dump has been written, strip the in-progress suffix.
                    let final_path = textdump_path.with_extension("moo");
                    if let Err(e) = std::fs::rename(&textdump_path, &final_path) {
                        error!(?e, "Could not rename objdefdump to final path");
                    }
                    info!(?final_path, "Objdefdump written.");
                } else {
                    let Ok(mut output) = File::create(&textdump_path) else {
                        error!("Could not open textdump file for writing");
                        checkpoint_flag.store(false, std::sync::atomic::Ordering::SeqCst);
                        return Ok(());
                    };

                    let textdump = make_textdump(loader_client.as_ref(), version_string);

                    let mut writer = TextdumpWriter::new(&mut output, encoding_mode);
                    if let Err(e) = writer.write_textdump(&textdump) {
                        error!(?e, "Could not write textdump");
                        checkpoint_flag.store(false, std::sync::atomic::Ordering::SeqCst);
                        return Ok(());
                    }

                    // Now that the dump has been written, strip the in-progress suffix.
                    let final_path = textdump_path.with_extension("moo-textdump");
                    if let Err(e) = std::fs::rename(&textdump_path, &final_path) {
                        error!(?e, "Could not rename textdump to final path");
                    }
                    info!(?final_path, "Textdump written.");
                }

                // Clear the checkpoint flag when operation completes successfully
                checkpoint_flag.store(false, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            }));

        if result.is_err() {
            self.checkpoint_in_progress
                .store(false, std::sync::atomic::Ordering::SeqCst);
            return Err(SchedulerError::CouldNotStartTask);
        }

        Ok(())
    }

    /// Request a checkpoint and wait for the textdump generation to complete.
    ///
    /// Unlike `checkpoint()`, this method blocks until the background textdump thread
    /// finishes, providing confirmation that the checkpoint has been written to disk.
    fn checkpoint_blocking(&self) -> Result<(), SchedulerError> {
        // Check if a checkpoint is already in progress
        if self
            .checkpoint_in_progress
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            )
            .is_err()
        {
            warn!("Checkpoint already in progress, skipping duplicate request");
            return Ok(());
        }

        let Some(textdump_path) = self.config.import_export.output_path.clone() else {
            self.checkpoint_in_progress
                .store(false, std::sync::atomic::Ordering::SeqCst);
            error!("Cannot textdump as output directory not configured");
            return Err(SchedulerError::CouldNotStartTask);
        };

        // Verify the directory exists / create it
        if let Err(e) = std::fs::create_dir_all(&textdump_path) {
            self.checkpoint_in_progress
                .store(false, std::sync::atomic::Ordering::SeqCst);
            error!(?e, "Could not create textdump directory");
            return Err(SchedulerError::CouldNotStartTask);
        }

        // Output file should be suffixed with an incrementing number, to avoid overwriting
        // existing dumps, so we can do rolling backups.
        // We should be able to just use seconds since epoch for this.
        let textdump_path = textdump_path.join(format!(
            "textdump-{}.in-progress",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_secs()
        ));

        let encoding_mode = self.config.import_export.output_encoding;
        let version_string = self
            .config
            .import_export
            .version_string(&self.version, &self.config.features);
        let dirdump = self.config.import_export.export_format == ImportExportFormat::Objdef;

        // Use a channel to wait for the async operation to complete
        let (completion_sender, completion_receiver) = std::sync::mpsc::channel();

        // Clone the checkpoint flag to pass to the callback
        let checkpoint_flag = self.checkpoint_in_progress.clone();

        // Use async snapshot creation but wait for completion
        let result = self
            .database
            .create_snapshot_async(Box::new(move |snapshot_result| {
                let result = (|| -> Result<(), SchedulerError> {
                    let loader_client = match snapshot_result {
                        Ok(loader_client) => loader_client,
                        Err(e) => {
                            error!(?e, "Could not create snapshot for checkpoint");
                            return Err(SchedulerError::CouldNotStartTask);
                        }
                    };

                    if dirdump {
                        info!("Collecting objects for dump...");
                        let objects = match collect_object_definitions(loader_client.as_ref()) {
                            Ok(objects) => objects,
                            Err(e) => {
                                error!(?e, "Failed to collect objects for dump");
                                return Err(SchedulerError::CouldNotStartTask);
                            }
                        };
                        info!("Dumping objects to {textdump_path:?}");
                        if let Err(e) = dump_object_definitions(&objects, &textdump_path) {
                            error!(?e, "Failed to dump objects");
                            return Err(SchedulerError::CouldNotStartTask);
                        }
                        // Now that the dump has been written, strip the in-progress suffix.
                        let final_path = textdump_path.with_extension("moo");
                        if let Err(e) = std::fs::rename(&textdump_path, &final_path) {
                            error!(?e, "Could not rename objdefdump to final path");
                            return Err(SchedulerError::CouldNotStartTask);
                        }
                        info!(?final_path, "Objdefdump written.");
                    } else {
                        let Ok(mut output) = File::create(&textdump_path) else {
                            error!("Could not open textdump file for writing");
                            return Err(SchedulerError::CouldNotStartTask);
                        };

                        let textdump = make_textdump(loader_client.as_ref(), version_string);

                        let mut writer = TextdumpWriter::new(&mut output, encoding_mode);
                        if let Err(e) = writer.write_textdump(&textdump) {
                            error!(?e, "Could not write textdump");
                            return Err(SchedulerError::CouldNotStartTask);
                        }

                        // Now that the dump has been written, strip the in-progress suffix.
                        let final_path = textdump_path.with_extension("moo-textdump");
                        if let Err(e) = std::fs::rename(&textdump_path, &final_path) {
                            error!(?e, "Could not rename textdump to final path");
                            return Err(SchedulerError::CouldNotStartTask);
                        }
                        info!(?final_path, "Textdump written.");
                    }
                    Ok(())
                })();

                // Clear the checkpoint flag regardless of the result
                checkpoint_flag.store(false, std::sync::atomic::Ordering::SeqCst);

                // Send the result back to the waiting thread
                if completion_sender.send(result).is_err() {
                    error!("Failed to send checkpoint completion result");
                }
                Ok(())
            }));

        if result.is_err() {
            self.checkpoint_in_progress
                .store(false, std::sync::atomic::Ordering::SeqCst);
            return Err(SchedulerError::CouldNotStartTask);
        }

        // Wait for the operation to complete
        completion_receiver.recv().unwrap_or_else(|_| {
            error!("Failed to receive checkpoint completion result");
            Err(SchedulerError::CouldNotStartTask)
        })
    }

    fn process_fork_request(
        &mut self,
        fork_request: Box<Fork>,
        reply: oneshot::Sender<TaskId>,
        session: Arc<dyn Session>,
    ) {
        let mut to_remove = vec![];

        // Fork the session.
        let forked_session = session.fork().unwrap();

        let suspended = fork_request.delay.is_some();
        let player = fork_request.player;
        let delay = fork_request.delay;
        let progr = fork_request.progr;

        let task_start = TaskStart::StartFork {
            fork_request,
            suspended,
        };
        let task_id = self.next_task_id;
        self.next_task_id += 1;
        match self.task_q.start_task_thread(
            task_id,
            task_start,
            &player,
            forked_session,
            delay,
            &progr,
            &self.server_options,
            &self.task_control_sender,
            self.database.as_ref(),
            self.builtin_registry.clone(),
            self.config.clone(),
            self.gc_collection_in_progress || self.gc_force_collect,
        ) {
            Ok(th) => th,
            Err(e) => {
                error!(?e, "Could not fork task");
                return;
            }
        };

        let reply = reply;
        if let Err(e) = reply.send(task_id) {
            error!(task = task_id, error = ?e, "Could not send fork reply. Parent task gone?  Remove.");
            to_remove.push(task_id);
        }
    }

    /// Stop the scheduler run loop.
    fn stop(&mut self, msg: Option<String>) -> Result<(), SchedulerError> {
        // Send shutdown notification to all live tasks.
        for (_, task) in self.task_q.active.iter() {
            let _ = task.session.notify_shutdown(msg.clone());
        }
        warn!("Issuing clean shutdown...");
        {
            // Send shut down to all the tasks.
            for (_, task) in self.task_q.active.drain() {
                task.kill_switch.store(true, Ordering::SeqCst);
            }
        }
        warn!("Waiting for tasks to finish...");

        // Then spin until they're all done.
        loop {
            {
                if self.task_q.active.is_empty() {
                    break;
                }
            }
            yield_now();
        }

        // Now ask the rpc server and hosts to shutdown
        self.system_control
            .shutdown(msg)
            .expect("Could not cleanly shutdown system");

        warn!("All tasks finished.  Stopping scheduler.");
        self.running = false;

        Ok(())
    }

    /// Check if garbage collection should run
    fn should_run_gc(&self) -> bool {
        // Force GC if requested via gc_collect() builtin
        if self.gc_force_collect {
            return true;
        }

        // Run automatic GC based on conditions
        self.should_run_automatic_gc()
    }

    /// Check if automatic GC should run based on heuristics
    fn should_run_automatic_gc(&self) -> bool {
        let gc_interval = if let Some(config_interval) = self.config.runtime.gc_interval {
            config_interval
        } else if let Some(db_secs) = self.server_options.gc_interval {
            Duration::from_secs(db_secs)
        } else {
            Duration::from_secs(DEFAULT_GC_INTERVAL_SECONDS)
        };

        let time_since_last_gc = self.gc_last_cycle_time.elapsed();

        if time_since_last_gc >= gc_interval {
            info!(
                "Triggering automatic GC after {} seconds of inactivity (interval: {} seconds)",
                time_since_last_gc.as_secs(),
                gc_interval.as_secs()
            );
            return true;
        }

        // In the future, this could also check:
        // - Number of anonymous objects created since last GC
        // - Memory pressure
        // - Task activity levels
        false
    }

    /// Run a garbage collection cycle - mark & sweep collection
    fn run_gc_cycle(&mut self) {
        self.gc_force_collect = false; // Clear force flag
        self.gc_cycle_count += 1;

        // Run concurrent mark & sweep GC with retry logic for conflicts
        let max_retries = 3;
        for attempt in 1..=max_retries {
            let result = self.run_concurrent_gc();

            match result {
                Ok(()) => break, // Success, exit retry loop
                Err(e)
                    if e.to_string().contains("GC transaction conflict")
                        && attempt < max_retries =>
                {
                    warn!(
                        "GC cycle attempt {} failed with conflict, retrying in {}ms",
                        attempt,
                        attempt * 10
                    );
                    std::thread::sleep(Duration::from_millis((attempt * 10) as u64));
                    continue;
                }
                Err(e) => {
                    error!("GC cycle failed after {} attempts: {}", attempt, e);
                    break;
                }
            }
        }

        // Update the timestamp AFTER GC completes, not before
        self.gc_last_cycle_time = std::time::Instant::now();
    }

    /// Run concurrent mark & sweep GC
    fn run_concurrent_gc(&mut self) -> Result<(), SchedulerError> {
        // Collect VM references before spawning thread
        let vm_refs = self.task_q.collect_anonymous_object_references();
        let mutation_timestamp_before_mark = self.last_mutation_timestamp;

        // Create GC transaction for the background thread
        let gc_tx = self.database.gc_interface().map_err(|e| {
            SchedulerError::GarbageCollectionFailed(format!("Failed to create GC interface: {e}"))
        })?;
        let config_clone = self.config.clone();
        let scheduler_sender = self.scheduler_sender.clone();
        let gc_cycle_count = self.gc_cycle_count;

        // Set flag to prevent additional concurrent GC
        self.gc_mark_in_progress = true;

        // Spawn the mark thread
        let _handle = spawn_gc_mark_phase(
            gc_tx,
            config_clone,
            scheduler_sender,
            vm_refs,
            mutation_timestamp_before_mark,
            gc_cycle_count,
        );

        Ok(())
    }

    /// Wait for all active tasks to finish before starting sweep phase
    fn wait_for_active_tasks_to_finish(&mut self) -> Result<(), SchedulerError> {
        if self.task_q.active.is_empty() {
            info!("No active tasks to wait for");
            return Ok(());
        }

        info!(
            "Waiting for {} active tasks to finish before GC sweep phase",
            self.task_q.active.len()
        );

        // Spin until all active tasks are done
        while !self.task_q.active.is_empty() {
            std::thread::sleep(Duration::from_millis(10));

            // Process any incoming messages while waiting
            if let Ok((task_id, msg)) = self.task_control_receiver.try_recv() {
                self.handle_task_msg(task_id, msg);
            }
        }

        Ok(())
    }

    /// Blocking sweep phase for concurrent GC - waits for tasks and collects objects
    fn run_blocking_sweep_phase(
        &mut self,
        unreachable_objects: std::collections::HashSet<Obj>,
    ) -> Result<(), SchedulerError> {
        info!(
            "Starting blocking sweep phase for {} unreachable objects",
            unreachable_objects.len()
        );

        // Block new tasks during sweep
        self.gc_sweep_in_progress = true;

        // Check mutation timestamp before waiting for tasks
        let mutation_timestamp_before_wait = self.last_mutation_timestamp;

        // Wait for all active tasks to finish
        self.wait_for_active_tasks_to_finish()?;

        // Check mutation timestamp after waiting for tasks
        let mutation_timestamp_after_wait = self.last_mutation_timestamp;
        if mutation_timestamp_before_wait != mutation_timestamp_after_wait {
            info!(
                "Minor GC cycle #{}: mutations detected while waiting for tasks (before: {:?}, after: {:?}), sweep phase invalidated",
                self.gc_cycle_count, mutation_timestamp_before_wait, mutation_timestamp_after_wait
            );
            self.gc_sweep_in_progress = false;
            return Ok(());
        }

        // Run the actual sweep
        let result = self.run_gc_sweep_phase(std::collections::HashSet::new(), unreachable_objects);

        // Unblock new tasks
        self.gc_sweep_in_progress = false;

        result
    }

    /// Sweep phase of minor GC - collects unreachable objects (promotion already done in mark phase)
    fn run_gc_sweep_phase(
        &mut self,
        _reachable_objects: std::collections::HashSet<Obj>,
        unreachable_objects: std::collections::HashSet<Obj>,
    ) -> Result<(), SchedulerError> {
        let start_time = std::time::Instant::now();
        let perfc = sched_counters();
        let _t = PerfTimerGuard::new(&perfc.gc_sweep_phase);
        // Get a new GC interface for the sweep phase transaction
        let mut gc = self
            .database
            .gc_interface()
            .map_err(|e| gc_error("Failed to create GC interface for sweep phase", e))?;

        // Collect unreachable objects
        let collected = if !unreachable_objects.is_empty() {
            gc.collect_unreachable_anonymous_objects(&unreachable_objects)
                .map_err(|e| gc_error("Failed to collect unreachable objects", e))?
        } else {
            0
        };

        let sweep_duration = start_time.elapsed();
        info!(
            "GC sweep: {} objects collected in {:.2}ms",
            collected,
            sweep_duration.as_secs_f64() * 1000.0
        );

        // Commit the sweep phase transaction
        match gc.commit() {
            Ok(CommitResult::Success { .. }) => Ok(()),
            Ok(CommitResult::ConflictRetry) => {
                // Transaction conflict - our optimism wasn't justified
                warn!("GC sweep transaction conflict - retry needed");
                Err(SchedulerError::GarbageCollectionFailed(
                    "GC transaction conflict - retry needed".to_string(),
                ))
            }
            Err(e) => {
                error!("Failed to commit GC sweep transaction: {:?}", e);
                Err(gc_error("GC commit failed", e))
            }
        }
    }
}

impl TaskQ {
    #[allow(clippy::too_many_arguments)]
    fn start_task_thread(
        &mut self,
        task_id: TaskId,
        task_start: TaskStart,
        player: &Obj,
        session: Arc<dyn Session>,
        delay_start: Option<Duration>,
        perms: &Obj,
        server_options: &ServerOptions,
        control_sender: &Sender<(TaskId, TaskControlMsg)>,
        database: &dyn Database,
        builtin_registry: BuiltinRegistry,
        config: Arc<Config>,
        gc_in_progress: bool,
    ) -> Result<TaskHandle, SchedulerError> {
        let perfc = sched_counters();
        let _t = PerfTimerGuard::new(&perfc.start_task);
        let is_background = task_start.is_background();

        let (sender, receiver) = flume::bounded(1);

        let task_scheduler_client = TaskSchedulerClient::new(task_id, control_sender.clone());

        let kill_switch = Arc::new(AtomicBool::new(false));
        let mut task = Task::new(
            task_id,
            *player,
            task_start.clone(),
            *perms,
            server_options,
            kill_switch.clone(),
        );

        // If this task is delayed, stick it into suspension state immediately.
        if let Some(delay) = delay_start {
            // However we'll need the task to be in a resumable state, which means executing
            //  setup_task_start in a transaction.
            let world_state = match database.new_world_state() {
                Ok(ws) => ws,
                Err(e) => {
                    error!(error = ?e, "Could not start transaction for delayed task");
                    return Err(SchedulerError::CouldNotStartTask);
                }
            };

            // Set up transaction context for task setup
            let _tx_guard = TaskGuard::new(
                world_state,
                task_scheduler_client.clone(),
                task_id,
                *player,
                session.clone(),
            );
            if !task.setup_task_start(control_sender) {
                error!(task_id, "Could not setup task start");
                return Err(SchedulerError::CouldNotStartTask);
            }
            task.retry_state = task.vm_host.snapshot_state();

            match crate::task_context::commit_current_transaction() {
                Ok(CommitResult::Success { .. }) => {}
                // TODO: perform a retry here in a modest loop.
                Ok(CommitResult::ConflictRetry) => {
                    error!(task_id, "Conflict during task start");
                    return Err(SchedulerError::CouldNotStartTask);
                }
                Err(e) => {
                    error!(task_id, error = ?e, "Error committing task start");
                    return Err(SchedulerError::CouldNotStartTask);
                }
            }
            let wake_condition = WakeCondition::Time(Instant::now() + delay);
            self.suspended
                .add_task(wake_condition, task, session, Some(sender));
            return Ok(TaskHandle(task_id, receiver));
        }

        // If GC is in progress, suspend this task instead of starting a thread
        if gc_in_progress {
            self.suspended
                .add_task(WakeCondition::GCComplete, task, session, Some(sender));
            return Ok(TaskHandle(task_id, receiver));
        }

        // Otherwise, we create a task control record and fire up a thread.
        let task_control = RunningTask {
            player: *player,
            kill_switch,
            task_start,
            session: session.clone(),
            result_sender: (!is_background).then_some(sender),
        };

        // Footgun warning: ALWAYS `self.tasks.insert` before spawning the task thread!
        self.active.insert(task_id, task_control);

        let control_sender = control_sender.clone();

        let world_state = match database.new_world_state() {
            Ok(ws) => ws,
            Err(e) => {
                error!(error = ?e, "Could not start transaction for task due to DB error");
                return Err(SchedulerError::CouldNotStartTask);
            }
        };
        self.thread_pool.spawn(move || {
            // Set up transaction context for this thread
            let _tx_guard = TaskGuard::new(
                world_state,
                task_scheduler_client.clone(),
                task.task_id,
                task.player,
                session.clone(),
            );

            // Start the db transaction, which will initially be used to resolve the verb before the task
            // starts executing.
            let setup_success = task.setup_task_start(&control_sender);

            if !setup_success {
                // Log level should be low here as this happens on every command if `do_command`
                // is not found.
                return;
            }
            task.retry_state = task.vm_host.snapshot_state();

            Task::run_task_loop(
                task,
                &task_scheduler_client,
                session,
                builtin_registry,
                config,
            );
        });

        Ok(TaskHandle(task_id, receiver))
    }

    #[allow(clippy::too_many_arguments)]
    fn resume_task_thread(
        &mut self,
        mut task: Box<Task>,
        resume_action: ResumeAction,
        session: Arc<dyn Session>,
        result_sender: Option<Sender<(TaskId, Result<TaskResult, SchedulerError>)>>,
        control_sender: &Sender<(TaskId, TaskControlMsg)>,
        database: &dyn Database,
        builtin_registry: BuiltinRegistry,
        config: Arc<Config>,
    ) -> Result<(), SchedulerError> {
        let perfc = sched_counters();
        let _t = PerfTimerGuard::new(&perfc.resume_task);

        // Take a task out of a suspended state and start running it again.
        // Means:
        //   Start a new transaction
        //   Create a new control record
        //   Push resume-value into the task

        // Start its new transaction...
        let world_state = match database.new_world_state() {
            Ok(ws) => ws,
            Err(e) => {
                error!(error = ?e, "Could not start transaction for task resumption due to DB error");
                return Err(SchedulerError::CouldNotStartTask);
            }
        };

        let task_id = task.task_id;
        let player = task.perms;

        // Brand new kill switch for the resumed task. The old one may have gotten toggled.
        let kill_switch = Arc::new(AtomicBool::new(false));
        task.kill_switch = kill_switch.clone();
        let task_control = RunningTask {
            player,
            kill_switch,
            session: session.clone(),
            result_sender,
            task_start: task.task_start.clone(),
        };

        self.active.insert(task_id, task_control);

        // Handle the resume action: either return a value or raise an error
        match resume_action {
            ResumeAction::Return(value) => {
                task.vm_host.resume_execution(value);
            }
            ResumeAction::Raise(error) => {
                task.vm_host.resume_with_error(error);
            }
        }

        let control_sender = control_sender.clone();
        let task_scheduler_client = TaskSchedulerClient::new(task_id, control_sender.clone());
        self.thread_pool.spawn(move || {
            // Set up transaction context for this thread
            let _tx_guard = TaskGuard::new(
                world_state,
                task_scheduler_client.clone(),
                task_id,
                player,
                session.clone(),
            );

            Task::run_task_loop(
                task,
                &task_scheduler_client,
                session,
                builtin_registry,
                config,
            );
        });

        Ok(())
    }

    fn send_task_result(&mut self, task_id: TaskId, result: Result<Var, SchedulerError>) {
        let Some(mut task_control) = self.active.remove(&task_id) else {
            // Missing task, must have ended already or gone into suspension?
            // This is odd though? So we'll warn.
            warn!(task_id, "Task not found for notification, ignoring");
            return;
        };
        let result_sender = task_control.result_sender.take();
        let Some(result_sender) = result_sender else {
            return;
        };
        let result = result.map(|v| TaskResult::Result(v.clone()));
        result_sender.send((task_id, result)).ok();
    }

    fn retry_task(
        &mut self,
        mut task: Box<Task>,
        control_sender: &Sender<(TaskId, TaskControlMsg)>,
        database: &dyn Database,
        builtin_registry: BuiltinRegistry,
        config: Arc<Config>,
    ) {
        let perfc = sched_counters();
        let _t = PerfTimerGuard::new(&perfc.retry_task);

        let task_id = task.task_id;

        // Make sure the old thread is dead.
        task.kill_switch.store(true, Ordering::SeqCst);

        // Remove this from the running tasks.
        // By definition we can't respond to a retry for a suspended task, so if it's not in the
        // running tasks there's something very wrong.
        let Some(old_tc) = self.active.remove(&task_id) else {
            error!(
                task_id,
                "Task not found for retry for {task_id}, ignoring -- consistency issue!"
            );
            return;
        };

        // If the number of retries has been exceeded, we'll just immediately respond with abort.
        if task.retries > MAX_TASK_RETRIES {
            info!(
                "Maximum number of retries exceeded for task {}.  Aborting.",
                task.task_id
            );
            self.send_task_result(task_id, Err(TaskAbortedError));
            return;
        }
        task.retries += 1;

        // Restore the VM state from its last snapshot, which would either be the original state of
        // the task, or its state as of the last commit.
        task.vm_host.restore_state(&task.retry_state);

        // Start a new session.
        let new_session = old_tc.session.fork().unwrap();

        // Brand new kill switch for the retried task. The old one was toggled to die.
        let kill_switch = Arc::new(AtomicBool::new(false));
        task.kill_switch = kill_switch.clone();

        // Otherwise, we create a task control record and fire up a thread.
        let task_control = RunningTask {
            player: old_tc.player,
            kill_switch,
            session: new_session.clone(),
            result_sender: old_tc.result_sender,
            task_start: task.task_start.clone(),
        };

        // Footgun warning: ALWAYS `self.tasks.insert` before spawning the task thread!
        self.active.insert(task_id, task_control);

        let control_sender = control_sender.clone();

        let world_state = match database.new_world_state() {
            Ok(ws) => ws,
            Err(e) => {
                // We panic here because this is a fundamental issue that will require admin
                // intervention.
                panic!("Could not start transaction for retry task due to DB error: {e:?}");
            }
        };
        let task_scheduler_client = TaskSchedulerClient::new(task_id, control_sender.clone());
        self.thread_pool.spawn(move || {
            // Set up transaction context for this thread
            let _tx_guard = TaskGuard::new(
                world_state,
                task_scheduler_client.clone(),
                task_id,
                task.player,
                new_session.clone(),
            );

            info!(?task.task_id, "Restarting retry task");
            Task::run_task_loop(
                task,
                &task_scheduler_client,
                new_session,
                builtin_registry,
                config,
            );
        });
    }

    fn kill_task(&mut self, victim_task_id: TaskId, sender_permissions: Perms) -> Var {
        let perfc = sched_counters();
        let _t = PerfTimerGuard::new(&perfc.kill_task);

        // Check if task exists in suspended tasks first, which we can combine with the perms
        // check.
        let is_suspended =
            if self
                .suspended
                .perms_check(victim_task_id, sender_permissions.who, false)
            {
                true
            } else if self.active.contains_key(&victim_task_id) {
                // For active tasks, check if sender has permission to kill them
                let tc = self.active.get(&victim_task_id).unwrap();
                if !sender_permissions
                    .check_is_wizard()
                    .expect("Could not check wizard status for kill request")
                    && sender_permissions.who != tc.player
                {
                    return v_err(E_PERM);
                }
                false
            } else {
                return v_err(E_INVARG);
            };

        // If not wizard and suspended task permission check failed, return permission error
        if is_suspended
            && !sender_permissions
                .check_is_wizard()
                .expect("Could not check wizard status for kill request")
        {
            return v_err(E_PERM);
        }

        // If suspended we can just remove completely and move on.
        if is_suspended {
            if self.suspended.remove_task(victim_task_id).is_none() {
                error!(
                    task = victim_task_id,
                    "Task not found in suspended list for kill request"
                );
            }
            return v_bool_int(false);
        }

        // Otherwise we have to check if the task is running, remove its control record, and flip
        // its kill switch.
        let victim_task = match self.active.remove(&victim_task_id) {
            Some(victim_task) => victim_task,
            None => {
                return v_err(E_INVARG);
            }
        };
        victim_task.kill_switch.store(true, Ordering::SeqCst);
        v_bool_int(false)
    }

    #[allow(clippy::too_many_arguments)]
    fn resume_task(
        &mut self,
        requesting_task_id: TaskId,
        queued_task_id: TaskId,
        sender_permissions: Perms,
        return_value: Var,
        control_sender: &Sender<(TaskId, TaskControlMsg)>,
        database: &dyn Database,
        builtin_registry: BuiltinRegistry,
        config: Arc<Config>,
    ) -> Var {
        // Task can't resume itself, it couldn't be queued. Builtin should not have sent this
        // request.
        if requesting_task_id == queued_task_id {
            error!(
                task = requesting_task_id,
                "Task requested to resume itself. Ignoring"
            );
            return v_err(E_INVARG);
        }

        // Check permissions for resumption.
        if !self
            .suspended
            .perms_check(queued_task_id, sender_permissions.who, true)
        {
            // If failed, and not wizard, permission denied
            if !sender_permissions
                .check_is_wizard()
                .expect("Could not check wizard status for resume request")
            {
                return v_err(E_PERM);
            }
            // Wizard can resume any task, but task must still exist
            if !self.suspended.tasks.contains_key(&queued_task_id) {
                error!(task = queued_task_id, "Task not found for resume request");
                return v_err(E_INVARG);
            }
        }

        let sr = self.suspended.remove_task(queued_task_id).unwrap();

        if self
            .resume_task_thread(
                sr.task,
                ResumeAction::Return(return_value),
                sr.session,
                sr.result_sender,
                control_sender,
                database,
                builtin_registry,
                config,
            )
            .is_err()
        {
            error!(task = queued_task_id, "Could not resume task");
            return v_err(E_INVARG);
        }
        v_bool_int(false)
    }

    fn disconnect_task(&mut self, disconnect_task_id: TaskId, player: &Obj) {
        let Some(task) = self.active.get_mut(&disconnect_task_id) else {
            warn!(task = disconnect_task_id, "Disconnecting task not found");
            return;
        };
        // First disconnect the player...
        warn!(?player, ?disconnect_task_id, "Disconnecting player");
        if let Err(e) = task.session.disconnect(*player) {
            warn!(?player, ?disconnect_task_id, error = ?e, "Could not disconnect player's session");
            return;
        }

        // Then abort all of their still-living forked tasks (that weren't the disconnect
        // task, we need to let that run to completion for sanity's sake.)
        for (task_id, tc) in self.active.iter() {
            if *task_id == disconnect_task_id {
                continue;
            }
            if tc.player.eq(player) {
                continue;
            }
            warn!(
                ?player,
                task_id, "Aborting task from disconnected player..."
            );
            tc.kill_switch.store(true, Ordering::SeqCst);
        }
        // Prune out non-background tasks for the player.
        self.suspended.prune_foreground_tasks(player);
    }
}
