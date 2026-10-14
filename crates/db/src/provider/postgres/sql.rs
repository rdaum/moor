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

//! Fixed, prepared set-based mutations. Publication order determines every upsert.
use super::{PostgresConnection, PostgresError, PostgresStorageConfig};
use std::time::Instant;

pub(super) struct RelationSql {
    pub name: &'static str,
    pub keys: &'static [&'static str],
    pub payload: &'static [&'static str],
}
impl RelationSql {
    pub fn columns(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.keys
            .iter()
            .copied()
            .chain(["logical_timestamp"])
            .chain(self.payload.iter().copied())
    }

    /// Property records have a physical sequence suffix, but deletion addresses the whole chain.
    pub fn physical_keys(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.keys
            .iter()
            .copied()
            .chain((self.name == "object_propvalues").then_some("record_sequence"))
    }
}
macro_rules! relation {
    ($name:ident, [$($key:literal),*], [$($value:literal),*]) => {
        RelationSql { name: stringify!($name), keys: &[$($key),*], payload: &[$($value),*] }
    };
}
pub(super) const RELATIONS: &[RelationSql] = &[
    relation!(object_location, ["object_ref"], ["location_ref"]),
    relation!(object_parent, ["object_ref"], ["parent_ref"]),
    relation!(object_owner, ["object_ref"], ["owner_ref"]),
    relation!(object_flags, ["object_ref"], ["flag_names", "flag_bits"]),
    relation!(object_name, ["object_ref"], ["name", "name_encoding"]),
    relation!(object_propdefs, ["object_ref"], ["definitions"]),
    relation!(object_verbdefs, ["object_ref"], ["definitions"]),
    relation!(
        object_verbs,
        ["object_ref", "verb_uuid"],
        ["source", "source_format", "compiler_profile"]
    ),
    relation!(
        object_propvalues,
        ["object_ref", "property_uuid"],
        [
            "record_sequence",
            "record_kind",
            "literal_format",
            "value_kind",
            "value_literal"
        ]
    ),
    relation!(
        object_propflags,
        ["object_ref", "property_uuid"],
        ["owner_ref", "flag_names", "flag_bits"]
    ),
    relation!(
        entity_metadata,
        [
            "object_ref",
            "entity_kind",
            "entity_uuid",
            "key_encoding",
            "key_folded"
        ],
        [
            "key_spelling",
            "key_spelling_encoding",
            "value_literal",
            "literal_format"
        ]
    ),
    relation!(
        object_last_move,
        ["object_ref"],
        ["value_literal", "literal_format"]
    ),
    relation!(
        anonymous_object_metadata,
        ["object_ref"],
        ["created_micros", "last_accessed_micros"]
    ),
];

/// Preparing every relation also rejects missing columns and unusable conflict targets on open.
pub(super) fn prepare(
    connection: &mut PostgresConnection,
    config: &PostgresStorageConfig,
    deadline: Instant,
) -> Result<(), PostgresError> {
    // A generic plan chosen for a new, tiny relation can retain a heap scan after
    // update churn grows it by hundreds of megabytes, until ANALYZE invalidates it.
    // Replan with current relation size on this private persistence connection.
    connection.query(
        "SET plan_cache_mode=force_custom_plan",
        &[],
        deadline,
        |_| unreachable!(),
    )?;
    for relation in RELATIONS {
        let name = relation.name;
        let table = config.schema.qualify(name)?;
        let columns = relation.columns().collect::<Vec<_>>().join(",");
        let conflict = if relation.physical_keys().count() != relation.keys.len() {
            String::new()
        } else {
            let updates = std::iter::once("logical_timestamp")
                .chain(relation.payload.iter().copied())
                .map(|column| format!("{column}=EXCLUDED.{column}"))
                .collect::<Vec<_>>()
                .join(",");
            format!(
                " ON CONFLICT ({}) DO UPDATE SET {updates}",
                relation.keys.join(",")
            )
        };
        connection.prepare(&format!("put_{name}"), &format!("INSERT INTO {table} ({columns}) SELECT {columns} FROM pg_catalog.jsonb_populate_recordset(NULL::{table},$1){conflict}"), &[3802], deadline)?;
        let predicate = relation
            .keys
            .iter()
            .map(|column| format!("t.{column}=d.{column}"))
            .collect::<Vec<_>>()
            .join(" AND ");
        let delete = delete_statement(name, &table, &predicate);
        connection.prepare(&format!("delete_{name}"), &delete, &[3802], deadline)?;
    }
    let slots = config.schema.qualify("sequence_slots")?;
    connection.prepare("sequence_maxima", &format!("INSERT INTO {slots} (slot,high_water) SELECT slot,high_water FROM pg_catalog.jsonb_populate_recordset(NULL::{slots},$1) ON CONFLICT(slot) DO UPDATE SET high_water=GREATEST({slots}.high_water,EXCLUDED.high_water)"), &[3802], deadline)?;
    Ok(())
}

