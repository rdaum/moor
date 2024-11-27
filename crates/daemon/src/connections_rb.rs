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
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use eyre::Error;
use strum::{AsRefStr, Display, EnumCount, EnumIter, EnumProperty, IntoEnumIterator};
use tracing::{error, warn};
use uuid::Uuid;

use bytes::Bytes;
use daumtils::SliceRef;
use moor_kernel::tasks::sessions::SessionError;
use moor_values::AsByteBuffer;
use moor_values::Objid;
use relbox::{relation_info_for, RelBox, RelationId, RelationInfo, Transaction};
use rpc_common::RpcMessageError;

use crate::connections::{ConnectionsDB, CONNECTION_TIMEOUT_DURATION};

#[repr(usize)]
// Don't warn about same-prefix, "I did that on purpose"
#[allow(clippy::enum_variant_names)]
#[derive(
    Copy, Clone, Debug, Eq, PartialEq, EnumIter, EnumCount, Display, EnumProperty, AsRefStr,
)]
enum ConnectionRelation {
    // One to many, client id <-> connection/player object. Secondary index will seek on object id.
    #[strum(props(
        DomainType = "Integer",
        CodomainType = "Bytes",
        SecondaryIndexed = "true",
        IndexType = "Hash"
    ))]
    ClientConnection = 0,
    // Client -> SystemTime of last activity
    #[strum(props(
        DomainType = "Bytes",
        CodomainType = "Bytes",
        SecondaryIndexed = "false",
        IndexType = "Hash"
    ))]
    ClientActivity = 1,
    // Client connect time.
    #[strum(props(
        DomainType = "Bytes",
        CodomainType = "Bytes",
        SecondaryIndexed = "false",
        IndexType = "Hash"
    ))]
    ClientConnectTime = 2,
    // Client last ping time.
    #[strum(props(
        DomainType = "Bytes",
        CodomainType = "Bytes",
        SecondaryIndexed = "false",
        IndexType = "Hash"
    ))]
    ClientPingTime = 3,
    // Client hostname / connection "name"
    #[strum(props(
        DomainType = "Bytes",
        CodomainType = "Bytes",
        SecondaryIndexed = "false",
        IndexType = "Hash"
    ))]
    ClientName = 4,
}

const CONNECTIONS_DB_MEM_SIZE: usize = 1 << 26;
pub struct ConnectionsRb {
    tb: Arc<RelBox>,
}

impl ConnectionsRb {
    pub fn new(path: Option<PathBuf>) -> Self {
        let mut relations: Vec<RelationInfo> =
            ConnectionRelation::iter().map(relation_info_for).collect();
        relations[ConnectionRelation::ClientConnection as usize].secondary_indexed = true;

        let tb = RelBox::new(CONNECTIONS_DB_MEM_SIZE, path, &relations, 1);
        Self { tb }
    }
}

impl ConnectionsRb {
    fn most_recent_client_connection(
        tx: &Transaction,
        connection_obj: Objid,
    ) -> Result<Vec<(SliceRef, SystemTime)>, SessionError> {
        let clients = tx
            .relation(RelationId(ConnectionRelation::ClientConnection as usize))
            .seek_by_codomain(SliceRef::from_byte_source(
                connection_obj
                    .as_bytes()
                    .expect("Invalid connection object"),
            ))
            .expect("Unable to seek client connection");

        // Seek the most recent activity for the connection, so pull in the activity relation for
        // each client.
        let mut times = Vec::new();
        for client in &clients {
            if let Ok(last_activity) = tx
                .relation(RelationId(ConnectionRelation::ClientActivity as usize))
                .seek_unique_by_domain(client.domain())
            {
                let epoch_time_millis: u128 =
                    u128::from_le_bytes(last_activity.codomain().as_slice().try_into().unwrap());
                let time = SystemTime::UNIX_EPOCH + Duration::from_millis(epoch_time_millis as u64);
                times.push((client.domain(), time));
            } else {
                warn!(
                    client = ?client.domain(),
                    ?connection_obj,
                    "Unable to find last activity for client"
                );
            }
        }
        times.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap());
        Ok(times)
    }
}

fn bytes_as_time(slc: SliceRef) -> SystemTime {
    let epoch_time_millis: u128 = u128::from_le_bytes(slc.as_slice().try_into().unwrap());
    SystemTime::UNIX_EPOCH + Duration::from_millis(epoch_time_millis as u64)
}

fn now_as_sliceref() -> SliceRef {
    SliceRef::from_bytes(
        &SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            .to_le_bytes(),
    )
}

