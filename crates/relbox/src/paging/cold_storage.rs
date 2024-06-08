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

//! TODO: replace OkayWAL with our own WAL implementation

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread::JoinHandle;

use binary_layout::{binary_layout, Field};
use crossbeam_channel::{unbounded, Receiver};
use human_bytes::human_bytes;
use okaywal::WriteAheadLog;
use tracing::{debug, error, info};

use crate::base_relation::BaseRelation;
use crate::paging::page_storage::PageStore;
use crate::paging::wal::{make_wal_entry, WalEntryType, WalManager};
use crate::paging::PageId;
use crate::paging::TupleBox;
use crate::tx::{TxTupleOp, WorkingSet};

use super::backing::{BackingStoreClient, WriterMessage};

// TODO: move "cold storage" functionality under the pager rather than above it.

/// Uses WAL + custom page store as the persistent backing store & write-ahead-log for the relbox.
pub struct ColdStorage {}

binary_layout!(sequence_page, LittleEndian, {
    // The number of sequences stored in this page.
    num_sequences: u64,
    // The sequences are here..
    sequences: [u8],
});

binary_layout!(sequence, LittleEndian, {
    // The sequence id.
    id: u64,
    // The current value of the sequence.
    value: u64,
});

const SEQUENCE_PAGE_ID: PageId = 0xfafe_babf;

impl ColdStorage {
    pub fn start(
        path: PathBuf,
        relations: &mut [BaseRelation],
        sequences: &mut [u64],
        tuple_box: Arc<TupleBox>,
    ) -> BackingStoreClient {
        let page_storage = PageStore::new(path.join("pages"));
        let wal_manager = WalManager::new(page_storage.clone());

        // Do initial recovery of anything left in the WAL before starting up, which should
        // flush everything to page storage, from which we can then go and load it.
        let wal = match WriteAheadLog::recover(path.join("wal"), wal_manager) {
            Ok(wal) => wal,
            Err(e) => {
                error!(?e, "Unable to recover write-ahead log");
                panic!("Unable to recover write-ahead log");
            }
        };

        // Grab page storage and wait for all the writes to complete.
        page_storage.wait_complete();

        // Get the sequence page, and load the sequences from it, if any.
        if let Ok(Some(sequence_page)) = page_storage.read_sequence_page() {
            let sequence_page = sequence_page::View::new(&sequence_page[..]);
            let num_sequences = sequence_page.num_sequences().read();
            assert_eq!(num_sequences, sequences.len() as u64,
                "Number of sequences in the sequence page does not match the number of sequences in the rdb");
            let sequences_bytes = sequence_page.sequences().to_vec();
            let sequence_size = sequence::SIZE.unwrap() as u64;
            for i in 0..num_sequences {
                let sequence =
                    sequence::View::new(&sequences_bytes[(i * sequence_size) as usize..]);
                let id = sequence.id().read();
                let value = sequence.value().read();
                sequences[id as usize] = value;
            }
        }

        // Recover all the pages from cold storage and re-index all the tuples in them.
        let ids = page_storage.list_pages();
        let mut restored_slots = HashMap::new();
        let mut restored_bytes = 0;
        for (page_size, page_num, relation_id) in ids {
            let tuple_ids = tuple_box
                .clone()
                .load_page(relation_id, page_num, page_size, |buf| {
                    page_storage
                        .read_page_buf(page_num, relation_id, buf)
                        .expect("Unable to read page")
                })
                .expect("Unable to get page");

            restored_slots
                .entry(relation_id)
                .or_insert_with(HashSet::new)
                .insert(tuple_ids);
            restored_bytes += page_size;
        }

        // Now iterate all the slots we restored, and re-establish their indexes in the relations they belong to.
        let mut restored_count = 0;
        for (relation_id, relation_tuple_ids) in restored_slots {
            for page_tuple_ids in relation_tuple_ids {
                for tuple_id in page_tuple_ids {
                    let relation = &mut relations[relation_id.0];
                    relation.load_tuple(tuple_id);
                    restored_count += 1;
                }
            }
        }
        info!(
            "Restored & re-indexed {} tuples from coldstorage across {} relations, in {}",
            restored_count,
            relations.len(),
            human_bytes(restored_bytes as f64)
        );

        // Start the listen loop
        let (writer_send, writer_receive) = unbounded();
        let cs_join = Self::start_listen_loop(
            writer_receive,
            wal.clone(),
            tuple_box.clone(),
            page_storage.clone(),
        );

        // And return the client to it.
        BackingStoreClient::new(writer_send, cs_join)
    }

