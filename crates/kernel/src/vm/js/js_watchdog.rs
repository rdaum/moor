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

//! JavaScript execution watchdog.
//! Monitors running JS executions and interrupts them if they exceed time/tick limits or are killed.

use lazy_static::lazy_static;
use minstant::Instant;
use moor_common::tasks::TaskId;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{debug, warn};
use v8;

/// Microseconds per tick (approximate conversion for time-based enforcement)
const MICROS_PER_TICK: u64 = 1000;

lazy_static! {
    static ref JS_WATCHDOG: JSWatchdog = JSWatchdog::new();
}

/// Tracks state for a single JS execution being monitored
struct ExecutionState {
    /// Thread-safe handle to the V8 isolate
    handle: v8::IsolateHandle,
    /// When this execution started
    start: Instant,
    /// Maximum time based on tick budget (ticks_remaining * MICROS_PER_TICK)
    tick_budget: Duration,
    /// Maximum wall-clock time remaining for the entire task
    time_budget: Duration,
    /// Kill switch shared with the task's RunningTask
    kill_flag: Arc<AtomicBool>,
}

/// Global watchdog that monitors all JS executions
pub(crate) struct JSWatchdog {
    /// Map of task_id -> execution state
    executions: Mutex<HashMap<TaskId, ExecutionState>>,
    /// Flag to signal watchdog thread to stop
    shutdown: Arc<AtomicBool>,
}

impl JSWatchdog {
    fn new() -> Self {
        let watchdog = Self {
            executions: Mutex::new(HashMap::new()),
            shutdown: Arc::new(AtomicBool::new(false)),
        };

        // Spawn the monitoring thread
        let shutdown_flag = watchdog.shutdown.clone();
        std::thread::Builder::new()
            .name("js-watchdog".to_string())
            .spawn(move || {
                debug!("JS watchdog thread started");
                Self::watchdog_loop(shutdown_flag);
                debug!("JS watchdog thread stopped");
            })
            .expect("Failed to spawn JS watchdog thread");

        watchdog
    }

    /// Watchdog monitoring loop
    fn watchdog_loop(shutdown: Arc<AtomicBool>) {
        while !shutdown.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(10));

            let executions = JS_WATCHDOG.executions.lock().unwrap();
            for (&task_id, state) in executions.iter() {
                let elapsed = state.start.elapsed();

                // Check kill flag first
                if state.kill_flag.load(Ordering::Relaxed) {
                    debug!(?task_id, "JS task killed");
                    state
                        .handle
                        .request_interrupt(Self::kill_callback, task_id as *mut std::ffi::c_void);
                    continue;
                }

                // Check tick budget
                if elapsed >= state.tick_budget {
                    warn!(?task_id, "JS task exceeded tick budget");
                    state.handle.request_interrupt(
                        Self::tick_abort_callback,
                        task_id as *mut std::ffi::c_void,
                    );
                    continue;
                }

                // Check time budget
                if elapsed >= state.time_budget {
                    warn!(?task_id, "JS task exceeded time budget");
                    state.handle.request_interrupt(
                        Self::time_abort_callback,
                        task_id as *mut std::ffi::c_void,
                    );
                }
            }
        }
    }

    /// Callback invoked when task is killed, ticks exceeded, or time exceeded
    /// Simply terminates execution - the outer exec_interpreter will check limits and report the proper reason
    extern "C" fn kill_callback(isolate: &mut v8::Isolate, _data: *mut std::ffi::c_void) {
        isolate.terminate_execution();
    }

    /// Callback invoked when tick budget exceeded
    extern "C" fn tick_abort_callback(isolate: &mut v8::Isolate, _data: *mut std::ffi::c_void) {
        isolate.terminate_execution();
    }

    /// Callback invoked when time budget exceeded
    extern "C" fn time_abort_callback(isolate: &mut v8::Isolate, _data: *mut std::ffi::c_void) {
        isolate.terminate_execution();
    }
}

/// Register a JS execution with the watchdog
pub(crate) fn register_execution(
    task_id: TaskId,
    handle: v8::IsolateHandle,
    ticks_remaining: usize,
    time_remaining: Duration,
    kill_flag: Arc<AtomicBool>,
) {
    let tick_budget = Duration::from_micros(ticks_remaining as u64 * MICROS_PER_TICK);
    let state = ExecutionState {
        handle,
        start: Instant::now(),
        tick_budget,
        time_budget: time_remaining,
        kill_flag,
    };

    let mut executions = JS_WATCHDOG.executions.lock().unwrap();
    executions.insert(task_id, state);
}

/// Unregister a JS execution from the watchdog
pub(crate) fn unregister_execution(task_id: TaskId) {
    let mut executions = JS_WATCHDOG.executions.lock().unwrap();
    executions.remove(&task_id);
}

/// Check if an exception is a watchdog-generated abort
pub(crate) fn is_watchdog_exception(exception_str: &str) -> Option<WatchdogAbortReason> {
    if exception_str.contains("__MOOR_TASK_KILLED__") {
        Some(WatchdogAbortReason::Killed)
    } else if exception_str.contains("__MOOR_TICKS_EXCEEDED__") {
        Some(WatchdogAbortReason::TicksExceeded)
    } else if exception_str.contains("__MOOR_TIME_EXCEEDED__") {
        Some(WatchdogAbortReason::TimeExceeded)
    } else {
        None
    }
}

/// Reason for watchdog abort
#[derive(Debug, Clone, Copy)]
pub(crate) enum WatchdogAbortReason {
    Killed,
    TicksExceeded,
    TimeExceeded,
}

/// RAII guard that automatically unregisters a JS execution when dropped
pub(crate) struct WatchdogGuard {
    task_id: TaskId,
}

impl WatchdogGuard {
    pub fn new(task_id: TaskId) -> Self {
        Self { task_id }
    }
}

impl Drop for WatchdogGuard {
    fn drop(&mut self) {
        unregister_execution(self.task_id);
    }
}
