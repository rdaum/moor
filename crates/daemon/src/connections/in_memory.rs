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

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use eyre::{Error, bail};
use moor_common::tasks::SessionError;
use moor_var::{Obj, Symbol};
use rpc_common::RpcMessageError;
use uuid::Uuid;

use crate::connections::persistence::{
    ClientMappingChanges, ConnectionRegistryPersistence, PlayerConnectionChanges,
};
use crate::connections::registry::{CONNECTION_TIMEOUT_DURATION, ConnectionRegistry};
use crate::connections::{ConnectionRecord, ConnectionsRecords};

/// Pure in-memory implementation of connections database with optional persistence
pub struct ConnectionRegistryMemory<P: ConnectionRegistryPersistence> {
    inner: Arc<Mutex<Inner>>,
    persistence: P,
}

struct Inner {
    // Maps client_id -> (connection_obj, player_obj)
    // connection_obj is always present, player_obj is Some after login
    client_objects: HashMap<Uuid, (Obj, Option<Obj>)>,
    // Maps connection objects to their connection records
    connection_records: HashMap<Obj, ConnectionsRecords>,
    // Maps player objects to their connection records (for logged-in players)
    player_connections: HashMap<Obj, ConnectionsRecords>,
}

impl<P: ConnectionRegistryPersistence> ConnectionRegistryMemory<P> {
    pub fn new(persistence: P) -> Result<Self, Error> {
        let initial_state = persistence.load_initial_state()?;

        // Convert old format to new format
        let mut client_objects = HashMap::new();
        let mut connection_records = HashMap::new();
        let mut player_connections = HashMap::new();

        // For now, treat existing data as player objects (backwards compatibility)
        for (client_id, obj) in initial_state.client_players {
            client_objects.insert(client_id, (obj, Some(obj)));
        }

        for (obj, connections) in initial_state.player_clients {
            connection_records.insert(obj, connections.clone());
            player_connections.insert(obj, connections);
        }

        let inner = Inner {
            client_objects,
            connection_records,
            player_connections,
        };

        Ok(Self {
            inner: Arc::new(Mutex::new(inner)),
            persistence,
        })
    }

    /// Persist all pending changes
    fn persist_changes(
        &self,
        client_changes: ClientMappingChanges,
        player_changes: PlayerConnectionChanges,
    ) -> Result<(), Error> {
        if !client_changes.is_empty() {
            self.persistence.persist_client_mappings(&client_changes)?;
        }
        if !player_changes.is_empty() {
            self.persistence
                .persist_player_connections(&player_changes)?;
        }
        Ok(())
    }
}

impl<P: ConnectionRegistryPersistence> ConnectionRegistry for ConnectionRegistryMemory<P> {
    fn associate_player_object(&self, connection_obj: Obj, player_obj: Obj) -> Result<(), Error> {
        let mut inner = self.inner.lock().unwrap();

        let Some(connections_record) = inner.connection_records.get(&connection_obj).cloned()
        else {
            bail!("No connection found for {:?}", connection_obj);
        };

        let mut client_changes = ClientMappingChanges::new();
        let mut player_changes = PlayerConnectionChanges::new();

        // Update all clients for this connection to have the player object
        for cr in &connections_record.connections {
            let client_id = Uuid::from_u128(cr.client_id);
            if let Some((conn_obj, _)) = inner.client_objects.get(&client_id).copied() {
                inner
                    .client_objects
                    .insert(client_id, (conn_obj, Some(player_obj)));
                client_changes.update(client_id, player_obj);
            }
        }

        // Add/merge connections to player_connections
        inner
            .player_connections
            .entry(player_obj)
            .and_modify(|existing| {
                existing
                    .connections
                    .extend(connections_record.connections.clone())
            })
            .or_insert(connections_record.clone());

        // Mark changes for persistence
        if let Some(connections) = inner.player_connections.get(&player_obj) {
            player_changes.update(player_obj, connections.clone());
        }

        drop(inner);
        self.persist_changes(client_changes, player_changes)?;
        Ok(())
    }

