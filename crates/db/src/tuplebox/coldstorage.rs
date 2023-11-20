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

//! TODO: replace OkayWAL with our own implementation, using io_uring.

use std::fmt::{Debug, Formatter};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use binary_layout::{define_layout, Field, LayoutAs};
use im::{HashMap, HashSet};
use okaywal::{Entry, EntryId, LogManager, SegmentReader, WriteAheadLog};
use strum::FromRepr;
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::{debug, error, info, warn};

use crate::tuplebox::backing::{BackingStoreClient, WriterMessage};
use crate::tuplebox::base_relation::BaseRelation;
use crate::tuplebox::page_storage::{PageStore, PageStoreMutation};
use crate::tuplebox::slots::{PageId, SlotBox, SlotId, TupleId};
use crate::tuplebox::tb::RelationInfo;
use crate::tuplebox::tuples::TxTuple;
use crate::tuplebox::tx::working_set::WorkingSet;
use crate::tuplebox::RelationId;

/// Uses OkayWal + Marble as the persistent backing store & write-ahead-log for the tuplebox.
pub struct ColdStorage {}

define_layout!(sequence_page, LittleEndian, {
    // The number of sequences stored in this page.
    num_sequences: u64,
    // The sequences are here..
    sequences: [u8],
});

define_layout!(sequence, LittleEndian, {
    // The sequence id.
    id: u64,
    // The current value of the sequence.
    value: u64,
});

const SEQUENCE_PAGE_ID: PageId = 0xfafe_babf;

