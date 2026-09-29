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

//! Ordered background writer for atomic Fjall transaction batches.
//!
//! Published transactions may reach this thread out of order because their CAS
//! winners enqueue from different task threads. The writer holds only those
//! out-of-order transactions and commits each contiguous transaction to Fjall
//! immediately as its own cross-keyspace write batch.

mod persistence;
mod workers;

use persistence::PreparedPersistence;
use workers::{WorkerGroup, Workers};

use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use ahash::AHashMap;
use fjall::PersistMode;
use flume::{Receiver, Sender};
use moor_common::model::HasUuid;
use moor_common::threading::spawn_efficient;
use moor_var::{Obj, Symbol, Var};
use tracing::{error, warn};
use uuid::Uuid;

use crate::{
    ObjAndUUIDHolder, db_counters,
    engine::property_definitions::PropertyDefinitionChange,
    provider::coordinator::{COMMIT_ADMISSION_CAPACITY, CommitAdmission},
    provider::fjall_relations::FjallRelations,
    provider::logical::{
        LogicalCommit, PreparedPropertyValueMutation, PreparedPropertyValueOp, SequenceUpdate,
    },
    provider::property_value_store::{
        PROPERTY_RECORD_KEY_BYTES, PropertyValueChain, PropertyValueChainLimits,
        encode_full_record, encode_list_append_record, encode_property_value_record_key,
        property_value_record_payload_bytes,
    },
    tx::{Error, Timestamp},
};
use moor_common::model::{WorldStateCountOp, WorldStateTimerOp};

#[cfg(test)]
use crate::provider::property_value_store::PROPERTY_VALUE_CHAIN_LIMITS;

/// A single operation to be written to fjall.
pub struct BatchOp {
    /// The fjall partition (keyspace) to write to
    pub partition: fjall::Keyspace,
    /// The operation type
    pub op_type: BatchOpType,
    /// Logical source of the operation for slow-write diagnostics.
    pub source: BatchOpSource,
}

/// Logical source of a batch operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BatchOpSource {
    Relation(&'static str),
    Property {
        relation: &'static str,
        object: Obj,
        uuid: Uuid,
    },
    #[cfg(test)]
    Internal(&'static str),
}

impl std::fmt::Display for BatchOpSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Relation(relation) => f.write_str(relation),
            #[cfg(test)]
            Self::Internal(relation) => f.write_str(relation),
            Self::Property {
                relation,
                object,
                uuid,
            } => write!(f, "{relation} {object} ({uuid})"),
        }
    }
}

struct BatchOpSourceDisplay<'a> {
    source: &'a BatchOpSource,
    property_names: &'a PropertyNames,
}

impl std::fmt::Display for BatchOpSourceDisplay<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let BatchOpSource::Property {
            relation,
            object,
            uuid,
        } = self.source
        else {
            return std::fmt::Display::fmt(self.source, f);
        };

        let Some(name) = self.property_names.by_uuid.get(uuid) else {
            return std::fmt::Display::fmt(self.source, f);
        };
        write!(f, "{relation} {object}.{} ({uuid})", name.as_str())
    }
}

struct PropertyNames {
    by_uuid: AHashMap<Uuid, Symbol>,
}

impl PropertyNames {
    fn new(by_uuid: AHashMap<Uuid, Symbol>) -> Self {
        Self { by_uuid }
    }

    fn apply(&mut self, changes: impl IntoIterator<Item = PropertyDefinitionChange>) {
        for change in changes {
            match change {
                PropertyDefinitionChange::Remove(uuid) => {
                    self.by_uuid.remove(&uuid);
                }
                PropertyDefinitionChange::Upsert(definition) => {
                    self.by_uuid.insert(definition.uuid(), definition.name());
                }
            }
        }
    }

    fn display<'a>(&'a self, source: &'a BatchOpSource) -> BatchOpSourceDisplay<'a> {
        BatchOpSourceDisplay {
            source,
            property_names: self,
        }
    }
}

pub enum BatchOpType {
    Insert {
        key: fjall::Slice,
        value: Box<dyn BatchValue>,
    },
    Delete {
        key: fjall::Slice,
    },
    PropertyValue(PreparedPropertyValueOp),
}

/// Reusable serialization state owned by an encoding worker.
pub struct BatchEncoder {
    var_builder: planus::Builder,
}

impl BatchEncoder {
    pub(crate) fn new() -> Self {
        Self {
            var_builder: planus::Builder::new(),
        }
    }

    pub(crate) fn var_builder(&mut self) -> &mut planus::Builder {
        &mut self.var_builder
    }
}

/// Value hook used by the writer thread to produce serialized bytes.
pub trait BatchValue: Send {
    fn encode(
        self: Box<Self>,
        timestamp: Timestamp,
        encoder: &mut BatchEncoder,
    ) -> Result<fjall::Slice, Error>;
}

#[cfg(test)]
struct EncodedBatchValue(fjall::Slice);

#[cfg(test)]
impl BatchValue for EncodedBatchValue {
    fn encode(
        self: Box<Self>,
        _timestamp: Timestamp,
        _encoder: &mut BatchEncoder,
    ) -> Result<fjall::Slice, Error> {
        Ok(self.0)
    }
}

/// A batch of operations from a single commit, spanning all relations.
pub struct CommitBatch {
    /// Contiguous version of the published world-state snapshot.
    pub version: u64,
    pub timestamp: Timestamp,
    pub operations: Vec<BatchOp>,
    property_definition_changes: Box<[PropertyDefinitionChange]>,
}

struct EncodedBatchOp {
    partition: fjall::Keyspace,
    op_type: EncodedBatchOpType,
}

enum EncodedBatchOpType {
    Insert {
        key: fjall::Slice,
        value: fjall::Slice,
    },
    Delete {
        key: fjall::Slice,
    },
    PropertyValue(EncodedPropertyValueOp),
}

struct EncodedPropertyValueOp {
    property: ObjAndUUIDHolder,
    mutation: EncodedPropertyValueMutation,
}

enum EncodedPropertyValueMutation {
    Replace {
        record: fjall::Slice,
    },
    AppendList {
        record: fjall::Slice,
        payload_bytes: usize,
        final_value: Var,
    },
    Delete,
}

enum PropertyValueChainChange {
    Reset {
        property: ObjAndUUIDHolder,
        full_version: u64,
    },
    Append {
        property: ObjAndUUIDHolder,
        record_version: u64,
        payload_bytes: usize,
    },
    Delete(ObjAndUUIDHolder),
}

struct EncodingStats {
    elapsed: Duration,
    encoded_bytes: usize,
    slowest: Option<(BatchOpSource, Duration, usize)>,
}

struct EncodedCommitBatch {
    version: u64,
    timestamp: Timestamp,
    operations: Vec<EncodedBatchOp>,
    property_definition_changes: Box<[PropertyDefinitionChange]>,
    sequences: Vec<SequenceUpdate>,
    admission: Option<CommitAdmission>,
    encoding: EncodingStats,
}

struct EncodedBatchResult {
    version: u64,
    result: Result<EncodedCommitBatch, String>,
}

impl CommitBatch {
    #[allow(dead_code)]
    pub fn with_capacity(version: u64, timestamp: Timestamp, expected_operations: usize) -> Self {
        Self {
            version,
            timestamp,
            operations: Vec::with_capacity(expected_operations),
            property_definition_changes: Box::default(),
        }
    }

    #[cfg(test)]
    pub fn insert(
        &mut self,
        partition: fjall::Keyspace,
        key: impl Into<fjall::Slice>,
        value: Box<dyn BatchValue>,
    ) {
        self.operations.push(BatchOp {
            partition,
            op_type: BatchOpType::Insert {
                key: key.into(),
                value,
            },
            source: BatchOpSource::Internal("direct batch value"),
        });
    }

