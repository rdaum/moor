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

use bincode::{Decode, Encode};
use moor_common::tasks::SchedulerError;
use thiserror::Error;

pub use client::{
    ClientEvent, ClientsBroadcastEvent, ConnectType, DaemonToClientReply, EntityType,
    HistoricalNarrativeEvent, HistoryRecall, HistoryResponse, HostClientToDaemonMessage, PropInfo,
    VerbInfo, VerbProgramResponse,
};
pub use host::{DaemonToHostReply, HostBroadcastEvent, HostToDaemonMessage, HostType};
pub use worker::{DaemonToWorkerMessage, DaemonToWorkerReply, WorkerToDaemonMessage};

pub use tokens::{
    AuthToken, ClientToken, HostToken, KeyError, MOOR_AUTH_TOKEN_FOOTER, MOOR_HOST_TOKEN_FOOTER,
    MOOR_SESSION_TOKEN_FOOTER, MOOR_WORKER_TOKEN_FOOTER, WorkerToken, load_keypair,
    make_host_token, parse_keypair,
};
mod client;
pub mod client_args;
mod host;
mod tokens;
mod worker;

/// A ZMQ topic for broadcasting to all clients of all hosts.
pub const CLIENT_BROADCAST_TOPIC: &[u8; 9] = b"broadcast";

/// A ZMQ topic for broadcasting to just the hosts.
pub const HOST_BROADCAST_TOPIC: &[u8; 5] = b"hosts";

/// A ZMQ topic for broadcasting to just the workers.
pub const WORKER_BROADCAST_TOPIC: &[u8; 7] = b"workers";

/// Errors at the RPC transport / encoding layer.
#[derive(Debug, Error)]
pub enum RpcError {
    #[error("could not initiate session: {0}")]
    CouldNotInitiateSession(String),
    #[error("could not authenticate: {0}")]
    AuthenticationError(String),
    #[error("could not send RPC request: {0}")]
    CouldNotSend(String),
    #[error("could not receive RPC response: {0}")]
    CouldNotReceive(String),
    #[error("could not decode RPC response: {0}")]
    CouldNotDecode(String),
    #[error("unexpected reply: {0}")]
    UnexpectedReply(String),
}

#[derive(Debug, Eq, PartialEq, Clone, Decode, Encode)]
pub enum MessageType {
    HostToDaemon(HostToken),
    /// A message from a host to the daemon on behalf of a client (client id is included)
    HostClientToDaemon(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub enum ReplyResult {
    HostSuccess(DaemonToHostReply),
    ClientSuccess(DaemonToClientReply),
    Failure(RpcMessageError),
}

/// Errors at the message passing level.
#[derive(Debug, PartialEq, Error, Clone, Decode, Encode)]
pub enum RpcMessageError {
    #[error("Already connected")]
    AlreadyConnected,
    #[error("Invalid request")]
    InvalidRequest(String),
    #[error("No connection for client")]
    NoConnection,
    #[error("Could not retrieve system property")]
    ErrorCouldNotRetrieveSysProp(String),
    #[error("Could not login")]
    LoginTaskFailed,
    #[error("Could not create session")]
    CreateSessionFailed,
    #[error("Permission denied")]
    PermissionDenied,
    #[error("Error scheduling task")]
    TaskError(SchedulerError),
    #[error("Error retreiving entity: {0}")]
    EntityRetrievalError(String),
    #[error("Internal error: {0}")]
    InternalError(String),
}
