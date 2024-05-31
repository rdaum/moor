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

use binary_layout::{binary_layout, Field, LayoutAs};
use okaywal::{Entry, EntryId, LogManager, SegmentReader, WriteAheadLog};
use std::fmt::{Debug, Formatter};
use std::sync::Arc;
use strum::FromRepr;
use thiserror::Error;
use tracing::{error, info, warn};

use crate::tuples::TupleId;
use crate::RelationId;

use super::page_storage::{PageStore, PageStoreMutation};
use super::{PageId, SlotId};

pub struct WalManager {
    page_storage: Arc<PageStore>,
}

impl WalManager {
    pub fn new(page_storage: Arc<PageStore>) -> Self {
        Self { page_storage }
    }
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
        let ps = self.page_storage.clone();
        ps.enqueue_page_mutations(write_batch)
            .expect("Unable to write batch");

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

        if let Err(e) = self.page_storage.enqueue_page_mutations(write_batch) {
            error!("Unable to write batch: {:?}", e);
            return Ok(());
        };

        Ok(())
    }
}

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, FromRepr)]
pub enum WalEntryType {
    // Update the page header, which is always the last operation after a commit.
    PageHeader = 0,
    // Record addition of a tuple
    Insert = 1,
    // Update
    Update = 2,
    // Record deletion of a tuple
    Delete = 3,
    // Write current state of sequences to the sequence page. Ignores page id, slot id. Data is
    // the contents of the sequence page.
    SequenceSync = 4,
}

#[derive(Error, Debug)]
pub enum WalEncodingError {
    #[error("Invalid WAL entry type: {0}")]
    InvalidType(u8),
}

impl LayoutAs<u8> for WalEntryType {
    type ReadError = WalEncodingError;
    type WriteError = WalEncodingError;

    fn try_read(v: u8) -> Result<Self, Self::ReadError> {
        Self::from_repr(v).ok_or(WalEncodingError::InvalidType(v))
    }

    fn try_write(v: Self) -> Result<u8, Self::WriteError> {
        Ok(v as u8)
    }
}

const WAL_MAGIC: u32 = 0xfeed_babe;

binary_layout!(wal_entry_header, LittleEndian, {
    // Validity marker.
    magic_marker: u32,
    // The timestamp when the write-ahead-log entry was created.
    timestamp: u64,
    // The action being taken; see WalEntryType.
    action: WalEntryType as u8,
    // The page id of the page being written
    pid: u64,
    // The slot id of the slot within the page being written.
    slot_id: u64,
    // The offset within the page of the data being written.
    offset: u64,
    // The size of the data being written.
    size: u64,
    // The relation this page belongs to
    // (Sequences don't have this, obv.)
    relation_id: u8,
});

// TODO: use builder pattern for WAL entry construction
#[allow(clippy::too_many_arguments)]
pub fn make_wal_entry<BF: FnMut(&mut [u8])>(
    typ: WalEntryType,
    page_id: PageId,
    relation_id: Option<RelationId>,
    slot_id: SlotId,
    ts: u64,
    page_offset: usize,
    page_data_size: usize,
    mut fill_func: BF,
) -> Result<Vec<u8>, WalEncodingError> {
    let mut wal_entry_buffer = vec![0; wal_entry::data::OFFSET + page_data_size];
    let mut wal_entry = wal_entry::View::new(&mut wal_entry_buffer);
    let mut wal_entry_header = wal_entry.header_mut();
    wal_entry_header
        .magic_marker_mut()
        .try_write(WAL_MAGIC)
        .expect("Failed to write magic marker");
    wal_entry_header
        .timestamp_mut()
        .try_write(ts)
        .expect("Failed to write timestamp");
    wal_entry_header
        .action_mut()
        .try_write(typ)
        .expect("Failed to write action");
    wal_entry_header
        .pid_mut()
        .try_write(page_id as u64)
        .expect("Failed to write page id");
    wal_entry_header
        .offset_mut()
        .try_write(page_offset as u64)
        .expect("Failed to write page offset");
    if let Some(relation_id) = relation_id {
        wal_entry_header
            .relation_id_mut()
            .write(relation_id.0 as u8);
    }
    wal_entry_header.slot_id_mut().write(slot_id as u64);
    wal_entry_header.size_mut().write(page_data_size as u64);

    let buffer = wal_entry.data_mut();
    fill_func(buffer);
    Ok(wal_entry_buffer)
}

binary_layout!(wal_entry, LittleEndian, {
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
        if chunk.len() < wal_entry::data::OFFSET {
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

        let action = wal_entry
            .header()
            .action()
            .try_read()
            .expect("Invalid WAL action");
        match action {
            WalEntryType::Insert | WalEntryType::Update => {
                let relation_id = RelationId(
                    wal_entry
                        .header()
                        .relation_id()
                        .try_read()
                        .expect("Invalid relation ID") as usize,
                );

                let mutation = PageStoreMutation::PageTupleWrite {
                    relation_id,
                    page_id: pid as PageId,
                    slot_id: wal_entry
                        .header()
                        .slot_id()
                        .try_read()
                        .expect("Could not read WAL slot id")
                        as SlotId,
                    page_offset: wal_entry
                        .header()
                        .offset()
                        .try_read()
                        .expect("Could not read WAL offset")
                        as usize,
                    data,
                };

                write_mutations.push(mutation);
            }
            WalEntryType::PageHeader => {
                let relation_id = RelationId(
                    wal_entry
                        .header()
                        .relation_id()
                        .try_read()
                        .expect("Invalid relation ID") as usize,
                );

                let mutation = PageStoreMutation::PageHeaderWrite {
                    relation_id,
                    page_id: pid as PageId,
                    data,
                };
                write_mutations.push(mutation);
            }

            WalEntryType::SequenceSync => {
                // Write current state of sequences to the sequence page. Ignores page id, slot id.
                // Data is the contents of the sequence page.
                write_mutations.push(PageStoreMutation::WriteSequencePage(data));
            }
            WalEntryType::Delete => {
                // Delete
                let relation_id = RelationId(wal_entry.header().relation_id().read() as usize);
                let slot_id = wal_entry.header().slot_id().read();
                write_mutations.push(PageStoreMutation::DeleteTuple(pid as PageId, relation_id));
                to_evict.push(TupleId {
                    page: pid as PageId,
                    slot: slot_id as SlotId,
                });
            }
        }
    }
}