    #[cfg(test)]
    pub fn insert_encoded(
        &mut self,
        partition: fjall::Keyspace,
        key: impl Into<fjall::Slice>,
        value: impl Into<fjall::Slice>,
    ) {
        self.insert(partition, key, Box::new(EncodedBatchValue(value.into())));
    }

    pub fn from_ops(version: u64, timestamp: Timestamp, operations: Vec<BatchOp>) -> Self {
        Self {
            version,
            timestamp,
            operations,
            property_definition_changes: Box::default(),
        }
    }

    pub(crate) fn set_property_definition_changes(
        &mut self,
        changes: Vec<PropertyDefinitionChange>,
    ) {
        self.property_definition_changes = changes.into_boxed_slice();
    }
}

/// Message sent to the writer thread.
enum WriterMsg {
    /// Persist one complete published transaction.
    Commit(EncodedBatchResult),
    /// Confirm that Fjall has committed through this publication version.
    Barrier {
        through_version: u64,
        reply: oneshot::Sender<Result<(), String>>,
    },
    /// Confirm that an applied prefix has crossed the durable-storage fence.
    Durable {
        through_version: u64,
        reply: oneshot::Sender<Result<(), String>>,
    },
    /// Return a cross-keyspace snapshot after committing through this version.
    Snapshot {
        through_version: u64,
        reply: oneshot::Sender<Result<fjall::Snapshot, String>>,
    },
}

/// Work item for an encoding worker.
pub(crate) enum EncodeRequest {
    /// Production path: backend-neutral logical commit, encoded on the worker.
    Logical(Box<LogicalCommit>),
    /// Test path: a pre-built Fjall batch with optional sequence observations.
    #[cfg(test)]
    PreBuilt(CommitBatch, Vec<SequenceUpdate>),
}

enum EncoderMsg {
    Commit {
        request: EncodeRequest,
        admission: Option<CommitAdmission>,
    },
    Stop,
}

enum RollupMsg {
    Encode {
        value: Var,
        timestamp: Timestamp,
        reply: oneshot::Sender<Result<RollupResult, String>>,
    },
    Stop,
}

struct RollupResult {
    record: fjall::Slice,
    elapsed: Duration,
}

#[derive(Clone)]
struct RollupEncoder {
    sender: Sender<RollupMsg>,
}

impl RollupEncoder {
    fn encode(&self, value: Var, timestamp: Timestamp) -> Result<RollupResult, String> {
        let (reply, receiver) = oneshot::channel();
        self.sender
            .send(RollupMsg::Encode {
                value,
                timestamp,
                reply,
            })
            .map_err(|_| "property-value rollup encoder disconnected".to_string())?;
        receiver
            .recv()
            .map_err(|error| format!("property-value rollup encoder failed to reply: {error}"))?
    }
}

const RECEIVE_POLL_INTERVAL: Duration = Duration::from_millis(10);
const WRITER_QUEUE_CAPACITY: usize = 64;
const MAX_ENCODER_THREADS: usize = 8;

struct WriterState {
    waiting_batches: BTreeMap<u64, Result<EncodedCommitBatch, String>>,
    barrier_waiters: Vec<(u64, oneshot::Sender<Result<(), String>>)>,
    durable_waiters: Vec<(u64, oneshot::Sender<Result<(), String>>)>,
    snapshot_waiters: Vec<(u64, oneshot::Sender<Result<fjall::Snapshot, String>>)>,
    next_version: u64,
    property_names: PropertyNames,
    property_value_chains: AHashMap<ObjAndUUIDHolder, PropertyValueChain>,
    property_value_limits: PropertyValueChainLimits,
    next_property_value_record_version: u64,
    sequences_partition: fjall::Keyspace,
    confirmed_sequences: Vec<i64>,
    durable_version: u64,
}

struct WriterInit {
    property_names: AHashMap<Uuid, Symbol>,
    property_value_chains: AHashMap<ObjAndUUIDHolder, PropertyValueChain>,
    property_value_limits: PropertyValueChainLimits,
    sequences_partition: fjall::Keyspace,
    initial_sequences: Vec<i64>,
    rollup_encoder: RollupEncoder,
}

impl WriterState {
    fn new(
        property_names: AHashMap<Uuid, Symbol>,
        property_value_chains: AHashMap<ObjAndUUIDHolder, PropertyValueChain>,
        property_value_limits: PropertyValueChainLimits,
        sequences_partition: fjall::Keyspace,
        initial_sequences: Vec<i64>,
    ) -> Self {
        let next_property_value_record_version = property_value_chains
            .values()
            .flat_map(PropertyValueChain::record_versions)
            .max()
            .map_or(1, |version| version.saturating_add(1));
        Self {
            waiting_batches: BTreeMap::new(),
            barrier_waiters: Vec::new(),
            durable_waiters: Vec::new(),
            snapshot_waiters: Vec::new(),
            next_version: 1,
            property_names: PropertyNames::new(property_names),
            property_value_chains,
            property_value_limits,
            next_property_value_record_version,
            sequences_partition,
            confirmed_sequences: initial_sequences,
            durable_version: 0,
        }
    }

    fn add_batch(&mut self, batch: EncodedBatchResult) -> Result<(), String> {
        let version = batch.version;
        if version < self.next_version
            || self.waiting_batches.insert(version, batch.result).is_some()
        {
            return Err(format!("duplicate persistence batch for version {version}"));
        }
        Ok(())
    }
}

/// Close admission before unwinding or returning from a failed worker.
struct WorkerHealthGuard {
    healthy: Arc<AtomicBool>,
    finished: bool,
}

impl Drop for WorkerHealthGuard {
    fn drop(&mut self) {
        if !self.finished {
            self.healthy.store(false, Ordering::Release);
        }
    }
}

/// Wait failures surfaced to the coordinator.
#[derive(Debug, thiserror::Error)]
pub(crate) enum WriterWaitError {
    #[error("timed out waiting for persistence through version {version}")]
    Timeout { version: u64 },
    #[error("persistence writer failed: {detail}")]
    Failed { detail: String },
    #[error("persistence writer is unavailable")]
    Unavailable,
}

/// Background writer that commits published transactions to Fjall in version order.
pub struct BatchWriter {
    sender: Sender<WriterMsg>,
    encoder_sender: Sender<EncoderMsg>,
    completed_version: Arc<AtomicU64>,
    durable_version: Arc<AtomicU64>,
    healthy: Arc<AtomicBool>,
    waiters_cancelled: AtomicBool,
    workers: Workers,
}

// If batch writes take longer than this, give a friendly warning to alert the user that something
// might be up in I/O land.
const WRITE_WARNING_DURATION: Duration = Duration::from_secs(5);
const ENCODE_WARNING_DURATION: Duration = Duration::from_secs(1);

impl BatchWriter {
    #[cfg(test)]
    pub fn new(db: fjall::Database) -> Self {
        Self::with_property_names(db, AHashMap::new())
    }

    #[cfg(test)]
    pub(crate) fn with_property_names(
        db: fjall::Database,
        property_names: AHashMap<Uuid, Symbol>,
    ) -> Self {
        let sequences_partition = db
            .keyspace("sequences", fjall::KeyspaceCreateOptions::default)
            .expect("test sequences keyspace");
        Self::with_property_value_state(
            db,
            sequences_partition,
            Vec::new(),
            None,
            property_names,
            AHashMap::new(),
            PROPERTY_VALUE_CHAIN_LIMITS,
        )
    }