    fn new_connection(
        &self,
        client_id: Uuid,
        hostname: String,
        player: Option<Obj>,
        acceptable_content_types: Option<Vec<Symbol>>,
    ) -> Result<Obj, RpcMessageError> {
        let mut inner = self.inner.lock().unwrap();

        let connection_id = {
            let id = self
                .persistence
                .next_connection_sequence()
                .map_err(|e| RpcMessageError::InternalError(e.to_string()))?;
            Obj::mk_id(id)
        };

        // Store both connection object and optional player object
        inner
            .client_objects
            .insert(client_id, (connection_id, player));

        let now = SystemTime::now();
        let cr = ConnectionRecord {
            client_id: client_id.as_u128(),
            connected_time: now,
            last_activity: now,
            last_ping: now,
            hostname,
            acceptable_content_types: acceptable_content_types
                .unwrap_or_else(|| vec![Symbol::mk("text_plain")]),
        };

        // Add to connection records
        inner
            .connection_records
            .entry(connection_id)
            .or_insert(ConnectionsRecords {
                connections: vec![],
            })
            .connections
            .push(cr.clone());

        // If there's a player, also add to player connections
        if let Some(player_obj) = player {
            inner
                .player_connections
                .entry(player_obj)
                .or_insert(ConnectionsRecords {
                    connections: vec![],
                })
                .connections
                .push(cr);
        }

        // Prepare changes for persistence
        let mut client_changes = ClientMappingChanges::new();
        let mut player_changes = PlayerConnectionChanges::new();

        if let Some(player_obj) = player {
            client_changes.update(client_id, player_obj);
            if let Some(connections) = inner.player_connections.get(&player_obj) {
                player_changes.update(player_obj, connections.clone());
            }
        }

        drop(inner);
        self.persist_changes(client_changes, player_changes)
            .map_err(|e| RpcMessageError::InternalError(e.to_string()))?;

        Ok(connection_id)
    }

    fn record_client_activity(&self, client_id: Uuid, connobj: Obj) -> Result<(), Error> {
        let mut inner = self.inner.lock().unwrap();

        // Update connection record
        let Some(connections_record) = inner.connection_records.get_mut(&connobj) else {
            bail!("No connection found for {:?}", connobj);
        };

        let now = SystemTime::now();
        let Some(client) = connections_record
            .connections
            .iter_mut()
            .find(|cr| cr.client_id == client_id.as_u128())
        else {
            bail!("No client found for {:?}", client_id);
        };

        client.last_activity = now;

        let mut player_changes = PlayerConnectionChanges::new();

        // Also update player connection record if logged in
        if let Some((_, Some(player_obj))) = inner.client_objects.get(&client_id).copied() {
            if let Some(player_connections) = inner.player_connections.get_mut(&player_obj) {
                if let Some(client) = player_connections
                    .connections
                    .iter_mut()
                    .find(|cr| cr.client_id == client_id.as_u128())
                {
                    client.last_activity = now;
                }
                player_changes.update(player_obj, player_connections.clone());
            }
        }

        drop(inner);
        self.persist_changes(ClientMappingChanges::new(), player_changes)?;
        Ok(())
    }

    fn notify_is_alive(&self, client_id: Uuid, connection: Obj) -> Result<(), Error> {
        let mut inner = self.inner.lock().unwrap();

        // Update connection record
        let Some(connections_record) = inner.connection_records.get_mut(&connection) else {
            bail!("No connection found for {:?}", connection);
        };

        let now = SystemTime::now();
        let Some(cr) = connections_record
            .connections
            .iter_mut()
            .find(|cr| cr.client_id == client_id.as_u128())
        else {
            return Ok(());
        };

        cr.last_ping = now;

        let mut player_changes = PlayerConnectionChanges::new();

        // Also update player connection record if logged in
        if let Some((_, Some(player_obj))) = inner.client_objects.get(&client_id).copied() {
            if let Some(player_connections) = inner.player_connections.get_mut(&player_obj) {
                if let Some(cr) = player_connections
                    .connections
                    .iter_mut()
                    .find(|cr| cr.client_id == client_id.as_u128())
                {
                    cr.last_ping = now;
                }
                player_changes.update(player_obj, player_connections.clone());
            }
        }

        drop(inner);
        self.persist_changes(ClientMappingChanges::new(), player_changes)?;
        Ok(())
    }

