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
use moor_var::{Obj, Symbol};
use std::net::SocketAddr;
use std::time::SystemTime;

/// Types of hosts that can listen.
#[derive(Copy, Debug, Eq, PartialEq, Clone, Decode, Encode)]
pub enum HostType {
    /// A "telnet" host. Line-oriented TCP stream.
    TCP,
    /// A "websocket" (or web generally) connection.
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

/// An RPC message sent from a host itself to the daemon, on behalf of the host. Replying to
/// HostBroadcastEvent, or unprovoked.
#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub enum HostToDaemonMessage {
    /// Register the presence of this host's listeners with the daemon.
    /// Lets the daemon know about the listeners, and then respond to the host with any additional
    /// listeners that the daemon expects the host to start listening on.
    RegisterHost(SystemTime, HostType, Vec<(Obj, SocketAddr)>),
    /// Unregister the presence of this host's listeners with the daemon.
    DetachHost,
    /// Please send the performance counters for the current running system.
    RequestPerformanceCounters,
    /// Respond to a host ping request.
    HostPong(SystemTime, HostType, Vec<(Obj, SocketAddr)>),
}

pub type Counters = Vec<(Symbol, Vec<(Symbol, isize, isize)>)>;

/// An RPC message sent from the daemon to a host in response to a HostToDaemonMessage.
#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub enum DaemonToHostReply {
    /// The daemon is happy with this host and its message. Continue on.
    Ack,
    /// The daemon does not like this host for some reason. The host should die.
    Reject(String),
    /// Here is a dump of the performance counters for the system, as requested.
    /// `[category, [ name, [cnt, total_cumulative_ns]]]`
    PerfCounters(SystemTime, Counters),
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