impl ConnectionsDB for ConnectionsRb {
    fn update_client_connection(
        &self,
        from_connection: Objid,
        to_player: Objid,
    ) -> Result<(), Error> {
        let tx = self.tb.clone().start_tx();
        let client_ids = tx
            .relation(RelationId(ConnectionRelation::ClientConnection as usize))
            .seek_by_codomain(SliceRef::from_byte_source(
                from_connection
                    .as_bytes()
                    .expect("Invalid connection object"),
            ))
            .expect("Unable to seek client connection");
        if client_ids.is_empty() {
            error!(?from_connection, ?to_player, "No client ids for connection");
            return Err(Error::msg("No client ids for connection"));
        }
        for client_id in client_ids {
            let _ = tx
                .relation(RelationId(ConnectionRelation::ClientConnection as usize))
                .update_by_domain(
                    client_id.domain().clone(),
                    SliceRef::from_byte_source(
                        to_player.as_bytes().expect("Invalid player object"),
                    ),
                );
        }
        tx.commit()?;
        Ok(())
    }

    fn new_connection(
        &self,
        client_id: Uuid,
        hostname: String,
        player: Option<Objid>,
    ) -> Result<Objid, RpcMessageError> {
        let connection_oid = match player {
            None => {
                // The connection object is pulled from the sequence, then we invert it and subtract from
                // -4 to get the connection object, since they always grow downwards from there.
                let connection_id = self.tb.clone().increment_sequence(0);
                let connection_id: i64 = -4 - (connection_id as i64);
                Objid::mk_id(connection_id)
            }
            Some(player) => player,
        };

        // Insert the initial tuples for the connection.
        let tx = self.tb.clone().start_tx();
        let client_id = SliceRef::from_bytes(client_id.as_bytes());
        tx.relation(RelationId(ConnectionRelation::ClientConnection as usize))
            .insert_tuple(
                client_id.clone(),
                SliceRef::from_bytes(
                    connection_oid
                        .as_bytes()
                        .expect("Invalid connection object")
                        .as_ref(),
                ),
            )
            .expect("Unable to insert client connection");
        tx.relation(RelationId(ConnectionRelation::ClientActivity as usize))
            .insert_tuple(client_id.clone(), now_as_sliceref())
            .expect("Unable to insert client activity");
        tx.relation(RelationId(ConnectionRelation::ClientConnectTime as usize))
            .insert_tuple(client_id.clone(), now_as_sliceref())
            .expect("Unable to insert client connect time");
        tx.relation(RelationId(ConnectionRelation::ClientPingTime as usize))
            .insert_tuple(client_id.clone(), now_as_sliceref())
            .expect("Unable to insert client ping time");
        tx.relation(RelationId(ConnectionRelation::ClientName as usize))
            .insert_tuple(client_id.clone(), SliceRef::from_bytes(hostname.as_bytes()))
            .expect("Unable to insert client name");

        tx.commit().expect("Unable to commit transaction");

        Ok(connection_oid)
    }

    fn record_client_activity(&self, client_id: Uuid, _connobj: Objid) -> Result<(), Error> {
        let tx = self.tb.clone().start_tx();
        tx.relation(RelationId(ConnectionRelation::ClientActivity as usize))
            .upsert_by_domain(
                SliceRef::from_bytes(client_id.as_bytes()),
                now_as_sliceref(),
            )
            .expect("Unable to update client activity");
        tx.commit()?;
        Ok(())
    }

    fn notify_is_alive(&self, client_id: Uuid, _connection: Objid) -> Result<(), Error> {
        let tx = self.tb.clone().start_tx();
        tx.relation(RelationId(ConnectionRelation::ClientPingTime as usize))
            .upsert_by_domain(
                SliceRef::from_bytes(client_id.as_bytes()),
                now_as_sliceref(),
            )
            .expect("Unable to update client ping time");
        tx.commit()?;
        Ok(())
    }

