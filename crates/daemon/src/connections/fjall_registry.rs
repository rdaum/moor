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

//! Direct fjall-backed connection registry with no in-memory caching

use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
    time::SystemTime,
};

use crate::connections::{
    ConnectionRecord, ConnectionsRecords, FIRST_CONNECTION_ID,
    conversions::{connections_records_from_bytes, connections_records_to_bytes},
    registry::{CONNECTION_TIMEOUT_DURATION, ConnectionRegistry, NewConnectionParams},
};
use byteview::ByteView;
use eyre::{Error, bail};
use fjall::{Config, Keyspace, PartitionCreateOptions, PartitionHandle};
use moor_common::tasks::SessionError;
use moor_var::{Obj, Symbol, Var};
use rpc_common::RpcMessageError;
use tracing::info;
use uuid::Uuid;
use zerocopy::IntoBytes;

/// Timestamp data for a client connection (cached in memory for performance)
#[derive(Debug, Clone)]
struct ClientTimestamps {
    last_activity: SystemTime,
    last_ping: SystemTime,
}

/// Direct fjall-backed connection registry
pub struct FjallConnectionRegistry {
    inner: Arc<Mutex<FjallInner>>,
    // In-memory cache for timestamps (hot path optimization)
    timestamps: Arc<Mutex<HashMap<Uuid, ClientTimestamps>>>,
}

struct FjallInner {
    _tmpdir: Option<tempfile::TempDir>,
    _keyspace: Keyspace,
    client_connection_table: PartitionHandle, // client_id (u128) -> connection_obj (Obj bytes)
    client_player_table: PartitionHandle, // client_id (u128) -> player_obj (Obj bytes), only after login
    connection_records_table: PartitionHandle, // connection_obj (Obj bytes) -> ConnectionsRecords
    player_clients_table: PartitionHandle, // player_obj (Obj bytes) -> ConnectionsRecords, only after login
    connection_id_sequence: i32,
    connection_id_sequence_table: PartitionHandle,
}

impl FjallConnectionRegistry {
    pub fn open(path: Option<&Path>) -> Result<Self, Error> {
        let (tmpdir, path) = match path {
            Some(path) => (None, path.to_path_buf()),
            None => {
                let tmpdir = tempfile::TempDir::new()?;
                let path = tmpdir.path().to_path_buf();
                (Some(tmpdir), path)
            }
        };

        info!("Opening connections database at {:?}", path);
        let keyspace = Config::new(&path).open()?;

        let sequences_partition =
            keyspace.open_partition("connection_sequences", PartitionCreateOptions::default())?;

        let client_connection_table =
            keyspace.open_partition("client_connection", PartitionCreateOptions::default())?;

        let client_player_table =
            keyspace.open_partition("client_player", PartitionCreateOptions::default())?;

        let connection_records_table =
            keyspace.open_partition("connection_records", PartitionCreateOptions::default())?;

        let player_clients_table =
            keyspace.open_partition("player_clients", PartitionCreateOptions::default())?;

        let connection_id_sequence = match sequences_partition.get("connection_id_sequence") {
            Ok(Some(bytes)) => i32::from_le_bytes(bytes[0..size_of::<i32>()].try_into()?),
            _ => FIRST_CONNECTION_ID,
        };

        let inner = FjallInner {
            _tmpdir: tmpdir,
            _keyspace: keyspace,
            client_connection_table,
            client_player_table,
            connection_records_table,
            player_clients_table,
            connection_id_sequence,
            connection_id_sequence_table: sequences_partition,
        };

        // Load existing timestamps from disk into memory cache
        let mut timestamps_cache = HashMap::new();
        for entry in inner.connection_records_table.iter() {
            if let Ok((_, value)) = entry
                && let Ok(connections_record) = connections_records_from_bytes(&value)
            {
                for cr in connections_record.connections {
                    timestamps_cache.insert(
                        Uuid::from_u128(cr.client_id),
                        ClientTimestamps {
                            last_activity: cr.last_activity,
                            last_ping: cr.last_ping,
                        },
                    );
                }
            }
        }

        for entry in inner.player_clients_table.iter() {
            if let Ok((_, value)) = entry
                && let Ok(connections_record) = connections_records_from_bytes(&value)
            {
                for cr in connections_record.connections {
                    // Use the most recent timestamp if already in cache
                    timestamps_cache
                        .entry(Uuid::from_u128(cr.client_id))
                        .and_modify(|ts| {
                            ts.last_activity = ts.last_activity.max(cr.last_activity);
                            ts.last_ping = ts.last_ping.max(cr.last_ping);
                        })
                        .or_insert(ClientTimestamps {
                            last_activity: cr.last_activity,
                            last_ping: cr.last_ping,
                        });
                }
            }
        }

        info!(
            "Loaded {} client timestamps into cache",
            timestamps_cache.len()
        );

        Ok(Self {
            inner: Arc::new(Mutex::new(inner)),
            timestamps: Arc::new(Mutex::new(timestamps_cache)),
        })
    }

