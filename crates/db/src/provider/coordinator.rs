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

//! Persistence coordination: admission, submission, receipts, and lifecycle.
//!
//! The coordinator owns the bounded admission permit pool and the backend writer. Transactions
//! must acquire a permit before publishing a root, and the permit is held until the backend has
//! applied that commit. Waiters request an applied or durable prefix by `PublicationId`, never by
//! an arbitrary future version.

use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::Duration;

use flume::{Receiver, Sender};
use moor_common::model::{WorldStateCountOp, WorldStateTimerOp};
use moor_common::util::Instant;
use parking_lot::Mutex;
use tracing::warn;

use crate::api::world_state::db_counters;
use crate::config::{AdmissionPolicy, PersistenceConfig};
#[cfg(test)]
use crate::provider::batch_writer::BatchWriter;
use crate::provider::logical::{
    LogicalCommit, PersistenceError, PersistenceReceipt, PublicationId, WriterEpoch,
};
use crate::provider::writer::StorageWriter;
use crate::tx::Timestamp;

/// Bounded number of admitted-but-unapplied write commits, and the size of the encoder queue.
pub(crate) const COMMIT_ADMISSION_CAPACITY: usize = 1000;

#[derive(Debug, thiserror::Error)]
pub(crate) enum CommitAdmissionError {
    #[error("database commit queue remained full for {waited:?}")]
    Timeout { waited: Duration },
    #[error("database commit queue admission is unavailable")]
    Unavailable,
}

/// A reserved slot in the bounded persistence pipeline.
///
/// The permit is held from pre-publication reservation until the backend applies the commit.
/// Dropping it returns the slot to the pool.
pub(crate) struct CommitAdmission {
    #[cfg(feature = "postgres")]
    pub(crate) preparation: super::writer::StoragePreparation,
    return_to: Sender<()>,
    reservation: Option<SubmissionReservation>,
    healthy: Arc<AtomicBool>,
}

/// Pins an admitted attempt until submission completes or the attempt is abandoned.
struct SubmissionReservation(Arc<CommitAdmissionGate>);

impl Drop for SubmissionReservation {
    fn drop(&mut self) {
        self.0.reservations.fetch_sub(1, Ordering::Release);
    }
}

impl CommitAdmission {
    pub(crate) fn fail(&self) {
        self.healthy.store(false, Ordering::Release);
    }
}

const ADMISSION_CLOSED: u64 = 1 << 63;

impl Drop for CommitAdmission {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.fail();
        }
        let _ = self.return_to.try_send(());
    }
}

#[derive(Default)]
struct BackpressureEpisode {
    started_at: Option<moor_common::util::Instant>,
    waiters: usize,
    rejected: usize,
    warned: bool,
}

struct CommitAdmissionGate {
    available: Receiver<()>,
    return_to: Sender<()>,
    capacity: usize,
    warn_after_nanos: AtomicU64,
    timeout_nanos: AtomicU64,
    reservations: AtomicU64,
    healthy: Arc<AtomicBool>,
    episode: Mutex<BackpressureEpisode>,
}

/// An unfinished wait is abandoned, not counted as a timeout rejection.
/// Only admission and timeout contribute to the existing block-time counter.
struct AdmissionWait<'a> {
    gate: &'a CommitAdmissionGate,
    started_at: Instant,
    outcome: AdmissionWaitOutcome,
}

enum AdmissionWaitOutcome {
    Abandoned,
    Admitted(Duration),
    TimedOut(Duration),
    Disconnected,
}

impl AdmissionWait<'_> {
    fn elapsed(&self) -> Duration {
        self.started_at.elapsed()
    }

    fn admitted(mut self, reservation: SubmissionReservation) -> CommitAdmission {
        self.outcome = AdmissionWaitOutcome::Admitted(self.elapsed());
        self.gate.permit(reservation)
    }

    fn timed_out(mut self, waited: Duration) -> CommitAdmissionError {
        self.outcome = AdmissionWaitOutcome::TimedOut(waited);
        CommitAdmissionError::Timeout { waited }
    }

    fn disconnected(mut self) -> CommitAdmissionError {
        self.outcome = AdmissionWaitOutcome::Disconnected;
        CommitAdmissionError::Unavailable
    }
}

impl Drop for AdmissionWait<'_> {
    fn drop(&mut self) {
        self.gate
            .finish_wait(matches!(self.outcome, AdmissionWaitOutcome::TimedOut(_)));
        if let AdmissionWaitOutcome::Admitted(waited) | AdmissionWaitOutcome::TimedOut(waited) =
            self.outcome
        {
            db_counters()
                .timers_rare
                .record_elapsed(WorldStateTimerOp::BatchWriterBackpressureBlock, waited);
        }
    }
}