    fn ping_check(&self) {
        let mut inner = self.inner.lock().unwrap();

        let mut to_remove = vec![];
        for (connection_id, connections_record) in inner.connection_records.iter() {
            for cr in &connections_record.connections {
                match cr.last_ping.elapsed() {
                    Ok(elapsed) if elapsed < CONNECTION_TIMEOUT_DURATION => {
                        continue;
                    }
                    _ => {
                        to_remove.push((*connection_id, cr.client_id));
                    }
                }
            }
        }

        let mut client_changes = ClientMappingChanges::new();
        let mut player_changes = PlayerConnectionChanges::new();

        for (connection_id, client_id) in to_remove {
            let client_uuid = Uuid::from_u128(client_id);

            // Remove from client_objects
            if let Some((_, player_obj)) = inner.client_objects.remove(&client_uuid) {
                client_changes.remove(client_uuid);

                // Remove from player connections if logged in
                if let Some(player_obj) = player_obj {
                    if let Some(player_connections) = inner.player_connections.get_mut(&player_obj)
                    {
                        player_connections
                            .connections
                            .retain(|cr| cr.client_id != client_id);
                        if player_connections.connections.is_empty() {
                            inner.player_connections.remove(&player_obj);
                            player_changes.remove(player_obj);
                        } else {
                            player_changes.update(player_obj, player_connections.clone());
                        }
                    }
                }
            }

            // Remove from connection records
            if let Some(connections_record) = inner.connection_records.get_mut(&connection_id) {
                connections_record
                    .connections
                    .retain(|cr| cr.client_id != client_id);
                if connections_record.connections.is_empty() {
                    inner.connection_records.remove(&connection_id);
                }
            }
        }

        drop(inner);
        self.persist_changes(client_changes, player_changes).ok();
    }

    fn last_activity_for(&self, connection: Obj) -> Result<SystemTime, SessionError> {
        let inner = self.inner.lock().unwrap();

        // Check both connection records and player connections
        let connections_record = inner
            .connection_records
            .get(&connection)
            .or_else(|| inner.player_connections.get(&connection))
            .ok_or(SessionError::NoConnectionForPlayer(connection))?;

        let Some(last_activity) = connections_record
            .connections
            .iter()
            .map(|cr| cr.last_activity)
            .max()
        else {
            return Err(SessionError::NoConnectionForPlayer(connection));
        };

        Ok(last_activity)
    }

    fn connection_name_for(&self, player: Obj) -> Result<String, SessionError> {
        let inner = self.inner.lock().unwrap();

        // Check both connection records and player connections
        let connections_records = inner
            .connection_records
            .get(&player)
            .or_else(|| inner.player_connections.get(&player))
            .ok_or(SessionError::NoConnectionForPlayer(player))?;

        let name = connections_records
            .connections
            .iter()
            .map(|cr| cr.hostname.clone())
            .next()
            .ok_or(SessionError::NoConnectionForPlayer(player))?;

        Ok(name)
    }

    fn connected_seconds_for(&self, player: Obj) -> Result<f64, SessionError> {
        let inner = self.inner.lock().unwrap();

        // Check both connection records and player connections
        let connections_record = inner
            .connection_records
            .get(&player)
            .or_else(|| inner.player_connections.get(&player))
            .ok_or(SessionError::NoConnectionForPlayer(player))?;

        let connected_seconds = connections_record
            .connections
            .iter()
            .map(|cr| cr.connected_time.elapsed().unwrap().as_secs_f64())
            .sum::<f64>();

        Ok(connected_seconds)
    }

    fn client_ids_for(&self, player: Obj) -> Result<Vec<Uuid>, SessionError> {
        let inner = self.inner.lock().unwrap();

        let empty_record = ConnectionsRecords {
            connections: vec![],
        };
        // Check both connection records and player connections
        let connections_record = inner
            .connection_records
            .get(&player)
            .or_else(|| inner.player_connections.get(&player))
            .unwrap_or(&empty_record);

        let client_ids = connections_record
            .connections
            .iter()
            .map(|cr| Uuid::from_u128(cr.client_id))
            .collect();

        Ok(client_ids)
    }

    fn connections(&self) -> Vec<Obj> {
        let inner = self.inner.lock().unwrap();

        let mut connections = Vec::new();

        // Add all connection objects
        connections.extend(
            inner
                .connection_records
                .iter()
                .filter(|&(_o, c)| !c.connections.is_empty())
                .map(|(o, _c)| *o),
        );

        // Add all player objects (for logged-in players)
        connections.extend(
            inner
                .player_connections
                .iter()
                .filter(|&(_o, c)| !c.connections.is_empty())
                .map(|(o, _c)| *o),
        );

        connections.sort();
        connections.dedup();
        connections
    }