    fn ping_check(&self) {
        let now = SystemTime::now();
        let timeout_threshold = now - CONNECTION_TIMEOUT_DURATION;

        // Full scan the last ping relation, and compare the last ping time to the current time.
        // If the difference is greater than the timeout duration, then we need to remove the
        // connection from all the relations.
        let tx = self.tb.clone().start_tx();

        let last_ping_relation =
            tx.relation(RelationId(ConnectionRelation::ClientPingTime as usize));
        let expired = last_ping_relation
            .predicate_scan(&|ping| {
                let last_ping_time = bytes_as_time(ping.codomain());
                last_ping_time < timeout_threshold
            })
            .expect("Unable to scan last ping relation");

        for expired_ping in expired {
            let client_id = expired_ping.domain().clone();
            let _ = tx
                .relation(RelationId(ConnectionRelation::ClientConnection as usize))
                .remove_by_domain(client_id.clone());
            let _ = tx
                .relation(RelationId(ConnectionRelation::ClientActivity as usize))
                .remove_by_domain(client_id.clone());
            let _ = tx
                .relation(RelationId(ConnectionRelation::ClientConnectTime as usize))
                .remove_by_domain(client_id.clone());
            let _ = tx
                .relation(RelationId(ConnectionRelation::ClientPingTime as usize))
                .remove_by_domain(client_id.clone());
            let _ = tx
                .relation(RelationId(ConnectionRelation::ClientName as usize))
                .remove_by_domain(client_id.clone());
        }
        tx.commit().expect("Unable to commit transaction");
    }

    fn last_activity_for(&self, connection_obj: Objid) -> Result<SystemTime, SessionError> {
        let tx = self.tb.clone().start_tx();
        let mut client_times = Self::most_recent_client_connection(&tx, connection_obj.clone())?;

        // Most recent time is the last one.
        let Some(time) = client_times.pop() else {
            return Err(SessionError::NoConnectionForPlayer(connection_obj));
        };
        tx.commit().expect("Unable to commit transaction");
        Ok(time.1)
    }

    fn connection_name_for(&self, connection_obj: Objid) -> Result<String, SessionError> {
        let tx = self.tb.clone().start_tx();
        let mut client_times = Self::most_recent_client_connection(&tx, connection_obj.clone())?;

        let Some(most_recent) = client_times.pop() else {
            return Err(SessionError::NoConnectionForPlayer(connection_obj));
        };

        let client_id = most_recent.0;
        let name = tx
            .relation(RelationId(ConnectionRelation::ClientName as usize))
            .seek_unique_by_domain(client_id.clone())
            .expect("Unable to seek client name");
        tx.commit().expect("Unable to commit transaction");
        Ok(String::from_utf8(name.codomain().as_slice().to_vec())
            .expect("Invalid UTF-8 in client name"))
    }

