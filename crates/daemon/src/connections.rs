use anyhow::bail;
use itertools::Itertools;
use moor_kernel::tasks::sessions::SessionError;
use moor_values::var::objid::Objid;
use rpc_common::RpcRequestError;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::RwLock;
use std::time::{Duration, Instant, SystemTime};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{error, info, warn};
use uuid::Uuid;

const CONNECTION_TIMEOUT_DURATION: Duration = Duration::from_secs(30);
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct ConnectionRecord {
    pub(crate) client_id: Uuid,
    pub(crate) player: Objid,
    pub(crate) name: String,
    pub(crate) last_activity: SystemTime,
    pub(crate) connect_time: SystemTime,
    pub(crate) last_ping: Instant,
}

/// A database for tracking the client connections, persistently between restarts

pub struct Connections {
    connections_file: PathBuf,
    connections_list: RwLock<ConnectionList>,
    next_connection_id: AtomicI64,
}

#[derive(Clone, Debug)]
struct ConnectionList {
    client_connections: HashMap<Uuid, Objid>,
    connections_client: HashMap<Objid, Vec<ConnectionRecord>>,
}

impl ConnectionList {
    /// Sync out the list of connections to a (human readable) file which we can restore from
    /// at restart.
    async fn sync(&self, connections_file: &Path) -> Result<(), anyhow::Error> {
        // Both hashtables can be reconstituded from the list of connection records, so we only need
        // to write that out.
        let mut file = File::create(connections_file).await?;
        let mut connections = vec![];
        for (_, records) in self.connections_client.iter() {
            for record in records {
                let connect_time = record
                    .connect_time
                    .duration_since(SystemTime::UNIX_EPOCH)?
                    .as_secs_f64();
                let last_activity = record
                    .last_activity
                    .duration_since(SystemTime::UNIX_EPOCH)?
                    .as_secs_f64();
                let entry = json!({
                    "client_id": record.client_id.to_string(),
                    "player": record.player.to_literal(),
                    "name": record.name,
                    "connect_time": connect_time,
                    "last_activity": last_activity,
                });
                connections.push(entry);
            }
        }
        let json_str = serde_json::to_string_pretty(&connections)?;
        file.write_all(json_str.as_bytes()).await?;
        Ok(())
    }

    async fn from_file(connections_file: &Path) -> Result<Self, anyhow::Error> {
        // Reconstitute the list of connections from the file.
        let mut file = File::open(connections_file).await?;
        let mut contents = vec![];
        file.read_to_end(&mut contents).await?;
        let connections: Vec<Value> = serde_json::from_slice(&contents)?;

        let mut client_connections = HashMap::new();
        let mut connections_client = HashMap::new();
        for record in connections {
            let client_id = record
                .get("client_id")
                .ok_or_else(|| anyhow::anyhow!("Missing client_id"))?;
            let client_id = Uuid::parse_str(client_id.as_str().unwrap())?;
            let player = record
                .get("player")
                .ok_or_else(|| anyhow::anyhow!("Missing player"))?
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Invalid player value"))?;
            // Object id: #1234 is the format, and we need the 1234 part.
            let Some(player_oid) = player.strip_prefix('#') else {
                bail!("Invalid player value");
            };
            let player = Objid(player_oid.parse()?);

            let name = record
                .get("name")
                .ok_or_else(|| anyhow::anyhow!("Missing name"))?
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Invalid name value"))?;

            let connect_time_str = record
                .get("connect_time")
                .ok_or_else(|| anyhow::anyhow!("Missing connect_time"))?;
            let connect_time_since_epoch = connect_time_str.as_f64().ok_or_else(|| {
                anyhow::anyhow!("Invalid connect_time value ({})", connect_time_str)
            })?;
            let connect_time = SystemTime::UNIX_EPOCH
                .checked_add(std::time::Duration::from_secs_f64(connect_time_since_epoch))
                .ok_or_else(|| {
                    anyhow::anyhow!("Invalid connect_time value ({})", connect_time_since_epoch)
                })?;

            let last_activity_str = record
                .get("last_activity")
                .ok_or_else(|| anyhow::anyhow!("Missing last_activity"))?;
            let last_activity_since_epoch = last_activity_str.as_f64().ok_or_else(|| {
                anyhow::anyhow!("Invalid last_activity value: {:?}", last_activity_str)
            })?;
            let last_activity = SystemTime::UNIX_EPOCH
                .checked_add(std::time::Duration::from_secs_f64(
                    last_activity_since_epoch,
                ))
                .ok_or_else(|| anyhow::anyhow!("Invalid last_activity value"))?;

            let cr = ConnectionRecord {
                client_id,
                player,
                name: name.to_string(),
                last_activity,
                connect_time,
                last_ping: Instant::now(),
            };

            client_connections.insert(cr.client_id, cr.player);
            match connections_client.get_mut(&cr.player) {
                None => {
                    connections_client.insert(cr.player, vec![cr]);
                }
                Some(ref mut crs) => {
                    crs.push(cr);
                }
            }
        }

        Ok(Self {
            client_connections,
            connections_client,
        })
    }
}