    // Helper function to persist a timestamp update to disk (called async)
    fn persist_timestamp_update_async(
        inner: Arc<Mutex<FjallInner>>,
        client_id: Uuid,
        now: SystemTime,
        is_activity: bool, // true for last_activity, false for last_ping
    ) {
        let guard = match inner.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        // Try to update connection_records
        if let Ok(conn_obj_bytes) = Self::get_connection_obj_for_client_sync(&guard, client_id)
            && let Ok(Some(bytes)) = guard.connection_records_table.get(&conn_obj_bytes)
            && let Ok(mut connections_record) = connections_records_from_bytes(&bytes)
            && let Some(cr) = connections_record
                .connections
                .iter_mut()
                .find(|cr| cr.client_id == client_id.as_u128())
        {
            if is_activity {
                cr.last_activity = now;
            } else {
                cr.last_ping = now;
            }

            if let Ok(encoded) = connections_records_to_bytes(&connections_record) {
                let _ = guard
                    .connection_records_table
                    .insert(conn_obj_bytes, &encoded);
            }
        }

        // Try to update player_clients if logged in
        if let Ok(Some(bytes)) = guard
            .client_player_table
            .get(client_id.as_u128().to_le_bytes())
            && let Ok(player_obj) = Obj::from_bytes(bytes.as_ref())
            && let Ok(Some(bytes)) = guard.player_clients_table.get(player_obj.as_bytes())
            && let Ok(mut player_connections) = connections_records_from_bytes(&bytes)
            && let Some(cr) = player_connections
                .connections
                .iter_mut()
                .find(|cr| cr.client_id == client_id.as_u128())
        {
            if is_activity {
                cr.last_activity = now;
            } else {
                cr.last_ping = now;
            }

            if let Ok(encoded) = connections_records_to_bytes(&player_connections) {
                let _ = guard
                    .player_clients_table
                    .insert(player_obj.as_bytes(), &encoded);
            }
        }
    }

    fn get_connection_obj_for_client_sync(
        guard: &FjallInner,
        client_id: Uuid,
    ) -> Result<ByteView, Error> {
        let v = guard
            .client_connection_table
            .get(client_id.as_u128().to_le_bytes())?;

        let Some(conn_obj_bytes) = v else {
            bail!("No connection found for {client_id}")
        };

        Ok(conn_obj_bytes.into())
    }

    fn remove_from_player_connections(inner: &FjallInner, client_uuid: Uuid, client_id: u128) {
        let Ok(Some(bytes)) = inner
            .client_player_table
            .get(client_uuid.as_u128().to_le_bytes())
        else {
            return;
        };

        let Ok(player_obj) = Obj::from_bytes(bytes.as_ref()) else {
            return;
        };

        let player_oid_bytes = player_obj.as_bytes();

        let Ok(Some(bytes)) = inner.player_clients_table.get(player_oid_bytes) else {
            return;
        };

        let Ok(mut player_connections) = connections_records_from_bytes(&bytes) else {
            return;
        };

        player_connections
            .connections
            .retain(|cr| cr.client_id != client_id);

        if player_connections.connections.is_empty() {
            let _ = inner.player_clients_table.remove(player_oid_bytes);
        } else if let Ok(encoded) = connections_records_to_bytes(&player_connections) {
            let _ = inner
                .player_clients_table
                .insert(player_oid_bytes, &encoded);
        }

        let _ = inner
            .client_player_table
            .remove(client_uuid.as_u128().to_le_bytes());
    }

