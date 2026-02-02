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

//! Chrome Trace Event integration for moor runtime tracing
//!
//! This module provides a background thread-based system for collecting and emitting
//! Chrome Trace Event Format compatible JSON for performance analysis and visualization.
//!
//! The system is only enabled when the `trace_events` feature is active, providing
//! zero runtime cost when disabled.

use moor_common::tasks::TaskId;
use std::path::PathBuf;

#[cfg(feature = "trace_events")]
use {
    flume::{Receiver, Sender, unbounded},
    moor_common::util::{EventPhase, InstantScope, TraceEvent, TraceFile, current_timestamp_us},
    serde_json::Value,
    std::collections::HashMap,
    std::fs::File,
    std::io::Write,
    std::sync::{Mutex, Once},
    std::thread,
};

/// Different types of trace events we can emit
#[derive(Debug, Clone)]
pub enum TraceEventType {
    /// Task lifecycle events - different create types for different task origins
    TaskCreateCommand {
        task_id: TaskId,
        player: String,
        command: String,
        handler_object: String,
    },
    TaskCreateVerb {
        task_id: TaskId,
        player: String,
        verb: String,
        vloc: String,
    },
    TaskCreateEval {
        task_id: TaskId,
        player: String,
    },
    TaskCreateOOB {
        task_id: TaskId,
        player: String,
        command: String,
    },
    TaskCreateFork {
        task_id: TaskId,
        player: String,
    },
    TaskStart {
        task_id: TaskId,
    },
    TaskComplete {
        task_id: TaskId,
        result: String,
    },
    TaskSuspend {
        task_id: TaskId,
        reason: String,
    },
    TaskResume {
        task_id: TaskId,
        wake_condition: String, // "Time", "Input", "Task", "Immediate", "Worker", "GCComplete", "Never"
        wake_reason: String,    // Human-readable description of why task woke
        return_value: String,   // The value being returned to resume execution
        max_ticks: usize,
        tick_count: usize,
    },
    TaskAbort {
        task_id: TaskId,
        reason: String,
    },

    /// VM execution events  
    VerbBegin {
        task_id: TaskId,
        verb_name: String,
        this: String,
        definer: String,
        caller_line: Option<usize>,
        args: Vec<String>,
    },
    VerbEnd {
        task_id: TaskId,
        verb_name: String,
    },
    BuiltinBegin {
        task_id: TaskId,
        builtin_name: String,
        caller_line: Option<usize>,
        args: Vec<String>,
    },
    BuiltinEnd {
        task_id: TaskId,
        builtin_name: String,
    },

    /// VM internal events
    OpcodeExecute {
        task_id: TaskId,
        opcode: String,
        count: u64,
    },
    StackUnwind {
        task_id: TaskId,
        reason: String,
        error_message: Option<String>,
        max_ticks: usize,
        tick_count: usize,
    },
    AbortLimitReached {
        task_id: TaskId,
        limit_type: String,  // "Ticks" or "Time"
        limit_value: String, // The actual limit that was hit
        max_ticks: usize,
        tick_count: usize,
        verb_name: String,
        this: String,
        line_number: usize,
    },

    /// Exception handler events
    ExceptionHandlerRequested {
        original_task_id: TaskId,
        handler_task_id: TaskId,
        exception_type: String,
    },
    ExceptionHandlerStarted {
        handler_task_id: TaskId,
        original_task_id: TaskId,
    },
    ExceptionHandlerCompleted {
        handler_task_id: TaskId,
        original_task_id: TaskId,
        handler_returned_true: bool,
    },

    /// Scheduler events
    SchedulerTick {
        active_tasks: usize,
        queued_tasks: usize,
    },

    /// Transaction events (database layer)
    TransactionBegin {
        tx_id: String,
        thread_id: u64,
    },
    TransactionCheck {
        tx_id: String,
        thread_id: u64,
        num_tuples: usize,
    },
    TransactionApply {
        tx_id: String,
        thread_id: u64,
        num_tuples: usize,
    },
    TransactionCommit {
        tx_id: String,
        thread_id: u64,
        success: bool,
        timestamp: u64,
    },
    TransactionRollback {
        tx_id: String,
        thread_id: u64,
        reason: String,
    },
    TransactionEnd {
        tx_id: String,
        thread_id: u64,
    },
}

/// Message sent to the background tracing thread
#[cfg(feature = "trace_events")]
#[derive(Debug)]
enum TracingMessage {
    Event(TraceEventType),
    Shutdown,
}

/// Global tracing state
#[cfg(feature = "trace_events")]
static TRACING_SENDER: Mutex<Option<Sender<TracingMessage>>> = Mutex::new(None);

#[cfg(feature = "trace_events")]
static TRACING_THREAD: Mutex<Option<thread::JoinHandle<()>>> = Mutex::new(None);

#[cfg(feature = "trace_events")]
static INIT: Once = Once::new();

/// Initialize the tracing system (only called once)
/// Returns true if tracing was successfully initialized
pub fn init_tracing(output_path: Option<PathBuf>) -> bool {
    #[cfg(feature = "trace_events")]
    {
        use std::sync::atomic::{AtomicBool, Ordering};
        static INITIALIZED: AtomicBool = AtomicBool::new(false);

        INIT.call_once(|| {
            let (sender, receiver) = unbounded();

            // Start the background thread
            let output_path = output_path.unwrap_or_else(|| PathBuf::from("moor_trace.json"));

            if let Ok(thread_handle) = thread::Builder::new()
                .name("moor-trace-events".to_string())
                .spawn(move || {
                    tracing_thread_main(receiver, output_path);
                })
            {
                // Store the sender and thread handle globally
                *TRACING_SENDER.lock().unwrap() = Some(sender);
                *TRACING_THREAD.lock().unwrap() = Some(thread_handle);
                INITIALIZED.store(true, Ordering::Relaxed);
            } else {
                tracing::error!("Failed to start tracing thread");
            }
        });
        INITIALIZED.load(Ordering::Relaxed)
    }

    #[cfg(not(feature = "trace_events"))]
    {
        let _ = output_path; // Silence unused parameter warning
        false
    }
}

/// Send a trace event (no-op if tracing not enabled)
#[inline]
pub fn emit_trace_event(event: TraceEventType) {
    #[cfg(feature = "trace_events")]
    {
        if let Ok(guard) = TRACING_SENDER.lock() {
            if let Some(sender) = guard.as_ref() {
                // Use try_send to avoid blocking if the channel is full
                match sender.try_send(TracingMessage::Event(event)) {
                    Ok(_) => {
                        // Event sent successfully
                    }
                    Err(flume::TrySendError::Full(_)) => {
                        tracing::warn!("Trace event channel is full, dropping event");
                    }
                    Err(flume::TrySendError::Disconnected(_)) => {
                        tracing::warn!("Trace event channel is disconnected");
                    }
                }
            } else {
                tracing::warn!("Trace event sender not initialized");
            }
        }
    }

    #[cfg(not(feature = "trace_events"))]
    {
        let _ = event; // Silence unused parameter warning
    }
}