    fn start_listen_loop(
        writer_receive: Receiver<WriterMessage>,
        wal: WriteAheadLog,
        tuple_box: Arc<TupleBox>,
        ps: Arc<PageStore>,
    ) -> JoinHandle<()> {
        std::thread::Builder::new()
            .name("moor-coldstorage-listen".to_string())
            .spawn(move || Self::listen_loop(writer_receive, wal, tuple_box, ps))
            .expect("Unable to spawn coldstorage listen thread")
    }

    fn listen_loop(
        writer_receive: Receiver<WriterMessage>,
        wal: WriteAheadLog,
        tuple_box: Arc<TupleBox>,
        ps: Arc<PageStore>,
    ) {
        ps.clone().start();
        loop {
            match writer_receive.recv() {
                Ok(WriterMessage::Commit(ts, ws, sequences)) => {
                    Self::perform_writes(wal.clone(), tuple_box.clone(), ts, ws, sequences);
                }
                Ok(WriterMessage::Shutdown) => {
                    // Flush the WAL
                    wal.shutdown().expect("Unable to flush WAL");

                    info!("Shutting down WAL writer thread");
                    break;
                }
                Err(e) => {
                    error!(?e, "Error receiving message from writer thread");
                    break;
                }
            }
        }

        // Shut down the eventfd thread.
        ps.stop();
    }