    pub(crate) fn with_property_value_state(
        db: fjall::Database,
        sequences_partition: fjall::Keyspace,
        initial_sequences: Vec<i64>,
        relations: Option<Arc<FjallRelations>>,
        property_names: AHashMap<Uuid, Symbol>,
        property_value_chains: AHashMap<ObjAndUUIDHolder, PropertyValueChain>,
        property_value_limits: PropertyValueChainLimits,
    ) -> Self {
        let kill_switch = Arc::new(AtomicBool::new(false));
        let completed_version = Arc::new(AtomicU64::new(0));
        let durable_version = Arc::new(AtomicU64::new(0));
        let healthy = Arc::new(AtomicBool::new(true));
        let (sender, receiver) = flume::bounded::<WriterMsg>(WRITER_QUEUE_CAPACITY);
        let (encoder_sender, encoder_receiver) =
            flume::bounded::<EncoderMsg>(COMMIT_ADMISSION_CAPACITY);
        let (rollup_sender, rollup_receiver) = flume::bounded::<RollupMsg>(1);
        let mut workers = WorkerGroup::new(
            sender.clone(),
            encoder_sender.clone(),
            kill_switch.clone(),
            rollup_sender.clone(),
        );
        let rollup_handle = moor_common::threading::spawn_perf("moor-db-rollup-enc", move || {
            Self::rollup_encoder_loop(rollup_receiver)
        })
        .expect("failed to spawn property-value rollup encoder thread");
        workers.set_rollup(rollup_handle);
        let rollup_encoder = RollupEncoder {
            sender: rollup_sender.clone(),
        };

        let ks = kill_switch.clone();
        let completed = completed_version.clone();
        let durable = durable_version.clone();
        let writer_healthy = healthy.clone();

        let join_handle = spawn_efficient("moor-batch-writer", move || {
            Self::writer_loop(
                db,
                receiver,
                ks,
                completed,
                durable,
                writer_healthy,
                WriterInit {
                    property_names,
                    property_value_chains,
                    property_value_limits,
                    sequences_partition,
                    initial_sequences,
                    rollup_encoder,
                },
            )
        })
        .expect("failed to spawn batch writer thread");
        workers.set_writer(join_handle);

        let encoder_count = Self::encoder_thread_count();
        for index in 0..encoder_count {
            let receiver = encoder_receiver.clone();
            let sender = sender.clone();
            let relations = relations.clone();
            let healthy = healthy.clone();
            let handle =
                moor_common::threading::spawn_perf(format!("moor-db-enc-{index}"), move || {
                    let mut guard = WorkerHealthGuard {
                        healthy,
                        finished: false,
                    };
                    let result = Self::encoder_loop(receiver, sender, relations);
                    guard.finished = result.is_ok();
                    result
                })
                .expect("failed to spawn batch encoder thread");
            workers.add_encoder(handle);
        }

        Self {
            sender,
            encoder_sender,
            completed_version,
            durable_version,
            healthy,
            waiters_cancelled: AtomicBool::new(false),
            workers: Workers::new(workers),
        }
    }

    fn encoder_thread_count() -> usize {
        std::thread::available_parallelism()
            .map(|cores| cores.get().div_ceil(8).clamp(1, MAX_ENCODER_THREADS))
            .unwrap_or(1)
    }

    fn encoder_loop(
        receiver: Receiver<EncoderMsg>,
        sender: Sender<WriterMsg>,
        relations: Option<Arc<FjallRelations>>,
    ) -> Result<(), String> {
        let mut encoder = BatchEncoder::new();
        loop {
            let msg = receiver
                .recv()
                .map_err(|_| "batch encoder channel disconnected".to_string())?;
            let EncoderMsg::Commit { request, admission } = msg else {
                return Ok(());
            };

            let mut encoded = match request {
                EncodeRequest::Logical(commit) => {
                    let version = commit.publication.version();
                    let result = match relations.as_ref() {
                        Some(relations) => Self::encode_logical(*commit, &mut encoder, relations),
                        None => {
                            Err("logical commit submitted without relation resources".to_string())
                        }
                    };
                    EncodedBatchResult { version, result }
                }
                #[cfg(test)]
                EncodeRequest::PreBuilt(batch, sequences) => {
                    let version = batch.version;
                    let result = Self::encode_batch(batch, sequences, &mut encoder);
                    EncodedBatchResult { version, result }
                }
            };
            if let Ok(batch) = &mut encoded.result {
                // Hold the admission permit until the batch has been applied.
                batch.admission = admission;
            } else if let Some(admission) = &admission {
                admission.fail();
            }

            sender.send(WriterMsg::Commit(encoded)).map_err(|error| {
                if let WriterMsg::Commit(EncodedBatchResult {
                    result: Ok(batch), ..
                }) = &error.0
                    && let Some(admission) = &batch.admission
                {
                    admission.fail();
                }
                "batch writer channel disconnected".to_string()
            })?;
        }
    }

    fn rollup_encoder_loop(receiver: Receiver<RollupMsg>) -> Result<(), String> {
        let mut encoder = BatchEncoder::new();
        loop {
            let msg = receiver
                .recv()
                .map_err(|_| "property-value rollup encoder channel disconnected".to_string())?;
            let RollupMsg::Encode {
                value,
                timestamp,
                reply,
            } = msg
            else {
                return Ok(());
            };

            let start = Instant::now();
            let result = encode_full_record(encoder.var_builder(), &value, timestamp)
                .map(|record| RollupResult {
                    record: record.into(),
                    elapsed: start.elapsed(),
                })
                .map_err(|error| format!("failed to encode property-value rollup: {error}"));
            reply.send(result).ok();
        }
    }

    fn writer_loop(
        db: fjall::Database,
        receiver: Receiver<WriterMsg>,
        kill_switch: Arc<AtomicBool>,
        completed_version: Arc<AtomicU64>,
        durable_version: Arc<AtomicU64>,
        healthy: Arc<AtomicBool>,
        init: WriterInit,
    ) -> Result<(), String> {
        let result = Self::run_writer(
            db,
            receiver,
            kill_switch,
            completed_version,
            durable_version,
            healthy.clone(),
            init,
        );
        if let Err(error) = &result {
            healthy.store(false, Ordering::Release);
            error!("Batch writer failed: {error}");
            #[cfg(not(test))]
            moor_common::util::signal_fatal_db_error("batch writer", error);
        }
        result
    }

