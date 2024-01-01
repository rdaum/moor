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

//! RocksDB implementation of a backing-store for relations.

use std::path::PathBuf;
use std::sync::Arc;

use rocksdb::{IteratorMode, DB};
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::{debug, error, info};

use crate::tuplebox::backing::{BackingStoreClient, WriterMessage};
use crate::tuplebox::base_relation::BaseRelation;
use crate::tuplebox::tb::RelationInfo;
use crate::tuplebox::tuples::TxTuple;
use crate::tuplebox::tx::working_set::WorkingSet;

/// In lieu of a proper write-ahead-log and our own pager and backing store (in development), we're
/// going to use RocksDB -- at least temporarily -- as a place to store the current canonical relation
/// set at commit time.
///
/// Rocks is really overkill for this, we don't need anything more than a non-transactional disk
/// based hash table + WAL. But Rust bindings for Rocks are stable, and Rocks itself fairly proven.
///
/// At each commit, the full set of committed changes from the working set are flushed out to
/// RocksDB column families. This is basically a copy of the WorkingSet of the "winning" transaction
/// at commit time. Concurrent commits are serialized here such that the most recent commit wins,
/// and all other commits are aborted, even mid-stream, when a new one comes in. The last write
/// wins.
///
/// At startup, we'll read the current canonical relation set from RocksDB and use that as the
/// starting point for the in-memory copies of the tuples. For now.
///
/// Notably, we're not using the transactional facilities of RocksDB here. There is only one writer
/// at a time.

pub struct RocksBackingStore {
    db: DB,
    schema: Vec<RelationInfo>,
}

impl RocksBackingStore {
    pub async fn start(
        path: PathBuf,
        schema: Vec<RelationInfo>,
        relations: &mut Vec<BaseRelation>,
        sequences: &mut Vec<u64>,
    ) -> BackingStoreClient {
        let mut options = rocksdb::Options::default();
        options.create_if_missing(true);
        options.create_missing_column_families(true);

        let mut column_families: Vec<&str> = schema.iter().map(|r| r.name.as_str()).collect();
        column_families.push("Sequences");

        // Note: Panic if we're not able to open the database, this is a fundamental system error.
        let db = DB::open_cf(&options, path, &column_families).expect("Unable to open database");

        // Load all the existing tuples into memory...
        let mut tuplecnt = 0;
        for (relation_id, relation) in relations.iter_mut().enumerate() {
            let cf = db
                .cf_handle(&column_families[relation_id])
                .expect("could not open column family for relation");
            let it = db.iterator_cf(&cf, IteratorMode::Start);
            for item in it {
                let (key, value) = item.expect("Could not retrieve tuple");
                relation.insert_tuple(&key, &value);
                tuplecnt += 1;
            }
        }
        info!("Finished loading {} tuples.", tuplecnt);

        // Load sequences cur values from the sequences column family
        let seq_cf = db
            .cf_handle("Sequences")
            .expect("Unable to open sequences column family");
        for (seq_number, sequence) in sequences.iter_mut().enumerate() {
            let seq_val = db
                .get_cf(seq_cf, format!("sequence_{}", seq_number))
                .expect("Could not read from seq CF");
            if let Some(seq_val) = seq_val {
                *sequence = u64::from_le_bytes(seq_val[..].try_into().unwrap());
            }
        }

        let bs = Arc::new(RocksBackingStore { db, schema });

        let (writer_send, writer_receive) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(bs.clone().listen_loop(writer_receive));

        BackingStoreClient::new(writer_send)
    }

    async fn listen_loop(self: Arc<Self>, mut writer_receive: UnboundedReceiver<WriterMessage>) {
        loop {
            let (abort_send, abort_receive) = tokio::sync::watch::channel(0);
            let bs = self.clone();
            match writer_receive.recv().await {
                Some(WriterMessage::Commit(ts, ws, sequences)) => {
                    debug!("Committing write-ahead for ts {}", ts);
                    abort_send.send(ts).unwrap();
                    tokio::spawn(bs.perform_writes(ts, ws, sequences, abort_receive.clone()));
                }
                Some(WriterMessage::Shutdown) => {
                    info!("Shutting down RocksDB writer thread");
                    return;
                }
                None => {
                    error!("Channel closed in RocksDB writer thread");
                    return;
                }
            }
        }
    }

    async fn perform_writes(
        self: Arc<Self>,
        ts: u64,
        committed_working_set: WorkingSet,
        current_sequences: Vec<u64>,
        abort: tokio::sync::watch::Receiver<u64>,
    ) {
        // Write the current state of sequences first.
        let seq_cf = self
            .db
            .cf_handle("Sequences")
            .expect("Unable to open sequences column family");
        for (seq_number, sequence) in current_sequences.iter().enumerate() {
            self.db
                .put_cf(
                    seq_cf,
                    format!("sequence_{}", seq_number),
                    sequence.to_le_bytes(),
                )
                .expect("Could not write seq CF");
        }

        // Go through the modified tuples and mutate the underlying column families
        for (relation_id, local_relation) in committed_working_set.relations.iter().enumerate() {
            let relation_info = &self.schema[relation_id];
            let cf = self
                .db
                .cf_handle(relation_info.name.as_str())
                .expect("Unable to open column family");
            for tuple in local_relation.tuples() {
                if let Ok(true) = abort.has_changed() {
                    let new_ts = abort.borrow();
                    if *new_ts != ts {
                        debug!(
                            "Aborting write-ahead due to abort flag flip from {} to {:?}",
                            ts, new_ts
                        );
                        return;
                    }
                }
                match &tuple {
                    TxTuple::Insert(t) | TxTuple::Update(t) => {
                        let v = t.get();
                        self.db
                            .put_cf(cf, v.domain().as_slice(), v.codomain().as_slice())
                            .expect("Unable to sync tuple to backing store");
                    }
                    TxTuple::Value(_) => {
                        // No-op, this should already exist in the backing store.
                        continue;
                    }
                    TxTuple::Tombstone { ts: _, domain: d } => {
                        self.db
                            .delete_cf(cf, d.as_slice())
                            .expect("Unable to delete tuple from backing store");
                    }
                }
            }
        }
    }
}
