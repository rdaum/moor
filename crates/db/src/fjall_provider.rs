// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use crate::tx_management::{Error, Provider, Timestamp};
use byteview::ByteView;
use fjall::UserValue;
use flume::Sender;
use gdt_cpus::ThreadPriority;
use moor_var::AsByteBuffer;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;
use std::time::Duration;
use tracing::error;

enum WriteOp<
    Domain: Clone + Eq + PartialEq + std::hash::Hash + AsByteBuffer + Send + Sync,
    Codomain: Clone + PartialEq + AsByteBuffer + Send + Sync,
> {
    Insert(Timestamp, Domain, Codomain),
    Delete(Domain),
    /// Barrier marker for snapshot consistency - reply when all writes up to this sequence are complete
    Barrier(u64, oneshot::Sender<()>),
}

/// Tracks operations that have been submitted to the background thread but not yet completed
struct PendingOperations<Domain, Codomain>
where
    Domain: Clone + Eq + PartialEq + std::hash::Hash + Send + Sync,
    Codomain: Clone + PartialEq + Send + Sync,
{
    /// Keys that have been deleted but delete hasn't flushed to backing store yet
    pending_deletes: HashSet<Domain>,
    /// Keys that have been written but write hasn't flushed to backing store yet  
    pending_writes: HashMap<Domain, (Timestamp, Codomain)>,
    /// Highest barrier sequence that has been processed
    completed_barrier: u64,
}

impl<Domain, Codomain> Default for PendingOperations<Domain, Codomain>
where
    Domain: Clone + Eq + PartialEq + std::hash::Hash + Send + Sync,
    Codomain: Clone + PartialEq + Send + Sync,
{
    fn default() -> Self {
        Self {
            pending_deletes: HashSet::new(),
            pending_writes: HashMap::new(),
            completed_barrier: 0,
        }
    }
}

/// A backing persistence provider that fills the DB cache from a Fjall partition.
#[derive(Clone)]
pub(crate) struct FjallProvider<Domain, Codomain>
where
    Domain: Clone + Eq + PartialEq + std::hash::Hash + AsByteBuffer + Send + Sync,
    Codomain: Clone + PartialEq + AsByteBuffer + Send + Sync,
{
    fjall_partition: fjall::PartitionHandle,
    ops: Sender<WriteOp<Domain, Codomain>>,
    kill_switch: Arc<AtomicBool>,
    /// Shared state tracking operations in-flight to background thread
    pending_ops: Arc<RwLock<PendingOperations<Domain, Codomain>>>,
    jh: Arc<Mutex<Option<JoinHandle<()>>>>,
}

fn decode<Codomain>(user_value: UserValue) -> Result<(Timestamp, Codomain), Error>
where
    Codomain: AsByteBuffer,
{
    let result: ByteView = user_value.into();
    let ts = Timestamp(u64::from_le_bytes(result[0..8].try_into().unwrap()));
    let codomain = Codomain::from_bytes(result.slice(8..)).map_err(|_| Error::EncodingFailure)?;
    Ok((ts, codomain))
}

fn encode<Codomain>(ts: Timestamp, codomain: &Codomain) -> Result<UserValue, Error>
where
    Codomain: AsByteBuffer,
{
    let as_bytes = codomain.as_bytes().map_err(|_| Error::EncodingFailure)?;
    let mut result = Vec::with_capacity(8 + as_bytes.len());
    result.extend_from_slice(&ts.0.to_le_bytes());
    result.extend_from_slice(&as_bytes);
    Ok(UserValue::from(ByteView::from(result)))
}