impl Connections {
    pub(crate) async fn new(connections_file: PathBuf) -> Self {
        // Attempt reconstitute from file, and if that file doesn't exist, create a new one.
        let connections_list = match ConnectionList::from_file(&connections_file).await {
            Ok(cl) => cl,
            Err(e) => {
                warn!("No connections list file at: {}, creating a fresh list", e);
                ConnectionList {
                    client_connections: HashMap::new(),
                    connections_client: HashMap::new(),
                }
            }
        };
        Self {
            connections_file,
            connections_list: RwLock::new(connections_list),
            next_connection_id: AtomicI64::new(-4),
        }
    }

    async fn sync(&self) {
        let copy = self.connections_list.read().unwrap().clone();
        let connections_file = self.connections_file.clone();
        // TODO: this may not need to run so frequently. Maybe we can do it on a timer?
        tokio::spawn(async move {
            if let Err(e) = copy.sync(&connections_file).await {
                error!("Error syncing connections: {}", e);
            }
        });
    }

    pub(crate) async fn activity_for_client(
        &self,
        client_id: Uuid,
        connobj: Objid,
    ) -> Result<(), anyhow::Error> {
        let mut inner = self.connections_list.write().unwrap();
        let connection_record = inner
            .connections_client
            .get_mut(&connobj)
            .ok_or_else(|| anyhow::anyhow!("No connection for player: {}", connobj))?
            .iter_mut()
            .find(|cr| cr.client_id == client_id)
            .ok_or_else(|| anyhow::anyhow!("No connection record for client: {}", client_id))?;
        connection_record.last_activity = SystemTime::now();
        Ok(())
    }

    /// Update the last ping time for a client / connection.
    pub(crate) async fn notify_is_alive(
        &self,
        client_id: Uuid,
        connection: Objid,
    ) -> Result<(), anyhow::Error> {
        let mut inner = self.connections_list.write().unwrap();
        let connections = inner
            .connections_client
            .get_mut(&connection)
            .ok_or_else(|| anyhow::anyhow!("No connection for player: {}", connection))?;
        for cr in connections.iter_mut() {
            if cr.client_id == client_id {
                cr.last_ping = Instant::now();
                break;
            }
        }
        Ok(())
    }

    pub(crate) async fn ping_check(&self) {
        let to_remove = {
            let inner = self.connections_list.read().unwrap();

            // Check all connections to see if they have timed out (no ping response in N interval).
            // If any have, remove them from the list.
            let mut to_remove = vec![];
            for (_, clients) in inner.connections_client.iter() {
                for c in clients {
                    if c.last_ping.elapsed() > CONNECTION_TIMEOUT_DURATION {
                        to_remove.push(c.client_id);
                    }
                }
            }
            to_remove
        };
        for client in to_remove {
            info!("Client {} timed out, removing", client);
            self.remove_client_connection(client)
                .await
                .expect("Unable to remove client connection");
        }
    }

