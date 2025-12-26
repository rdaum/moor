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

//! Fjall-backed persistence provider with per-type encoding strategies.
//!
//! This module implements a `Provider` using Fjall (an embedded LSM-tree database) as the backing
//! store. The key architectural feature is **per-type encoding**: each data type can use its own
//! optimal serialization strategy via the `EncodeFor` trait.
//!
//! ## Encoding Strategies
//!
//! Different types use different encoding approaches for performance and efficiency:
//!
//! - **Zerocopy types** (`Obj`, `BitEnum`, etc.): Direct byte representation using the `zerocopy`
//!   crate's `IntoBytes`/`FromBytes` traits. No serialization overhead.
//!
//! - **ByteView wrappers** (`ObjSet`, `PropPerms`): Zero-copy passthrough - these types already
//!   hold a `ByteView` internally, so encoding just extracts the view via `AsRef<ByteView>` and
//!   decoding uses `From<ByteView>`.
//!
//! - **FlatBuffer types** (`ProgramType`, `Var`, `VerbDefs`, `PropDefs`): Uses FlatBuffers via
//!   the `planus` crate for efficient schema-based serialization with forward/backward
//!   compatibility. `Var` uses `var_to_db_flatbuffer` which allows lambdas and anonymous object
//!   references for DB storage.
//!
//! - **UTF-8 types** (`StringHolder`): Direct UTF-8 byte encoding without additional framing.
//!
//! ## Background Writing
//!
//! Writes are performed asynchronously on a background thread to avoid blocking the main
//! transaction path. The provider maintains pending operation tracking to ensure read-after-write
//! consistency: reads check pending writes before hitting the backing store.

use crate::{
    db_counters,
    tx_management::{EncodeFor, Error, Timestamp},
};
use byteview::ByteView;
use dashmap::{DashMap, DashSet};
use fjall::UserValue;
use flume::Sender;
use gdt_cpus::ThreadPriority;
use moor_common::util::PerfTimerGuard;
use planus::{ReadAsRoot, WriteAsOffset};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64},
    },
    thread::JoinHandle,
    time::Duration,
};
use tracing::{error, warn};

use crate::THREAD_JOIN_TIMEOUT;

/// Join a thread with a timeout.
///
/// Returns `Ok(())` if the thread joined successfully within the timeout,
/// or `Err(JoinHandle)` if the timeout elapsed (returning the handle for potential retry).
///
/// This uses polling with `is_finished()` since Rust's standard library doesn't
/// provide a `join_timeout()` method on `JoinHandle`.
fn join_with_timeout<T>(
    handle: JoinHandle<T>,
    timeout: Duration,
    thread_name: &str,
) -> Result<T, JoinHandle<T>> {
    let start = std::time::Instant::now();
    let poll_interval = Duration::from_millis(10);

    // Poll until thread finishes or timeout elapses
    while !handle.is_finished() {
        if start.elapsed() >= timeout {
            warn!(
                "Thread '{}' did not terminate within {:?}, continuing shutdown",
                thread_name, timeout
            );
            return Err(handle);
        }
        std::thread::sleep(poll_interval);
    }

    // Thread is finished, join should be immediate
    match handle.join() {
        Ok(result) => Ok(result),
        Err(e) => {
            warn!("Thread '{}' panicked during shutdown: {:?}", thread_name, e);
            // Thread panicked, but we still "joined" it - just report and continue
            std::panic::resume_unwind(e);
        }
    }
}

// ==================== Test Hooks ====================
// These hooks allow tests to control barrier processing for deterministic failure testing.

