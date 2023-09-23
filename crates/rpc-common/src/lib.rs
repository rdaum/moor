use bincode::{Decode, Encode};
use moor_values::model::{NarrativeEvent, WorldStateError};
use moor_values::var::objid::Objid;
use moor_values::var::Var;
use thiserror::Error;

#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub enum RpcRequest {
    ConnectionEstablish(String),
    RequestSysProp(String, String),
    LoginCommand(Vec<String>),
    Command(String),
    OutOfBand(String),
    Eval(String),
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
    Failure(RpcError),
}

#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub enum RpcResponse {
    NewConnection(Objid),
    SysPropValue(Option<Var>),
    LoginResult(Option<(ConnectType, Objid)>),
    CommandComplete,
    EvalResult(Var),
    Disconnected,
}

#[derive(Debug, Eq, PartialEq, Error, Clone, Decode, Encode)]
pub enum RpcError {
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
    #[error("Could not parse command")]
    CouldNotParseCommand,
    #[error("Could not find match for command '{0}'")]
    NoCommandMatch(String),
    #[error("Could not start transaction due to database error: {0}")]
    DatabaseError(WorldStateError),
    #[error("Permission denied")]
    PermissionDenied,
    #[error("Internal error: {0}")]
    InternalError(String),
}

/// Events which occur over the 'connection' pubsub channel.
#[derive(Debug, Eq, PartialEq, Clone, Decode, Encode)]
pub enum ConnectionEvent {
    Narrative(Objid, NarrativeEvent),
    SystemMessage(Objid, String),
    Disconnect(),
}
