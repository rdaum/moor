use async_trait::async_trait;

use moor_value::var::objid::Objid;
use moor_value::var::Var;

pub mod command_parse;
pub mod scheduler;
mod task;

/// The interface for managing the user connection side of state, exposed by the scheduler to the VM
/// during execution.
#[async_trait]
pub trait Sessions: Send + Sync {
    /// Spool output to the given player's connection, from a given task.
    /// The actual output will not be sent until the task commits, and will be thrown out on
    /// rollback.
    async fn send_text(&mut self, player: Objid, msg: &str) -> Result<(), anyhow::Error>;

    /// Process a (wizard) request for system shutdown, with an optional shutdown message.
    async fn shutdown(&mut self, msg: Option<String>) -> Result<(), anyhow::Error>;

    async fn connection_name(&self, player: Objid) -> Result<String, anyhow::Error>;

    /// Disconnect the given player's connection.
    async fn disconnect(&mut self, player: Objid) -> Result<(), anyhow::Error>;

    /// Return the list of other currently-connected players.
    fn connected_players(&self) -> Result<Vec<Objid>, anyhow::Error>;

    /// Return how many seconds the given player has been connected.
    fn connected_seconds(&self, player: Objid) -> Result<f64, anyhow::Error>;

    /// Return how many seconds the given player has been idle (no tasks submitted).
    fn idle_seconds(&self, player: Objid) -> Result<f64, anyhow::Error>;
}

pub type TaskId = usize;

/// The minimum set of information needed to make a *resolution* call for a verb.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct VerbCall {
    pub verb_name: String,
    pub location: Objid,
    pub this: Objid,
    pub player: Objid,
    pub args: Vec<Var>,
    pub caller: Objid,
}
