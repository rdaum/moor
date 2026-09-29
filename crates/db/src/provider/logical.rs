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

//! Backend-neutral logical commit and receipt types.
//!
//! A `LogicalCommit` describes one published world commit in domain terms. It carries no
//! storage keyspaces, encoded bytes, or backend handles: backends consume it on their own
//! encoding workers. `PublicationId` names published roots within a single engine lifetime;
//! it is not a tuple timestamp and is not portable across restarts.

use crate::engine::moor_db::RelationChanges;
use crate::engine::property_definitions::PropertyDefinitionChange;
use crate::tx::{OpType, Timestamp, WorkingSet};
use crate::{ObjAndUUIDHolder, db_counters};
use moor_common::model::{WorldStateCountOp, WorldStateTimerOp};
use moor_var::{List, Var};

/// Identifies one database-engine lifetime.
///
/// Every open generates a fresh epoch. Retained publications from an earlier open are stale,
/// even when their numeric version has been reached again in this lifetime.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct WriterEpoch(u64);

impl WriterEpoch {
    /// Generate a fresh epoch for a database open.
    #[must_use]
    pub fn random() -> Self {
        Self(rand::random())
    }

    #[must_use]
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

/// A published world root within one engine lifetime.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct PublicationId {
    epoch: WriterEpoch,
    version: u64,
}

impl PublicationId {
    #[must_use]
    pub fn new(epoch: WriterEpoch, version: u64) -> Self {
        Self { epoch, version }
    }

    #[must_use]
    pub fn epoch(self) -> WriterEpoch {
        self.epoch
    }

    #[must_use]
    pub fn version(self) -> u64 {
        self.version
    }
}

/// One sequence-slot high-water observation captured after publication.
///
/// These are allocation observations, not transaction-local writes, and the writer applies
/// them monotonically.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SequenceUpdate {
    pub slot: usize,
    pub value: i64,
}

/// A published commit in backend-neutral, typed form.
pub struct LogicalCommit {
    pub publication: PublicationId,
    pub timestamp: Timestamp,
    pub changes: RelationChanges,
    pub sequences: Vec<SequenceUpdate>,
    pub property_definition_changes: Vec<PropertyDefinitionChange>,
}

/// Proof that a storage prefix through a publication is applied or durable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PersistenceReceipt {
    pub publication: PublicationId,
}

/// Typed persistence failures shared by storage backends and the coordinator.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PersistenceError {
    #[error("publication {publication:?} belongs to a stale writer epoch")]
    StalePublication { publication: PublicationId },

    #[error("publication version {version} has not been published by this engine")]
    UnpublishedPublication { version: u64 },

    #[error("timed out waiting for persistence through version {version}")]
    Timeout { version: u64 },

    #[error("persistence writer failed: {detail}")]
    WriterFailed { detail: String },

    #[error("persistence coordinator is shut down")]
    ShutDown,
}

const LIST_APPEND_COMPARISON_BUDGET: usize = 128;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PreparedPropertyValueOp {
    pub property: ObjAndUUIDHolder,
    pub mutation: PreparedPropertyValueMutation,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PreparedPropertyValueMutation {
    Replace { value: Var },
    AppendList { suffix: List, final_value: Var },
    Delete,
}

pub(crate) fn prepare_property_value_working_set(
    working_set: WorkingSet<ObjAndUUIDHolder, Var>,
) -> Vec<PreparedPropertyValueOp> {
    let operation_count = working_set.len();
    let (operations, base_index) = working_set.into_parts();
    let mut prepared = Vec::with_capacity(operation_count);

    for (property, operation) in operations {
        let base = base_index.index_lookup(&property).map(|entry| &entry.value);
        prepared.push(PreparedPropertyValueOp {
            property,
            mutation: prepare_property_value_mutation(base, operation.operation),
        });
    }
    prepared
}

pub(crate) fn prepare_property_value_mutation(
    base: Option<&Var>,
    operation: OpType<Var>,
) -> PreparedPropertyValueMutation {
    let value = match operation {
        OpType::Delete => return PreparedPropertyValueMutation::Delete,
        OpType::Insert(value) => {
            db_counters()
                .counters
                .inc(WorldStateCountOp::PropertyValueCompleteReplacement);
            return PreparedPropertyValueMutation::Replace { value };
        }
        OpType::Update(value) => value,
    };

    if value.op_hint() != moor_var::OP_HINT_LIST_APPEND {
        db_counters()
            .counters
            .inc(WorldStateCountOp::PropertyValueCompleteReplacement);
        return PreparedPropertyValueMutation::Replace { value };
    }

    let counters = &db_counters().counters;
    counters.inc(WorldStateCountOp::PropertyListAppendCandidate);
    let _classification_timer = db_counters()
        .timers_rare
        .start(WorldStateTimerOp::PropertyListAppendClassify);
    let Some(base) = base else {
        counters.inc(WorldStateCountOp::PropertyListAppendMissingBase);
        counters.inc(WorldStateCountOp::PropertyValueCompleteReplacement);
        return PreparedPropertyValueMutation::Replace { value };
    };
    let (Some(base), Some(final_value)) = (base.as_list(), value.as_list()) else {
        counters.inc(WorldStateCountOp::PropertyListAppendNonList);
        counters.inc(WorldStateCountOp::PropertyValueCompleteReplacement);
        return PreparedPropertyValueMutation::Replace { value };
    };

    let suffix = match base.append_suffix(final_value, LIST_APPEND_COMPARISON_BUDGET) {
        Ok(suffix) => suffix,
        Err(reason) => {
            let counter = match reason {
                moor_var::ListAppendError::NotLonger => {
                    WorldStateCountOp::PropertyListAppendNotLonger
                }
                moor_var::ListAppendError::PrefixMismatch => {
                    WorldStateCountOp::PropertyListAppendPrefixMismatch
                }
                moor_var::ListAppendError::ComparisonBudgetExceeded => {
                    WorldStateCountOp::PropertyListAppendComparisonBudget
                }
            };
            counters.inc(counter);
            counters.inc(WorldStateCountOp::PropertyValueCompleteReplacement);
            return PreparedPropertyValueMutation::Replace { value };
        }
    };

    let suffix_bytes = suffix
        .iter_ref()
        .map(moor_var::ByteSized::size_bytes)
        .sum::<usize>();
    counters.inc(WorldStateCountOp::PropertyListAppendAccepted);
    counters.add(
        WorldStateCountOp::PropertyListAppendSuffixElements,
        isize::try_from(suffix.len()).unwrap_or(isize::MAX),
    );
    counters.add(
        WorldStateCountOp::PropertyListAppendSuffixBytes,
        isize::try_from(suffix_bytes).unwrap_or(isize::MAX),
    );
    PreparedPropertyValueMutation::AppendList {
        suffix,
        final_value: value,
    }
}
