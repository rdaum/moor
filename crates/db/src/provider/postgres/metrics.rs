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

//! Per-writer numeric diagnostics and payload leases released on every exit path.
use crate::PostgresPersistenceStats;
use parking_lot::Mutex;
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};

#[derive(Default)]
pub(super) struct Metrics(Mutex<State>);
#[derive(Default)]
struct State {
    totals: PostgresPersistenceStats,
    next_id: u64,
    recovery_started: Option<Instant>,
    retained: BTreeMap<u64, Retained>,
}
struct Retained {
    published: Option<Instant>,
    encoded_bytes: u64,
    append_value_bytes: u64,
}
pub(super) struct PayloadLease {
    metrics: Arc<Metrics>,
    id: u64,
}
impl PayloadLease {
    pub fn published(&self) {
        if let Some(entry) = self.metrics.0.lock().retained.get_mut(&self.id) {
            entry.published = Some(Instant::now());
        }
    }
}
impl Drop for PayloadLease {
    fn drop(&mut self) {
        self.metrics.0.lock().retained.remove(&self.id);
    }
}
impl Metrics {
    pub fn update(&self, update: impl FnOnce(&mut PostgresPersistenceStats)) {
        update(&mut self.0.lock().totals);
    }
    pub fn retain(
        self: &Arc<Self>,
        encoded_bytes: usize,
        append_value_bytes: usize,
    ) -> PayloadLease {
        let mut state = self.0.lock();
        let id = state.next_id;
        state.next_id = state
            .next_id
            .checked_add(1)
            .expect("diagnostic lease identity exhausted");
        state.retained.insert(
            id,
            Retained {
                published: None,
                encoded_bytes: encoded_bytes as u64,
                append_value_bytes: append_value_bytes as u64,
            },
        );
        PayloadLease {
            metrics: self.clone(),
            id,
        }
    }
    pub fn snapshot(&self) -> PostgresPersistenceStats {
        let state = self.0.lock();
        let mut status = state.totals;
        let now = Instant::now();
        if let Some(started) = state.recovery_started {
            status.recovery_ns = status
                .recovery_ns
                .saturating_add(nanos(now.saturating_duration_since(started)));
        }
        for entry in state.retained.values() {
            status.retained_encoded_bytes += entry.encoded_bytes;
            status.retained_append_value_bytes += entry.append_value_bytes;
            if let Some(published) = entry.published {
                status.unapplied_commits += 1;
                status.oldest_unapplied_micros = status.oldest_unapplied_micros.max(
                    now.saturating_duration_since(published)
                        .as_micros()
                        .min(u64::MAX as u128) as u64,
                );
            } else {
                status.prepared_commits += 1;
            }
        }
        status
    }
}
pub(super) fn nanos(duration: Duration) -> u64 {
    duration.as_nanos().min(u64::MAX as u128) as u64
}

pub(super) struct StageTimer {
    metrics: Arc<Metrics>,
    stage: moor_common::model::WorldStateTimerOp,
    started: Instant,
}
impl Drop for StageTimer {
    fn drop(&mut self) {
        use moor_common::model::WorldStateTimerOp::*;
        let elapsed = self.started.elapsed();
        crate::db_counters()
            .timers_rare
            .record_elapsed(self.stage, elapsed);
        let mut state = self.metrics.0.lock();
        if matches!(self.stage, PostgresRecovery) {
            state.recovery_started = None;
        }
        let m = &mut state.totals;
        let total = match self.stage {
            PostgresEncode => &mut m.encoding_ns,
            PostgresApply => &mut m.sql_application_ns,
            PostgresCommit => &mut m.sql_commit_ns,
            PostgresFence => &mut m.fence_ns,
            PostgresRecovery => &mut m.recovery_ns,
            _ => unreachable!("not a PostgreSQL stage timer"),
        };
        *total = total.saturating_add(nanos(elapsed));
    }
}
impl Metrics {
    pub fn timer(self: &Arc<Self>, stage: moor_common::model::WorldStateTimerOp) -> StageTimer {
        let started = Instant::now();
        if matches!(
            stage,
            moor_common::model::WorldStateTimerOp::PostgresRecovery
        ) {
            self.0.lock().recovery_started = Some(started);
        }
        StageTimer {
            metrics: self.clone(),
            stage,
            started,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_recovery_time_is_visible_and_stops_when_recovery_ends() {
        let metrics = Arc::new(Metrics::default());
        let timer = metrics.timer(moor_common::model::WorldStateTimerOp::PostgresRecovery);
        std::thread::sleep(Duration::from_millis(1));
        let during = metrics.snapshot().recovery_ns;
        assert!(during > 0);
        drop(timer);
        let after = metrics.snapshot().recovery_ns;
        assert!(after >= during);
        assert_eq!(after, metrics.snapshot().recovery_ns);
    }

    #[test]
    fn dropped_and_abandoned_payloads_release_diagnostics() {
        let metrics = Arc::new(Metrics::default());
        let prepared = metrics.retain(10, 20);
        assert_eq!(metrics.snapshot().prepared_commits, 1);
        let published = metrics.retain(30, 40);
        published.published();
        let status = metrics.snapshot();
        assert_eq!(status.prepared_commits, 1);
        assert_eq!(status.unapplied_commits, 1);
        assert_eq!(status.retained_encoded_bytes, 40);
        assert_eq!(status.retained_append_value_bytes, 60);
        drop(prepared);
        drop(published);
        assert_eq!(metrics.snapshot().retained_encoded_bytes, 0);
        assert_eq!(metrics.snapshot().retained_append_value_bytes, 0);
        assert_eq!(metrics.snapshot().oldest_unapplied_micros, 0);
    }
}