impl CommitAdmissionGate {
    fn new(capacity: usize, policy: AdmissionPolicy, healthy: Arc<AtomicBool>) -> Self {
        let (return_to, available) = flume::bounded(capacity);
        for _ in 0..capacity {
            return_to
                .send(())
                .expect("commit admission token channel must accept its initial capacity");
        }
        Self {
            available,
            return_to,
            capacity,
            warn_after_nanos: AtomicU64::new(Self::duration_nanos(policy.warn_after)),
            timeout_nanos: AtomicU64::new(Self::duration_nanos(policy.timeout)),
            reservations: AtomicU64::new(0),
            healthy,
            episode: Mutex::new(BackpressureEpisode::default()),
        }
    }

    fn duration_nanos(duration: Duration) -> u64 {
        u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
    }

    fn duration_from_nanos(nanos: u64) -> Duration {
        Duration::from_nanos(nanos)
    }

    fn set_policy(&self, warn_after: Duration, timeout: Duration) {
        self.warn_after_nanos
            .store(Self::duration_nanos(warn_after), Ordering::Release);
        self.timeout_nanos
            .store(Self::duration_nanos(timeout), Ordering::Release);
    }

    fn policy(&self) -> AdmissionPolicy {
        AdmissionPolicy {
            warn_after: Self::duration_from_nanos(self.warn_after_nanos.load(Ordering::Acquire)),
            timeout: Self::duration_from_nanos(self.timeout_nanos.load(Ordering::Acquire)),
        }
    }

    fn permit(&self, reservation: SubmissionReservation) -> CommitAdmission {
        CommitAdmission {
            #[cfg(feature = "postgres")]
            preparation: Default::default(),
            return_to: self.return_to.clone(),
            reservation: Some(reservation),
            healthy: self.healthy.clone(),
        }
    }

    fn begin_wait(&self) -> AdmissionWait<'_> {
        let started_at = Instant::now();
        let mut episode = self.episode.lock();
        episode.waiters += 1;
        episode.started_at.get_or_insert_with(Instant::now);
        AdmissionWait {
            gate: self,
            started_at,
            outcome: AdmissionWaitOutcome::Abandoned,
        }
    }

    fn warn_if_needed(&self, transaction: Timestamp, waited: Duration) {
        let waiters = {
            let mut episode = self.episode.lock();
            if episode.warned {
                return;
            }
            episode.warned = true;
            episode.waiters
        };
        warn!(
            transaction = transaction.0,
            ?waited,
            waiters,
            queue_used = self.capacity.saturating_sub(self.available.len()),
            queue_capacity = self.capacity,
            "Database commit queue remains full"
        );
    }

    fn finish_wait(&self, rejected: bool) {
        let finished = {
            let mut episode = self.episode.lock();
            episode.waiters = episode.waiters.saturating_sub(1);
            if rejected {
                episode.rejected += 1;
            }
            if episode.waiters != 0 {
                return;
            }
            let result = episode
                .started_at
                .map(|started_at| (started_at.elapsed(), episode.rejected, episode.warned));
            *episode = BackpressureEpisode::default();
            result
        };
        if let Some((elapsed, rejected, true)) = finished {
            warn!(
                ?elapsed,
                rejected, "Database commit queue wait episode ended"
            );
        }
    }

    fn acquire(
        self: &Arc<Self>,
        transaction: Timestamp,
    ) -> Result<CommitAdmission, CommitAdmissionError> {
        self.reservations
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |state| {
                (state & ADMISSION_CLOSED == 0 && self.healthy.load(Ordering::Acquire))
                    .then_some(state + 1)
            })
            .map_err(|_| CommitAdmissionError::Unavailable)?;
        let reservation = SubmissionReservation(self.clone());
        match self.available.try_recv() {
            Ok(()) => {
                if self.is_closed() {
                    self.return_to.try_send(()).ok();
                    return Err(CommitAdmissionError::Unavailable);
                }
                return Ok(self.permit(reservation));
            }
            Err(flume::TryRecvError::Disconnected) => {
                return Err(CommitAdmissionError::Unavailable);
            }
            Err(flume::TryRecvError::Empty) => {}
        }

        db_counters()
            .counters
            .inc(WorldStateCountOp::BatchWriterBackpressure);
        let wait = self.begin_wait();

        loop {
            if self.is_closed() {
                return Err(wait.disconnected());
            }
            let policy = self.policy();
            let waited = wait.elapsed();
            if waited >= policy.timeout {
                self.warn_if_needed(transaction, waited);
                return Err(wait.timed_out(waited));
            }

            let until_warning = policy.warn_after.saturating_sub(waited);
            let until_timeout = policy.timeout.saturating_sub(waited);
            let wait_for = if until_warning.is_zero() {
                self.warn_if_needed(transaction, waited);
                until_timeout
            } else {
                until_warning.min(until_timeout)
            };

            match self
                .available
                .recv_timeout(wait_for.min(Duration::from_millis(10)))
            {
                Ok(()) => {
                    if self.is_closed() {
                        self.return_to.try_send(()).ok();
                        return Err(wait.disconnected());
                    }
                    return Ok(wait.admitted(reservation));
                }
                Err(flume::RecvTimeoutError::Timeout) => {
                    let waited = wait.elapsed();
                    if waited >= policy.warn_after {
                        self.warn_if_needed(transaction, waited);
                    }
                }
                Err(flume::RecvTimeoutError::Disconnected) => {
                    return Err(wait.disconnected());
                }
            }
        }
    }

    fn is_closed(&self) -> bool {
        self.reservations.load(Ordering::Acquire) & ADMISSION_CLOSED != 0
            || !self.healthy.load(Ordering::Acquire)
    }

    fn pending_submissions(&self) -> u64 {
        self.reservations.load(Ordering::Acquire) & !ADMISSION_CLOSED
    }

    fn close(&self) {
        self.reservations
            .fetch_or(ADMISSION_CLOSED, Ordering::AcqRel);
    }
}

