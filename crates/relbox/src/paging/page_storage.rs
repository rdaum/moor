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

// TODO: Robustness testing and proofing for page storage and WAL.
//   A battery of tests is going to be needed on this, to verify the ACIDity.

use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::fd::{AsRawFd, RawFd};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex};
use std::thread::{yield_now, JoinHandle};
use std::time::Duration;

use io_uring::squeue::Flags;
use io_uring::types::Fd;
use io_uring::{opcode, IoUring};
use libc::eventfd_write;
use tracing::{debug, info, trace};

use crate::paging::{PageId, SlotId};
use crate::RelationId;

/// The size of the submission queue for the io_uring, in requests.
/// We currently do not have any way to handle backpressure, so we will not be able to handle WAL
/// writes faster than this so this is set very large.
// TODO: we should probably have a way to handle io_uring backpressure.
const IO_URING_SUBMISSION_Q_SIZE: u32 = 4096;

#[derive(Debug)]
pub enum PageStoreMutation {
    PageTupleWrite {
        relation_id: RelationId,
        page_id: PageId,
        slot_id: SlotId,
        page_offset: usize,
        data: Box<[u8]>,
    },
    PageHeaderWrite {
        relation_id: RelationId,
        page_id: PageId,
        data: Box<[u8]>,
    },
    WriteSequencePage(Box<[u8]>),
    DeleteTuple(PageId, RelationId),
}

/// Manages the directory of page files, currently one file per page.
/// Each page is a fixed size.
/// Uses io_uring to do the writes async. Reads are synchronous
///
/// TODO: deleted pages are not destroyed, they are just left on disk, which means if the same
///   page id is re-used, the old data could be read.
/// TODO: right now page storage is page-per-file which is maybe not the most efficient.
/// TODO: verify the fsync chained to writes via io_uring is actually working, and that
///   the durability guarantees are, at least approximately, correct.
/// TODO: we'll need reads once eviction/paging is implemented.
/// TODO: we should have CRCs on disk-bound pages, and verify them on reads.  
///   could live in the page header maybe

pub struct PageStore {
    dir: PathBuf,
    next_request_id: AtomicU64,
    event_fd: Fd,
    inner: Mutex<Inner>,
    running: Arc<AtomicBool>,
    join_handle: Mutex<Option<JoinHandle<()>>>,
}

struct Inner {
    uring: IoUring,
    buffers: HashMap<u64, (Arc<File>, Box<[u8]>)>,
}
// Use libc to set up eventfd.
fn make_eventfd() -> Fd {
    let raw_fd = unsafe { libc::eventfd(0, 0) as RawFd };
    if raw_fd < 0 {
        panic!("Unable to create eventfd");
    }
    Fd(raw_fd)
}

fn event_fd_listen_thread(event_fd: &Fd, ps: Arc<PageStore>, running_flag: Arc<AtomicBool>) {
    info!("Listening for eventfd events for page storage");
    loop {
        {
            let rf = running_flag.clone();
            let running = rf.load(std::sync::atomic::Ordering::SeqCst);
            if !running {
                break;
            }
        }
        let mut eventfd_v: libc::eventfd_t = 0;

        let ret = unsafe { libc::eventfd_read(event_fd.0, &mut eventfd_v) };

        if ret < 0 {
            panic!("Unable to read eventfd");
        }
        trace!("Poll returned {}", ret);

        let completed = ps.clone().process_completions();

        debug!("Synced all pages to disk? {}", completed);
    }
    info!("Shutting down eventfd listener");
}

impl PageStore {
    /// Establish the page store, creating the directories if they don't exist,
    /// setting up the io_uring, and tying it to the passed-in eventfd for
    /// signaling when requests complete.
    pub(crate) fn new(dir: PathBuf) -> Arc<Self> {
        // Check for dir path, if not there, create.
        if !dir.exists() {
            std::fs::create_dir_all(&dir).unwrap();
        }
        let uring = IoUring::new(IO_URING_SUBMISSION_Q_SIZE).unwrap();

        let event_fd = make_eventfd();

        // Set up the eventfd and start the listener thread.
        uring.submitter().register_eventfd(event_fd.0).unwrap();

        let inner = Inner {
            uring,
            buffers: Default::default(),
        };

        Arc::new(Self {
            dir,
            next_request_id: AtomicU64::new(0),
            inner: Mutex::new(inner),
            event_fd,
            running: Arc::new(AtomicBool::new(false)),
            join_handle: Mutex::new(None),
        })
    }

