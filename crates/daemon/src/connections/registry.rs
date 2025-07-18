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

use crate::connections::fjall_persistence::FjallPersistence;
use crate::connections::in_memory::ConnectionRegistryMemory;
use crate::connections::persistence::NullPersistence;
use eyre::Report as Error;
use moor_common::tasks::SessionError;
use moor_var::{Obj, Symbol};
use rpc_common::RpcMessageError;
use std::path::Path;

pub const CONNECTION_TIMEOUT_DURATION: Duration = Duration::from_secs(30);

pub trait ConnectionRegistry {
    /// Associate the given player object with the connection object.
    /// This is used when a player logs in.
    /// The connection object remains associated with the client.
    fn associate_player_object(
        &self,
        connection_obj: Obj,
        player_obj: Obj,
    ) -> Result<(), eyre::Error>;

    /// Create a new connection object for the given client.
    fn new_connection(
        &self,
        client_id: Uuid,
        hostname: String,
        player: Option<Obj>,
        acceptable_content_types: Option<Vec<Symbol>>,
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

    /// Retrieve the player object for the given client (if logged in).
    fn player_object_for_client(&self, client_id: Uuid) -> Option<Obj>;

    /// Remove the given client from the connection database.
    fn remove_client_connection(&self, client_id: Uuid) -> Result<(), eyre::Error>;

    /// Get the acceptable content types for a connection.
    fn acceptable_content_types_for(&self, connection: Obj) -> Result<Vec<Symbol>, SessionError>;
}

pub enum ConnectionRegistryConfig {
    /// In-memory only, no persistence
    InMemoryOnly,
    /// In-memory with Fjall persistence
    WithFjallPersistence { path: Option<Box<Path>> },
}

/// Factory for creating connections database instances
pub struct ConnectionRegistryFactory;

impl ConnectionRegistryFactory {
    /// Create a connections database based on configuration
    pub fn create(
        config: ConnectionRegistryConfig,
    ) -> Result<Box<dyn ConnectionRegistry + Send + Sync>, Error> {
        match config {
            ConnectionRegistryConfig::InMemoryOnly => {
                let persistence = NullPersistence::new();
                let db = ConnectionRegistryMemory::new(persistence)?;
                Ok(Box::new(db))
            }

            ConnectionRegistryConfig::WithFjallPersistence { path } => {
                let persistence = FjallPersistence::open(path.as_deref())?;
                let db = ConnectionRegistryMemory::new(persistence)?;
                Ok(Box::new(db))
            }
        }
    }

    /// Create in-memory only database (useful for testing)
    pub fn in_memory_only() -> Result<Box<dyn ConnectionRegistry + Send + Sync>, Error> {
        Self::create(ConnectionRegistryConfig::InMemoryOnly)
    }

    /// Create in-memory database with Fjall persistence
    pub fn with_fjall_persistence<P: AsRef<Path>>(
        path: Option<P>,
    ) -> Result<Box<dyn ConnectionRegistry + Send + Sync>, Error> {
        let path = path.map(|p| p.as_ref().to_path_buf().into_boxed_path());
        Self::create(ConnectionRegistryConfig::WithFjallPersistence { path })
    }
}

#[cfg(test)]
mod tests {
    use crate::connections::registry::ConnectionRegistryFactory;
    use uuid::Uuid;

    #[test]
    fn test_in_memory_only_factory() {
        let db = ConnectionRegistryFactory::in_memory_only().unwrap();

        let client_id = Uuid::new_v4();
        let connection_obj = db
            .new_connection(client_id, "test.host".to_string(), None, None)
            .unwrap();

        assert_eq!(
            db.connection_object_for_client(client_id),
            Some(connection_obj)
        );
        assert_eq!(db.player_object_for_client(client_id), None);
        assert_eq!(db.connections(), vec![connection_obj]);
    }

    #[test]
    fn test_fjall_persistence_factory() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db = ConnectionRegistryFactory::with_fjall_persistence(Some(temp_dir.path())).unwrap();

        let client_id = Uuid::new_v4();
        let connection_obj = db
            .new_connection(client_id, "persistent.host".to_string(), None, None)
            .unwrap();

        assert_eq!(
            db.connection_object_for_client(client_id),
            Some(connection_obj)
        );
        assert_eq!(db.player_object_for_client(client_id), None);
        assert_eq!(db.connections(), vec![connection_obj]);
    }

    #[test]
    fn test_configuration_compatibility() {
        // All implementations should behave identically for basic operations
        let configs = vec![
            ConnectionRegistryFactory::in_memory_only().unwrap(),
            ConnectionRegistryFactory::with_fjall_persistence(None::<&str>).unwrap(),
        ];

        for db in configs {
            let client_id = Uuid::new_v4();
            let connection_obj = db
                .new_connection(client_id, "compat.test".to_string(), None, None)
                .unwrap();

            // Basic operations should work identically
            assert_eq!(
                db.connection_object_for_client(client_id),
                Some(connection_obj)
            );
            assert_eq!(db.player_object_for_client(client_id), None);
            assert_eq!(db.client_ids_for(connection_obj).unwrap(), vec![client_id]);
            assert_eq!(db.connections(), vec![connection_obj]);

            // Activity tracking
            db.record_client_activity(client_id, connection_obj)
                .unwrap();
            db.notify_is_alive(client_id, connection_obj).unwrap();
            assert!(db.last_activity_for(connection_obj).is_ok());

            // Cleanup
            db.remove_client_connection(client_id).unwrap();
            assert_eq!(db.connections(), vec![]);
        }
    }

    #[test]
    fn test_connection_player_association() {
        let db = ConnectionRegistryFactory::in_memory_only().unwrap();

        let client_id = Uuid::new_v4();
        let connection_obj = db
            .new_connection(client_id, "test.host".to_string(), None, None)
            .unwrap();

        // Initially no player associated
        assert_eq!(
            db.connection_object_for_client(client_id),
            Some(connection_obj)
        );
        assert_eq!(db.player_object_for_client(client_id), None);

        // Associate a player object
        use moor_var::Obj;
        let player_obj = Obj::mk_id(100);
        db.associate_player_object(connection_obj, player_obj)
            .unwrap();

        // Now should have both connection and player
        assert_eq!(
            db.connection_object_for_client(client_id),
            Some(connection_obj)
        );
        assert_eq!(db.player_object_for_client(client_id), Some(player_obj));

        // Both connection and player should be in connections list
        let mut connections = db.connections();
        connections.sort();
        let mut expected = vec![connection_obj, player_obj];
        expected.sort();
        assert_eq!(connections, expected);

        // Client IDs should work for both connection and player objects
        assert_eq!(db.client_ids_for(connection_obj).unwrap(), vec![client_id]);
        assert_eq!(db.client_ids_for(player_obj).unwrap(), vec![client_id]);
    }
}