/// Point-in-time persistence progress and health.
#[derive(Clone, Copy, Debug)]
pub struct PersistenceStatus {
    pub postgres: Option<crate::PostgresPersistenceStats>,
    /// False when concurrent publication prevented a stable progress/admission sample.
    pub sampling_consistent: bool,
    pub epoch: WriterEpoch,
    /// Highest publication observed from a world root.
    pub published: u64,
    /// Highest submitted version; earlier submissions can still be in flight.
    pub last_submitted: u64,
    /// Admitted commits that have not yet applied, including unpublished reservations.
    pub outstanding: usize,
    pub healthy: bool,
    pub applied: u64,
    pub durable: u64,
    pub shutdown: bool,
}

impl PersistenceStatus {
    /// Numeric operator values. The builtin exposes these as `{value, 0}` entries.
    /// Backend payload gauges have their own sampling instant, after the progress sample.
    pub fn operator_metrics(&self) -> Vec<(&'static str, u64)> {
        let mut metrics = vec![
            ("persistence_published", self.published),
            ("persistence_applied", self.applied),
            ("persistence_durable", self.durable),
            ("persistence_outstanding", self.outstanding as u64),
            ("persistence_healthy", u64::from(self.healthy)),
            (
                "persistence_sampling_consistent",
                u64::from(self.sampling_consistent),
            ),
        ];
        if let Some(pg) = &self.postgres {
            metrics.extend(pg.operator_metrics());
        }
        metrics
    }
}

/// Owns admission, publication tracking, waiters, and backend writer lifecycle.
pub(crate) struct PersistenceCoordinator {
    epoch: WriterEpoch,
    admission: Arc<CommitAdmissionGate>,
    writer: StorageWriter,
    last_submitted: AtomicU64,
    published: AtomicU64,
    shutdown: AtomicBool,
    shutdown_timeout: Duration,
}

impl PersistenceCoordinator {
    pub(crate) fn new(
        epoch: WriterEpoch,
        config: PersistenceConfig,
        writer: impl Into<StorageWriter>,
    ) -> Self {
        let writer = writer.into();
        Self {
            epoch,
            admission: Arc::new(CommitAdmissionGate::new(
                COMMIT_ADMISSION_CAPACITY,
                config.admission,
                writer.health_flag(),
            )),
            writer,
            last_submitted: AtomicU64::new(0),
            published: AtomicU64::new(0),
            shutdown: AtomicBool::new(false),
            shutdown_timeout: config.shutdown_timeout,
        }
    }

    pub(crate) fn epoch(&self) -> WriterEpoch {
        self.epoch
    }

    /// Record a version read from a published world root, even before its batch is submitted.
    pub(crate) fn published(&self, version: u64) -> PublicationId {
        self.published.fetch_max(version, Ordering::AcqRel);
        PublicationId::new(self.epoch, version)
    }

    /// Reserve a persistence slot before publishing a write.
    pub(crate) fn admit(
        &self,
        transaction: Timestamp,
    ) -> Result<CommitAdmission, CommitAdmissionError> {
        if self.shutdown.load(Ordering::Acquire) || !self.writer.healthy() {
            return Err(CommitAdmissionError::Unavailable);
        }
        self.admission.acquire(transaction)
    }

    /// Validate backend limits before any root can become visible.
    pub(crate) fn prepare(
        &self,
        changes: &crate::engine::moor_db::RelationWorkingSets,
        timestamp: Timestamp,
        admission: &mut CommitAdmission,
    ) -> Result<(), String> {
        self.writer.prepare(changes, timestamp, admission)
    }

    /// Transfer a published logical commit and its permit to the backend.
    pub(crate) fn submit(
        &self,
        commit: LogicalCommit,
        mut admission: CommitAdmission,
    ) -> Result<(), PersistenceError> {
        // Shutdown must wait until the transfer finishes, including an abandoned attempt.
        let _reservation = admission.reservation.take();
        self.ensure_writer_healthy()?;
        let publication = commit.publication;
        if publication.epoch() != self.epoch {
            return Err(PersistenceError::StalePublication { publication });
        }
        self.published(publication.version());
        self.last_submitted
            .fetch_max(publication.version(), Ordering::AcqRel);
        let result = self.writer.submit(commit, admission);
        if result.is_err() {
            self.admission.healthy.store(false, Ordering::Release);
        }
        result.map_err(|detail| PersistenceError::WriterFailed { detail })
    }