impl ColdStorage {
    pub async fn start(
        path: PathBuf,
        _schema: &[RelationInfo],
        relations: &mut [BaseRelation],
        sequences: &mut Vec<u64>,
        slot_box: Arc<SlotBox>,
    ) -> BackingStoreClient {
        let page_storage = Arc::new(Mutex::new(PageStore::new(path.join("pages"))));
        let wal_manager = WalManager {
            page_storage: page_storage.clone(),
            slot_box: slot_box.clone(),
        };
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
        let mut cs = page_storage.lock().unwrap();
        cs.wait_complete();

        // Get the sequence page, and load the sequences from it, if any.
        if let Ok(Some(sequence_page)) = cs.read_sequence_page() {
            let sequence_page = sequence_page::View::new(&sequence_page[..]);
            let num_sequences = sequence_page.num_sequences().read();
            assert_eq!(num_sequences, sequences.len() as u64,
                "Number of sequences in the sequence page does not match the number of sequences in the tuplebox");
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
        let ids = cs.list_pages();
        let mut restored_slots = HashMap::new();
        let mut restored_bytes = 0;
        for (page_size, page_num, relation_id) in ids {
            let sb_page = slot_box.page_for(page_num);
            let slot_ids = sb_page.load(|buf| {
                cs.read_page_buf(page_num, relation_id, buf)
                    .expect("Unable to read page")
            });
            // The allocator needs to know that this page is used.
            slot_box.mark_page_used(relation_id, sb_page.free_space_bytes(), page_num);
            restored_slots
                .entry(relation_id)
                .or_insert_with(HashSet::new)
                .insert((page_num, slot_ids));
            restored_bytes += page_size;
        }

        // Now iterate all the slots we restored, and re-establish their indexes in the relations they belong to.
        let mut restored_count = 0;
        for (relation_id, pages) in restored_slots {
            for (page_num, slot_ids) in pages {
                let relation = &mut relations[relation_id.0];
                for slot_id in slot_ids {
                    let tuple_id: TupleId = (page_num, slot_id);
                    relation.index_tuple(tuple_id);
                    restored_count += 1;
                }
            }
        }
        info!(
            "Restored & re-indexed {} tuples from coldstorage across {} relations, in {} bytes",
            restored_count,
            relations.len(),
            restored_bytes
        );

        // Start the listen loop
        let (writer_send, writer_receive) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(Self::listen_loop(writer_receive, wal, slot_box.clone()));

        // And return the client to it.
        BackingStoreClient::new(writer_send)
    }

    async fn listen_loop(
        mut writer_receive: UnboundedReceiver<WriterMessage>,
        wal: WriteAheadLog,
        slot_box: Arc<SlotBox>,
    ) {
        loop {
            match writer_receive.recv().await {
                Some(WriterMessage::Commit(ts, ws, sequences)) => {
                    Self::perform_writes(wal.clone(), slot_box.clone(), ts, ws, sequences).await;
                }
                Some(WriterMessage::Shutdown) => {
                    // Flush the WAL
                    wal.shutdown().expect("Unable to flush WAL");

                    info!("Shutting down WAL writer thread");
                    return;
                }
                None => {
                    error!("Writer thread channel closed, shutting down");
                    return;
                }
            }
        }
    }

    /// Receive an (already committed) working set and write the modified pages out to the write-ahead-log to make
    /// the changes durable.
    async fn perform_writes(
        wal: WriteAheadLog,
        slot_box: Arc<SlotBox>,
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
        );
        write_batch.push((SEQUENCE_PAGE_ID, Some(seq_wal_entry)));

        // Now iterate over all the tuples referred to in the working set.
        // For syncing pages, we don't need to sync each individual tuple, we we just find the set of dirty pages
        // and sync them.
        // The pages that are modified will be need be read-locked while they are copied.
        let mut dirty_tuple_count = 0;
        let mut dirty_pages = HashSet::new();
        for r in &ws.relations {
            for t in r.tuples() {
                match t {
                    TxTuple::Insert(_) | TxTuple::Update(_) | TxTuple::Tombstone { .. } => {
                        dirty_tuple_count += 1;
                        let (page_id, _slot_id) = t.tuple_id();
                        dirty_pages.insert((page_id, r.id));
                    }
                    TxTuple::Value(_) => {
                        // Untouched value (view), noop, should already exist in backing store.
                    }
                }
            }
        }

        let mut total_synced_tuples = 0;

        for (page_id, r) in &dirty_pages {
            // Get the page for this tuple.
            let page = slot_box.page_for(*page_id);
            total_synced_tuples += page.num_active_slots();

            // Copy the page into the WAL entry directly.
            let wal_entry_buffer = make_wal_entry(
                WalEntryType::PageSync,
                *page_id,
                Some(*r),
                0,
                ts,
                page.page_size,
                |buf| page.save_into(buf),
            );
            write_batch.push((*page_id, Some(wal_entry_buffer)));
        }

        let mut total_tuples = 0;
        for p in slot_box.used_pages() {
            let page = slot_box.page_for(p);
            total_tuples += page.num_active_slots();
        }

        debug!(
            dirty_tuple_count,
            dirt_pages = dirty_pages.len(),
            num_relations = ws.relations.len(),
            total_synced_tuples,
            total_tuples,
            "Syncing dirty pages to WAL"
        );

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

struct WalManager {
    // TODO: having a lock on the cold storage should not be necessary, but it is not !Sync, despite
    //  it supposedly being thread safe.
    page_storage: Arc<Mutex<PageStore>>,
    slot_box: Arc<SlotBox>,
}

impl Debug for WalManager {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WalManager").finish()
    }
}

impl LogManager for WalManager {
    fn recover(&mut self, entry: &mut Entry<'_>) -> std::io::Result<()> {
        let Some(chunks) = entry.read_all_chunks()? else {
            info!("No chunks found, nothing to recover");
            return Ok(());
        };
        let mut write_batch = vec![];
        let mut evicted = vec![];
        for chunk in chunks {
            Self::chunk_to_mutations(&chunk, &mut write_batch, &mut evicted);
        }
        let mut cs = self.page_storage.lock().unwrap();
        cs.write_batch(write_batch).expect("Unable to write batch");
        Ok(())
    }

    fn checkpoint_to(
        &mut self,
        _last_checkpointed_id: EntryId,
        checkpointed_entries: &mut SegmentReader,
        _wal: &WriteAheadLog,
    ) -> std::io::Result<()> {
        // Replay the write-ahead log into cold storage, to make sure that it's consistent.
        let mut write_batch = vec![];
        let mut evicted = vec![];

        while let Some(mut entry) = checkpointed_entries.read_entry()? {
            let chunks = match entry.read_all_chunks() {
                Ok(c) => c,
                Err(e) => {
                    error!(?e, "Failed to read chunks from entry");
                    continue;
                }
            };
            let Some(chunks) = chunks else {
                continue;
            };
            for chunk in chunks {
                Self::chunk_to_mutations(&chunk, &mut write_batch, &mut evicted);
            }
        }

        let Ok(mut cs) = self.page_storage.lock() else {
            error!("Unable to lock cold storage");
            return Ok(());
        };
        if let Err(e) = cs.write_batch(write_batch) {
            error!("Unable to write batch: {:?}", e);
            return Ok(());
        };

        for tuple_id in evicted {
            if let Err(e) = self.slot_box.remove(tuple_id) {
                warn!(?tuple_id, e = ?e, "Failed to evict page from slot box");
            }
        }

        Ok(())
    }
}

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, FromRepr)]
pub enum WalEntryType {
    // Sync, write the updated data in the page.
    PageSync = 0,
    // Delete the page.
    Delete = 1,
    // Write current state of sequences to the sequence page. Ignores page id, slot id. Data is
    // the contents of the sequence page.
    SequenceSync = 2,
}

