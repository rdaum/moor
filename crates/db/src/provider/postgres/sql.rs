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
    for relation in RELATIONS {
        let name = relation.name;
        let table = config.schema.qualify(name)?;
        let columns: Vec<_> = relation
            .keys
            .iter()
            .copied()
            .chain(["logical_timestamp"])
            .chain(relation.payload.iter().copied())
            .collect();
        let columns = columns.join(",");
        let conflict = if name == "object_propvalues" {
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
        connection.prepare(&format!("delete_{name}"), &format!("DELETE FROM {table} t USING pg_catalog.jsonb_populate_recordset(NULL::{table},$1) d WHERE {predicate}"), &[3802], deadline)?;
    }
    let slots = config.schema.qualify("sequence_slots")?;
    connection.prepare("sequence_maxima", &format!("INSERT INTO {slots} (slot,high_water) SELECT slot,high_water FROM pg_catalog.jsonb_populate_recordset(NULL::{slots},$1) ON CONFLICT(slot) DO UPDATE SET high_water=GREATEST({slots}.high_water,EXCLUDED.high_water)"), &[3802], deadline)?;
    Ok(())
}
