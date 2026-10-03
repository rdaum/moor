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

//! Typed point reads and bounded cursor fetches on one owned PostgreSQL snapshot.
use super::{
    PostgresError,
    codec::invalid,
    rows::{self, RowKey, RowValue},
    seed,
    snapshot::{Command, ReadSession, Reply},
};
use crate::{
    AnonymousObjectMetadata, EntityMetadataKey, ObjAndUUIDHolder, StringHolder,
    provider::{
        property_value_store::{PROPERTY_VALUE_CHAIN_LIMITS, PropertyValueReconstructor},
        read::{
            MetadataEntity, MetadataScan, ObjectScan, ObjectUuidScan, ReadKey, RelationReader,
            SnapshotReaders, TupleCursor,
        },
    },
    tx::{Error, RelationCodomain, RelationDomain, Timestamp},
};
use moor_common::{
    model::{ObjFlag, PropDefs, PropPerms, VerbDefs},
    util::BitEnum,
};
use moor_var::{Obj, Var, program::ProgramType};
use serde_json::Value;
use std::{collections::VecDeque, marker::PhantomData, sync::Arc};
use uuid::Uuid;

/// SQL must match the object byte order used by the shared streaming export joins.
/// Object storage uses native-endian u64 bytes; identities stay readable in SQL.
fn object_order() -> String {
    let bits = "CASE WHEN t.object_ref LIKE '#anon_%' THEN (('x'||replace(substr(t.object_ref,7),'-',''))::bit(64) | X'8000000000000000'::bit(64))::bigint WHEN t.object_ref ~ '^#-?[0-9]+$' THEN (substr(t.object_ref,2)::bigint & 4294967295) ELSE (('x'||replace(substr(t.object_ref,2),'-',''))::bit(64) | X'4000000000000000'::bit(64))::bigint END";
    let bytes = format!("pg_catalog.int8send({bits})");
    if cfg!(target_endian = "big") {
        return bytes;
    }
    (1..=8)
        .rev()
        .map(|index| format!("substr({bytes},{index},1)"))
        .collect::<Vec<_>>()
        .join(" || ")
}
fn uuid_column(relation: &str) -> &'static str {
    if relation == "object_verbs" {
        "verb_uuid"
    } else {
        "property_uuid"
    }
}

