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

//! Bounded encoder queues and one publication-ordered PostgreSQL session.
use super::{
    PostgresCommitPolicy, PostgresError, PostgresShutdown, PostgresStorageConfig,
    apply::Session,
    encode::{self, EncodedCommit},
    metrics::Metrics,
};
use crate::{
    ObjAndUUIDHolder, PostgresGroupEnd, Timestamp,
    engine::moor_db::{RelationWorkingSets, Relations},
    provider::{
        backend::SeededWorld,
        batch_writer::WriterWaitError,
        coordinator::{COMMIT_ADMISSION_CAPACITY, CommitAdmission},
        logical::{LogicalCommit, PublicationId, WriterEpoch},
    },
};
use flume::{Receiver, Sender};
use moor_var::Var;
use parking_lot::{Condvar, Mutex};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const POLL: Duration = Duration::from_millis(10);
// Drain only work already available; a light workload never waits to fill a group.
const GROUP_COMMITS: usize = 64;
const GROUP_BYTES: usize = 1024 * 1024;
const GROUP_OPERATIONS: usize = 4096;
const GROUP_AGE: Duration = Duration::from_millis(1);
enum EncodingJob {
    Prepare(LogicalCommit, Sender<Result<EncodedCommit, PostgresError>>),
    Submit(LogicalCommit, CommitAdmission),
}
type EncodedJob = (u64, Result<EncodedCommit, PostgresError>, CommitAdmission);
struct RollupJob {
    key: ObjAndUUIDHolder,
    value: Var,
    timestamp: Timestamp,
    reply: Sender<Result<Value, PostgresError>>,
}
struct Shared {
    metrics: Arc<Metrics>,
    healthy: Arc<AtomicBool>,
    stopping: AtomicBool,
    failure_reported: AtomicBool,
    cancelled: AtomicBool,
    applied: AtomicU64,
    durable: AtomicU64,
    storage_bytes: Arc<AtomicU64>,
    error: Mutex<Option<String>>,
    notification: Condvar,
    notification_lock: Mutex<()>,
    io_shutdown: PostgresShutdown,
}
impl Shared {
    fn running(&self) -> bool {
        !self.stopping.load(Ordering::Acquire) && self.healthy.load(Ordering::Acquire)
    }
    fn wake(&self) {
        let _guard = self.notification_lock.lock();
        self.notification.notify_all();
    }
    fn fail(&self, error: String) {
        self.healthy.store(false, Ordering::Release);
        let first = !self.failure_reported.swap(true, Ordering::AcqRel);
        *self.error.lock() = Some(error.clone());
        self.io_shutdown.request();
        self.wake();
        if first {
            tracing::error!(%error, "PostgreSQL persistence failed; published work may remain unapplied");
            #[cfg(not(test))]
            moor_common::util::signal_fatal_db_error("PostgreSQL writer", &error);
        }
    }
    fn wait(
        &self,
        version: u64,
        durable: bool,
        deadline: Option<Instant>,
    ) -> Result<(), WriterWaitError> {
        let mut guard = self.notification_lock.lock();
        loop {
            if !self.healthy.load(Ordering::Acquire) {
                return Err(WriterWaitError::Failed {
                    detail: self
                        .error
                        .lock()
                        .clone()
                        .unwrap_or_else(|| "PostgreSQL writer failed".into()),
                });
            }
            if self.cancelled.load(Ordering::Acquire) {
                return Err(WriterWaitError::Unavailable);
            }
            let completed = if durable {
                &self.durable
            } else {
                &self.applied
            };
            if completed.load(Ordering::Acquire) >= version {
                return Ok(());
            }
            if deadline.is_some_and(|d| Instant::now() >= d) {
                return Err(WriterWaitError::Timeout { version });
            }
            self.notification.wait_for(
                &mut guard,
                deadline.map_or(POLL, |d| {
                    POLL.min(d.saturating_duration_since(Instant::now()))
                }),
            );
        }
    }
}

