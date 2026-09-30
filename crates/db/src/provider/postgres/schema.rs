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

//! Explicit transactional creation of the readable world schema.
use super::{
    PostgresConnection, PostgresError, PostgresParam, PostgresShutdown, PostgresStorageConfig,
    codec,
};
use crate::engine::moor_db::SEQUENCE_COUNT;
use moor_compiler::{PERSISTENT_LITERAL_VERSION, PERSISTENT_SOURCE_VERSION};
use std::time::Instant;
use uuid::Uuid;

pub(super) const SCHEMA_VERSION: u32 = 1;
pub(super) const PROFILE_ID: &str = "moo-v1";

/// Initialize a new schema. Ordinary world opening never creates or repairs tables.
///
/// A pre-existing schema is an error. PostgreSQL rolls back all DDL if setup fails.
/// The initial metadata commit is durable regardless of the later data commit policy.
pub fn initialize_postgres_schema(config: &PostgresStorageConfig) -> Result<Uuid, PostgresError> {
    config.validate()?;
    let mut options = config.connection.clone();
    options.application_name = "moor-setup".into();
    let mut connection = PostgresConnection::connect(
        &options,
        Instant::now() + config.connect_timeout,
        PostgresShutdown::default(),
    )?;
    let deadline = Instant::now() + config.query_timeout;
    lock_writer(&mut connection, config, deadline)?;
    connection.query("BEGIN", &[], deadline, |_| unreachable!())?;
    connection.query(
        "SET LOCAL synchronous_commit = on",
        &[],
        deadline,
        |_| unreachable!(),
    )?;
    for statement in ddl(config)? {
        connection.query(&statement, &[], deadline, |_| unreachable!())?;
    }
    let identity = Uuid::new_v4();
    let metadata = config.schema.qualify("world_metadata")?;
    let identity_text = identity.to_string();
    let profile = codec::profile_json(&config.profile)?.to_string();
    let schema_version = SCHEMA_VERSION.to_string();
    let literal_version = PERSISTENT_LITERAL_VERSION.to_string();
    let source_version = PERSISTENT_SOURCE_VERSION.to_string();
    connection.query(&format!("INSERT INTO {metadata} (singleton, database_id, schema_version, literal_format, source_format, compiler_profile, profile) VALUES (true, $1, $2, $3, $4, $5, $6)"), &[
        PostgresParam::Text(2950,&identity_text), PostgresParam::Text(23,&schema_version),
        PostgresParam::Text(23,&literal_version), PostgresParam::Text(23,&source_version),
        PostgresParam::Text(25,PROFILE_ID), PostgresParam::Text(3802,&profile),
    ], deadline, |_| unreachable!())?;
    let progress = config.schema.qualify("writer_progress")?;
    connection.query(
        &format!("INSERT INTO {progress} VALUES (true,0,0,0,0,0,0)"),
        &[],
        deadline,
        |_| unreachable!(),
    )?;
    let sequences = config.schema.qualify("sequence_slots")?;
    let count = SEQUENCE_COUNT.to_string();
    connection.query(
        &format!(
            "INSERT INTO {sequences} SELECT n,-1 FROM pg_catalog.generate_series(0,$1::int-1) n"
        ),
        &[PostgresParam::Text(23, &count)],
        deadline,
        |_| unreachable!(),
    )?;
    connection.query("COMMIT", &[], deadline, |_| unreachable!())?;
    Ok(identity)
}

/// The session lock is scoped by database and schema, never by a random writer epoch.
pub(super) fn lock_writer(
    connection: &mut PostgresConnection,
    config: &PostgresStorageConfig,
    deadline: Instant,
) -> Result<(), PostgresError> {
    let mut acquired = false;
    let result = connection.query(
        "SELECT pg_catalog.pg_try_advisory_lock(pg_catalog.hashtextextended($1,0))",
        &[PostgresParam::Text(25, config.schema.as_str())],
        deadline,
        |row| {
            acquired = row.columns.len() == 1 && row.columns[0].as_deref() == Some(b"t");
            Ok(())
        },
    )?;
    if !acquired || result.rows != 1 {
        return Err(PostgresError::OwnershipLost);
    }
    Ok(())
}