    fn remove_from_connection_records(inner: &FjallInner, connection_id: Obj, client_id: u128) {
        let conn_oid_bytes = connection_id.as_bytes();

        let Ok(Some(bytes)) = inner.connection_records_table.get(conn_oid_bytes) else {
            return;
        };

        let Ok(mut connections_record) = connections_records_from_bytes(&bytes) else {
            return;
        };

        connections_record
            .connections
            .retain(|cr| cr.client_id != client_id);

        if connections_record.connections.is_empty() {
            let _ = inner.connection_records_table.remove(conn_oid_bytes);
        } else if let Ok(encoded) = connections_records_to_bytes(&connections_record) {
            let _ = inner
                .connection_records_table
                .insert(conn_oid_bytes, &encoded);
        }
    }
}

impl ConnectionRegistry for FjallConnectionRegistry {
    fn associate_player_object(&self, connection_obj: Obj, player_obj: Obj) -> Result<(), Error> {
        let inner = self.inner.lock().unwrap();

        // Get connection records for this connection_obj
        let oid_bytes = connection_obj.as_bytes();
        let connections_record = match inner.connection_records_table.get(oid_bytes)? {
            Some(bytes) => connections_records_from_bytes(&bytes)?,
            None => bail!("No connection found for {:?}", connection_obj),
        };

        // Get or create player connections
        let player_oid_bytes = player_obj.as_bytes();
        let mut player_conns = match inner.player_clients_table.get(player_oid_bytes)? {
            Some(bytes) => connections_records_from_bytes(&bytes)?,
            None => ConnectionsRecords {
                connections: vec![],
            },
        };

        // Update all clients for this connection to have the player object
        for cr in &connections_record.connections {
            let client_id = Uuid::from_u128(cr.client_id);

            // Check if already associated with this player
            let already_associated = match inner
                .client_player_table
                .get(client_id.as_u128().to_le_bytes())?
            {
                Some(bytes) => {
                    let oid = Obj::from_bytes(bytes.as_ref())?;
                    oid == player_obj
                }
                None => false,
            };

            if !already_associated {
                inner
                    .client_player_table
                    .insert(client_id.as_u128().to_le_bytes(), player_obj.as_bytes())?;
            }

            // Add to player connections if not already there
            if !player_conns
                .connections
                .iter()
                .any(|c| c.client_id == cr.client_id)
            {
                player_conns.connections.push(cr.clone());
            }
        }

        // Save updated player connections
        let encoded = connections_records_to_bytes(&player_conns)?;
        inner
            .player_clients_table
            .insert(player_oid_bytes, &encoded)?;

        Ok(())
    }

    fn switch_player_for_client(&self, client_id: Uuid, new_player: Obj) -> Result<(), Error> {
        let inner = self.inner.lock().unwrap();

        // Get old player if any
        let old_player = match inner
            .client_player_table
            .get(client_id.as_u128().to_le_bytes())?
        {
            Some(bytes) => Some(Obj::from_bytes(bytes.as_ref())?),
            None => None,
        };

        // Get connection_obj for this client
        let connection_obj = match inner
            .client_connection_table
            .get(client_id.as_u128().to_le_bytes())?
        {
            Some(bytes) => Obj::from_bytes(bytes.as_ref())?,
            None => bail!("No connection found for client {:?}", client_id),
        };

        // Get the connection record
        let conn_oid_bytes = connection_obj.as_bytes();
        let connections_record = match inner.connection_records_table.get(conn_oid_bytes)? {
            Some(bytes) => connections_records_from_bytes(&bytes)?,
            None => bail!("No connection records found"),
        };

        let connection_record = connections_record
            .connections
            .iter()
            .find(|cr| cr.client_id == client_id.as_u128())
            .ok_or_else(|| eyre::eyre!("No client found"))?
            .clone();

        // Update client -> player mapping
        inner
            .client_player_table
            .insert(client_id.as_u128().to_le_bytes(), new_player.as_bytes())?;

        // Remove from old player's connections if exists
        if let Some(old_player_obj) = old_player {
            let old_player_bytes = old_player_obj.as_bytes();
            if let Some(bytes) = inner.player_clients_table.get(old_player_bytes)? {
                let mut old_conns = connections_records_from_bytes(&bytes)?;
                old_conns
                    .connections
                    .retain(|cr| cr.client_id != client_id.as_u128());

                if old_conns.connections.is_empty() {
                    inner.player_clients_table.remove(old_player_bytes)?;
                } else {
                    let encoded = connections_records_to_bytes(&old_conns)?;
                    inner
                        .player_clients_table
                        .insert(old_player_bytes, &encoded)?;
                }
            }
        }

        // Add to new player's connections
        let new_player_bytes = new_player.as_bytes();
        let mut new_conns = match inner.player_clients_table.get(new_player_bytes)? {
            Some(bytes) => connections_records_from_bytes(&bytes)?,
            None => ConnectionsRecords {
                connections: vec![],
            },
        };
        new_conns.connections.push(connection_record);
        let encoded = connections_records_to_bytes(&new_conns)?;
        inner
            .player_clients_table
            .insert(new_player_bytes, &encoded)?;

        Ok(())
    }