    /// Receive an (already committed) working set and write the modified pages out to the write-ahead-log to make
    /// the changes durable.
    fn perform_writes(
        wal: WriteAheadLog,
        tuple_box: Arc<TupleBox>,
        ts: u64,
        ws: WorkingSet,
        sequences: Vec<u64>,
    ) {
        debug!("Committing write-ahead for ts {}", ts);

        // Where we stick all the page mutations we're going to write out.
        let mut write_batch = vec![];

        // TODO: sequences shouldn't mutate if they haven't changed during the
        //   transaction, so we need some kind of signal from above that they have
        //   changed.

        // Build the sequence page first, by copying the current values of all the
        // sequences into it.
        let seq_size = sequence::SIZE.unwrap();
        let seq_page_size = sequence_page::sequences::OFFSET;
        let seq_wal_entry = make_wal_entry(
            WalEntryType::SequenceSync,
            SEQUENCE_PAGE_ID as PageId,
            None,
            0,
            ts,
            0,
            seq_page_size + (seq_size * sequences.len()),
            |buf| {
                let mut sequence_page = sequence_page::View::new(buf);
                sequence_page
                    .num_sequences_mut()
                    .write(sequences.len() as u64);
                for (i, sequence_value) in sequences.iter().enumerate() {
                    let mut sequence = sequence::View::new(
                        &mut sequence_page.sequences_mut()[i * seq_size..(i + 1) * seq_size],
                    );
                    sequence.id_mut().write(i as u64);
                    sequence.value_mut().write(*sequence_value);
                }
            },
        )
        .expect("Failed to encode sequence WAL entry");
        write_batch.push((SEQUENCE_PAGE_ID, Some(seq_wal_entry)));

        // Now iterate over all the tuples referred to in the working set and produce WAL entries for them.
        let mut dirty_pages = HashSet::new();
        for r in ws.relations.iter() {
            for t in r.1.tuples() {
                match &t.op {
                    TxTupleOp::Insert(new_tuple) => {
                        let (tuple_offset, tuple_size) = tuple_box
                            .page_for(new_tuple.id().page)
                            .expect("Unable to get page for tuple")
                            .offset_of(new_tuple.id().slot)
                            .expect("Unable to get tuple offset");
                        let slotbuf = new_tuple.slot_buffer();
                        assert!(tuple_size >= slotbuf.len(), "Tuple size too small");
                        let wal_entry_buffer = make_wal_entry(
                            WalEntryType::Insert,
                            new_tuple.id().page,
                            Some(r.1.id),
                            new_tuple.id().slot,
                            ts,
                            tuple_offset,
                            slotbuf.len(),
                            |buf| buf.copy_from_slice(slotbuf.as_slice()),
                        )
                        .expect("Failed to encode insert WAL entry");

                        write_batch.push((new_tuple.id().page, Some(wal_entry_buffer)));
                        dirty_pages.insert((new_tuple.id().page, r.1.id));
                    }
                    TxTupleOp::Update {
                        from_tuple: old_tuple,
                        to_tuple: new_tuple,
                    } => {
                        let (tuple_offset, tuple_size) = tuple_box
                            .page_for(new_tuple.id().page)
                            .expect("Unable to get page for tuple")
                            .offset_of(new_tuple.id().slot)
                            .expect("Unable to get tuple offset");
                        let slotbuf = new_tuple.slot_buffer();
                        assert!(tuple_size >= slotbuf.len(), "Tuple size too small");
                        let wal_entry_buffer = make_wal_entry(
                            WalEntryType::Update,
                            new_tuple.id().page,
                            Some(r.1.id),
                            new_tuple.id().slot,
                            ts,
                            tuple_offset,
                            slotbuf.len(),
                            |buf| buf.copy_from_slice(slotbuf.as_slice()),
                        )
                        .expect("Failed to encode update WAL entry");

                        write_batch.push((new_tuple.id().page, Some(wal_entry_buffer)));
                        dirty_pages.insert((old_tuple.id().page, r.1.id));
                    }
                    TxTupleOp::Tombstone(tref, _) => {
                        let tuple_id = tref.id();
                        let wal_entry_buffer = make_wal_entry(
                            WalEntryType::Delete,
                            tuple_id.page,
                            Some(r.1.id),
                            tuple_id.slot,
                            ts,
                            0,
                            0,
                            |_| (),
                        )
                        .expect("Failed to encode tombstone WAL entry");
                        write_batch.push((tuple_id.page, Some(wal_entry_buffer)));
                        dirty_pages.insert((tuple_id.page, r.1.id));
                    }
                    TxTupleOp::Value(_) => {
                        // Untouched value (view), noop, should already exist in backing store.
                    }
                }
            }
        }

        // Now write out the updated page headers for the dirty pages
        for (page_id, r) in &dirty_pages {
            // Get the slotboxy page for this tuple.
            let Ok(page) = tuple_box.page_for(*page_id) else {
                // If the slot or page is already gone, ce la vie, we don't need to sync it.
                continue;
            };

            // Copy the page into the WAL entry directly.
            let wal_entry_buffer = make_wal_entry(
                WalEntryType::PageHeader,
                *page_id,
                Some(*r),
                0, /* not used */
                ts,
                0,
                page.header_size(),
                |buf| page.write_header(buf),
            )
            .expect("Failed to encode page index WAL entry");
            write_batch.push((*page_id, Some(wal_entry_buffer)));
        }

        let mut sync_wal = wal.begin_entry().expect("Failed to begin WAL entry");
        for (_page_id, wal_entry_buf) in write_batch {
            if let Some(wal_entry_buf) = wal_entry_buf {
                sync_wal
                    .write_chunk(&wal_entry_buf)
                    .expect("Failed to write to WAL");
            }
        }
        sync_wal.commit().expect("Failed to commit WAL entry");
    }
}
