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

use std::time::{Duration, SystemTime};

use uuid::Uuid;

use moor_kernel::tasks::sessions::SessionError;
use moor_var::Obj;
use rpc_common::RpcMessageError;

pub const CONNECTION_TIMEOUT_DURATION: Duration = Duration::from_secs(30);

pub trait ConnectionsDB {
    /// Update the connection record for the given connection object to point to the given player.
    /// This is used when a player logs in.
    fn update_client_connection(
        &self,
        from_connection: Obj,
        to_player: Obj,
    ) -> Result<(), eyre::Error>;

    /// Create a new connection object for the given client.
    fn new_connection(
        &self,
        client_id: Uuid,
        hostname: String,
        player: Option<Obj>,
    ) -> Result<Obj, RpcMessageError>;

    /// Record activity for the given client.
    fn record_client_activity(&self, client_id: Uuid, connobj: Obj) -> Result<(), eyre::Error>;

    /// Update the last ping time for a client / connection.
    fn notify_is_alive(&self, client_id: Uuid, connection: Obj) -> Result<(), eyre::Error>;

    /// Prune any connections that have not been active for longer than the required duration.
    fn ping_check(&self);

    fn last_activity_for(&self, connection: Obj) -> Result<SystemTime, SessionError>;

    fn connection_name_for(&self, player: Obj) -> Result<String, SessionError>;

    fn connected_seconds_for(&self, player: Obj) -> Result<f64, SessionError>;

    fn client_ids_for(&self, player: Obj) -> Result<Vec<Uuid>, SessionError>;

    /// Return all connection objects (player or not)
    fn connections(&self) -> Vec<Obj>;

    /// Retrieve the connection object for the given client.
    fn connection_object_for_client(&self, client_id: Uuid) -> Option<Obj>;

    /// Remove the given client from the connection database.
    fn remove_client_connection(&self, client_id: Uuid) -> Result<(), eyre::Error>;
}