    fn new_connection(&self, params: NewConnectionParams) -> Result<Obj, RpcMessageError> {
        let NewConnectionParams {
            client_id,
            hostname,
            local_port,
            remote_port,
            player,
            acceptable_content_types,
            connection_attributes,
        } = params;

        let mut inner = self.inner.lock().unwrap();

        // Allocate connection ID
        let connection_id = {
            let id = inner.connection_id_sequence;
            inner.connection_id_sequence -= 1;
            inner
                .connection_id_sequence_table
                .insert(
                    "connection_id_sequence",
                    inner.connection_id_sequence.to_le_bytes(),
                )
                .map_err(|e| RpcMessageError::InternalError(e.to_string()))?;
            Obj::mk_id(id)
        };

        let now = SystemTime::now();

        // Initialize in-memory timestamp cache immediately
        {
            let mut timestamps = self.timestamps.lock().unwrap();
            timestamps.insert(
                client_id,
                ClientTimestamps {
                    last_activity: now,
                    last_ping: now,
                },
            );
        }

        // Store client -> connection mapping (synchronous - needed for immediate lookup)
        inner
            .client_connection_table
            .insert(client_id.as_u128().to_le_bytes(), connection_id.as_bytes())
            .map_err(|e| RpcMessageError::InternalError(e.to_string()))?;

        // Build connection record
        let cr = ConnectionRecord {
            client_id: client_id.as_u128(),
            connected_time: now,
            last_activity: now,
            last_ping: now,
            hostname,
            local_port,
            remote_port,
            acceptable_content_types: acceptable_content_types
                .unwrap_or_else(|| vec![Symbol::mk("text_plain")]),
            client_attributes: connection_attributes.unwrap_or_default(),
        };

        // Store connection records (synchronous - needed for immediate lookup)
        let conn_oid_bytes = connection_id.as_bytes();

        let Ok(existing) = inner.connection_records_table.get(conn_oid_bytes) else {
            return Err(RpcMessageError::InternalError(
                "Failed to read connection records".to_string(),
            ));
        };

        let mut connections_record = match existing {
            Some(bytes) => {
                let Ok(record) = connections_records_from_bytes(&bytes) else {
                    return Err(RpcMessageError::InternalError(
                        "Failed to deserialize connection records".to_string(),
                    ));
                };
                record
            }
            None => ConnectionsRecords {
                connections: vec![],
            },
        };

        connections_record.connections.push(cr.clone());

        let Ok(encoded) = connections_records_to_bytes(&connections_record) else {
            return Err(RpcMessageError::InternalError(
                "Failed to serialize connection records".to_string(),
            ));
        };

        let Ok(()) = inner
            .connection_records_table
            .insert(conn_oid_bytes, &encoded)
        else {
            return Err(RpcMessageError::InternalError(
                "Failed to write connection records".to_string(),
            ));
        };

        // Store client -> player mapping and player connections if needed (synchronous - needed for immediate lookup)
        if let Some(player_obj) = player {
            let player_oid_bytes = player_obj.as_bytes();

            let Ok(()) = inner
                .client_player_table
                .insert(client_id.as_u128().to_le_bytes(), player_oid_bytes)
            else {
                return Err(RpcMessageError::InternalError(
                    "Failed to write client-player mapping".to_string(),
                ));
            };

            let Ok(existing_conns) = inner.player_clients_table.get(player_oid_bytes) else {
                return Err(RpcMessageError::InternalError(
                    "Failed to read player connections".to_string(),
                ));
            };

            let mut player_conns = match existing_conns {
                Some(bytes) => {
                    let Ok(conns) = connections_records_from_bytes(&bytes) else {
                        return Err(RpcMessageError::InternalError(
                            "Failed to deserialize player connections".to_string(),
                        ));
                    };
                    conns
                }
                None => ConnectionsRecords {
                    connections: vec![],
                },
            };

            player_conns.connections.push(cr);

            let Ok(encoded) = connections_records_to_bytes(&player_conns) else {
                return Err(RpcMessageError::InternalError(
                    "Failed to serialize player connections".to_string(),
                ));
            };

            let Ok(()) = inner
                .player_clients_table
                .insert(player_oid_bytes, &encoded)
            else {
                return Err(RpcMessageError::InternalError(
                    "Failed to write player connections".to_string(),
                ));
            };
        }

        Ok(connection_id)
    }

