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

//! An implementation of the connections db that uses tuplebox.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::Error;
use async_trait::async_trait;
use strum::{Display, EnumCount, EnumIter, IntoEnumIterator};
use tracing::{debug, error, warn};
use uuid::Uuid;

use moor_db::tuplebox::{RelationId, RelationInfo, Transaction, TupleBox};
use moor_kernel::tasks::sessions::SessionError;
use moor_values::util::slice_ref::SliceRef;
use moor_values::var::objid::Objid;
use moor_values::AsByteBuffer;
use rpc_common::RpcRequestError;

use crate::connections::{ConnectionsDB, CONNECTION_TIMEOUT_DURATION};

const CONNECTIONS_DB_MEM_SIZE: usize = 1 << 26;
pub struct ConnectionsTb {
    tb: Arc<TupleBox>,
}

impl ConnectionsTb {
    pub async fn new(path: Option<PathBuf>) -> Self {
        let mut relations: Vec<RelationInfo> = ConnectionRelation::iter()
            .map(|r| {
                RelationInfo {
                    name: r.to_string(),
                    domain_type_id: 0, /* tbd */
                    codomain_type_id: 0,
                    secondary_indexed: false,
                }
            })
            .collect();
        relations[ConnectionRelation::ClientConnection as usize].secondary_indexed = true;

        let tb = TupleBox::new(CONNECTIONS_DB_MEM_SIZE, path, &relations, 1).await;
        Self { tb }
    }
}

#[repr(usize)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, EnumIter, EnumCount, Display)]
// Don't warn about same-prefix, "I did that on purpose"
#[allow(clippy::enum_variant_names)]
enum ConnectionRelation {
    // One to many, client id <-> connection/player object. Secondary index will seek on object id.
    ClientConnection = 0,
    // Client -> SystemTime of last activity
    ClientActivity = 1,
    // Client connect time.
    ClientConnectTime = 2,
    // Client last ping time.
    ClientPingTime = 3,
    // Client hostname / connection "name"
    ClientName = 4,
}