impl<Domain, Codomain> FjallProvider<Domain, Codomain>
where
    Domain: Clone + Eq + PartialEq + std::hash::Hash + AsByteBuffer + Send + Sync + 'static,
    Codomain: Clone + PartialEq + AsByteBuffer + Send + Sync + 'static,
{
    pub fn new(relation_name: &str, fjall_partition: fjall::PartitionHandle) -> Self {
        let kill_switch = Arc::new(AtomicBool::new(false));
        let (ops_tx, ops_rx) = flume::unbounded::<WriteOp<Domain, Codomain>>();
        let pending_ops = Arc::new(RwLock::new(PendingOperations::default()));

        let fj = fjall_partition.clone();
        let ks = kill_switch.clone();
        let pending_ops_bg = pending_ops.clone();
        let thread_name = format!("moor-w-{relation_name}");
        let tb = std::thread::Builder::new().name(thread_name);
        let jh = tb
            .spawn(move || {
                gdt_cpus::set_thread_priority(ThreadPriority::Background).ok();
                loop {
                    if ks.load(std::sync::atomic::Ordering::SeqCst) {
                        break;
                    }
                    match ops_rx.recv_timeout(Duration::from_millis(5)) {
                        Ok(WriteOp::Insert(ts, domain, codomain)) => {
                            let Ok(key) = domain.as_bytes().map_err(|_| {
                                error!("failed to encode domain to database");
                            }) else {
                                // Remove from pending operations even on encoding error
                                if let Ok(mut pending) = pending_ops_bg.write() {
                                    pending.pending_writes.remove(&domain);
                                }
                                continue;
                            };
                            let Ok(value) = encode::<Codomain>(ts, &codomain) else {
                                error!("failed to encode codomain to database");
                                // Remove from pending operations even on encoding error
                                if let Ok(mut pending) = pending_ops_bg.write() {
                                    pending.pending_writes.remove(&domain);
                                }
                                continue;
                            };

                            // Perform the actual write
                            let write_result = fj.insert(key, value);

                            // Remove from pending operations after completion (success or failure)
                            if let Ok(mut pending) = pending_ops_bg.write() {
                                pending.pending_writes.remove(&domain);
                            }

                            if let Err(e) = write_result {
                                error!("failed to insert into database: {}", e);
                            }
                        }
                        Ok(WriteOp::Delete(domain)) => {
                            let Ok(key) = domain.as_bytes().map_err(|_| {
                                error!("failed to encode domain to database for deletion");
                            }) else {
                                // Remove from pending operations even on encoding error
                                if let Ok(mut pending) = pending_ops_bg.write() {
                                    pending.pending_deletes.remove(&domain);
                                }
                                continue;
                            };

                            // Perform the actual delete
                            let delete_result = fj.remove(key);

                            // Remove from pending operations after completion (success or failure)
                            if let Ok(mut pending) = pending_ops_bg.write() {
                                pending.pending_deletes.remove(&domain);
                            }

                            if let Err(e) = delete_result {
                                error!("failed to delete from database: {}", e);
                            }
                        }
                        Ok(WriteOp::Barrier(seq, reply)) => {
                            // Mark this barrier as completed and reply
                            if let Ok(mut pending) = pending_ops_bg.write() {
                                pending.completed_barrier = seq;
                            }
                            // Reply to indicate barrier is processed
                            reply.send(()).ok();
                        }
                        Err(_e) => {
                            continue;
                        }
                    }
                }
            })
            .expect("failed to spawn fjall-write");
        Self {
            fjall_partition,
            ops: ops_tx,
            kill_switch,
            pending_ops,
            jh: Arc::new(Mutex::new(Some(jh))),
        }
    }

    pub fn partition(&self) -> &fjall::PartitionHandle {
        &self.fjall_partition
    }

    /// Wait for all writes up to the specified barrier sequence to be completed.
    /// This ensures that all pending writes submitted before this barrier are flushed
    /// to the backing store, providing a consistent point for snapshots.
    pub fn wait_for_write_barrier(&self, barrier_seq: u64, timeout: Duration) -> Result<(), Error> {
        let (send, recv) = oneshot::channel();

        // Send barrier message to background thread
        if let Err(e) = self.ops.send(WriteOp::Barrier(barrier_seq, send)) {
            return Err(Error::StorageFailure(format!(
                "failed to send barrier message: {e}"
            )));
        }

        // Wait for the barrier to be processed
        match recv.recv_timeout(timeout) {
            Ok(()) => Ok(()),
            Err(oneshot::RecvTimeoutError::Timeout) => Err(Error::StorageFailure(format!(
                "Timeout waiting for write barrier {barrier_seq}"
            ))),
            Err(oneshot::RecvTimeoutError::Disconnected) => Err(Error::StorageFailure(
                "Background thread disconnected while waiting for barrier".to_string(),
            )),
        }
    }
}