    fn record_client_activity(&self, client_id: Uuid, _connobj: Obj) -> Result<(), Error> {
        let now = SystemTime::now();

        // Update in-memory timestamp cache (hot path - no DB I/O)
        {
            let mut timestamps = self.timestamps.lock().unwrap();
            timestamps
                .entry(client_id)
                .and_modify(|ts| ts.last_activity = now)
                .or_insert(ClientTimestamps {
                    last_activity: now,
                    last_ping: now,
                });
        }

        // Async write-through to database (fire and forget)
        let inner_clone = Arc::clone(&self.inner);
        std::thread::spawn(move || {
            Self::persist_timestamp_update_async(inner_clone, client_id, now, true);
        });

        Ok(())
    }

    fn notify_is_alive(&self, client_id: Uuid, _connection: Obj) -> Result<(), Error> {
        let now = SystemTime::now();

        // Update in-memory timestamp cache (hot path - no DB I/O)
        {
            let mut timestamps = self.timestamps.lock().unwrap();
            timestamps
                .entry(client_id)
                .and_modify(|ts| ts.last_ping = now)
                .or_insert(ClientTimestamps {
                    last_activity: now,
                    last_ping: now,
                });
        }

        // Async write-through to database (fire and forget)
        let inner_clone = Arc::clone(&self.inner);
        std::thread::spawn(move || {
            Self::persist_timestamp_update_async(inner_clone, client_id, now, false);
        });

        Ok(())
    }

    fn ping_check(&self) {
        let inner = match self.inner.lock() {
            Ok(inner) => inner,
            Err(_) => return,
        };

        // Check timestamps in-memory cache for stale connections
        let timestamps = self.timestamps.lock().unwrap();
        let mut to_remove = vec![];

        for (&client_uuid, ts) in timestamps.iter() {
            // Check if last_ping is too old
            match ts.last_ping.elapsed() {
                Ok(elapsed) if elapsed < CONNECTION_TIMEOUT_DURATION => continue,
                _ => {
                    // Need to find the connection_id for this client
                    if let Ok(Some(bytes)) = inner
                        .client_connection_table
                        .get(client_uuid.as_u128().to_le_bytes())
                        && let Ok(connection_id) = Obj::from_bytes(bytes.as_ref())
                    {
                        to_remove.push((connection_id, client_uuid.as_u128()));
                    }
                }
            }
        }
        drop(timestamps);

        for (connection_id, client_id) in to_remove {
            let client_uuid = Uuid::from_u128(client_id);

            // Remove client -> connection mapping
            let _ = inner
                .client_connection_table
                .remove(client_uuid.as_u128().to_le_bytes());

            // Remove from player connections if logged in
            Self::remove_from_player_connections(&inner, client_uuid, client_id);

            // Remove from connection records
            Self::remove_from_connection_records(&inner, connection_id, client_id);

            // Remove timestamp cache entry
            let _ = self.timestamps.lock().unwrap().remove(&client_uuid);
        }
    }

