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

//! Interface for the backing store writer thread.
//! Used for write-ahead type storage at commit-time, and backed by whatever preferred physical
//! storage mechanism is desired.

use crossbeam_channel::Sender;
use std::thread::yield_now;

use crate::tx::WorkingSet;

pub struct BackingStoreClient {
    sender: Sender<WriterMessage>,
    join_handle: std::thread::JoinHandle<()>,
}

pub enum WriterMessage {
    Commit(u64, WorkingSet, Vec<u64>),
    Shutdown,
}

impl BackingStoreClient {
    pub fn new(sender: Sender<WriterMessage>, join_handle: std::thread::JoinHandle<()>) -> Self {
        Self {
            sender,
            join_handle,
        }
    }

    /// Sync out the working set from a committed transaction for the given transaction timestamp.
    /// Used to support persistent storage of committed transactions, effectively as a write-ahead
    /// log.
    pub fn sync(&self, ts: u64, ws: WorkingSet, sequences: Vec<u64>) {
        self.sender
            .send(WriterMessage::Commit(ts, ws, sequences))
            .expect("Unable to send write-ahead sync message");
    }

    /// Shutdown the backing store writer thread.
    pub fn shutdown(&self) {
        self.sender
            .send(WriterMessage::Shutdown)
            .expect("Unable to send shutdown message");
        while !self.join_handle.is_finished() {
            yield_now()
        }
    }
}
