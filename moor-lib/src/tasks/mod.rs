use async_trait::async_trait;

use crate::var::Objid;

pub mod command_parse;
pub mod scheduler;

#[async_trait]
pub trait Sessions: Send + Sync {
    async fn send_text(&mut self, player: Objid, msg: String) -> Result<(), anyhow::Error>;
    async fn connected_players(&self) -> Result<Vec<Objid>, anyhow::Error>;
}