    fn last_activity_for(&self, connection: Obj) -> Result<SystemTime, SessionError> {
        // Get client IDs for this connection/player
        let client_ids = self.client_ids_for(connection)?;

        if client_ids.is_empty() {
            return Err(SessionError::NoConnectionForPlayer(connection));
        }

        // Get most recent last_activity from in-memory cache
        let timestamps = self.timestamps.lock().unwrap();
        client_ids
            .iter()
            .filter_map(|client_id| timestamps.get(client_id))
            .map(|ts| ts.last_activity)
            .max()
            .ok_or(SessionError::NoConnectionForPlayer(connection))
    }

    fn connection_name_for(&self, player: Obj) -> Result<String, SessionError> {
        let inner = self.inner.lock().unwrap();

        let player_oid_bytes = player.as_bytes();

        let connections_record =
            if let Ok(Some(bytes)) = inner.player_clients_table.get(player_oid_bytes) {
                connections_records_from_bytes(&bytes)
                    .map_err(|_| SessionError::NoConnectionForPlayer(player))?
            } else if let Ok(Some(bytes)) = inner.connection_records_table.get(player_oid_bytes) {
                connections_records_from_bytes(&bytes)
                    .map_err(|_| SessionError::NoConnectionForPlayer(player))?
            } else {
                return Err(SessionError::NoConnectionForPlayer(player));
            };

        let cr = connections_record
            .connections
            .first()
            .ok_or(SessionError::NoConnectionForPlayer(player))?;

        Ok(format!(
            "port {} from {}, port {}",
            cr.local_port, cr.hostname, cr.remote_port
        ))
    }

    fn connected_seconds_for(&self, player: Obj) -> Result<f64, SessionError> {
        let inner = self.inner.lock().unwrap();

        let player_oid_bytes = player.as_bytes();

        let connections_record =
            if let Ok(Some(bytes)) = inner.player_clients_table.get(player_oid_bytes) {
                connections_records_from_bytes(&bytes)
                    .map_err(|_| SessionError::NoConnectionForPlayer(player))?
            } else if let Ok(Some(bytes)) = inner.connection_records_table.get(player_oid_bytes) {
                connections_records_from_bytes(&bytes)
                    .map_err(|_| SessionError::NoConnectionForPlayer(player))?
            } else {
                return Err(SessionError::NoConnectionForPlayer(player));
            };

        let seconds: f64 = connections_record
            .connections
            .iter()
            .map(|cr| {
                cr.connected_time
                    .elapsed()
                    .unwrap_or_default()
                    .as_secs_f64()
            })
            .sum();

        Ok(seconds)
    }

    fn client_ids_for(&self, player: Obj) -> Result<Vec<Uuid>, SessionError> {
        let inner = self.inner.lock().unwrap();

        let player_oid_bytes = player.as_bytes();

        let connections_record =
            if let Ok(Some(bytes)) = inner.player_clients_table.get(player_oid_bytes) {
                connections_records_from_bytes(&bytes)
                    .map_err(|_| SessionError::NoConnectionForPlayer(player))?
            } else if let Ok(Some(bytes)) = inner.connection_records_table.get(player_oid_bytes) {
                connections_records_from_bytes(&bytes)
                    .map_err(|_| SessionError::NoConnectionForPlayer(player))?
            } else {
                return Ok(vec![]);
            };

        Ok(connections_record
            .connections
            .iter()
            .map(|cr| Uuid::from_u128(cr.client_id))
            .collect())
    }

    fn connections(&self) -> Vec<Obj> {
        let inner = match self.inner.lock() {
            Ok(inner) => inner,
            Err(_) => return vec![],
        };

        let mut connections = Vec::new();

        // Get all connection objects
        for entry in inner.connection_records_table.iter() {
            let Ok((key, _)) = entry else {
                continue;
            };
            if let Ok(oid) = Obj::from_bytes(key.as_ref()) {
                connections.push(oid);
            }
        }

        // Get all player objects
        for entry in inner.player_clients_table.iter() {
            if let Ok((key, _)) = entry
                && let Ok(oid) = Obj::from_bytes(key.as_ref())
            {
                connections.push(oid);
            }
        }

        connections.sort();
        connections.dedup();
        connections
    }

