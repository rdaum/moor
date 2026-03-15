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

//! V8-based JavaScript verb execution engine.
//!
//! Provides a worker pool that runs JS verbs on V8 isolates and trampolines
//! back to the moor kernel for property access and verb calls.

mod marshal;
mod worker;

use std::sync::Arc;

use moor_var::{Obj, Symbol, Var};

/// Error from JavaScript execution.
#[derive(Debug, Clone)]
pub struct JsError {
    pub message: String,
}

impl std::fmt::Display for JsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// A request from the JS worker back to the kernel via the trampoline channel.
/// Each request that expects a response carries a `resolver_id` so the worker
/// can match the response to the right pending promise.
#[derive(Debug)]
pub enum TrampolineRequest {
    CallVerb {
        resolver_id: usize,
        this: Obj,
        verb: Symbol,
        args: Vec<Var>,
    },
    GetProp {
        resolver_id: usize,
        obj: Obj,
        prop: Symbol,
    },
    SetProp {
        resolver_id: usize,
        obj: Obj,
        prop: Symbol,
        value: Var,
    },
    /// JS execution of this verb is complete.
    Complete(Result<Var, JsError>),
}

/// A response from the kernel back to the JS worker.
#[derive(Debug)]
pub enum TrampolineResponse {
    Value(Var),
    Error(JsError),
}

/// Messages sent TO the worker. Combines dock requests and trampoline responses
/// on a single channel so the worker can accept new work while waiting for
/// responses (required for reentrant JS→Moo→JS).
pub enum WorkerInput {
    Dock(DockRequest),
    Response {
        resolver_id: usize,
        response: TrampolineResponse,
    },
}

/// A request to dock a JS verb execution onto a worker.
pub struct DockRequest {
    pub source: Arc<str>,
    pub this: Obj,
    pub player: Obj,
    pub args: Vec<Var>,
    /// Worker sends trampoline requests here (per-verb channel).
    pub trampoline_tx: flume::Sender<TrampolineRequest>,
}

/// Pool of V8 worker threads. Currently single-threaded for the spike.
pub struct JsWorkerPool {
    worker_tx: flume::Sender<WorkerInput>,
    _worker_handle: std::thread::JoinHandle<()>,
}

impl JsWorkerPool {
    pub fn new() -> Self {
        let (worker_tx, worker_rx) = flume::unbounded::<WorkerInput>();
        let handle = std::thread::Builder::new()
            .name("moor-js-worker".into())
            .spawn(move || {
                worker::worker_main(worker_rx);
            })
            .expect("Failed to spawn JS worker thread");

        Self {
            worker_tx,
            _worker_handle: handle,
        }
    }

    /// Submit a JS verb for execution.
    /// Returns (trampoline_rx, worker_tx) — the task thread reads trampoline
    /// requests from trampoline_rx and sends responses (and nested dock
    /// requests) on worker_tx.
    pub fn submit(
        &self,
        source: Arc<str>,
        this: Obj,
        player: Obj,
        args: Vec<Var>,
    ) -> (
        flume::Receiver<TrampolineRequest>,
        flume::Sender<WorkerInput>,
    ) {
        let (trampoline_tx, trampoline_rx) = flume::unbounded();

        let dock = DockRequest {
            source,
            this,
            player,
            args,
            trampoline_tx,
        };

        self.worker_tx
            .send(WorkerInput::Dock(dock))
            .expect("JS worker pool has shut down");

        (trampoline_rx, self.worker_tx.clone())
    }
}

impl Default for JsWorkerPool {
    fn default() -> Self {
        Self::new()
    }
}
