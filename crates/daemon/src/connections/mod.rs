// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

//! Shared data types for connections management

use moor_var::{Symbol, Var};
use std::{collections::HashMap, time::SystemTime};

#[allow(dead_code, clippy::all)]
mod connections_generated;
mod conversions;
mod fjall_registry;
mod registry;

pub const FIRST_CONNECTION_ID: i32 = -4;

pub use registry::{ConnectionRegistry, ConnectionRegistryFactory, NewConnectionParams};

/// In-memory representation of a connection record
#[derive(Debug, Clone)]
pub struct ConnectionRecord {
    pub client_id: u128,
    pub connected_time: SystemTime,
    pub last_activity: SystemTime,
    pub last_ping: SystemTime,
    pub hostname: String,
    pub local_port: u16,
    pub remote_port: u16,
    pub acceptable_content_types: Vec<Symbol>,
    pub client_attributes: HashMap<Symbol, Var>,
}

/// In-memory representation of a collection of connection records
#[derive(Debug, Clone)]
pub struct ConnectionsRecords {
    pub connections: Vec<ConnectionRecord>,
}