    fn connection_object_for_client(&self, client_id: Uuid) -> Option<Obj> {
        let inner = self.inner.lock().ok()?;
        let bytes = inner
            .client_connection_table
            .get(client_id.as_u128().to_le_bytes())
            .ok()??;
        Obj::from_bytes(bytes.as_ref()).ok()
    }

    fn player_object_for_client(&self, client_id: Uuid) -> Option<Obj> {
        let inner = self.inner.lock().ok()?;
        let bytes = inner
            .client_player_table
            .get(client_id.as_u128().to_le_bytes())
            .ok()??;
        Obj::from_bytes(bytes.as_ref()).ok()
    }

    fn remove_client_connection(&self, client_id: Uuid) -> Result<(), Error> {
        let inner = self.inner.lock().unwrap();

        // Get timestamps from cache before removal
        let timestamps = self.timestamps.lock().unwrap().remove(&client_id);

        // Get connection_obj and player_obj
        let connection_obj = match inner
            .client_connection_table
            .get(client_id.as_u128().to_le_bytes())?
        {
            Some(bytes) => Obj::from_bytes(bytes.as_ref())?,
            None => bail!("No connection to prune found for {:?}", client_id),
        };

        let player_obj = match inner
            .client_player_table
            .get(client_id.as_u128().to_le_bytes())?
        {
            Some(bytes) => Some(Obj::from_bytes(bytes.as_ref())?),
            None => None,
        };

        // Remove client mappings
        inner
            .client_connection_table
            .remove(client_id.as_u128().to_le_bytes())?;
        inner
            .client_player_table
            .remove(client_id.as_u128().to_le_bytes())?;

        // Remove from connection records (flush cached timestamps first)
        let conn_oid_bytes = connection_obj.as_bytes();
        if let Some(bytes) = inner.connection_records_table.get(conn_oid_bytes)? {
            let mut connections_record = connections_records_from_bytes(&bytes)?;

            // Flush cached timestamps before removing
            if let Some(ts) = &timestamps {
                for cr in &mut connections_record.connections {
                    if cr.client_id == client_id.as_u128() {
                        cr.last_activity = ts.last_activity;
                        cr.last_ping = ts.last_ping;
                        break;
                    }
                }
            }

            connections_record
                .connections
                .retain(|cr| cr.client_id != client_id.as_u128());

            if connections_record.connections.is_empty() {
                inner.connection_records_table.remove(conn_oid_bytes)?;
            } else {
                let encoded = connections_records_to_bytes(&connections_record)?;
                inner
                    .connection_records_table
                    .insert(conn_oid_bytes, &encoded)?;
            }
        }

        // Remove from player connections if logged in (flush cached timestamps first)
        if let Some(player_obj) = player_obj {
            let player_oid_bytes = player_obj.as_bytes();
            if let Some(bytes) = inner.player_clients_table.get(player_oid_bytes)? {
                let mut connections_record = connections_records_from_bytes(&bytes)?;

                // Flush cached timestamps before removing
                if let Some(ts) = &timestamps {
                    for cr in &mut connections_record.connections {
                        if cr.client_id == client_id.as_u128() {
                            cr.last_activity = ts.last_activity;
                            cr.last_ping = ts.last_ping;
                            break;
                        }
                    }
                }

                connections_record
                    .connections
                    .retain(|cr| cr.client_id != client_id.as_u128());

                if connections_record.connections.is_empty() {
                    inner.player_clients_table.remove(player_oid_bytes)?;
                } else {
                    let encoded = connections_records_to_bytes(&connections_record)?;
                    inner
                        .player_clients_table
                        .insert(player_oid_bytes, &encoded)?;
                }
            }
        }

        Ok(())
    }

    fn acceptable_content_types_for(&self, connection: Obj) -> Result<Vec<Symbol>, SessionError> {
        let inner = self.inner.lock().unwrap();

        let conn_oid_bytes = connection.as_bytes();
        let connection_records =
            if let Ok(Some(bytes)) = inner.connection_records_table.get(conn_oid_bytes) {
                connections_records_from_bytes(&bytes)
                    .map_err(|_| SessionError::NoConnectionForPlayer(connection))?
            } else if let Ok(Some(bytes)) = inner.player_clients_table.get(conn_oid_bytes) {
                connections_records_from_bytes(&bytes)
                    .map_err(|_| SessionError::NoConnectionForPlayer(connection))?
            } else {
                return Err(SessionError::NoConnectionForPlayer(connection));
            };

        Ok(connection_records
            .connections
            .first()
            .map(|cr| cr.acceptable_content_types.clone())
            .unwrap_or_else(|| vec![Symbol::mk("text_plain")]))
    }