#[cfg(test)]
pub(crate) mod test_hooks {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Condvar, Mutex};

    /// When true, background threads will block before processing barriers
    static BLOCK_BARRIERS: AtomicBool = AtomicBool::new(false);

    /// Condition variable for waiting/signaling
    static BARRIER_CONDVAR: (Mutex<()>, Condvar) = (Mutex::new(()), Condvar::new());

    /// Counter for how many threads are currently blocked
    static BLOCKED_COUNT: AtomicUsize = AtomicUsize::new(0);

    /// Signal that barrier processing should be blocked.
    /// Call this before starting operations that will trigger barriers.
    pub fn block_barrier_processing() {
        BLOCK_BARRIERS.store(true, Ordering::SeqCst);
    }

    /// Release all blocked barrier processing and allow normal operation.
    pub fn unblock_barrier_processing() {
        BLOCK_BARRIERS.store(false, Ordering::SeqCst);
        let (_lock, cvar) = &BARRIER_CONDVAR;
        cvar.notify_all();
    }

    /// Returns the number of threads currently blocked waiting to process barriers.
    pub fn blocked_thread_count() -> usize {
        BLOCKED_COUNT.load(Ordering::SeqCst)
    }

    /// Reset all test hooks to default state. Call at start of each test.
    pub fn reset() {
        BLOCK_BARRIERS.store(false, Ordering::SeqCst);
        BLOCKED_COUNT.store(0, Ordering::SeqCst);
        // Wake any lingering blocked threads
        let (_lock, cvar) = &BARRIER_CONDVAR;
        cvar.notify_all();
    }

    /// Called by background thread before processing a barrier.
    /// Will block if `block_barrier_processing()` was called.
    pub(super) fn wait_if_blocked() {
        if BLOCK_BARRIERS.load(Ordering::SeqCst) {
            BLOCKED_COUNT.fetch_add(1, Ordering::SeqCst);
            let (lock, cvar) = &BARRIER_CONDVAR;
            let guard = lock.lock().unwrap();
            // Wait until unblocked
            let _guard = cvar
                .wait_while(guard, |_| BLOCK_BARRIERS.load(Ordering::SeqCst))
                .unwrap();
            BLOCKED_COUNT.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

// Background thread operations work with pre-encoded bytes
enum WriteOp<Domain>
where
    Domain: Clone + Eq + PartialEq + std::hash::Hash + Send + Sync,
{
    /// Insert with pre-encoded key and value bytes
    Insert(Vec<u8>, UserValue, Domain), // key_bytes, value_bytes, domain (for pending ops tracking)
    /// Delete with pre-encoded key bytes
    Delete(Vec<u8>, Domain), // key_bytes, domain (for pending ops tracking)
    /// Barrier marker for snapshot consistency - reply when all writes up to this timestamp are complete
    Barrier(Timestamp, oneshot::Sender<()>),
}

/// A backing persistence provider that fills the DB cache from a Fjall partition.
#[derive(Clone)]
pub(crate) struct FjallProvider<Domain, Codomain>
where
    Domain: Clone + Eq + PartialEq + std::hash::Hash + Send + Sync,
    Codomain: Clone + PartialEq + Send + Sync,
{
    fjall_partition: fjall::PartitionHandle,
    ops: Sender<WriteOp<Domain>>,
    kill_switch: Arc<AtomicBool>,
    /// Keys that have been written but write hasn't flushed to backing store yet.
    /// Uses lock-free DashMap for high-frequency concurrent reads.
    pending_writes: Arc<DashMap<Domain, (Timestamp, Codomain)>>,
    /// Keys that have been deleted but delete hasn't flushed to backing store yet.
    /// Uses lock-free DashSet for high-frequency concurrent reads.
    pending_deletes: Arc<DashSet<Domain>>,
    /// Set of domains that have been checked and found to not exist (tombstones/misses).
    /// This is shared across all transactions to avoid redundant database lookups.
    /// Uses lock-free DashSet for high-frequency concurrent reads.
    tombstones: Arc<DashSet<Domain>>,
    /// Atomic tracking of the highest completed barrier timestamp
    completed_barrier: Arc<AtomicU64>,
    jh: Arc<Mutex<Option<JoinHandle<()>>>>,
}

fn decode_codomain_with_ts<P, Codomain>(
    provider: &P,
    user_value: UserValue,
) -> Result<(Timestamp, Codomain), Error>
where
    P: EncodeFor<Codomain, Stored = ByteView>,
{
    let result: ByteView = user_value.into();
    let ts = Timestamp(u64::from_le_bytes(result[0..8].try_into().unwrap()));
    let codomain_bytes = result.slice(8..);
    let codomain = provider.decode(codomain_bytes)?;
    Ok((ts, codomain))
}

fn encode_codomain_with_ts<P, Codomain>(
    provider: &P,
    ts: Timestamp,
    codomain: &Codomain,
) -> Result<UserValue, Error>
where
    P: EncodeFor<Codomain, Stored = ByteView>,
{
    let codomain_stored = provider.encode(codomain)?;
    let mut result = Vec::with_capacity(8 + codomain_stored.len());
    result.extend_from_slice(&ts.0.to_le_bytes());
    result.extend_from_slice(&codomain_stored);
    Ok(UserValue::from(ByteView::from(result)))
}

impl<Domain, Codomain> FjallProvider<Domain, Codomain>
where
    Domain: Clone + Eq + PartialEq + std::hash::Hash + Send + Sync + 'static,
    Codomain: Clone + PartialEq + Send + Sync + 'static,
{
    pub fn new(relation_name: &str, fjall_partition: fjall::PartitionHandle) -> Self
    where
        Self: EncodeFor<Domain, Stored = ByteView> + EncodeFor<Codomain, Stored = ByteView>,
    {
        let kill_switch = Arc::new(AtomicBool::new(false));
        let (ops_tx, ops_rx) = flume::unbounded::<WriteOp<Domain>>();
        let pending_writes = Arc::new(DashMap::new());
        let pending_deletes = Arc::new(DashSet::new());
        let tombstones = Arc::new(DashSet::new());
        let completed_barrier = Arc::new(AtomicU64::new(0));

        let fj = fjall_partition.clone();
        let ks = kill_switch.clone();
        let pending_writes_bg = pending_writes.clone();
        let pending_deletes_bg = pending_deletes.clone();
        let completed_barrier_bg = completed_barrier.clone();
        let thread_name = format!("moor-w-{relation_name}");
        let tb = std::thread::Builder::new().name(thread_name);
        let jh = tb
            .spawn(move || {
                gdt_cpus::set_thread_priority(ThreadPriority::Background).ok();
                loop {
                    // Check kill_switch at top of loop
                    if ks.load(std::sync::atomic::Ordering::SeqCst) {
                        // Drain any remaining operations before exiting
                        while let Ok(op) = ops_rx.try_recv() {
                            match op {
                                WriteOp::Insert(key_bytes, value, _domain) => {
                                    let _ = fj.insert(ByteView::from(key_bytes), value);
                                }
                                WriteOp::Delete(key_bytes, _domain) => {
                                    let _ = fj.remove(ByteView::from(key_bytes));
                                }
                                WriteOp::Barrier(timestamp, reply) => {
                                    // Test hook: allow tests to block barrier processing
                                    #[cfg(test)]
                                    test_hooks::wait_if_blocked();

                                    completed_barrier_bg
                                        .store(timestamp.0, std::sync::atomic::Ordering::SeqCst);
                                    reply.send(()).ok();
                                }
                            }
                        }
                        break;
                    }

                    // Use timeout so we can periodically check kill_switch
                    let op = match ops_rx.recv_timeout(Duration::from_millis(100)) {
                        Ok(op) => op,
                        Err(flume::RecvTimeoutError::Timeout) => continue,
                        Err(flume::RecvTimeoutError::Disconnected) => break,
                    };

                    match op {
                        WriteOp::Insert(key_bytes, value, domain) => {
                            // Bytes are already encoded by the per-type EncodeFor impl!
                            // Perform the actual write
                            let write_result = fj.insert(ByteView::from(key_bytes), value);

                            // Remove from pending operations after completion (success or failure)
                            // DashMap::remove is lock-free
                            pending_writes_bg.remove(&domain);

                            if let Err(e) = write_result {
                                error!("failed to insert into database: {}", e);
                            }
                        }
                        WriteOp::Delete(key_bytes, domain) => {
                            // Bytes are already encoded by the per-type EncodeFor impl!
                            // Perform the actual delete
                            let delete_result = fj.remove(ByteView::from(key_bytes));

                            // Remove from pending operations after completion (success or failure)
                            // DashSet::remove is lock-free
                            pending_deletes_bg.remove(&domain);

                            if let Err(e) = delete_result {
                                error!("failed to delete from database: {}", e);
                            }
                        }
                        WriteOp::Barrier(timestamp, reply) => {
                            // Test hook: allow tests to block barrier processing
                            #[cfg(test)]
                            test_hooks::wait_if_blocked();

                            // Mark this barrier as completed and reply
                            completed_barrier_bg
                                .store(timestamp.0, std::sync::atomic::Ordering::SeqCst);
                            // Reply to indicate barrier is processed
                            reply.send(()).ok();
                        }
                    }
                }
            })
            .expect("failed to spawn fjall-write");
        Self {
            fjall_partition,
            ops: ops_tx,
            kill_switch,
            pending_writes,
            pending_deletes,
            tombstones,
            completed_barrier,
            jh: Arc::new(Mutex::new(Some(jh))),
        }
    }

    pub fn partition(&self) -> &fjall::PartitionHandle {
        &self.fjall_partition
    }

    /// Send a barrier message to track transaction timestamp without waiting.
    /// This is used after write transactions commit to track their completion.
    pub fn send_barrier(&self, barrier_timestamp: Timestamp) -> Result<(), Error> {
        // Check if we've already processed this barrier or a later one
        let completed = self
            .completed_barrier
            .load(std::sync::atomic::Ordering::Acquire);
        if completed >= barrier_timestamp.0 {
            return Ok(());
        }

        let (send, _recv) = oneshot::channel();

        // Send barrier message to background thread but don't wait for response
        if let Err(e) = self.ops.send(WriteOp::Barrier(barrier_timestamp, send)) {
            return Err(Error::StorageFailure(format!(
                "failed to send barrier message: {e}"
            )));
        }

        Ok(())
    }

    /// Send a barrier and return the receiver for waiting.
    ///
    /// This is used for parallel barrier waiting: the caller can collect receivers
    /// from multiple providers and wait on them concurrently.
    ///
    /// Returns `Ok(None)` if the barrier was already completed, `Ok(Some(receiver))`
    /// if a barrier message was sent, or an error if sending failed.
    pub fn send_barrier_with_reply(
        &self,
        barrier_timestamp: Timestamp,
    ) -> Result<Option<oneshot::Receiver<()>>, Error> {
        // Check if we've already processed this barrier or a later one
        let completed = self
            .completed_barrier
            .load(std::sync::atomic::Ordering::Acquire);
        if completed >= barrier_timestamp.0 {
            return Ok(None);
        }

        // Send a new barrier and return the receiver
        let (send, recv) = oneshot::channel();

        if let Err(e) = self.ops.send(WriteOp::Barrier(barrier_timestamp, send)) {
            return Err(Error::StorageFailure(format!(
                "failed to send barrier message: {e}"
            )));
        }

        Ok(Some(recv))
    }

    /// Wait for all writes up to the specified barrier timestamp to be completed.
    /// This ensures that all pending writes submitted before this barrier are flushed
    /// to the backing store, providing a consistent point for snapshots.
    ///
    /// This function sends a new barrier and waits for it to be processed, rather than
    /// just polling for a previously-sent barrier. This ensures reliable synchronization
    /// even if the original barrier was lost or never sent.
    ///
    /// Note: The Relations layer now uses send_barrier_with_reply for parallel waiting,
    /// but this method is kept for individual provider testing and debugging.
    #[allow(dead_code)]
    pub fn wait_for_write_barrier(
        &self,
        barrier_timestamp: Timestamp,
        timeout: Duration,
    ) -> Result<(), Error> {
        // Use the new send_barrier_with_reply method
        let recv = match self.send_barrier_with_reply(barrier_timestamp)? {
            Some(recv) => recv,
            None => return Ok(()), // Already completed
        };

        // Wait for the barrier to be processed with timeout
        match recv.recv_timeout(timeout) {
            Ok(()) => Ok(()),
            Err(oneshot::RecvTimeoutError::Timeout) => Err(Error::StorageFailure(format!(
                "Timeout waiting for write barrier {}",
                barrier_timestamp.0
            ))),
            Err(oneshot::RecvTimeoutError::Disconnected) => Err(Error::StorageFailure(format!(
                "Provider thread disconnected while waiting for barrier {}",
                barrier_timestamp.0
            ))),
        }
    }
}

const MAX_TOMBSTONE_COUNT: usize = 100_000;

impl<Domain, Codomain> Provider<Domain, Codomain> for FjallProvider<Domain, Codomain>
where
    Domain: Clone + Eq + PartialEq + std::hash::Hash + Send + Sync,
    Codomain: Clone + PartialEq + Send + Sync,
    Self: EncodeFor<Domain, Stored = ByteView> + EncodeFor<Codomain, Stored = ByteView>,
{
    fn get(&self, domain: &Domain) -> Result<Option<(Timestamp, Codomain)>, Error> {
        let _t = PerfTimerGuard::new(&db_counters().provider_tuple_check);

        // Check pending operations and tombstones using lock-free lookups.
        //
        // ORDERING SAFETY: Although these are separate operations without a shared lock,
        // the ordering is correct because mutations maintain these invariants:
        // - put(): inserts to pending_writes, THEN removes from pending_deletes
        // - del(): inserts to pending_deletes, THEN removes from pending_writes
        // - background thread: removes from pending_writes/pending_deletes after DB write
        //
        // By checking deletes first, then writes, we get correct last-write-wins semantics:
        // - If del() is in progress: we see the delete (correct)
        // - If put() is in progress: we see the write (correct)
        // - If put() races with del(): either order gives a valid snapshot

        // If pending delete, definitely doesn't exist
        if self.pending_deletes.contains(domain) {
            return Ok(None);
        }

        // If pending write, return that value
        if let Some(entry) = self.pending_writes.get(domain) {
            let (ts, value) = entry.value();
            return Ok(Some((*ts, value.clone())));
        }

        // If tombstoned, we know it doesn't exist - no need to hit database
        if self.tombstones.contains(domain) {
            return Ok(None);
        }

        // 2. Only hit backing store if not in pending ops or tombstones
        let _t = PerfTimerGuard::new(&db_counters().provider_tuple_load);
        let key_stored = <Self as EncodeFor<Domain>>::encode(self, domain)?;
        let Some(result) = self
            .fjall_partition
            .get(key_stored)
            .map_err(|e| Error::RetrievalFailure(e.to_string()))?
        else {
            // Database miss - add to tombstones to avoid future lookups
            // DashSet::insert is lock-free
            self.tombstones.insert(domain.clone());

            // If tombstones set gets too large, clear it to bound memory usage
            if self.tombstones.len() > MAX_TOMBSTONE_COUNT {
                self.tombstones.clear();
            }

            return Ok(None);
        };
        let (ts, codomain) = decode_codomain_with_ts::<Self, Codomain>(self, result)?;
        Ok(Some((ts, codomain)))
    }

    fn put(&self, timestamp: Timestamp, domain: &Domain, codomain: &Codomain) -> Result<(), Error> {
        // Add to pending writes and clear from tombstones immediately (lock-free operations)
        self.pending_writes
            .insert(domain.clone(), (timestamp, codomain.clone()));
        // Also remove from pending deletes if it was there (overwriting a deleted key)
        self.pending_deletes.remove(domain);
        // Remove from tombstones since this key now exists
        self.tombstones.remove(domain);

        // Encode using per-type EncodeFor impl before sending to background thread
        let key_bytes = <Self as EncodeFor<Domain>>::encode(self, domain)?;
        let value = encode_codomain_with_ts::<Self, Codomain>(self, timestamp, codomain)?;

        // Send pre-encoded bytes to async operation
        if let Err(e) = self
            .ops
            .send(WriteOp::Insert(key_bytes.to_vec(), value, domain.clone()))
        {
            // If sending fails, remove from pending operations
            self.pending_writes.remove(domain);
            return Err(Error::StorageFailure(format!(
                "failed to insert into database: {e}"
            )));
        }
        Ok(())
    }

    fn del(&self, _timestamp: Timestamp, domain: &Domain) -> Result<(), Error> {
        // Add to pending deletes immediately (lock-free operations)
        self.pending_deletes.insert(domain.clone());
        // Also remove from pending writes if it was there
        self.pending_writes.remove(domain);

        // Encode using per-type EncodeFor impl before sending to background thread
        let key_bytes = <Self as EncodeFor<Domain>>::encode(self, domain)?;

        // Send pre-encoded bytes to async operation
        if let Err(e) = self
            .ops
            .send(WriteOp::Delete(key_bytes.to_vec(), domain.clone()))
        {
            // If sending fails, remove from pending operations
            self.pending_deletes.remove(domain);
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

        // Scan backing store first
        for entry in self.fjall_partition.iter() {
            let (key, value) = entry.map_err(|e| Error::RetrievalFailure(e.to_string()))?;
            let domain = <Self as EncodeFor<Domain>>::decode(self, key.clone().into())?;

            // Skip if this domain is pending deletion (lock-free check)
            if self.pending_deletes.contains(&domain) {
                continue;
            }

            let (ts, codomain) = decode_codomain_with_ts::<Self, Codomain>(self, value)?;
            if predicate(&domain, &codomain) {
                result.push((ts, domain, codomain));
            }
        }

        // Add pending writes that match the predicate (lock-free iteration)
        for entry in self.pending_writes.iter() {
            let domain = entry.key();
            let (ts, codomain) = entry.value();
            if predicate(domain, codomain) {
                result.push((*ts, domain.clone(), codomain.clone()));
            }
        }

        Ok(result)
    }

    fn stop(&self) -> Result<(), Error> {
        self.stop_internal()
    }
}

// Non-trait impl for stop - doesn't require EncodeFor constraints
impl<Domain, Codomain> FjallProvider<Domain, Codomain>
where
    Domain: Clone + Eq + PartialEq + std::hash::Hash + Send + Sync,
    Codomain: Clone + PartialEq + Send + Sync,
{
    fn stop_internal(&self) -> Result<(), Error> {
        self.kill_switch
            .store(true, std::sync::atomic::Ordering::SeqCst);

        // Join the background thread with timeout - it will wake up from recv_timeout and see kill_switch
        let mut jh = self.jh.lock().unwrap();
        if let Some(handle) = jh.take() {
            // Use timeout to prevent indefinite hang if thread is deadlocked
            if let Err(abandoned_handle) =
                join_with_timeout(handle, THREAD_JOIN_TIMEOUT, "fjall-provider-writer")
            {
                // Thread didn't terminate - store handle back for potential retry in Drop
                // but don't block shutdown
                *jh = Some(abandoned_handle);
            }
        }

        Ok(())
    }
}

impl<Domain, Codomain> Drop for FjallProvider<Domain, Codomain>
where
    Domain: Clone + Eq + PartialEq + std::hash::Hash + Send + Sync,
    Codomain: Clone + PartialEq + Send + Sync,
{
    fn drop(&mut self) {
        // stop_internal already handles the join with timeout
        self.stop_internal().ok();
        // If thread still exists (timeout during stop), try one more time with shorter timeout
        let mut jh = self.jh.lock().unwrap();
        if let Some(handle) = jh.take() {
            // Final attempt - if still hanging, let the thread be abandoned
            let _ = join_with_timeout(handle, Duration::from_secs(1), "fjall-provider-writer-drop");
        }
    }
}

// ============================================================================
// Fjall Codec - Shared encoding logic for FjallProvider and SnapshotLoader
// ============================================================================

/// Zero-sized type that provides encoding/decoding for all Fjall-stored types.
/// This allows both FjallProvider and SnapshotLoader to share the same encoding logic.
#[derive(Clone, Copy)]
pub(crate) struct FjallCodec;

// ============================================================================
// Per-Type Encoding Implementations for FjallCodec
// ============================================================================
// Each type gets its own EncodeFor impl, allowing custom encoding logic

use crate::{AnonymousObjectMetadata, ObjAndUUIDHolder, StringHolder, provider::Provider};
use moor_common::{
    model::{ObjFlag, ObjSet, PropDefs, PropPerms, VerbDefs},
    util::BitEnum,
};
use moor_schema::convert::{
    program_to_stored, stored_to_program, var_from_db_flatbuffer_ref, var_to_db_flatbuffer,
};
use moor_var::{Obj, Var, program::ProgramType};
// Per-type encoding implementations
// Each type can be encoded regardless of whether it's used as Domain or Codomain
// We use a blanket impl for all FjallProvider<Domain, Codomain> combinations

/// Encoding for zerocopy types (IntoBytes + FromBytes) - zero-copy serialization
macro_rules! impl_zerocopy_encode {
    ($type:ty) => {
        impl EncodeFor<$type> for FjallCodec {
            type Stored = ByteView;

            fn encode(&self, value: &$type) -> Result<Self::Stored, Error> {
                use zerocopy::IntoBytes;
                Ok(ByteView::from(IntoBytes::as_bytes(value)))
            }

            fn decode(&self, stored: Self::Stored) -> Result<$type, Error> {
                use zerocopy::FromBytes;
                let bytes = stored.as_ref();
                if bytes.len() != std::mem::size_of::<$type>() {
                    return Err(Error::EncodingFailure);
                }

                // Handle potentially unaligned data safely
                let mut aligned_buffer = vec![0u8; std::mem::size_of::<$type>()];
                aligned_buffer.copy_from_slice(bytes);

                <$type>::read_from_bytes(&aligned_buffer).map_err(|_| Error::EncodingFailure)
            }
        }
    };
}

/// Encoding for types that wrap ByteView - zero-copy passthrough
macro_rules! impl_byteview_wrapper_encode {
    ($type:ty) => {
        impl EncodeFor<$type> for FjallCodec {
            type Stored = ByteView;

            fn encode(&self, value: &$type) -> Result<Self::Stored, Error> {
                Ok(AsRef::<ByteView>::as_ref(value).clone())
            }

            fn decode(&self, stored: Self::Stored) -> Result<$type, Error> {
                Ok(<$type>::from(stored))
            }
        }
    };
}

// Zerocopy types - direct byte access, no serialization overhead
impl_zerocopy_encode!(Obj);
impl_zerocopy_encode!(ObjAndUUIDHolder);
impl_zerocopy_encode!(AnonymousObjectMetadata);
impl_zerocopy_encode!(BitEnum<ObjFlag>);

// ByteView wrappers - zero-copy passthrough
impl_byteview_wrapper_encode!(ObjSet);
impl_byteview_wrapper_encode!(PropPerms);

// Var - FlatBuffer encoding for DB storage (allows lambdas and anonymous objects)
impl EncodeFor<Var> for FjallCodec {
    type Stored = ByteView;

    fn encode(&self, value: &Var) -> Result<Self::Stored, Error> {
        // Convert to FlatBuffer struct
        let fb_var = var_to_db_flatbuffer(value).map_err(|_| Error::EncodingFailure)?;

        // Serialize to bytes
        let mut builder = planus::Builder::new();
        let offset = fb_var.prepare(&mut builder);
        let bytes = builder.finish(offset, None);

        Ok(ByteView::from(bytes))
    }

    fn decode(&self, stored: Self::Stored) -> Result<Var, Error> {
        // Parse FlatBuffer and convert directly from ref (avoids intermediate owned struct copy)
        let fb_ref =
            moor_schema::var::VarRef::read_as_root(&stored).map_err(|_| Error::EncodingFailure)?;
        var_from_db_flatbuffer_ref(fb_ref).map_err(|_| Error::EncodingFailure)
    }
}

// FlatBuffer types - VerbDefs and PropDefs
impl EncodeFor<VerbDefs> for FjallCodec {
    type Stored = ByteView;

    fn encode(&self, value: &VerbDefs) -> Result<Self::Stored, Error> {
        let fb_verbdefs = moor_schema::convert::verbdefs_to_flatbuffer(value)
            .map_err(|_| Error::EncodingFailure)?;
        let mut builder = planus::Builder::new();
        let offset = fb_verbdefs.prepare(&mut builder);
        let bytes = builder.finish(offset, None);
        Ok(ByteView::from(bytes))
    }

    fn decode(&self, stored: Self::Stored) -> Result<VerbDefs, Error> {
        let fb_ref = moor_schema::common::VerbDefsRef::read_as_root(&stored)
            .map_err(|_| Error::EncodingFailure)?;
        let fb_verbdefs: moor_schema::common::VerbDefs =
            fb_ref.try_into().map_err(|_| Error::EncodingFailure)?;
        moor_schema::convert::verbdefs_from_flatbuffer(&fb_verbdefs)
            .map_err(|_| Error::EncodingFailure)
    }
}

impl EncodeFor<PropDefs> for FjallCodec {
    type Stored = ByteView;

    fn encode(&self, value: &PropDefs) -> Result<Self::Stored, Error> {
        let fb_propdefs = moor_schema::convert::propdefs_to_flatbuffer(value)
            .map_err(|_| Error::EncodingFailure)?;
        let mut builder = planus::Builder::new();
        let offset = fb_propdefs.prepare(&mut builder);
        let bytes = builder.finish(offset, None);
        Ok(ByteView::from(bytes))
    }

    fn decode(&self, stored: Self::Stored) -> Result<PropDefs, Error> {
        let fb_ref = moor_schema::common::PropDefsRef::read_as_root(&stored)
            .map_err(|_| Error::EncodingFailure)?;
        let fb_propdefs: moor_schema::common::PropDefs =
            fb_ref.try_into().map_err(|_| Error::EncodingFailure)?;
        moor_schema::convert::propdefs_from_flatbuffer(&fb_propdefs)
            .map_err(|_| Error::EncodingFailure)
    }
}

// StringHolder - direct UTF-8 encoding
impl EncodeFor<StringHolder> for FjallCodec {
    type Stored = ByteView;

    fn encode(&self, value: &StringHolder) -> Result<Self::Stored, Error> {
        Ok(ByteView::from(value.0.as_bytes()))
    }

    fn decode(&self, stored: Self::Stored) -> Result<StringHolder, Error> {
        let s = String::from_utf8(stored.to_vec()).map_err(|_| Error::EncodingFailure)?;
        Ok(StringHolder(s))
    }
}

// ProgramType uses flatbuffer encoding - see below

// ============================================================================
// ProgramType - uses FlatBuffer encoding via program_convert
// ============================================================================

impl EncodeFor<ProgramType> for FjallCodec {
    type Stored = ByteView;

    fn encode(&self, program: &ProgramType) -> Result<Self::Stored, Error> {
        match program {
            ProgramType::MooR(prog) => {
                let stored = program_to_stored(prog)
                    .map_err(|e| Error::StorageFailure(format!("Failed to encode program: {e}")))?;
                // StoredProgram is a ByteView wrapper - extract the inner ByteView
                Ok(AsRef::<ByteView>::as_ref(&stored).clone())
            }
        }
    }

    fn decode(&self, stored: Self::Stored) -> Result<ProgramType, Error> {
        use moor_var::program::stored_program::StoredProgram;

        let stored_program = StoredProgram::from(stored);

        // Read the FlatBuffer and extract the language union
        use moor_schema::program as fb;
        use planus::ReadAsRoot;

        let fb_program = fb::StoredProgramRef::read_as_root(stored_program.as_bytes())
            .map_err(|e| Error::StorageFailure(format!("Failed to read program: {e}")))?;

        let language = fb_program
            .language()
            .map_err(|e| Error::StorageFailure(format!("Failed to read language union: {e}")))?;

        // Match on language variant and construct appropriate ProgramType
        match language {
            fb::StoredProgramLanguageRef::StoredMooRProgram(_moor_ref) => {
                // Decode the full program using the existing function
                let program = stored_to_program(&stored_program).map_err(|e| {
                    Error::StorageFailure(format!("Failed to decode MooR program: {e}"))
                })?;
                Ok(ProgramType::MooR(program))
            }
        }
    }
}

// ============================================================================
// Blanket impl: FjallProvider delegates to FjallCodec for all types
// ============================================================================

impl<Domain, Codomain, T> EncodeFor<T> for FjallProvider<Domain, Codomain>
where
    Domain: Clone + Eq + PartialEq + std::hash::Hash + Send + Sync,
    Codomain: Clone + PartialEq + Send + Sync,
    FjallCodec: EncodeFor<T, Stored = ByteView>,
{
    type Stored = ByteView;

    fn encode(&self, value: &T) -> Result<Self::Stored, Error> {
        FjallCodec.encode(value)
    }

    fn decode(&self, stored: Self::Stored) -> Result<T, Error> {
        FjallCodec.decode(stored)
    }
}

// ============================================================================
// SequenceWriter - Background writer for sequence persistence
// ============================================================================

/// Background writer for sequence values.
///
/// Similar to FjallProvider but specialized for the fixed-size sequence array.
/// Writes are sent to a background thread to avoid blocking the commit path.
pub struct SequenceWriter {
    ops: Sender<[i64; 16]>,
    kill_switch: Arc<AtomicBool>,
    jh: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl SequenceWriter {
    /// Create a new sequence writer with a background thread.
    pub fn new(partition: fjall::PartitionHandle) -> Self {
        let kill_switch = Arc::new(AtomicBool::new(false));
        let (ops_tx, ops_rx) = flume::unbounded::<[i64; 16]>();

        let ks = kill_switch.clone();
        let jh = std::thread::Builder::new()
            .name("moor-seq-writer".to_string())
            .spawn(move || {
                gdt_cpus::set_thread_priority(ThreadPriority::Background).ok();
                loop {
                    if ks.load(std::sync::atomic::Ordering::Relaxed) {
                        // Drain remaining writes before exiting
                        while let Ok(seq_values) = ops_rx.try_recv() {
                            Self::write_sequences(&partition, &seq_values);
                        }
                        break;
                    }

                    match ops_rx.recv_timeout(Duration::from_millis(100)) {
                        Ok(seq_values) => {
                            Self::write_sequences(&partition, &seq_values);
                        }
                        Err(flume::RecvTimeoutError::Timeout) => continue,
                        Err(flume::RecvTimeoutError::Disconnected) => break,
                    }
                }
            })
            .expect("failed to spawn sequence writer thread");

        Self {
            ops: ops_tx,
            kill_switch,
            jh: Arc::new(Mutex::new(Some(jh))),
        }
    }

    fn write_sequences(partition: &fjall::PartitionHandle, seq_values: &[i64; 16]) {
        for (i, val) in seq_values.iter().enumerate() {
            if let Err(e) = partition.insert(i.to_le_bytes(), val.to_le_bytes()) {
                error!("Failed to persist sequence {}: {}", i, e);
            }
        }
    }

    /// Queue sequence values for background persistence.
    pub fn write(&self, seq_values: [i64; 16]) {
        if let Err(e) = self.ops.send(seq_values) {
            error!("Failed to queue sequence write: {}", e);
        }
    }

    /// Stop the background writer thread.
    pub fn stop(&self) {
        self.kill_switch
            .store(true, std::sync::atomic::Ordering::SeqCst);

        let mut jh = self.jh.lock().unwrap();
        if let Some(handle) = jh.take() {
            // Use timeout to prevent indefinite hang if thread is deadlocked
            if let Err(abandoned_handle) =
                join_with_timeout(handle, THREAD_JOIN_TIMEOUT, "moor-seq-writer")
            {
                // Thread didn't terminate - store handle back for potential retry in Drop
                *jh = Some(abandoned_handle);
            }
        }
    }
}

impl Drop for SequenceWriter {
    fn drop(&mut self) {
        self.stop();
        // If thread still exists after stop(), try one more time with shorter timeout
        let mut jh = self.jh.lock().unwrap();
        if let Some(handle) = jh.take() {
            let _ = join_with_timeout(handle, Duration::from_secs(1), "moor-seq-writer-drop");
        }
    }
}
