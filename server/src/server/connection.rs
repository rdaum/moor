use std::time::Instant;

use anyhow::Error;
use async_trait::async_trait;
use moor_values::model::NarrativeEvent;
use moor_values::var::objid::Objid;

use super::{ConnectType, DisconnectReason};

#[async_trait]
pub trait Connection: Send + Sync {
    async fn write_message(&mut self, msg: NarrativeEvent) -> Result<(), Error>;
    async fn notify_connected(
        &mut self,
        player: Objid,
        connect_type: ConnectType,
    ) -> Result<(), Error>;
    async fn disconnect(&mut self, reason: DisconnectReason) -> Result<(), Error>;
    async fn connection_name(&self, player: Objid) -> Result<String, Error>;
    async fn player(&self) -> Objid;
    async fn update_player(&mut self, player: Objid) -> Result<(), Error>;
    async fn last_activity(&self) -> Instant;
    async fn record_activity(&mut self, when: Instant);
    async fn connected_time(&self) -> Instant;
}