    fn run_writer(
        db: fjall::Database,
        receiver: Receiver<WriterMsg>,
        kill_switch: Arc<AtomicBool>,
        completed_version: Arc<AtomicU64>,
        durable_version: Arc<AtomicU64>,
        healthy: Arc<AtomicBool>,
        init: WriterInit,
    ) -> Result<(), String> {
        let WriterInit {
            property_names,
            property_value_chains,
            property_value_limits,
            sequences_partition,
            initial_sequences,
            rollup_encoder,
        } = init;
        let mut state = WriterState::new(
            property_names,
            property_value_chains,
            property_value_limits,
            sequences_partition,
            initial_sequences,
        );

        let mut guard = WorkerHealthGuard {
            healthy,
            finished: false,
        };
        loop {
            if !guard.healthy.load(Ordering::Acquire) {
                let detail = "persistence worker failed";
                Self::fail_waiters(&mut state, detail);
                return Err(detail.to_string());
            }
            state
                .barrier_waiters
                .retain(|(_, reply)| !reply.is_closed());
            state
                .durable_waiters
                .retain(|(_, reply)| !reply.is_closed());
            state
                .snapshot_waiters
                .retain(|(_, reply)| !reply.is_closed());
            if kill_switch.load(Ordering::Relaxed) {
                while let Ok(msg) = receiver.try_recv() {
                    Self::handle_message(
                        &db,
                        msg,
                        &mut state,
                        completed_version.load(Ordering::Acquire),
                    )?;
                    Self::persist_ready(
                        &db,
                        &mut state,
                        &completed_version,
                        &durable_version,
                        &rollup_encoder,
                    )?;
                }

                if !state.waiting_batches.is_empty() {
                    let error = format!(
                        "batch writer stopped with a gap before version {}",
                        state.next_version
                    );
                    Self::fail_waiters(&mut state, &error);
                    return Err(error);
                }

                Self::establish_durable(
                    &db,
                    &mut state,
                    completed_version.load(Ordering::Acquire),
                    &durable_version,
                )?;
                Self::reply_ready_barriers(&mut state, completed_version.load(Ordering::Acquire));
                Self::reply_ready_snapshots(
                    &db,
                    &mut state,
                    completed_version.load(Ordering::Acquire),
                );
                guard.finished = true;
                return Ok(());
            }

            match receiver.recv_timeout(RECEIVE_POLL_INTERVAL) {
                Ok(msg) => {
                    Self::handle_message(
                        &db,
                        msg,
                        &mut state,
                        completed_version.load(Ordering::Acquire),
                    )?;
                    // Release queued encoders together before applying the ready prefix.
                    // Each logical commit still becomes its own atomic Fjall batch.
                    for _ in 1..WRITER_QUEUE_CAPACITY {
                        let Ok(msg) = receiver.try_recv() else { break };
                        Self::handle_message(
                            &db,
                            msg,
                            &mut state,
                            completed_version.load(Ordering::Acquire),
                        )?;
                    }
                    Self::persist_ready(
                        &db,
                        &mut state,
                        &completed_version,
                        &durable_version,
                        &rollup_encoder,
                    )?;
                }
                Err(flume::RecvTimeoutError::Timeout) => {}
                Err(flume::RecvTimeoutError::Disconnected) => {
                    let error = "batch writer channel disconnected".to_string();
                    Self::fail_waiters(&mut state, &error);
                    return Err(error);
                }
            }
        }
    }

    fn handle_message(
        db: &fjall::Database,
        msg: WriterMsg,
        state: &mut WriterState,
        completed_version: u64,
    ) -> Result<(), String> {
        match msg {
            WriterMsg::Commit(batch) => state.add_batch(batch),
            WriterMsg::Barrier {
                through_version,
                reply,
            } => {
                if through_version <= completed_version {
                    reply.send(Ok(())).ok();
                } else {
                    state.barrier_waiters.push((through_version, reply));
                }
                Ok(())
            }
            WriterMsg::Durable {
                through_version,
                reply,
            } => {
                if through_version != 0 && through_version <= state.durable_version {
                    reply.send(Ok(())).ok();
                } else {
                    state.durable_waiters.push((through_version, reply));
                }
                Ok(())
            }
            WriterMsg::Snapshot {
                through_version,
                reply,
            } => {
                if through_version <= completed_version {
                    reply.send(Ok(db.snapshot())).ok();
                } else {
                    state.snapshot_waiters.push((through_version, reply));
                }
                Ok(())
            }
        }
    }

    fn persist_ready(
        db: &fjall::Database,
        state: &mut WriterState,
        completed_version: &AtomicU64,
        durable_version: &AtomicU64,
        rollup_encoder: &RollupEncoder,
    ) -> Result<(), String> {
        while let Some(batch) = state.waiting_batches.remove(&state.next_version) {
            let mut batch = match batch {
                Ok(batch) => batch,
                Err(error) => {
                    Self::fail_waiters(state, &error);
                    return Err(error);
                }
            };
            let admission = batch.admission.take();
            let result = PreparedPersistence::prepare(db, batch, state, rollup_encoder)
                .and_then(PreparedPersistence::commit);
            match result {
                Ok(persisted) => persisted.complete(completed_version, admission),
                Err(error) => {
                    if let Some(admission) = &admission {
                        admission.fail();
                    }
                    Self::fail_waiters(state, &error);
                    return Err(error);
                }
            }
        }
        // A durable request can arrive after its prefix has already applied.
        let completed = completed_version.load(Ordering::Acquire);
        Self::establish_durable(db, state, completed, durable_version)?;
        Ok(())
    }

    /// Establish the durable fence for every queued request the applied prefix now covers.
    fn establish_durable(
        db: &fjall::Database,
        state: &mut WriterState,
        completed_version: u64,
        durable_version: &AtomicU64,
    ) -> Result<(), String> {
        let Some(requested) = state
            .durable_waiters
            .iter()
            .map(|(version, _)| *version)
            .filter(|version| *version <= completed_version)
            .max()
        else {
            return Ok(());
        };
        if requested > completed_version {
            return Ok(());
        }
        if state.durable_version < completed_version || requested == 0 {
            let start = Instant::now();
            db.persist(PersistMode::SyncAll)
                .map_err(|error| format!("failed to flush Fjall to durable storage: {error}"))?;
            db_counters()
                .timers_rare
                .record_elapsed(WorldStateTimerOp::BatchWriterDurableFlush, start.elapsed());
            state.durable_version = completed_version;
            durable_version.store(completed_version, Ordering::Release);
        }

        let durable = state.durable_version;
        let mut pending = Vec::with_capacity(state.durable_waiters.len());
        for (version, reply) in state.durable_waiters.drain(..) {
            if version <= durable {
                reply.send(Ok(())).ok();
            } else {
                pending.push((version, reply));
            }
        }
        state.durable_waiters = pending;
        Ok(())
    }

    /// Encode one assembled Fjall batch, including its sequence observations.
    fn encode_batch(
        batch: CommitBatch,
        sequences: Vec<SequenceUpdate>,
        encoder: &mut BatchEncoder,
    ) -> Result<EncodedCommitBatch, String> {
        let CommitBatch {
            version,
            timestamp,
            operations,
            property_definition_changes,
        } = batch;
        (move || {
            let start = Instant::now();
            let mut encoded_bytes = 0usize;
            let mut slowest = None;
            let mut encoded_operations = Vec::with_capacity(operations.len());

            for op in operations {
                let BatchOp {
                    partition,
                    op_type,
                    source,
                } = op;
                let op_type = match op_type {
                    BatchOpType::Insert { key, value } => {
                        let op_start = Instant::now();
                        let value = value
                            .encode(timestamp, encoder)
                            .map_err(|error| format!("failed to encode batch value: {error}"))?;
                        let elapsed = op_start.elapsed();
                        let bytes = key.len() + value.len();
                        encoded_bytes += bytes;
                        if slowest
                            .as_ref()
                            .is_none_or(|(_, slowest_elapsed, _)| elapsed > *slowest_elapsed)
                        {
                            slowest = Some((source, elapsed, bytes));
                        }
                        EncodedBatchOpType::Insert { key, value }
                    }
                    BatchOpType::Delete { key } => {
                        encoded_bytes += key.len();
                        EncodedBatchOpType::Delete { key }
                    }
                    BatchOpType::PropertyValue(prepared) => {
                        let op_start = Instant::now();
                        let PreparedPropertyValueOp { property, mutation } = prepared;
                        let mutation = match mutation {
                            PreparedPropertyValueMutation::Replace { value } => {
                                let record =
                                    encode_full_record(encoder.var_builder(), &value, timestamp)
                                        .map_err(|error| {
                                            format!(
                                                "failed to encode complete property value: {error}"
                                            )
                                        })?;
                                db_counters().counters.add(
                                    WorldStateCountOp::PropertyValueFullEncodedBytes,
                                    isize::try_from(record.len()).unwrap_or(isize::MAX),
                                );
                                EncodedPropertyValueMutation::Replace {
                                    record: record.into(),
                                }
                            }
                            PreparedPropertyValueMutation::AppendList {
                                suffix,
                                final_value,
                            } => {
                                let record = encode_list_append_record(
                                    encoder.var_builder(),
                                    &suffix,
                                    timestamp,
                                )
                                .map_err(|error| {
                                    format!("failed to encode property list append: {error}")
                                })?;
                                let payload_bytes = property_value_record_payload_bytes(&record)
                                    .map_err(|error| {
                                        format!(
                                            "failed to inspect encoded property list append: {error}"
                                        )
                                    })?;
                                db_counters().counters.add(
                                    WorldStateCountOp::PropertyValueAppendEncodedBytes,
                                    isize::try_from(record.len()).unwrap_or(isize::MAX),
                                );
                                EncodedPropertyValueMutation::AppendList {
                                    record: record.into(),
                                    payload_bytes,
                                    final_value,
                                }
                            }
                            PreparedPropertyValueMutation::Delete => {
                                EncodedPropertyValueMutation::Delete
                            }
                        };
                        let elapsed = op_start.elapsed();
                        let bytes = match &mutation {
                            EncodedPropertyValueMutation::Replace { record }
                            | EncodedPropertyValueMutation::AppendList { record, .. } => {
                                PROPERTY_RECORD_KEY_BYTES + record.len()
                            }
                            EncodedPropertyValueMutation::Delete => PROPERTY_RECORD_KEY_BYTES,
                        };
                        encoded_bytes += bytes;
                        if slowest
                            .as_ref()
                            .is_none_or(|(_, slowest_elapsed, _)| elapsed > *slowest_elapsed)
                        {
                            slowest = Some((source, elapsed, bytes));
                        }
                        EncodedBatchOpType::PropertyValue(EncodedPropertyValueOp {
                            property,
                            mutation,
                        })
                    }
                };
                encoded_operations.push(EncodedBatchOp { partition, op_type });
            }

            let elapsed = start.elapsed();
            db_counters()
                .timers_rare
                .record_elapsed(WorldStateTimerOp::BatchWriterEncode, elapsed);

            Ok(EncodedCommitBatch {
                version,
                timestamp,
                operations: encoded_operations,
                property_definition_changes,
                sequences,
                admission: None,
                encoding: EncodingStats {
                    elapsed,
                    encoded_bytes,
                    slowest,
                },
            })
        })()
    }