    pub(crate) fn start(self: Arc<Self>) {
        let running_flag = self.running.clone();

        {
            running_flag
                .as_ref()
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }

        // Set up polling for the eventfd.
        let ps = self.clone();
        let rf = running_flag.clone();
        let jh = std::thread::Builder::new()
            .name("moor-eventfd-listen".to_string())
            .spawn(move || {
                event_fd_listen_thread(&ps.clone().event_fd, ps.clone(), rf);
            })
            .expect("Unable to spawn eventfd listener thread");
        self.join_handle.lock().unwrap().replace(jh);
    }

    pub(crate) fn stop(self: Arc<Self>) {
        self.running
            .store(false, std::sync::atomic::Ordering::SeqCst);
        let jh = self.join_handle.lock().unwrap().take();
        // Write to the eventfd
        unsafe {
            let ret = eventfd_write(self.event_fd.0, 1);
            if ret != 0 {
                panic!("Unable to write to eventfd");
            }
        }
        if let Some(jh) = jh {
            jh.join().unwrap();
            let inner = self.inner.lock().unwrap();
            inner.uring.submitter().unregister_eventfd().unwrap();
        }
    }

    /// Blocking call to wait for all outstanding requests to complete.
    pub(crate) fn wait_complete(&self) {
        let start_time = std::time::Instant::now();
        debug!("Waiting for page storage to sync to disk");
        let mut inner = self.inner.lock().unwrap();

        while !inner.buffers.is_empty() {
            // If we've taken too long, just give up and return.
            if start_time.elapsed() > Duration::from_secs(5) {
                panic!("Page storage sync timed out");
            }
            let mut completions = vec![];
            while let Some(completion) = inner.uring.completion().next() {
                let request_id = completion.user_data();
                completions.push(request_id);
            }
            for request_id in completions {
                inner.buffers.remove(&request_id);
            }
            yield_now();
        }

        debug!("Page storage synced to disk");
    }

    /// Process any completions that have come in since the last time this was called, and
    /// return true if there are no outstanding requests.
    pub(crate) fn process_completions(&self) -> bool {
        let mut inner = self.inner.lock().unwrap();
        let mut completions = vec![];
        while let Some(completion) = inner.uring.completion().next() {
            let request_id = completion.user_data();
            completions.push(request_id);
        }
        for request_id in completions {
            inner.buffers.remove(&request_id);
        }
        inner.buffers.is_empty()
    }

    /// Get a catalog of all the pages in the store; their sizes, their page numbers, and the
    /// relation they belong to.
    pub(crate) fn list_pages(&self) -> HashSet<(usize, PageId, RelationId)> {
        let mut pages = HashSet::new();
        for entry in std::fs::read_dir(&self.dir).unwrap() {
            let entry = entry.unwrap();
            let filename = entry.file_name();
            if filename == "sequences.page" {
                continue;
            }
            let filename = filename.to_str().unwrap();
            let parts: Vec<&str> = filename.split('.').collect();
            let parts: Vec<&str> = parts[0].split('_').collect();
            let page_id = parts[0].parse::<PageId>().unwrap();
            let relation_id: usize = parts[1].parse().unwrap();
            let page_size = entry.metadata().unwrap().len();
            pages.insert((page_size as usize, page_id, RelationId(relation_id)));
        }
        pages
    }

