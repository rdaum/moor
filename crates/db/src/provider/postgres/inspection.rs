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

//! Optional inspection projections. Authoritative world rows retain schema version 1.
use super::{
    PostgresConnection, PostgresError, PostgresShutdown, PostgresStorageConfig, schema, state,
};
use crate::provider::property_value_store::PROPERTY_VALUE_CHAIN_LIMITS;
use std::time::Instant;

/// Install or replace inspection objects explicitly, while no writer owns the schema.
/// Does not claim an epoch, rewrite world rows, or run during ordinary startup.
pub fn install_postgres_inspection(config: &PostgresStorageConfig) -> Result<(), PostgresError> {
    config.validate()?;
    let mut options = config.connection.clone();
    options.application_name = "moor-inspection-setup".into();
    let mut connection = PostgresConnection::connect(
        &options,
        Instant::now() + config.connect_timeout,
        PostgresShutdown::default(),
    )?;
    let deadline = Instant::now() + config.query_timeout;
    schema::lock_writer(&mut connection, config, deadline)?;
    connection.query("BEGIN", &[], deadline, |_| unreachable!())?;
    connection.query(
        "SET LOCAL synchronous_commit=on",
        &[],
        deadline,
        |_| unreachable!(),
    )?;
    state::validate_metadata(&mut connection, config, deadline)?;
    for statement in ddl(config) {
        connection.query(&statement, &[], deadline, |_| unreachable!())?;
    }
    connection.query("COMMIT", &[], deadline, |_| unreachable!())?;
    Ok(())
}