#[derive(Default)]
struct Filter {
    clauses: Vec<String>,
    parameters: Vec<String>,
}
impl Filter {
    fn add(&mut self, column: &str, value: String, cast: &str) {
        self.parameters.push(value);
        self.clauses
            .push(format!("t.{column}=${}::{cast}", self.parameters.len()));
    }
    fn object(object: Obj) -> Self {
        let mut filter = Self::default();
        filter.add("object_ref", object.to_literal(), "text");
        filter
    }
    fn predicate(&self) -> String {
        if self.clauses.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", self.clauses.join(" AND "))
        }
    }
}
trait PgReadKey: ReadKey + RowKey + RelationDomain {
    fn point_filter(&self, relation: &'static str) -> Filter;
    fn scan_filter(request: Self::Scan) -> Filter;
    fn order(relation: &'static str) -> String;
}
impl PgReadKey for Obj {
    fn point_filter(&self, _: &'static str) -> Filter {
        Filter::object(*self)
    }
    fn scan_filter(_: ObjectScan) -> Filter {
        Filter::default()
    }
    fn order(_: &'static str) -> String {
        object_order()
    }
}
impl PgReadKey for ObjAndUUIDHolder {
    fn point_filter(&self, relation: &'static str) -> Filter {
        let mut filter = Filter::object(self.obj());
        filter.add(uuid_column(relation), self.uuid().to_string(), "uuid");
        filter
    }
    fn scan_filter(request: ObjectUuidScan) -> Filter {
        match request {
            ObjectUuidScan::All => Filter::default(),
            ObjectUuidScan::Object(obj) => Filter::object(obj),
        }
    }
    fn order(relation: &'static str) -> String {
        let extra = if relation == "object_propvalues" {
            ", t.record_sequence"
        } else {
            ""
        };
        format!("{}, t.{}{extra}", object_order(), uuid_column(relation))
    }
}
impl PgReadKey for EntityMetadataKey {
    fn point_filter(&self, relation: &'static str) -> Filter {
        let row = self.encode_key(relation);
        let mut filter = Filter::default();
        for (column, cast) in [
            ("object_ref", "text"),
            ("entity_kind", "text"),
            ("entity_uuid", "uuid"),
            ("key_encoding", "text"),
            ("key_folded", "text"),
        ] {
            filter.add(
                column,
                row[column].as_str().expect("encoded key field").into(),
                cast,
            );
        }
        filter
    }
    fn scan_filter(request: MetadataScan) -> Filter {
        match request {
            MetadataScan::All => Filter::default(),
            MetadataScan::Object(object) => Filter::object(object),
            MetadataScan::Entity(object, entity) => {
                let mut filter = Filter::object(object);
                let (kind, uuid) = match entity {
                    MetadataEntity::Object => ("object", Uuid::nil()),
                    MetadataEntity::Property(uuid) => ("property", uuid),
                    MetadataEntity::Verb(uuid) => ("verb", uuid),
                };
                filter.add("entity_kind", kind.into(), "text");
                filter.add("entity_uuid", uuid.to_string(), "uuid");
                filter
            }
        }
    }
    fn order(_: &'static str) -> String {
        format!(
            "{}, t.entity_kind COLLATE \"C\", t.entity_uuid, t.key_encoding COLLATE \"C\", t.key_folded COLLATE \"C\"",
            object_order()
        )
    }
}

struct RowCursor {
    session: Arc<ReadSession>,
    cursor: u64,
    buffered: VecDeque<Value>,
    ended: bool,
}
impl RowCursor {
    fn open(
        session: &Arc<ReadSession>,
        relation: &'static str,
        filter: Filter,
        order: String,
    ) -> Result<Self, PostgresError> {
        let Reply::Cursor(cursor) = session.request(Command::Open {
            relation,
            predicate: filter.predicate(),
            parameters: filter.parameters,
            order,
        })?
        else {
            return Err(invalid("snapshot", "unexpected cursor reply"));
        };
        Ok(Self {
            session: session.clone(),
            cursor,
            buffered: VecDeque::new(),
            ended: false,
        })
    }
}
impl Iterator for RowCursor {
    type Item = Result<Value, PostgresError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.ended {
            return None;
        }
        if self.buffered.is_empty() {
            match self.session.request(Command::Fetch(self.cursor)) {
                Ok(Reply::Rows(rows)) if !rows.is_empty() => self.buffered = rows.into(),
                Ok(Reply::Rows(_)) => {
                    self.ended = true;
                    return None;
                }
                Ok(_) => {
                    self.ended = true;
                    return Some(Err(invalid("snapshot", "unexpected fetch reply")));
                }
                Err(error) => {
                    self.ended = true;
                    return Some(Err(error));
                }
            }
        }
        self.buffered.pop_front().map(Ok)
    }
}
impl Drop for RowCursor {
    fn drop(&mut self) {
        let _ = self.session.request(Command::Close(self.cursor));
    }
}
fn retrieval(error: PostgresError) -> Error {
    Error::SnapshotFailure(error.to_string())
}

struct Reader<K, V> {
    session: Arc<ReadSession>,
    relation: &'static str,
    types: PhantomData<(K, V)>,
}
impl<K, V> Reader<K, V> {
    fn new(session: &Arc<ReadSession>, relation: &'static str) -> Self {
        Self {
            session: session.clone(),
            relation,
            types: PhantomData,
        }
    }
}
impl<K: PgReadKey, V: RowValue + RelationCodomain> Reader<K, V> {
    fn cursor(&self, filter: Filter) -> Result<TupleCursor<K, V>, Error> {
        let rows = RowCursor::open(
            &self.session,
            self.relation,
            filter,
            K::order(self.relation),
        )
        .map_err(retrieval)?;
        let session = self.session.clone();
        let relation = self.relation;
        Ok(Box::new(rows.map(move |row| {
            let (ts, key, value) =
                rows::decode(relation, row.map_err(retrieval)?, &session.config.profile)
                    .map_err(retrieval)?;
            seed::check_timestamp(ts, &session.progress).map_err(retrieval)?;
            Ok((ts, key, value))
        })))
    }
}
impl<K: PgReadKey, V: RowValue + RelationCodomain> RelationReader<K, V> for Reader<K, V> {
    fn get(&self, key: &K) -> Result<Option<(Timestamp, V)>, Error> {
        let mut cursor = self.cursor(key.point_filter(self.relation))?;
        let result = cursor.next().transpose()?.map(|(ts, _, value)| (ts, value));
        if cursor.next().transpose()?.is_some() {
            return Err(retrieval(invalid(self.relation, "duplicate key")));
        }
        Ok(result)
    }
    fn scan(&self, request: K::Scan) -> Result<TupleCursor<K, V>, Error> {
        self.cursor(K::scan_filter(request))
    }
}

struct PropertyReader {
    session: Arc<ReadSession>,
}
struct PropertyCursor {
    rows: RowCursor,
    pending: Option<Value>,
    ended: bool,
}
impl Iterator for PropertyCursor {
    type Item = Result<(Timestamp, ObjAndUUIDHolder, Var), Error>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.ended {
            return None;
        }
        let result = (|| {
            let first = match self.pending.take() {
                Some(row) => row,
                None => match self.rows.next().transpose()? {
                    Some(row) => row,
                    None => return Ok(None),
                },
            };
            let key = ObjAndUUIDHolder::decode_key(
                first
                    .as_object()
                    .ok_or_else(|| invalid("snapshot", "expected property row"))?,
                "object_propvalues",
            )?;
            let mut value = Some(first);
            let mut reconstructor = PropertyValueReconstructor::new(PROPERTY_VALUE_CHAIN_LIMITS);
            while let Some(next) = value {
                let row = next
                    .as_object()
                    .ok_or_else(|| invalid("snapshot", "expected property row"))?;
                let next_key = ObjAndUUIDHolder::decode_key(row, "object_propvalues")?;
                if next_key != key {
                    self.pending = Some(next);
                    break;
                }
                let record = seed::decode_property_record(
                    row,
                    &self.rows.session.config,
                    &self.rows.session.progress,
                )?;
                reconstructor
                    .push_decoded(
                        record.sequence,
                        record.kind,
                        record.timestamp,
                        record.literal_bytes,
                        record.value,
                    )
                    .map_err(|_| {
                        rows::contextual(
                            "object_propvalues",
                            row,
                            invalid("object_propvalues", "invalid property chain"),
                        )
                    })?;
                value = self.rows.next().transpose()?;
            }
            let value = reconstructor
                .finish()
                .map_err(|_| invalid("object_propvalues", "invalid property chain"))?;
            Ok(Some((value.logical_timestamp, key, value.value)))
        })();
        match result {
            Ok(Some(row)) => Some(Ok(row)),
            Ok(None) => {
                self.ended = true;
                None
            }
            Err(error) => {
                self.ended = true;
                Some(Err(retrieval(error)))
            }
        }
    }
}
impl PropertyReader {
    fn cursor(&self, filter: Filter) -> Result<TupleCursor<ObjAndUUIDHolder, Var>, Error> {
        Ok(Box::new(PropertyCursor {
            rows: RowCursor::open(
                &self.session,
                "object_propvalues",
                filter,
                ObjAndUUIDHolder::order("object_propvalues"),
            )
            .map_err(retrieval)?,
            pending: None,
            ended: false,
        }))
    }
}
impl RelationReader<ObjAndUUIDHolder, Var> for PropertyReader {
    fn get(&self, key: &ObjAndUUIDHolder) -> Result<Option<(Timestamp, Var)>, Error> {
        let mut cursor = self.cursor(key.point_filter("object_propvalues"))?;
        cursor
            .next()
            .transpose()
            .map(|value| value.map(|(ts, _, value)| (ts, value)))
    }
    fn scan(&self, request: ObjectUuidScan) -> Result<TupleCursor<ObjAndUUIDHolder, Var>, Error> {
        self.cursor(ObjAndUUIDHolder::scan_filter(request))
    }
}
macro_rules! define_readers {
    ($( $field:ident $category:ident $policy:ident $arrow:tt $domain:ty, $codomain:ty ),* $(,)?) => {
        pub(super) fn readers(session: Arc<ReadSession>) -> SnapshotReaders {
            SnapshotReaders { $( $field: define_readers!(@reader $category, &session, stringify!($field), $domain, $codomain), )* }
        }
    };
    (@reader Ordinary, $session:expr, $relation:expr, $key:ty, $value:ty) => { Arc::new(Reader::<$key,$value>::new($session,$relation)) };
    (@reader PropertyValueChain, $session:expr, $relation:expr, $key:ty, $value:ty) => { Arc::new(PropertyReader { session: $session.clone() }) };
}
crate::relation_registry::relation_registry!(define_readers);