    fn set_client_attribute(
        &self,
        client_id: Uuid,
        key: Symbol,
        value: Option<Var>,
    ) -> Result<(), RpcMessageError> {
        let inner = self.inner.lock().unwrap();

        // Get connection object
        let connection_obj = match inner
            .client_connection_table
            .get(client_id.as_u128().to_le_bytes())
            .map_err(|e| RpcMessageError::InternalError(e.to_string()))?
        {
            Some(bytes) => Obj::from_bytes(bytes.as_ref())
                .map_err(|e| RpcMessageError::InternalError(e.to_string()))?,
            None => return Err(RpcMessageError::NoConnection),
        };

        // Update in connection_records
        let conn_oid_bytes = connection_obj.as_bytes();

        if let Some(bytes) = inner
            .connection_records_table
            .get(conn_oid_bytes)
            .map_err(|e| RpcMessageError::InternalError(e.to_string()))?
        {
            let mut connection_records = connections_records_from_bytes(&bytes)
                .map_err(|e| RpcMessageError::InternalError(e.to_string()))?;

            for record in &mut connection_records.connections {
                if record.client_id == client_id.as_u128() {
                    match &value {
                        Some(val) => {
                            record.client_attributes.insert(key, val.clone());
                        }
                        None => {
                            record.client_attributes.remove(&key);
                        }
                    }
                    break;
                }
            }

            let encoded = connections_records_to_bytes(&connection_records)
                .map_err(|e| RpcMessageError::InternalError(e.to_string()))?;
            inner
                .connection_records_table
                .insert(conn_oid_bytes, &encoded)
                .map_err(|e| RpcMessageError::InternalError(e.to_string()))?;
        }

        // Also update player_records if logged in
        if let Ok(Some(bytes)) = inner
            .client_player_table
            .get(client_id.as_u128().to_le_bytes())
            && let Ok(player_obj) = Obj::from_bytes(bytes.as_ref())
            && let Ok(Some(bytes)) = inner.player_clients_table.get(player_obj.as_bytes())
            && let Ok(mut player_records) = connections_records_from_bytes(&bytes)
        {
            for record in &mut player_records.connections {
                if record.client_id == client_id.as_u128() {
                    match &value {
                        Some(val) => {
                            record.client_attributes.insert(key, val.clone());
                        }
                        None => {
                            record.client_attributes.remove(&key);
                        }
                    }
                    break;
                }
            }

            if let Ok(encoded) = connections_records_to_bytes(&player_records) {
                let _ = inner
                    .player_clients_table
                    .insert(player_obj.as_bytes(), &encoded);
            }
        }

        Ok(())
    }

    fn get_client_attributes(&self, obj: Obj) -> Result<HashMap<Symbol, Var>, SessionError> {
        let inner = self.inner.lock().unwrap();

        let oid_bytes = obj.as_bytes();

        let connection_records = if !obj.is_positive() {
            // This is a connection object - look in connection_records
            match inner.connection_records_table.get(oid_bytes) {
                Ok(Some(bytes)) => connections_records_from_bytes(&bytes)
                    .map_err(|_| SessionError::NoConnectionForPlayer(obj))?,
                _ => return Err(SessionError::NoConnectionForPlayer(obj)),
            }
        } else {
            // This is a player object - look in player_connections
            match inner.player_clients_table.get(oid_bytes) {
                Ok(Some(bytes)) => connections_records_from_bytes(&bytes)
                    .map_err(|_| SessionError::NoConnectionForPlayer(obj))?,
                _ => return Err(SessionError::NoConnectionForPlayer(obj)),
            }
        };

        Ok(connection_records
            .connections
            .first()
            .map(|cr| cr.client_attributes.clone())
            .unwrap_or_default())
    }
}