    /// Wait until the backend reports an applied prefix through this publication.
    pub(crate) fn wait_applied(
        &self,
        publication: PublicationId,
        deadline: Duration,
    ) -> Result<PersistenceReceipt, PersistenceError> {
        self.validate_publication(publication)?;
        self.writer
            .wait_applied(publication.version(), deadline)
            .map_err(|error| self.map_wait_error(publication.version(), error))?;
        Ok(PersistenceReceipt { publication })
    }

    /// Unbounded applied wait used by the public application-level barrier.
    ///
    /// The publication can precede batch submission. The backend holds the barrier until the
    /// target version is applied or the writer fails, without a caller deadline.
    pub(crate) fn wait_applied_unbounded(
        &self,
        publication: PublicationId,
    ) -> Result<PersistenceReceipt, PersistenceError> {
        if publication.epoch() != self.epoch {
            return Err(PersistenceError::StalePublication { publication });
        }
        if publication.version() == 0 {
            return Ok(PersistenceReceipt { publication });
        }
        self.writer
            .wait_applied_unbounded(publication.version())
            .map_err(|error| self.map_wait_error(publication.version(), error))?;
        Ok(PersistenceReceipt { publication })
    }

    /// Wait until an applied prefix has crossed the backend's durable-storage fence.
    pub(crate) fn wait_durable(
        &self,
        publication: PublicationId,
        deadline: Duration,
    ) -> Result<PersistenceReceipt, PersistenceError> {
        self.validate_publication(publication)?;
        self.writer
            .wait_durable(publication.version(), deadline)
            .map_err(|error| self.map_wait_error(publication.version(), error))?;
        Ok(PersistenceReceipt { publication })
    }

    fn ensure_writer_healthy(&self) -> Result<(), PersistenceError> {
        if self.writer.healthy() {
            return Ok(());
        }
        Err(PersistenceError::WriterFailed {
            detail: "persistence writer has failed; the database must be reopened".to_string(),
        })
    }

    fn validate_publication(&self, publication: PublicationId) -> Result<(), PersistenceError> {
        if publication.epoch() != self.epoch {
            return Err(PersistenceError::StalePublication { publication });
        }
        if publication.version() > self.published.load(Ordering::Acquire) {
            return Err(PersistenceError::UnpublishedPublication {
                version: publication.version(),
            });
        }
        Ok(())
    }

    fn map_wait_error(
        &self,
        version: u64,
        error: crate::provider::batch_writer::WriterWaitError,
    ) -> PersistenceError {
        use crate::provider::batch_writer::WriterWaitError;
        match error {
            WriterWaitError::ResourceBusy => PersistenceError::ResourceBusy,
            WriterWaitError::Timeout { .. } => PersistenceError::Timeout { version },
            WriterWaitError::Failed { detail } => PersistenceError::WriterFailed { detail },
            WriterWaitError::Unavailable => PersistenceError::WriterFailed {
                detail: "persistence writer is unavailable".to_string(),
            },
        }
    }

    /// Acquire a storage snapshot after the requested version has applied.
    pub(crate) fn snapshot(
        &self,
        through_version: u64,
        deadline: Duration,
    ) -> Result<super::backend::StorageSnapshot, PersistenceError> {
        if self.shutdown.load(Ordering::Acquire) {
            return Err(PersistenceError::ShutDown);
        }
        self.ensure_writer_healthy()?;
        self.writer
            .snapshot(through_version, deadline)
            .map_err(|error| self.map_wait_error(through_version, error))
    }

    pub(crate) fn status(&self) -> PersistenceStatus {
        // Read downstream progress first: its prerequisite is already visible. Bracket
        // admission with publication reads and retry transient permit-return windows.
        // Sampling must remain bounded even if a producer publishes continuously.
        for attempt in 0..8 {
            let before = self.published.load(Ordering::Acquire);
            let outstanding = self
                .admission
                .capacity
                .saturating_sub(self.admission.available.len());
            let durable = self.writer.durable_version();
            let applied = self.writer.completed_version();
            let last_submitted = self.last_submitted.load(Ordering::Acquire);
            let published = self.published.load(Ordering::Acquire);
            let healthy = self.writer.healthy();
            let sampling_consistent =
                before == published && outstanding as u64 >= published.saturating_sub(applied);
            if sampling_consistent || attempt == 7 {
                return PersistenceStatus {
                    postgres: self.writer.postgres_diagnostics(),
                    sampling_consistent,
                    epoch: self.epoch,
                    published,
                    outstanding,
                    healthy,
                    last_submitted,
                    applied,
                    durable,
                    shutdown: self.shutdown.load(Ordering::Acquire),
                };
            }
            std::hint::spin_loop();
        }
        unreachable!()
    }

