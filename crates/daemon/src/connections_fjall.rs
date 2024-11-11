// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

//! An implementation of the connections db that uses relbox.

use std::collections::HashSet;
use std::fmt::{Debug, Display, Formatter};
use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, SystemTime};

use eyre::Error;
use moor_db::{RelationalError, RelationalTransaction, StringHolder, SystemTimeHolder};
use strum::{AsRefStr, Display, EnumCount, EnumIter, EnumProperty};
use tracing::{error, warn};
use uuid::Uuid;

use bytes::Bytes;
use moor_db_fjall::{FjallDb, FjallTransaction};
use moor_kernel::tasks::sessions::SessionError;
use moor_values::model::{CommitResult, ValSet};
use moor_values::Objid;
use moor_values::{AsByteBuffer, DecodingError, EncodingError};
use rpc_common::RpcMessageError;

use crate::connections::{ConnectionsDB, CONNECTION_TIMEOUT_DURATION};
use crate::connections_fjall::ConnectionRelation::{
    ClientActivity, ClientConnectTime, ClientConnection, ClientName, ClientPingTime,
};
use crate::connections_fjall::Sequences::ConnectionId;

#[repr(usize)]
// Don't warn about same-prefix, "I did that on purpose"
#[allow(clippy::enum_variant_names)]
#[derive(
    Copy, Clone, Debug, Eq, PartialEq, EnumIter, EnumCount, Display, EnumProperty, AsRefStr,
)]
enum ConnectionRelation {
    // One to many, client id <-> connection/player object. Secondary index will seek on object id.
    #[strum(props(SecondaryIndexed = "true",))]
    ClientConnection = 0,
    /// Client -> SystemTime of last activity
    ClientActivity = 1,
    /// Client connect time.
    ClientConnectTime = 2,
    /// Client last ping time.
    ClientPingTime = 3,
    /// Client hostname / connection "name"
    ClientName = 4,
}

#[repr(u8)]
enum Sequences {
    ConnectionId = 0,
}

impl From<Sequences> for u8 {
    fn from(val: Sequences) -> Self {
        val as u8
    }
}
impl From<ConnectionRelation> for usize {
    fn from(val: ConnectionRelation) -> Self {
        val as usize
    }
}

pub struct ConnectionsFjall {
    db: FjallDb<ConnectionRelation>,
}

impl ConnectionsFjall {
    pub fn new(path: Option<PathBuf>) -> Self {
        let (db, _) = FjallDb::open(path.as_deref());

        Self { db }
    }
}

impl ConnectionsFjall {
    fn most_recent_client_connection(
        tx: &FjallTransaction<ConnectionRelation>,
        connection_obj: Objid,
    ) -> Result<Vec<(ClientId, SystemTime)>, RelationalError> {
        let clients: ClientSet =
            tx.seek_by_codomain::<ClientId, Objid, ClientSet>(ClientConnection, connection_obj)?;

        // Seek the most recent activity for the connection, so pull in the activity relation for
        // each client.
        let mut times = Vec::new();
        for client in clients.iter() {
            if let Some(last_activity) =
                tx.seek_unique_by_domain::<ClientId, SystemTimeHolder>(ClientActivity, client)?
            {
                times.push((client, last_activity.0));
            } else {
                warn!(
                    ?client,
                    ?connection_obj,
                    "Unable to find last activity for client"
                );
            }
        }
        times.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap());
        Ok(times)
    }
}

fn retry_tx_action<
    R,
    F: FnMut(&FjallTransaction<ConnectionRelation>) -> Result<R, RelationalError>,
>(
    db: &FjallDb<ConnectionRelation>,
    mut f: F,
) -> Result<R, RelationalError> {
    for _try_num in 0..50 {
        let tx = db.new_transaction();
        let r = f(&tx);

        let r = match r {
            Ok(r) => r,
            Err(RelationalError::ConflictRetry) => {
                error!("Conflict in transaction, retrying");
                tx.rollback();
                sleep(Duration::from_millis(100));
                continue;
            }
            Err(e) => {
                error!(?e, "Non-rollback error in transaction");
                return Err(e);
            }
        };
        // Commit the transaction.
        if let CommitResult::Success = tx.commit() {
            return Ok(r);
        }
        sleep(Duration::from_millis(100))
    }
    panic!("Unable to commit transaction after 50 tries");
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
struct ClientId(Uuid);

impl AsByteBuffer for ClientId {
    fn size_bytes(&self) -> usize {
        16
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> Result<R, EncodingError> {
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(self.0.as_bytes());
        Ok(f(&bytes))
    }

    fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError> {
        Ok(self.0.as_bytes().to_vec())
    }

    fn from_bytes(bytes: Bytes) -> Result<Self, DecodingError>
    where
        Self: Sized,
    {
        let bytes = bytes.as_ref();
        assert_eq!(bytes.len(), 16, "Decode client id: Invalid UUID length");
        let mut uuid_bytes = [0u8; 16];
        uuid_bytes.copy_from_slice(bytes);
        Ok(ClientId(Uuid::from_bytes(uuid_bytes)))
    }

    fn as_bytes(&self) -> Result<Bytes, EncodingError> {
        let buf = self.0.as_bytes();
        assert_eq!(buf.len(), 16, "Encode client id: Invalid UUID length");
        Ok(Bytes::copy_from_slice(buf))
    }
}
impl Display for ClientId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "ClientId({})", self.0)
    }
}