impl<Domain, Codomain> Provider<Domain, Codomain> for FjallProvider<Domain, Codomain>
where
    Domain: Clone + Eq + PartialEq + std::hash::Hash + AsByteBuffer + Send + Sync,
    Codomain: Clone + PartialEq + AsByteBuffer + Send + Sync,
{
    fn get(&self, domain: &Domain) -> Result<Option<(Timestamp, Codomain)>, Error> {
        // 1. Check pending operations first
        {
            let pending = self.pending_ops.read().map_err(|_| {
                Error::StorageFailure("Failed to acquire pending ops read lock".to_string())
            })?;

            // If pending delete, definitely doesn't exist
            if pending.pending_deletes.contains(domain) {
                return Ok(None);
            }

            // If pending write, return that value
            if let Some((ts, value)) = pending.pending_writes.get(domain) {
                return Ok(Some((*ts, value.clone())));
            }
        }

        // 2. Only then check backing store
        let key = domain.as_bytes().map_err(|_| Error::EncodingFailure)?;
        let Some(result) = self
            .fjall_partition
            .get(key)
            .map_err(|e| Error::RetrievalFailure(e.to_string()))?
        else {
            return Ok(None);
        };
        let (ts, codomain) = decode::<Codomain>(result)?;
        Ok(Some((ts, codomain)))
    }

    fn put(&self, timestamp: Timestamp, domain: &Domain, codomain: &Codomain) -> Result<(), Error> {
        // Add to pending writes immediately
        {
            let mut pending = self.pending_ops.write().map_err(|_| {
                Error::StorageFailure("Failed to acquire pending ops write lock".to_string())
            })?;
            pending
                .pending_writes
                .insert(domain.clone(), (timestamp, codomain.clone()));
            // Also remove from pending deletes if it was there (overwriting a deleted key)
            pending.pending_deletes.remove(domain);
        }

        // Send async operation
        if let Err(e) = self
            .ops
            .send(WriteOp::Insert(timestamp, domain.clone(), codomain.clone()))
        {
            // If sending fails, remove from pending operations
            if let Ok(mut pending) = self.pending_ops.write() {
                pending.pending_writes.remove(domain);
            }
            return Err(Error::StorageFailure(format!(
                "failed to insert into database: {e}"
            )));
        }
        Ok(())
    }

    fn del(&self, _timestamp: Timestamp, domain: &Domain) -> Result<(), Error> {
        // Add to pending deletes immediately
        {
            let mut pending = self.pending_ops.write().map_err(|_| {
                Error::StorageFailure("Failed to acquire pending ops write lock".to_string())
            })?;
            pending.pending_deletes.insert(domain.clone());
            // Also remove from pending writes if it was there
            pending.pending_writes.remove(domain);
        }

        // Send async operation
        if let Err(e) = self.ops.send(WriteOp::Delete(domain.clone())) {
            // If sending fails, remove from pending operations
            if let Ok(mut pending) = self.pending_ops.write() {
                pending.pending_deletes.remove(domain);
            }
            return Err(Error::StorageFailure(format!(
                "failed to delete from database: {e}"
            )));
        };
        Ok(())
    }

    fn scan<F>(&self, predicate: &F) -> Result<Vec<(Timestamp, Domain, Codomain)>, Error>
    where
        F: Fn(&Domain, &Codomain) -> bool,
    {
        let mut result = Vec::new();

        // Get snapshot of pending operations
        let pending = self.pending_ops.read().map_err(|_| {
            Error::StorageFailure("Failed to acquire pending ops read lock".to_string())
        })?;

        // Scan backing store first
        for entry in self.fjall_partition.iter() {
            let (key, value) = entry.map_err(|e| Error::RetrievalFailure(e.to_string()))?;
            let domain =
                Domain::from_bytes(key.clone().into()).map_err(|_| Error::EncodingFailure)?;

            // Skip if this domain is pending deletion
            if pending.pending_deletes.contains(&domain) {
                continue;
            }

            let (ts, codomain) = decode::<Codomain>(value)?;
            if predicate(&domain, &codomain) {
                result.push((ts, domain, codomain));
            }
        }

        // Add pending writes that match the predicate
        for (domain, (ts, codomain)) in &pending.pending_writes {
            if predicate(domain, codomain) {
                result.push((*ts, domain.clone(), codomain.clone()));
            }
        }

        Ok(result)
    }

    fn stop(&self) -> Result<(), Error> {
        self.kill_switch
            .store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

impl<Domain, Codomain> Drop for FjallProvider<Domain, Codomain>
where
    Domain: Clone + Eq + PartialEq + std::hash::Hash + AsByteBuffer + Send + Sync,
    Codomain: Clone + PartialEq + AsByteBuffer + Send + Sync,
{
    fn drop(&mut self) {
        self.stop().unwrap();
        let mut jh = self.jh.lock().unwrap();
        if let Some(jh) = jh.take() {
            jh.join().unwrap();
        }
    }
}
