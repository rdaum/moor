use std::sync::Arc;
use strum::{Display, EnumIter, EnumString, EnumVariantNames};

use crate::loader::LoaderInterface;
use crate::rocksdb::db_server::RocksDbServer;
use moor_values::model::world_state::WorldStateSource;
use moor_values::model::WorldStateError;
use tuplebox::tb_worldstate::TupleBoxWorldStateSource;

mod channel_db_tx_client;
mod db_loader_client;
mod db_message;
mod db_tx;
mod db_worldstate;
pub mod loader;
pub mod mock;
pub mod rocksdb;
pub mod tuplebox;

/// Enumeration of potential worldstate/database backends for Moor.
#[derive(Debug, Display, EnumString, EnumVariantNames, EnumIter, Clone, Copy)]
pub enum DatabaseType {
    /// Custom in-memory transactional database.
    /// Relations are stored in memory as copy-on-write hashmaps (HAMTs from im::HashMap), with
    /// each transaction having a fully snapshot-isolated view of the world.
    ///
    /// Objects are backed up to disk on commit, and restored from disk on startup, but are not
    /// paged out on inactivity, so the dataset size is limited to the amount of available memory,
    /// for now.
    ///
    /// Faster, but currently only suitable for worlds that fit in main memory.   
    Tuplebox,
    /// Direct translation to RocksDB, using its OptimisticTransaction model.
    /// Potentially slower, and the transactional model not as robust.
    /// But can handle larger worlds.
    RocksDb,
}

pub struct DatabaseBuilder {
    db_type: DatabaseType,
    path: Option<std::path::PathBuf>,
}

pub trait Database {
    fn loader_client(&mut self) -> Result<Box<dyn LoaderInterface>, WorldStateError>;
    fn world_state_source(self: Box<Self>) -> Result<Arc<dyn WorldStateSource>, WorldStateError>;
}

impl DatabaseBuilder {
    pub fn new() -> Self {
        Self {
            db_type: DatabaseType::RocksDb,
            path: None,
        }
    }

    pub fn with_db_type(mut self, db_type: DatabaseType) -> Self {
        self.db_type = db_type;
        self
    }
    pub fn with_path(mut self, path: std::path::PathBuf) -> Self {
        self.path = Some(path);
        self
    }

    /// Returns a new database instance. The second value in the result tuple is true if the
    /// database was newly created, and false if it was already present.
    pub async fn open_db(&self) -> Result<(Box<dyn Database>, bool), String> {
        match self.db_type {
            DatabaseType::RocksDb => {
                let Some(path) = self.path.clone() else {
                    return Err("Must specify path for RocksDB".to_string());
                };
                let (db, fresh) = RocksDbServer::new(path).map_err(|e| format!("{:?}", e))?;
                Ok((Box::new(db), fresh))
            }
            DatabaseType::Tuplebox => {
                let (db, fresh) = TupleBoxWorldStateSource::open(self.path.clone()).await;
                Ok((Box::new(db), fresh))
            }
        }
    }
}

impl Default for DatabaseBuilder {
    fn default() -> Self {
        Self::new()
    }
}
