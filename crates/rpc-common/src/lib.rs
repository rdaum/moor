pub mod pubsub_client;
pub mod rpc_client;

use bincode::{Decode, Encode};
use moor_values::model::{CommandError, NarrativeEvent, WorldStateError};
use moor_values::var::objid::Objid;
use moor_values::var::Var;
use std::time::SystemTime;
use thiserror::Error;

pub const BROADCAST_TOPIC: &[u8; 9] = b"broadcast";

/// Errors at the RPC transport / encoding layer.
#[derive(Debug, thiserror::Error)]
pub enum RpcError {
    #[error("could not send RPC request: {0}")]
    CouldNotSend(String),
    #[error("could not receive RPC response: {0}")]
    CouldNotReceive(String),
    #[error("could not decode RPC response: {0}")]
    CouldNotDecode(String),
}

#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub enum RpcRequest {
    ConnectionEstablish(String),
    RequestSysProp(String, String),
    LoginCommand(Vec<String>),
    Command(String),
    RequestedInput(u128, String),
    OutOfBand(String),
    Eval(String),
    Pong(SystemTime),
    Detach,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Encode, Decode)]
#[repr(u8)]
pub enum ConnectType {
    Connected,
    Reconnected,
    Created,
}

#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub enum RpcResult {
    Success(RpcResponse),
    Failure(RpcRequestError),
}

#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub enum RpcResponse {
    NewConnection(Objid),
    SysPropValue(Option<Var>),
    LoginResult(Option<(ConnectType, Objid)>),
    CommandSubmitted(usize /* task id */),
    InputThanks,
    EvalResult(Var),
    ThanksPong(SystemTime),
    Disconnected,
}

/// Errors at the call/request level.
#[derive(Debug, Eq, PartialEq, Error, Clone, Decode, Encode)]
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
    #[error("Could not create narrative session")]
    CreateSessionFailed,
    #[error("Could not parse or execute command")]
    CommandError(CommandError),
    #[error("Could not start transaction due to database error: {0}")]
    DatabaseError(WorldStateError),
    #[error("Permission denied")]
    PermissionDenied,
    #[error("Internal error: {0}")]
    InternalError(String),
}

/// Events which occur over the pubsub channel, per client.
#[derive(Debug, Eq, PartialEq, Clone, Decode, Encode)]
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
    // TODO: Shutdown, Broadcast messages, etc.
}