pub(crate) struct PostgresWriter {
    config: PostgresStorageConfig,
    epoch: WriterEpoch,
    exports: Arc<super::snapshot::ExportPool>,
    encoder: Sender<EncodingJob>,
    preparation_timeout: Duration,
    encoded: Sender<EncodedJob>,
    fences: Sender<u64>,
    shared: Arc<Shared>,
    submitted: AtomicU64,
    handles: Mutex<Vec<JoinHandle<()>>>,
}

impl PostgresWriter {
    /// The writer creates its own connection and returns only the resident seed across threads.
    pub(crate) fn open(
        config: PostgresStorageConfig,
        relations: Arc<Relations>,
        epoch: WriterEpoch,
    ) -> Result<(Self, SeededWorld, bool, u64), PostgresError> {
        config.validate()?;
        let shared = Arc::new(Shared {
            metrics: Arc::new(Metrics::default()),
            healthy: Arc::new(AtomicBool::new(true)),
            stopping: AtomicBool::new(false),
            failure_reported: AtomicBool::new(false),
            cancelled: AtomicBool::new(false),
            applied: AtomicU64::new(0),
            durable: AtomicU64::new(0),
            storage_bytes: Arc::new(AtomicU64::new(0)),
            error: Mutex::new(None),
            notification: Condvar::new(),
            notification_lock: Mutex::new(()),
            io_shutdown: PostgresShutdown::default(),
        });
        let (encoder, jobs) = flume::bounded::<EncodingJob>(COMMIT_ADMISSION_CAPACITY);
        let (encoded, ready) = flume::bounded::<EncodedJob>(COMMIT_ADMISSION_CAPACITY);
        let (fences, fence_requests) = flume::bounded(64);
        let (rollup_sender, rollups) = flume::bounded::<RollupJob>(1);
        let (opened, opening) = flume::bounded(1);
        let mut handles = Vec::new();
        let writer_shared = shared.clone();
        let writer_config = config.clone();
        handles.push(
            thread::Builder::new()
                .name("pg-writer".into())
                .spawn(move || {
                    let result = Session::open(
                        writer_config.clone(),
                        &relations,
                        epoch,
                        writer_shared.io_shutdown.clone(),
                    );
                    let (mut session, seed) = match result {
                        Ok(value) => value,
                        Err(error) => {
                            let _ = opened.send(Err(error));
                            return;
                        }
                    };
                    session.metrics = writer_shared.metrics.clone();
                    let Some(start_tx) = session.progress.max_timestamp.checked_add(1) else {
                        let _ = opened.send(Err(super::codec::invalid(
                            "max_timestamp",
                            "transaction timestamp exhausted",
                        )));
                        return;
                    };
                    match session.storage_bytes() {
                        Ok(bytes) => writer_shared.storage_bytes.store(bytes, Ordering::Release),
                        Err(error) => {
                            let _ = opened.send(Err(error));
                            return;
                        }
                    }
                    let fresh = session.progress.commits == 0;
                    if opened.send(Ok((seed, fresh, start_tx.max(1)))).is_err() {
                        return;
                    }
                    guarded(&writer_shared, || {
                        writer_loop(
                            &mut session,
                            &writer_config,
                            &writer_shared,
                            ready,
                            fence_requests,
                            rollup_sender,
                        )
                    });
                })
                .map_err(|_| PostgresError::Configuration("cannot spawn PostgreSQL writer"))?,
        );
        let (seed, fresh, start_tx) = match opening.recv() {
            Ok(Ok(value)) => value,
            result => {
                for handle in handles {
                    let _ = handle.join();
                }
                return Err(match result {
                    Ok(Err(error)) => error,
                    _ => PostgresError::Protocol("writer disconnected while opening"),
                });
            }
        };
        // Spawn only after opening succeeds. A failed spawn cancels and joins existing workers.
        let spawn_result = (|| -> Result<(), PostgresError> {
            let rollup_shared = shared.clone();
            let profile = config.profile.clone();
            handles.push(
                thread::Builder::new()
                    .name("pg-rollup".into())
                    .spawn(move || {
                        guarded(&rollup_shared, || {
                            while rollup_shared.running() {
                                let job = match rollups.recv_timeout(POLL) {
                                    Ok(job) => job,
                                    Err(flume::RecvTimeoutError::Timeout) => continue,
                                    Err(flume::RecvTimeoutError::Disconnected) => return Ok(()),
                                };
                                let result = encode::property_row(
                                    &job.key,
                                    &job.value,
                                    job.timestamp,
                                    false,
                                    &profile,
                                );
                                let _ = job.reply.send(result);
                            }
                            Ok(())
                        })
                    })
                    .map_err(|_| {
                        PostgresError::Configuration("cannot spawn PostgreSQL rollup encoder")
                    })?,
            );
            for index in 0..thread::available_parallelism().map_or(1, |v| v.get().min(8)) {
                let shared = shared.clone();
                let jobs = jobs.clone();
                let encoded = encoded.clone();
                let profile = config.profile.clone();
                let max_row_bytes = config.connection.max_row_bytes;
                handles.push(
                    thread::Builder::new()
                        .name(format!("pg-encoder-{index}"))
                        .spawn(move || {
                            guarded(&shared, || {
                                while shared.running() {
                                    let job = match jobs.recv_timeout(POLL) {
                                        Ok(job) => job,
                                        Err(flume::RecvTimeoutError::Timeout) => continue,
                                        Err(flume::RecvTimeoutError::Disconnected) => return Ok(()),
                                    };
                                    let (commit, mut permit) = match job {
                                        EncodingJob::Prepare(commit, reply) => {
                                            let timer = shared.metrics.timer(moor_common::model::WorldStateTimerOp::PostgresEncode);
                                            let mut result = encode::prepare(commit, &profile, max_row_bytes);
                                            let (bytes, retained) = result.as_ref().map_or((0, 0), EncodedCommit::payload_sizes);
                                            shared.metrics.update(|m| {
                                                m.encoding_calls += 1;
                                                m.encoding_failures += u64::from(result.is_err());
                                                m.encoded_bytes += bytes as u64;
                                            });
                                            if let Ok(commit) = &mut result {
                                                commit.payload_lease = Some(shared.metrics.retain(bytes, retained));
                                            }
                                            drop(timer);
                                            let _ = reply.send(result);
                                            continue;
                                        }
                                        EncodingJob::Submit(commit, permit) => (commit, permit),
                                    };
                                    let version = commit.publication.version();
                                    let result = match permit.preparation.postgres.take() {
                                        Some(prepared) => {
                                            Ok(encode::finish_prepared(*prepared, commit))
                                        }
                                        // Internal conformance tests can submit logical batches directly.
                                        None => encode::prepare(commit, &profile, max_row_bytes),
                                    };
                                    let mut message = (version, result, permit);
                                    loop {
                                        if !shared.running() {
                                            return Ok(());
                                        }
                                        match encoded.send_timeout(message, POLL) {
                                            Ok(()) => break,
                                            Err(flume::SendTimeoutError::Timeout(returned)) => {
                                                message = returned
                                            }
                                            Err(flume::SendTimeoutError::Disconnected(_)) => {
                                                return Err(PostgresError::Closed);
                                            }
                                        }
                                    }
                                }
                                Ok(())
                            })
                        })
                        .map_err(|_| {
                            PostgresError::Configuration("cannot spawn PostgreSQL encoder")
                        })?,
                );
            }
            Ok(())
        })();
        if let Err(error) = spawn_result {
            shared.stopping.store(true, Ordering::Release);
            shared.io_shutdown.request();
            for handle in handles {
                let _ = handle.join();
            }
            return Err(error);
        }
        Ok((
            Self {
                encoder,
                epoch,
                exports: super::snapshot::ExportPool::new(config.max_exports),
                config: config.clone(),
                preparation_timeout: config.query_timeout,
                encoded,
                fences,
                shared,
                submitted: AtomicU64::new(0),
                handles: Mutex::new(handles),
            },
            seed,
            fresh,
            start_tx,
        ))
    }
    pub(crate) fn prepare(
        &self,
        changes: &RelationWorkingSets,
        timestamp: Timestamp,
    ) -> Result<EncodedCommit, String> {
        let (reply, result) = flume::bounded(1);
        let commit = LogicalCommit {
            // The actual publication is assigned only after validation and a successful CAS.
            publication: PublicationId::new(WriterEpoch::random(), 0),
            timestamp,
            changes: changes.clone().into_changes(),
            sequences: vec![],
            property_definition_changes: vec![],
        };
        let deadline = Instant::now() + self.preparation_timeout;
        self.encoder
            .send_deadline(EncodingJob::Prepare(commit, reply), deadline)
            .map_err(|_| "PostgreSQL preparation queue unavailable".to_owned())?;
        loop {
            if !self.shared.running() {
                return Err("PostgreSQL writer is unavailable".into());
            }
            if Instant::now() >= deadline {
                return Err("PostgreSQL encoding exceeded its preparation deadline".into());
            }
            match result.recv_timeout(POLL) {
                Ok(result) => return result.map_err(|error| error.to_string()),
                Err(flume::RecvTimeoutError::Timeout) => continue,
                Err(flume::RecvTimeoutError::Disconnected) => {
                    return Err("PostgreSQL encoder stopped".into());
                }
            }
        }
    }

