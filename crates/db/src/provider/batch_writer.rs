// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

//! Coalescing batch writer for fjall operations.
//!
//! Instead of writing every transaction's changes immediately, we buffer writes
//! in per-partition HashMaps where later writes to the same key overwrite earlier ones.
//! This reduces actual I/O when the same keys are written repeatedly.
//!
//! Flush triggers:
//! - Total pending ops exceed threshold
//! - Time since last flush exceeds interval
//! - Barrier request (for snapshots)
//! - Shutdown

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use flume::{Receiver, Sender};
use gdt_cpus::ThreadPriority;
use tracing::{error, info, warn};

use crate::tx_management::{Error, Timestamp};

/// A single operation to be written to fjall.
#[derive(Clone)]
pub struct BatchOp {
    /// The fjall partition (keyspace) to write to
    pub partition: fjall::Keyspace,
    /// The operation type
    pub op_type: BatchOpType,
}

#[derive(Clone)]
pub enum BatchOpType {
    Insert {
        key: Vec<u8>,
        value: Arc<dyn BatchValue>,
    },
    Delete {
        key: Vec<u8>,
    },
}

/// Value hook used by the writer thread to produce serialized bytes.
pub trait BatchValue: Send + Sync {
    fn encode(&self) -> Result<Vec<u8>, Error>;
}

/// A batch of operations from a single commit, spanning all relations.
pub struct CommitBatch {
    pub timestamp: Timestamp,
    pub operations: Vec<BatchOp>,
}

impl CommitBatch {
    pub fn with_capacity(timestamp: Timestamp, expected_operations: usize) -> Self {
        Self {
            timestamp,
            operations: Vec::with_capacity(expected_operations),
        }
    }

    pub fn insert(&mut self, partition: fjall::Keyspace, key: Vec<u8>, value: Arc<dyn BatchValue>) {
        self.operations.push(BatchOp {
            partition,
            op_type: BatchOpType::Insert { key, value },
        });
    }

    pub fn delete(&mut self, partition: fjall::Keyspace, key: Vec<u8>) {
        self.operations.push(BatchOp {
            partition,
            op_type: BatchOpType::Delete { key },
        });
    }

    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }
}

/// Manages the current commit batch. Shared across all FjallProviders.
#[derive(Default)]
pub struct BatchCollector {
    current: Mutex<Option<CommitBatch>>,
}

impl BatchCollector {
    pub fn new() -> Self {
        Self {
            current: Mutex::new(None),
        }
    }

    pub fn start_commit(&self, timestamp: Timestamp, expected_operations: usize) {
        let mut current = self.current.lock().unwrap();
        debug_assert!(
            current.is_none(),
            "Previous commit batch not finished (timestamp {:?})",
            current.as_ref().map(|b| b.timestamp)
        );
        *current = Some(CommitBatch::with_capacity(timestamp, expected_operations));
    }

    pub fn insert(&self, partition: fjall::Keyspace, key: Vec<u8>, value: Arc<dyn BatchValue>) {
        let mut current = self.current.lock().unwrap();
        current
            .as_mut()
            .expect("No active commit batch - call start_commit() first")
            .insert(partition, key, value);
    }

    pub fn delete(&self, partition: fjall::Keyspace, key: Vec<u8>) {
        let mut current = self.current.lock().unwrap();
        current
            .as_mut()
            .expect("No active commit batch - call start_commit() first")
            .delete(partition, key);
    }

    pub fn finish_commit(&self) -> CommitBatch {
        self.current
            .lock()
            .unwrap()
            .take()
            .expect("No active commit batch to finish")
    }

    pub fn abort_commit(&self) {
        self.current.lock().unwrap().take();
    }
}

/// Pending operation for a key - either insert with value or delete.
#[derive(Clone)]
enum PendingOp {
    Insert(Arc<dyn BatchValue>),
    Delete,
}

/// Per-partition coalescing buffer.
struct PartitionBuffer {
    keyspace: fjall::Keyspace,
    pending: HashMap<Vec<u8>, PendingOp>,
}

impl PartitionBuffer {
    fn new(keyspace: fjall::Keyspace) -> Self {
        Self {
            keyspace,
            pending: HashMap::new(),
        }
    }

    fn insert(&mut self, key: Vec<u8>, value: Arc<dyn BatchValue>) {
        self.pending.insert(key, PendingOp::Insert(value));
    }

    fn delete(&mut self, key: Vec<u8>) {
        self.pending.insert(key, PendingOp::Delete);
    }

    fn len(&self) -> usize {
        self.pending.len()
    }

    fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    fn drain(&mut self) -> impl Iterator<Item = (Vec<u8>, PendingOp)> + '_ {
        self.pending.drain()
    }
}

/// Message sent to the writer thread.
enum WriterMsg {
    /// Merge a batch into the coalescing buffers.
    Write(CommitBatch),
    /// Flush all pending writes and reply when complete.
    Barrier(Timestamp, oneshot::Sender<()>),
}

