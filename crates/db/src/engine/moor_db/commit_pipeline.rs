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

//! Write and read-only commit execution pipeline for `MoorDB`.
//!
//! Uses a lock-free CAS loop for write commits: multiple workers can check
//! conflicts and build candidate snapshots in parallel. Only the final atomic
//! publish (via `ArcSwap::rcu`) serializes.
//!
//! On CAS failure, we first attempt a cheap rebase: if the winner only modified
//! different relations than us, we can re-slot our prepared indexes onto the
//! winner's snapshot and CAS again without re-checking or re-preparing. Only if
//! both we and the winner touched the same relation do we fall back to a full
//! re-check cycle.

use super::{Caches, MoorDB, WorkingSets};
use crate::api::world_state::db_counters;
use moor_common::model::CommitResult;
use moor_common::util::Instant;
use moor_common::util::PerfTimerGuard;
use std::time::Duration;
use tracing::warn;

/// Maximum number of rebase attempts after the initial CAS before giving up.
const MAX_REBASE_ATTEMPTS: u32 = 16;

impl MoorDB {
    /// Publish read-only cache updates for the transaction snapshot version.
    pub(crate) fn commit_read_only(&self, snapshot_version: u64, combined_caches: Caches) {
        self.snapshot_planes
            .publish_read_only_cache(snapshot_version, combined_caches);
    }

    /// Persist a successfully published snapshot to the durable store.
    fn persist_commit(
        &self,
        persist_ops: &super::RelationPersistOps,
        tx_timestamp: crate::tx::Timestamp,
    ) {
        let batch = self
            .relations
            .persist_ops_to_batch(persist_ops, tx_timestamp);
        if !batch.is_empty() {
            self.batch_writer.write(batch);
        }

        self.last_write_commit
            .store(tx_timestamp.0, std::sync::atomic::Ordering::Release);
        self.batch_writer.send_barrier(tx_timestamp);

        self.sequences[15].store(
            self.monotonic.load(std::sync::atomic::Ordering::Relaxed) as i64,
            std::sync::atomic::Ordering::Relaxed,
        );
        let mut seq_values = [0i64; 16];
        for (i, seq) in self.sequences.iter().enumerate() {
            seq_values[i] = seq.load(std::sync::atomic::Ordering::Relaxed);
        }
        self.sequence_writer.write(seq_values);
    }

    /// Execute the write-commit path for a transaction via CAS loop.
    pub(crate) fn commit_writes(
        &self,
        ws: Box<WorkingSets>,
        _enqueued_at: Instant,
    ) -> CommitResult {
        let counters = db_counters();
        let _process_timer = PerfTimerGuard::new(&counters.commit_process_phase);

        let num_tuples = ws.total_tuples();
        if num_tuples > 10_000 {
            warn!("Potential large batch @ commit... {num_tuples} total tuples in working set");
        }

        let tx_timestamp = ws.tx.ts;
        let snapshot_version = ws.tx.snapshot_version;
        let has_mutations = ws.has_mutations;
        let tx_bloom = ws.tx_bloom.clone();
        let (mut relation_ws, verb_cache, prop_cache, ancestry_cache) =
            ws.extract_relation_working_sets();

        // Read-only fast path
        if !has_mutations {
            self.commit_read_only(
                snapshot_version,
                Caches {
                    verb_resolution_cache: verb_cache,
                    prop_resolution_cache: prop_cache,
                    ancestry_cache,
                },
            );
            return CommitResult::Success {
                mutations_made: false,
                timestamp: tx_timestamp.0,
            };
        }

        let start_time = Instant::now();

        // Phase 1: Check conflicts and prepare indexes against current snapshot
        let current_root = self.snapshot_planes.load_root();
        let mut checkers = self.relations.begin_check_all(&current_root);

        // Skip conflict check if:
        // - No commits since our snapshot (existing fast path), OR
        // - The snapshot's cumulative bloom filter covers all commits since
        //   our snapshot, and our keys don't intersect it
        let skip_conflict_check = snapshot_version == current_root.version
            || (snapshot_version >= current_root.bloom_since_version
                && current_root
                    .commit_bloom
                    .as_ref()
                    .is_some_and(|snap_bloom| !tx_bloom.might_intersect(snap_bloom)));

        if !skip_conflict_check {
            let _t = PerfTimerGuard::new(&counters.commit_check_phase);
            if let Err(conflict_info) = checkers.check_all(&mut relation_ws) {
                warn!("Transaction conflict during commit: {conflict_info}");
                return CommitResult::ConflictRetry {
                    conflict_info: Some(conflict_info),
                };
            }
        }

        if start_time.elapsed() > Duration::from_secs(5) {
            warn!(
                "Long running commit; check phase took {}s for {num_tuples} tuples",
                start_time.elapsed().as_secs_f32()
            );
        }

        let _t = PerfTimerGuard::new(&counters.commit_apply_phase);
        let (persist_ops, bloom) = checkers.prepare_apply_all(&relation_ws);
        let combined_caches = Caches {
            verb_resolution_cache: verb_cache.fork(),
            prop_resolution_cache: prop_cache.fork(),
            ancestry_cache: ancestry_cache.fork(),
        };
        let next_root =
            checkers.build_snapshot(&current_root, tx_timestamp, combined_caches, bloom.clone());
        drop(_t);

        // Phase 2: Try to publish
        if self
            .snapshot_planes
            .try_publish_write_root(current_root.version, next_root)
        {
            self.persist_commit(&persist_ops, tx_timestamp);
            return CommitResult::Success {
                mutations_made: true,
                timestamp: tx_timestamp.0,
            };
        }

        // Phase 3: CAS failed — try to rebase onto the winner's snapshot.
        // Test our keys against the winner's bloom filter for key-level
        // conflict detection. If no hits, rebase is safe.
        for _rebase in 0..MAX_REBASE_ATTEMPTS {
            let winner = self.snapshot_planes.load_root();

            let combined_caches = Caches {
                verb_resolution_cache: verb_cache.fork(),
                prop_resolution_cache: prop_cache.fork(),
                ancestry_cache: ancestry_cache.fork(),
            };

            if let Some(rebased) =
                checkers.try_rebase(
                    &relation_ws,
                    current_root.version,
                    &winner,
                    tx_timestamp,
                    combined_caches,
                    &bloom,
                )
            {
                // Rebase succeeded — no key overlap. Try CAS again.
                if self
                    .snapshot_planes
                    .try_publish_write_root(winner.version, rebased)
                {
                    self.persist_commit(&persist_ops, tx_timestamp);
                    return CommitResult::Success {
                        mutations_made: true,
                        timestamp: tx_timestamp.0,
                    };
                }
                // CAS failed again — another writer snuck in. Loop to rebase
                // against the new winner.
                continue;
            }

            // Bloom filter hit: possible key overlap. Fall back to full retry.
            break;
        }

        CommitResult::ConflictRetry {
            conflict_info: None,
        }
    }
}
