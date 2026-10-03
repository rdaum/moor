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

//! Frozen SQL inputs and tentative bookkeeping for one atomic publication range.
use super::{
    PostgresError,
    codec::invalid,
    encode::{EncodedCommit, PropertyMutation, RelationBatch},
    rows::{self, RowKey},
    seed::Chains,
    state::Progress,
};
use crate::{
    ObjAndUUIDHolder, Timestamp,
    provider::property_value_store::{PROPERTY_VALUE_CHAIN_LIMITS, PropertyValueChain},
};
use moor_var::Var;
use serde_json::{Value, json};

type ChainChanges = ahash::AHashMap<ObjAndUUIDHolder, Option<PropertyValueChain>>;
struct PropertyPlan {
    batch: RelationBatch,
    changes: Vec<(ObjAndUUIDHolder, Option<PropertyValueChain>)>,
}

/// Built once before SQL begins. Execution and recovery borrow this plan without mutation.
/// The caller retains commit payloads and admission permits until execution confirms this plan.
pub(super) struct TransactionPlan<'a> {
    pub before: Progress,
    pub after: Progress,
    pub commits: &'a [EncodedCommit],
    pub properties: Vec<RelationBatch>,
    pub changes: ChainChanges,
    pub bytes: usize,
    pub force_sync: bool,
}

impl<'a> TransactionPlan<'a> {
    pub fn group(
        before: &Progress,
        chains: &Chains,
        commits: &'a [EncodedCommit],
        max_bytes: usize,
        mut rollup: impl FnMut(&ObjAndUUIDHolder, Var, Timestamp) -> Result<Value, PostgresError>,
    ) -> Result<Self, PostgresError> {
        let mut after = before.clone();
        let mut changes = ChainChanges::default();
        let mut properties = Vec::with_capacity(commits.len());
        let mut bytes = 0usize;
        for commit in commits {
            let before = after.clone();
            if commit.publication.epoch().as_u64() != after.epoch {
                return Err(PostgresError::OwnershipLost);
            }
            after.applied = after
                .applied
                .checked_add(1)
                .ok_or_else(|| invalid("applied_version", "counter exhausted"))?;
            if after.applied != commit.publication.version() {
                return Err(invalid("applied_version", "nonconsecutive publication"));
            }
            after.commits = after
                .commits
                .checked_add(1)
                .ok_or_else(|| invalid("commit_sequence", "counter exhausted"))?;
            after.max_timestamp = after.max_timestamp.max(commit.timestamp.0);
            if !commit.properties.is_empty() {
                after.property_sequence = after
                    .property_sequence
                    .checked_add(1)
                    .ok_or_else(|| invalid("property_record_sequence", "counter exhausted"))?;
            }
            let plan = plan_properties(
                chains,
                commit,
                after.property_sequence,
                &changes,
                &mut rollup,
            )?;
            let member_bytes = commit
                .ordinary
                .iter()
                .map(RelationBatch::encoded_bytes)
                .sum::<usize>()
                + plan.batch.encoded_bytes()
                + commit.sequences.as_ref().map_or(0, String::len);
            // A rollup can expand a small suffix into a large full value. Seal before
            // that member; one indivisible commit may exceed the payload limit.
            if !properties.is_empty() && bytes.saturating_add(member_bytes) > max_bytes {
                after = before;
                break;
            }
            bytes = bytes.saturating_add(member_bytes);
            changes.extend(plan.changes);
            properties.push(plan.batch);
            if bytes >= max_bytes {
                break;
            }
        }
        Ok(Self {
            before: before.clone(),
            after,
            commits: &commits[..properties.len()],
            properties,
            changes,
            bytes,
            force_sync: false,
        })
    }

    pub fn fence(before: &Progress) -> Result<Self, PostgresError> {
        let mut after = before.clone();
        after.durable_fence = after
            .durable_fence
            .checked_add(1)
            .ok_or_else(|| invalid("durable_fence", "counter exhausted"))?;
        Ok(Self {
            before: before.clone(),
            after,
            commits: &[],
            properties: Vec::new(),
            changes: ChainChanges::default(),
            bytes: 0,
            force_sync: true,
        })
    }

    pub fn statements(&self) -> u64 {
        self.commits
            .iter()
            .zip(&self.properties)
            .map(|(commit, properties)| {
                commit
                    .ordinary
                    .iter()
                    .chain(std::iter::once(properties))
                    .map(|batch| {
                        u64::from(batch.puts.is_some()) + u64::from(batch.deletes.is_some())
                    })
                    .sum::<u64>()
                    + u64::from(commit.sequences.is_some())
            })
            .sum()
    }
}

fn plan_properties(
    chains: &Chains,
    commit: &EncodedCommit,
    sequence: i64,
    overlay: &ChainChanges,
    rollup: &mut impl FnMut(&ObjAndUUIDHolder, Var, Timestamp) -> Result<Value, PostgresError>,
) -> Result<PropertyPlan, PostgresError> {
    let mut puts = Vec::new();
    let mut deletes = Vec::new();
    let mut changes = Vec::with_capacity(commit.properties.len());
    for property in &commit.properties {
        let key = &property.key;
        let (mut row, chain, replace) = match &property.mutation {
            PropertyMutation::Delete => {
                deletes.push(Value::Object(key.encode_key("object_propvalues")));
                changes.push((key.clone(), None));
                continue;
            }
            PropertyMutation::Full(row) => {
                (row.clone(), PropertyValueChain::full(sequence as u64), true)
            }
            PropertyMutation::Append { row, final_value } => {
                let bytes = rows::text(
                    row.as_object()
                        .ok_or_else(|| invalid("row", "invalid property row"))?,
                    "value_literal",
                )?
                .len();
                let previous = match overlay.get(key) {
                    Some(chain) => chain.as_ref(),
                    None => chains.get(key),
                };
                if previous
                    .is_none_or(|chain| chain.reaches_limit(bytes, PROPERTY_VALUE_CHAIN_LIMITS))
                {
                    (
                        rollup(key, final_value.clone(), commit.timestamp)?,
                        PropertyValueChain::full(sequence as u64),
                        true,
                    )
                } else {
                    let mut chain = previous.unwrap().clone();
                    chain.push_append(sequence as u64, bytes);
                    (row.clone(), chain, false)
                }
            }
        };
        row["record_sequence"] = json!(sequence);
        puts.push(row);
        if replace {
            deletes.push(Value::Object(key.encode_key("object_propvalues")));
        }
        changes.push((key.clone(), Some(chain)));
    }
    Ok(PropertyPlan {
        batch: RelationBatch {
            relation: "object_propvalues",
            puts: array(puts),
            deletes: array(deletes),
        },
        changes,
    })
}

fn array(rows: Vec<Value>) -> Option<String> {
    (!rows.is_empty()).then(|| Value::Array(rows).to_string())
}
