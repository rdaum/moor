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

use eyre::Error;
use moor_var::Obj;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::connections::{ConnectionsRecords, FIRST_CONNECTION_ID};

/// Abstraction for persisting connection data.
/// This trait separates the persistence layer from the in-memory connection management.
pub trait ConnectionRegistryPersistence: Send + Sync {
    /// Load initial state from persistent storage
    fn load_initial_state(&self) -> Result<InitialConnectionRegistryState, Error>;

    /// Persist changes to client->player mappings
    fn persist_client_mappings(&self, changes: &ClientMappingChanges) -> Result<(), Error>;

    /// Persist changes to player->connections mappings  
    fn persist_player_connections(&self, changes: &PlayerConnectionChanges) -> Result<(), Error>;

    /// Get and increment the connection sequence number
    fn next_connection_sequence(&self) -> Result<i32, Error>;
}

/// Initial state loaded from persistence layer
#[derive(Debug, Default)]
pub struct InitialConnectionRegistryState {
    pub client_players: HashMap<Uuid, Obj>,
    pub player_clients: HashMap<Obj, ConnectionsRecords>,
}

/// Batched changes to client->player mappings
#[derive(Debug, Default)]
pub struct ClientMappingChanges {
    pub updates: HashMap<Uuid, Obj>, // client_id -> player_obj
    pub removals: HashSet<Uuid>,     // client_ids to remove
}

/// Batched changes to player->connections mappings
#[derive(Debug, Default)]
pub struct PlayerConnectionChanges {
    pub updates: HashMap<Obj, ConnectionsRecords>, // player_obj -> connections
    pub removals: HashSet<Obj>,                    // player_objs to remove
}

impl ClientMappingChanges {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, client_id: Uuid, player_obj: Obj) {
        self.removals.remove(&client_id);
        self.updates.insert(client_id, player_obj);
    }

    pub fn remove(&mut self, client_id: Uuid) {
        self.updates.remove(&client_id);
        self.removals.insert(client_id);
    }

    pub fn is_empty(&self) -> bool {
        self.updates.is_empty() && self.removals.is_empty()
    }
}

impl PlayerConnectionChanges {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, player_obj: Obj, connections: ConnectionsRecords) {
        self.removals.remove(&player_obj);
        if connections.connections.is_empty() {
            self.removals.insert(player_obj);
        } else {
            self.updates.insert(player_obj, connections);
        }
    }

    pub fn remove(&mut self, player_obj: Obj) {
        self.updates.remove(&player_obj);
        self.removals.insert(player_obj);
    }

    pub fn is_empty(&self) -> bool {
        self.updates.is_empty() && self.removals.is_empty()
    }
}

/// No-op persistence implementation for in-memory-only mode
pub struct NullPersistence {
    sequence: std::sync::atomic::AtomicI32,
}

impl NullPersistence {
    pub fn new() -> Self {
        Self {
            sequence: std::sync::atomic::AtomicI32::new(FIRST_CONNECTION_ID),
        }
    }
}

impl ConnectionRegistryPersistence for NullPersistence {
    fn load_initial_state(&self) -> Result<InitialConnectionRegistryState, Error> {
        Ok(InitialConnectionRegistryState::default())
    }

    fn persist_client_mappings(&self, _changes: &ClientMappingChanges) -> Result<(), Error> {
        // No-op for in-memory only
        Ok(())
    }

    fn persist_player_connections(&self, _changes: &PlayerConnectionChanges) -> Result<(), Error> {
        // No-op for in-memory only
        Ok(())
    }

    fn next_connection_sequence(&self) -> Result<i32, Error> {
        Ok(self
            .sequence
            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst))
    }
}