impl LayoutAs<u8> for WalEntryType {
    fn read(v: u8) -> Self {
        Self::from_repr(v).unwrap()
    }

    fn write(v: Self) -> u8 {
        v as u8
    }
}

const WAL_MAGIC: u32 = 0xfeed_babe;

define_layout!(wal_entry_header, LittleEndian, {
    // Validity marker.
    magic_marker: u32,
    // The timestamp when the write-ahead-log entry was created.
    timestamp: u64,
    // The action being taken; see WalEntryType.
    action: WalEntryType as u8,
    // The page id of the page being written
    pid: u64,
    // The relation this page belongs to, if this is a pagesync type
    // (Sequences don't have this, obv.)
    relation_id: u8,
    // The slot id of the slot within the page being written.
    // Note we always sync full pages, but we may delete a single slot.
    slot_id: u64,

    // The size of the data being written.
    size: u64,
});

fn make_wal_entry<BF: FnMut(&mut [u8])>(
    typ: WalEntryType,
    page_id: PageId,
    relation_id: Option<RelationId>,
    slot_id: SlotId,
    ts: u64,
    page_size: usize,
    mut fill_func: BF,
) -> Vec<u8> {
    let mut wal_entry_buffer = vec![0; wal_entry::data::OFFSET + page_size];
    let mut wal_entry = wal_entry::View::new(&mut wal_entry_buffer);
    let mut wal_entry_header = wal_entry.header_mut();
    wal_entry_header.magic_marker_mut().write(WAL_MAGIC);
    wal_entry_header.timestamp_mut().write(ts);
    wal_entry_header.action_mut().write(typ);
    wal_entry_header.pid_mut().write(page_id as u64);
    if let Some(relation_id) = relation_id {
        wal_entry_header
            .relation_id_mut()
            .write(relation_id.0 as u8);
    }
    wal_entry_header.slot_id_mut().write(slot_id as u64);
    wal_entry_header.size_mut().write(page_size as u64);
    fill_func(wal_entry.data_mut());
    wal_entry_buffer
}

define_layout!(wal_entry, LittleEndian, {
    header: wal_entry_header::NestedView,
    // The entire buffer frame for the page being written, except for delete.
    data: [u8],
});

impl WalManager {
    fn chunk_to_mutations(
        chunk: &[u8],
        write_mutations: &mut Vec<PageStoreMutation>,
        to_evict: &mut Vec<TupleId>,
    ) {
        // The first N bytes have to be WAL_MAGIC or this is an invalid chunk.
        if chunk.len() < wal_entry::header::OFFSET {
            warn!("Chunk is too small to be valid");
            return;
        }
        if chunk[0..4] != WAL_MAGIC.to_le_bytes() {
            warn!("Chunk does not have valid magic marker");
            return;
        }
        let wal_entry = wal_entry::View::new(&chunk);
        if wal_entry.header().magic_marker().read() != WAL_MAGIC {
            warn!("Chunk does not have valid magic marker");
            return;
        }
        let pid = wal_entry.header().pid().read();

        // Copied onto heap so we can pass it to the write batch without it getting moved around,
        // because the kernel will need a stable pointer to it.
        let data = wal_entry.data().to_vec().into_boxed_slice();

        let action = wal_entry.header().action().read();
        match action {
            WalEntryType::PageSync => {
                // Sync, write the updated data.
                // The relation # is the first part of the id, its lower 8 bits. The remainder is the
                // page number.
                let relation_id = RelationId(wal_entry.header().relation_id().read() as usize);

                write_mutations.push(PageStoreMutation::SyncRelationPage(
                    relation_id,
                    pid as PageId,
                    data,
                ));
            }
            WalEntryType::SequenceSync => {
                // Write current state of sequences to the sequence page. Ignores page id, slot id.
                // Data is the contents of the sequence page.
                write_mutations.push(PageStoreMutation::SyncSequencePage(data));
            }
            WalEntryType::Delete => {
                // Delete
                let relation_id = RelationId(wal_entry.header().relation_id().read() as usize);
                let slot_id = wal_entry.header().slot_id().read();
                write_mutations.push(PageStoreMutation::DeleteRelationPage(
                    pid as PageId,
                    relation_id,
                ));
                to_evict.push((pid as PageId, slot_id as SlotId));
            }
        }
    }
}
