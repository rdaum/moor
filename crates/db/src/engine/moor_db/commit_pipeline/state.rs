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

//! Private constructors and consuming transitions for write commits.

use super::super::property_policy::property_can_clobber;
use super::super::snapshot_planes::SnapshotPlanes;
use super::super::{
    Caches, MoorDB, RelationCheckers, RelationWorkingSets, WorkingSets, WorldStateSnapshot,
};
use crate::api::world_state::db_counters;
use crate::engine::property_definitions::{
    PropertyDefinitionChange, collect_property_definition_changes,
};
use crate::engine::relation_defs::{RebaseCheck, define_relations};
use crate::provider::coordinator::{CommitAdmission, CommitAdmissionError};
use crate::provider::logical::{LogicalCommit, PublicationId, SequenceUpdate};
use crate::tx::{CommitBloom, Timestamp};
use moor_common::model::{
    CommitResult, ConflictInfo, ConflictTarget, WorldStateError, WorldStateTimerOp,
};
use moor_common::util::Instant;
use std::{sync::Arc, time::Duration};
use tracing::{error, trace, warn};

define_relation_commit_methods!();

/// The operations validated by preparation, retained unchanged across rebases.
struct CommitData<'db> {
    db: &'db MoorDB,
    working_sets: RelationWorkingSets,
    checkers: RelationCheckers,
    caches: Caches,
    bloom: CommitBloom,
    tx_timestamp: Timestamp,
    admission: CommitAdmission,
}

/// Owns a candidate built from `checked_root` and admitted for publication.
#[must_use]
pub(super) struct PreparedCommit<'db> {
    data: CommitData<'db>,
    checked_root: Arc<WorldStateSnapshot>,
    candidate: Arc<WorldStateSnapshot>,
    definition_changes: Vec<PropertyDefinitionChange>,
}

/// A lost CAS retains validation history, but no publishable candidate.
#[must_use]
pub(super) struct RebaseRequired<'db> {
    data: CommitData<'db>,
    checked_root: Arc<WorldStateSnapshot>,
}

/// Publication succeeded; this payload must be submitted to ordered persistence.
#[must_use]
pub(super) struct PublishedCommit<'db> {
    db: &'db MoorDB,
    working_sets: RelationWorkingSets,
    publication_version: u64,
    tx_timestamp: Timestamp,
    definition_changes: Vec<PropertyDefinitionChange>,
    admission: CommitAdmission,
}

pub(super) enum PrepareError {
    Conflict(ConflictInfo),
    Database(WorldStateError),
}

/// Only a prepared commit can construct this capability. The destination,
/// expected version, and candidate travel together to the atomic publication code.
pub(in crate::engine::moor_db) struct RootPublication<'db> {
    planes: &'db SnapshotPlanes,
    expected_version: u64,
    candidate: Arc<WorldStateSnapshot>,
}

impl<'db> RootPublication<'db> {
    pub(in crate::engine::moor_db) fn into_parts(
        self,
    ) -> (&'db SnapshotPlanes, u64, Arc<WorldStateSnapshot>) {
        (self.planes, self.expected_version, self.candidate)
    }
}

fn enrich_conflict_info(root: &WorldStateSnapshot, mut info: ConflictInfo) -> ConflictInfo {
    if let Some(ConflictTarget::Property { object, uuid, name }) = &mut info.target
        && name.is_none()
    {
        *name = root.property_name(*object, *uuid);
    }
    info
}

impl<'db> PreparedCommit<'db> {
    pub(super) fn prepare(db: &'db MoorDB, ws: Box<WorkingSets>) -> Result<Self, PrepareError> {
        let counters = db_counters();
        let num_tuples = ws.total_tuples();
        let tx_timestamp = ws.tx.ts;
        let snapshot_version = ws.tx.snapshot_version;
        let tx_bloom = ws.tx_bloom.clone();
        let (mut relation_ws, verb_cache, prop_cache, ancestry_cache) =
            ws.extract_relation_working_sets();

        let start_time = Instant::now();

        // Phase 1: Check conflicts and prepare indexes against current snapshot
        let current_root = db.snapshot_planes.load_root();
        if snapshot_version != current_root.version
            && let Err(info) = relation_ws.check_property_policies(&current_root)
        {
            return Err(PrepareError::Conflict(enrich_conflict_info(
                &current_root,
                info,
            )));
        }
        relation_ws.clear_clobber_hints();
        let mut checkers = db.relations.begin_check_all(&current_root, &relation_ws);

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
            let _t = counters
                .timers_hot
                .start(WorldStateTimerOp::CommitCheckPhase);
            if let Err(conflict_info) = checkers.check_all(&mut relation_ws) {
                let conflict_info = enrich_conflict_info(&current_root, conflict_info);
                trace!("Transaction conflict during commit: {conflict_info}");
                return Err(PrepareError::Conflict(conflict_info));
            }
        }

