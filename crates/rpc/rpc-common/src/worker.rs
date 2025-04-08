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

use crate::tokens::WorkerToken;
use bincode::{Decode, Encode};
use moor_common::tasks::WorkerError;
use moor_var::{Obj, Symbol, Var};
use uuid::Uuid;

#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub enum DaemonToWorkerMessage {
    /// Ping a worker to ensure it is still alive.
    PingWorker(WorkerToken, #[bincode(with_serde)] Uuid),
    /// Initiate a one-shot request from the daemon to a specific worker to execute something in its worker
    /// specific way.
    /// The interpretation of the request is left to the worker.
    /// Only the worker with this specific worker id should respond to this request.
    WorkerRequest {
        #[bincode(with_serde)]
        worker_id: Uuid,
        token: WorkerToken,
        #[bincode(with_serde)]
        id: Uuid,
        perms: Obj,
        request: Vec<Var>,
    },
    // TODO: sessions/connections, which are longer running multiple-request -- potentially
    //  transaction-attached, potentially bi-directional -- for things like e.g. outbound network connections
    /// Ask the worker to shut down.
    PleaseDie(WorkerToken, #[bincode(with_serde)] Uuid),
}

#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub enum WorkerToDaemonMessage {
    /// Register this worker with the daemon as available.
    AttachWorker {
        token: WorkerToken,
        worker_type: Symbol,
    },
    /// Respond to a ping from the daemon.
    Pong(WorkerToken),
    /// Detach this worker from the daemon.
    DetachWorker(WorkerToken),
    /// Return the results of a daemon initiated request.
    RequestResult(WorkerToken, #[bincode(with_serde)] Uuid, Vec<Var>),
    /// Return an error from a daemon initiated request.
    RequestError(WorkerToken, #[bincode(with_serde)] Uuid, WorkerError),
}

#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub enum DaemonToWorkerReply {
    Ack,
    Rejected,
    /// Let the worker know that it is attached to the daemon.
    Attached(WorkerToken, #[bincode(with_serde)] Uuid),
}
