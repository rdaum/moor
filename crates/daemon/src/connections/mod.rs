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

//! Shared data types for connections management

use bincode::{Decode, Encode};
use moor_var::{Symbol, Var};
use std::collections::HashMap;
use std::time::SystemTime;

mod fjall_persistence;
mod in_memory;
mod persistence;
mod registry;

pub const FIRST_CONNECTION_ID: i32 = -4;

pub use registry::{ConnectionRegistry, ConnectionRegistryFactory};

#[derive(Debug, Clone, Encode, Decode)]
pub struct ConnectionRecord {
    pub client_id: u128,
    pub connected_time: SystemTime,
    pub last_activity: SystemTime,
    pub last_ping: SystemTime,
    pub hostname: String,
    pub acceptable_content_types: Vec<Symbol>,
    pub client_attributes: HashMap<Symbol, Var>,
}

#[derive(Debug, Clone, Encode, Decode)]
pub struct ConnectionsRecords {
    pub connections: Vec<ConnectionRecord>,
}
