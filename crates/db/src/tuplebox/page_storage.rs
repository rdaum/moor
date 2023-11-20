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

use crate::tuplebox::slots::PageId;
use crate::tuplebox::RelationId;
use im::{HashMap, HashSet};
use io_uring::types::Fd;
use io_uring::{opcode, IoUring};
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::fd::{IntoRawFd, RawFd};
use std::path::PathBuf;
use std::pin::Pin;
use tracing::info;

pub(crate) enum PageStoreMutation {
    SyncRelationPage(RelationId, PageId, Box<[u8]>),
    SyncSequencePage(Box<[u8]>),
    DeleteRelationPage(PageId, RelationId),
}

/// Manages the directory of pages, one file per page.
/// Each page is a fixed size.
/// will attempt to use io_uring to do the writes async. reads are synchronous
pub(crate) struct PageStore {
    dir: PathBuf,
    uring: IoUring,
    next_request_id: u64,
    buffers: HashMap<u64, (RawFd, Box<[u8]>)>,
}

impl PageStore {
    pub(crate) fn new(dir: PathBuf) -> Self {
        // Check for dir path, if not there, create.
        if !dir.exists() {
            std::fs::create_dir_all(&dir).unwrap();
        }
        let uring = IoUring::new(8).unwrap();
        Self {
            dir,
            uring,
            next_request_id: 0,
            buffers: Default::default(),
        }
    }

    pub(crate) fn wait_complete(&mut self) {
        info!("Waiting for {} completions", self.buffers.len());
        while !self.buffers.is_empty() {
            while let Some(completion) = self.uring.completion().next() {
                let request_id = completion.user_data();
                self.buffers.remove(&request_id);
            }
        }
        info!("All completions done");
    }

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

    // TODO: batch submit + fsync
    pub(crate) fn write_batch(&mut self, batch: Vec<PageStoreMutation>) -> std::io::Result<()> {
        // go through previous completions and remove the buffers
        while let Some(completion) = self.uring.completion().next() {
            let request_id = completion.user_data();
            self.buffers.remove(&request_id);
        }
        for mutation in batch {
            let request_id = self.next_request_id;
            self.next_request_id += 1;
            match mutation {
                PageStoreMutation::SyncRelationPage(relation_id, page_id, data) => {
                    let path = self.dir.join(format!("{}_{}.page", page_id, relation_id.0));
                    let len = data.len();
                    let mut options = OpenOptions::new();
                    let file = options.write(true).append(false).create(true).open(path)?;
                    let fd = file.into_raw_fd();

                    self.buffers.insert(request_id, (fd, data));
                    let data_ptr = self.buffers.get(&request_id).unwrap().1.as_ptr();

                    let write_e = opcode::Write::new(Fd(fd), data_ptr as _, len as _)
                        .build()
                        .user_data(request_id);
                    unsafe {
                        self.uring
                            .submission()
                            .push(&write_e)
                            .expect("Unable to push write to submission queue");
                    }
                }
                PageStoreMutation::SyncSequencePage(data) => {
                    let path = self.dir.join("sequences.page");

                    let len = data.len();
                    let mut options = OpenOptions::new();
                    let file = options.write(true).append(false).create(true).open(path)?;
                    let fd = file.into_raw_fd();
                    self.buffers.insert(request_id, (fd, data));
                    let data_ptr = self.buffers.get(&request_id).unwrap().1.as_ptr();

                    let write_e = opcode::Write::new(Fd(fd), data_ptr as _, len as _)
                        .build()
                        .user_data(request_id);
                    unsafe {
                        self.uring
                            .submission()
                            .push(&write_e)
                            .expect("Unable to push write to submission queue");
                    }
                }
                PageStoreMutation::DeleteRelationPage(_, _) => {
                    // TODO
                }
            }
            self.uring.submit()?;
        }
        Ok(())
    }
}