    fn connection_object_for_client(&self, client_id: Uuid) -> Option<Obj> {
        let inner = self.inner.lock().unwrap();
        inner
            .client_objects
            .get(&client_id)
            .map(|(conn_obj, _)| *conn_obj)
    }

    fn player_object_for_client(&self, client_id: Uuid) -> Option<Obj> {
        let inner = self.inner.lock().unwrap();
        inner
            .client_objects
            .get(&client_id)
            .and_then(|(_, player_obj)| *player_obj)
    }

    fn remove_client_connection(&self, client_id: Uuid) -> Result<(), Error> {
        let mut inner = self.inner.lock().unwrap();

        let Some((connection_obj, player_obj)) = inner.client_objects.remove(&client_id) else {
            bail!("No connection to prune found for {:?}", client_id);
        };

        let mut client_changes = ClientMappingChanges::new();
        let mut player_changes = PlayerConnectionChanges::new();

        client_changes.remove(client_id);

        // Remove from connection records
        if let Some(connections_record) = inner.connection_records.get_mut(&connection_obj) {
            connections_record
                .connections
                .retain(|cr| cr.client_id != client_id.as_u128());
            if connections_record.connections.is_empty() {
                inner.connection_records.remove(&connection_obj);
            }
        }

        // Remove from player connections if logged in
        if let Some(player_obj) = player_obj {
            if let Some(connections_record) = inner.player_connections.get_mut(&player_obj) {
                connections_record
                    .connections
                    .retain(|cr| cr.client_id != client_id.as_u128());
                if connections_record.connections.is_empty() {
                    inner.player_connections.remove(&player_obj);
                    player_changes.remove(player_obj);
                } else {
                    player_changes.update(player_obj, connections_record.clone());
                }
            }
        }

        drop(inner);
        self.persist_changes(client_changes, player_changes)?;
        Ok(())
    }

