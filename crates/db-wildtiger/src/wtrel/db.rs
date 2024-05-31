// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use std::path::Path;
use std::sync::atomic::AtomicI64;
use std::sync::Arc;

use tracing::{debug, info};

use crate::bindings::{
    Connection, CursorConfig, Datum, Error, Isolation, LogConfig, OpenConfig, SessionConfig,
    SyncMethod, TransactionConfig, TransactionSync,
};
use crate::wtrel::rel_transaction::WiredTigerRelTransaction;
use crate::wtrel::relation::WiredTigerRelation;

pub const MAX_NUM_SEQUENCES: usize = 32;

pub struct WiredTigerRelDb<TableType>
where
    TableType: WiredTigerRelation,
    TableType: Copy,
{
    connection: Connection,
    sequence_table: TableType,

    /// The current value of sequences. Which are loaded on startup, and periodically flushed to
    /// table independent of transaction.
    sequences: Arc<[AtomicI64; MAX_NUM_SEQUENCES]>,
}

impl<TableType> WiredTigerRelDb<TableType>
where
    TableType: WiredTigerRelation,
    TableType: Copy,
{
    pub fn new(path: &Path, sequence_table: TableType, transient: bool) -> Arc<Self> {
        // The directory needs to exist if not already there.
        std::fs::create_dir_all(path).expect("Failed to create database directory");

        // TODO: provide an options struct for configuration of cache size, and durability mode.
        //   esp with durability, some users may not care about full fsync durable transactions,
        //     and would be willing to live with checkpoint-only durability.
        let options = OpenConfig::new()
            .create(true)
            .cache_cursors(true)
            .in_memory(transient)
            .cache_size(1 << 30)
            .log(LogConfig::new().enabled(true))
            .transaction_sync(
                TransactionSync::new()
                    .enabled(true)
                    .method(SyncMethod::Fsync),
            );
        let connection = Connection::open(path, options).unwrap();

        let sequences = Arc::new([(); MAX_NUM_SEQUENCES].map(|_| AtomicI64::new(0)));

        Arc::new(WiredTigerRelDb {
            connection,
            sequence_table,
            sequences,
        })
    }

    pub fn create_tables(&self) {
        let session = self
            .connection
            .open_session(SessionConfig::new().isolation(Isolation::Snapshot))
            .unwrap();
        session.begin_transaction(None).unwrap();
        TableType::create_tables(&session);
        session.commit().unwrap();
    }

    pub fn start_tx(&self) -> WiredTigerRelTransaction<TableType> {
        let session_config = SessionConfig::new().isolation(Isolation::Snapshot);
        let session = self.connection.open_session(session_config).unwrap();
        let tx_config = TransactionConfig::new();
        session.begin_transaction(Some(tx_config)).unwrap();
        info!("Starting transaction...");
        WiredTigerRelTransaction::new(session, self.sequences.clone())
    }

    pub fn load_sequences(&self) {
        let session = self
            .connection
            .open_session(SessionConfig::new().isolation(Isolation::Snapshot))
            .unwrap();

        // Preload the sequences from the sequence table.
        // This is a full scan of the table, pre-loading the vector of sequences.
        let cursor = session
            .open_cursor(
                &self.sequence_table.into(),
                Some(CursorConfig::new().readonly(true)),
            )
            .unwrap();
        cursor.reset().unwrap();
        loop {
            match cursor.next() {
                Ok(_) => {
                    let key = cursor.get_key().unwrap();
                    let value = cursor.get_value().unwrap();
                    let sequence = usize::from_le_bytes(
                        key.as_slice().try_into().expect("Invalid sequence key"),
                    );
                    self.sequences[sequence].store(
                        i64::from_le_bytes(
                            value.as_slice().try_into().expect("Invalid sequence value"),
                        ),
                        std::sync::atomic::Ordering::Relaxed,
                    );
                }
                Err(Error::NotFound) => break,
                Err(e) => panic!("Unexpected error: {:?}", e),
            }
        }
    }
    pub fn sync_sequences(&self) {
        debug!("Syncing sequences...");
        let session_config = SessionConfig::new().isolation(Isolation::Snapshot);
        let session = self.connection.open_session(session_config).unwrap();
        let tx_config = TransactionConfig::new();
        session.begin_transaction(Some(tx_config)).unwrap();
        let cursor = session
            .open_cursor(
                &self.sequence_table.into(),
                Some(CursorConfig::new().overwrite(true)),
            )
            .unwrap();
        for (sequence, value) in self.sequences.iter().enumerate() {
            let key = sequence.to_le_bytes();
            let value = value
                .load(std::sync::atomic::Ordering::Relaxed)
                .to_le_bytes();
            cursor.set_key(Datum::from_vec(key.to_vec())).unwrap();
            cursor.set_value(Datum::from_vec(value.to_vec())).unwrap();
            cursor.update().unwrap();
        }
        session.commit().unwrap();
    }
}

impl<TableType: Copy> Drop for WiredTigerRelDb<TableType>
where
    TableType: WiredTigerRelation,
{
    fn drop(&mut self) {
        debug!("Synchronizing sequences...");
        self.sync_sequences();
    }
}
