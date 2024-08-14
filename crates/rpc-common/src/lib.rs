// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use bincode::{Decode, Encode};
use moor_values::tasks::{NarrativeEvent, SchedulerError, VerbProgramError};
use moor_values::{Objid, Var, Symbol};
use std::time::SystemTime;
use thiserror::Error;

pub const BROADCAST_TOPIC: &[u8; 9] = b"broadcast";

pub const MOOR_SESSION_TOKEN_FOOTER: &str = "key-id:moor_rpc";
pub const MOOR_AUTH_TOKEN_FOOTER: &str = "key-id:moor_player";

/// Errors at the RPC transport / encoding layer.
#[derive(Debug, Error)]
pub enum RpcError {
    #[error("could not send RPC request: {0}")]
    CouldNotSend(String),
    #[error("could not receive RPC response: {0}")]
    CouldNotReceive(String),
    #[error("could not decode RPC response: {0}")]
    CouldNotDecode(String),
}

/// PASETO public token for a connection, used for the validation of RPC requests after the initial
/// connection is established.
#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub struct ClientToken(pub String);

/// PASTEO public token for an authenticated player, encoding the player's identity.
#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub struct AuthToken(pub String);

#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub enum RpcRequest {
    /// Establish a new connection, requesting a client token and a connection object
    ConnectionEstablish(String),
    /// Anonymously request a sysprop (e.g. $login.welcome_message)
    RequestSysProp(ClientToken, Symbol, Symbol),
    /// Login using the words (e.g. "create player bob" or "connect player bob") and return an
    /// auth token and the object id of the player. None if the login failed.
    LoginCommand(ClientToken, Vec<String>, bool /* attach? */),
    /// Attach to a previously-authenticated user, returning the object id of the player,
    /// and a client token -- or None if the auth token is not valid.
    /// If a ConnectType is specified, the user_connected verb will be called.
    Attach(AuthToken, Option<ConnectType>, String),
    /// Send a command to be executed.
    Command(ClientToken, AuthToken, String),
    /// Return the (visible) verbs on the given object.
    Verbs(ClientToken, AuthToken, Objid),
    /// Return the (visible) properties on the given object.
    Properties(ClientToken, AuthToken, Objid),
    /// Retrieve the given verb code or property.
    Retrieve(ClientToken, AuthToken, Objid, EntityType, Symbol),
    /// Attempt to program the object with the given verb code
    Program(ClientToken, AuthToken, String, String, Vec<String>),
    /// Respond to a request for input.
    RequestedInput(ClientToken, AuthToken, u128, String),
    /// Send an "out of band" command to be executed.
    OutOfBand(ClientToken, AuthToken, String),
    /// Evaluate a MOO expression.
    Eval(ClientToken, AuthToken, String),
    /// Respond to a ping request.
    Pong(ClientToken, SystemTime),
    /// We're done with this connection, buh-bye.
    Detach(ClientToken),
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Encode, Decode)]
#[repr(u8)]
pub enum EntityType {
    Property,
    Verb,
}
#[derive(Debug, Copy, Clone, Eq, PartialEq, Encode, Decode)]
#[repr(u8)]
pub enum ConnectType {
    Connected,
    Reconnected,
    Created,
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub enum RpcResult {
    Success(RpcResponse),
    Failure(RpcRequestError),
}

#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub enum VerbProgramResponse {
    Success(Objid, String),
    Failure(VerbProgramError),
}

#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub struct VerbInfo {
    pub location: Objid,
    pub owner: Objid,
    pub names: Vec<Symbol>,
    pub r: bool,
    pub w: bool,
    pub x: bool,
    pub d: bool,
    pub arg_spec: Vec<Symbol>,
}

#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub struct PropInfo {
    pub definer: Objid,
    pub location: Objid,
    pub name: Symbol,
    pub owner: Objid,
    pub r: bool,
    pub w: bool,
    pub chown: bool,
}

#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub enum RpcResponse {
    NewConnection(ClientToken, Objid),
    SysPropValue(Option<Var>),
    LoginResult(Option<(AuthToken, ConnectType, Objid)>),
    AttachResult(Option<(ClientToken, Objid)>),
    CommandSubmitted(usize /* task id */),
    InputThanks,
    EvalResult(Var),
    ThanksPong(SystemTime),
    Disconnected,
    Verbs(Vec<VerbInfo>),
    Properties(Vec<PropInfo>),
    ProgramResponse(VerbProgramResponse),
    PropertyValue(PropInfo, Var),
    VerbValue(VerbInfo, Vec<String>),
}

/// Errors at the call/request level.
#[derive(Debug, PartialEq, Error, Clone, Decode, Encode)]
pub enum RpcRequestError {
    #[error("Already connected")]
    AlreadyConnected,
    #[error("Invalid request")]
    InvalidRequest,
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

/// Events which occur over the pubsub channel, per client.
#[derive(Debug, PartialEq, Clone, Decode, Encode)]
pub enum ConnectionEvent {
    /// An event has occurred in the narrative that the connections for the given object are
    /// expected to see.
    Narrative(Objid, NarrativeEvent),
    /// The server wants the client to prompt the user for input, and the task this session is
    /// attached to will suspend until the client sends an RPC with a `RequestedInput` message and
    /// the attached request id.
    RequestInput(u128),
    /// The system wants to send a message to the given object on its current active connections.
    SystemMessage(Objid, String),
    /// The system wants to disconnect the given object from all its current active connections.
    Disconnect(),
    /// Task errors that should be sent to the client.
    TaskError(SchedulerError),
    /// Task return values on success that the client can get.
    TaskSuccess(Var),
}

/// Events which occur over the pubsub channel, but are for all hosts.
#[derive(Debug, Eq, PartialEq, Clone, Decode, Encode)]
pub enum BroadcastEvent {
    /// The system wants to know which clients are still alive. The host should respond by sending
    /// a `Pong` message RPC to the server (and it will then respond with ThanksPong) for each
    /// active client it still has.
    /// (The time parameter is the server's current time. The client will respond with its own
    /// current time. This could be used in the future to synchronize event times, but isn't currently
    /// used.)
    PingPong(SystemTime),
    // TODO: Shutdown, Broadcast BroadcastEvent messages in RPC layer
}