/// Thresholds for determining when to coalesce vs flush immediately.
/// Under normal operation, we flush immediately for durability.
/// Under pressure (slow flushes or high pending count), we coalesce to reduce load.
const PRESSURE_PENDING_THRESHOLD: usize = 10_000;
const PRESSURE_FLUSH_DURATION: Duration = Duration::from_millis(100);
const MAX_PENDING_OPS: usize = 50_000;
const MAX_COALESCE_INTERVAL: Duration = Duration::from_millis(100);

/// Coalescing batch writer that deduplicates writes before hitting fjall.
pub struct BatchWriter {
    sender: Sender<WriterMsg>,
    kill_switch: Arc<AtomicBool>,
    completed_timestamp: Arc<AtomicU64>,
    join_handle: Mutex<Option<JoinHandle<()>>>,
}

impl BatchWriter {
    pub fn new(db: fjall::Database) -> Self {
        let kill_switch = Arc::new(AtomicBool::new(false));
        let completed_timestamp = Arc::new(AtomicU64::new(0));
        let (sender, receiver) = flume::bounded::<WriterMsg>(1000);

        let ks = kill_switch.clone();
        let completed = completed_timestamp.clone();

        let join_handle = std::thread::Builder::new()
            .name("moor-batch-writer".to_string())
            .spawn(move || {
                gdt_cpus::set_thread_priority(ThreadPriority::Background).ok();
                Self::writer_loop(db, receiver, ks, completed);
            })
            .expect("failed to spawn batch writer thread");

        Self {
            sender,
            kill_switch,
            completed_timestamp,
            join_handle: Mutex::new(Some(join_handle)),
        }
    }

    fn writer_loop(
        db: fjall::Database,
        receiver: Receiver<WriterMsg>,
        kill_switch: Arc<AtomicBool>,
        completed_timestamp: Arc<AtomicU64>,
    ) {
        // Coalescing buffers keyed by partition name
        let mut buffers: HashMap<String, PartitionBuffer> = HashMap::new();
        let mut newest_timestamp = Timestamp(0);
        let mut last_flush = Instant::now();
        let mut last_flush_duration = Duration::ZERO;
        let mut total_pending = 0usize;

        loop {
            if kill_switch.load(Ordering::Relaxed) {
                // Drain remaining messages, coalescing everything for one final flush
                let mut barrier_replies = Vec::new();
                while let Ok(msg) = receiver.try_recv() {
                    match msg {
                        WriterMsg::Write(batch) => {
                            Self::merge_batch(
                                &mut buffers,
                                batch,
                                &mut newest_timestamp,
                                &mut total_pending,
                            );
                        }
                        WriterMsg::Barrier(ts, reply) => {
                            if ts > newest_timestamp {
                                newest_timestamp = ts;
                            }
                            barrier_replies.push(reply);
                        }
                    }
                }
                // One final flush for all pending data
                if total_pending > 0 {
                    info!(
                        "BatchWriter shutdown: flushing {} pending ops",
                        total_pending
                    );
                    Self::flush_all(&db, &mut buffers, &mut total_pending);
                }
                completed_timestamp.store(newest_timestamp.0, Ordering::Release);
                // Reply to all barriers after flush completes
                for reply in barrier_replies {
                    reply.send(()).ok();
                }
                break;
            }

            // Force flush if we've hit hard limits (even under pressure)
            let force_flush = total_pending >= MAX_PENDING_OPS
                || (total_pending > 0 && last_flush.elapsed() >= MAX_COALESCE_INTERVAL);

            if force_flush {
                last_flush_duration = Self::flush_all(&db, &mut buffers, &mut total_pending);
                completed_timestamp.store(newest_timestamp.0, Ordering::Release);
                last_flush = Instant::now();
            }

            // Receive with short timeout
            match receiver.recv_timeout(Duration::from_millis(10)) {
                Ok(WriterMsg::Write(batch)) => {
                    Self::merge_batch(
                        &mut buffers,
                        batch,
                        &mut newest_timestamp,
                        &mut total_pending,
                    );

                    // Decide: flush immediately or coalesce?
                    // Coalesce only when under pressure (slow flushes or high pending count)
                    let under_pressure = last_flush_duration > PRESSURE_FLUSH_DURATION
                        || total_pending > PRESSURE_PENDING_THRESHOLD;

                    if !under_pressure && total_pending > 0 {
                        // Normal operation: flush immediately for durability
                        last_flush_duration =
                            Self::flush_all(&db, &mut buffers, &mut total_pending);
                        completed_timestamp.store(newest_timestamp.0, Ordering::Release);
                        last_flush = Instant::now();
                    }
                }
                Ok(WriterMsg::Barrier(ts, reply)) => {
                    if ts > newest_timestamp {
                        newest_timestamp = ts;
                    }
                    // Always flush on barrier
                    last_flush_duration = Self::flush_all(&db, &mut buffers, &mut total_pending);
                    completed_timestamp.store(newest_timestamp.0, Ordering::Release);
                    last_flush = Instant::now();
                    reply.send(()).ok();
                }
                Err(flume::RecvTimeoutError::Timeout) => continue,
                Err(flume::RecvTimeoutError::Disconnected) => break,
            }
        }
    }

