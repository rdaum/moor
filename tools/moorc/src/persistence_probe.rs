// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

//! Sample persistence occupancy during explicitly phased benchmark tests.

use moor_db::TxDB;
use std::{sync::mpsc, thread::JoinHandle, time::Duration};

pub struct PersistenceProbe {
    stop: mpsc::Sender<()>,
    worker: Option<JoinHandle<()>>,
}

impl PersistenceProbe {
    pub fn start(db: TxDB, test: String, phase: u16) -> Self {
        let (stop, receiver) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let mut max_outstanding = 0;
            let mut max_unapplied = 0;
            let mut samples = 0_u64;
            loop {
                let status = db.persistence_status();
                max_outstanding = max_outstanding.max(status.outstanding);
                max_unapplied = max_unapplied.max(status.published.saturating_sub(status.applied));
                samples += 1;
                if !matches!(
                    receiver.recv_timeout(Duration::from_millis(1)),
                    Err(mpsc::RecvTimeoutError::Timeout)
                ) {
                    break;
                }
            }
            tracing::info!(%test, phase, samples, max_outstanding, max_unapplied, "PERSISTENCE_OCCUPANCY");
        });
        Self {
            stop,
            worker: Some(worker),
        }
    }
}

impl Drop for PersistenceProbe {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
