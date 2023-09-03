use std::path::PathBuf;
use std::sync::Arc;
use std::thread::spawn;

use async_trait::async_trait;
use metrics_macros::increment_counter;
use strum::VariantNames;

use moor_value::model::world_state::{WorldState, WorldStateSource};
use moor_value::model::WorldStateError;

use crate::db::db_client::DbTxClient;
use crate::db::rocksdb::tx_server::run_tx_server;
use crate::db::rocksdb::ColumnFamilies;
use crate::db::DbTxWorldState;

// Rocks implementation of 'WorldStateSource' -- opens the physical database and provides
// transactional 'WorldState' implementations for each new transaction.
pub struct RocksDbServer {
    db: Arc<rocksdb::OptimisticTransactionDB>,
}

// TODO get some metrics gauges in here to export various DB-level stats.

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
    pub fn start_transaction(&self) -> Result<DbTxWorldState, WorldStateError> {
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
            increment_counter!("rocksdb.start_transaction_server");
            run_tx_server(receive, tx, column_families.collect()).expect("Error running tx server");
        });
        Ok(DbTxWorldState {
            join_handle: jh,
            client: DbTxClient::new(send),
        })
    }
}

#[async_trait]
impl WorldStateSource for RocksDbServer {
    #[tracing::instrument(skip(self))]
    async fn new_world_state(&mut self) -> Result<Box<dyn WorldState>, WorldStateError> {
        // Return a transaction wrapped by the higher level RocksDbWorldState.
        let tx = self.start_transaction()?;
        Ok(Box::new(tx))
    }
}