    /// Encode a backend-neutral logical commit using the Fjall relation resources.
    fn encode_logical(
        commit: LogicalCommit,
        encoder: &mut BatchEncoder,
        relations: &FjallRelations,
    ) -> Result<EncodedCommitBatch, String> {
        let LogicalCommit {
            publication,
            timestamp,
            changes,
            sequences,
            property_definition_changes,
        } = commit;
        let mut batch = relations
            .working_sets_to_batch(changes, publication.version(), timestamp)
            .map_err(|error| format!("failed to encode relation changes: {error}"))?;
        batch.set_property_definition_changes(property_definition_changes);
        Self::encode_batch(batch, sequences, encoder)
    }

    fn reply_ready_barriers(state: &mut WriterState, completed_version: u64) {
        let mut pending = Vec::with_capacity(state.barrier_waiters.len());
        for (version, reply) in state.barrier_waiters.drain(..) {
            if version <= completed_version {
                reply.send(Ok(())).ok();
            } else {
                pending.push((version, reply));
            }
        }
        state.barrier_waiters = pending;
    }

    fn reply_ready_snapshots(
        db: &fjall::Database,
        state: &mut WriterState,
        completed_version: u64,
    ) {
        let mut pending = Vec::with_capacity(state.snapshot_waiters.len());
        for (version, reply) in state.snapshot_waiters.drain(..) {
            if version <= completed_version {
                reply.send(Ok(db.snapshot())).ok();
            } else {
                pending.push((version, reply));
            }
        }
        state.snapshot_waiters = pending;
    }

    fn fail_waiters(state: &mut WriterState, detail: &str) {
        for (_, reply) in state.barrier_waiters.drain(..) {
            reply.send(Err(detail.to_string())).ok();
        }
        for (_, reply) in state.durable_waiters.drain(..) {
            reply.send(Err(detail.to_string())).ok();
        }
        for (_, reply) in state.snapshot_waiters.drain(..) {
            reply.send(Err(detail.to_string())).ok();
        }
    }

    /// Submit a published logical commit to the encoder pool.
    ///
    /// The admission permit travels with the commit and is released only after the batch has
    /// been applied.
    pub(crate) fn submit(
        &self,
        commit: LogicalCommit,
        admission: CommitAdmission,
    ) -> Result<(), String> {
        self.try_send_request(EncodeRequest::Logical(Box::new(commit)), Some(admission))
    }

    #[cfg(test)]
    pub(crate) fn write_batch_for_test(&self, batch: CommitBatch) -> Result<(), String> {
        self.try_send_request(EncodeRequest::PreBuilt(batch, Vec::new()), None)
    }

    pub(crate) fn try_send_request(
        &self,
        request: EncodeRequest,
        admission: Option<CommitAdmission>,
    ) -> Result<(), String> {
        self.encoder_sender
            .try_send(EncoderMsg::Commit { request, admission })
            .map_err(|error| {
                self.healthy.store(false, Ordering::Release);
                match error {
                    flume::TrySendError::Full(_) => {
                        "batch encoder queue full after admission was reserved".to_string()
                    }
                    flume::TrySendError::Disconnected(_) => {
                        "batch encoder channel disconnected".to_string()
                    }
                }
            })
    }

    pub fn completed_version(&self) -> u64 {
        self.completed_version.load(Ordering::Acquire)
    }

    pub fn durable_version(&self) -> u64 {
        self.durable_version.load(Ordering::Acquire)
    }

    /// Whether the writer is still able to apply commits.
    pub fn healthy(&self) -> bool {
        self.healthy.load(Ordering::Acquire)
    }

    pub(crate) fn health_flag(&self) -> Arc<AtomicBool> {
        self.healthy.clone()
    }

    pub(crate) fn cancel_waiters(&self) {
        self.waiters_cancelled.store(true, Ordering::Release);
    }

    fn check_wait_health(&self) -> Result<(), WriterWaitError> {
        if !self.healthy() || self.waiters_cancelled.load(Ordering::Acquire) {
            return Err(WriterWaitError::Unavailable);
        }
        Ok(())
    }

    fn send_request(
        &self,
        mut message: WriterMsg,
        version: u64,
        deadline: Option<Instant>,
    ) -> Result<(), WriterWaitError> {
        loop {
            self.check_wait_health()?;
            let wait = remaining(deadline).min(RECEIVE_POLL_INTERVAL);
            match self.sender.send_timeout(message, wait) {
                Ok(()) => return Ok(()),
                Err(flume::SendTimeoutError::Disconnected(_)) => {
                    return Err(WriterWaitError::Unavailable);
                }
                Err(flume::SendTimeoutError::Timeout(returned)) => message = returned,
            }
            if remaining(deadline).is_zero() {
                return Err(WriterWaitError::Timeout { version });
            }
        }
    }

    fn receive_reply<T>(
        &self,
        receiver: oneshot::Receiver<Result<T, String>>,
        version: u64,
        deadline: Option<Instant>,
    ) -> Result<T, WriterWaitError> {
        loop {
            match receiver.recv_timeout(remaining(deadline).min(RECEIVE_POLL_INTERVAL)) {
                Ok(result) => return result.map_err(|detail| WriterWaitError::Failed { detail }),
                Err(oneshot::RecvTimeoutError::Disconnected) => {
                    return Err(WriterWaitError::Unavailable);
                }
                Err(oneshot::RecvTimeoutError::Timeout) => {}
            }
            self.check_wait_health()?;
            if remaining(deadline).is_zero() {
                return Err(WriterWaitError::Timeout { version });
            }
        }
    }

    /// Wait for application, without requesting a durable flush.
    pub fn wait_applied(&self, version: u64, timeout: Duration) -> Result<(), WriterWaitError> {
        self.wait_applied_until(version, Instant::now().checked_add(timeout))
    }

