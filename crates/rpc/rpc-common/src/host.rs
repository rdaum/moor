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

use moor_common::schema::rpc;

use crate::RpcMessageError;

/// Types of hosts that can listen.
#[derive(Copy, Debug, Eq, PartialEq, Clone)]
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

    /// Convert from FlatBuffer HostType enum
    pub fn from_flatbuffer(fb_type: rpc::HostType) -> Self {
        match fb_type {
            rpc::HostType::Tcp => HostType::TCP,
            rpc::HostType::WebSocket => HostType::WebSocket,
        }
    }

    /// Convert to FlatBuffer HostType enum
    pub fn to_flatbuffer(&self) -> rpc::HostType {
        match self {
            HostType::TCP => rpc::HostType::Tcp,
            HostType::WebSocket => rpc::HostType::WebSocket,
        }
    }
}

/// Extract and convert a HostType from a FlatBuffer message
pub fn extract_host_type<T>(
    msg: &T,
    field_name: &str,
    get_field: impl FnOnce(&T) -> Result<rpc::HostType, planus::Error>,
) -> Result<HostType, RpcMessageError> {
    get_field(msg)
        .map(HostType::from_flatbuffer)
        .map_err(|e| RpcMessageError::InvalidRequest(format!("Missing {field_name}: {e}")))
}