pub(super) fn ddl(config: &PostgresStorageConfig) -> Result<Vec<String>, PostgresError> {
    let s = format!("\"{}\"", config.schema.as_str());
    let statements = [
        "CREATE SCHEMA @s@",
        "CREATE DOMAIN @s@.object_ref AS text COLLATE \"C\"",
        "CREATE DOMAIN @s@.u64_counter AS numeric(20,0) CHECK (VALUE BETWEEN 0 AND 18446744073709551615)",
        "CREATE DOMAIN @s@.u128_counter AS numeric(39,0) CHECK (VALUE BETWEEN 0 AND 340282366920938463463374607431768211455)",
        "CREATE DOMAIN @s@.flag_bits AS integer CHECK (VALUE BETWEEN 0 AND 65535)",
        "CREATE DOMAIN @s@.text_encoding AS text CHECK (VALUE IN ('utf8','json_string'))",
        "CREATE TABLE @s@.world_metadata (singleton boolean PRIMARY KEY CHECK(singleton), database_id uuid NOT NULL, schema_version integer NOT NULL, literal_format integer NOT NULL, source_format integer NOT NULL, compiler_profile text NOT NULL, profile jsonb NOT NULL CHECK(jsonb_typeof(profile)='object'))",
        "CREATE TABLE @s@.writer_progress (singleton boolean PRIMARY KEY CHECK(singleton), writer_epoch @s@.u64_counter NOT NULL, applied_version @s@.u64_counter NOT NULL, commit_sequence @s@.u64_counter NOT NULL, max_timestamp @s@.u64_counter NOT NULL, property_record_sequence bigint NOT NULL CHECK(property_record_sequence>=0), durable_fence @s@.u64_counter NOT NULL)",
        "CREATE TABLE @s@.sequence_slots (slot integer PRIMARY KEY CHECK(slot>=0), high_water bigint NOT NULL)",
        "CREATE TABLE @s@.object_location (object_ref @s@.object_ref PRIMARY KEY, logical_timestamp @s@.u64_counter NOT NULL, location_ref @s@.object_ref NOT NULL)",
        "CREATE TABLE @s@.object_parent (object_ref @s@.object_ref PRIMARY KEY, logical_timestamp @s@.u64_counter NOT NULL, parent_ref @s@.object_ref NOT NULL)",
        "CREATE TABLE @s@.object_owner (object_ref @s@.object_ref PRIMARY KEY, logical_timestamp @s@.u64_counter NOT NULL, owner_ref @s@.object_ref NOT NULL)",
        "CREATE TABLE @s@.object_flags (object_ref @s@.object_ref PRIMARY KEY, logical_timestamp @s@.u64_counter NOT NULL, flag_names text[] NOT NULL, flag_bits @s@.flag_bits NOT NULL)",
        "CREATE TABLE @s@.object_name (object_ref @s@.object_ref PRIMARY KEY, logical_timestamp @s@.u64_counter NOT NULL, name text NOT NULL, name_encoding @s@.text_encoding NOT NULL)",
        "CREATE TABLE @s@.object_propdefs (object_ref @s@.object_ref PRIMARY KEY, logical_timestamp @s@.u64_counter NOT NULL, definitions jsonb NOT NULL CHECK(jsonb_typeof(definitions)='array'))",
        "CREATE TABLE @s@.object_verbdefs (object_ref @s@.object_ref PRIMARY KEY, logical_timestamp @s@.u64_counter NOT NULL, definitions jsonb NOT NULL CHECK(jsonb_typeof(definitions)='array'))",
        "CREATE TABLE @s@.object_verbs (object_ref @s@.object_ref NOT NULL, verb_uuid uuid NOT NULL, logical_timestamp @s@.u64_counter NOT NULL, source text NOT NULL, source_format integer NOT NULL, compiler_profile text NOT NULL, PRIMARY KEY(object_ref,verb_uuid))",
        "CREATE TABLE @s@.object_propvalues (object_ref @s@.object_ref NOT NULL, property_uuid uuid NOT NULL, record_sequence bigint NOT NULL CHECK(record_sequence>0), logical_timestamp @s@.u64_counter NOT NULL, record_kind text NOT NULL CHECK(record_kind IN ('full','list_append')), literal_format integer NOT NULL, value_kind text NOT NULL CHECK(value_kind IN ('none','bool','int','float','object','symbol','string','list','map','error','flyweight','binary','lambda')), value_literal text NOT NULL, PRIMARY KEY(object_ref,property_uuid,record_sequence), CHECK(record_kind<>'list_append' OR value_kind='list'))",
        "CREATE TABLE @s@.object_propflags (object_ref @s@.object_ref NOT NULL, property_uuid uuid NOT NULL, logical_timestamp @s@.u64_counter NOT NULL, owner_ref @s@.object_ref NOT NULL, flag_names text[] NOT NULL, flag_bits @s@.flag_bits NOT NULL, PRIMARY KEY(object_ref,property_uuid))",
        "CREATE TABLE @s@.entity_metadata (object_ref @s@.object_ref NOT NULL, entity_kind text NOT NULL CHECK(entity_kind IN ('object','property','verb')), entity_uuid uuid NOT NULL, key_encoding @s@.text_encoding NOT NULL, key_folded text COLLATE \"C\" NOT NULL, logical_timestamp @s@.u64_counter NOT NULL, key_spelling text NOT NULL, key_spelling_encoding @s@.text_encoding NOT NULL, value_literal text NOT NULL, literal_format integer NOT NULL, PRIMARY KEY(object_ref,entity_kind,entity_uuid,key_encoding,key_folded), CHECK(entity_kind<>'object' OR entity_uuid='00000000-0000-0000-0000-000000000000'))",
        "CREATE TABLE @s@.object_last_move (object_ref @s@.object_ref PRIMARY KEY, logical_timestamp @s@.u64_counter NOT NULL, value_literal text NOT NULL, literal_format integer NOT NULL)",
        "CREATE TABLE @s@.anonymous_object_metadata (object_ref @s@.object_ref PRIMARY KEY, logical_timestamp @s@.u64_counter NOT NULL, created_micros @s@.u128_counter NOT NULL, last_accessed_micros @s@.u128_counter NOT NULL)",
    ];
    Ok(statements
        .into_iter()
        .map(|statement| statement.replace("@s@", &s))
        .collect())
}