    /// Return all connection objects (player or not)
    pub(crate) async fn connections(&self) -> Vec<Objid> {
        self.connections_list
            .read()
            .unwrap()
            .connections_client
            .keys()
            .cloned()
            .collect()
    }

    pub(crate) async fn is_valid_client(&self, client_id: Uuid) -> bool {
        self.connections_list
            .read()
            .unwrap()
            .client_connections
            .contains_key(&client_id)
    }

    pub(crate) async fn connection_object_for_client(&self, client_id: Uuid) -> Option<Objid> {
        self.connections_list
            .read()
            .unwrap()
            .client_connections
            .get(&client_id)
            .cloned()
    }

    pub(crate) async fn remove_client_connection(
        &self,
        client_id: Uuid,
    ) -> Result<(), anyhow::Error> {
        {
            let mut inner = self.connections_list.write().unwrap();

            let Some(connection) = inner.client_connections.remove(&client_id) else {
                bail!("No (expected) connection for client: {}", client_id);
            };

            let Some(clients) = inner.connections_client.get_mut(&connection) else {
                bail!("No (expected) connection record for player: {}", connection);
            };

            clients.retain(|c| c.client_id != client_id);
        }
        self.sync().await;
        Ok(())
    }

    pub(crate) async fn new_connection(
        &self,
        client_id: Uuid,
        hostname: String,
    ) -> Result<Objid, RpcRequestError> {
        let connection_id = {
            let mut inner = self.connections_list.write().unwrap();

            // We should not already have an object connection id for this client. If we do,
            // respond with an error.

            if inner.client_connections.contains_key(&client_id) {
                return Err(RpcRequestError::AlreadyConnected);
            }

            // Get a new connection id, and create an entry for it.
            let connection_id = Objid(self.next_connection_id.fetch_sub(1, Ordering::SeqCst));
            inner.client_connections.insert(client_id, connection_id);
            inner.connections_client.insert(
                connection_id,
                vec![ConnectionRecord {
                    client_id,
                    player: connection_id,
                    name: hostname,
                    last_activity: SystemTime::now(),
                    connect_time: SystemTime::now(),
                    last_ping: Instant::now(),
                }],
            );
            connection_id
        };
        self.sync().await;
        Ok(connection_id)
    }

    pub(crate) async fn connection_records_for(
        &self,
        player: Objid,
    ) -> Result<Vec<ConnectionRecord>, SessionError> {
        let inner = self.connections_list.read().unwrap();
        let Some(connections) = inner.connections_client.get(&player) else {
            return Ok(vec![]);
        };

        if connections.is_empty() {
            return Ok(vec![]);
        }

        Ok(connections
            .iter()
            .sorted_by_key(|a| a.last_activity)
            .cloned()
            .collect())
    }

    pub(crate) async fn update_client_connection(
        &self,
        from_connection: Objid,
        to_player: Objid,
    ) -> Result<(), anyhow::Error> {
        {
            let mut inner = self.connections_list.write().unwrap();

            let mut connection_records = inner
                .connections_client
                .remove(&from_connection)
                .expect("connection record missing");
            assert_eq!(
                connection_records.len(),
                1,
                "connection record for unlogged in connection has multiple entries"
            );
            let mut cr = connection_records.pop().unwrap();
            cr.player = to_player;
            cr.last_activity = SystemTime::now();

            inner.client_connections.insert(cr.client_id, to_player);
            match inner.connections_client.get_mut(&to_player) {
                None => {
                    inner.connections_client.insert(to_player, vec![cr]);
                }
                Some(ref mut crs) => {
                    crs.push(cr);
                }
            }
            inner.connections_client.remove(&from_connection);
        }
        self.sync().await;

        Ok(())
    }
}

impl Drop for Connections {
    fn drop(&mut self) {
        let copy = self.connections_list.read().unwrap().clone();
        let connections_file = self.connections_file.clone();
        if let Err(e) = tokio::runtime::Handle::current().block_on(copy.sync(&connections_file)) {
            error!("Error syncing connections: {}", e);
        }
        info!("Connections sync'd: {:?} ... {:?}", connections_file, copy);
    }
}