    /// Unbounded application barrier; failure and shutdown still wake the caller.
    pub fn wait_applied_unbounded(&self, version: u64) -> Result<(), WriterWaitError> {
        self.wait_applied_until(version, None)
    }

    fn wait_applied_until(
        &self,
        version: u64,
        deadline: Option<Instant>,
    ) -> Result<(), WriterWaitError> {
        if version <= self.completed_version() {
            return Ok(());
        }
        let (reply, receiver) = oneshot::channel();
        self.send_request(
            WriterMsg::Barrier {
                through_version: version,
                reply,
            },
            version,
            deadline,
        )?;
        self.receive_reply(receiver, version, deadline)
    }

    /// Establish a local durable fence, including recovered state at runtime version zero.
    pub fn wait_durable(&self, version: u64, timeout: Duration) -> Result<(), WriterWaitError> {
        if version != 0 && version <= self.durable_version() {
            return Ok(());
        }
        let deadline = Instant::now().checked_add(timeout);
        let (reply, receiver) = oneshot::channel();
        self.send_request(
            WriterMsg::Durable {
                through_version: version,
                reply,
            },
            version,
            deadline,
        )?;
        self.receive_reply(receiver, version, deadline)
    }

    /// Acquire a consistent snapshot with one deadline for submission and receipt.
    pub fn snapshot(
        &self,
        version: u64,
        timeout: Duration,
    ) -> Result<fjall::Snapshot, WriterWaitError> {
        let deadline = Instant::now().checked_add(timeout);
        let (reply, receiver) = oneshot::channel();
        self.send_request(
            WriterMsg::Snapshot {
                through_version: version,
                reply,
            },
            version,
            deadline,
        )?;
        self.receive_reply(receiver, version, deadline)
    }

    #[cfg(test)]
    pub fn stop(&self) -> Result<(), String> {
        self.stop_with_deadline(Duration::from_secs(30))
    }

    /// Stop workers, draining published work within the given deadline.
    pub fn stop_with_deadline(&self, timeout: Duration) -> Result<(), String> {
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or_else(|| "shutdown deadline exceeds the monotonic clock range".to_string())?;
        let result = self.workers.stop_until(deadline);
        self.cancel_waiters();
        if result.is_err() {
            self.healthy.store(false, Ordering::Release);
        }
        result
    }
}

fn remaining(deadline: Option<Instant>) -> Duration {
    deadline.map_or(Duration::MAX, |deadline| {
        deadline.saturating_duration_since(Instant::now())
    })
}

