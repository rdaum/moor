use std::time::{Duration, SystemTime};

use uuid::Uuid;

use moor_kernel::tasks::sessions::SessionError;
use moor_values::var::objid::Objid;
use rpc_common::RpcRequestError;

pub const CONNECTION_TIMEOUT_DURATION: Duration = Duration::from_secs(30);

#[async_trait::async_trait]
pub trait ConnectionsDB {
    /// Update the connection record for the given connection object to point to the given player.
    /// This is used when a player logs in.
    async fn update_client_connection(
        &self,
        from_connection: Objid,
        to_player: Objid,
    ) -> Result<(), anyhow::Error>;

    /// Create a new connection object for the given client.
    async fn new_connection(
        &self,
        client_id: Uuid,
        hostname: String,
    ) -> Result<Objid, RpcRequestError>;

    /// Record activity for the given client.
    async fn record_client_activity(
        &self,
        client_id: Uuid,
        connobj: Objid,
    ) -> Result<(), anyhow::Error>;

    /// Update the last ping time for a client / connection.
    async fn notify_is_alive(
        &self,
        client_id: Uuid,
        connection: Objid,
    ) -> Result<(), anyhow::Error>;

    /// Prune any connections that have not been active for longer than the required duration.
    async fn ping_check(&self);

    async fn last_activity_for(&self, connection: Objid) -> Result<SystemTime, SessionError>;

    async fn connection_name_for(&self, player: Objid) -> Result<String, SessionError>;

    async fn connected_seconds_for(&self, player: Objid) -> Result<f64, SessionError>;

    async fn client_ids_for(&self, player: Objid) -> Result<Vec<Uuid>, SessionError>;

    /// Return all connection objects (player or not)
    async fn connections(&self) -> Vec<Objid>;

    /// Return whether the given client is a valid client.
    async fn is_valid_client(&self, client_id: Uuid) -> bool;

    /// Retrieve the connection object for the given client.
    async fn connection_object_for_client(&self, client_id: Uuid) -> Option<Objid>;

    /// Remove the given client from the connection database.
    async fn remove_client_connection(&self, client_id: Uuid) -> Result<(), anyhow::Error>;
}