        if start_time.elapsed() > Duration::from_secs(5) {
            warn!(
                "Long running commit; check phase took {}s for {num_tuples} tuples",
                start_time.elapsed().as_secs_f32()
            );
        }

        let _t = counters
            .timers_hot
            .start(WorldStateTimerOp::CommitApplyPhase);
        let bloom = checkers.prepare_apply_all(&relation_ws);
        let combined_caches = Caches {
            verb_resolution_cache: verb_cache.fork(),
            prop_resolution_cache: prop_cache.fork(),
            ancestry_cache: ancestry_cache.fork(),
        };
        let property_definition_changes = collect_property_definition_changes(
            &*current_root.object_propdefs,
            &relation_ws.object_propdefs,
        );
        let next_root =
            checkers.build_snapshot(&current_root, tx_timestamp, combined_caches, bloom.clone());
        drop(_t);

        let admission = db
            .coordinator
            .admit(tx_timestamp)
            .map_err(|error| match error {
                CommitAdmissionError::Timeout { waited } => {
                    PrepareError::Database(WorldStateError::DatabaseOverloaded(waited))
                }
                CommitAdmissionError::Unavailable => {
                    PrepareError::Database(WorldStateError::DatabaseError(
                        "Database commit queue admission is unavailable".to_string(),
                    ))
                }
            })?;

        Ok(Self {
            data: CommitData {
                db,
                working_sets: relation_ws,
                checkers,
                caches: Caches {
                    verb_resolution_cache: verb_cache,
                    prop_resolution_cache: prop_cache,
                    ancestry_cache,
                },
                bloom,
                tx_timestamp,
                admission,
            },
            checked_root: current_root,
            candidate: next_root,
            definition_changes: property_definition_changes,
        })
    }

    // Return the owned working set on CAS loss without allocating a retry box.
    #[allow(clippy::result_large_err)]
    pub(super) fn try_publish(self) -> Result<PublishedCommit<'db>, RebaseRequired<'db>> {
        let Self {
            data,
            checked_root,
            candidate,
            definition_changes,
        } = self;
        let publication_version = candidate.version;
        if SnapshotPlanes::try_publish_write_root(RootPublication {
            planes: &data.db.snapshot_planes,
            expected_version: checked_root.version,
            candidate,
        }) {
            Ok(PublishedCommit {
                db: data.db,
                working_sets: data.working_sets,
                publication_version,
                tx_timestamp: data.tx_timestamp,
                definition_changes,
                admission: data.admission,
            })
        } else {
            Err(RebaseRequired { data, checked_root })
        }
    }
}

impl<'db> RebaseRequired<'db> {
    pub(super) fn rebase(self) -> Result<PreparedCommit<'db>, ConflictInfo> {
        let Self { data, checked_root } = self;
        let winner = data.db.snapshot_planes.load_root();
        if let RebaseCheck::ActualOverlap(info) =
            data.checkers
                .rebase_check(&data.working_sets, &checked_root, &winner)
        {
            let info = enrich_conflict_info(&winner, info);
            trace!(checked_version = checked_root.version, winner_version = winner.version,
                %info, "Transaction found an exact key overlap after CAS loss");
            return Err(info);
        }
        let definition_changes = collect_property_definition_changes(
            &*winner.object_propdefs,
            &data.working_sets.object_propdefs,
        );
        let candidate = data.checkers.build_rebased_snapshot(
            &data.working_sets,
            &winner,
            data.tx_timestamp,
            data.caches.fork(),
            &data.bloom,
        );
        Ok(PreparedCommit {
            data,
            checked_root: winner,
            candidate,
            definition_changes,
        })
    }
}

impl PublishedCommit<'_> {
    pub(super) fn enqueue_persistence(self) -> CommitResult {
        let Self {
            db,
            working_sets,
            publication_version,
            tx_timestamp,
            definition_changes,
            admission,
        } = self;
        let result = CommitResult::Success {
            mutations_made: true,
            timestamp: tx_timestamp.0,
        };
        let dirty_sequences = db.sequences.claim_dirty();
        let mut sequences = Vec::new();
        for slot in 0_usize..super::super::SEQUENCE_COUNT {
            if dirty_sequences & (1_u16 << slot) != 0 {
                sequences.push(SequenceUpdate {
                    slot,
                    value: db.sequences.load(slot),
                });
            }
        }
        let commit = LogicalCommit {
            publication: PublicationId::new(db.coordinator.epoch(), publication_version),
            timestamp: tx_timestamp,
            changes: working_sets.into_changes(),
            sequences,
            property_definition_changes: definition_changes,
        };
        if let Err(error) = db.coordinator.submit(commit, admission) {
            report_persistence_failure(&format!(
                "failed to submit transaction {publication_version}: {error}"
            ));
        }
        result
    }
}

fn report_persistence_failure(detail: &str) {
    error!("FATAL: {detail}");
    #[cfg(not(test))]
    moor_common::util::signal_fatal_db_error("transaction persistence", detail);
}

#[cfg(test)]
mod tests;
