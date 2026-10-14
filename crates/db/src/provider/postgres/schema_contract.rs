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

//! Registry, descriptor, native DDL, and typed row-codec conformance.
use super::{
    rows::{self, RowKey, RowValue},
    *,
};
use crate::{
    AnonymousObjectMetadata, EntityMetadataKey, ObjAndUUIDHolder, StringHolder, Timestamp,
};
use moor_common::{
    model::{ObjFlag, PropDefs, PropFlag, PropPerms, ValSet, VerbDefs},
    util::BitEnum,
};
use moor_compiler::SourceProfile;
use moor_var::{Obj, Symbol, Var, program::ProgramType, v_list, v_str};
use serde_json::{Value, json};
use std::{collections::BTreeSet, time::Instant};
use uuid::Uuid;

trait Sample {
    fn sample() -> Self;
}
macro_rules! sample {
    ($type:ty, $value:expr) => {
        impl Sample for $type {
            fn sample() -> Self {
                $value
            }
        }
    };
}
sample!(Obj, Obj::mk_id(7));
sample!(
    ObjAndUUIDHolder,
    ObjAndUUIDHolder::new(&Obj::sample(), Uuid::from_u128(17))
);
sample!(
    EntityMetadataKey,
    EntityMetadataKey::property(Obj::sample(), Uuid::from_u128(17), Symbol::mk("Straße\0"))
);
sample!(StringHolder, StringHolder("name\0".into()));
sample!(BitEnum<ObjFlag>, BitEnum::from_u16(0xffff));
sample!(
    PropPerms,
    PropPerms::new(Obj::mk_id(-1), BitEnum::<PropFlag>::from_u16(0xffff))
);
sample!(PropDefs, PropDefs::from_items(&[]));
sample!(VerbDefs, VerbDefs::from_items(&[]));
sample!(Var, v_list(&[v_str("nested, \"value\"\0")]));
sample!(
    AnonymousObjectMetadata,
    AnonymousObjectMetadata::from_micros(u128::MAX, u128::MAX - 1)
);
sample!(
    ProgramType,
    moor_compiler::read_persistent_source("return 7;", &SourceProfile::default()).unwrap()
);

