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
    batch: Vec<RelationBatch>,
    changes: Vec<(ObjAndUUIDHolder, Option<PropertyValueChain>)>,
}

/// Built once before SQL begins. Execution and recovery borrow this plan without mutation.
/// The caller retains commit payloads and admission permits until execution confirms this plan.
pub(super) struct TransactionPlan<'a> {
    pub before: Progress,
    pub after: Progress,
    pub commits: &'a [EncodedCommit],
    pub properties: Vec<Vec<RelationBatch>>,
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
                + plan
                    .batch
                    .iter()
                    .map(RelationBatch::encoded_bytes)
                    .sum::<usize>()
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
                    .chain(properties.iter())
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
        let previous = match overlay.get(key) {
            Some(chain) => chain.as_ref(),
            None => chains.get(key),
        };
        let deletion = || {
            let mut row = key.encode_key("object_propvalues");
            row.insert(
                "record_sequence".into(),
                json!(previous.map_or(0, |chain| {
                    chain
                        .record_versions()
                        .next()
                        .expect("property chains have a full record")
                })),
            );
            Value::Object(row)
        };
        let (mut row, chain, replace) = match &property.mutation {
            PropertyMutation::Delete => {
                deletes.push(deletion());
                changes.push((key.clone(), None));
                continue;
            }
            PropertyMutation::Full(row) => {
                (row.clone(), PropertyValueChain::full(sequence as u64), true)
            }
            PropertyMutation::Append {
                row, final_value, ..
            } => {
                let bytes = rows::text(
                    row.as_object()
                        .ok_or_else(|| invalid("row", "invalid property row"))?,
                    "value_literal",
                )?
                .len();
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
            deletes.push(deletion());
        }
        changes.push((key.clone(), Some(chain)));
    }
    // Bound the SQL TID array: each valid key has at most 64 physical records.
    // An indivisible commit can have many keys, so split only its delete statements.
    let mut batch: Vec<_> = deletes
        .chunks(1024)
        .map(|keys| RelationBatch {
            relation: "object_propvalues",
            puts: None,
            deletes: Some(Value::Array(keys.to_vec()).to_string()),
        })
        .collect();
    if let Some(puts) = array(puts) {
        batch.push(RelationBatch {
            relation: "object_propvalues",
            puts: Some(puts),
            deletes: None,
        });
    }
    Ok(PropertyPlan { batch, changes })
}

fn array(rows: Vec<Value>) -> Option<String> {
    (!rows.is_empty()).then(|| Value::Array(rows).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{
        logical::WriterEpoch,
        postgres::{encode::PropertyMutationRow, tests::empty},
    };
    use moor_var::Obj;
    use uuid::Uuid;

    #[test]
    fn indivisible_delete_commit_keeps_bounded_arrays_and_every_key() {
        let epoch = WriterEpoch::random();
        let mut commit = empty(epoch, 1, 1);
        for id in 0..2049 {
            commit.properties.push(PropertyMutationRow {
                key: ObjAndUUIDHolder::new(&Obj::mk_id(id), Uuid::nil()),
                mutation: PropertyMutation::Delete,
            });
        }
        let before = Progress {
            epoch: epoch.as_u64(),
            applied: 0,
            commits: 0,
            max_timestamp: 0,
            property_sequence: 0,
            durable_fence: 0,
        };
        let plan = TransactionPlan::group(
            &before,
            &Chains::default(),
            std::slice::from_ref(&commit),
            1,
            |_, _, _| unreachable!(),
        )
        .unwrap();
        assert_eq!(plan.commits.len(), 1);
        assert_eq!(plan.after.applied, 1);
        assert_eq!(plan.after.property_sequence, 1);
        assert_eq!(plan.changes.len(), 2049);
        let mut keys = std::collections::BTreeSet::new();
        for batch in &plan.properties[0] {
            assert!(batch.puts.is_none());
            let rows: Vec<Value> = serde_json::from_str(batch.deletes.as_ref().unwrap()).unwrap();
            assert!(rows.len() <= 1024);
            for row in rows {
                keys.insert(row["object_ref"].as_str().unwrap().to_owned());
            }
        }
        assert_eq!(keys, (0..2049).map(|id| format!("#{id}")).collect());
        assert_eq!(plan.statements(), 3);
    }
    #[test]
    fn delete_bounds_follow_confirmed_and_tentative_full_records() {
        use super::super::encode::property_row;
        let epoch = WriterEpoch::random();
        let key = ObjAndUUIDHolder::new(&Obj::mk_id(7), Uuid::nil());
        let mut chain = PropertyValueChain::full(50);
        chain.push_append(51, 3);
        let chains = [(key.clone(), chain)].into_iter().collect();
        let mut first = empty(epoch, 1, 1);
        first.properties.push(PropertyMutationRow {
            key: key.clone(),
            mutation: PropertyMutation::Full(
                property_row(
                    &key,
                    &moor_var::v_int(1),
                    Timestamp(1),
                    false,
                    &moor_compiler::SourceProfile::default(),
                )
                .unwrap(),
            ),
        });
        let mut second = empty(epoch, 2, 2);
        second.properties.push(PropertyMutationRow {
            key: key.clone(),
            mutation: PropertyMutation::Delete,
        });
        let before = Progress {
            epoch: epoch.as_u64(),
            applied: 0,
            commits: 0,
            max_timestamp: 0,
            property_sequence: 60,
            durable_fence: 0,
        };
        let commits = [first, second];
        let plan = TransactionPlan::group(
            &before,
            &chains,
            &commits,
            usize::MAX,
            |_, _, _| unreachable!(),
        )
        .unwrap();
        for (batches, expected) in plan.properties.iter().zip([50, 61]) {
            let rows: Value = serde_json::from_str(batches[0].deletes.as_ref().unwrap()).unwrap();
            assert_eq!(rows[0]["record_sequence"], json!(expected));
        }
        assert_eq!(plan.after.property_sequence, 62);
        assert_eq!(plan.changes.get(&key), Some(&None));
        assert_eq!(
            chains[&key].record_versions().collect::<Vec<_>>(),
            vec![50, 51]
        );
    }
}