    pub(crate) fn submit(
        &self,
        commit: LogicalCommit,
        mut permit: CommitAdmission,
    ) -> Result<(), String> {
        let version = commit.publication.version();
        if !self.shared.running() {
            return Err("PostgreSQL writer is unavailable".into());
        }
        let submitted = if let Some(prepared) = permit.preparation.postgres.take() {
            let commit = encode::finish_prepared(*prepared, commit);
            if let Some(lease) = &commit.payload_lease {
                lease.published();
            }
            self.encoded
                .try_send((version, Ok(commit), permit))
                .map_err(|_| ())
        } else {
            self.encoder
                .try_send(EncodingJob::Submit(commit, permit))
                .map_err(|_| ())
        };
        submitted.map_err(|()| {
            let error = "PostgreSQL submission queue unavailable after admission".to_string();
            self.shared.fail(error.clone());
            error
        })?;
        self.submitted.fetch_max(version, Ordering::Release);
        Ok(())
    }
    pub(crate) fn diagnostics(&self) -> crate::PostgresPersistenceStats {
        let mut stats = self.shared.metrics.snapshot();
        (stats.active_exports, stats.oldest_export_micros) = self.exports.diagnostics();
        stats.export_limit = self.config.max_exports as u64;
        stats
    }
    pub(crate) fn snapshot(
        &self,
        version: u64,
        timeout: Duration,
    ) -> Result<crate::provider::read::SnapshotReaders, WriterWaitError> {
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(WriterWaitError::Timeout { version })?;
        self.shared.wait(version, false, Some(deadline))?;
        let session = super::snapshot::ReadSession::open(
            self.config.clone(),
            &self.exports,
            PublicationId::new(self.epoch, version),
            deadline,
            || self.shared.running(),
        )
        .map_err(|error| match error {
            PostgresError::Timeout => WriterWaitError::Timeout { version },
            other => WriterWaitError::Failed {
                detail: other.to_string(),
            },
        })?;
        Ok(super::reader::readers(session))
    }
    pub(crate) fn storage_bytes(&self) -> Arc<AtomicU64> {
        self.shared.storage_bytes.clone()
    }
    pub(crate) fn healthy(&self) -> bool {
        self.shared.healthy.load(Ordering::Acquire)
    }
    pub(crate) fn health_flag(&self) -> Arc<AtomicBool> {
        self.shared.healthy.clone()
    }
    pub(crate) fn completed_version(&self) -> u64 {
        self.shared.applied.load(Ordering::Acquire)
    }
    pub(crate) fn durable_version(&self) -> u64 {
        self.shared.durable.load(Ordering::Acquire)
    }
    pub(crate) fn wait_applied(
        &self,
        version: u64,
        timeout: Duration,
    ) -> Result<(), WriterWaitError> {
        self.shared
            .wait(version, false, Some(Instant::now() + timeout))
    }
    pub(crate) fn wait_applied_unbounded(&self, version: u64) -> Result<(), WriterWaitError> {
        self.shared.wait(version, false, None)
    }
    pub(crate) fn wait_durable(
        &self,
        version: u64,
        timeout: Duration,
    ) -> Result<(), WriterWaitError> {
        let deadline = Instant::now() + timeout;
        if self.durable_version() >= version {
            return self.shared.wait(version, true, Some(deadline));
        }
        self.fences
            .send_timeout(version, timeout)
            .map_err(|e| match e {
                flume::SendTimeoutError::Timeout(_) => WriterWaitError::Timeout { version },
                flume::SendTimeoutError::Disconnected(_) => WriterWaitError::Unavailable,
            })?;
        self.shared.wait(version, true, Some(deadline))
    }
    pub(crate) fn cancel_waiters(&self) {
        self.shared.cancelled.store(true, Ordering::Release);
        self.shared.wake();
    }
    pub(crate) fn stop_with_deadline(&self, timeout: Duration) -> Result<(), String> {
        let deadline = Instant::now() + timeout;
        let mut handles = self
            .handles
            .try_lock_for(timeout)
            .ok_or_else(|| "PostgreSQL shutdown is already in progress".to_string())?;
        if handles.is_empty() {
            return self.shared.error.lock().clone().map_or(Ok(()), Err);
        }
        let drain = self
            .wait_applied(
                self.submitted.load(Ordering::Acquire),
                deadline.saturating_duration_since(Instant::now()),
            )
            .map_err(|e| e.to_string());
        self.shared.stopping.store(true, Ordering::Release);
        self.shared.io_shutdown.request();
        self.cancel_waiters();
        let result = (|| {
            for handle in handles.drain(..) {
                while !handle.is_finished() && Instant::now() < deadline {
                    thread::sleep(POLL.min(deadline.saturating_duration_since(Instant::now())));
                }
                if !handle.is_finished() {
                    return Err(
                    "PostgreSQL shutdown timed out with incomplete persistence or worker cleanup"
                        .into(),
                );
                }
                if handle.join().is_err() {
                    return Err("PostgreSQL worker panicked".into());
                }
            }
            drain
        })();
        if let Err(error) = &result {
            *self.shared.error.lock() = Some(error.clone());
            self.shared.healthy.store(false, Ordering::Release);
        }
        result
    }
}
impl Drop for PostgresWriter {
    fn drop(&mut self) {
        self.shared.stopping.store(true, Ordering::Release);
        self.shared.io_shutdown.request();
        self.cancel_waiters();
    }
}
fn guarded(shared: &Shared, run: impl FnOnce() -> Result<(), PostgresError>) {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)) {
        Ok(Ok(())) => {}
        Ok(Err(error)) if shared.stopping.load(Ordering::Acquire) && is_shutdown(&error) => {}
        Ok(Err(error)) => shared.fail(error.to_string()),
        Err(_) => shared.fail("PostgreSQL persistence worker panicked".into()),
    }
}
fn is_shutdown(error: &PostgresError) -> bool {
    match error {
        // Worker queues can disconnect when peers exit after the drain completes.
        PostgresError::Shutdown | PostgresError::Closed => true,
        PostgresError::Operation { source, .. } => is_shutdown(source),
        _ => false,
    }
}
fn writer_loop(
    session: &mut Session,
    config: &PostgresStorageConfig,
    shared: &Shared,
    ready: Receiver<EncodedJob>,
    fences: Receiver<u64>,
    rollup: Sender<RollupJob>,
) -> Result<(), PostgresError> {
    let mut pending = BTreeMap::new();
    let mut fence_requests = BTreeSet::new();
    let mut last_size_sample = Instant::now();
    while shared.running() {
        if last_size_sample.elapsed() >= Duration::from_secs(5) {
            match session.storage_bytes() {
                Ok(bytes) => shared.storage_bytes.store(bytes, Ordering::Release),
                Err(error) => tracing::debug!(%error, "PostgreSQL size sample unavailable"),
            }
            last_size_sample = Instant::now();
        }
        fence_requests.extend(fences.try_iter());
        fence_requests.retain(|through| *through > shared.durable.load(Ordering::Acquire));
        if fence_requests
            .first()
            .is_some_and(|through| *through <= session.progress.applied)
        {
            session.fence()?;
            shared
                .durable
                .store(session.progress.applied, Ordering::Release);
            shared.wake();
        }
        let next_version = session
            .progress
            .applied
            .checked_add(1)
            .ok_or_else(|| super::codec::invalid("applied_version", "counter exhausted"))?;
        if !pending.contains_key(&next_version) {
            match wait_for_work(&ready, &fences, POLL) {
                Ok(WriterEvent::Commit(job)) => {
                    insert_pending(
                        &mut pending,
                        job.map_err(|_| PostgresError::Closed)?,
                        session.progress.applied,
                    )?;
                }
                Ok(WriterEvent::Fence(through)) => {
                    fence_requests.insert(through.map_err(|_| PostgresError::Closed)?);
                    continue;
                }
                Err(flume::select::SelectError::Timeout) => continue,
            }
        }
        // Bound channel draining too, so an encoder stream cannot starve fences or SQL.
        for job in ready.try_iter().take(GROUP_COMMITS) {
            insert_pending(&mut pending, job, session.progress.applied)?;
        }
        fence_requests.extend(fences.try_iter());
        if fence_requests.iter().any(|through| {
            *through > shared.durable.load(Ordering::Acquire)
                && *through <= session.progress.applied
        }) {
            continue;
        }
        let (commits, permits, reason) = take_group(
            &mut pending,
            next_version,
            fence_requests.range(next_version..).next().copied(),
            Instant::now() + GROUP_AGE,
        )?;
        if !commits.is_empty() {
            session.group_end = reason;
            let applied = session.apply_group(&commits, GROUP_BYTES, |key, value, timestamp| {
                let (reply, response) = flume::bounded(1);
                rollup
                    .try_send(RollupJob {
                        key: key.clone(),
                        value,
                        timestamp,
                        reply,
                    })
                    .map_err(|_| PostgresError::Closed)?;
                let deadline = Instant::now() + config.query_timeout;
                loop {
                    shared.io_shutdown.check(deadline)?;
                    match response.recv_timeout(POLL) {
                        Ok(result) => return result,
                        Err(flume::RecvTimeoutError::Timeout) => continue,
                        Err(flume::RecvTimeoutError::Disconnected) => {
                            return Err(PostgresError::Closed);
                        }
                    }
                }
            })?;
            // The publication watermark becomes visible only after chain state is confirmed.
            for (index, (commit, permit)) in commits.into_iter().zip(permits).enumerate() {
                if index < applied {
                    drop(permit);
                } else {
                    pending.insert(commit.publication.version(), (Ok(commit), permit));
                }
            }
            shared
                .applied
                .store(session.progress.applied, Ordering::Release);
            if config.commit_policy == PostgresCommitPolicy::Synchronous {
                shared
                    .durable
                    .store(session.progress.applied, Ordering::Release);
            }
            shared.wake();
        }
    }
    Ok(())
}

