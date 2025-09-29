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
}
