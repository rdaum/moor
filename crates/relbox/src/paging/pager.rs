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

//! The Pager fronts the BufferPool and issues page ids & fault-able page ptrs
//! It is responsible for managing pages; bringing them in and out of memory
//! from disk, providing locks on them, etc.

use crate::pool::BufferPool;
use crate::{
    base_relation::BaseRelation,
    pool::{Bid, BufferPoolError, MmapBufferPool},
    tx::WorkingSet,
};
use dashmap::DashMap;
use std::{
    path::PathBuf,
    pin::Pin,
    sync::{
        atomic::{AtomicPtr, AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

use super::{backing::BackingStoreClient, cold_storage::ColdStorage, PageId, TupleBox};

pub struct Pager {
    inner: Inner,
    next_pid: AtomicUsize,
    cold_storage: Mutex<Option<BackingStoreClient>>,
}

struct Inner {
    pool: MmapBufferPool,
    page_table: DashMap<PageId, Bid>,
}

impl Pager {
    /// Construct a pager with the resident maximum memory size of `size` bytes.
    pub fn new(size: usize) -> Result<Self, BufferPoolError> {
        let pool = MmapBufferPool::new(size)?;

        Ok(Self {
            inner: Inner {
                pool,
                page_table: DashMap::new(),
            },
            cold_storage: Mutex::new(None),
            next_pid: AtomicUsize::new(0),
        })
    }

    /// Restore pages and the tuples they contain, and the indexes to those tuples, and set up
    /// the pager to use the provided directory for cold storage.
    pub fn open(
        &self,
        path: PathBuf,
        relations: &mut [BaseRelation],
        sequences: &mut [u64],
        tuple_box: Arc<TupleBox>,
    ) -> Result<(), BufferPoolError> {
        let mut cs = self.cold_storage.lock().unwrap();
        (*cs) = Some(ColdStorage::start(
            path,
            relations,
            sequences,
            tuple_box.clone(),
        ));

        Ok(())
    }

    /// Allocate a page, and fill it with the provided function.
    pub fn alloc<F>(
        &self,
        size: usize,
        mut fill_func: F,
    ) -> Result<(PageId, usize), BufferPoolError>
    where
        F: FnMut(Pin<&mut [u8]>),
    {
        let (bid, buf_ptr, used_size) = self.inner.pool.alloc(size)?;
        let as_slice = unsafe { std::slice::from_raw_parts_mut(buf_ptr, size) };
        let mut as_pin = Pin::new(as_slice);
        fill_func(as_pin.as_mut());

        let page_id = self.next_pid.fetch_add(1, Ordering::SeqCst);
        self.inner.page_table.insert(page_id, bid);
        Ok((page_id, used_size))
    }

    /// Free a page, and return it to the pool.
    pub fn free(&self, page_id: PageId) -> Result<(), BufferPoolError> {
        let bid = self
            .inner
            .page_table
            .remove(&page_id)
            .ok_or(BufferPoolError::InvalidPage)?;
        self.inner.pool.free(bid.1)
    }

    /// Resolve a page id to a pointer to the page's buffer.
    pub fn resolve_ptr(&self, page_id: PageId) -> Result<(*mut u8, usize), BufferPoolError> {
        let bid = self
            .inner
            .page_table
            .get(&page_id)
            .ok_or(BufferPoolError::InvalidPage)?;
        self.inner.pool.resolve_ptr(*bid)
    }

    /// Restore knowledge of a page (and a buffer) provided from cold storage.
    pub fn restore_page(
        &self,
        page_id: PageId,
        page_size: usize,
    ) -> Result<(AtomicPtr<u8>, usize), BufferPoolError> {
        // If there's already a buffer for this page, confirm it's the
        // right size, and just return its existing address.
        // Otherwise allocate a new buffer.
        if let Some(bid) = self.inner.page_table.get(&page_id) {
            let (ptr, size) = self.inner.pool.resolve_ptr(*bid)?;
            if size != page_size {
                panic!("Page size mismatch at restore for pid {} already-mapped to bid {} (expected {}, got {})",
                       page_id, bid.0, page_size, size);
            }
            return Ok((AtomicPtr::new(ptr), size));
        }

        // Allocate a buffer for this page, and insert it into the page table.
        let (bid, buf_ptr, used_size) = self.inner.pool.alloc(page_size)?;
        self.inner.page_table.insert(page_id, bid);
        Ok((AtomicPtr::new(buf_ptr), used_size))
    }

    /// Sync the working set to cold storage (if any)
    pub fn sync(&self, ts: u64, ws: WorkingSet, sequences: Vec<u64>) {
        let cs = self.cold_storage.lock().unwrap();
        if let Some(cold_storage) = cs.as_ref() {
            cold_storage.sync(ts, ws, sequences);
        }
    }

    /// Shutdown the pager and its minions.
    pub fn shutdown(&self) {
        let cs = self.cold_storage.lock().unwrap();
        if let Some(cold_storage) = cs.as_ref() {
            cold_storage.shutdown();
        }
    }
}