    pub(crate) fn set_commit_queue_policy(&self, warn_after: Duration, timeout: Duration) {
        self.admission.set_policy(warn_after, timeout);
    }

    /// Stop admission, drain published work, and stop backend workers.
    pub(crate) fn shutdown(&self) -> Result<(), PersistenceError> {
        self.shutdown.store(true, Ordering::Release);
        self.admission.close();
        let started = std::time::Instant::now();
        while self.admission.pending_submissions() != 0 {
            if started.elapsed() >= self.shutdown_timeout {
                self.admission.healthy.store(false, Ordering::Release);
                self.writer.cancel_waiters();
                return Err(PersistenceError::Timeout {
                    version: self.last_submitted.load(Ordering::Acquire),
                });
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        self.writer
            .stop_with_deadline(self.shutdown_timeout.saturating_sub(started.elapsed()))
            .map_err(|detail| PersistenceError::WriterFailed { detail })
    }

    #[cfg(test)]
    pub(crate) fn hold_all_admission(&self) -> Vec<CommitAdmission> {
        (0..COMMIT_ADMISSION_CAPACITY)
            .map(|index| {
                self.admit(Timestamp(index as u64))
                    .expect("test should be able to reserve the configured admission capacity")
            })
            .collect()
    }
}

impl Drop for PersistenceCoordinator {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            tracing::error!("Failed to stop batch writer: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PersistenceConfig;
    use crate::provider::batch_writer::{
        BatchEncoder, BatchValue, CommitBatch, EncodeRequest, WriterWaitError,
    };
    use crate::tx::Error as TxError;
    use fjall::KeyspaceCreateOptions;

    #[test]
    fn admission_wait_guards_close_overlapping_and_abandoned_episodes() {
        let gate = Arc::new(CommitAdmissionGate::new(
            1,
            AdmissionPolicy {
                warn_after: Duration::from_secs(1),
                timeout: Duration::from_secs(2),
            },
            Arc::new(AtomicBool::new(true)),
        ));
        let last = gate.begin_wait();
        let timed_out = gate.begin_wait();
        let disconnected = gate.begin_wait();
        gate.warn_if_needed(Timestamp(1), Duration::from_secs(1));
        assert!(matches!(
            timed_out.timed_out(Duration::from_secs(2)),
            CommitAdmissionError::Timeout { .. }
        ));
        assert!(matches!(
            disconnected.disconnected(),
            CommitAdmissionError::Unavailable
        ));
        {
            let episode = gate.episode.lock();
            assert_eq!(episode.waiters, 1);
            assert_eq!(episode.rejected, 1);
            assert!(episode.warned);
        }
        // Unwinding also drops a registered wait, without adding a rejection.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _wait = gate.begin_wait();
            panic!("abandon admission");
        }));
        assert!(result.is_err());
        assert_eq!(gate.episode.lock().waiters, 1);
        assert_eq!(gate.episode.lock().rejected, 1);
        drop(last);
        let episode = gate.episode.lock();
        assert_eq!(episode.waiters, 0);
        assert_eq!(episode.rejected, 0);
        assert!(!episode.warned);
        assert!(episode.started_at.is_none());
    }

    #[test]
    fn successful_slow_admission_closes_its_episode_and_returns_permit() {
        let gate = Arc::new(CommitAdmissionGate::new(
            1,
            AdmissionPolicy {
                warn_after: Duration::from_secs(1),
                timeout: Duration::from_secs(5),
            },
            Arc::new(AtomicBool::new(true)),
        ));
        let held = gate.acquire(Timestamp(1)).unwrap();
        let waiter = {
            let gate = gate.clone();
            std::thread::spawn(move || gate.acquire(Timestamp(2)).unwrap())
        };
        let deadline = Instant::now() + Duration::from_secs(2);
        while gate.episode.lock().waiters == 0 {
            assert!(Instant::now() < deadline, "waiter did not enter admission");
            std::thread::yield_now();
        }
        drop(held);
        let admitted = waiter.join().unwrap();
        assert_eq!(gate.episode.lock().waiters, 0);
        assert!(gate.episode.lock().started_at.is_none());
        assert!(gate.available.is_empty());
        drop(admitted);
        assert_eq!(gate.available.len(), 1);
    }

    #[test]
    fn admission_times_out_and_recovers_without_waiting_forever() {
        let gate = Arc::new(CommitAdmissionGate::new(
            1,
            AdmissionPolicy {
                warn_after: Duration::from_millis(1),
                timeout: Duration::from_millis(10),
            },
            Arc::new(AtomicBool::new(true)),
        ));
        let held = gate.acquire(Timestamp(1)).unwrap();

        let error = match gate.acquire(Timestamp(2)) {
            Ok(_) => panic!("admission unexpectedly succeeded while its only permit was held"),
            Err(error) => error,
        };
        assert!(matches!(error, CommitAdmissionError::Timeout { .. }));
        assert_eq!(gate.episode.lock().waiters, 0);
        assert!(gate.episode.lock().started_at.is_none());

        drop(held);
        assert!(gate.acquire(Timestamp(3)).is_ok());
    }