    /// Merge a commit batch into the coalescing buffers.
    fn merge_batch(
        buffers: &mut HashMap<String, PartitionBuffer>,
        batch: CommitBatch,
        newest_timestamp: &mut Timestamp,
        total_pending: &mut usize,
    ) {
        if batch.timestamp > *newest_timestamp {
            *newest_timestamp = batch.timestamp;
        }

        for op in batch.operations {
            let partition_name = op.partition.name().to_string();
            let buffer = buffers
                .entry(partition_name)
                .or_insert_with(|| PartitionBuffer::new(op.partition.clone()));

            let prev_len = buffer.len();
            match op.op_type {
                BatchOpType::Insert { key, value } => buffer.insert(key, value),
                BatchOpType::Delete { key } => buffer.delete(key),
            }
            // Only count as added if this was a new key
            if buffer.len() > prev_len {
                *total_pending += 1;
            }
        }
    }

    /// Flush all pending operations to fjall. Returns the flush duration.
    fn flush_all(
        db: &fjall::Database,
        buffers: &mut HashMap<String, PartitionBuffer>,
        total_pending: &mut usize,
    ) -> Duration {
        if *total_pending == 0 {
            return Duration::ZERO;
        }

        let start = Instant::now();
        let op_count = *total_pending;

        let mut write_batch = db.batch();

        for buffer in buffers.values_mut() {
            if buffer.is_empty() {
                continue;
            }
            // Collect to avoid borrow conflict with keyspace
            let ops: Vec<_> = buffer.drain().collect();
            for (key, op) in ops {
                match op {
                    PendingOp::Insert(value) => match value.encode() {
                        Ok(encoded) => {
                            write_batch.insert(&buffer.keyspace, &key, &encoded);
                        }
                        Err(e) => {
                            error!("Failed to encode batch value: {e}");
                        }
                    },
                    PendingOp::Delete => {
                        write_batch.remove(&buffer.keyspace, &key);
                    }
                }
            }
        }

        if let Err(e) = write_batch.commit() {
            error!("Failed to commit write batch: {}", e);
        }

        *total_pending = 0;

        let elapsed = start.elapsed();
        if elapsed > Duration::from_secs(1) {
            warn!("Slow fjall flush: {} ops took {:?}", op_count, elapsed);
        }
        elapsed
    }

    pub fn write(&self, batch: CommitBatch) {
        let ts = batch.timestamp;
        let op_count = batch.operations.len();
        let msg = WriterMsg::Write(batch);

        match self.sender.try_send(msg) {
            Ok(()) => {}
            Err(flume::TrySendError::Full(msg)) => {
                warn!(
                    "BatchWriter backpressure: queue full, blocking on commit {} ({} ops)",
                    ts.0, op_count
                );
                let start = Instant::now();
                if let Err(e) = self.sender.send(msg) {
                    error!("Failed to send batch to writer: {}", e);
                    return;
                }
                let elapsed = start.elapsed();
                if elapsed > Duration::from_secs(1) {
                    warn!(
                        "BatchWriter backpressure: blocked {} for {:?}",
                        ts.0, elapsed
                    );
                }
            }
            Err(flume::TrySendError::Disconnected(_)) => {
                error!("BatchWriter channel disconnected");
            }
        }
    }

    pub fn send_barrier(&self, timestamp: Timestamp) {
        let (send, _recv) = oneshot::channel();
        if let Err(e) = self.sender.send(WriterMsg::Barrier(timestamp, send)) {
            warn!("Failed to send barrier: {}", e);
        }
    }

    pub fn completed_timestamp(&self) -> u64 {
        self.completed_timestamp.load(Ordering::Acquire)
    }

    pub fn wait_for_barrier(&self, timestamp: Timestamp, timeout: Duration) -> Result<(), String> {
        let start = Instant::now();
        while start.elapsed() < timeout {
            let completed = self.completed_timestamp.load(Ordering::Acquire);
            if completed >= timestamp.0 {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        Err(format!("Timeout waiting for write barrier {}", timestamp.0))
    }

    pub fn stop(&self) {
        self.kill_switch.store(true, Ordering::SeqCst);

        let mut jh = self.join_handle.lock().unwrap();
        if let Some(handle) = jh.take() {
            handle.join().ok();
        }
    }
}

impl Drop for BatchWriter {
    fn drop(&mut self) {
        self.stop();
    }
}
