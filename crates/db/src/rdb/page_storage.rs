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

// TODO: there's no way this is "robust" enough to be used in production

use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::fd::{IntoRawFd, RawFd};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};
use std::thread::yield_now;
use std::time::Duration;

use io_uring::squeue::Flags;
use io_uring::types::Fd;
use io_uring::{opcode, IoUring};
use tracing::{debug, error, info};

use crate::rdb::paging::PageId;
use crate::rdb::RelationId;

pub(crate) enum PageStoreMutation {
    SyncRelation(RelationId, PageId, Box<[u8]>),
    SyncSequence(Box<[u8]>),
    DeleteRelation(PageId, RelationId),
}

/// Manages the directory of pages, one file per page.
/// Each page is a fixed size.
/// will attempt to use io_uring to do the writes async. reads are synchronous
///
/// TODO: deleted pages are not destroyed, they are just left on disk, which means if the same
///   page id is re-used, the old data could be read.
/// TODO: right now this is a page-per-file which is maybe not the most efficient.
/// TODO: verify the fsync chained to writes via io_uring is actually working, and that
///   the durability guarantees are, at least approximately, correct.
/// TODO: we'll need reads once eviction/paging is implemented.
/// TODO: it's weird that the eventfd is handled outside of this struct, but the io_uring is
///   handled inside. it got this way because of ownership and initialization patterns, but
///   it's not ideal.
/// TODO: probably end up needing similar functionality for the implementation of the
///   write-ahead-log, so abstract up the notion of an io_uring+eventfd "io q" and use that
///   for both.

pub(crate) struct PageStore {
    dir: PathBuf,
    next_request_id: AtomicU64,
    event_fd: Fd,
    inner: Mutex<Inner>,
    running: Arc<Mutex<bool>>,
}

struct Inner {
    uring: IoUring,
    buffers: HashMap<u64, (RawFd, Box<[u8]>)>,
}
// Use libc to set up eventfd.
fn make_eventfd() -> Fd {
    let raw_fd = unsafe { libc::eventfd(0, 0) as RawFd };
    if raw_fd < 0 {
        panic!("Unable to create eventfd");
    }
    Fd(raw_fd)
}

fn event_fd_listen_thread(event_fd: &Fd, ps: Arc<PageStore>, running_flag: Arc<Mutex<bool>>) {
    info!("Listening for eventfd events for page storage");
    loop {
        {
            let rf = running_flag.clone();
            let running = rf.lock().unwrap();
            if !*running {
                break;
            }
        }
        let mut eventfd_v: libc::eventfd_t = 0;

        let ret = unsafe { libc::eventfd_read(event_fd.0, &mut eventfd_v) };

        if ret < 0 {
            panic!("Unable to read eventfd");
        }

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
        let uring = IoUring::new(8).unwrap();

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
            running: Arc::new(Mutex::new(false)),
        })
    }

    pub(crate) fn start(self: Arc<Self>) {
        let running_flag = self.running.clone();

        {
            let mut running = running_flag.lock().unwrap();
            *running = true;
        }

        // Set up polling for the eventfd.
        let ps = self.clone();
        let rf = running_flag.clone();
        std::thread::Builder::new()
            .name("moor-eventfd-listen".to_string())
            .spawn(move || {
                event_fd_listen_thread(&self.event_fd, ps, rf);
            })
            .expect("Unable to spawn eventfd listener thread");
    }

    pub(crate) fn stop(self: Arc<Self>) {
        let mut running = self.running.lock().unwrap();
        *running = false;
    }

    /// Blocking call to wait for all outstanding requests to complete.
    pub(crate) fn wait_complete(&self) {
        let start_time = std::time::Instant::now();
        debug!("Waiting for page storage to sync to disk");
        let mut inner = self.inner.lock().unwrap();
        while !inner.buffers.is_empty() {
            // If we've taken too long, just give up and return.
            if start_time.elapsed() > Duration::from_secs(5) {
                error!("Page storage sync timed out");
                return;
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
        assert_eq!(len, buf.as_ref().len());
        Ok(())
    }

    /// Enqueue a batch of mutations to be written to disk. Will return immediately after
    /// submitting the batch to the kernel via io_uring.
    pub(crate) fn enqueue_page_mutations(
        &self,
        batch: Vec<PageStoreMutation>,
    ) -> std::io::Result<()> {
        // TODO: We prolly shouldn't submit a new batch until all the previous requests have completed.
        self.process_completions();

        for mutation in batch {
            let request_id = self
                .next_request_id
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            match mutation {
                PageStoreMutation::SyncRelation(relation_id, page_id, data) => {
                    let path = self.dir.join(format!("{}_{}.page", page_id, relation_id.0));
                    let len = data.len();
                    let mut options = OpenOptions::new();
                    let file = options.write(true).append(false).create(true).open(path)?;
                    let fd = file.into_raw_fd();

                    let mut inner = self.inner.lock().unwrap();
                    inner.buffers.insert(request_id, (fd, data));
                    let data_ptr = inner.buffers.get(&request_id).unwrap().1.as_ptr();

                    let write_e = opcode::Write::new(Fd(fd), data_ptr as _, len as _)
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

                    // Tell the kernel to flush the file to disk after writing it, and this should be
                    // linked to the write above.
                    let fsync_e = opcode::Fsync::new(Fd(fd)).build().user_data(request_id);
                    unsafe {
                        inner
                            .uring
                            .submission()
                            .push(&fsync_e)
                            .expect("Unable to push fsync to submission queue");
                    }
                }
                PageStoreMutation::SyncSequence(data) => {
                    let path = self.dir.join("sequences.page");

                    let len = data.len();
                    let mut options = OpenOptions::new();
                    let file = options.write(true).append(false).create(true).open(path)?;
                    let fd = file.into_raw_fd();

                    let mut inner = self.inner.lock().unwrap();

                    inner.buffers.insert(request_id, (fd, data));
                    let data_ptr = inner.buffers.get(&request_id).unwrap().1.as_ptr();

                    let write_e = opcode::Write::new(Fd(fd), data_ptr as _, len as _)
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

                    let fsync_e = opcode::Fsync::new(Fd(fd)).build().user_data(request_id);
                    unsafe {
                        inner
                            .uring
                            .submission()
                            .push(&fsync_e)
                            .expect("Unable to push fsync to submission queue");
                    }
                }
                PageStoreMutation::DeleteRelation(_, _) => {
                    // TODO
                }
            }
            let inner = self.inner.lock().unwrap();
            inner.uring.submit().expect("Unable to submit to io_uring");
        }

        Ok(())
    }
}
