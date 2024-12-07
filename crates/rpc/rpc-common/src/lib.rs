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
use moor_values::model::ObjectRef;
use moor_values::tasks::{NarrativeEvent, SchedulerError, VerbProgramError};
use moor_values::{Obj, Symbol, Var};
use pem::PemError;
use rusty_paseto::prelude::Key;
use std::net::SocketAddr;
use std::path::Path;
use std::time::SystemTime;
use thiserror::Error;

pub mod client_args;

/// A ZMQ topic for broadcasting to all clients of all hosts.
pub const CLIENT_BROADCAST_TOPIC: &[u8; 9] = b"broadcast";

/// A ZMQ topic for broadcasting to just the hosts.
pub const HOST_BROADCAST_TOPIC: &[u8; 5] = b"hosts";

pub const MOOR_HOST_TOKEN_FOOTER: &str = "key-id:moor_host";
pub const MOOR_SESSION_TOKEN_FOOTER: &str = "key-id:moor_client";
pub const MOOR_AUTH_TOKEN_FOOTER: &str = "key-id:moor_player";

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

/// PASETO public token representing the host's identity.
#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode, Hash)]
pub struct HostToken(pub String);

/// PASETO public token for a connection, used for the validation of RPC requests after the initial
/// connection is established.
#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode, Hash)]
pub struct ClientToken(pub String);

/// PASTEO public token for an authenticated player, encoding the player's identity.
#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode, Hash)]
pub struct AuthToken(pub String);

#[derive(Debug, Eq, PartialEq, Clone, Decode, Encode)]
pub enum MessageType {
    HostToDaemon(HostToken),
    /// A message from a host to the daemon on behalf of a client (client id is included)
    HostClientToDaemon(Vec<u8>),
}

#[derive(Copy, Debug, Eq, PartialEq, Clone, Decode, Encode)]
pub enum HostType {
    TCP,
    WebSocket,
}

impl HostType {
    pub fn id_str(&self) -> &str {
        match self {
            HostType::TCP => "tcp",
            HostType::WebSocket => "websocket",
        }
    }

    pub fn parse_id_str(id_str: &str) -> Option<Self> {
        match id_str {
            "tcp" => Some(HostType::TCP),
            "websocket" => Some(HostType::WebSocket),
            _ => None,
        }
    }
}
/// An RPC message sent from a host itself to the daemon, on behalf of the host
#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub enum HostToDaemonMessage {
    /// Register the presence of this host's listeners with the daemon.
    /// Lets the daemon know about the listeners, and then respond to the host with any additional
    /// listeners that the daemon expects the host to start listening on.
    RegisterHost(SystemTime, HostType, Vec<(Obj, SocketAddr)>),
    /// Unregister the presence of this host's listeners with the daemon.
    DetachHost(),
    /// Respond to a host ping request.
    HostPong(SystemTime, HostType, Vec<(Obj, SocketAddr)>),
}

/// An RPC message sent from a host to the daemon on behalf of a client.
#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub enum HostClientToDaemonMessage {
    /// Establish a new connection, requesting a client token and a connection object
    ConnectionEstablish(String),
    /// Anonymously request a sysprop (e.g. $login.welcome_message)
    RequestSysProp(ClientToken, ObjectRef, Symbol),
    /// Login using the words (e.g. "create player bob" or "connect player bob") and return an
    /// auth token and the object id of the player. None if the login failed.
    LoginCommand(ClientToken, Obj, Vec<String>, bool /* attach? */),
    /// Attach to a previously-authenticated user, returning the object id of the player,
    /// and a client token -- or None if the auth token is not valid.
    /// If a ConnectType is specified, the user_connected verb will be called.
    Attach(AuthToken, Option<ConnectType>, Obj, String),
    /// Send a command to be executed.
    Command(ClientToken, AuthToken, Obj, String),
    /// Return the (visible) verbs on the given object.
    Verbs(ClientToken, AuthToken, ObjectRef),
    /// Invoke the given verb on the given object.
    InvokeVerb(ClientToken, AuthToken, ObjectRef, Symbol, Vec<Var>),
    /// Return the (visible) properties on the given object.
    Properties(ClientToken, AuthToken, ObjectRef),
    /// Retrieve the given verb code or property.
    Retrieve(ClientToken, AuthToken, ObjectRef, EntityType, Symbol),
    /// Attempt to program the object with the given verb code
    Program(ClientToken, AuthToken, ObjectRef, Symbol, Vec<String>),
    /// Respond to a request for input.
    RequestedInput(ClientToken, AuthToken, u128, String),
    /// Send an "out of band" command to be executed.
    OutOfBand(ClientToken, AuthToken, Obj, String),
    /// Evaluate a MOO expression.
    Eval(ClientToken, AuthToken, String),
    /// Resolve an object reference into a Var
    Resolve(ClientToken, AuthToken, ObjectRef),
    /// Respond to a client ping request.
    ClientPong(ClientToken, SystemTime, Obj, HostType, SocketAddr),
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
pub enum ReplyResult {
    HostSuccess(DaemonToHostReply),
    ClientSuccess(DaemonToClientReply),
    Failure(RpcMessageError),
}