enum WriterEvent<T> {
    Commit(Result<T, flume::RecvError>),
    Fence(Result<u64, flume::RecvError>),
}

fn wait_for_work<T>(
    ready: &Receiver<T>,
    fences: &Receiver<u64>,
    timeout: Duration,
) -> Result<WriterEvent<T>, flume::select::SelectError> {
    flume::Selector::new()
        .recv(ready, WriterEvent::Commit)
        .recv(fences, WriterEvent::Fence)
        .wait_timeout(timeout)
}

type Pending<P> = BTreeMap<u64, (Result<EncodedCommit, PostgresError>, P)>;

fn insert_pending<P>(
    pending: &mut Pending<P>,
    (version, commit, permit): (u64, Result<EncodedCommit, PostgresError>, P),
    applied: u64,
) -> Result<(), PostgresError> {
    if version <= applied || pending.insert(version, (commit, permit)).is_some() {
        return Err(super::codec::invalid(
            "applied_version",
            "duplicate publication",
        ));
    }
    Ok(())
}

/// A single large logical commit stays indivisible. All other members fit within
/// count, payload, operation and collection-time limits, and stop at a known fence.
fn take_group<P>(
    pending: &mut Pending<P>,
    mut next: u64,
    fence: Option<u64>,
    deadline: Instant,
) -> Result<(Vec<EncodedCommit>, Vec<P>, PostgresGroupEnd), PostgresError> {
    let mut commits = Vec::new();
    let mut permits = Vec::new();
    let mut bytes = 0usize;
    let mut operations = 0usize;
    let mut reason = PostgresGroupEnd::Available;
    while let Some((commit, _)) = pending.get(&next) {
        if !commits.is_empty() {
            if commits.len() >= GROUP_COMMITS {
                reason = PostgresGroupEnd::CommitLimit;
                break;
            }
            if Instant::now() >= deadline {
                reason = PostgresGroupEnd::AgeLimit;
                break;
            }
            if let Ok(commit) = commit {
                if bytes.saturating_add(commit.group_bytes) > GROUP_BYTES {
                    reason = PostgresGroupEnd::PayloadLimit;
                    break;
                }
                if operations.saturating_add(commit.group_operations) > GROUP_OPERATIONS {
                    reason = PostgresGroupEnd::OperationLimit;
                    break;
                }
            }
        }
        let (commit, permit) = pending.remove(&next).unwrap();
        let commit = commit?;
        bytes = bytes.saturating_add(commit.group_bytes);
        operations = operations.saturating_add(commit.group_operations);
        commits.push(commit);
        permits.push(permit);
        if fence == Some(next) {
            reason = PostgresGroupEnd::Fence;
            break;
        }
        if next == u64::MAX {
            break;
        }
        next += 1;
    }
    Ok((commits, permits, reason))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::postgres::tests::empty;

    #[test]
    fn idle_writer_wakes_for_a_fence_without_a_commit() {
        let (_send_commit, ready) = flume::bounded::<()>(1);
        let (send_fence, fences) = flume::bounded(1);
        let (started, start) = flume::bounded(1);
        let (completed, completion) = flume::bounded(1);
        let worker = thread::spawn(move || {
            started.send(()).unwrap();
            let result = wait_for_work(&ready, &fences, Duration::from_secs(30));
            completed
                .send(matches!(result, Ok(WriterEvent::Fence(Ok(42)))))
                .unwrap();
        });
        start.recv().unwrap();
        send_fence.send(42).unwrap();
        // A wide deadline tests channel wakeup rather than sub-millisecond scheduling.
        assert!(completion.recv_timeout(Duration::from_secs(2)).unwrap());
        worker.join().unwrap();
    }

    fn pending(count: u64, bytes: usize, operations: usize) -> Pending<()> {
        let epoch = WriterEpoch::random();
        (1..=count)
            .map(|version| {
                let mut commit = empty(epoch, version, version);
                commit.group_bytes = bytes;
                commit.group_operations = operations;
                (version, (Ok(commit), ()))
            })
            .collect()
    }

    #[test]
    fn groups_stop_at_gaps_fences_and_work_limits() {
        for (bytes, operations, expected, reason) in [
            (0, 0, GROUP_COMMITS, PostgresGroupEnd::CommitLimit),
            (GROUP_BYTES / 3, 0, 3, PostgresGroupEnd::PayloadLimit),
            (0, GROUP_OPERATIONS / 3, 3, PostgresGroupEnd::OperationLimit),
            (
                GROUP_BYTES + 1,
                GROUP_OPERATIONS + 1,
                1,
                PostgresGroupEnd::PayloadLimit,
            ),
        ] {
            let mut pending = pending(100, bytes, operations);
            let (group, permits, end) = take_group(
                &mut pending,
                1,
                None,
                Instant::now() + Duration::from_secs(2),
            )
            .unwrap();
            assert_eq!(end, reason);
            assert_eq!(group.len(), expected);
            assert_eq!(permits.len(), expected);
            assert_eq!(pending.len(), 100 - expected);
        }
        let mut gap = pending(5, 0, 0);
        gap.remove(&3);
        assert_eq!(
            take_group(&mut gap, 1, None, Instant::now() + GROUP_AGE)
                .unwrap()
                .0
                .len(),
            2
        );
        let mut barrier = pending(5, 0, 0);
        assert_eq!(
            take_group(
                &mut barrier,
                1,
                Some(3),
                Instant::now() + Duration::from_secs(2)
            )
            .unwrap()
            .0
            .len(),
            3
        );
        let mut aged = pending(5, 0, 0);
        assert_eq!(
            take_group(&mut aged, 1, None, Instant::now())
                .unwrap()
                .0
                .len(),
            1
        );
    }
}