    fn test_policy() -> AdmissionPolicy {
        AdmissionPolicy {
            warn_after: Duration::from_millis(1),
            timeout: Duration::from_millis(25),
        }
    }

    fn test_config() -> PersistenceConfig {
        PersistenceConfig {
            admission: test_policy(),
            shutdown_timeout: Duration::from_secs(1),
        }
    }

    struct BlockingValue {
        started: Sender<()>,
        release: Receiver<()>,
    }

    impl BatchValue for BlockingValue {
        fn encode(
            self: Box<Self>,
            _timestamp: Timestamp,
            _encoder: &mut BatchEncoder,
        ) -> Result<fjall::Slice, TxError> {
            let _ = self.started.send(());
            let _ = self.release.recv();
            Ok(vec![1_u8].into())
        }
    }

    struct FailingValue;

    impl BatchValue for FailingValue {
        fn encode(
            self: Box<Self>,
            _timestamp: Timestamp,
            _encoder: &mut BatchEncoder,
        ) -> Result<fjall::Slice, TxError> {
            Err(TxError::EncodingFailure)
        }
    }

    fn test_writer() -> (tempfile::TempDir, fjall::Database, BatchWriter) {
        let tempdir = tempfile::tempdir().unwrap();
        let database = fjall::Database::builder(tempdir.path()).open().unwrap();
        let writer = BatchWriter::new(database.clone());
        (tempdir, database, writer)
    }

    #[test]
    fn closed_admission_rejects_new_waits() {
        let gate = Arc::new(CommitAdmissionGate::new(
            1,
            test_policy(),
            Arc::new(AtomicBool::new(true)),
        ));
        let held = gate.acquire(Timestamp(1)).unwrap();
        let waiter_gate = gate.clone();
        let waiter = std::thread::spawn(move || waiter_gate.acquire(Timestamp(2)));

        gate.close();

        let result = waiter.join().unwrap();
        assert!(matches!(result, Err(CommitAdmissionError::Unavailable)));
        drop(held);
        assert!(matches!(
            gate.acquire(Timestamp(3)),
            Err(CommitAdmissionError::Unavailable)
        ));
    }

    /// The permit must not return to the pool while the commit is encoding or applying.
    #[test]
    fn admission_permit_is_held_until_application() {
        let (_tempdir, database, writer) = test_writer();
        let partition = database
            .keyspace("values", KeyspaceCreateOptions::default)
            .unwrap();
        let gate = Arc::new(CommitAdmissionGate::new(
            1,
            test_policy(),
            Arc::new(AtomicBool::new(true)),
        ));
        let permit = gate.acquire(Timestamp(1)).unwrap();

        let (started_tx, started_rx) = flume::bounded(1);
        let (release_tx, release_rx) = flume::bounded(1);
        let mut batch = CommitBatch::with_capacity(1, Timestamp(1), 1);
        batch.insert(
            partition.clone(),
            b"key".to_vec(),
            Box::new(BlockingValue {
                started: started_tx,
                release: release_rx,
            }),
        );
        writer
            .try_send_request(EncodeRequest::PreBuilt(batch, Vec::new()), Some(permit))
            .unwrap();

        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("encoder should pick up the batch");
        let blocked = gate.acquire(Timestamp(2));
        assert!(matches!(blocked, Err(CommitAdmissionError::Timeout { .. })));

        release_tx.send(()).unwrap();
        writer.wait_applied(1, Duration::from_secs(2)).unwrap();

        assert!(gate.acquire(Timestamp(3)).is_ok());
    }

    #[test]
    fn stale_epochs_and_unpublished_targets_are_rejected() {
        let (_tempdir, _database, writer) = test_writer();
        let coordinator = PersistenceCoordinator::new(WriterEpoch::random(), test_config(), writer);

        let stale = coordinator.wait_applied(
            PublicationId::new(WriterEpoch::random(), 1),
            Duration::from_millis(10),
        );
        assert!(matches!(
            stale,
            Err(PersistenceError::StalePublication { .. })
        ));

        let unpublished = coordinator.wait_applied(
            PublicationId::new(coordinator.epoch(), 7),
            Duration::from_millis(10),
        );
        assert!(matches!(
            unpublished,
            Err(PersistenceError::UnpublishedPublication { version: 7 })
        ));

        let initial = coordinator
            .wait_applied(PublicationId::new(coordinator.epoch(), 0), Duration::ZERO)
            .unwrap();
        assert_eq!(initial.publication.version(), 0);
    }

