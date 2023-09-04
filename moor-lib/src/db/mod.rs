use anyhow::bail;
use std::sync::Arc;
use std::thread;

use strum::{Display, EnumIter, EnumString, EnumVariantNames};

use crate::db::db_client::DbTxClient;
use crate::db::inmemtransient::InMemTransientDatabase;
use crate::db::loader::LoaderInterface;
use crate::db::rocksdb::db_server::RocksDbServer;
use moor_value::model::world_state::WorldStateSource;
use moor_value::model::WorldStateError;

pub mod matching;

mod db_client;
mod db_loader_client;
mod db_message;
mod db_worldstate;
pub mod inmemtransient;
pub mod loader;
pub mod match_env;
pub mod mock;
pub mod rocksdb;

pub struct DbTxWorldState {
    pub join_handle: thread::JoinHandle<()>,
    client: DbTxClient,
}

/// Enumeration of potential database backends.
#[derive(Debug, Display, EnumString, EnumVariantNames, EnumIter, Clone, Copy)]
pub enum DatabaseType {
    /// Persistent transactional RocksDB backend.
    RocksDb,
    /// Transient, non-transactional, in-memory only. Useful for testing only.
    InMemTransient,
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

    pub fn open_db(&self) -> Result<Box<dyn Database>, anyhow::Error> {
        match self.db_type {
            DatabaseType::RocksDb => {
                let Some(path) = self.path.clone() else {
                    bail!("Must specify path for RocksDB");
                };
                let db = RocksDbServer::new(path)?;
                Ok(Box::new(db))
            }
            DatabaseType::InMemTransient => {
                let db = InMemTransientDatabase::new();
                Ok(Box::new(db))
            }
        }
    }
}
