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

use crate::connections::{ConnectionsDB, CONNECTION_TIMEOUT_DURATION};
use bincode::{Decode, Encode};
use bytes::Bytes;
use eyre::{bail, Error};
use fjall::{Config, Keyspace, PartitionCreateOptions, PartitionHandle};
use moor_kernel::tasks::sessions::SessionError;
use moor_values::{AsByteBuffer, Obj, BINCODE_CONFIG};
use rpc_common::RpcMessageError;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Debug, Clone, Encode, Decode)]
struct ConnectionRecord {
    client_id: u128,
    connected_time: SystemTime,
    last_activity: SystemTime,
    last_ping: SystemTime,
    hostname: String,
}

#[derive(Debug, Clone, Encode, Decode)]
struct ConnectionsRecords {
    connections: Vec<ConnectionRecord>,
}

pub struct ConnectionsFjall {
    inner: Arc<Mutex<Inner>>,
}
struct Inner {
    _tmpdir: Option<tempfile::TempDir>,
    _keyspace: Keyspace,
    /// From ClientId -> Obj
    client_player_table: PartitionHandle,
    /// From Objid -> Connectionsrecord
    player_clients_table: PartitionHandle,

    connection_id_sequence: i32,
    connection_id_sequence_table: PartitionHandle,

    client_players: HashMap<Uuid, Obj>,
    player_clients: HashMap<Obj, ConnectionsRecords>,
}

impl ConnectionsFjall {
    pub fn open(path: Option<&Path>) -> Self {
        let (tmpdir, path) = match path {
            Some(path) => (None, path.to_path_buf()),
            None => {
                let tmpdir = tempfile::TempDir::new().unwrap();
                let path = tmpdir.path().to_path_buf();
                (Some(tmpdir), path)
            }
        };

        info!("Opening connections database at {:?}", path);
        let keyspace = Config::new(&path).open().unwrap();
        let sequences_partition = keyspace
            .open_partition("connection_sequences", PartitionCreateOptions::default())
            .unwrap();

        let client_player_table = keyspace
            .open_partition("client_player", PartitionCreateOptions::default())
            .unwrap();

        let player_clients_table = keyspace
            .open_partition("player_clients", PartitionCreateOptions::default())
            .unwrap();

        // Fill in the connection_id_sequence.
        let connection_id_sequence = match sequences_partition.get("connection_id_sequence") {
            Ok(Some(bytes)) => i32::from_le_bytes(bytes[0..size_of::<i32>()].try_into().unwrap()),
            _ => -3,
        };

        // Fill in all the caches.
        let mut client_players = HashMap::new();
        let mut player_clients = HashMap::new();
        for entry in client_player_table.iter() {
            let (key, value) = entry.unwrap();
            let client_id = Uuid::from_u128(u128::from_le_bytes(
                key[0..size_of::<u128>()].try_into().unwrap(),
            ));
            let oid = Obj::from_bytes(Bytes::from(value)).unwrap();
            client_players.insert(client_id, oid);
        }
        for entry in player_clients_table.iter() {
            let (key, value) = entry.unwrap();
            let oid = Obj::from_bytes(Bytes::from(key)).unwrap();
            let (connections_record, _) =
                bincode::decode_from_slice(&value, *BINCODE_CONFIG).unwrap();
            player_clients.insert(oid, connections_record);
        }

        Self {
            inner: Arc::new(Mutex::new(Inner {
                _tmpdir: tmpdir,
                _keyspace: keyspace,
                client_player_table,
                player_clients_table,
                connection_id_sequence,
                connection_id_sequence_table: sequences_partition,
                client_players,
                player_clients,
            })),
        }
    }
}