    fn acceptable_content_types_for(&self, connection: Obj) -> Result<Vec<Symbol>, SessionError> {
        let inner = self.inner.lock().unwrap();

        if let Some(connection_records) = inner.connection_records.get(&connection) {
            // Return the content types from the first connection record
            // (all records for the same connection should have the same content types)
            if let Some(record) = connection_records.connections.first() {
                Ok(record.acceptable_content_types.clone())
            } else {
                // Default to text/plain if no records
                Ok(vec![Symbol::mk("text_plain")])
            }
        } else {
            Err(SessionError::NoConnectionForPlayer(connection))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connections::FIRST_CONNECTION_ID;
    use crate::connections::persistence::{InitialConnectionRegistryState, NullPersistence};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::Duration;
    use uuid::Uuid;

    #[test]
    fn test_in_memory_only_database() {
        let persistence = NullPersistence::new();
        let db = ConnectionRegistryMemory::new(persistence).unwrap();

        let client_id = Uuid::new_v4();
        let connection_obj = db
            .new_connection(client_id, "test.host".to_string(), None, None)
            .unwrap();

        // Test basic operations
        assert_eq!(
            db.connection_object_for_client(client_id),
            Some(connection_obj)
        );
        assert_eq!(db.player_object_for_client(client_id), None);
        assert_eq!(db.client_ids_for(connection_obj).unwrap(), vec![client_id]);
        assert_eq!(db.connections(), vec![connection_obj]);

        // Test activity tracking
        db.record_client_activity(client_id, connection_obj)
            .unwrap();
        db.notify_is_alive(client_id, connection_obj).unwrap();
        assert!(db.last_activity_for(connection_obj).is_ok());

        // Test removal
        db.remove_client_connection(client_id).unwrap();
        assert_eq!(db.connection_object_for_client(client_id), None);
        assert_eq!(db.connections(), vec![]);
    }

    #[test]
    fn test_persistence_batching() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        // Mock persistence that counts calls
        struct CountingPersistence {
            client_calls: Arc<AtomicUsize>,
            player_calls: Arc<AtomicUsize>,
            sequence: std::sync::atomic::AtomicI32,
        }

        impl CountingPersistence {
            fn new(client_calls: Arc<AtomicUsize>, player_calls: Arc<AtomicUsize>) -> Self {
                Self {
                    client_calls,
                    player_calls,
                    sequence: std::sync::atomic::AtomicI32::new(FIRST_CONNECTION_ID),
                }
            }
        }

        impl ConnectionRegistryPersistence for CountingPersistence {
            fn load_initial_state(&self) -> Result<InitialConnectionRegistryState, Error> {
                Ok(InitialConnectionRegistryState::default())
            }

            fn persist_client_mappings(
                &self,
                _changes: &ClientMappingChanges,
            ) -> Result<(), Error> {
                self.client_calls.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }

            fn persist_player_connections(
                &self,
                _changes: &PlayerConnectionChanges,
            ) -> Result<(), Error> {
                self.player_calls.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }

            fn next_connection_sequence(&self) -> Result<i32, Error> {
                Ok(self
                    .sequence
                    .fetch_sub(1, std::sync::atomic::Ordering::SeqCst))
            }
        }

        let client_calls = Arc::new(AtomicUsize::new(0));
        let player_calls = Arc::new(AtomicUsize::new(0));
        let persistence = CountingPersistence::new(client_calls.clone(), player_calls.clone());
        let db = ConnectionRegistryMemory::new(persistence).unwrap();

        // Create connection - should not trigger persistence calls (no player logged in)
        let client_id = Uuid::new_v4();
        let connection_obj = db
            .new_connection(client_id, "test.host".to_string(), None, None)
            .unwrap();

        assert_eq!(client_calls.load(Ordering::SeqCst), 0);
        assert_eq!(player_calls.load(Ordering::SeqCst), 0);

        // Activity update - should not trigger persistence (no player logged in)
        db.record_client_activity(client_id, connection_obj)
            .unwrap();
        assert_eq!(client_calls.load(Ordering::SeqCst), 0); // No change
        assert_eq!(player_calls.load(Ordering::SeqCst), 0); // No change
    }

    #[test]
    fn test_concurrent_connection_creation() {
        let persistence = NullPersistence::new();
        let db = Arc::new(ConnectionRegistryMemory::new(persistence).unwrap());
        let num_threads = 10;
        let connections_per_thread = 20;

        let barrier = Arc::new(Barrier::new(num_threads));
        let mut handles = vec![];

        for i in 0..num_threads {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);

            let handle = thread::spawn(move || {
                barrier.wait();

                let mut created_clients = vec![];
                for j in 0..connections_per_thread {
                    let client_id = Uuid::new_v4();
                    let hostname = format!("host-{}-{}.test", i, j);

                    match db.new_connection(client_id, hostname, None, None) {
                        Ok(connection_obj) => {
                            created_clients.push((client_id, connection_obj));
                        }
                        Err(e) => panic!("Failed to create connection: {:?}", e),
                    }
                }
                created_clients
            });
            handles.push(handle);
        }

        let mut all_connections = vec![];
        for handle in handles {
            all_connections.extend(handle.join().unwrap());
        }

        // Verify all connections were created successfully
        assert_eq!(all_connections.len(), num_threads * connections_per_thread);

        // Verify all client IDs are unique
        let mut client_ids: Vec<_> = all_connections.iter().map(|(id, _)| *id).collect();
        client_ids.sort();
        client_ids.dedup();
        assert_eq!(client_ids.len(), num_threads * connections_per_thread);

        // Verify all connections are accessible
        for (client_id, connection_obj) in &all_connections {
            assert_eq!(
                db.connection_object_for_client(*client_id),
                Some(*connection_obj)
            );
        }
    }

