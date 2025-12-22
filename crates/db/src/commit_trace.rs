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

//! Commit trace logging for debugging transaction latency.
//! Enable with the `commit_trace` feature flag.

use std::io::Write;
use std::sync::Mutex;
use std::time::Instant;

use lazy_static::lazy_static;

lazy_static! {
    static ref TRACE_START: Instant = Instant::now();
    static ref TRACE_FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);
}

/// Initialize the trace log file. Call once at startup.
pub fn init_trace(path: &str) {
    let file = std::fs::File::create(path).expect("Failed to create trace file");
    *TRACE_FILE.lock().unwrap() = Some(file);
}

/// Log a trace event. Format: timestamp_ns,thread_name,event,commit_id
#[inline]
pub fn trace_event(event: &str, commit_id: u64) {
    let elapsed = TRACE_START.elapsed().as_nanos() as u64;
    let thread_name = std::thread::current()
        .name()
        .unwrap_or("unknown")
        .to_string();

    if let Ok(mut guard) = TRACE_FILE.lock() {
        if let Some(ref mut file) = *guard {
            writeln!(file, "{},{},{},{}", elapsed, thread_name, event, commit_id).ok();
        }
    }
}

/// Flush the trace file
pub fn flush_trace() {
    if let Ok(mut guard) = TRACE_FILE.lock() {
        if let Some(ref mut file) = *guard {
            file.flush().ok();
        }
    }
}
