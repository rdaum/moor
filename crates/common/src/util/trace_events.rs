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

//! Chrome Trace Event Format implementation
//!
//! This module provides types for generating Chrome Trace Event Format compatible JSON
//! for performance tracing and visualization in Chrome DevTools or other compatible tools.

use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

/// A single trace event in the Chrome Trace Event Format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent {
    /// The name of the event, as displayed in Trace Viewer
    pub name: String,

    /// The event categories. This is a comma separated list of categories for the event
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cat: Option<String>,

    /// The event type. This is a single character which changes depending on the type of event
    pub ph: EventPhase,

    /// The tracing clock timestamp of the event (microseconds)
    pub ts: u64,

    /// Optional thread clock timestamp of the event (microseconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tts: Option<u64>,

    /// The process ID for the process that output this event
    pub pid: u64,

    /// The thread ID for the thread that output this event
    pub tid: u64,

    /// Duration for complete events (microseconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dur: Option<u64>,

    /// Thread duration for complete events (microseconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tdur: Option<u64>,

    /// Event-specific arguments
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<HashMap<String, serde_json::Value>>,

    /// Stack frame ID for stack traces
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sf: Option<String>,

    /// Direct stack trace (array of strings)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack: Option<Vec<String>>,

    /// End stack frame ID for complete events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub esf: Option<String>,

    /// End stack trace for complete events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estack: Option<Vec<String>>,

    /// Scope for instant events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub s: Option<InstantScope>,

    /// ID for async, object, and flow events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// Scope string to avoid ID conflicts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,

    /// Fixed color name for the event
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cname: Option<String>,
}

/// Event phases (single character event type identifiers)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum EventPhase {
    /// Duration event begin
    #[serde(rename = "B")]
    Begin,
    /// Duration event end
    #[serde(rename = "E")]
    End,
    /// Complete event (contains duration)
    #[serde(rename = "X")]
    Complete,
    /// Instant event
    #[serde(rename = "i")]
    Instant,
    /// Counter event
    #[serde(rename = "C")]
    Counter,
    /// Async event start (nestable)
    #[serde(rename = "b")]
    AsyncBegin,
    /// Async event instant (nestable)
    #[serde(rename = "n")]
    AsyncInstant,
    /// Async event end (nestable)
    #[serde(rename = "e")]
    AsyncEnd,
    /// Flow event start
    #[serde(rename = "s")]
    FlowStart,
    /// Flow event step
    #[serde(rename = "t")]
    FlowStep,
    /// Flow event end
    #[serde(rename = "f")]
    FlowEnd,
    /// Sample event (deprecated)
    #[serde(rename = "P")]
    Sample,
    /// Object created
    #[serde(rename = "N")]
    ObjectCreated,
    /// Object snapshot
    #[serde(rename = "O")]
    ObjectSnapshot,
    /// Object destroyed
    #[serde(rename = "D")]
    ObjectDestroyed,
    /// Metadata event
    #[serde(rename = "M")]
    Metadata,
    /// Global memory dump
    #[serde(rename = "V")]
    GlobalMemoryDump,
    /// Process memory dump
    #[serde(rename = "v")]
    ProcessMemoryDump,
    /// Mark event
    #[serde(rename = "R")]
    Mark,
    /// Clock sync event
    #[serde(rename = "c")]
    ClockSync,
    /// Context enter
    #[serde(rename = "(")]
    ContextEnter,
    /// Context leave
    #[serde(rename = ")")]
    ContextLeave,
}

/// Scope for instant events
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum InstantScope {
    /// Global scope
    #[serde(rename = "g")]
    Global,
    /// Process scope
    #[serde(rename = "p")]
    Process,
    /// Thread scope
    #[serde(rename = "t")]
    Thread,
}

/// Stack frame information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackFrame {
    /// Function/method name
    pub name: String,
    /// Category (e.g., library name)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Parent stack frame ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

