use std::path::PathBuf;
use std::sync::Arc;
use std::thread::spawn;

use strum::VariantNames;
use tracing::error;

use crate::db::rocksdb::tx_server::run_tx_server;
use crate::db::rocksdb::{ColumnFamilies, RocksDbTransaction};
use crate::db::state::{WorldState, WorldStateSource};

pub struct RocksDbServer {
    db: Arc<rocksdb::OptimisticTransactionDB>,
}

impl RocksDbServer {
    #[tracing::instrument()]
    pub fn new(path: PathBuf) -> Result<Self, anyhow::Error> {
        let mut options = rocksdb::Options::default();
        options.create_if_missing(true);
        options.create_missing_column_families(true);
        let column_families = ColumnFamilies::VARIANTS;
        let db: rocksdb::OptimisticTransactionDB =
            rocksdb::OptimisticTransactionDB::open_cf(&options, path, column_families)?;

        Ok(Self { db: Arc::new(db) })
    }

    #[tracing::instrument(skip(self))]
    pub fn start_transaction(&self) -> Result<RocksDbTransaction, anyhow::Error> {
        // Spawn a thread to handle the transaction, and return a mailbox to it.
        let (send, receive) = crossbeam_channel::unbounded();
        let db = self.db.clone();
        let jh = spawn(move || {
            // Open up all the column families.
            let column_families = ColumnFamilies::VARIANTS
                .iter()
                .enumerate()
                .map(|cf| db.cf_handle(cf.1).unwrap());

            let tx = db.transaction();
            let e = run_tx_server(receive, tx, column_families.collect());
            if let Err(e) = e {
                error!("System error in database transaction: {:?}", e);
            }
        });
        Ok(RocksDbTransaction {
            join_handle: jh,
            mailbox: send,
        })
    }
}

impl WorldStateSource for RocksDbServer {
    #[tracing::instrument(skip(self))]
    fn new_world_state(&mut self) -> Result<Box<dyn WorldState>, anyhow::Error> {
        // Return a transaction wrapped by the higher level RocksDbWorldState.
        let tx = self.start_transaction()?;
        Ok(Box::new(tx))
    }
}
