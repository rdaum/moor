use std::sync::Arc;

use moor_values::model::world_state::WorldStateSource;
use moor_values::model::WorldStateError;

use crate::loader::LoaderInterface;
use crate::tb_worldstate::TupleBoxWorldStateSource;

mod db_loader_client;
mod db_tx;
mod db_worldstate;
pub mod loader;
mod object_relations;
pub mod tb_worldstate;
pub mod tuplebox;

#[doc(hidden)]
pub mod testing;

pub struct DatabaseBuilder {
    path: Option<std::path::PathBuf>,
    memory_size: Option<usize>,
}

pub trait Database {
    fn loader_client(&mut self) -> Result<Box<dyn LoaderInterface>, WorldStateError>;
    fn world_state_source(self: Box<Self>) -> Result<Arc<dyn WorldStateSource>, WorldStateError>;
}

impl DatabaseBuilder {
    pub fn new() -> Self {
        Self {
            path: None,
            memory_size: None,
        }
    }

    pub fn with_path(mut self, path: std::path::PathBuf) -> Self {
        self.path = Some(path);
        self
    }

    pub fn with_memory_size(&mut self, memory_size_bytes: usize) -> &mut Self {
        self.memory_size = Some(memory_size_bytes);
        self
    }

    /// Returns a new database instance. The second value in the result tuple is true if the
    /// database was newly created, and false if it was already present.
    pub async fn open_db(&self) -> Result<(Box<dyn Database>, bool), String> {
        let (db, fresh) =
            TupleBoxWorldStateSource::open(self.path.clone(), self.memory_size.unwrap_or(1 << 40))
                .await;
        Ok((Box::new(db), fresh))
    }
}

impl Default for DatabaseBuilder {
    fn default() -> Self {
        Self::new()
    }
}
