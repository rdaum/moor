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
use fjall::UserValue;
use flume::Sender;
use gdt_cpus::ThreadPriority;
use moor_common::util::PerfTimerGuard;
use planus::{ReadAsRoot, WriteAsOffset};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, RwLock, atomic::AtomicBool},
    thread::JoinHandle,
    time::Duration,
};
use tracing::error;

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
    /// Shared state tracking operations in-flight to background thread
    pending_ops: Arc<RwLock<PendingOperations<Domain, Codomain>>>,
    /// Set of domains that have been checked and found to not exist (tombstones/misses)
    /// This is shared across all transactions to avoid redundant database lookups
    tombstones: Arc<RwLock<HashSet<Domain>>>,
    /// Atomic tracking of the highest completed barrier timestamp
    completed_barrier: Arc<RwLock<u128>>,
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
    let ts = Timestamp(u128::from_le_bytes(result[0..16].try_into().unwrap()));
    let codomain_bytes = result.slice(16..);
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
    let mut result = Vec::with_capacity(16 + codomain_stored.len());
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
        let pending_ops = Arc::new(RwLock::new(PendingOperations::default()));
        let tombstones = Arc::new(RwLock::new(HashSet::new()));
        let completed_barrier = Arc::new(RwLock::new(0));

        let fj = fjall_partition.clone();
        let ks = kill_switch.clone();
        let pending_ops_bg = pending_ops.clone();
        let completed_barrier_bg = completed_barrier.clone();
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
                        Ok(WriteOp::Insert(key_bytes, value, domain)) => {
                            // Bytes are already encoded by the per-type EncodeFor impl!
                            // Perform the actual write
                            let write_result = fj.insert(ByteView::from(key_bytes), value);

                            // Remove from pending operations after completion (success or failure)
                            if let Ok(mut pending) = pending_ops_bg.write() {
                                pending.pending_writes.remove(&domain);
                            }

                            if let Err(e) = write_result {
                                error!("failed to insert into database: {}", e);
                            }
                        }
                        Ok(WriteOp::Delete(key_bytes, domain)) => {
                            // Bytes are already encoded by the per-type EncodeFor impl!
                            // Perform the actual delete
                            let delete_result = fj.remove(ByteView::from(key_bytes));

                            // Remove from pending operations after completion (success or failure)
                            if let Ok(mut pending) = pending_ops_bg.write() {
                                pending.pending_deletes.remove(&domain);
                            }

                            if let Err(e) = delete_result {
                                error!("failed to delete from database: {}", e);
                            }
                        }
                        Ok(WriteOp::Barrier(timestamp, reply)) => {
                            // Mark this barrier as completed and reply
                            *completed_barrier_bg.write().unwrap() = timestamp.0;
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
        let completed = *self.completed_barrier.read().unwrap();
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

    /// Wait for all writes up to the specified barrier timestamp to be completed.
    /// This ensures that all pending writes submitted before this barrier are flushed
    /// to the backing store, providing a consistent point for snapshots.
    /// Note: This only waits, it doesn't send the barrier - barriers must be sent separately.
    pub fn wait_for_write_barrier(
        &self,
        barrier_timestamp: Timestamp,
        timeout: Duration,
    ) -> Result<(), Error> {
        // Check if we've already processed this barrier or a later one
        let completed = *self.completed_barrier.read().unwrap();
        if completed >= barrier_timestamp.0 {
            return Ok(());
        }

        // Wait by polling the completed barrier timestamp
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            let completed = *self.completed_barrier.read().unwrap();
            if completed >= barrier_timestamp.0 {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(1));
        }

        Err(Error::StorageFailure(format!(
            "Timeout waiting for write barrier {}",
            barrier_timestamp.0
        )))
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

        // 1. Check both pending operations and tombstones together for consistency
        {
            let pending = self.pending_ops.read().map_err(|_| {
                Error::StorageFailure("Failed to acquire pending ops read lock".to_string())
            })?;
            let tombstones = self.tombstones.read().map_err(|_| {
                Error::StorageFailure("Failed to acquire tombstones read lock".to_string())
            })?;

            // If pending delete, definitely doesn't exist
            if pending.pending_deletes.contains(domain) {
                return Ok(None);
            }

            // If pending write, return that value
            if let Some((ts, value)) = pending.pending_writes.get(domain) {
                return Ok(Some((*ts, value.clone())));
            }

            // If tombstoned, we know it doesn't exist - no need to hit database
            if tombstones.contains(domain) {
                return Ok(None);
            }
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
            let mut tombstones = self.tombstones.write().map_err(|_| {
                Error::StorageFailure("Failed to acquire tombstones write lock".to_string())
            })?;
            tombstones.insert(domain.clone());

            // If tombstones set gets too large, clear it to bound memory usage
            if tombstones.len() > MAX_TOMBSTONE_COUNT {
                tombstones.clear();
            }

            return Ok(None);
        };
        let (ts, codomain) = decode_codomain_with_ts::<Self, Codomain>(self, result)?;
        Ok(Some((ts, codomain)))
    }

    fn put(&self, timestamp: Timestamp, domain: &Domain, codomain: &Codomain) -> Result<(), Error> {
        // Add to pending writes and clear from tombstones immediately
        {
            let mut pending = self.pending_ops.write().map_err(|_| {
                Error::StorageFailure("Failed to acquire pending ops write lock".to_string())
            })?;
            let mut tombstones = self.tombstones.write().map_err(|_| {
                Error::StorageFailure("Failed to acquire tombstones write lock".to_string())
            })?;

            pending
                .pending_writes
                .insert(domain.clone(), (timestamp, codomain.clone()));
            // Also remove from pending deletes if it was there (overwriting a deleted key)
            pending.pending_deletes.remove(domain);
            // Remove from tombstones since this key now exists
            tombstones.remove(domain);
        }

        // Encode using per-type EncodeFor impl before sending to background thread
        let key_bytes = <Self as EncodeFor<Domain>>::encode(self, domain)?;
        let value = encode_codomain_with_ts::<Self, Codomain>(self, timestamp, codomain)?;

        // Send pre-encoded bytes to async operation
        if let Err(e) = self
            .ops
            .send(WriteOp::Insert(key_bytes.to_vec(), value, domain.clone()))
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

        // Encode using per-type EncodeFor impl before sending to background thread
        let key_bytes = <Self as EncodeFor<Domain>>::encode(self, domain)?;

        // Send pre-encoded bytes to async operation
        if let Err(e) = self
            .ops
            .send(WriteOp::Delete(key_bytes.to_vec(), domain.clone()))
        {
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
            let domain = <Self as EncodeFor<Domain>>::decode(self, key.clone().into())?;

            // Skip if this domain is pending deletion
            if pending.pending_deletes.contains(&domain) {
                continue;
            }

            let (ts, codomain) = decode_codomain_with_ts::<Self, Codomain>(self, value)?;
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
        Ok(())
    }
}

impl<Domain, Codomain> Drop for FjallProvider<Domain, Codomain>
where
    Domain: Clone + Eq + PartialEq + std::hash::Hash + Send + Sync,
    Codomain: Clone + PartialEq + Send + Sync,
{
    fn drop(&mut self) {
        self.stop_internal().unwrap();
        let mut jh = self.jh.lock().unwrap();
        if let Some(jh) = jh.take() {
            jh.join().unwrap();
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
use moor_schema::convert::{program_to_stored, stored_to_program, var_to_db_flatbuffer};
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
        // Parse FlatBuffer
        let fb_ref =
            moor_schema::var::VarRef::read_as_root(&stored).map_err(|_| Error::EncodingFailure)?;

        // Convert to owned struct
        let fb_var: moor_schema::var::Var =
            fb_ref.try_into().map_err(|_| Error::EncodingFailure)?;

        // Decode to Var
        moor_schema::convert::var_from_db_flatbuffer(&fb_var).map_err(|_| Error::EncodingFailure)
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
        let program = stored_to_program(&stored_program)
            .map_err(|e| Error::StorageFailure(format!("Failed to decode program: {e}")))?;
        Ok(ProgramType::MooR(program))
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