impl Drop for BatchWriter {
    fn drop(&mut self) {
        if let Err(error) = self.stop_with_deadline(Duration::from_secs(30)) {
            error!("Failed to stop batch writer: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::property_value_store::{
        PropertyValueReconstructor, encode_property_value_record_key,
    };
    use fjall::{KeyspaceCreateOptions, Readable};
    use moor_common::model::PropDef;
    use moor_var::{List, v_int, v_list};

    fn test_database() -> (tempfile::TempDir, fjall::Database) {
        let tempdir = tempfile::tempdir().unwrap();
        let database = fjall::Database::builder(tempdir.path()).open().unwrap();
        (tempdir, database)
    }

    fn encoded_batch(
        version: u64,
        partition: &fjall::Keyspace,
        key: &[u8],
        value: &[u8],
    ) -> CommitBatch {
        let mut batch = CommitBatch::with_capacity(version, Timestamp(version), 1);
        batch.insert_encoded(partition.clone(), key.to_vec(), value.to_vec());
        batch
    }

    fn write(writer: &BatchWriter, batch: CommitBatch) -> Result<(), String> {
        writer.write_batch_for_test(batch)
    }

    fn property_batch(
        version: u64,
        partition: &fjall::Keyspace,
        property: &ObjAndUUIDHolder,
        mutation: PreparedPropertyValueMutation,
    ) -> CommitBatch {
        CommitBatch::from_ops(
            version,
            Timestamp(version),
            vec![BatchOp {
                partition: partition.clone(),
                op_type: BatchOpType::PropertyValue(PreparedPropertyValueOp {
                    property: property.clone(),
                    mutation,
                }),
                source: BatchOpSource::Property {
                    relation: "object_propvalues",
                    object: property.obj(),
                    uuid: property.uuid(),
                },
            }],
        )
    }

    #[test]
    fn property_source_uses_writer_local_name() {
        let object = Obj::mk_id(42);
        let uuid = Uuid::new_v4();
        let source = BatchOpSource::Property {
            relation: "object_propvalues",
            object,
            uuid,
        };
        let name = Symbol::mk("context");
        let definition = PropDef::new(uuid, object, object, name);
        let mut property_names = PropertyNames::new(AHashMap::new());
        property_names.apply(vec![PropertyDefinitionChange::Upsert(definition)]);

        let rendered = property_names.display(&source).to_string();
        assert!(rendered.contains("object_propvalues"));
        assert!(rendered.contains("context"));
        assert!(rendered.contains(&uuid.to_string()));

        let renamed = PropDef::new(uuid, object, object, Symbol::mk("history"));
        property_names.apply(vec![PropertyDefinitionChange::Upsert(renamed)]);
        let rendered = property_names.display(&source).to_string();
        assert!(rendered.contains("history"));
        assert!(!rendered.contains("context"));

        property_names.apply(vec![PropertyDefinitionChange::Remove(uuid)]);
        let rendered = property_names.display(&source).to_string();
        assert!(!rendered.contains("context"));
        assert!(rendered.contains(&uuid.to_string()));
    }

    #[test]
    fn transaction_is_committed_before_snapshot() {
        let (_tempdir, database) = test_database();
        let partition = database
            .keyspace("values", KeyspaceCreateOptions::default)
            .unwrap();
        let writer = BatchWriter::new(database);

        write(&writer, encoded_batch(1, &partition, b"key", b"value")).unwrap();
        let snapshot = writer.snapshot(1, Duration::from_secs(1)).unwrap();

        assert_eq!(writer.completed_version(), 1);
        assert_eq!(
            snapshot.get(&partition, b"key").unwrap().as_deref(),
            Some(&b"value"[..])
        );
    }

    #[test]
    fn persistence_barrier_waits_for_transaction() {
        let (_tempdir, database) = test_database();
        let partition = database
            .keyspace("values", KeyspaceCreateOptions::default)
            .unwrap();
        let writer = BatchWriter::new(database);

        write(&writer, encoded_batch(1, &partition, b"key", b"value")).unwrap();
        writer.wait_applied(1, Duration::from_secs(1)).unwrap();

        assert_eq!(writer.completed_version(), 1);
        assert_eq!(
            partition.get(b"key").unwrap().as_deref(),
            Some(&b"value"[..])
        );
    }

    #[test]
    fn later_durable_wait_does_not_block_an_applied_prefix() {
        let (_tempdir, database) = test_database();
        let partition = database
            .keyspace("values", KeyspaceCreateOptions::default)
            .unwrap();
        let writer = BatchWriter::new(database);
        write(&writer, encoded_batch(1, &partition, b"key", b"value")).unwrap();
        writer.wait_applied(1, Duration::from_secs(1)).unwrap();
        let (reply, _later) = oneshot::channel();
        writer
            .sender
            .send(WriterMsg::Durable {
                through_version: 2,
                reply,
            })
            .unwrap();
        writer.wait_durable(1, Duration::from_secs(1)).unwrap();
        assert_eq!(writer.durable_version(), 1);
    }

    #[test]
    fn snapshot_keeps_its_view_after_writer_shutdown() {
        let (_tempdir, database) = test_database();
        let partition = database
            .keyspace("values", KeyspaceCreateOptions::default)
            .unwrap();
        let writer = BatchWriter::new(database);
        write(&writer, encoded_batch(1, &partition, b"key", b"first")).unwrap();
        let snapshot = writer.snapshot(1, Duration::from_secs(1)).unwrap();
        write(&writer, encoded_batch(2, &partition, b"key", b"second")).unwrap();
        writer.stop_with_deadline(Duration::from_secs(1)).unwrap();
        assert_eq!(
            snapshot.get(&partition, b"key").unwrap().unwrap().as_ref(),
            b"first"
        );
    }

    #[test]
    fn shutdown_deadline_includes_a_full_encoder_queue() {
        let (sender, _receiver) = flume::bounded(1);
        let (encoder_sender, _encoder_receiver) = flume::bounded(1);
        encoder_sender.send(EncoderMsg::Stop).unwrap();
        let (rollup_sender, _rollup_receiver) = flume::bounded(1);
        let (release, wait) = flume::bounded(1);
        let worker = std::thread::spawn(move || {
            wait.recv().unwrap();
            Ok(())
        });
        let mut group = WorkerGroup::new(
            sender.clone(),
            encoder_sender.clone(),
            Arc::new(AtomicBool::new(false)),
            rollup_sender,
        );
        group.add_encoder(worker);
        let writer = BatchWriter {
            sender,
            encoder_sender,
            completed_version: Arc::new(AtomicU64::new(0)),
            durable_version: Arc::new(AtomicU64::new(0)),
            healthy: Arc::new(AtomicBool::new(true)),
            waiters_cancelled: AtomicBool::new(false),
            workers: Workers::new(group),
        };
        let started = Instant::now();
        let result = writer.stop_with_deadline(Duration::from_millis(20));
        release.send(()).unwrap();
        assert!(result.is_err());
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(!writer.healthy());
    }

    #[test]
    fn sequence_updates_never_lower_the_stored_high_water() {
        let (_tempdir, database) = test_database();
        let sequences_partition = database
            .keyspace("sequences", KeyspaceCreateOptions::default)
            .unwrap();
        let writer = BatchWriter::with_property_value_state(
            database.clone(),
            sequences_partition.clone(),
            vec![-1],
            None,
            AHashMap::new(),
            AHashMap::new(),
            PROPERTY_VALUE_CHAIN_LIMITS,
        );

        // Submit the later publication first, then an earlier submission that observed a higher
        // allocation. The writer must apply the maximum rather than the last-applied value.
        let newer = CommitBatch::with_capacity(2, Timestamp(2), 0);
        writer
            .try_send_request(
                EncodeRequest::PreBuilt(newer, vec![SequenceUpdate { slot: 0, value: 10 }]),
                None,
            )
            .unwrap();
        let older = CommitBatch::with_capacity(1, Timestamp(1), 0);
        writer
            .try_send_request(
                EncodeRequest::PreBuilt(older, vec![SequenceUpdate { slot: 0, value: 20 }]),
                None,
            )
            .unwrap();

        writer.wait_applied(2, Duration::from_secs(2)).unwrap();
        let stored = sequences_partition
            .get(0_usize.to_le_bytes())
            .unwrap()
            .expect("sequence slot written");
        assert_eq!(i64::from_le_bytes(stored.as_ref().try_into().unwrap()), 20);
    }

    #[test]
    fn publication_versions_are_persisted_in_order() {
        let (_tempdir, database) = test_database();
        let partition = database
            .keyspace("values", KeyspaceCreateOptions::default)
            .unwrap();
        let writer = BatchWriter::new(database);

        write(&writer, encoded_batch(2, &partition, b"key", b"newer")).unwrap();
        assert_eq!(writer.completed_version(), 0);

        write(&writer, encoded_batch(1, &partition, b"key", b"older")).unwrap();
        let snapshot = writer.snapshot(2, Duration::from_secs(1)).unwrap();

        assert_eq!(
            snapshot.get(&partition, b"key").unwrap().as_deref(),
            Some(&b"newer"[..])
        );
    }

    #[test]
    fn missing_version_fails_shutdown_without_completing_waiters() {
        let (_tempdir, database) = test_database();
        let partition = database
            .keyspace("values", KeyspaceCreateOptions::default)
            .unwrap();
        let writer = BatchWriter::new(database);
        write(&writer, encoded_batch(2, &partition, b"key", b"later")).unwrap();
        let (reply, receiver) = oneshot::channel();
        writer
            .sender
            .send(WriterMsg::Barrier {
                through_version: 2,
                reply,
            })
            .unwrap();
        assert!(writer.stop().unwrap_err().contains("gap before version 1"));
        assert!(
            receiver
                .recv()
                .unwrap()
                .unwrap_err()
                .contains("gap before version 1")
        );
        assert_eq!(writer.completed_version(), 0);
        assert!(partition.get(b"key").unwrap().is_none());
    }

    #[test]
    fn duplicate_batches_are_rejected_before_and_after_persistence() {
        let (_tempdir, database) = test_database();
        let partition = database
            .keyspace("values", KeyspaceCreateOptions::default)
            .unwrap();
        let mut state = WriterState::new(
            AHashMap::new(),
            AHashMap::new(),
            PROPERTY_VALUE_CHAIN_LIMITS,
            database
                .keyspace("sequences", KeyspaceCreateOptions::default)
                .unwrap(),
            Vec::new(),
        );
        let mut encoder = BatchEncoder::new();
        let mut encode = |batch: CommitBatch| EncodedBatchResult {
            version: batch.version,
            result: BatchWriter::encode_batch(batch, Vec::new(), &mut encoder),
        };
        let batch = || encoded_batch(2, &partition, b"key", b"value");
        state.add_batch(encode(batch())).unwrap();
        assert!(
            state
                .add_batch(encode(batch()))
                .unwrap_err()
                .contains("duplicate persistence batch")
        );
        assert_eq!(state.next_version, 1);
        let (sender, _receiver) = flume::bounded(1);
        let completed = AtomicU64::new(0);
        state
            .add_batch(encode(encoded_batch(1, &partition, b"key", b"first")))
            .unwrap();
        BatchWriter::persist_ready(
            &database,
            &mut state,
            &completed,
            &AtomicU64::new(0),
            &RollupEncoder { sender },
        )
        .unwrap();
        let duplicate = encode(encoded_batch(1, &partition, b"key", b"wrong"));
        assert!(
            state
                .add_batch(duplicate)
                .unwrap_err()
                .contains("duplicate persistence batch")
        );
        assert_eq!(completed.load(Ordering::Acquire), 2);
        assert_eq!(partition.get(b"key").unwrap().unwrap().as_ref(), b"value");
    }

    #[test]
    fn drop_drains_pending_writes_and_stop_is_repeatable() {
        let (_tempdir, database) = test_database();
        let partition = database
            .keyspace("values", KeyspaceCreateOptions::default)
            .unwrap();
        {
            let writer = BatchWriter::new(database.clone());
            for version in (1..=32).rev() {
                write(
                    &writer,
                    encoded_batch(version, &partition, b"key", &version.to_be_bytes()),
                )
                .unwrap();
            }
        }
        assert_eq!(
            partition.get(b"key").unwrap().unwrap().as_ref(),
            &32_u64.to_be_bytes()
        );
        let writer = Arc::new(BatchWriter::new(database));
        let other = writer.clone();
        let stop = std::thread::spawn(move || other.stop());
        writer.stop().unwrap();
        stop.join().unwrap().unwrap();
        writer.stop().unwrap();
    }

    #[test]
    fn separate_transactions_use_separate_fjall_batches() {
        let (_tempdir, database) = test_database();
        let partition = database
            .keyspace("values", KeyspaceCreateOptions::default)
            .unwrap();
        let writer = BatchWriter::new(database.clone());
        let initial_seqno = database.visible_seqno();

        write(&writer, encoded_batch(1, &partition, b"key", b"first")).unwrap();
        write(&writer, encoded_batch(2, &partition, b"key", b"second")).unwrap();
        let snapshot = writer.snapshot(2, Duration::from_secs(1)).unwrap();

        assert_eq!(database.visible_seqno(), initial_seqno + 2);
        assert_eq!(
            snapshot.get(&partition, b"key").unwrap().as_deref(),
            Some(&b"second"[..])
        );
    }

    #[test]
    fn one_batch_updates_multiple_relations_before_snapshot() {
        let (_tempdir, database) = test_database();
        let first = database
            .keyspace("first", KeyspaceCreateOptions::default)
            .unwrap();
        let second = database
            .keyspace("second", KeyspaceCreateOptions::default)
            .unwrap();
        let writer = BatchWriter::new(database.clone());
        let mut batch = CommitBatch::with_capacity(1, Timestamp(1), 2);
        batch.insert_encoded(first.clone(), b"key".to_vec(), b"first".to_vec());
        batch.insert_encoded(second.clone(), b"key".to_vec(), b"second".to_vec());

        write(&writer, batch).unwrap();
        let snapshot = writer.snapshot(1, Duration::from_secs(1)).unwrap();

        assert_eq!(
            snapshot.get(&first, b"key").unwrap().as_deref(),
            Some(&b"first"[..])
        );
        assert_eq!(
            snapshot.get(&second, b"key").unwrap().as_deref(),
            Some(&b"second"[..])
        );
    }

    #[test]
    fn property_appends_roll_up_in_publication_order() {
        let (_tempdir, database) = test_database();
        let partition = database
            .keyspace("object_propvalues", KeyspaceCreateOptions::default)
            .unwrap();
        let limits = PropertyValueChainLimits::new(2, usize::MAX);
        let sequences_partition = database
            .keyspace("sequences", KeyspaceCreateOptions::default)
            .unwrap();
        let writer = BatchWriter::with_property_value_state(
            database,
            sequences_partition,
            Vec::new(),
            None,
            AHashMap::new(),
            AHashMap::new(),
            limits,
        );
        let property = ObjAndUUIDHolder::new(&Obj::mk_id(42), Uuid::from_u128(7));

        write(
            &writer,
            property_batch(
                2,
                &partition,
                &property,
                PreparedPropertyValueMutation::AppendList {
                    suffix: List::mk_list(&[v_int(2)]),
                    final_value: v_list(&[v_int(1), v_int(2)]),
                },
            ),
        )
        .unwrap();
        write(
            &writer,
            property_batch(
                1,
                &partition,
                &property,
                PreparedPropertyValueMutation::Replace {
                    value: v_list(&[v_int(1)]),
                },
            ),
        )
        .unwrap();

        let before_rollup = writer.snapshot(2, Duration::from_secs(2)).unwrap();
        let full_record = before_rollup
            .get(&partition, encode_property_value_record_key(&property, 1))
            .unwrap()
            .expect("complete record");
        let append_record = before_rollup
            .get(&partition, encode_property_value_record_key(&property, 2))
            .unwrap()
            .expect("append record");
        let mut reconstructor = PropertyValueReconstructor::new(limits);
        reconstructor.push(1, &full_record).unwrap();
        reconstructor.push(2, &append_record).unwrap();
        let reconstructed = reconstructor.finish().unwrap();
        assert_eq!(reconstructed.value, v_list(&[v_int(1), v_int(2)]));
        assert_eq!(reconstructed.chain.append_count(), 1);

        write(
            &writer,
            property_batch(
                3,
                &partition,
                &property,
                PreparedPropertyValueMutation::AppendList {
                    suffix: List::mk_list(&[v_int(3)]),
                    final_value: v_list(&[v_int(1), v_int(2), v_int(3)]),
                },
            ),
        )
        .unwrap();
        let after_rollup = writer.snapshot(3, Duration::from_secs(2)).unwrap();
        assert!(
            before_rollup
                .get(&partition, encode_property_value_record_key(&property, 1))
                .unwrap()
                .is_some()
        );
        assert!(
            before_rollup
                .get(&partition, encode_property_value_record_key(&property, 2))
                .unwrap()
                .is_some()
        );
        assert_eq!(
            before_rollup
                .get(&partition, encode_property_value_record_key(&property, 3))
                .unwrap(),
            None
        );
        assert_eq!(
            after_rollup
                .get(&partition, encode_property_value_record_key(&property, 1))
                .unwrap(),
            None
        );
        assert_eq!(
            after_rollup
                .get(&partition, encode_property_value_record_key(&property, 2))
                .unwrap(),
            None
        );
        let record = after_rollup
            .get(&partition, encode_property_value_record_key(&property, 3))
            .unwrap()
            .expect("rollup record");
        let mut reconstructor = PropertyValueReconstructor::new(limits);
        reconstructor.push(3, &record).unwrap();
        let reconstructed = reconstructor.finish().unwrap();
        assert_eq!(reconstructed.logical_timestamp, Timestamp(3));
        assert_eq!(reconstructed.value, v_list(&[v_int(1), v_int(2), v_int(3)]));
        assert_eq!(reconstructed.chain, PropertyValueChain::full(3));
    }

    #[test]
    fn rollup_encoding_does_not_use_the_writer_queue() {
        let (writer_sender, _writer_receiver) = flume::bounded::<WriterMsg>(1);
        let (barrier_reply, _barrier_receiver) = oneshot::channel();
        writer_sender
            .send(WriterMsg::Barrier {
                through_version: 1,
                reply: barrier_reply,
            })
            .unwrap();
        assert!(writer_sender.is_full());

        let (rollup_sender, rollup_receiver) = flume::bounded(1);
        let handle = std::thread::spawn(move || BatchWriter::rollup_encoder_loop(rollup_receiver));
        let encoder = RollupEncoder {
            sender: rollup_sender.clone(),
        };
        let encoded = encoder
            .encode(v_list(&[v_int(1), v_int(2)]), Timestamp(9))
            .unwrap();
        assert!(!encoded.record.is_empty());
        assert!(writer_sender.is_full());

        rollup_sender.send(RollupMsg::Stop).unwrap();
        handle.join().unwrap().unwrap();
    }

    struct FailingValue;

    impl BatchValue for FailingValue {
        fn encode(
            self: Box<Self>,
            _timestamp: Timestamp,
            _encoder: &mut BatchEncoder,
        ) -> Result<fjall::Slice, Error> {
            Err(Error::EncodingFailure)
        }
    }

    #[test]
    fn failed_encoding_does_not_complete_snapshot_boundary() {
        let (_tempdir, database) = test_database();
        let partition = database
            .keyspace("values", KeyspaceCreateOptions::default)
            .unwrap();
        let writer = BatchWriter::new(database);
        let mut batch = CommitBatch::with_capacity(2, Timestamp(2), 1);
        batch.insert(partition.clone(), b"key".to_vec(), Box::new(FailingValue));

        write(&writer, batch).unwrap();

        let (reply, receiver) = oneshot::channel();
        writer
            .sender
            .send(WriterMsg::Snapshot {
                through_version: 2,
                reply,
            })
            .unwrap();

        write(&writer, encoded_batch(1, &partition, b"other", b"value")).unwrap();
        let error = match receiver.recv_timeout(Duration::from_secs(1)).unwrap() {
            Ok(_) => panic!("snapshot unexpectedly succeeded"),
            Err(error) => error,
        };

        assert!(error.contains("failed to encode batch value"));
        assert_eq!(writer.completed_version(), 1);
        assert_eq!(partition.get(b"key").unwrap(), None);
    }
}
