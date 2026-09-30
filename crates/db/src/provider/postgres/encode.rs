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

//! Parallel-worker rendering of logical mutations into readable PostgreSQL rows.
use super::{
    PostgresError, codec,
    rows::{self, RowKey, RowValue},
};
use crate::{
    ObjAndUUIDHolder, Timestamp,
    engine::moor_db::RelationChanges,
    provider::logical::{
        LogicalCommit, PreparedPropertyValueMutation, PreparedPropertyValueOp, PublicationId,
    },
    tx::{OpType, RelationCodomain, RelationDomain, WorkingSetTuples},
};
use moor_compiler::SourceProfile;
use moor_var::Var;
use serde_json::{Value, json};

pub(super) struct RelationBatch {
    pub relation: &'static str,
    pub puts: Option<String>,
    pub deletes: Option<String>,
}
impl RelationBatch {
    pub fn encoded_bytes(&self) -> usize {
        self.puts.as_ref().map_or(0, String::len) + self.deletes.as_ref().map_or(0, String::len)
    }
}
pub(super) enum PropertyMutation {
    Delete,
    Full(Value),
    Append { row: Value, final_value: Var },
}
pub(super) struct PropertyMutationRow {
    pub key: ObjAndUUIDHolder,
    pub mutation: PropertyMutation,
}
pub(super) struct EncodedCommit {
    pub publication: PublicationId,
    pub timestamp: Timestamp,
    pub ordinary: Vec<RelationBatch>,
    pub properties: Vec<PropertyMutationRow>,
    pub sequences: Option<String>,
    /// Conservative encoded payload size, computed off the SQL writer thread.
    pub group_bytes: usize,
    pub group_operations: usize,
}

fn json_array(rows: Vec<Value>) -> Option<String> {
    if rows.is_empty() {
        return None;
    }
    Some(Value::Array(rows).to_string())
}
fn ordinary<K: RowKey + RelationDomain, V: RowValue + RelationCodomain>(
    relation: &'static str,
    changes: WorkingSetTuples<K, V>,
    timestamp: Timestamp,
    profile: &SourceProfile,
) -> Result<RelationBatch, PostgresError> {
    let mut puts = Vec::new();
    let mut deletes = Vec::new();
    for (key, op) in changes {
        match op.operation {
            OpType::Delete => deletes.push(Value::Object(key.encode_key(relation))),
            OpType::Insert(value) | OpType::Update(value) => {
                puts.push(rows::encode(relation, timestamp, &key, &value, profile)?)
            }
        }
    }
    Ok(RelationBatch {
        relation,
        puts: json_array(puts),
        deletes: json_array(deletes),
    })
}

/// Full values are rendered on encoders; append candidates render only their proven suffix.
pub(super) fn property_row(
    key: &ObjAndUUIDHolder,
    value: &Var,
    timestamp: Timestamp,
    append: bool,
    profile: &SourceProfile,
) -> Result<Value, PostgresError> {
    let mut row = rows::encode("object_propvalues", timestamp, key, value, profile)?;
    row["record_kind"] = json!(if append { "list_append" } else { "full" });
    row["value_kind"] = json!(codec::value_kind(value));
    Ok(row)
}
fn properties(
    changes: Vec<PreparedPropertyValueOp>,
    timestamp: Timestamp,
    profile: &SourceProfile,
) -> Result<Vec<PropertyMutationRow>, PostgresError> {
    changes
        .into_iter()
        .map(|op| {
            let mutation = match op.mutation {
                PreparedPropertyValueMutation::Delete => PropertyMutation::Delete,
                PreparedPropertyValueMutation::Replace { value } => PropertyMutation::Full(
                    property_row(&op.property, &value, timestamp, false, profile)?,
                ),
                PreparedPropertyValueMutation::AppendList {
                    suffix,
                    final_value,
                } => PropertyMutation::Append {
                    row: property_row(&op.property, &Var::from(suffix), timestamp, true, profile)?,
                    final_value,
                },
            };
            Ok(PropertyMutationRow {
                key: op.property,
                mutation,
            })
        })
        .collect()
}

macro_rules! define_encode {
    ($( $field:ident $category:ident $policy:ident $arrow:tt $domain:ty, $codomain:ty ),* $(,)?) => {
        fn operation_count(changes: &RelationChanges) -> usize {
            0 $(+ changes.$field.len())*
        }
        fn relations(changes: RelationChanges, timestamp: Timestamp, profile: &SourceProfile) -> Result<(Vec<RelationBatch>, Vec<PropertyMutationRow>), PostgresError> {
            let mut ordinary = Vec::new();
            let mut properties = Vec::new();
            $(define_encode!(@encode $category, changes.$field, stringify!($field), timestamp, profile, ordinary, properties);)*
            Ok((ordinary, properties))
        }
    };
    (@encode Ordinary, $changes:expr, $relation:expr, $ts:expr, $profile:expr, $ordinary:ident, $properties:ident) => {
        if !$changes.is_empty() { $ordinary.push(ordinary($relation, $changes, $ts, $profile)?); }
    };
    (@encode PropertyValueChain, $changes:expr, $relation:expr, $ts:expr, $profile:expr, $ordinary:ident, $properties:ident) => {
        $properties.extend(properties($changes, $ts, $profile)?);
    };
}
crate::relation_registry::relation_registry!(define_encode);

pub(super) fn encode(
    commit: LogicalCommit,
    profile: &SourceProfile,
) -> Result<EncodedCommit, PostgresError> {
    // Count both the delete and insert work of a property replacement or rollup.
    let group_operations = operation_count(&commit.changes)
        + commit.changes.object_propvalues.len()
        + commit.sequences.len();
    let (ordinary, properties) = relations(commit.changes, commit.timestamp, profile)?;
    if commit
        .sequences
        .iter()
        .any(|s| s.slot >= crate::engine::moor_db::SEQUENCE_COUNT)
    {
        return Err(codec::invalid("sequence_slots", "unknown sequence slot"));
    }
    let sequences = json_array(
        commit
            .sequences
            .iter()
            .map(|s| json!({"slot":s.slot,"high_water":s.value}))
            .collect(),
    );
    let group_bytes = ordinary
        .iter()
        .map(RelationBatch::encoded_bytes)
        .sum::<usize>()
        + sequences.as_ref().map_or(0, String::len)
        + properties
            .iter()
            .map(|property| match &property.mutation {
                PropertyMutation::Delete => 128,
                PropertyMutation::Full(row) | PropertyMutation::Append { row, .. } => {
                    json_size_bound(row) + 128
                }
            })
            .sum::<usize>();
    Ok(EncodedCommit {
        publication: commit.publication,
        timestamp: commit.timestamp,
        ordinary,
        properties,
        sequences,
        group_bytes,
        group_operations,
    })
}

// JSON escapes use at most six bytes per input byte. Avoid rendering large literals twice
// just to bound a group; the allowance also covers record sequences added by the writer.
fn json_size_bound(value: &Value) -> usize {
    match value {
        Value::String(text) => 2 + 6 * text.len(),
        Value::Array(values) => 2 + values.iter().map(|v| 1 + json_size_bound(v)).sum::<usize>(),
        Value::Object(values) => {
            2 + values
                .iter()
                .map(|(k, v)| 4 + 6 * k.len() + json_size_bound(v))
                .sum::<usize>()
        }
        _ => 32,
    }
}