impl ConnectionsDB for ConnectionsFjall {
    fn update_client_connection(&self, from_connection: Obj, to_player: Obj) -> Result<(), Error> {
        let mut inner = self.inner.lock().unwrap();

        let Some(mut crs) = inner.player_clients.remove(&from_connection) else {
            bail!("No connection found for {:?}", from_connection);
        };

        let from_oid_bytes = from_connection.as_bytes().unwrap();
        inner.player_clients_table.remove(from_oid_bytes).ok();

        for cr in &mut crs.connections {
            let client_id = cr.client_id;
            // Associate the client with the new player id.
            inner
                .client_players
                .insert(Uuid::from_u128(client_id), to_player.clone());

            let to_oid_bytes = to_player.as_bytes().unwrap();
            inner
                .client_player_table
                .insert(
                    Uuid::from_u128(client_id).as_u128().to_le_bytes(),
                    to_oid_bytes,
                )
                .ok();
        }

        // If `to_player` already had a connection record, merge the two.
        if let Some(mut to_player_connections) = inner.player_clients.remove(&to_player) {
            // Remove from the underlying physical storage.
            let to_oid_bytes = to_player.as_bytes().unwrap();
            inner.player_clients_table.remove(to_oid_bytes).ok();

            crs.connections
                .append(&mut to_player_connections.connections);
        }

        inner.player_clients.insert(to_player.clone(), crs.clone());
        let encoded_cr = bincode::encode_to_vec(crs, *BINCODE_CONFIG).unwrap();
        inner
            .player_clients_table
            .insert(to_player.as_bytes().unwrap(), &encoded_cr)
            .ok();
        Ok(())
    }

    fn new_connection(
        &self,
        client_id: Uuid,
        hostname: String,
        player: Option<Obj>,
    ) -> Result<Obj, RpcMessageError> {
        // Increment sequence.
        let mut inner = self.inner.lock().unwrap();

        let player_id = match player {
            None => {
                let id = inner.connection_id_sequence;
                inner.connection_id_sequence -= 1;
                inner
                    .connection_id_sequence_table
                    .insert(
                        "connection_id_sequence",
                        inner.connection_id_sequence.to_le_bytes(),
                    )
                    .unwrap();
                Obj::mk_id(id)
            }
            Some(id) => id,
        };

        inner.client_players.insert(client_id, player_id.clone());
        let oid_bytes = player_id.as_bytes().unwrap();
        inner
            .client_player_table
            .insert(client_id.as_u128().to_le_bytes(), oid_bytes.clone())
            .unwrap();

        let now = SystemTime::now();
        let cr = ConnectionRecord {
            client_id: client_id.as_u128(),
            connected_time: now,
            last_activity: now,
            last_ping: now,
            hostname,
        };
        inner
            .player_clients
            .entry(player_id.clone())
            .or_insert(ConnectionsRecords {
                connections: vec![],
            })
            .connections
            .push(cr);

        let connections_record = inner.player_clients.remove(&player_id).unwrap();
        inner
            .player_clients
            .insert(player_id.clone(), connections_record.clone());
        let encoded_connected =
            bincode::encode_to_vec(connections_record, *BINCODE_CONFIG).unwrap();
        inner
            .player_clients_table
            .insert(oid_bytes, &encoded_connected)
            .unwrap();

        Ok(player_id)
    }