/// Complete trace file in Chrome Trace Event Format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceFile {
    /// Array of trace events
    #[serde(rename = "traceEvents")]
    pub trace_events: Vec<TraceEvent>,

    /// Time unit for timestamps ("ms" or "ns")
    #[serde(rename = "displayTimeUnit", skip_serializing_if = "Option::is_none")]
    pub display_time_unit: Option<String>,

    /// Stack frames dictionary
    #[serde(rename = "stackFrames", skip_serializing_if = "Option::is_none")]
    pub stack_frames: Option<HashMap<String, StackFrame>>,

    /// System trace events (Linux ftrace data)
    #[serde(rename = "systemTraceEvents", skip_serializing_if = "Option::is_none")]
    pub system_trace_events: Option<String>,

    /// Power trace data
    #[serde(rename = "powerTraceAsString", skip_serializing_if = "Option::is_none")]
    pub power_trace_as_string: Option<String>,

    /// Controller trace data key
    #[serde(
        rename = "controllerTraceDataKey",
        skip_serializing_if = "Option::is_none"
    )]
    pub controller_trace_data_key: Option<String>,

    /// Additional metadata
    #[serde(flatten)]
    pub other_data: HashMap<String, serde_json::Value>,
}

/// Builder for creating trace events more ergonomically
#[derive(Debug, Clone)]
pub struct TraceEventBuilder {
    event: TraceEvent,
}

impl TraceEventBuilder {
    /// Create a new trace event builder
    pub fn new(name: impl Into<String>, phase: EventPhase, pid: u64, tid: u64) -> Self {
        Self {
            event: TraceEvent {
                name: name.into(),
                ph: phase,
                ts: current_timestamp_us(),
                pid,
                tid,
                cat: None,
                tts: None,
                dur: None,
                tdur: None,
                args: None,
                sf: None,
                stack: None,
                esf: None,
                estack: None,
                s: None,
                id: None,
                scope: None,
                cname: None,
            },
        }
    }

    /// Set the category
    pub fn category(mut self, cat: impl Into<String>) -> Self {
        self.event.cat = Some(cat.into());
        self
    }

    /// Set the timestamp (microseconds since Unix epoch)
    pub fn timestamp(mut self, ts: u64) -> Self {
        self.event.ts = ts;
        self
    }

    /// Set the duration (microseconds)
    pub fn duration(mut self, dur: u64) -> Self {
        self.event.dur = Some(dur);
        self
    }

    /// Add an argument
    pub fn arg(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.event
            .args
            .get_or_insert_with(HashMap::new)
            .insert(key.into(), value.into());
        self
    }

    /// Set multiple arguments
    pub fn args(mut self, args: HashMap<String, serde_json::Value>) -> Self {
        self.event.args = Some(args);
        self
    }

    /// Set the stack trace
    pub fn stack(mut self, stack: Vec<String>) -> Self {
        self.event.stack = Some(stack);
        self
    }

    /// Set the stack frame ID
    pub fn stack_frame(mut self, sf: impl Into<String>) -> Self {
        self.event.sf = Some(sf.into());
        self
    }

    /// Set the scope for instant events
    pub fn scope_instant(mut self, scope: InstantScope) -> Self {
        self.event.s = Some(scope);
        self
    }

    /// Set the ID for async/flow/object events
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.event.id = Some(id.into());
        self
    }

    /// Set the scope string
    pub fn scope_str(mut self, scope: impl Into<String>) -> Self {
        self.event.scope = Some(scope.into());
        self
    }

    /// Set the color name
    pub fn color(mut self, color: impl Into<String>) -> Self {
        self.event.cname = Some(color.into());
        self
    }

    /// Build the trace event
    pub fn build(self) -> TraceEvent {
        self.event
    }
}

impl TraceFile {
    /// Create a new empty trace file
    pub fn new() -> Self {
        Self {
            trace_events: Vec::new(),
            display_time_unit: Some("ms".to_string()),
            stack_frames: None,
            system_trace_events: None,
            power_trace_as_string: None,
            controller_trace_data_key: None,
            other_data: HashMap::new(),
        }
    }

    /// Add a trace event
    pub fn add_event(&mut self, event: TraceEvent) {
        self.trace_events.push(event);
    }