#[derive(Debug)]
struct ClientSet(Vec<ClientId>);
impl ValSet<ClientId> for ClientSet {
    fn empty() -> Self {
        Self(Vec::new())
    }

    fn from_items(items: &[ClientId]) -> Self {
        Self(items.to_vec())
    }

    fn iter(&self) -> impl Iterator<Item = ClientId> {
        self.0.iter().cloned()
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl FromIterator<ClientId> for ClientSet {
    fn from_iter<T: IntoIterator<Item = ClientId>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl ConnectionsDB for ConnectionsFjall {
    fn update_client_connection(
        &self,
        from_connection: Objid,
        to_player: Objid,
    ) -> Result<(), Error> {
        Ok(retry_tx_action(&self.db, |tx| {
            let client_ids = tx.seek_by_codomain::<ClientId, Objid, ClientSet>(
                ClientConnection,
                from_connection,
            )?;
            if client_ids.is_empty() {
                error!(?from_connection, ?to_player, "No client ids for connection");
                return Err(RelationalError::NotFound);
            }
            // TODO use join once it's implemented
            for client_id in client_ids.iter() {
                tx.upsert(ClientConnection, client_id, to_player)?;
            }
            Ok(())
        })?)
    }

    fn new_connection(
        &self,
        client_id: Uuid,
        hostname: String,
        player: Option<Objid>,
    ) -> Result<Objid, RpcMessageError> {
        retry_tx_action(&self.db, |tx| {
            let connection_oid = match player {
                None => {
                    // The connection object is pulled from the sequence, then we invert it and subtract from
                    // -4 to get the connection object, since they always grow downwards from there.
                    let connection_id = tx.increment_sequence(ConnectionId);
                    let connection_id: i64 = -4 - connection_id;
                    Objid(connection_id)
                }
                Some(player) => player,
            };

            // Insert the initial tuples for the connection.
            let client_id = ClientId(client_id);
            let now = SystemTimeHolder(SystemTime::now());
            tx.insert_tuple(ClientConnection, client_id, connection_oid)?;
            tx.insert_tuple(ClientActivity, client_id, now.clone())?;
            tx.insert_tuple(ClientConnectTime, client_id, now.clone())?;
            tx.insert_tuple(ClientPingTime, client_id, now)?;
            tx.insert_tuple(ClientName, client_id, StringHolder(hostname.clone()))?;

            Ok(connection_oid)
        })
        .map_err(|e| RpcMessageError::InternalError(e.to_string()))
    }

    fn record_client_activity(&self, client_id: Uuid, _connobj: Objid) -> Result<(), Error> {
        Ok(retry_tx_action(&self.db, |tx| {
            let client_id = ClientId(client_id);
            tx.upsert(
                ClientActivity,
                client_id,
                SystemTimeHolder(SystemTime::now()),
            )?;
            Ok(())
        })?)
    }

    fn notify_is_alive(&self, client_id: Uuid, _connection: Objid) -> Result<(), Error> {
        Ok(retry_tx_action(&self.db, |tx| {
            let client_id = ClientId(client_id);
            tx.upsert(
                ClientPingTime,
                client_id,
                SystemTimeHolder(SystemTime::now()),
            )?;
            Ok(())
        })?)
    }

    fn ping_check(&self) {
        let now = SystemTime::now();
        let timeout_threshold = now - CONNECTION_TIMEOUT_DURATION;

        retry_tx_action::<(), _>(&self.db, |tx| {
            // Full scan the last ping relation, and compare the last ping time to the current time.
            // If the difference is greater than the timeout duration, then we need to remove the
            // connection from all the relations.

            let expired = tx.scan_with_predicate::<_, ClientId, SystemTimeHolder>(
                ClientPingTime,
                |_, ping| ping.0 < timeout_threshold,
            )?;

            for expired_ping in expired.iter() {
                let client_id = expired_ping.0;
                tx.remove_by_domain(ClientConnection, client_id)?;
                tx.remove_by_domain(ClientActivity, client_id)?;
                tx.remove_by_domain(ClientConnectTime, client_id)?;
                tx.remove_by_domain(ClientPingTime, client_id)?;
                tx.remove_by_domain(ClientName, client_id)?;
            }
            Ok::<(), RelationalError>(())
        })
        .expect("Unable to commit transaction");
    }

    fn last_activity_for(&self, connection_obj: Objid) -> Result<SystemTime, SessionError> {
        let result = retry_tx_action(&self.db, |tx| {
            let mut client_times = Self::most_recent_client_connection(tx, connection_obj)?;
            let Some(time) = client_times.pop() else {
                return Err(RelationalError::NotFound);
            };
            Ok(time.1)
        });
        match result {
            Ok(time) => Ok(time),
            Err(RelationalError::NotFound) => {
                Err(SessionError::NoConnectionForPlayer(connection_obj))
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    fn connection_name_for(&self, connection_obj: Objid) -> Result<String, SessionError> {
        let result = retry_tx_action(&self.db, |tx| {
            let mut client_times = Self::most_recent_client_connection(tx, connection_obj)?;
            let Some(most_recent) = client_times.pop() else {
                return Err(RelationalError::NotFound);
            };
            let client_id = most_recent.0;
            let Some(name) =
                tx.seek_unique_by_domain::<ClientId, StringHolder>(ClientName, client_id)?
            else {
                return Err(RelationalError::NotFound);
            };
            Ok(name)
        });
        match result {
            Ok(name) => Ok(name.0),
            Err(RelationalError::NotFound) => {
                Err(SessionError::NoConnectionForPlayer(connection_obj))
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    fn connected_seconds_for(&self, player: Objid) -> Result<f64, SessionError> {
        retry_tx_action(&self.db, |tx| {
            // In this case we need to find the earliest connection time for the player, and then
            // subtract that from the current time.
            let clients =
                tx.seek_by_codomain::<ClientId, Objid, ClientSet>(ClientConnection, player)?;
            if clients.is_empty() {
                return Err(RelationalError::NotFound);
            }

            let mut times: Vec<(ClientId, SystemTime)> = vec![];
            for client in clients.iter() {
                if let Some(connect_time) =
                    tx.seek_unique_by_domain::<_, SystemTimeHolder>(ClientConnectTime, client)?
                {
                    {
                        times.push((client, connect_time.0));
                    }
                }
            }

            times.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap());
            let earliest = times.pop().expect("No connection for player");
            let earliest = earliest.1;
            let now = SystemTime::now();
            let duration = now.duration_since(earliest).expect("Invalid duration");
            Ok(duration.as_secs_f64())
        })
        .map_err(|e| match e {
            RelationalError::NotFound => SessionError::NoConnectionForPlayer(player),
            _ => panic!("Unexpected error: {:?}", e),
        })
    }

    fn client_ids_for(&self, player: Objid) -> Result<Vec<Uuid>, SessionError> {
        retry_tx_action(&self.db, |tx| {
            let clients =
                tx.seek_by_codomain::<ClientId, Objid, ClientSet>(ClientConnection, player)?;
            Ok(clients.iter().map(|c| c.0).collect())
        })
        .map_err(|e| match e {
            RelationalError::NotFound => SessionError::NoConnectionForPlayer(player),
            _ => panic!("Unexpected error: {:?}", e),
        })
    }

    fn connections(&self) -> Vec<Objid> {
        // Full scan from ClientConnection relation to get all connections, and dump them into a
        // hashset (to remove dupes) and return as a vector.
        retry_tx_action(&self.db, |tx| {
            let mut connections = HashSet::new();
            let clients =
                tx.scan_with_predicate::<_, ClientId, Objid>(ClientConnection, |_, _| true)?;

            for entry in clients.iter() {
                let oid = entry.1;
                connections.insert(oid);
            }
            Ok::<Vec<Objid>, RelationalError>(connections.into_iter().collect())
        })
        .expect("Unable to commit transaction")
    }

    fn connection_object_for_client(&self, client_id: Uuid) -> Option<Objid> {
        retry_tx_action(&self.db, |tx| {
            tx.seek_unique_by_domain(ClientConnection, ClientId(client_id))
        })
        .unwrap()
    }

    fn remove_client_connection(&self, client_id: Uuid) -> Result<(), Error> {
        Ok(retry_tx_action(&self.db, |tx| {
            tx.remove_by_domain(ClientConnection, ClientId(client_id))?;
            tx.remove_by_domain(ClientActivity, ClientId(client_id))?;
            tx.remove_by_domain(ClientConnectTime, ClientId(client_id))?;
            tx.remove_by_domain(ClientPingTime, ClientId(client_id))?;
            tx.remove_by_domain(ClientName, ClientId(client_id))?;
            Ok(())
        })?)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use moor_values::Objid;

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
        let db = Arc::new(ConnectionsFjall::new(None));
        let mut jh = vec![];

        for x in 1..10 {
            let db = db.clone();
            jh.push(std::thread::spawn(move || {
                let client_id = uuid::Uuid::new_v4();
                let oid = db
                    .new_connection(client_id, "localhost".to_string(), None)
                    .unwrap();
                let client_ids = db.client_ids_for(oid).unwrap();
                assert_eq!(client_ids.len(), 1);
                assert_eq!(client_ids[0], client_id);
                db.record_client_activity(client_id, oid).unwrap();
                db.notify_is_alive(client_id, oid).unwrap();
                let last_activity = db.last_activity_for(oid);
                assert!(
                    last_activity.is_ok(),
                    "Unable to get last activity for {x} ({oid}) client {client_id}",
                );
                let last_activity = last_activity.unwrap().elapsed().unwrap().as_secs_f64();
                assert!(last_activity < 1.0);
                assert_eq!(db.connection_object_for_client(client_id), Some(oid));
                let connection_object = Objid(x);
                db.update_client_connection(oid, connection_object)
                    .unwrap_or_else(|e| {
                        panic!("Unable to update client connection for {:?}: {:?}", x, e)
                    });
                let client_ids = db.client_ids_for(connection_object).unwrap();
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

    /// Test that a given player can have multiple clients connected to it.
    #[test]
    fn test_multiple_connections() {
        let db = Arc::new(ConnectionsFjall::new(None));
        let mut jh = vec![];
        for x in 1..50 {
            let db = db.clone();
            jh.push(std::thread::spawn(move || {
                let client_id1 = uuid::Uuid::new_v4();
                let client_id2 = uuid::Uuid::new_v4();
                let con_oid1 = db
                    .new_connection(client_id1, "localhost".to_string(), None)
                    .unwrap();
                let con_oid2 = db
                    .new_connection(client_id2, "localhost".to_string(), None)
                    .unwrap();
                let new_conn = Objid(x);
                db.update_client_connection(con_oid1, new_conn)
                    .expect("Unable to update client connection");
                let client_ids = db.client_ids_for(new_conn).unwrap();
                assert_eq!(client_ids.len(), 1);
                assert!(client_ids.contains(&client_id1));

                db.update_client_connection(con_oid2, new_conn)
                    .expect("Unable to update client connection");
                let client_ids = db.client_ids_for(new_conn).unwrap();
                assert_eq!(
                    client_ids.len(),
                    2,
                    "Client ids: {:?}, should be ({client_id1}, {client_id2}) in {x}th oid",
                    client_ids
                );
                assert!(client_ids.contains(&client_id2));

                db.record_client_activity(client_id1, new_conn).unwrap();
                let last_activity = db
                    .last_activity_for(new_conn)
                    .unwrap()
                    .elapsed()
                    .unwrap()
                    .as_secs_f64();
                assert!(last_activity < 1.0);
                db.remove_client_connection(client_id1).unwrap();
                let client_ids = db.client_ids_for(new_conn).unwrap();
                assert_eq!(client_ids.len(), 1);
                assert!(client_ids.contains(&client_id2));
            }));
        }
        for j in jh {
            j.join().unwrap();
        }
    }

    // Validate that ping check works.
    #[test]
    fn ping_test() {
        let db = Arc::new(ConnectionsFjall::new(None));
        let client_id1 = uuid::Uuid::new_v4();
        let ob = db
            .new_connection(client_id1, "localhost".to_string(), None)
            .unwrap();
        db.ping_check();
        let client_ids = db.connections();
        assert_eq!(client_ids.len(), 1);
        assert_eq!(db.connection_object_for_client(client_id1), Some(ob));

        let client_ids = db.client_ids_for(ob).unwrap();
        assert_eq!(client_ids.len(), 1);
        assert_eq!(client_ids[0], client_id1);
    }
}