    /// Read the special sequences page into a buffer.
    pub(crate) fn read_sequence_page(&self) -> std::io::Result<Option<Vec<u8>>> {
        let path = self.dir.join("sequences.page");
        let mut file = match File::open(path) {
            Ok(f) => f,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    return Ok(None);
                } else {
                    return Err(e);
                }
            }
        };
        let mut buf = vec![0; file.metadata()?.len() as usize];
        let len = file.read(&mut buf)?;
        buf.truncate(len);
        Ok(Some(buf))
    }

    /// Read a page into a pre-allocated buffer.
    pub(crate) fn read_page_buf(
        &self,
        page_id: PageId,
        relation_id: RelationId,
        mut buf: Pin<&mut [u8]>,
    ) -> std::io::Result<()> {
        let path = self.dir.join(format!("{}_{}.page", page_id, relation_id.0));
        let mut file = match File::open(path) {
            Ok(f) => f,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    return Ok(());
                } else {
                    return Err(e);
                }
            }
        };
        let len = file.read(buf.as_mut().get_mut())?;
        assert!(len <= buf.as_ref().len());
        Ok(())
    }

    /// Enqueue a batch of mutations to be written to disk. Will return immediately after
    /// submitting the batch to the kernel via io_uring.
    pub(crate) fn enqueue_page_mutations(
        &self,
        batch: Vec<PageStoreMutation>,
    ) -> std::io::Result<()> {
        self.wait_complete();

        // Open all the pages mentioned in the batch and index them by page id so we only have one file descriptor
        // open per page file.
        // TODO: it's possible to preregister file descriptors with io_uring to potentially speed things up, but
        //   I had some trixky issues with that, so for now we'll just open a new file descriptor for each batch.
        let mut pages = HashMap::new();
        for mutation in &batch {
            match mutation {
                PageStoreMutation::PageHeaderWrite {
                    relation_id,
                    page_id,
                    ..
                }
                | PageStoreMutation::PageTupleWrite {
                    relation_id,
                    page_id,
                    ..
                } => {
                    if pages.contains_key(page_id) {
                        continue;
                    }
                    let path = self.dir.join(format!("{}_{}.page", page_id, relation_id.0));
                    let file = OpenOptions::new()
                        .write(true)
                        .append(false)
                        .create(true)
                        .truncate(false)
                        .open(path)?;
                    pages.insert(*page_id, Arc::new(file));
                }
                _ => continue,
            }
        }

        for mutation in batch {
            let request_id = self
                .next_request_id
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            match mutation {
                PageStoreMutation::PageHeaderWrite { page_id, data, .. } => {
                    let file = pages.get(&page_id).unwrap();
                    let len = data.len();
                    let raw_fd = file.as_raw_fd();

                    let mut inner = self.inner.lock().unwrap();
                    inner.buffers.insert(request_id, (file.clone(), data));
                    let data_ptr = inner.buffers.get(&request_id).unwrap().1.as_ptr();

                    // Seek to the offset and write there
                    let write_e = opcode::Write::new(Fd(raw_fd), data_ptr as _, len as _)
                        .build()
                        .user_data(request_id)
                        .flags(Flags::IO_LINK);
                    unsafe {
                        inner
                            .uring
                            .submission()
                            .push(&write_e)
                            .expect("Unable to push write to submission queue");
                    }

                    // Tell the kernel to flush the file to disk after writing out the WAL, and this should be
                    // linked to all the other writes above.
                    let fsync_e = opcode::Fsync::new(Fd(raw_fd)).build().user_data(request_id);
                    unsafe {
                        inner
                            .uring
                            .submission()
                            .push(&fsync_e)
                            .expect("Unable to push fsync to submission queue");
                    }
                }
                PageStoreMutation::PageTupleWrite {
                    page_id,
                    page_offset,
                    data,
                    ..
                } => {
                    let file = pages.get(&page_id).unwrap();
                    let len = data.len();
                    let raw_fd = file.as_raw_fd();

                    let mut inner = self.inner.lock().unwrap();
                    inner.buffers.insert(request_id, (file.clone(), data));
                    let data_ptr = inner.buffers.get(&request_id).unwrap().1.as_ptr();

                    // Seek to the offset and write there
                    let write_e = opcode::Write::new(Fd(raw_fd), data_ptr as _, len as _)
                        .offset(page_offset as _)
                        .build()
                        .user_data(request_id)
                        .flags(Flags::IO_LINK);
                    unsafe {
                        inner
                            .uring
                            .submission()
                            .push(&write_e)
                            .expect("Unable to push write to submission queue");
                    }
                }
                PageStoreMutation::WriteSequencePage(data) => {
                    let path = self.dir.join("sequences.page");

                    let len = data.len();
                    let mut options = OpenOptions::new();
                    let file = options.write(true).append(false).create(true).open(path)?;
                    let raw_fd = file.as_raw_fd();

                    let mut inner = self.inner.lock().unwrap();

                    inner.buffers.insert(request_id, (Arc::new(file), data));
                    let data_ptr = inner.buffers.get(&request_id).unwrap().1.as_ptr();

                    let write_e = opcode::Write::new(Fd(raw_fd), data_ptr as _, len as _)
                        .build()
                        .user_data(request_id)
                        .flags(Flags::IO_HARDLINK);

                    unsafe {
                        inner
                            .uring
                            .submission()
                            .push(&write_e)
                            .expect("Unable to push write to submission queue");
                    }

                    let fsync_e = opcode::Fsync::new(Fd(raw_fd)).build().user_data(request_id);
                    unsafe {
                        inner
                            .uring
                            .submission()
                            .push(&fsync_e)
                            .expect("Unable to push fsync to submission queue");
                    }
                }
                PageStoreMutation::DeleteTuple(_, _) => {
                    // We could zero-out this data, but it's not really necessary, the header will
                    // be updated to indicate the slot is free.
                }
            }
            let inner = self.inner.lock().unwrap();
            inner.uring.submit().expect("Unable to submit to io_uring");
        }

        Ok(())
    }
}