    fn connected_seconds_for(&self, player: Objid) -> Result<f64, SessionError> {
        let tx = self.tb.clone().start_tx();
        // In this case we need to find the earliest connection time for the player, and then
        // subtract that from the current time.
        let Ok(clients) = tx
            .relation(RelationId(ConnectionRelation::ClientConnection as usize))
            .seek_by_codomain(SliceRef::from_byte_source(
                player.as_bytes().expect("Invalid player object"),
            ))
        else {
            return Err(SessionError::NoConnectionForPlayer(player));
        };

        let mut times = Vec::new();
        for client in clients {
            if let Ok(connect_time) = tx
                .relation(RelationId(ConnectionRelation::ClientConnectTime as usize))
                .seek_unique_by_domain(client.domain())
            {
                let time = bytes_as_time(connect_time.codomain());
                times.push((client.domain(), time));
            }
        }
        times.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap());
        let earliest = times.pop().expect("No connection for player");
        let earliest = earliest.1;
        let now = SystemTime::now();
        let duration = now.duration_since(earliest).expect("Invalid duration");
        let seconds = duration.as_secs_f64();
        tx.commit().expect("Unable to commit transaction");
        Ok(seconds)
    }

    fn client_ids_for(&self, player: Objid) -> Result<Vec<Uuid>, SessionError> {
        let tx = self.tb.clone().start_tx();
        let Ok(clients) = tx
            .relation(RelationId(ConnectionRelation::ClientConnection as usize))
            .seek_by_codomain(SliceRef::from_byte_source(
                player.as_bytes().expect("Invalid player object"),
            ))
        else {
            return Ok(vec![]);
        };

        let mut client_ids = Vec::new();
        for client in clients {
            let client_id = Uuid::from_slice(client.domain().as_slice()).expect("Invalid UUID");
            client_ids.push(client_id);
        }
        tx.commit().expect("Unable to commit transaction");
        Ok(client_ids)
    }

    fn connections(&self) -> Vec<Objid> {
        // Full scan from ClientConnection relation to get all connections, and dump them into a
        // hashset (to remove dupes) and return as a vector.
        let tx = self.tb.clone().start_tx();
        let mut connections = HashSet::new();
        let clients = tx
            .relation(RelationId(ConnectionRelation::ClientConnection as usize))
            .predicate_scan(&|_| true)
            .expect("Unable to scan client connection relation");

        for client in clients {
            let connection = Objid::from_bytes(Bytes::from(client.codomain().as_slice().to_vec()))
                .expect("Invalid connection");
            connections.insert(connection);
        }

        tx.commit().expect("Unable to commit transaction");
        connections.into_iter().collect()
    }

    fn connection_object_for_client(&self, client_id: Uuid) -> Option<Objid> {
        let tx = self.tb.clone().start_tx();
        let connection = match tx
            .relation(RelationId(ConnectionRelation::ClientConnection as usize))
            .seek_unique_by_domain(SliceRef::from_bytes(&client_id.as_bytes()[..]))
        {
            Ok(connection) => {
                let bytes = Bytes::from(connection.codomain().as_slice().to_vec());
                Some(Objid::from_bytes(bytes).expect("Invalid connection"))
            }
            Err(_) => None,
        };
        tx.commit().expect("Unable to commit transaction");
        connection
    }

    fn remove_client_connection(&self, client_id: Uuid) -> Result<(), Error> {
        let tx = self.tb.clone().start_tx();
        let _ = tx
            .relation(RelationId(ConnectionRelation::ClientConnection as usize))
            .remove_by_domain(SliceRef::from_bytes(&client_id.as_bytes()[..]));
        let _ = tx
            .relation(RelationId(ConnectionRelation::ClientActivity as usize))
            .remove_by_domain(SliceRef::from_bytes(client_id.as_bytes()));
        let _ = tx
            .relation(RelationId(ConnectionRelation::ClientConnectTime as usize))
            .remove_by_domain(SliceRef::from_bytes(client_id.as_bytes()));
        let _ = tx
            .relation(RelationId(ConnectionRelation::ClientPingTime as usize))
            .remove_by_domain(SliceRef::from_bytes(client_id.as_bytes()));
        let _ = tx
            .relation(RelationId(ConnectionRelation::ClientName as usize))
            .remove_by_domain(SliceRef::from_bytes(client_id.as_bytes()));

        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use moor_values::Objid;

    use crate::connections::ConnectionsDB;
    use crate::connections_rb::ConnectionsRb;

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
        let db = Arc::new(ConnectionsRb::new(None));
        let mut jh = vec![];

        for x in 1..100 {
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
                    "Unable to get last activity for {x}th oid ({oid}) client {client_id}",
                );
                let last_activity = last_activity.unwrap().elapsed().unwrap().as_secs_f64();
                assert!(last_activity < 1.0);
                db.update_client_connection(oid, Objid::mk_id(x))
                    .expect("Unable to update client connection");
                let client_ids = db.client_ids_for(Objid::mk_id(x)).unwrap();
                assert_eq!(client_ids.len(), 1);
                assert_eq!(client_ids[0], client_id);
                db.remove_client_connection(client_id).unwrap();
                let client_ids = db.client_ids_for(Objid::mk_id(x)).unwrap();
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
        let db = Arc::new(ConnectionsRb::new(None));
        let mut jh = vec![];
        for x in 1..100 {
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
                db.update_client_connection(con_oid1, Objid::mk_id(x))
                    .expect("Unable to update client connection");
                let client_ids = db.client_ids_for(Objid::mk_id(x)).unwrap();
                assert_eq!(client_ids.len(), 1);
                assert!(client_ids.contains(&client_id1));

                db.update_client_connection(con_oid2, Objid::mk_id(x))
                    .expect("Unable to update client connection");
                let client_ids = db.client_ids_for(Objid::mk_id(x)).unwrap();
                assert_eq!(
                    client_ids.len(),
                    2,
                    "Client ids: {:?}, should be ({client_id1}, {client_id2}) in {x}th oid",
                    client_ids
                );
                assert!(client_ids.contains(&client_id2));

                db.record_client_activity(client_id1, Objid::mk_id(x))
                    .unwrap();
                let last_activity = db
                    .last_activity_for(Objid::mk_id(x))
                    .unwrap()
                    .elapsed()
                    .unwrap()
                    .as_secs_f64();
                assert!(last_activity < 1.0);
                db.remove_client_connection(client_id1).unwrap();
                let client_ids = db.client_ids_for(Objid::mk_id(x)).unwrap();
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
        let db = Arc::new(ConnectionsRb::new(None));
        let client_id1 = uuid::Uuid::new_v4();
        let ob = db
            .new_connection(client_id1, "localhost".to_string(), None)
            .unwrap();
        db.ping_check();
        let client_ids = db.connections();
        assert_eq!(client_ids.len(), 1);
        assert_eq!(db.connection_object_for_client(client_id1), Some(ob));
    }
}
