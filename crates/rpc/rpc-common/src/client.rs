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

use crate::host::HostType;
use crate::{AuthToken, ClientToken};
use bincode::{Decode, Encode};
use moor_common::model::ObjectRef;
use moor_common::tasks::{NarrativeEvent, SchedulerError, VerbProgramError};
use moor_var::{Obj, Symbol, Var};
use std::net::SocketAddr;
use std::time::SystemTime;
use uuid::Uuid;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Encode, Decode)]
#[repr(u8)]
pub enum ConnectType {
    Connected,
    Reconnected,
    Created,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Encode, Decode)]
#[repr(u8)]
pub enum EntityType {
    Property,
    Verb,
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
    RequestedInput(ClientToken, AuthToken, #[bincode(with_serde)] Uuid, String),
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

/// An RPC message sent from the daemon to a client on a specific host, in response to a
/// HostClientToDaemonMessage.
#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub enum DaemonToClientReply {
    /// A new non-logged-in connection has been established, tied to the given connection object.
    NewConnection(ClientToken, Obj),
    /// Here's the result of the LoginCommand you sent me. An AuthToken, the type of connection
    /// event, and the player I've authenticated you against. If any.
    LoginResult(Option<(AuthToken, ConnectType, Obj)>),
    /// Here's the result of the attachment request you sent me.
    AttachResult(Option<(ClientToken, Obj)>),
    /// Here's a value for the system property you asked for...
    SysPropValue(Option<Var>),
    /// Response to `Command`: I created a task for you with the given ID.
    TaskSubmitted(usize /* task id */),
    /// Response to the reception of `RequestedInput`
    InputThanks,
    /// Response for evaluation.
    EvalResult(Var),
    /// The third part of the PingPong->ClientPong->ThanksPong cycle.
    ThanksPong(SystemTime),
    /// Response to `Verbs`, the list of verbs on the requested object.
    Verbs(Vec<VerbInfo>),
    /// Response to `Properties`, the list of properties on the requested object.
    Properties(Vec<PropInfo>),
    /// Response to `Program` -- successful or failed compilation.
    ProgramResponse(VerbProgramResponse),
    /// Property value response to `Retrieve`
    PropertyValue(PropInfo, Var),
    /// Verb value response to `Retrieve`
    VerbValue(VerbInfo, Vec<String>),
    /// Response to `Resolve`
    ResolveResult(Var),
    /// This Client has been disconnected and is not expected to be heard from again.
    Disconnected,
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

/// Events which occur over the pubsub channel, but destined for specific clients.
#[derive(Debug, PartialEq, Clone, Decode, Encode)]
pub enum ClientEvent {
    /// An event has occurred in the narrative that the connections for the given object are
    /// expected to see.
    Narrative(Obj, NarrativeEvent),
    /// The server wants the client to prompt the user for input, and the task this session is
    /// attached to will suspend until the client sends an RPC with a `RequestedInput` message and
    /// the attached request id.
    RequestInput(#[bincode(with_serde)] Uuid),
    /// The system wants to send a message to the given object on its current active connections.
    SystemMessage(Obj, String),
    /// The system wants to disconnect the given object from all its current active connections.
    Disconnect(),
    /// Task errors that should be sent to the client.
    TaskError(usize, SchedulerError),
    /// Task return common on success that the client can get.
    TaskSuccess(usize, Var),
}