fn contract<K: Sample + RowKey, V: Sample + RowValue>(
    name: &'static str,
    connection: &mut PostgresConnection,
    config: &PostgresStorageConfig,
) {
    let descriptor = sql::RELATIONS.iter().find(|d| d.name == name).unwrap();
    let key = K::sample();
    let mut row = rows::encode(
        name,
        Timestamp(u64::MAX),
        &key,
        &V::sample(),
        &config.profile,
    )
    .unwrap();
    if name == "object_propvalues" {
        row["record_sequence"] = json!(1);
        row["record_kind"] = json!("full");
        row["value_kind"] = json!("list");
    }
    let expected: BTreeSet<_> = descriptor.columns().collect();
    let actual: BTreeSet<_> = row
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(expected, actual, "encoded columns for {name}");
    let deadline = Instant::now() + config.query_timeout;
    let mut columns = BTreeSet::new();
    connection.query(
        "SELECT a.attname FROM pg_catalog.pg_attribute a JOIN pg_catalog.pg_class c ON c.oid=a.attrelid JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=$1 AND c.relname=$2 AND a.attnum>0 AND NOT a.attisdropped",
        &[PostgresParam::Text(25, config.schema.as_str()), PostgresParam::Text(25, name)], deadline,
        |row| { columns.insert(String::from_utf8(row.columns[0].clone().unwrap()).unwrap()); Ok(()) },
    ).unwrap();
    assert_eq!(
        expected,
        columns.iter().map(String::as_str).collect(),
        "DDL columns for {name}"
    );
    let mut keys = Vec::new();
    connection.query(
        "SELECT a.attname FROM pg_catalog.pg_constraint p JOIN pg_catalog.pg_class c ON c.oid=p.conrelid JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace CROSS JOIN LATERAL unnest(p.conkey) WITH ORDINALITY k(attnum, position) JOIN pg_catalog.pg_attribute a ON a.attrelid=c.oid AND a.attnum=k.attnum WHERE n.nspname=$1 AND c.relname=$2 AND p.contype='p' ORDER BY k.position",
        &[PostgresParam::Text(25, config.schema.as_str()), PostgresParam::Text(25, name)], deadline,
        |row| { keys.push(String::from_utf8(row.columns[0].clone().unwrap()).unwrap()); Ok(()) },
    ).unwrap();
    assert_eq!(
        descriptor.physical_keys().collect::<Vec<_>>(),
        keys,
        "primary key for {name}"
    );
    let payload = json!([row]).to_string();
    connection
        .execute_prepared(
            &format!("put_{name}"),
            &[PostgresParam::Text(3802, &payload)],
            deadline,
            |_| unreachable!(),
        )
        .unwrap();
    let table = config.schema.qualify(name).unwrap();
    let mut stored = Vec::new();
    connection
        .query(
            &format!("SELECT {} FROM {table} t", rows::select_row(name)),
            &[],
            deadline,
            |row| {
                stored.push(rows::parse_row(row)?);
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(stored.len(), 1);
    let (timestamp, decoded_key, decoded_value) =
        rows::decode::<K, V>(name, stored.pop().unwrap(), &config.profile).unwrap();
    let decoded = rows::encode(
        name,
        timestamp,
        &decoded_key,
        &decoded_value,
        &config.profile,
    )
    .unwrap();
    // Physical property-chain fields are supplied by planning, not the logical value codec.
    let mut original = row;
    if name == "object_propvalues" {
        for field in ["record_sequence", "record_kind", "value_kind"] {
            original.as_object_mut().unwrap().remove(field);
        }
    }
    assert_eq!(decoded, original, "native round trip for {name}");
    let deletion = json!([Value::Object(key.encode_key(name))]).to_string();
    connection
        .execute_prepared(
            &format!("delete_{name}"),
            &[PostgresParam::Text(3802, &deletion)],
            deadline,
            |_| unreachable!(),
        )
        .unwrap();
    let result = connection
        .query(&format!("SELECT 1 FROM {table}"), &[], deadline, |_| Ok(()))
        .unwrap();
    assert_eq!(result.rows, 0, "delete key for {name}");
}

#[test]
#[ignore = "requires PostgreSQL fixture"]
fn registry_descriptors_ddl_and_codecs_agree() {
    let config = tests::config();
    initialize_postgres_schema(&config).unwrap();
    let mut connection = PostgresConnection::connect(
        &config.connection,
        Instant::now() + config.connect_timeout,
        PostgresShutdown::default(),
    )
    .unwrap();
    sql::prepare(
        &mut connection,
        &config,
        Instant::now() + config.query_timeout,
    )
    .unwrap();
    let mut tables = BTreeSet::new();
    connection.query(
        "SELECT c.relname FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=$1 AND c.relkind='r'",
        &[PostgresParam::Text(25, config.schema.as_str())], Instant::now() + config.query_timeout,
        |row| { tables.insert(String::from_utf8(row.columns[0].clone().unwrap()).unwrap()); Ok(()) },
    ).unwrap();
    let expected: BTreeSet<_> = sql::RELATIONS
        .iter()
        .map(|r| r.name)
        .chain(["world_metadata", "writer_progress", "sequence_slots"])
        .collect();
    assert_eq!(
        expected,
        tables.iter().map(String::as_str).collect(),
        "DDL relation coverage"
    );
    macro_rules! check_registry {
        ($( $name:ident $kind:ident $policy:ident $eq:tt $key:ty, $value:ty, )*) => {{
            let registry: BTreeSet<_> = [$(stringify!($name)),*].into_iter().collect();
            let descriptors: BTreeSet<_> = sql::RELATIONS.iter().map(|r| r.name).collect();
            assert_eq!(registry, descriptors);
            assert_eq!(descriptors.len(), sql::RELATIONS.len(), "duplicate descriptor");
            $( contract::<$key, $value>(stringify!($name), &mut connection, &config); )*
        }};
    }
    crate::relation_registry::relation_registry!(check_registry);
}