impl ConnectionsTb {
    async fn most_recent_client_connection(
        tx: &Transaction,
        connection_obj: Objid,
    ) -> Result<Vec<(SliceRef, SystemTime)>, SessionError> {
        let clients = tx
            .relation(RelationId(ConnectionRelation::ClientConnection as usize))
            .await
            .seek_by_codomain(connection_obj.as_sliceref())
            .await
            .expect("Unable to seek client connection");

        // Seek the most recent activity for the connection, so pull in the activity relation for
        // each client.
        let mut times = Vec::new();
        for (client, _) in clients {
            if let Ok(last_activity) = tx
                .relation(RelationId(ConnectionRelation::ClientActivity as usize))
                .await
                .seek_by_domain(client.clone())
                .await
            {
                let epoch_time_millis: u128 =
                    u128::from_le_bytes(last_activity.1.as_slice().try_into().unwrap());
                let time = SystemTime::UNIX_EPOCH + Duration::from_millis(epoch_time_millis as u64);
                times.push((client.clone(), time));
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

fn sliceref_as_time(slc: SliceRef) -> SystemTime {
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

#[async_trait]
impl ConnectionsDB for ConnectionsTb {
    async fn update_client_connection(
        &self,
        from_connection: Objid,
        to_player: Objid,
    ) -> Result<(), Error> {
        let tx = self.tb.clone().start_tx();
        let client_ids = tx
            .relation(RelationId(ConnectionRelation::ClientConnection as usize))
            .await
            .seek_by_codomain(from_connection.as_sliceref())
            .await
            .expect("Unable to seek client connection");
        if client_ids.is_empty() {
            error!(?from_connection, ?to_player, "No client ids for connection");
            return Err(Error::msg("No client ids for connection"));
        }
        for (client_id, _) in client_ids {
            let _ = tx
                .relation(RelationId(ConnectionRelation::ClientConnection as usize))
                .await
                .update_tuple(client_id.clone(), to_player.as_sliceref())
                .await;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn new_connection(
        &self,
        client_id: Uuid,
        hostname: String,
        player: Option<Objid>,
    ) -> Result<Objid, RpcRequestError> {
        let connection_oid = match player {
            None => {
                // The connection object is pulled from the sequence, then we invert it and subtract from
                // -4 to get the connection object, since they always grow downwards from there.
                let connection_id = self.tb.clone().increment_sequence(0).await;
                let connection_id: i64 = -4 - (connection_id as i64);
                Objid(connection_id)
            }
            Some(player) => player,
        };

        // Insert the initial tuples for the connection.
        let tx = self.tb.clone().start_tx();
        let client_id = SliceRef::from_bytes(client_id.as_bytes());
        tx.relation(RelationId(ConnectionRelation::ClientConnection as usize))
            .await
            .insert_tuple(client_id.clone(), connection_oid.as_sliceref())
            .await
            .expect("Unable to insert client connection");
        tx.relation(RelationId(ConnectionRelation::ClientActivity as usize))
            .await
            .insert_tuple(client_id.clone(), now_as_sliceref())
            .await
            .expect("Unable to insert client activity");
        tx.relation(RelationId(ConnectionRelation::ClientConnectTime as usize))
            .await
            .insert_tuple(client_id.clone(), now_as_sliceref())
            .await
            .expect("Unable to insert client connect time");
        tx.relation(RelationId(ConnectionRelation::ClientPingTime as usize))
            .await
            .insert_tuple(client_id.clone(), now_as_sliceref())
            .await
            .expect("Unable to insert client ping time");
        tx.relation(RelationId(ConnectionRelation::ClientName as usize))
            .await
            .insert_tuple(client_id.clone(), SliceRef::from_bytes(hostname.as_bytes()))
            .await
            .expect("Unable to insert client name");

        tx.commit().await.expect("Unable to commit transaction");

        Ok(connection_oid)
    }

    async fn record_client_activity(&self, client_id: Uuid, _connobj: Objid) -> Result<(), Error> {
        let tx = self.tb.clone().start_tx();
        tx.relation(RelationId(ConnectionRelation::ClientActivity as usize))
            .await
            .upsert_tuple(
                SliceRef::from_bytes(client_id.as_bytes()),
                now_as_sliceref(),
            )
            .await
            .expect("Unable to update client activity");
        tx.commit().await?;
        Ok(())
    }

    async fn notify_is_alive(&self, client_id: Uuid, _connection: Objid) -> Result<(), Error> {
        let tx = self.tb.clone().start_tx();
        tx.relation(RelationId(ConnectionRelation::ClientPingTime as usize))
            .await
            .upsert_tuple(
                SliceRef::from_bytes(client_id.as_bytes()),
                now_as_sliceref(),
            )
            .await
            .expect("Unable to update client ping time");
        tx.commit().await?;
        Ok(())
    }

    async fn ping_check(&self) {
        let now = SystemTime::now();
        let timeout_threshold = now - CONNECTION_TIMEOUT_DURATION;

        // Full scan the last ping relation, and compare the last ping time to the current time.
        // If the difference is greater than the timeout duration, then we need to remove the
        // connection from all the relations.
        let tx = self.tb.clone().start_tx();

        let last_ping_relation = tx
            .relation(RelationId(ConnectionRelation::ClientPingTime as usize))
            .await;
        let expired = last_ping_relation
            .predicate_scan(&|(_, last_ping_time)| {
                let last_ping_time = sliceref_as_time(last_ping_time.clone());
                last_ping_time < timeout_threshold
            })
            .await
            .expect("Unable to scan last ping relation");

        for (client_id, last_ping_time) in expired {
            debug!(
                "Expiring connection for client {:?} because last_ping_time = {:?}",
                client_id,
                sliceref_as_time(last_ping_time)
            );
            let _ = tx
                .relation(RelationId(ConnectionRelation::ClientConnection as usize))
                .await
                .remove_by_domain(client_id.clone())
                .await;
            let _ = tx
                .relation(RelationId(ConnectionRelation::ClientActivity as usize))
                .await
                .remove_by_domain(client_id.clone())
                .await;
            let _ = tx
                .relation(RelationId(ConnectionRelation::ClientConnectTime as usize))
                .await
                .remove_by_domain(client_id.clone())
                .await;
            let _ = tx
                .relation(RelationId(ConnectionRelation::ClientPingTime as usize))
                .await
                .remove_by_domain(client_id.clone())
                .await;
            let _ = tx
                .relation(RelationId(ConnectionRelation::ClientName as usize))
                .await
                .remove_by_domain(client_id.clone())
                .await;
        }
        tx.commit().await.expect("Unable to commit transaction");
    }

    async fn last_activity_for(&self, connection_obj: Objid) -> Result<SystemTime, SessionError> {
        let tx = self.tb.clone().start_tx();
        let mut client_times = Self::most_recent_client_connection(&tx, connection_obj).await?;

        // Most recent time is the last one.
        let Some(time) = client_times.pop() else {
            return Err(SessionError::NoConnectionForPlayer(connection_obj));
        };
        tx.commit().await.expect("Unable to commit transaction");
        Ok(time.1)
    }

    async fn connection_name_for(&self, connection_obj: Objid) -> Result<String, SessionError> {
        let tx = self.tb.clone().start_tx();
        let mut client_times = Self::most_recent_client_connection(&tx, connection_obj).await?;

        let Some(most_recent) = client_times.pop() else {
            return Err(SessionError::NoConnectionForPlayer(connection_obj));
        };

        let client_id = most_recent.0;
        let name = tx
            .relation(RelationId(ConnectionRelation::ClientName as usize))
            .await
            .seek_by_domain(client_id.clone())
            .await
            .expect("Unable to seek client name");
        tx.commit().await.expect("Unable to commit transaction");
        Ok(String::from_utf8(name.1.as_slice().to_vec()).expect("Invalid UTF-8 in client name"))
    }

    async fn connected_seconds_for(&self, player: Objid) -> Result<f64, SessionError> {
        let tx = self.tb.clone().start_tx();
        // In this case we need to find the earliest connection time for the player, and then
        // subtract that from the current time.
        let Ok(clients) = tx
            .relation(RelationId(ConnectionRelation::ClientConnection as usize))
            .await
            .seek_by_codomain(player.as_sliceref())
            .await
        else {
            return Err(SessionError::NoConnectionForPlayer(player));
        };

        let mut times = Vec::new();
        for (client, _) in clients {
            if let Ok(connect_time) = tx
                .relation(RelationId(ConnectionRelation::ClientConnectTime as usize))
                .await
                .seek_by_domain(client.clone())
                .await
            {
                let time = sliceref_as_time(connect_time.1);
                times.push((client.clone(), time));
            }
        }
        times.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap());
        let earliest = times.pop().expect("No connection for player");
        let earliest = earliest.1;
        let now = SystemTime::now();
        let duration = now.duration_since(earliest).expect("Invalid duration");
        let seconds = duration.as_secs_f64();
        tx.commit().await.expect("Unable to commit transaction");
        Ok(seconds)
    }

    async fn client_ids_for(&self, player: Objid) -> Result<Vec<Uuid>, SessionError> {
        let tx = self.tb.clone().start_tx();
        let Ok(clients) = tx
            .relation(RelationId(ConnectionRelation::ClientConnection as usize))
            .await
            .seek_by_codomain(player.as_sliceref())
            .await
        else {
            return Ok(vec![]);
        };

        let mut client_ids = Vec::new();
        for (client, _) in clients {
            let client_id = Uuid::from_slice(client.as_slice()).expect("Invalid UUID");
            client_ids.push(client_id);
        }
        tx.commit().await.expect("Unable to commit transaction");
        Ok(client_ids)
    }

    async fn connections(&self) -> Vec<Objid> {
        // Full scan from ClientConnection relation to get all connections, and dump them into a
        // hashset (to remove dupes) and return as a vector.
        let tx = self.tb.clone().start_tx();
        let mut connections = HashSet::new();
        let clients = tx
            .relation(RelationId(ConnectionRelation::ClientConnection as usize))
            .await
            .predicate_scan(&|_| true)
            .await
            .expect("Unable to scan client connection relation");

        for (_, connection) in clients {
            let connection = Objid::from_sliceref(connection);
            connections.insert(connection);
        }

        tx.commit().await.expect("Unable to commit transaction");
        connections.into_iter().collect()
    }

    async fn is_valid_client(&self, client_id: Uuid) -> bool {
        let tx = self.tb.clone().start_tx();
        let is_valid = tx
            .relation(RelationId(ConnectionRelation::ClientConnection as usize))
            .await
            .seek_by_domain(client_id.as_bytes().as_sliceref())
            .await
            .is_ok();
        tx.commit().await.expect("Unable to commit transaction");
        is_valid
    }

    async fn connection_object_for_client(&self, client_id: Uuid) -> Option<Objid> {
        let tx = self.tb.clone().start_tx();
        let connection = match tx
            .relation(RelationId(ConnectionRelation::ClientConnection as usize))
            .await
            .seek_by_domain(client_id.as_bytes().as_sliceref())
            .await
        {
            Ok((_, connection)) => Some(Objid::from_sliceref(connection)),
            Err(_) => None,
        };
        tx.commit().await.expect("Unable to commit transaction");
        connection
    }

    async fn remove_client_connection(&self, client_id: Uuid) -> Result<(), Error> {
        let tx = self.tb.clone().start_tx();
        let _ = tx
            .relation(RelationId(ConnectionRelation::ClientConnection as usize))
            .await
            .remove_by_domain(client_id.as_bytes().as_sliceref())
            .await;
        let _ = tx
            .relation(RelationId(ConnectionRelation::ClientActivity as usize))
            .await
            .remove_by_domain(client_id.as_bytes().as_sliceref())
            .await;
        let _ = tx
            .relation(RelationId(ConnectionRelation::ClientConnectTime as usize))
            .await
            .remove_by_domain(client_id.as_bytes().as_sliceref())
            .await;
        let _ = tx
            .relation(RelationId(ConnectionRelation::ClientPingTime as usize))
            .await
            .remove_by_domain(client_id.as_bytes().as_sliceref())
            .await;
        let _ = tx
            .relation(RelationId(ConnectionRelation::ClientName as usize))
            .await
            .remove_by_domain(client_id.as_bytes().as_sliceref())
            .await;

        tx.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use moor_values::var::objid::Objid;

    use crate::connections::ConnectionsDB;
    use crate::connections_tb::ConnectionsTb;

    /// Simple test of:
    ///     * Attach a connection<->client
    ///     * Record activity & verify
    ///     * Update connection<->client to a new connection
    ///     * Verify the old connection has no clients
    ///     * Verify the new connection has the client
    ///     * Remove the connection<->client
    ///     * Verify the connection has no clients
    #[tokio::test]
    async fn test_single_connection() {
        let db = Arc::new(ConnectionsTb::new(None).await);
        let mut jh = vec![];

        for x in 1..100 {
            let db = db.clone();
            jh.push(tokio::spawn(async move {
                let client_id = uuid::Uuid::new_v4();
                let oid = db
                    .new_connection(client_id, "localhost".to_string(), None)
                    .await
                    .unwrap();
                let client_ids = db.client_ids_for(oid).await.unwrap();
                assert_eq!(client_ids.len(), 1);
                assert_eq!(client_ids[0], client_id);
                db.record_client_activity(client_id, oid).await.unwrap();
                db.notify_is_alive(client_id, oid).await.unwrap();
                let last_activity = db
                    .last_activity_for(oid)
                    .await
                    .unwrap()
                    .elapsed()
                    .unwrap()
                    .as_secs_f64();
                assert!(last_activity < 1.0);
                assert!(db.is_valid_client(client_id).await);
                db.update_client_connection(oid, Objid(x))
                    .await
                    .expect("Unable to update client connection");
                let client_ids = db.client_ids_for(Objid(x)).await.unwrap();
                assert_eq!(client_ids.len(), 1);
                assert_eq!(client_ids[0], client_id);
                db.remove_client_connection(client_id).await.unwrap();
                assert!(!db.is_valid_client(client_id).await);
                let client_ids = db.client_ids_for(Objid(x)).await.unwrap();
                assert!(client_ids.is_empty());
            }));
        }
        for j in jh {
            j.await.unwrap();
        }
    }

    /// Test that a given player can have multiple clients connected to it.
    #[tokio::test]
    async fn test_multiple_connections() {
        let db = Arc::new(ConnectionsTb::new(None).await);
        let mut jh = vec![];
        for x in 1..100 {
            let db = db.clone();
            jh.push(tokio::spawn(async move {
                let client_id1 = uuid::Uuid::new_v4();
                let client_id2 = uuid::Uuid::new_v4();
                let con_oid1 = db
                    .new_connection(client_id1, "localhost".to_string(), None)
                    .await
                    .unwrap();
                let con_oid2 = db
                    .new_connection(client_id2, "localhost".to_string(), None)
                    .await
                    .unwrap();
                db.update_client_connection(con_oid1, Objid(x))
                    .await
                    .expect("Unable to update client connection");
                let client_ids = db.client_ids_for(Objid(x)).await.unwrap();
                assert_eq!(client_ids.len(), 1);
                assert!(client_ids.contains(&client_id1));

                db.update_client_connection(con_oid2, Objid(x))
                    .await
                    .expect("Unable to update client connection");
                let client_ids = db.client_ids_for(Objid(x)).await.unwrap();
                assert_eq!(client_ids.len(), 2);
                assert!(client_ids.contains(&client_id2));

                db.record_client_activity(client_id1, Objid(x))
                    .await
                    .unwrap();
                let last_activity = db
                    .last_activity_for(Objid(x))
                    .await
                    .unwrap()
                    .elapsed()
                    .unwrap()
                    .as_secs_f64();
                assert!(last_activity < 1.0);
                db.remove_client_connection(client_id1).await.unwrap();
                let client_ids = db.client_ids_for(Objid(x)).await.unwrap();
                assert_eq!(client_ids.len(), 1);
                assert!(client_ids.contains(&client_id2));
            }));
        }
        for j in jh {
            j.await.unwrap();
        }
    }

    // Validate that ping check works.
    #[tokio::test]
    async fn ping_test() {
        let db = Arc::new(ConnectionsTb::new(None).await);
        let client_id1 = uuid::Uuid::new_v4();
        let ob = db
            .new_connection(client_id1, "localhost".to_string(), None)
            .await
            .unwrap();
        db.ping_check().await;
        let client_ids = db.connections().await;
        assert_eq!(client_ids.len(), 1);
        assert!(db.is_valid_client(client_id1).await);
        assert_eq!(db.connection_object_for_client(client_id1).await, Some(ob));
    }
}
