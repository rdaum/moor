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

use std::{collections::HashMap, path::Path, sync::Mutex};

use crate::connections::{
    FIRST_CONNECTION_ID,
    persistence::{
        ClientMappingChanges, ConnectionRegistryPersistence, InitialConnectionRegistryState,
        PlayerConnectionChanges,
    },
};
use byteview::ByteView;
use eyre::Error;
use fjall::{Config, Keyspace, PartitionCreateOptions, PartitionHandle};
use moor_var::{AsByteBuffer, BINCODE_CONFIG, Obj};
use tracing::info;
use uuid::Uuid;

pub struct FjallPersistence {
    inner: Mutex<FjallInner>,
}

struct FjallInner {
    _tmpdir: Option<tempfile::TempDir>,
    _keyspace: Keyspace,
    client_player_table: PartitionHandle,
    player_clients_table: PartitionHandle,
    connection_id_sequence: i32,
    connection_id_sequence_table: PartitionHandle,
}

impl FjallPersistence {
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

        let client_player_table =
            keyspace.open_partition("client_player", PartitionCreateOptions::default())?;

        let player_clients_table =
            keyspace.open_partition("player_clients", PartitionCreateOptions::default())?;

        // Fill in the connection_id_sequence
        let connection_id_sequence = match sequences_partition.get("connection_id_sequence") {
            Ok(Some(bytes)) => i32::from_le_bytes(bytes[0..size_of::<i32>()].try_into()?),
            _ => FIRST_CONNECTION_ID,
        };

        let inner = FjallInner {
            _tmpdir: tmpdir,
            _keyspace: keyspace,
            client_player_table,
            player_clients_table,
            connection_id_sequence,
            connection_id_sequence_table: sequences_partition,
        };

        Ok(Self {
            inner: Mutex::new(inner),
        })
    }
}

impl ConnectionRegistryPersistence for FjallPersistence {
    fn load_initial_state(&self) -> Result<InitialConnectionRegistryState, Error> {
        let inner = self.inner.lock().unwrap();

        let mut client_players = HashMap::new();
        let mut player_clients = HashMap::new();

        // Load client->player mappings
        for entry in inner.client_player_table.iter() {
            let (key, value) = entry?;
            let client_id =
                Uuid::from_u128(u128::from_le_bytes(key[0..size_of::<u128>()].try_into()?));
            let oid = Obj::from_bytes(ByteView::from(value.as_ref()))?;
            client_players.insert(client_id, oid);
        }

        // Load player->connections mappings
        for entry in inner.player_clients_table.iter() {
            let (key, value) = entry?;
            let oid = Obj::from_bytes(ByteView::from(key.as_ref()))?;
            let (connections_record, _) = bincode::decode_from_slice(&value, *BINCODE_CONFIG)?;
            player_clients.insert(oid, connections_record);
        }

        Ok(InitialConnectionRegistryState {
            client_players,
            player_clients,
        })
    }

    fn persist_client_mappings(&self, changes: &ClientMappingChanges) -> Result<(), Error> {
        let inner = self.inner.lock().unwrap();

        // Handle updates
        for (&client_id, &player_obj) in &changes.updates {
            let oid_bytes = player_obj.as_bytes()?;
            inner
                .client_player_table
                .insert(client_id.as_u128().to_le_bytes(), oid_bytes)?;
        }

        // Handle removals
        for &client_id in &changes.removals {
            inner
                .client_player_table
                .remove(client_id.as_u128().to_le_bytes())?;
        }

        Ok(())
    }

    fn persist_player_connections(&self, changes: &PlayerConnectionChanges) -> Result<(), Error> {
        let inner = self.inner.lock().unwrap();

        // Handle updates
        for (&player_obj, connections_record) in &changes.updates {
            let oid_bytes = player_obj.as_bytes()?;
            let encoded = bincode::encode_to_vec(connections_record, *BINCODE_CONFIG)?;
            inner.player_clients_table.insert(oid_bytes, &encoded)?;
        }

        // Handle removals
        for &player_obj in &changes.removals {
            let oid_bytes = player_obj.as_bytes()?;
            inner.player_clients_table.remove(oid_bytes)?;
        }

        Ok(())
    }

    fn next_connection_sequence(&self) -> Result<i32, Error> {
        let mut inner = self.inner.lock().unwrap();

        let id = inner.connection_id_sequence;
        inner.connection_id_sequence -= 1;

        inner.connection_id_sequence_table.insert(
            "connection_id_sequence",
            inner.connection_id_sequence.to_le_bytes(),
        )?;

        Ok(id)
    }
}