    fn record_client_activity(&self, client_id: Uuid, connobj: Obj) -> Result<(), Error> {
        let mut inner = self.inner.lock().unwrap();
        let Some(connections_record) = inner.player_clients.get_mut(&connobj) else {
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

        let oid_bytes = connobj.as_bytes()?;
        let encoded_connected =
            bincode::encode_to_vec(connections_record.clone(), *BINCODE_CONFIG).unwrap();
        inner
            .player_clients_table
            .insert(oid_bytes, &encoded_connected)
            .unwrap();

        Ok(())
    }

    fn notify_is_alive(&self, client_id: Uuid, connection: Obj) -> Result<(), Error> {
        let mut inner = self.inner.lock().unwrap();
        let Some(connections_record) = inner.player_clients.get_mut(&connection) else {
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

        let oid_bytes = connection.as_bytes()?;
        let encoded_connected =
            bincode::encode_to_vec(connections_record.clone(), *BINCODE_CONFIG).unwrap();
        inner
            .player_clients_table
            .insert(oid_bytes, &encoded_connected)
            .unwrap();

        Ok(())
    }

    fn ping_check(&self) {
        // Scan all connections and if ping time is older than CONNECTION_TIMEOUT_DURATION,
        // turf.
        let mut inner = self.inner.lock().unwrap();
        let mut to_remove = vec![];
        for (player_id, connections_record) in inner.player_clients.iter_mut() {
            for cr in &connections_record.connections {
                if cr.last_ping.elapsed().unwrap() > CONNECTION_TIMEOUT_DURATION {
                    to_remove.push((player_id.clone(), cr.client_id));
                }
            }
        }

        for (player_id, client_id) in to_remove {
            let oid_bytes = player_id.as_bytes().unwrap();
            let mut connections_record = inner.player_clients.get(&player_id).unwrap().clone();
            connections_record
                .connections
                .retain(|cr| cr.client_id != client_id);
            inner
                .player_clients
                .insert(player_id.clone(), connections_record.clone());
            let encoded_connected =
                bincode::encode_to_vec(&connections_record, *BINCODE_CONFIG).unwrap();
            inner
                .player_clients_table
                .insert(oid_bytes, &encoded_connected)
                .unwrap();
        }
    }

    fn last_activity_for(&self, connection: Obj) -> Result<SystemTime, SessionError> {
        let inner = self.inner.lock().unwrap();
        let Some(connections_record) = inner.player_clients.get(&connection) else {
            return Err(SessionError::NoConnectionForPlayer(connection));
        };
        let connections_record = connections_record.clone();
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
        let connections_records = inner
            .player_clients
            .get(&player)
            .expect("no record")
            .clone();
        let name = connections_records
            .connections
            .iter()
            .map(|cr| cr.hostname.clone())
            .next()
            .unwrap();
        Ok(name)
    }

    fn connected_seconds_for(&self, player: Obj) -> Result<f64, SessionError> {
        let inner = self.inner.lock().unwrap();
        let Some(connections_record) = inner.player_clients.get(&player) else {
            return Err(SessionError::NoConnectionForPlayer(player));
        };
        let connected_seconds = connections_record
            .connections
            .iter()
            .map(|cr| cr.connected_time.elapsed().unwrap().as_secs_f64())
            .sum::<f64>();
        Ok(connected_seconds)
    }

    fn client_ids_for(&self, player: Obj) -> Result<Vec<Uuid>, SessionError> {
        let inner = self.inner.lock().unwrap();
        let connections_record = inner
            .player_clients
            .get(&player)
            .unwrap_or(&ConnectionsRecords {
                connections: vec![],
            })
            .clone();
        let client_ids = connections_record
            .connections
            .iter()
            .map(|cr| Uuid::from_u128(cr.client_id))
            .collect();
        Ok(client_ids)
    }

    fn connections(&self) -> Vec<Obj> {
        let inner = self.inner.lock().unwrap();
        inner
            .player_clients
            .iter()
            .filter_map(|(o, c)| (!c.connections.is_empty()).then(|| o.clone()))
            .collect()
    }

    fn connection_object_for_client(&self, client_id: Uuid) -> Option<Obj> {
        let inner = self.inner.lock().unwrap();
        inner.client_players.get(&client_id).cloned()
    }

    fn remove_client_connection(&self, client_id: Uuid) -> Result<(), Error> {
        let mut inner = self.inner.lock().unwrap();
        let Some(player_id) = inner.client_players.remove(&client_id) else {
            bail!("No connection to prune found for {:?}", client_id);
        };
        if !inner
            .client_player_table
            .remove(client_id.as_u128().to_le_bytes())
            .is_ok()
        {
            warn!("No existing record for client {client_id:?} at removal");
        };

        let Some(mut connections_record) = inner.player_clients.remove(&player_id) else {
            return Ok(());
        };
        connections_record
            .connections
            .retain(|cr| cr.client_id != client_id.as_u128());

        let oid_bytes = player_id.as_bytes().unwrap();
        if connections_record.connections.is_empty() {
            inner
                .player_clients
                .insert(player_id.clone(), connections_record.clone());
            inner.player_clients_table.remove(oid_bytes).ok();
        } else {
            let encoded_connected =
                bincode::encode_to_vec(connections_record, *BINCODE_CONFIG).unwrap();
            inner
                .player_clients_table
                .insert(oid_bytes, &encoded_connected)
                .ok();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use moor_values::Obj;

    use crate::connections::ConnectionsDB;
    use crate::connections_fjall::ConnectionsFjall;

    /// Simple test of:
    ///     * Attach a connection<->client
    ///     * Record activity & verify
    ///     * Update connection<->client to a new connection
    ///     * Verify the old connection has no clients
    ///     * Verify the new connection has the client
    ///     * Remove the connection<->client
    ///     * Verify the connection has no clients
    #[test]
    fn test_single_connection() {
        let db = Arc::new(ConnectionsFjall::open(None));
        let mut jh = vec![];

        for x in 1..10 {
            let db = db.clone();
            jh.push(std::thread::spawn(move || {
                let client_id = uuid::Uuid::new_v4();
                let oid = db
                    .new_connection(client_id, "localhost".to_string(), None)
                    .unwrap();
                let client_ids = db.client_ids_for(oid.clone()).unwrap();
                assert_eq!(client_ids.len(), 1);
                assert_eq!(client_ids[0], client_id);
                db.record_client_activity(client_id, oid.clone()).unwrap();
                db.notify_is_alive(client_id, oid.clone()).unwrap();
                let last_activity = db.last_activity_for(oid.clone());
                assert!(
                    last_activity.is_ok(),
                    "Unable to get last activity for {x} ({oid}) client {client_id}",
                );
                let last_activity = last_activity.unwrap().elapsed().unwrap().as_secs_f64();
                assert!(last_activity < 1.0);
                assert_eq!(
                    db.connection_object_for_client(client_id),
                    Some(oid.clone())
                );
                let connection_object = Obj::mk_id(x);
                db.update_client_connection(oid, connection_object.clone())
                    .unwrap_or_else(|e| {
                        panic!("Unable to update client connection for {:?}: {:?}", x, e)
                    });
                let client_ids = db.client_ids_for(connection_object.clone()).unwrap();
                assert_eq!(client_ids.len(), 1);
                assert_eq!(client_ids[0], client_id);
                db.remove_client_connection(client_id).unwrap();
                assert!(db.connection_object_for_client(client_id).is_none());
                let client_ids = db.client_ids_for(connection_object).unwrap();
                assert!(client_ids.is_empty());
            }));
        }
        for j in jh {
            j.join().unwrap();
        }
    }

    #[test]
    fn open_close() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let db = Arc::new(ConnectionsFjall::open(Some(tmp_dir.path())));
        let client_id1 = uuid::Uuid::new_v4();
        let ob = db
            .new_connection(client_id1, "localhost".to_string(), None)
            .unwrap();
        db.ping_check();
        let client_ids = db.connections();
        assert_eq!(client_ids.len(), 1);
        assert_eq!(
            db.connection_object_for_client(client_id1),
            Some(ob.clone())
        );

        let client_ids = db.client_ids_for(ob.clone()).unwrap();
        assert_eq!(client_ids.len(), 1);
        assert_eq!(client_ids[0], client_id1);

        drop(db);
        let db = Arc::new(ConnectionsFjall::open(Some(tmp_dir.path())));
        let client_ids = db.connections();
        assert_eq!(client_ids.len(), 1);
        assert_eq!(db.connection_object_for_client(client_id1), Some(ob));
    }

    // Validate that ping check works.
    #[test]
    fn ping_test() {
        let db = Arc::new(ConnectionsFjall::open(None));
        let client_id1 = uuid::Uuid::new_v4();
        let ob = db
            .new_connection(client_id1, "localhost".to_string(), None)
            .unwrap();
        db.ping_check();
        let client_ids = db.connections();
        assert_eq!(client_ids.len(), 1);
        assert_eq!(
            db.connection_object_for_client(client_id1),
            Some(ob.clone())
        );

        let client_ids = db.client_ids_for(ob).unwrap();
        assert_eq!(client_ids.len(), 1);
        assert_eq!(client_ids[0], client_id1);
    }

    /// When restarting the DB, old connections that were removed from the cache were living a
    /// a second life because they weren't removed from the DB.
    #[test]
    fn update_client_connections_regression() {
        let db_path = tempfile::tempdir().unwrap();
        let db = Arc::new(ConnectionsFjall::open(Some(db_path.path())));

        let client_id1 = uuid::Uuid::new_v4();
        let ob = db
            .new_connection(client_id1, "localhost".to_string(), None)
            .unwrap();
        assert_eq!(db.connections(), vec![ob.clone()]);
        db.update_client_connection(ob.clone(), Obj::mk_id(1))
            .unwrap();
        let connections = db.connections();
        assert_eq!(connections, vec![Obj::mk_id(1)]);

        drop(db);

        let db = Arc::new(ConnectionsFjall::open(Some(db_path.path())));
        let connections = db.connections();
        assert_eq!(connections, vec![Obj::mk_id(1)]);
    }

    #[test]
    fn remove_connection() {
        let db_path = tempfile::tempdir().unwrap();
        let db = Arc::new(ConnectionsFjall::open(Some(db_path.path())));

        let client_id1 = uuid::Uuid::new_v4();
        let ob = db
            .new_connection(client_id1, "localhost".to_string(), None)
            .unwrap();
        assert_eq!(db.connections(), vec![ob.clone()]);
        db.remove_client_connection(client_id1).unwrap();
        assert_eq!(db.connections(), vec![]);
    }
}
