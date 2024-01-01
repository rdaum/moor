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

use tokio::sync::mpsc::UnboundedSender;

use crate::tuplebox::tx::working_set::WorkingSet;

pub struct BackingStoreClient {
    sender: UnboundedSender<WriterMessage>,
}

pub enum WriterMessage {
    Commit(u64, WorkingSet, Vec<u64>),
    Shutdown,
}

impl BackingStoreClient {
    pub fn new(sender: UnboundedSender<WriterMessage>) -> Self {
        Self { sender }
    }

    /// Sync out the working set from a committed transaction for the given transaction timestamp.
    /// Used to support persistent storage of committed transactions, effectively as a write-ahead
    /// log.
    pub async fn sync(&self, ts: u64, ws: WorkingSet, sequences: Vec<u64>) {
        self.sender
            .send(WriterMessage::Commit(ts, ws, sequences))
            .expect("Unable to send write-ahead sync message");
    }

    /// Shutdown the backing store writer thread.
    pub async fn shutdown(&self) {
        self.sender
            .send(WriterMessage::Shutdown)
            .expect("Unable to send shutdown message");
    }
}