/// Shutdown the tracing system
pub fn shutdown_tracing() {
    #[cfg(feature = "trace_events")]
    {
        // Send shutdown message
        if let Ok(guard) = TRACING_SENDER.lock()
            && let Some(sender) = guard.as_ref()
        {
            let _ = sender.send(TracingMessage::Shutdown);
        }

        // Wait for the background thread to finish
        if let Ok(mut guard) = TRACING_THREAD.lock()
            && let Some(thread_handle) = guard.take()
        {
            tracing::info!("Shutting down tracing thread...");
            if let Err(e) = thread_handle.join() {
                tracing::error!("Failed to join tracing thread: {:?}", e);
            } else {
                tracing::info!("Tracing thread shutdown complete");
            }
        }

        // Clear the sender
        if let Ok(mut guard) = TRACING_SENDER.lock() {
            *guard = None;
        }
    }
}

/// Main function for the background tracing thread
#[cfg(feature = "trace_events")]
fn tracing_thread_main(receiver: Receiver<TracingMessage>, output_path: PathBuf) {
    let mut trace_file = TraceFile::new();
    let start_time = current_timestamp_us();
    let mut seen_tids = std::collections::HashSet::new();

    // Add initial metadata
    add_metadata_events(&mut trace_file, start_time);

    // Track when we last flushed to disk
    let mut last_flush = std::time::Instant::now();
    const FLUSH_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);

    // Main event processing loop
    loop {
        match receiver.recv_timeout(std::time::Duration::from_millis(1000)) {
            Ok(TracingMessage::Event(event_type)) => {
                if let Some(trace_event) = convert_to_trace_event(event_type, start_time) {
                    let tid = trace_event.tid;

                    // Emit thread_name metadata on first encounter with a new thread
                    if seen_tids.insert(tid) {
                        let mut args = HashMap::new();
                        args.insert(
                            "name".to_string(),
                            Value::String(format!("Task Thread {}", tid)),
                        );

                        let thread_name_event = TraceEvent {
                            name: "thread_name".to_string(),
                            cat: None,
                            ph: EventPhase::Metadata,
                            ts: trace_event.ts,
                            tts: None,
                            pid: trace_event.pid,
                            tid,
                            dur: None,
                            tdur: None,
                            args: Some(args),
                            sf: None,
                            stack: None,
                            esf: None,
                            estack: None,
                            s: None,
                            id: None,
                            scope: None,
                            cname: None,
                        };
                        trace_file.add_event(thread_name_event);
                    }

                    trace_file.add_event(trace_event);
                }

                // Check if we should flush to disk
                if last_flush.elapsed() >= FLUSH_INTERVAL {
                    flush_trace_file(&trace_file, &output_path);
                    last_flush = std::time::Instant::now();
                }
            }
            Ok(TracingMessage::Shutdown) => {
                break;
            }
            Err(flume::RecvTimeoutError::Timeout) => {
                // Use timeout as opportunity to flush if needed
                if last_flush.elapsed() >= FLUSH_INTERVAL {
                    flush_trace_file(&trace_file, &output_path);
                    last_flush = std::time::Instant::now();
                }
                continue;
            }
            Err(flume::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }

    // Final flush on shutdown
    flush_trace_file(&trace_file, &output_path);
}

/// Flush trace events to disk
#[cfg(feature = "trace_events")]
fn flush_trace_file(trace_file: &TraceFile, output_path: &PathBuf) {
    match trace_file.to_json() {
        Ok(json) => match File::create(output_path) {
            Ok(mut file_output) => {
                if let Err(e) = file_output.write_all(json.as_bytes()) {
                    tracing::error!("Failed to write trace file: {}", e);
                }
            }
            Err(e) => {
                tracing::error!(
                    "Failed to create trace output file {:?}: {}",
                    output_path,
                    e
                );
            }
        },
        Err(e) => {
            tracing::error!("Failed to serialize trace events: {}", e);
        }
    }
}

/// Add initial metadata events to the trace file
#[cfg(feature = "trace_events")]
fn add_metadata_events(trace_file: &mut TraceFile, start_time: u64) {
    use std::process;

    let pid = process::id() as u64;

    // Add process name metadata
    let mut args = HashMap::new();
    args.insert("name".to_string(), Value::String("moor".to_string()));

    let process_name_event = TraceEvent {
        name: "process_name".to_string(),
        cat: None,
        ph: EventPhase::Metadata,
        ts: start_time,
        tts: None,
        pid,
        tid: 0,
        dur: None,
        tdur: None,
        args: Some(args),
        sf: None,
        stack: None,
        esf: None,
        estack: None,
        s: None,
        id: None,
        scope: None,
        cname: None,
    };

    trace_file.add_event(process_name_event);

    // Add thread name for scheduler thread
    let mut args = HashMap::new();
    args.insert("name".to_string(), Value::String("Scheduler".to_string()));

    let scheduler_thread_event = TraceEvent {
        name: "thread_name".to_string(),
        cat: None,
        ph: EventPhase::Metadata,
        ts: start_time,
        tts: None,
        pid,
        tid: 0,
        dur: None,
        tdur: None,
        args: Some(args),
        sf: None,
        stack: None,
        esf: None,
        estack: None,
        s: None,
        id: None,
        scope: None,
        cname: None,
    };

    trace_file.add_event(scheduler_thread_event);
}

/// Get the current OS thread ID as a u64
#[cfg(feature = "trace_events")]
fn current_thread_id() -> u64 {
    #[cfg(target_os = "linux")]
    {
        unsafe { libc::gettid() as u64 }
    }

    #[cfg(not(target_os = "linux"))]
    {
        // Fallback: use pthread_self on Unix-like systems, or a simple counter on Windows
        #[cfg(unix)]
        {
            unsafe { libc::pthread_self() as u64 }
        }

        #[cfg(not(unix))]
        {
            std::thread::current().id().as_u64().get()
        }
    }
}

/// Convert a TraceEventType to a Chrome TraceEvent
#[cfg(feature = "trace_events")]
fn convert_to_trace_event(event_type: TraceEventType, _start_time: u64) -> Option<TraceEvent> {
    let now = current_timestamp_us();
    let pid = std::process::id() as u64;

    match event_type {
        TraceEventType::TaskCreateCommand {
            task_id,
            player,
            command,
            handler_object,
        } => {
            let mut args = HashMap::new();
            args.insert(
                "task_id".to_string(),
                Value::Number((task_id as u64).into()),
            );
            args.insert("player".to_string(), Value::String(player));
            args.insert("command".to_string(), Value::String(command.clone()));
            args.insert("handler_object".to_string(), Value::String(handler_object));

            Some(TraceEvent {
                name: format!("Command Task ({task_id}): {command}"),
                cat: Some("task".to_string()),
                ph: EventPhase::Instant,
                ts: now,
                tts: None,
                pid,
                tid: current_thread_id(),
                dur: None,
                tdur: None,
                args: Some(args),
                sf: None,
                stack: None,
                esf: None,
                estack: None,
                s: Some(InstantScope::Thread),
                id: None,
                scope: None,
                cname: Some("good".to_string()),
            })
        }

        TraceEventType::TaskCreateVerb {
            task_id,
            player,
            verb,
            vloc,
        } => {
            let mut args = HashMap::new();
            args.insert(
                "task_id".to_string(),
                Value::Number((task_id as u64).into()),
            );
            args.insert("player".to_string(), Value::String(player));
            args.insert("verb".to_string(), Value::String(verb.clone()));
            args.insert("vloc".to_string(), Value::String(vloc));

            Some(TraceEvent {
                name: format!("Verb Task ({task_id}): {verb}"),
                cat: Some("task".to_string()),
                ph: EventPhase::Instant,
                ts: now,
                tts: None,
                pid,
                tid: current_thread_id(),
                dur: None,
                tdur: None,
                args: Some(args),
                sf: None,
                stack: None,
                esf: None,
                estack: None,
                s: Some(InstantScope::Thread),
                id: None,
                scope: None,
                cname: Some("good".to_string()),
            })
        }

        TraceEventType::TaskCreateEval { task_id, player } => {
            let mut args = HashMap::new();
            args.insert(
                "task_id".to_string(),
                Value::Number((task_id as u64).into()),
            );
            args.insert("player".to_string(), Value::String(player));

            Some(TraceEvent {
                name: format!("Eval Task ({task_id})"),
                cat: Some("task".to_string()),
                ph: EventPhase::Instant,
                ts: now,
                tts: None,
                pid,
                tid: current_thread_id(),
                dur: None,
                tdur: None,
                args: Some(args),
                sf: None,
                stack: None,
                esf: None,
                estack: None,
                s: Some(InstantScope::Thread),
                id: None,
                scope: None,
                cname: Some("good".to_string()),
            })
        }

        TraceEventType::TaskCreateOOB {
            task_id,
            player,
            command,
        } => {
            let mut args = HashMap::new();
            args.insert(
                "task_id".to_string(),
                Value::Number((task_id as u64).into()),
            );
            args.insert("player".to_string(), Value::String(player));
            args.insert("command".to_string(), Value::String(command.clone()));

            Some(TraceEvent {
                name: format!("OOB Task ({task_id}): {command}"),
                cat: Some("task".to_string()),
                ph: EventPhase::Instant,
                ts: now,
                tts: None,
                pid,
                tid: current_thread_id(),
                dur: None,
                tdur: None,
                args: Some(args),
                sf: None,
                stack: None,
                esf: None,
                estack: None,
                s: Some(InstantScope::Thread),
                id: None,
                scope: None,
                cname: Some("good".to_string()),
            })
        }

        TraceEventType::TaskCreateFork { task_id, player } => {
            let mut args = HashMap::new();
            args.insert(
                "task_id".to_string(),
                Value::Number((task_id as u64).into()),
            );
            args.insert("player".to_string(), Value::String(player));

            Some(TraceEvent {
                name: format!("Fork Task ({task_id})"),
                cat: Some("task".to_string()),
                ph: EventPhase::Instant,
                ts: now,
                tts: None,
                pid,
                tid: current_thread_id(),
                dur: None,
                tdur: None,
                args: Some(args),
                sf: None,
                stack: None,
                esf: None,
                estack: None,
                s: Some(InstantScope::Thread),
                id: None,
                scope: None,
                cname: Some("good".to_string()),
            })
        }

        TraceEventType::TaskStart { task_id } => {
            let mut args = HashMap::new();
            args.insert(
                "task_id".to_string(),
                Value::Number((task_id as u64).into()),
            );

            Some(TraceEvent {
                name: format!("Task Execution ({task_id})"),
                cat: Some("task".to_string()),
                ph: EventPhase::Begin,
                ts: now,
                tts: None,
                pid,
                tid: current_thread_id(),
                dur: None,
                tdur: None,
                args: Some(args),
                sf: None,
                stack: None,
                esf: None,
                estack: None,
                s: None,
                id: None,
                scope: None,
                cname: Some("good".to_string()),
            })
        }

        TraceEventType::TaskComplete { task_id, result } => {
            let mut args = HashMap::new();
            args.insert(
                "task_id".to_string(),
                Value::Number((task_id as u64).into()),
            );
            args.insert("result".to_string(), Value::String(result));

            Some(TraceEvent {
                name: format!("Task Complete ({task_id})"),
                cat: Some("task".to_string()),
                ph: EventPhase::End,
                ts: now,
                tts: None,
                pid,
                tid: current_thread_id(),
                dur: None,
                tdur: None,
                args: Some(args),
                sf: None,
                stack: None,
                esf: None,
                estack: None,
                s: None,
                id: None,
                scope: None,
                cname: Some("good".to_string()),
            })
        }

        TraceEventType::TaskSuspend { task_id, reason } => {
            let mut args = HashMap::new();
            args.insert(
                "task_id".to_string(),
                Value::Number((task_id as u64).into()),
            );
            args.insert("reason".to_string(), Value::String(reason.clone()));

            Some(TraceEvent {
                name: format!("Task Suspend ({task_id}): {reason}"),
                cat: Some("task".to_string()),
                ph: EventPhase::Instant,
                ts: now,
                tts: None,
                pid,
                tid: current_thread_id(),
                dur: None,
                tdur: None,
                args: Some(args),
                sf: None,
                stack: None,
                esf: None,
                estack: None,
                s: Some(InstantScope::Thread),
                id: None,
                scope: None,
                cname: Some("terrible".to_string()),
            })
        }

        TraceEventType::TaskResume {
            task_id,
            wake_condition,
            wake_reason,
            return_value,
            max_ticks,
            tick_count,
        } => {
            let mut args = HashMap::new();
            args.insert(
                "task_id".to_string(),
                Value::Number((task_id as u64).into()),
            );
            args.insert(
                "wake_condition".to_string(),
                Value::String(wake_condition.clone()),
            );
            args.insert(
                "wake_reason".to_string(),
                Value::String(wake_reason.clone()),
            );
            args.insert(
                "return_value".to_string(),
                Value::String(return_value.clone()),
            );
            args.insert("max_ticks".to_string(), Value::Number(max_ticks.into()));
            args.insert("tick_count".to_string(), Value::Number(tick_count.into()));
            args.insert(
                "remaining_ticks".to_string(),
                Value::Number((max_ticks.saturating_sub(tick_count)).into()),
            );

            Some(TraceEvent {
                name: format!("Task Resume ({task_id}): {wake_condition} - {wake_reason}"),
                cat: Some("task".to_string()),
                ph: EventPhase::Instant,
                ts: now,
                tts: None,
                pid,
                tid: current_thread_id(),
                dur: None,
                tdur: None,
                args: Some(args),
                sf: None,
                stack: None,
                esf: None,
                estack: None,
                s: Some(InstantScope::Thread),
                id: None,
                scope: None,
                cname: Some("good".to_string()),
            })
        }

        TraceEventType::TaskAbort { task_id, reason } => {
            let mut args = HashMap::new();
            args.insert(
                "task_id".to_string(),
                Value::Number((task_id as u64).into()),
            );
            args.insert("reason".to_string(), Value::String(reason.clone()));

            Some(TraceEvent {
                name: format!("Task Abort ({task_id}): {reason}"),
                cat: Some("task".to_string()),
                ph: EventPhase::End,
                ts: now,
                tts: None,
                pid,
                tid: current_thread_id(),
                dur: None,
                tdur: None,
                args: Some(args),
                sf: None,
                stack: None,
                esf: None,
                estack: None,
                s: None,
                id: None,
                scope: None,
                cname: Some("bad".to_string()),
            })
        }

        TraceEventType::VerbBegin {
            task_id,
            verb_name,
            this,
            definer,
            caller_line,
            args: verb_args,
        } => {
            let mut args = HashMap::new();
            args.insert(
                "task_id".to_string(),
                Value::Number((task_id as u64).into()),
            );
            args.insert("this".to_string(), Value::String(this));
            args.insert("definer".to_string(), Value::String(definer));
            args.insert(
                "args".to_string(),
                Value::Array(verb_args.into_iter().map(Value::String).collect()),
            );
            if let Some(line_num) = caller_line {
                args.insert(
                    "caller_line".to_string(),
                    Value::Number((line_num as u64).into()),
                );
            }

            Some(TraceEvent {
                name: format!("{verb_name} (task {task_id})"),
                cat: Some("verb".to_string()),
                ph: EventPhase::Begin,
                ts: now,
                tts: None,
                pid,
                tid: current_thread_id(),
                dur: None,
                tdur: None,
                args: Some(args),
                sf: None,
                stack: None,
                esf: None,
                estack: None,
                s: None,
                id: None,
                scope: None,
                cname: Some("thread_state_running".to_string()),
            })
        }

        TraceEventType::VerbEnd { task_id, verb_name } => {
            let mut args = HashMap::new();
            args.insert(
                "task_id".to_string(),
                Value::Number((task_id as u64).into()),
            );

            Some(TraceEvent {
                name: format!("{verb_name} (task {task_id})"),
                cat: Some("verb".to_string()),
                ph: EventPhase::End,
                ts: now,
                tts: None,
                pid,
                tid: current_thread_id(),
                dur: None,
                tdur: None,
                args: Some(args),
                sf: None,
                stack: None,
                esf: None,
                estack: None,
                s: None,
                id: None,
                scope: None,
                cname: Some("thread_state_running".to_string()),
            })
        }

        TraceEventType::BuiltinBegin {
            task_id,
            builtin_name,
            caller_line,
            args: builtin_args,
        } => {
            let mut args = HashMap::new();
            args.insert(
                "task_id".to_string(),
                Value::Number((task_id as u64).into()),
            );
            args.insert(
                "args".to_string(),
                Value::Array(builtin_args.into_iter().map(Value::String).collect()),
            );
            if let Some(line_num) = caller_line {
                args.insert(
                    "caller_line".to_string(),
                    Value::Number((line_num as u64).into()),
                );
            }

            Some(TraceEvent {
                name: format!("{builtin_name} (task {task_id})"),
                cat: Some("builtin".to_string()),
                ph: EventPhase::Begin,
                ts: now,
                tts: None,
                pid,
                tid: current_thread_id(),
                dur: None,
                tdur: None,
                args: Some(args),
                sf: None,
                stack: None,
                esf: None,
                estack: None,
                s: None,
                id: None,
                scope: None,
                cname: Some("thread_state_iowait".to_string()),
            })
        }

        TraceEventType::BuiltinEnd {
            task_id,
            builtin_name,
        } => {
            let mut args = HashMap::new();
            args.insert(
                "task_id".to_string(),
                Value::Number((task_id as u64).into()),
            );

            Some(TraceEvent {
                name: format!("{builtin_name} (task {task_id})"),
                cat: Some("builtin".to_string()),
                ph: EventPhase::End,
                ts: now,
                tts: None,
                pid,
                tid: current_thread_id(),
                dur: None,
                tdur: None,
                args: Some(args),
                sf: None,
                stack: None,
                esf: None,
                estack: None,
                s: None,
                id: None,
                scope: None,
                cname: Some("thread_state_iowait".to_string()),
            })
        }

        TraceEventType::OpcodeExecute {
            task_id,
            opcode,
            count,
        } => {
            let mut args = HashMap::new();
            args.insert(
                "task_id".to_string(),
                Value::Number((task_id as u64).into()),
            );
            args.insert("count".to_string(), Value::Number(count.into()));

            Some(TraceEvent {
                name: format!("{opcode} (task {task_id}) [{count}]"),
                cat: Some("opcode".to_string()),
                ph: EventPhase::Counter,
                ts: now,
                tts: None,
                pid,
                tid: current_thread_id(),
                dur: None,
                tdur: None,
                args: Some(args),
                sf: None,
                stack: None,
                esf: None,
                estack: None,
                s: None,
                id: None,
                scope: None,
                cname: None,
            })
        }

        TraceEventType::StackUnwind {
            task_id,
            reason,
            error_message,
            max_ticks,
            tick_count,
        } => {
            let mut args = HashMap::new();
            args.insert(
                "task_id".to_string(),
                Value::Number((task_id as u64).into()),
            );
            args.insert("reason".to_string(), Value::String(reason.clone()));
            args.insert("max_ticks".to_string(), Value::Number(max_ticks.into()));
            args.insert("tick_count".to_string(), Value::Number(tick_count.into()));
            args.insert(
                "remaining_ticks".to_string(),
                Value::Number((max_ticks.saturating_sub(tick_count)).into()),
            );

            if let Some(msg) = error_message {
                args.insert("error_message".to_string(), Value::String(msg));
            }

            Some(TraceEvent {
                name: format!("Stack Unwind (task {task_id}): {reason}"),
                cat: Some("vm".to_string()),
                ph: EventPhase::Instant,
                ts: now,
                tts: None,
                pid,
                tid: current_thread_id(),
                dur: None,
                tdur: None,
                args: Some(args),
                sf: None,
                stack: None,
                esf: None,
                estack: None,
                s: Some(InstantScope::Thread),
                id: None,
                scope: None,
                cname: Some("yellow".to_string()),
            })
        }

        TraceEventType::AbortLimitReached {
            task_id,
            limit_type,
            limit_value,
            max_ticks,
            tick_count,
            verb_name,
            this,
            line_number,
        } => {
            let mut args = HashMap::new();
            args.insert(
                "task_id".to_string(),
                Value::Number((task_id as u64).into()),
            );
            args.insert("limit_type".to_string(), Value::String(limit_type.clone()));
            args.insert(
                "limit_value".to_string(),
                Value::String(limit_value.clone()),
            );
            args.insert("max_ticks".to_string(), Value::Number(max_ticks.into()));
            args.insert("tick_count".to_string(), Value::Number(tick_count.into()));
            args.insert(
                "remaining_ticks".to_string(),
                Value::Number((max_ticks.saturating_sub(tick_count)).into()),
            );
            args.insert("verb_name".to_string(), Value::String(verb_name.clone()));
            args.insert("this".to_string(), Value::String(this.clone()));
            args.insert("line_number".to_string(), Value::Number(line_number.into()));

            Some(TraceEvent {
                name: format!(
                    "Abort Limit (task {task_id}): {limit_type} exceeded at {verb_name}:{line_number}"
                ),
                cat: Some("abort".to_string()),
                ph: EventPhase::Instant,
                ts: now,
                tts: None,
                pid,
                tid: current_thread_id(),
                dur: None,
                tdur: None,
                args: Some(args),
                sf: None,
                stack: None,
                esf: None,
                estack: None,
                s: Some(InstantScope::Thread),
                id: None,
                scope: None,
                cname: Some("bad".to_string()),
            })
        }

        TraceEventType::ExceptionHandlerRequested {
            original_task_id,
            handler_task_id,
            exception_type,
        } => {
            let mut args = HashMap::new();
            args.insert(
                "original_task_id".to_string(),
                Value::String(original_task_id.to_string()),
            );
            args.insert(
                "handler_task_id".to_string(),
                Value::String(handler_task_id.to_string()),
            );
            args.insert(
                "exception_type".to_string(),
                Value::String(exception_type.clone()),
            );

            Some(TraceEvent {
                name: format!(
                    "Exception Handler Requested (task {original_task_id}): {exception_type}"
                ),
                cat: Some("exception".to_string()),
                ph: EventPhase::Instant,
                ts: now,
                tts: None,
                pid,
                tid: current_thread_id(),
                dur: None,
                tdur: None,
                args: Some(args),
                sf: None,
                stack: None,
                esf: None,
                estack: None,
                s: Some(InstantScope::Thread),
                id: None,
                scope: None,
                cname: Some("orange".to_string()),
            })
        }

        TraceEventType::ExceptionHandlerStarted {
            handler_task_id,
            original_task_id,
        } => {
            let mut args = HashMap::new();
            args.insert(
                "handler_task_id".to_string(),
                Value::String(handler_task_id.to_string()),
            );
            args.insert(
                "original_task_id".to_string(),
                Value::String(original_task_id.to_string()),
            );

            Some(TraceEvent {
                name: format!("Exception Handler Started (task {handler_task_id})"),
                cat: Some("exception".to_string()),
                ph: EventPhase::Begin,
                ts: now,
                tts: None,
                pid,
                tid: current_thread_id(),
                dur: None,
                tdur: None,
                args: Some(args),
                sf: None,
                stack: None,
                esf: None,
                estack: None,
                s: None,
                id: None,
                scope: None,
                cname: Some("orange".to_string()),
            })
        }

        TraceEventType::ExceptionHandlerCompleted {
            handler_task_id,
            original_task_id,
            handler_returned_true,
        } => {
            let mut args = HashMap::new();
            args.insert(
                "handler_task_id".to_string(),
                Value::String(handler_task_id.to_string()),
            );
            args.insert(
                "original_task_id".to_string(),
                Value::String(original_task_id.to_string()),
            );
            args.insert(
                "suppress_traceback".to_string(),
                Value::Bool(handler_returned_true),
            );

            Some(TraceEvent {
                name: format!(
                    "Exception Handler Completed (task {handler_task_id}): suppress_traceback={}",
                    handler_returned_true
                ),
                cat: Some("exception".to_string()),
                ph: EventPhase::End,
                ts: now,
                tts: None,
                pid,
                tid: current_thread_id(),
                dur: None,
                tdur: None,
                args: Some(args),
                sf: None,
                stack: None,
                esf: None,
                estack: None,
                s: None,
                id: None,
                scope: None,
                cname: Some("orange".to_string()),
            })
        }

        TraceEventType::SchedulerTick {
            active_tasks,
            queued_tasks,
        } => {
            let mut args = HashMap::new();
            args.insert("active".to_string(), Value::Number(active_tasks.into()));
            args.insert("queued".to_string(), Value::Number(queued_tasks.into()));

            Some(TraceEvent {
                name: "Scheduler Tick".to_string(),
                cat: Some("scheduler".to_string()),
                ph: EventPhase::Counter,
                ts: now,
                tts: None,
                pid,
                tid: 0, // Scheduler uses thread 0
                dur: None,
                tdur: None,
                args: Some(args),
                sf: None,
                stack: None,
                esf: None,
                estack: None,
                s: None,
                id: None,
                scope: None,
                cname: None,
            })
        }

        // Transaction events
        TraceEventType::TransactionBegin { tx_id, thread_id } => {
            let mut args = HashMap::new();
            args.insert("tx_id".to_string(), Value::String(tx_id.clone()));

            Some(TraceEvent {
                name: format!("Transaction {tx_id}"),
                cat: Some("database".to_string()),
                ph: EventPhase::Begin,
                ts: now,
                tts: None,
                pid,
                tid: thread_id,
                dur: None,
                tdur: None,
                args: Some(args),
                sf: None,
                stack: None,
                esf: None,
                estack: None,
                s: None,
                id: None,
                scope: None,
                cname: Some("blue".to_string()),
            })
        }

        TraceEventType::TransactionCheck {
            tx_id,
            thread_id,
            num_tuples,
        } => {
            let mut args = HashMap::new();
            args.insert("tx_id".to_string(), Value::String(tx_id.clone()));
            args.insert("num_tuples".to_string(), Value::Number(num_tuples.into()));

            Some(TraceEvent {
                name: format!("TX Check: {tx_id}"),
                cat: Some("database".to_string()),
                ph: EventPhase::Begin,
                ts: now,
                tts: None,
                pid,
                tid: thread_id,
                dur: None,
                tdur: None,
                args: Some(args),
                sf: None,
                stack: None,
                esf: None,
                estack: None,
                s: None,
                id: None,
                scope: None,
                cname: Some("orange".to_string()),
            })
        }

        TraceEventType::TransactionApply {
            tx_id,
            thread_id,
            num_tuples,
        } => {
            let mut args = HashMap::new();
            args.insert("tx_id".to_string(), Value::String(tx_id.clone()));
            args.insert("num_tuples".to_string(), Value::Number(num_tuples.into()));

            Some(TraceEvent {
                name: format!("TX Apply: {tx_id}"),
                cat: Some("database".to_string()),
                ph: EventPhase::Begin,
                ts: now,
                tts: None,
                pid,
                tid: thread_id,
                dur: None,
                tdur: None,
                args: Some(args),
                sf: None,
                stack: None,
                esf: None,
                estack: None,
                s: None,
                id: None,
                scope: None,
                cname: Some("red".to_string()),
            })
        }

        TraceEventType::TransactionCommit {
            tx_id,
            thread_id,
            success,
            timestamp,
        } => {
            let mut args = HashMap::new();
            args.insert("tx_id".to_string(), Value::String(tx_id.clone()));
            args.insert("success".to_string(), Value::Bool(success));
            args.insert("timestamp".to_string(), Value::Number(timestamp.into()));

            let event_name = if success {
                format!("TX Commit: {tx_id}")
            } else {
                format!("TX Conflict: {tx_id}")
            };

            Some(TraceEvent {
                name: event_name,
                cat: Some("database".to_string()),
                ph: EventPhase::Instant,
                ts: now,
                tts: None,
                pid,
                tid: thread_id,
                dur: None,
                tdur: None,
                args: Some(args),
                sf: None,
                stack: None,
                esf: None,
                estack: None,
                s: Some(InstantScope::Thread),
                id: None,
                scope: None,
                cname: if success {
                    Some("green".to_string())
                } else {
                    Some("red".to_string())
                },
            })
        }

        TraceEventType::TransactionRollback {
            tx_id,
            thread_id,
            reason,
        } => {
            let mut args = HashMap::new();
            args.insert("tx_id".to_string(), Value::String(tx_id.clone()));
            args.insert("reason".to_string(), Value::String(reason));

            Some(TraceEvent {
                name: format!("TX Rollback: {tx_id}"),
                cat: Some("database".to_string()),
                ph: EventPhase::Instant,
                ts: now,
                tts: None,
                pid,
                tid: thread_id,
                dur: None,
                tdur: None,
                args: Some(args),
                sf: None,
                stack: None,
                esf: None,
                estack: None,
                s: Some(InstantScope::Thread),
                id: None,
                scope: None,
                cname: Some("red".to_string()),
            })
        }

        TraceEventType::TransactionEnd { tx_id, thread_id } => {
            let mut args = HashMap::new();
            args.insert("tx_id".to_string(), Value::String(tx_id.clone()));

            Some(TraceEvent {
                name: format!("Transaction {tx_id}"),
                cat: Some("database".to_string()),
                ph: EventPhase::End,
                ts: now,
                tts: None,
                pid,
                tid: thread_id,
                dur: None,
                tdur: None,
                args: Some(args),
                sf: None,
                stack: None,
                esf: None,
                estack: None,
                s: None,
                id: None,
                scope: None,
                cname: Some("good".to_string()),
            })
        }
    }
}

/// Clean helper functions for emitting trace events (zero-cost when feature disabled)
/// Macro to emit task creation events for commands - generates no code when trace_events is disabled
#[macro_export]
macro_rules! trace_task_create_command {
    ($task_id:expr, $player:expr, $command:expr, $handler_object:expr) => {
        #[cfg(feature = "trace_events")]
        {
            use $crate::tracing_events::{TraceEventType, emit_trace_event};
            emit_trace_event(TraceEventType::TaskCreateCommand {
                task_id: $task_id,
                player: format!("{}", $player),
                command: $command.to_string(),
                handler_object: format!("{}", $handler_object),
            });
        }
    };
}

/// Macro to emit task creation events for verbs - generates no code when trace_events is disabled
#[macro_export]
macro_rules! trace_task_create_verb {
    ($task_id:expr, $player:expr, $verb:expr, $vloc:expr) => {
        #[cfg(feature = "trace_events")]
        {
            use moor_compiler::to_literal;
            use $crate::tracing_events::{TraceEventType, emit_trace_event};
            emit_trace_event(TraceEventType::TaskCreateVerb {
                task_id: $task_id,
                player: format!("{}", $player),
                verb: $verb.to_string(),
                vloc: format!("{}", to_literal($vloc)),
            });
        }
    };
}

/// Macro to emit task creation events for eval - generates no code when trace_events is disabled
#[macro_export]
macro_rules! trace_task_create_eval {
    ($task_id:expr, $player:expr) => {
        #[cfg(feature = "trace_events")]
        {
            use $crate::tracing_events::{TraceEventType, emit_trace_event};
            emit_trace_event(TraceEventType::TaskCreateEval {
                task_id: $task_id,
                player: format!("{}", $player),
            });
        }
    };
}

/// Macro to emit task creation events for OOB commands - accepts slice and joins only when tracing enabled
#[macro_export]
macro_rules! trace_task_create_oob {
    ($task_id:expr, $player:expr, $command_slice:expr) => {
        #[cfg(feature = "trace_events")]
        {
            use $crate::tracing_events::{TraceEventType, emit_trace_event};
            emit_trace_event(TraceEventType::TaskCreateOOB {
                task_id: $task_id,
                player: format!("{}", $player),
                command: $command_slice.join(" "),
            });
        }
    };
}

/// Macro to emit task creation events for fork - generates no code when trace_events is disabled
#[macro_export]
macro_rules! trace_task_create_fork {
    ($task_id:expr, $player:expr) => {
        #[cfg(feature = "trace_events")]
        {
            use $crate::tracing_events::{TraceEventType, emit_trace_event};
            emit_trace_event(TraceEventType::TaskCreateFork {
                task_id: $task_id,
                player: format!("{}", $player),
            });
        }
    };
}

/// Macro to emit task creation events for exception handlers - generates no code when trace_events is disabled
#[macro_export]
macro_rules! trace_task_create_exception_handler {
    ($task_id:expr, $player:expr) => {
        #[cfg(feature = "trace_events")]
        {
            use $crate::tracing_events::{TraceEventType, emit_trace_event};
            emit_trace_event(TraceEventType::TaskCreateFork {
                task_id: $task_id,
                player: format!("{}", $player),
            });
        }
    };
}

/// Macro to emit task start events - generates no code when trace_events is disabled
#[macro_export]
macro_rules! trace_task_start {
    ($task_id:expr) => {
        #[cfg(feature = "trace_events")]
        {
            use $crate::tracing_events::{TraceEventType, emit_trace_event};
            emit_trace_event(TraceEventType::TaskStart { task_id: $task_id });
        }
    };
}

/// Macro to emit task completion events - generates no code when trace_events is disabled
#[macro_export]
macro_rules! trace_task_complete {
    ($task_id:expr, $result:expr) => {
        #[cfg(feature = "trace_events")]
        {
            use $crate::tracing_events::{TraceEventType, emit_trace_event};
            emit_trace_event(TraceEventType::TaskComplete {
                task_id: $task_id,
                result: $result.to_string(),
            });
        }
    };
}

/// Macro to emit task suspend events - generates no code when trace_events is disabled
#[macro_export]
macro_rules! trace_task_suspend {
    ($task_id:expr, $reason:expr) => {
        #[cfg(feature = "trace_events")]
        {
            use $crate::tracing_events::{TraceEventType, emit_trace_event};
            emit_trace_event(TraceEventType::TaskSuspend {
                task_id: $task_id,
                reason: $reason.to_string(),
            });
        }
    };
}

/// Macro to emit task suspend events with delay information - generates no code when trace_events is disabled
#[macro_export]
macro_rules! trace_task_suspend_with_delay {
    ($task_id:expr, $delay:expr) => {
        #[cfg(feature = "trace_events")]
        {
            use $crate::{
                tracing_events::{TraceEventType, emit_trace_event},
                vm::TaskSuspend,
            };
            let reason = match $delay {
                TaskSuspend::Commit(_) => "Commit".to_string(),
                TaskSuspend::Timed(d) => format!("Timed({:?})", d),
                TaskSuspend::Never => "Never".to_string(),
                TaskSuspend::WaitTask(tid) => format!("WaitTask({})", tid),
                TaskSuspend::WorkerRequest(sym, args, timeout) => format!(
                    "WorkerRequest({}, {} args, {:?})",
                    sym.as_string(),
                    args.len(),
                    timeout
                ),
                TaskSuspend::RecvMessages(timeout) => format!("RecvMessages({:?})", timeout),
            };
            emit_trace_event(TraceEventType::TaskSuspend {
                task_id: $task_id,
                reason,
            });
        }
    };
}

/// Macro to emit task resume events - generates no code when trace_events is disabled
#[macro_export]
macro_rules! trace_task_resume {
    ($task_id:expr, $wake_condition:expr, $wake_reason:expr, $return_value:expr, $max_ticks:expr, $tick_count:expr) => {
        #[cfg(feature = "trace_events")]
        {
            use $crate::tracing_events::{TraceEventType, emit_trace_event};
            emit_trace_event(TraceEventType::TaskResume {
                task_id: $task_id,
                wake_condition: $wake_condition.to_string(),
                wake_reason: $wake_reason.to_string(),
                return_value: $return_value,
                max_ticks: $max_ticks,
                tick_count: $tick_count,
            });
        }
    };
}

/// Macro to emit task abort events - generates no code when trace_events is disabled
#[macro_export]
macro_rules! trace_task_abort {
    ($task_id:expr, $reason:expr) => {
        #[cfg(feature = "trace_events")]
        {
            use $crate::tracing_events::{TraceEventType, emit_trace_event};
            emit_trace_event(TraceEventType::TaskAbort {
                task_id: $task_id,
                reason: $reason.to_string(),
            });
        }
    };
}

/// Macro to emit verb begin events - generates no code when trace_events is disabled
#[macro_export]
macro_rules! trace_verb_begin {
    ($task_id:expr, $verb_name:expr, $this:expr, $definer:expr, $caller_line:expr, $args:expr) => {
        #[cfg(feature = "trace_events")]
        {
            use moor_compiler::to_literal;
            use moor_var::v_obj;
            use $crate::tracing_events::{TraceEventType, emit_trace_event};
            let args_literals: Vec<String> = $args.iter().map(|arg| to_literal(&arg)).collect();
            emit_trace_event(TraceEventType::VerbBegin {
                task_id: $task_id,
                verb_name: $verb_name.to_string(),
                this: format!("{}", to_literal($this)),
                definer: format!("{}", to_literal(&v_obj(*$definer))),
                caller_line: $caller_line,
                args: args_literals,
            });
        }
    };
}

/// Macro to emit verb end events - generates no code when trace_events is disabled
#[macro_export]
macro_rules! trace_verb_end {
    ($task_id:expr, $verb_name:expr) => {
        #[cfg(feature = "trace_events")]
        {
            use $crate::tracing_events::{TraceEventType, emit_trace_event};
            emit_trace_event(TraceEventType::VerbEnd {
                task_id: $task_id,
                verb_name: $verb_name.to_string(),
            });
        }
    };
}

/// Macro to emit builtin begin events - generates no code when trace_events is disabled
#[macro_export]
macro_rules! trace_builtin_begin {
    ($task_id:expr, $builtin_name:expr, $caller_line:expr, $args:expr) => {
        #[cfg(feature = "trace_events")]
        {
            use moor_compiler::to_literal;
            use $crate::tracing_events::{TraceEventType, emit_trace_event};
            let args_literals: Vec<String> = $args.iter().map(|arg| to_literal(&arg)).collect();
            emit_trace_event(TraceEventType::BuiltinBegin {
                task_id: $task_id,
                builtin_name: $builtin_name.to_string(),
                caller_line: $caller_line,
                args: args_literals,
            });
        }
    };
}

/// Macro to emit builtin end events - generates no code when trace_events is disabled
#[macro_export]
macro_rules! trace_builtin_end {
    ($task_id:expr, $builtin_name:expr) => {
        #[cfg(feature = "trace_events")]
        {
            use $crate::tracing_events::{TraceEventType, emit_trace_event};
            emit_trace_event(TraceEventType::BuiltinEnd {
                task_id: $task_id,
                builtin_name: $builtin_name.to_string(),
            });
        }
    };
}

/// Macro to emit stack unwind events - generates no code when trace_events is disabled
#[macro_export]
macro_rules! trace_stack_unwind {
    ($task_id:expr, $reason:expr) => {
        #[cfg(feature = "trace_events")]
        {
            use $crate::tracing_events::{TraceEventType, emit_trace_event};
            emit_trace_event(TraceEventType::StackUnwind {
                task_id: $task_id,
                reason: $reason.to_string(),
                error_message: None,
                max_ticks: 0,
                tick_count: 0,
            });
        }
    };
    ($task_id:expr, $reason:expr, $error_message:expr, $max_ticks:expr, $tick_count:expr) => {
        #[cfg(feature = "trace_events")]
        {
            use $crate::tracing_events::{TraceEventType, emit_trace_event};
            emit_trace_event(TraceEventType::StackUnwind {
                task_id: $task_id,
                reason: $reason.to_string(),
                error_message: $error_message,
                max_ticks: $max_ticks,
                tick_count: $tick_count,
            });
        }
    };
}

// Transaction tracing macros
#[macro_export]
macro_rules! trace_transaction_begin {
    ($tx_id:expr, $thread_id:expr) => {
        #[cfg(feature = "trace_events")]
        {
            use $crate::tracing_events::{TraceEventType, emit_trace_event};
            emit_trace_event(TraceEventType::TransactionBegin {
                tx_id: $tx_id.to_string(),
                thread_id: $thread_id,
            });
        }
    };
}

#[macro_export]
macro_rules! trace_transaction_check {
    ($tx_id:expr, $thread_id:expr, $num_tuples:expr) => {
        #[cfg(feature = "trace_events")]
        {
            use $crate::tracing_events::{TraceEventType, emit_trace_event};
            emit_trace_event(TraceEventType::TransactionCheck {
                tx_id: $tx_id.to_string(),
                thread_id: $thread_id,
                num_tuples: $num_tuples,
            });
        }
    };
}

#[macro_export]
macro_rules! trace_transaction_apply {
    ($tx_id:expr, $thread_id:expr, $num_tuples:expr) => {
        #[cfg(feature = "trace_events")]
        {
            use $crate::tracing_events::{TraceEventType, emit_trace_event};
            emit_trace_event(TraceEventType::TransactionApply {
                tx_id: $tx_id.to_string(),
                thread_id: $thread_id,
                num_tuples: $num_tuples,
            });
        }
    };
}

#[macro_export]
macro_rules! trace_transaction_commit {
    ($tx_id:expr, $thread_id:expr, $success:expr, $timestamp:expr) => {
        #[cfg(feature = "trace_events")]
        {
            use $crate::tracing_events::{TraceEventType, emit_trace_event};
            emit_trace_event(TraceEventType::TransactionCommit {
                tx_id: $tx_id.to_string(),
                thread_id: $thread_id,
                success: $success,
                timestamp: $timestamp,
            });
        }
    };
}

#[macro_export]
macro_rules! trace_transaction_rollback {
    ($tx_id:expr, $thread_id:expr, $reason:expr) => {
        #[cfg(feature = "trace_events")]
        {
            use $crate::tracing_events::{TraceEventType, emit_trace_event};
            emit_trace_event(TraceEventType::TransactionRollback {
                tx_id: $tx_id.to_string(),
                thread_id: $thread_id,
                reason: $reason.to_string(),
            });
        }
    };
}

/// Macro to emit abort limit reached events - generates no code when trace_events is disabled
#[macro_export]
macro_rules! trace_abort_limit_reached {
    (
        $task_id:expr,
        $limit_type:expr,
        $limit_value:expr,
        $max_ticks:expr,
        $tick_count:expr,
        $verb_name:expr,
        $this:expr,
        $line_number:expr
    ) => {
        #[cfg(feature = "trace_events")]
        {
            use moor_compiler::to_literal;
            use $crate::tracing_events::{TraceEventType, emit_trace_event};
            emit_trace_event(TraceEventType::AbortLimitReached {
                task_id: $task_id,
                limit_type: $limit_type.to_string(),
                limit_value: $limit_value,
                max_ticks: $max_ticks,
                tick_count: $tick_count,
                verb_name: $verb_name.to_string(),
                this: format!("{}", to_literal(&$this)),
                line_number: $line_number,
            });
        }
    };
}

/// Macro to emit exception handler requested events - generates no code when trace_events is disabled
#[macro_export]
macro_rules! trace_exception_handler_requested {
    ($original_task_id:expr, $handler_task_id:expr, $exception_type:expr) => {
        #[cfg(feature = "trace_events")]
        {
            use $crate::tracing_events::{TraceEventType, emit_trace_event};
            emit_trace_event(TraceEventType::ExceptionHandlerRequested {
                original_task_id: $original_task_id,
                handler_task_id: $handler_task_id,
                exception_type: $exception_type.to_string(),
            });
        }
    };
}

/// Macro to emit exception handler started events - generates no code when trace_events is disabled
#[macro_export]
macro_rules! trace_exception_handler_started {
    ($handler_task_id:expr, $original_task_id:expr) => {
        #[cfg(feature = "trace_events")]
        {
            use $crate::tracing_events::{TraceEventType, emit_trace_event};
            emit_trace_event(TraceEventType::ExceptionHandlerStarted {
                handler_task_id: $handler_task_id,
                original_task_id: $original_task_id,
            });
        }
    };
}

/// Macro to emit exception handler completed events - generates no code when trace_events is disabled
#[macro_export]
macro_rules! trace_exception_handler_completed {
    ($handler_task_id:expr, $original_task_id:expr, $handler_returned_true:expr) => {
        #[cfg(feature = "trace_events")]
        {
            use $crate::tracing_events::{TraceEventType, emit_trace_event};
            emit_trace_event(TraceEventType::ExceptionHandlerCompleted {
                handler_task_id: $handler_task_id,
                original_task_id: $original_task_id,
                handler_returned_true: $handler_returned_true,
            });
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracing_disabled_by_default() {
        // When feature is disabled, these should be no-ops
        // These should not panic
        emit_trace_event(TraceEventType::TaskStart { task_id: 1 });
        shutdown_tracing();
    }

    #[test]
    #[cfg(feature = "trace_events")]
    fn test_tracing_initialization() {
        use std::time::Duration;
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let output_path = temp_dir.path().join("test_trace.json");

        assert!(init_tracing(Some(output_path.clone())));

        // Should have a sender now
        assert!(TRACING_SENDER.lock().unwrap().is_some());

        // Send a test event
        emit_trace_event(TraceEventType::TaskStart { task_id: 1 });

        // Give the background thread a moment to process
        std::thread::sleep(Duration::from_millis(100));

        shutdown_tracing();

        // Give shutdown time to complete
        std::thread::sleep(Duration::from_millis(100));

        // File should exist
        assert!(output_path.exists());
    }
}