    /// Add a stack frame
    pub fn add_stack_frame(&mut self, id: String, frame: StackFrame) {
        self.stack_frames
            .get_or_insert_with(HashMap::new)
            .insert(id, frame);
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Convert to compact JSON string
    pub fn to_json_compact(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

impl Default for TraceFile {
    fn default() -> Self {
        Self::new()
    }
}

/// Get current timestamp in microseconds since Unix epoch
pub fn current_timestamp_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64
}

/// Convenience function to create a duration begin event
pub fn duration_begin(name: impl Into<String>, pid: u64, tid: u64) -> TraceEvent {
    TraceEventBuilder::new(name, EventPhase::Begin, pid, tid).build()
}

/// Convenience function to create a duration end event
pub fn duration_end(name: impl Into<String>, pid: u64, tid: u64) -> TraceEvent {
    TraceEventBuilder::new(name, EventPhase::End, pid, tid).build()
}

/// Convenience function to create a complete event
pub fn complete_event(
    name: impl Into<String>,
    pid: u64,
    tid: u64,
    start_us: u64,
    duration_us: u64,
) -> TraceEvent {
    TraceEventBuilder::new(name, EventPhase::Complete, pid, tid)
        .timestamp(start_us)
        .duration(duration_us)
        .build()
}

/// Convenience function to create an instant event
pub fn instant_event(
    name: impl Into<String>,
    pid: u64,
    tid: u64,
    scope: InstantScope,
) -> TraceEvent {
    TraceEventBuilder::new(name, EventPhase::Instant, pid, tid)
        .scope_instant(scope)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_trace_event() {
        let event = TraceEventBuilder::new("test_function", EventPhase::Complete, 1, 1)
            .category("test")
            .duration(1000)
            .arg("arg1", "value1")
            .build();

        assert_eq!(event.name, "test_function");
        assert_eq!(event.pid, 1);
        assert_eq!(event.tid, 1);
        assert_eq!(event.dur, Some(1000));
        assert_eq!(event.cat, Some("test".to_string()));
    }

    #[test]
    fn test_trace_file_serialization() {
        let mut trace_file = TraceFile::new();

        let event1 = duration_begin("task_start", 1, 1);
        let event2 = duration_end("task_start", 1, 1);

        trace_file.add_event(event1);
        trace_file.add_event(event2);

        let json = trace_file.to_json().unwrap();
        assert!(json.contains("traceEvents"));
        assert!(json.contains("task_start"));
    }

    #[test]
    fn test_event_phases_serialize_correctly() {
        let phases = vec![
            (EventPhase::Begin, "B"),
            (EventPhase::End, "E"),
            (EventPhase::Complete, "X"),
            (EventPhase::Instant, "i"),
            (EventPhase::Counter, "C"),
        ];

        for (phase, expected) in phases {
            let json = serde_json::to_string(&phase).unwrap();
            assert_eq!(json, format!("\"{expected}\""));
        }
    }

    #[test]
    fn test_instant_scope_serialization() {
        let scopes = vec![
            (InstantScope::Global, "g"),
            (InstantScope::Process, "p"),
            (InstantScope::Thread, "t"),
        ];

        for (scope, expected) in scopes {
            let json = serde_json::to_string(&scope).unwrap();
            assert_eq!(json, format!("\"{expected}\""));
        }
    }

    #[test]
    fn test_chrome_trace_format_compatibility() {
        // Test that we can create a format compatible with the example from the spec
        let mut trace_file = TraceFile::new();

        let event = TraceEventBuilder::new("Asub", EventPhase::Begin, 22630, 22630)
            .category("PERF")
            .timestamp(829)
            .build();

        trace_file.add_event(event);

        let event = TraceEventBuilder::new("Asub", EventPhase::End, 22630, 22630)
            .category("PERF")
            .timestamp(833)
            .build();

        trace_file.add_event(event);

        let json = trace_file.to_json().unwrap();

        // Verify it contains the expected structure
        assert!(json.contains("traceEvents"));
        assert!(json.contains("\"name\": \"Asub\""));
        assert!(json.contains("\"cat\": \"PERF\""));
        assert!(json.contains("\"ph\": \"B\""));
        assert!(json.contains("\"ph\": \"E\""));
    }
}