pub(super) fn ddl(config: &PostgresStorageConfig) -> Vec<String> {
    let schema = format!("\"{}\"", config.schema.as_str());
    [
        r#"CREATE OR REPLACE VIEW @s@.objects WITH (security_invoker=true) AS
        SELECT f.object_ref, n.name, n.name_encoding, p.parent_ref, o.owner_ref, l.location_ref,
               f.flag_bits, f.flag_names
        FROM @s@.object_flags f
        LEFT JOIN @s@.object_name n USING (object_ref)
        LEFT JOIN @s@.object_parent p USING (object_ref)
        LEFT JOIN @s@.object_owner o USING (object_ref)
        LEFT JOIN @s@.object_location l USING (object_ref)"#,
        r#"CREATE OR REPLACE VIEW @s@.verb_definitions WITH (security_invoker=true) AS
        SELECT t.object_ref, d.ordinality AS ordinal, (d.value->>'uuid')::uuid AS verb_uuid,
               d.value->>'location_ref' AS location_ref, d.value->>'owner_ref' AS owner_ref,
               d.value->'names' AS names, d.value->'flags' AS flags, d.value->'args' AS arguments,
               t.logical_timestamp
        FROM @s@.object_verbdefs t CROSS JOIN LATERAL jsonb_array_elements(t.definitions) WITH ORDINALITY d"#,
        r#"CREATE OR REPLACE VIEW @s@.verb_names WITH (security_invoker=true) AS
        SELECT d.object_ref, d.verb_uuid, d.ordinal AS verb_ordinal, n.ordinality AS name_ordinal,
               CASE jsonb_typeof(n.value) WHEN 'string' THEN n.value #>> '{}' ELSE n.value->>'value' END AS name,
               CASE jsonb_typeof(n.value) WHEN 'string' THEN 'utf8' ELSE n.value->>'encoding' END AS name_encoding
        FROM @s@.verb_definitions d CROSS JOIN LATERAL jsonb_array_elements(d.names) WITH ORDINALITY n"#,
        r#"CREATE OR REPLACE VIEW @s@.verbs WITH (security_invoker=true) AS
        SELECT d.*, v.source, v.source_format, v.compiler_profile, v.logical_timestamp AS source_timestamp
        FROM @s@.verb_definitions d LEFT JOIN @s@.object_verbs v USING(object_ref, verb_uuid)"#,
        r#"CREATE OR REPLACE VIEW @s@.property_definitions WITH (security_invoker=true) AS
        SELECT t.object_ref, d.ordinality AS ordinal, (d.value->>'uuid')::uuid AS property_uuid,
               d.value->>'definer_ref' AS definer_ref, d.value->>'location_ref' AS location_ref,
               CASE jsonb_typeof(d.value->'name') WHEN 'string' THEN d.value->>'name' ELSE d.value->'name'->>'value' END AS property_name,
               CASE jsonb_typeof(d.value->'name') WHEN 'string' THEN 'utf8' ELSE d.value->'name'->>'encoding' END AS name_encoding,
               t.logical_timestamp
        FROM @s@.object_propdefs t CROSS JOIN LATERAL jsonb_array_elements(t.definitions) WITH ORDINALITY d"#,
        r#"CREATE OR REPLACE VIEW @s@.property_permissions WITH (security_invoker=true) AS
        SELECT p.*, d.property_name, d.name_encoding, d.definer_ref
        FROM @s@.object_propflags p LEFT JOIN @s@.property_definitions d USING(property_uuid)"#,
        r#"CREATE OR REPLACE VIEW @s@.property_records WITH (security_invoker=true) AS
        SELECT p.*, d.property_name, d.name_encoding, d.definer_ref
        FROM @s@.object_propvalues p LEFT JOIN @s@.property_definitions d USING(property_uuid)"#,
        r#"CREATE OR REPLACE FUNCTION @s@.read_property_value(target_object text, target_property uuid)
        RETURNS TABLE (value_literal text, value_kind text, logical_timestamp numeric, record_count bigint)
        LANGUAGE plpgsql STABLE STRICT SECURITY INVOKER SET search_path=pg_catalog AS $moor$
        DECLARE
            stored record;
            interior text;
            items text;
            append_bytes bigint := 0;
        BEGIN
            record_count := 0;
            FOR stored IN SELECT p.* FROM @s@.object_propvalues p
                          WHERE p.object_ref=target_object AND p.property_uuid=target_property
                          ORDER BY p.record_sequence LOOP
                IF stored.literal_format <> @literal@ THEN
                    RAISE EXCEPTION 'unsupported property literal format' USING ERRCODE='22000';
                END IF;
                IF record_count=0 THEN
                    IF stored.record_kind <> 'full' THEN
                        RAISE EXCEPTION 'property chain has no initial full record' USING ERRCODE='22000';
                    END IF;
                    value_literal := stored.value_literal;
                    value_kind := stored.value_kind;
                ELSE
                    IF stored.record_kind <> 'list_append' OR value_kind <> 'list' OR stored.value_kind <> 'list' THEN
                        RAISE EXCEPTION 'invalid property append chain' USING ERRCODE='22000';
                    END IF;
                    append_bytes := append_bytes + octet_length(stored.value_literal);
                    IF record_count >= @records@ OR append_bytes > @bytes@ THEN
                        RAISE EXCEPTION 'property append chain exceeds its bounds' USING ERRCODE='22000';
                    END IF;
                END IF;
                IF stored.value_kind='list' THEN
                    IF left(stored.value_literal,1) <> '{' OR right(stored.value_literal,1) <> '}' THEN
                        RAISE EXCEPTION 'property list requires canonical outer braces' USING ERRCODE='22000';
                    END IF;
                    interior := btrim(substring(stored.value_literal FROM 2 FOR char_length(stored.value_literal)-2), E' \t\n\r');
                    IF record_count=0 THEN
                        items := interior;
                    ELSE
                        IF interior='' THEN
                            RAISE EXCEPTION 'property append record is empty' USING ERRCODE='22000';
                        END IF;
                        items := concat_ws(', ', nullif(items,''), interior);
                    END IF;
                END IF;
                logical_timestamp := stored.logical_timestamp;
                record_count := record_count+1;
            END LOOP;
            IF record_count>0 THEN
                IF value_kind='list' THEN value_literal := '{' || items || '}'; END IF;
                RETURN NEXT;
            END IF;
        END $moor$"#,
        r#"CREATE OR REPLACE VIEW @s@.property_values WITH (security_invoker=true) AS
        SELECT k.object_ref, k.property_uuid, v.*, d.property_name, d.name_encoding, d.definer_ref
        FROM (SELECT DISTINCT object_ref, property_uuid FROM @s@.object_propvalues) k
        CROSS JOIN LATERAL @s@.read_property_value(k.object_ref, k.property_uuid) v
        LEFT JOIN @s@.property_definitions d USING(property_uuid)"#,
        r#"CREATE OR REPLACE VIEW @s@.persistence_status WITH (security_invoker=true) AS
        SELECT m.database_id, m.schema_version, m.literal_format, m.source_format, m.compiler_profile,
               p.writer_epoch, p.applied_version, p.commit_sequence, p.max_timestamp,
               p.property_record_sequence, p.durable_fence
        FROM @s@.world_metadata m CROSS JOIN @s@.writer_progress p"#,
    ].into_iter().map(|sql| sql.replace("@s@", &schema)
        .replace("@literal@", &moor_compiler::PERSISTENT_LITERAL_VERSION.to_string())
        .replace("@records@", &PROPERTY_VALUE_CHAIN_LIMITS.max_records.to_string())
        .replace("@bytes@", &PROPERTY_VALUE_CHAIN_LIMITS.max_append_bytes.to_string())).collect()
}