    #[test]
    fn test_concurrent_activity_updates() {
        let persistence = NullPersistence::new();
        let db = Arc::new(ConnectionRegistryMemory::new(persistence).unwrap());

        // Create some initial connections
        let mut client_connections = vec![];
        for i in 0..5 {
            let client_id = Uuid::new_v4();
            let connection_obj = db
                .new_connection(client_id, format!("host-{}.test", i), None, None)
                .unwrap();
            client_connections.push((client_id, connection_obj));
        }

        let num_threads = 8;
        let updates_per_thread = 50;
        let barrier = Arc::new(Barrier::new(num_threads));
        let mut handles = vec![];

        for _ in 0..num_threads {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            let connections = client_connections.clone();

            let handle = thread::spawn(move || {
                barrier.wait();

                for _ in 0..updates_per_thread {
                    for (client_id, connection_obj) in &connections {
                        // Alternate between different types of updates
                        let _ = db.record_client_activity(*client_id, *connection_obj);
                        let _ = db.notify_is_alive(*client_id, *connection_obj);
                        let _ = db.last_activity_for(*connection_obj);
                        let _ = db.client_ids_for(*connection_obj);
                    }
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all connections are still accessible and valid
        for (client_id, connection_obj) in &client_connections {
            assert_eq!(
                db.connection_object_for_client(*client_id),
                Some(*connection_obj)
            );
            assert!(db.last_activity_for(*connection_obj).is_ok());
            assert!(!db.client_ids_for(*connection_obj).unwrap().is_empty());
        }
    }

    #[test]
    fn test_concurrent_creation_and_removal() {
        let persistence = NullPersistence::new();
        let db = Arc::new(ConnectionRegistryMemory::new(persistence).unwrap());
        let num_threads = 6;
        let operations_per_thread = 30;

        let barrier = Arc::new(Barrier::new(num_threads));
        let creation_count = Arc::new(AtomicUsize::new(0));
        let removal_count = Arc::new(AtomicUsize::new(0));
        let mut handles = vec![];

        for thread_id in 0..num_threads {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            let creation_count = Arc::clone(&creation_count);
            let removal_count = Arc::clone(&removal_count);

            let handle = thread::spawn(move || {
                barrier.wait();

                let mut local_connections = vec![];

                for i in 0..operations_per_thread {
                    // Create connections
                    let client_id = Uuid::new_v4();
                    let hostname = format!("host-{}-{}.test", thread_id, i);

                    if let Ok(connection_obj) = db.new_connection(client_id, hostname, None, None) {
                        creation_count.fetch_add(1, Ordering::SeqCst);
                        local_connections.push((client_id, connection_obj));

                        // Do some activity
                        let _ = db.record_client_activity(client_id, connection_obj);
                        let _ = db.notify_is_alive(client_id, connection_obj);
                    }

                    // Randomly remove some connections
                    if i > 5 && i % 3 == 0 && !local_connections.is_empty() {
                        let idx = i % local_connections.len();
                        let (client_id, _) = local_connections.remove(idx);
                        if db.remove_client_connection(client_id).is_ok() {
                            removal_count.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                }

                // Clean up remaining connections
                for (client_id, _) in local_connections {
                    if db.remove_client_connection(client_id).is_ok() {
                        removal_count.fetch_add(1, Ordering::SeqCst);
                    }
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let final_creation_count = creation_count.load(Ordering::SeqCst);
        let final_removal_count = removal_count.load(Ordering::SeqCst);

        // Should have created some connections
        assert!(final_creation_count > 0);
        // Should have removed some connections
        assert!(final_removal_count > 0);
        // Final connection count should make sense
        let remaining_connections = db.connections().len();
        assert_eq!(
            remaining_connections,
            final_creation_count - final_removal_count
        );
    }

    #[test]
    fn test_ping_check_with_concurrent_operations() {
        let persistence = NullPersistence::new();
        let db = Arc::new(ConnectionRegistryMemory::new(persistence).unwrap());

        // Create some connections that will timeout
        let mut old_connections = vec![];
        for i in 0..3 {
            let client_id = Uuid::new_v4();
            let connection_obj = db
                .new_connection(client_id, format!("old-host-{}.test", i), None, None)
                .unwrap();
            old_connections.push((client_id, connection_obj));
        }

        // Simulate old connections by not updating their ping times
        // (In real usage, ping_check would clean these up after timeout)

        let num_threads = 4;
        let barrier = Arc::new(Barrier::new(num_threads + 1)); // +1 for ping thread
        let mut handles = vec![];

        // Spawn ping checker thread
        let ping_db = Arc::clone(&db);
        let ping_barrier = Arc::clone(&barrier);
        let ping_handle = thread::spawn(move || {
            ping_barrier.wait();

            // Run ping check multiple times
            for _ in 0..20 {
                ping_db.ping_check();
                thread::sleep(Duration::from_millis(10));
            }
        });

        // Spawn worker threads doing various operations
        for i in 0..num_threads {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);

            let handle = thread::spawn(move || {
                barrier.wait();

                for j in 0..30 {
                    let client_id = Uuid::new_v4();
                    let hostname = format!("new-host-{}-{}.test", i, j);

                    if let Ok(connection_obj) = db.new_connection(client_id, hostname, None, None) {
                        // Keep these connections alive
                        let _ = db.notify_is_alive(client_id, connection_obj);
                        let _ = db.record_client_activity(client_id, connection_obj);

                        // Do some queries
                        let _ = db.connection_object_for_client(client_id);
                        let _ = db.client_ids_for(connection_obj);
                        let _ = db.connections();

                        // Some connections get removed
                        if j % 5 == 0 {
                            let _ = db.remove_client_connection(client_id);
                        }
                    }

                    thread::sleep(Duration::from_millis(5));
                }
            });
            handles.push(handle);
        }

        ping_handle.join().unwrap();
        for handle in handles {
            handle.join().unwrap();
        }

        // Database should still be in a consistent state
        let connections = db.connections();
        for conn in &connections {
            let client_ids = db.client_ids_for(*conn).unwrap();
            for client_id in client_ids {
                assert_eq!(db.connection_object_for_client(client_id), Some(*conn));
            }
        }
    }

    #[test]
    fn test_persistence_under_concurrency() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct ConcurrentCountingPersistence {
            client_calls: Arc<AtomicUsize>,
            player_calls: Arc<AtomicUsize>,
            sequence: std::sync::atomic::AtomicI32,
        }

        impl ConcurrentCountingPersistence {
            fn new(client_calls: Arc<AtomicUsize>, player_calls: Arc<AtomicUsize>) -> Self {
                Self {
                    client_calls,
                    player_calls,
                    sequence: std::sync::atomic::AtomicI32::new(-1000),
                }
            }
        }

        impl ConnectionRegistryPersistence for ConcurrentCountingPersistence {
            fn load_initial_state(&self) -> Result<InitialConnectionRegistryState, Error> {
                Ok(InitialConnectionRegistryState::default())
            }

            fn persist_client_mappings(
                &self,
                _changes: &ClientMappingChanges,
            ) -> Result<(), Error> {
                self.client_calls.fetch_add(1, Ordering::SeqCst);
                // Simulate some work
                thread::sleep(Duration::from_micros(100));
                Ok(())
            }

            fn persist_player_connections(
                &self,
                _changes: &PlayerConnectionChanges,
            ) -> Result<(), Error> {
                self.player_calls.fetch_add(1, Ordering::SeqCst);
                // Simulate some work
                thread::sleep(Duration::from_micros(100));
                Ok(())
            }

            fn next_connection_sequence(&self) -> Result<i32, Error> {
                Ok(self.sequence.fetch_sub(1, Ordering::SeqCst))
            }
        }

        let client_calls = Arc::new(AtomicUsize::new(0));
        let player_calls = Arc::new(AtomicUsize::new(0));
        let persistence =
            ConcurrentCountingPersistence::new(client_calls.clone(), player_calls.clone());
        let db = Arc::new(ConnectionRegistryMemory::new(persistence).unwrap());

        let num_threads = 6;
        let operations_per_thread = 15;
        let barrier = Arc::new(Barrier::new(num_threads));
        let mut handles = vec![];

        for thread_id in 0..num_threads {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);

            let handle = thread::spawn(move || {
                barrier.wait();

                let mut connections = vec![];

                for i in 0..operations_per_thread {
                    let client_id = Uuid::new_v4();
                    let hostname = format!("persist-host-{}-{}.test", thread_id, i);

                    if let Ok(connection_obj) = db.new_connection(client_id, hostname, None, None) {
                        connections.push((client_id, connection_obj));

                        // For some connections, associate a player to test player persistence
                        if i % 2 == 0 {
                            let player_obj = Obj::mk_id(-(thread_id as i32 + 1000) - i);
                            let _ = db.associate_player_object(connection_obj, player_obj);
                        }

                        // Generate activity that requires persistence
                        let _ = db.record_client_activity(client_id, connection_obj);
                        let _ = db.notify_is_alive(client_id, connection_obj);
                    }
                }

                // Clean up half the connections
                for (client_id, _connection_obj) in &connections {
                    let _ = db.remove_client_connection(*client_id);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify persistence was called appropriately
        let total_client_calls = client_calls.load(Ordering::SeqCst);
        let total_player_calls = player_calls.load(Ordering::SeqCst);

        // Should have made persistence calls
        assert!(total_client_calls > 0);
        assert!(total_player_calls > 0);

        // Player calls should be higher (creation + activity + removal)
        assert!(total_player_calls >= total_client_calls);

        // Database should remain consistent
        let connections = db.connections();
        for conn in &connections {
            let client_ids = db.client_ids_for(*conn).unwrap();
            assert!(!client_ids.is_empty());
        }
    }
}