    #[test]
    fn applied_and_durable_receipts_prove_the_requested_prefix() {
        let (_tempdir, database, writer) = test_writer();
        let partition = database
            .keyspace("values", KeyspaceCreateOptions::default)
            .unwrap();
        let coordinator = PersistenceCoordinator::new(WriterEpoch::random(), test_config(), writer);
        coordinator.published(1);
        coordinator.last_submitted.store(1, Ordering::Release);

        let mut batch = CommitBatch::with_capacity(1, Timestamp(1), 1);
        batch.insert_encoded(partition.clone(), b"key".to_vec(), b"value".to_vec());
        coordinator
            .writer
            .try_send_request(EncodeRequest::PreBuilt(batch, Vec::new()), None)
            .unwrap();

        let publication = PublicationId::new(coordinator.epoch(), 1);
        let applied = coordinator
            .wait_applied(publication, Duration::from_secs(2))
            .unwrap();
        assert_eq!(applied.publication, publication);
        let durable = coordinator
            .wait_durable(publication, Duration::from_secs(2))
            .unwrap();
        assert_eq!(durable.publication, publication);
        assert!(coordinator.status().durable >= 1);
    }

    #[test]
    fn terminal_writer_failure_closes_admission() {
        let (_tempdir, database, writer) = test_writer();
        let partition = database
            .keyspace("values", KeyspaceCreateOptions::default)
            .unwrap();
        let coordinator = PersistenceCoordinator::new(WriterEpoch::random(), test_config(), writer);
        coordinator.published(1);
        coordinator.last_submitted.store(1, Ordering::Release);

        let mut batch = CommitBatch::with_capacity(1, Timestamp(1), 1);
        batch.insert(partition.clone(), b"key".to_vec(), Box::new(FailingValue));
        coordinator
            .writer
            .try_send_request(EncodeRequest::PreBuilt(batch, Vec::new()), None)
            .unwrap();

        let start = moor_common::util::Instant::now();
        while coordinator.writer.healthy() && start.elapsed() < Duration::from_secs(2) {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(!coordinator.writer.healthy());
        assert!(matches!(
            coordinator.admit(Timestamp(2)),
            Err(CommitAdmissionError::Unavailable)
        ));

        let error = coordinator
            .wait_applied(
                PublicationId::new(coordinator.epoch(), 1),
                Duration::from_millis(50),
            )
            .unwrap_err();
        assert!(matches!(error, PersistenceError::WriterFailed { .. }));
    }

    #[test]
    fn stale_initial_publication_is_rejected_for_both_receipts() {
        let (_tempdir, _database, writer) = test_writer();
        let coordinator = PersistenceCoordinator::new(WriterEpoch::random(), test_config(), writer);
        let stale = PublicationId::new(WriterEpoch::random(), 0);
        assert!(matches!(
            coordinator.wait_applied(stale, Duration::ZERO),
            Err(PersistenceError::StalePublication { .. })
        ));
        assert!(matches!(
            coordinator.wait_durable(stale, Duration::ZERO),
            Err(PersistenceError::StalePublication { .. })
        ));
    }

    #[test]
    fn failure_wakes_blocked_admission_before_credit_reuse() {
        let healthy = Arc::new(AtomicBool::new(true));
        let gate = Arc::new(CommitAdmissionGate::new(
            1,
            AdmissionPolicy {
                warn_after: Duration::from_secs(60),
                timeout: Duration::from_secs(120),
            },
            healthy,
        ));
        let held = gate.acquire(Timestamp(1)).unwrap();
        let blocked_gate = gate.clone();
        let (result_tx, result_rx) = flume::bounded(1);
        let waiter = std::thread::spawn(move || {
            result_tx
                .send(blocked_gate.acquire(Timestamp(2)).map(|_| ()))
                .unwrap();
        });
        held.fail();
        drop(held);
        assert!(matches!(
            result_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
            Err(CommitAdmissionError::Unavailable)
        ));
        waiter.join().unwrap();
        assert!(matches!(
            gate.acquire(Timestamp(3)),
            Err(CommitAdmissionError::Unavailable)
        ));
    }

    #[test]
    fn abandoned_reservation_allows_shutdown_to_finish() {
        let (_tempdir, _database, writer) = test_writer();
        let coordinator = Arc::new(PersistenceCoordinator::new(
            WriterEpoch::random(),
            test_config(),
            writer,
        ));
        let permit = coordinator.admit(Timestamp(1)).unwrap();
        let draining = coordinator.clone();
        let shutdown = std::thread::spawn(move || draining.shutdown());
        let started = std::time::Instant::now();
        while !coordinator.status().shutdown {
            assert!(started.elapsed() < Duration::from_secs(1));
            std::thread::yield_now();
        }
        assert!(!shutdown.is_finished());
        drop(permit);
        shutdown.join().unwrap().unwrap();
        assert_eq!(coordinator.status().last_submitted, 0);
    }

    #[test]
    fn shutdown_deadline_wakes_unbounded_waiter_with_live_reservation() {
        let (_tempdir, _database, writer) = test_writer();
        let coordinator = Arc::new(PersistenceCoordinator::new(
            WriterEpoch::random(),
            PersistenceConfig {
                shutdown_timeout: Duration::from_millis(20),
                ..test_config()
            },
            writer,
        ));
        let permit = coordinator.admit(Timestamp(1)).unwrap();
        let waiting = coordinator.clone();
        let (result_tx, result_rx) = flume::bounded(1);
        let waiter = std::thread::spawn(move || {
            result_tx
                .send(waiting.wait_applied_unbounded(PublicationId::new(waiting.epoch(), 1)))
                .unwrap();
        });
        let started = std::time::Instant::now();
        assert!(coordinator.shutdown().is_err());
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(
            result_rx
                .recv_timeout(Duration::from_secs(1))
                .unwrap()
                .is_err()
        );
        drop(permit);
        waiter.join().unwrap();
    }

    #[test]
    fn encoded_out_of_order_commit_keeps_its_admission_credit() {
        let (_tempdir, database, writer) = test_writer();
        let partition = database
            .keyspace("values", KeyspaceCreateOptions::default)
            .unwrap();
        let gate = Arc::new(CommitAdmissionGate::new(
            1,
            test_policy(),
            writer.health_flag(),
        ));
        let permit = gate.acquire(Timestamp(2)).unwrap();
        let mut later = CommitBatch::with_capacity(2, Timestamp(2), 1);
        later.insert_encoded(partition.clone(), b"key".to_vec(), b"second".to_vec());
        writer
            .try_send_request(EncodeRequest::PreBuilt(later, Vec::new()), Some(permit))
            .unwrap();
        assert!(matches!(
            gate.acquire(Timestamp(3)),
            Err(CommitAdmissionError::Timeout { .. })
        ));
        assert_eq!(writer.completed_version(), 0);
        let mut first = CommitBatch::with_capacity(1, Timestamp(1), 1);
        first.insert_encoded(partition.clone(), b"key".to_vec(), b"first".to_vec());
        writer.write_batch_for_test(first).unwrap();
        writer.wait_applied(2, Duration::from_secs(1)).unwrap();
        assert!(gate.acquire(Timestamp(3)).is_ok());
        assert_eq!(partition.get(b"key").unwrap().unwrap().as_ref(), b"second");
    }

    #[test]
    fn applied_wait_timeout_does_not_cancel_the_commit() {
        let (_tempdir, database, writer) = test_writer();
        let partition = database
            .keyspace("values", KeyspaceCreateOptions::default)
            .unwrap();
        let gate = Arc::new(CommitAdmissionGate::new(
            1,
            test_policy(),
            writer.health_flag(),
        ));
        let permit = gate.acquire(Timestamp(1)).unwrap();
        let (started_tx, started_rx) = flume::bounded(1);
        let (release_tx, release_rx) = flume::bounded(1);
        let mut batch = CommitBatch::with_capacity(1, Timestamp(1), 1);
        batch.insert(
            partition.clone(),
            b"key".to_vec(),
            Box::new(BlockingValue {
                started: started_tx,
                release: release_rx,
            }),
        );
        writer
            .try_send_request(EncodeRequest::PreBuilt(batch, Vec::new()), Some(permit))
            .unwrap();
        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(matches!(
            writer.wait_applied(1, Duration::from_millis(10)),
            Err(WriterWaitError::Timeout { .. })
        ));
        assert!(gate.available.is_empty());
        release_tx.send(()).unwrap();
        writer.wait_applied(1, Duration::from_secs(1)).unwrap();
        assert_eq!(partition.get(b"key").unwrap().unwrap().as_ref(), &[1]);
        assert!(gate.acquire(Timestamp(2)).is_ok());
    }

    #[test]
    fn snapshot_deadline_bounds_the_receipt_wait() {
        let (_tempdir, database, writer) = test_writer();
        let partition = database
            .keyspace("values", KeyspaceCreateOptions::default)
            .unwrap();
        let (started_tx, started_rx) = flume::bounded(1);
        let (release_tx, release_rx) = flume::bounded(1);
        let mut batch = CommitBatch::with_capacity(1, Timestamp(1), 1);
        batch.insert(
            partition.clone(),
            b"key".to_vec(),
            Box::new(BlockingValue {
                started: started_tx,
                release: release_rx,
            }),
        );
        writer
            .try_send_request(EncodeRequest::PreBuilt(batch, Vec::new()), None)
            .unwrap();
        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("encoder should pick up the batch");

        let error = match writer.snapshot(1, Duration::from_millis(10)) {
            Ok(_) => panic!("snapshot must not succeed while the requested version is unapplied"),
            Err(error) => error,
        };
        assert!(matches!(error, WriterWaitError::Timeout { version: 1 }));

        release_tx.send(()).unwrap();
        writer.wait_applied(1, Duration::from_secs(2)).unwrap();
        writer.snapshot(1, Duration::from_secs(1)).unwrap();
    }
}
