use std::path::PathBuf;
use std::sync::Arc;
use std::thread::spawn;

use async_trait::async_trait;
use metrics_macros::increment_counter;
use strum::VariantNames;

use moor_values::model::world_state::{WorldState, WorldStateSource};
use moor_values::model::WorldStateError;
use moor_values::SYSTEM_OBJECT;

use crate::db_client::DbTxClient;
use crate::loader::LoaderInterface;
use crate::rocksdb::tx_db_impl::oid_key;
use crate::rocksdb::tx_server::run_tx_server;
use crate::rocksdb::ColumnFamilies;
use crate::{Database, DbTxWorldState};

// Rocks implementation of 'WorldStateSource' -- opens the physical database and provides
// transactional 'WorldState' implementations for each new transaction.
pub struct RocksDbServer {
    db: Arc<rocksdb::OptimisticTransactionDB>,
}

// TODO get some metrics gauges in here to export various DB-level stats.

impl RocksDbServer {
    #[tracing::instrument()]
    pub fn new(path: PathBuf) -> Result<(Self, bool), WorldStateError> {
        let mut options = rocksdb::Options::default();
        options.create_if_missing(true);
        options.create_missing_column_families(true);
        let column_families = ColumnFamilies::VARIANTS;

        // Note: Panic if we're not able to open the database, this is a fundamental system error.
        let db: rocksdb::OptimisticTransactionDB =
            rocksdb::OptimisticTransactionDB::open_cf(&options, path, column_families)
                .expect("Unable to open database");

        // Check if the database was created by looking for objid #0. If that's present, we assume
        // the database was already created.
        let op_cf = db
            .cf_handle(column_families[ColumnFamilies::ObjectFlags as usize])
            .expect("Unable to open object flags column family");

        let was_created = db
            .get_cf(op_cf, oid_key(SYSTEM_OBJECT))
            .expect("Unable to check for database freshness")
            .is_none();

        Ok((Self { db: Arc::new(db) }, was_created))
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
    async fn new_world_state(&self) -> Result<Box<dyn WorldState>, WorldStateError> {
        // Return a transaction wrapped by the higher level RocksDbWorldState.
        let tx = self.start_transaction()?;
        Ok(Box::new(tx))
    }
}

impl Database for RocksDbServer {
    fn loader_client(&mut self) -> Result<Box<dyn LoaderInterface>, WorldStateError> {
        Ok(Box::new(self.start_transaction()?))
    }

    fn world_state_source(self: Box<Self>) -> Result<Arc<dyn WorldStateSource>, WorldStateError> {
        Ok(Arc::new(*self))
    }
}
