use async_trait::async_trait;

use crate::values::objid::Objid;

pub mod command_parse;
pub mod scheduler;
mod task;

#[async_trait]
pub trait Sessions: Send + Sync {
    async fn send_text(&mut self, player: Objid, msg: &str) -> Result<(), anyhow::Error>;
    async fn connected_players(&self) -> Result<Vec<Objid>, anyhow::Error>;
}

pub type TaskId = usize;
