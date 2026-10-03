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
pub(crate) struct EncodedCommit {
    pub(super) publication: PublicationId,
    pub(super) timestamp: Timestamp,
    pub(super) ordinary: Vec<RelationBatch>,
    pub(super) properties: Vec<PropertyMutationRow>,
    pub(super) sequences: Option<String>,
    /// Conservative encoded payload size, computed off the SQL writer thread.
    pub(super) group_bytes: usize,
    pub(super) group_operations: usize,
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
    limits: &mut PayloadLimits,
) -> Result<RelationBatch, PostgresError> {
    let mut puts = Vec::new();
    let mut deletes = Vec::new();
    for (key, op) in changes {
        match op.operation {
            OpType::Delete => {
                let row = Value::Object(key.encode_key(relation));
                limits.charge(postgres_json_bytes(&row))?;
                deletes.push(row);
            }
            OpType::Insert(value) | OpType::Update(value) => {
                let row = rows::encode(relation, timestamp, &key, &value, profile)?;
                limits.charge(validate_row(relation, &row, limits.row_bytes)?)?;
                puts.push(row);
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
    limits: &mut PayloadLimits,
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
            let size = match &mutation {
                PropertyMutation::Delete => 128,
                PropertyMutation::Full(row) => {
                    validate_row("object_propvalues", row, limits.row_bytes)?
                }
                PropertyMutation::Append { row, final_value } => {
                    let suffix = validate_row("object_propvalues", row, limits.row_bytes)?;
                    // A valid suffix must also permit a future full replacement or rollup.
                    let full = property_row(&op.property, final_value, timestamp, false, profile)?;
                    suffix.max(validate_row("object_propvalues", &full, limits.row_bytes)?)
                }
            };
            limits.charge(size)?;
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
        fn relations(changes: RelationChanges, timestamp: Timestamp, profile: &SourceProfile, limits: &mut PayloadLimits) -> Result<(Vec<RelationBatch>, Vec<PropertyMutationRow>), PostgresError> {
            let mut ordinary = Vec::new();
            let mut properties = Vec::new();
            $(define_encode!(@encode $category, changes.$field, stringify!($field), timestamp, profile, limits, ordinary, properties);)*
            Ok((ordinary, properties))
        }
    };
    (@encode Ordinary, $changes:expr, $relation:expr, $ts:expr, $profile:expr, $limits:ident, $ordinary:ident, $properties:ident) => {
        if !$changes.is_empty() { $ordinary.push(ordinary($relation, $changes, $ts, $profile, $limits)?); }
    };
    (@encode PropertyValueChain, $changes:expr, $relation:expr, $ts:expr, $profile:expr, $limits:ident, $ordinary:ident, $properties:ident) => {
        $properties.extend(properties($changes, $ts, $profile, $limits)?);
    };
}
crate::relation_registry::relation_registry!(define_encode);

/// Encode and validate on a worker before publication, retaining the result for submission.
pub(super) fn prepare(
    commit: LogicalCommit,
    profile: &SourceProfile,
    max_row_bytes: usize,
) -> Result<EncodedCommit, PostgresError> {
    // Count both the delete and insert work of a property replacement or rollup.
    let group_operations = operation_count(&commit.changes)
        + commit.changes.object_propvalues.len()
        + commit.sequences.len();
    let mut limits = PayloadLimits {
        row_bytes: max_row_bytes,
        commit_bytes: 1024,
    };
    let (ordinary, properties) = relations(commit.changes, commit.timestamp, profile, &mut limits)?;
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

/// Maximum encoded payload, including possible full property rollups, per logical commit.
/// Leave ample room below PostgreSQL's per-field and protocol limits for SQL grouping.
pub(super) const MAX_COMMIT_BYTES: usize = 256 * 1024 * 1024;

/// Validate the exact JSON text size used by PostgreSQL startup, including its whitespace.
/// Reserve the widest record sequence because the writer assigns it after publication.
fn validate_row(relation: &'static str, row: &Value, limit: usize) -> Result<usize, PostgresError> {
    let mut size = postgres_json_bytes(row);
    if relation == "object_propvalues" {
        size += ", \"record_sequence\": 9223372036854775807".len();
    }
    if size > limit {
        return Err(rows::contextual(
            relation,
            row.as_object().unwrap(),
            PostgresError::RowLimit,
        ));
    }
    Ok(size)
}

fn json_string_bytes(text: &str) -> usize {
    2 + text
        .bytes()
        .map(|byte| match byte {
            b'"' | b'\\' | b'\x08' | b'\x0c' | b'\n' | b'\r' | b'\t' => 2,
            0..=31 => 6,
            _ => 1,
        })
        .sum::<usize>()
}
fn postgres_json_bytes(value: &Value) -> usize {
    match value {
        Value::String(text) => json_string_bytes(text),
        Value::Array(values) => {
            2 + values.iter().map(postgres_json_bytes).sum::<usize>()
                + 2 * values.len().saturating_sub(1)
        }
        Value::Object(values) => {
            2 + values
                .iter()
                .map(|(key, value)| json_string_bytes(key) + 2 + postgres_json_bytes(value))
                .sum::<usize>()
                + 2 * values.len().saturating_sub(1)
        }
        _ => value.to_string().len(),
    }
}

struct PayloadLimits {
    row_bytes: usize,
    commit_bytes: usize,
}
impl PayloadLimits {
    fn charge(&mut self, bytes: usize) -> Result<(), PostgresError> {
        // Array framing and the delete key of a possible property replacement.
        self.commit_bytes = self.commit_bytes.saturating_add(bytes).saturating_add(128);
        if self.commit_bytes > MAX_COMMIT_BYTES {
            return Err(PostgresError::Configuration(
                "logical commit exceeds the 256 MiB encoded payload limit",
            ));
        }
        Ok(())
    }
}

pub(super) fn finish_prepared(mut prepared: EncodedCommit, commit: LogicalCommit) -> EncodedCommit {
    prepared.publication = commit.publication;
    prepared.sequences = json_array(
        commit
            .sequences
            .iter()
            .map(|s| json!({"slot":s.slot,"high_water":s.value}))
            .collect(),
    );
    prepared.group_bytes += prepared.sequences.as_ref().map_or(0, String::len);
    prepared.group_operations += commit.sequences.len();
    prepared
}

#[cfg(test)]
mod limit_tests {
    use super::*;

    #[test]
    fn commit_budget_accepts_its_boundary_and_rejects_overflow() {
        let mut limits = PayloadLimits {
            row_bytes: usize::MAX,
            commit_bytes: 1024,
        };
        assert!(limits.charge(MAX_COMMIT_BYTES - 1024 - 128).is_ok());
        assert!(limits.charge(1).is_err());
        assert!(limits.charge(usize::MAX).is_err());
    }

    #[test]
    fn row_limit_includes_json_spacing_and_future_sequence_width() {
        let row = json!({"object_ref":"#1", "property_uuid": "00000000-0000-0000-0000-000000000001",
            "logical_timestamp":"18446744073709551615", "value_literal":"\"a\\\\b\"",
            "record_kind":"full", "literal_format":1, "value_kind":"string"});
        let size = validate_row("object_propvalues", &row, usize::MAX).unwrap();
        assert!(validate_row("object_propvalues", &row, size).is_ok());
        assert!(matches!(
            validate_row("object_propvalues", &row, size - 1),
            Err(PostgresError::Row { .. })
        ));
        let mut persisted = row;
        persisted["record_sequence"] = json!(i64::MAX);
        assert_eq!(size, postgres_json_bytes(&persisted));
    }

    #[test]
    #[ignore = "requires PostgreSQL fixture"]
    fn json_size_matches_postgres_text_for_nested_and_escaped_fields() {
        let config = super::super::tests::config();
        let mut connection = super::super::PostgresConnection::connect(
            &config.connection,
            std::time::Instant::now() + config.connect_timeout,
            super::super::PostgresShutdown::default(),
        )
        .unwrap();
        for row in [
            json!({"empty":[], "none":null, "object":{}, "bools":[true,false]}),
            json!({"α🐄":["quotes\"and\\slashes", "\n\r\t\u{1}\u{8}\u{c}", "牛"],
                "number":18446744073709551615_u64, "nested":{"keys":["one","two"]}}),
        ] {
            let text = row.to_string();
            let expected = postgres_json_bytes(&row);
            connection
                .query(
                    "SELECT octet_length($1::jsonb::text)",
                    &[super::super::PostgresParam::Text(3802, &text)],
                    std::time::Instant::now() + config.query_timeout,
                    |wire| {
                        assert_eq!(
                            std::str::from_utf8(wire.columns[0].as_ref().unwrap())
                                .unwrap()
                                .parse::<usize>()
                                .unwrap(),
                            expected
                        );
                        Ok(())
                    },
                )
                .unwrap();
        }
    }
}
