use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread::spawn;

use strum::VariantNames;
use tracing::error;

use crate::db::rocksdb::tx_server::run_tx_server;
use crate::db::rocksdb::{ColumnFamilies, RocksDbTransaction};
use crate::model::permissions::PermissionsContext;
use crate::model::world_state::{WorldState, WorldStateSource};
use moor_value::var::objid::Objid;

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

#[async_trait]
impl WorldStateSource for RocksDbServer {
    #[tracing::instrument(skip(self))]
    async fn new_world_state(
        &mut self,
        player: Objid,
    ) -> Result<(Box<dyn WorldState>, PermissionsContext), anyhow::Error> {
        // Return a transaction wrapped by the higher level RocksDbWorldState.
        let mut tx = self.start_transaction()?;
        let player_flags = tx.flags_of(player).await?;
        let player_permissions = PermissionsContext::root_for(player, player_flags);
        Ok((Box::new(tx), player_permissions))
    }
}