#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub enum VerbProgramResponse {
    Success(Obj, String),
    Failure(VerbProgramError),
}

#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub struct VerbInfo {
    pub location: Obj,
    pub owner: Obj,
    pub names: Vec<Symbol>,
    pub r: bool,
    pub w: bool,
    pub x: bool,
    pub d: bool,
    pub arg_spec: Vec<Symbol>,
}

#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub struct PropInfo {
    pub definer: Obj,
    pub location: Obj,
    pub name: Symbol,
    pub owner: Obj,
    pub r: bool,
    pub w: bool,
    pub chown: bool,
}

/// An RPC message sent from the daemon to a host in response to a HostToDaemonMessage.
#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub enum DaemonToHostReply {
    /// The daemon is happy with this host and its messages.
    Ack,
    /// The daemon does not like this host for some reason. The host should die.
    Reject(String),
}

/// An RPC message sent from the daemon to a client on a specific host, in response to a
/// HostClientToDaemonMessage.
#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub enum DaemonToClientReply {
    NewConnection(ClientToken, Obj),
    SysPropValue(Option<Var>),
    LoginResult(Option<(AuthToken, ConnectType, Obj)>),
    AttachResult(Option<(ClientToken, Obj)>),
    TaskSubmitted(usize /* task id */),
    InputThanks,
    EvalResult(Var),
    ThanksPong(SystemTime),
    Disconnected,
    Verbs(Vec<VerbInfo>),
    Properties(Vec<PropInfo>),
    ProgramResponse(VerbProgramResponse),
    PropertyValue(PropInfo, Var),
    VerbValue(VerbInfo, Vec<String>),
    ResolveResult(Var),
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

/// Events which occur over the pubsub channel, but destined for specific clients.
#[derive(Debug, PartialEq, Clone, Decode, Encode)]
pub enum ClientEvent {
    /// An event has occurred in the narrative that the connections for the given object are
    /// expected to see.
    Narrative(Obj, NarrativeEvent),
    /// The server wants the client to prompt the user for input, and the task this session is
    /// attached to will suspend until the client sends an RPC with a `RequestedInput` message and
    /// the attached request id.
    RequestInput(u128),
    /// The system wants to send a message to the given object on its current active connections.
    SystemMessage(Obj, String),
    /// The system wants to disconnect the given object from all its current active connections.
    Disconnect(),
    /// Task errors that should be sent to the client.
    TaskError(usize, SchedulerError),
    /// Task return common on success that the client can get.
    TaskSuccess(usize, Var),
}

/// Events which occur over the pubsub endpoint, but are for all the hosts.
#[derive(Debug, Eq, PartialEq, Clone, Decode, Encode)]
pub enum HostBroadcastEvent {
    /// The system is requesting that all hosts are of the given HostType begin listening on
    /// the given port.
    /// Triggered from the `listen` builtin.
    Listen {
        handler_object: Obj,
        host_type: HostType,
        port: u16,
        print_messages: bool,
    },
    /// The system is requesting that all hosts of the given HostType stop listening on the given port.
    Unlisten { host_type: HostType, port: u16 },
    /// The system wants to know which hosts are still alive. They should respond by sending
    /// a `HostPong` message RPC to the server.
    /// If a host does not respond, the server will assume it is dead and remove its listeners
    /// from the list of active listeners.
    PingPong(SystemTime),
}

/// Events which occur over the pubsub endpoint, but are for all clients on all hosts.
#[derive(Debug, Eq, PartialEq, Clone, Decode, Encode)]
pub enum ClientsBroadcastEvent {
    /// The system wants to know which clients are still alive. The host should respond by sending
    /// a `Pong` message RPC to the server (and it will then respond with ThanksPong) for each
    /// active client it still has, along with the host type and IP address of the client.
    /// This is used to keep track of which clients are still connected to the server, and
    /// also to fill in output from `listeners`.
    ///
    /// (The time parameter is the server's current time. The client will respond with its own
    /// current time. This could be used in the future to synchronize event times, but isn't currently
    /// used.)
    PingPong(SystemTime),
}

#[derive(Error, Debug)]
pub enum KeyError {
    #[error("Could not read key from file: {0}")]
    ParseError(PemError),
    #[error("Could not read key from file: {0}")]
    ReadError(std::io::Error),
}

/// Load a keypair from the given public and private key (PEM) files.
pub fn load_keypair(public_key: &Path, private_key: &Path) -> Result<Key<64>, KeyError> {
    let (Some(pubkey_pem), Some(privkey_pem)) = (
        std::fs::read(public_key).ok(),
        std::fs::read(private_key).ok(),
    ) else {
        return Err(KeyError::ReadError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not read key from file",
        )));
    };

    let privkey_pem = pem::parse(privkey_pem).map_err(KeyError::ParseError)?;
    let pubkey_pem = pem::parse(pubkey_pem).map_err(KeyError::ParseError)?;

    let mut key_bytes = privkey_pem.contents().to_vec();
    key_bytes.extend_from_slice(pubkey_pem.contents());

    Ok(Key::from(&key_bytes[0..64]))
}
