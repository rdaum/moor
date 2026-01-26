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
//! ## Batched Writing
//!
//! All providers share a single BatchCollector. During a commit's apply phase, each provider
//! adds its operations to the shared batch. After apply completes, MoorDB sends the entire
//! batch to a single BatchWriter thread for atomic persistence via fjall's WriteBatch API.
//! This reduces I/O contention and enables atomic multi-relation commits.

use crate::{
    db_counters,
    provider::batch_writer::BatchCollector,
    tx_management::{EncodeFor, Error, RelationCodomain, RelationDomain, Timestamp},
};
use byteview::ByteView;
use fjall::Slice;
use flume::Sender;
use gdt_cpus::ThreadPriority;
use moor_common::util::PerfTimerGuard;
use moor_common::util::signal_fatal_db_error;
use planus::{ReadAsRoot, WriteAsOffset};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, RwLock, atomic::AtomicBool},
    thread::JoinHandle,
    time::Duration,
};
use tracing::error;

/// Handle a fjall error, with special handling for Poisoned errors.
/// Returns true if this was a Poisoned error (caller should stop retrying).
fn handle_fjall_error(e: &fjall::Error, operation: &str) -> bool {
    if matches!(e, fjall::Error::Poisoned) {
        // Use the common fatal error handler - it will log once and signal shutdown
        signal_fatal_db_error(operation, "database poisoned (fsync failure)");
        true
    } else {
        error!("Database error during {operation}: {e}");
        false
    }
}

/// Tracks pending operations during a commit for read-your-writes consistency.
///
/// Even though writes go to the shared BatchCollector, we still track them here
/// so reads during the commit window see uncommitted changes.
struct PendingOperations<Domain, Codomain>
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
{
    /// Keys that have been deleted but delete hasn't flushed to backing store yet
    pending_deletes: HashSet<Domain>,
    /// Keys that have been written but write hasn't flushed to backing store yet
    pending_writes: HashMap<Domain, (Timestamp, Codomain)>,
}

impl<Domain, Codomain> Default for PendingOperations<Domain, Codomain>
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
{
    fn default() -> Self {
        Self {
            pending_deletes: HashSet::new(),
            pending_writes: HashMap::new(),
        }
    }
}

/// A backing persistence provider that uses a shared BatchCollector for writes.
///
/// All providers share a single BatchCollector. During a commit's apply phase,
/// each provider adds its operations to the shared batch. After apply completes,
/// MoorDB sends the entire batch to a single BatchWriter thread for atomic
/// persistence via fjall's WriteBatch API.
#[derive(Clone)]
pub(crate) struct FjallProvider<Domain, Codomain>
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
{
    fjall_keyspace: fjall::Keyspace,
    /// Shared batch collector - all providers add to this during commit
    batch_collector: Arc<BatchCollector>,
    /// Shared state tracking operations in-flight for read-your-writes consistency
    pending_ops: Arc<RwLock<PendingOperations<Domain, Codomain>>>,
    /// Set of domains that have been checked and found to not exist (tombstones/misses)
    /// This is shared across all transactions to avoid redundant database lookups
    tombstones: Arc<RwLock<HashSet<Domain>>>,
}

fn decode_codomain_with_ts<P, Codomain>(
    provider: &P,
    user_value: Slice,
) -> Result<(Timestamp, Codomain), Error>
where
    P: EncodeFor<Codomain, Stored = ByteView>,
{
    let result = ByteView::from(user_value);
    let ts = Timestamp(u64::from_le_bytes(result[0..8].try_into().unwrap()));
    let codomain_bytes = result.slice(8..);
    let codomain = provider.decode(codomain_bytes)?;
    Ok((ts, codomain))
}

fn encode_codomain_with_ts<P, Codomain>(
    provider: &P,
    ts: Timestamp,
    codomain: &Codomain,
) -> Result<Vec<u8>, Error>
where
    P: EncodeFor<Codomain, Stored = ByteView>,
{
    let codomain_stored = provider.encode(codomain)?;
    let mut result = Vec::with_capacity(8 + codomain_stored.len());
    result.extend_from_slice(&ts.0.to_le_bytes());
    result.extend_from_slice(&codomain_stored);
    Ok(result)
}

impl<Domain, Codomain> FjallProvider<Domain, Codomain>
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
{
    /// Create a new provider that uses the shared BatchCollector for writes.
    ///
    /// Unlike the old design, this does NOT spawn a background thread. All writes
    /// are collected into the shared batch and written by the central BatchWriter.
    pub fn new(
        _relation_name: &str,
        fjall_keyspace: fjall::Keyspace,
        batch_collector: Arc<BatchCollector>,
    ) -> Self {
        Self {
            fjall_keyspace,
            batch_collector,
            pending_ops: Arc::new(RwLock::new(PendingOperations::default())),
            tombstones: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    pub fn partition(&self) -> &fjall::Keyspace {
        &self.fjall_keyspace
    }
}

const MAX_TOMBSTONE_COUNT: usize = 100_000;

impl<Domain, Codomain> Provider<Domain, Codomain> for FjallProvider<Domain, Codomain>
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
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
            .fjall_keyspace
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

        // Encode and add to shared batch collector
        let key_bytes = <Self as EncodeFor<Domain>>::encode(self, domain)?;
        let value = encode_codomain_with_ts::<Self, Codomain>(self, timestamp, codomain)?;

        self.batch_collector
            .insert(self.fjall_keyspace.clone(), key_bytes.to_vec(), value);

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

        // Encode and add to shared batch collector
        let key_bytes = <Self as EncodeFor<Domain>>::encode(self, domain)?;

        self.batch_collector
            .delete(self.fjall_keyspace.clone(), key_bytes.to_vec());

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
        for entry in self.fjall_keyspace.iter() {
            let (key, value) = entry
                .into_inner()
                .map_err(|e| Error::RetrievalFailure(e.to_string()))?;
            let domain = <Self as EncodeFor<Domain>>::decode(self, ByteView::from(key))?;

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
        // No-op: providers no longer have their own background threads.
        // The BatchWriter is stopped at the MoorDB level.
        Ok(())
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
    Domain: RelationDomain,
    Codomain: RelationCodomain,
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
    pub fn new(keyspace: fjall::Keyspace) -> Self {
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
                            Self::write_sequences(&keyspace, &seq_values);
                        }
                        break;
                    }

                    match ops_rx.recv_timeout(Duration::from_millis(100)) {
                        Ok(seq_values) => {
                            Self::write_sequences(&keyspace, &seq_values);
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

    fn write_sequences(keyspace: &fjall::Keyspace, seq_values: &[i64; 16]) {
        for (i, val) in seq_values.iter().enumerate() {
            if let Err(e) = keyspace.insert(i.to_le_bytes(), val.to_le_bytes()) {
                handle_fjall_error(&e, &format!("sequence {i} persist"));
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
        if let Some(jh) = jh.take() {
            jh.join().unwrap();
        }
    }
}

impl Drop for SequenceWriter {
    fn drop(&mut self) {
        self.stop();
    }
}