/// Look up property chains by their indexed logical keys, then delete those tuple versions.
/// OFFSET 0 keeps the per-key lookup from becoming a hash join over the growing heap.
/// The outer TID scan avoids scanning it again when matching the selected tuples.
/// The planned base sequence excludes dead index entries from earlier full replacements.
pub(super) fn delete_statement(name: &str, table: &str, predicate: &str) -> String {
    if name == "object_propvalues" {
        format!(
            "DELETE FROM {table} WHERE ctid = ANY (ARRAY(
            SELECT matched.ctid
            FROM pg_catalog.jsonb_populate_recordset(NULL::{table},$1) d
            CROSS JOIN LATERAL (
                SELECT t.ctid FROM {table} t WHERE {predicate}
                    AND t.record_sequence >= COALESCE(d.record_sequence,0) OFFSET 0
            ) matched
        ))"
        )
    } else {
        format!(
            "DELETE FROM {table} t USING pg_catalog.jsonb_populate_recordset(NULL::{table},$1) d WHERE {predicate}"
        )
    }
}

/// Compare the complete prior progress and install the exact planned result atomically.
pub(super) fn advance_progress(
    connection: &mut PostgresConnection,
    config: &PostgresStorageConfig,
    before: &super::state::Progress,
    after: &super::state::Progress,
    deadline: Instant,
) -> Result<(), PostgresError> {
    use super::PostgresParam;
    let table = config.schema.qualify("writer_progress")?;
    // Keep parameter positions adjacent to their names; recovery uses these same values.
    let values = [
        after.epoch.to_string(),              // $1
        after.applied.to_string(),            // $2
        after.commits.to_string(),            // $3
        after.max_timestamp.to_string(),      // $4
        after.durable_fence.to_string(),      // $5
        before.applied.to_string(),           // $6
        before.commits.to_string(),           // $7
        before.max_timestamp.to_string(),     // $8
        before.durable_fence.to_string(),     // $9
        after.property_sequence.to_string(),  // $10
        before.property_sequence.to_string(), // $11
    ];
    let params: Vec<_> = values
        .iter()
        .map(|v| PostgresParam::Text(1700, v))
        .collect();
    let result = connection
        .query(
            &format!(
                "UPDATE {table}
            SET applied_version=$2, commit_sequence=$3,
                max_timestamp=GREATEST(max_timestamp,$4), durable_fence=$5,
                property_record_sequence=$10::bigint
            WHERE singleton AND writer_epoch=$1 AND applied_version=$6
                AND commit_sequence=$7 AND max_timestamp=$8 AND durable_fence=$9
                AND property_record_sequence=$11::bigint"
            ),
            &params,
            deadline,
            |_| unreachable!(),
        )
        .map_err(|source| PostgresError::Operation {
            relation: "writer_progress",
            operation: "advance",
            source: Box::new(source),
        })?;
    if result.affected_rows != Some(1) {
        return Err(PostgresError::OwnershipLost);
    }
    Ok(())
}
