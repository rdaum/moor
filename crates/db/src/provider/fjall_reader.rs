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

//! Streaming typed reads against an owned Fjall snapshot.
use super::{
    fjall_provider::{FjallCodec, decode_fjall_value},
    property_value_store::{
        PROPERTY_VALUE_CHAIN_LIMITS, PropertyValueScan, property_value_record_bounds,
        reconstruct_property_value,
    },
    read::{
        MetadataEntity, MetadataScan, ObjectScan, ObjectUuidScan, ReadKey, RelationReader,
        TupleCursor,
    },
};
use crate::{
    EntityMetadataKey, ObjAndUUIDHolder,
    tx::{EncodeFor, Error, Timestamp},
};
use byteview::ByteView;
use fjall::Readable;
use moor_var::{Obj, Var};
use std::marker::PhantomData;
use zerocopy::IntoBytes;

pub(crate) trait FjallReadKey: ReadKey {
    fn prefix(request: Self::Scan) -> Option<Vec<u8>>;
}
impl FjallReadKey for Obj {
    fn prefix(_: ObjectScan) -> Option<Vec<u8>> {
        None
    }
}
impl FjallReadKey for ObjAndUUIDHolder {
    fn prefix(request: ObjectUuidScan) -> Option<Vec<u8>> {
        match request {
            ObjectUuidScan::All => None,
            ObjectUuidScan::Object(obj) => Some(obj.as_bytes().to_vec()),
        }
    }
}
impl FjallReadKey for EntityMetadataKey {
    fn prefix(request: MetadataScan) -> Option<Vec<u8>> {
        let (object, entity) = match request {
            MetadataScan::All => return None,
            MetadataScan::Object(obj) => (obj, None),
            MetadataScan::Entity(obj, entity) => (obj, Some(entity)),
        };
        let mut bytes = object.as_bytes().to_vec();
        if let Some(entity) = entity {
            let (tag, uuid) = match entity {
                MetadataEntity::Object => (0, uuid::Uuid::nil()),
                MetadataEntity::Property(uuid) => (1, uuid),
                MetadataEntity::Verb(uuid) => (2, uuid),
            };
            bytes.push(tag);
            bytes.extend_from_slice(uuid.as_bytes());
        }
        Some(bytes)
    }
}

pub(crate) struct FjallReader<K, V> {
    snapshot: std::sync::Arc<super::backend::FjallReadSnapshot>,
    keyspace: fjall::Keyspace,
    types: PhantomData<(K, V)>,
}
impl<K, V> FjallReader<K, V> {
    pub(crate) fn new(
        snapshot: std::sync::Arc<super::backend::FjallReadSnapshot>,
        keyspace: fjall::Keyspace,
    ) -> Self {
        Self {
            snapshot,
            keyspace,
            types: PhantomData,
        }
    }
}
impl<K, V> RelationReader<K, V> for FjallReader<K, V>
where
    K: FjallReadKey,
    V: Send + Sync + 'static,
    FjallCodec: EncodeFor<K, Stored = ByteView> + EncodeFor<V, Stored = ByteView>,
{
    fn get(&self, key: &K) -> Result<Option<(Timestamp, V)>, Error> {
        self.snapshot
            .get(&self.keyspace, FjallCodec.encode(key)?)
            .map_err(|error| Error::RetrievalFailure(error.to_string()))?
            .map(decode_fjall_value)
            .transpose()
    }
    fn scan(&self, request: K::Scan) -> Result<TupleCursor<K, V>, Error> {
        let iter = match K::prefix(request) {
            Some(prefix) => self.snapshot.prefix(&self.keyspace, prefix),
            None => self.snapshot.iter(&self.keyspace),
        };
        let lease = self.snapshot.clone();
        Ok(Box::new(iter.map(move |entry| {
            let _lease = &lease;
            let (key, value) = entry
                .into_inner()
                .map_err(|error| Error::RetrievalFailure(error.to_string()))?;
            let key = FjallCodec.decode(ByteView::from(key))?;
            let (timestamp, value) = decode_fjall_value(value)?;
            Ok((timestamp, key, value))
        })))
    }
}
pub(crate) struct FjallPropertyReader {
    snapshot: std::sync::Arc<super::backend::FjallReadSnapshot>,
    keyspace: fjall::Keyspace,
}
impl FjallPropertyReader {
    pub(crate) fn new(
        snapshot: std::sync::Arc<super::backend::FjallReadSnapshot>,
        keyspace: fjall::Keyspace,
    ) -> Self {
        Self { snapshot, keyspace }
    }
}
impl RelationReader<ObjAndUUIDHolder, Var> for FjallPropertyReader {
    fn get(&self, key: &ObjAndUUIDHolder) -> Result<Option<(Timestamp, Var)>, Error> {
        let (start, end) = property_value_record_bounds(key);
        let value = reconstruct_property_value(
            self.snapshot.range(&self.keyspace, start..=end),
            PROPERTY_VALUE_CHAIN_LIMITS,
        )
        .map_err(|error| Error::RetrievalFailure(error.to_string()))?;
        value
            .map(|(stored, value)| {
                if stored != *key {
                    return Err(Error::RetrievalFailure(
                        "Property range returned another key".into(),
                    ));
                }
                Ok((value.logical_timestamp, value.value))
            })
            .transpose()
    }
    fn scan(&self, request: ObjectUuidScan) -> Result<TupleCursor<ObjAndUUIDHolder, Var>, Error> {
        let iter = match ObjAndUUIDHolder::prefix(request) {
            Some(prefix) => self.snapshot.prefix(&self.keyspace, prefix),
            None => self.snapshot.iter(&self.keyspace),
        };
        let lease = self.snapshot.clone();
        Ok(Box::new(
            PropertyValueScan::new(iter, PROPERTY_VALUE_CHAIN_LIMITS).map(move |entry| {
                let _lease = &lease;
                let (key, value) =
                    entry.map_err(|error| Error::RetrievalFailure(error.to_string()))?;
                Ok((value.logical_timestamp, key, value.value))
            }),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DatabaseConfig, StringHolder,
        engine::moor_db::Relations,
        provider::{
            Provider,
            backend::FjallReadSnapshot,
            fjall_relations::FjallRelations,
            property_value_store::{
                encode_full_record, encode_list_append_record, encode_property_value_record_key,
            },
        },
    };
    use moor_var::{Symbol, v_int, v_list};
    use std::sync::Arc;
    use uuid::Uuid;

    fn lease(
        database: &fjall::Database,
        directory: Arc<Option<tempfile::TempDir>>,
    ) -> Arc<FjallReadSnapshot> {
        Arc::new(FjallReadSnapshot {
            snapshot: database.snapshot(),
            _directory: directory,
        })
    }

    #[test]
    fn seed_and_export_readers_share_the_openers_snapshot() {
        let directory = Arc::new(Some(tempfile::TempDir::new().unwrap()));
        let path = directory.as_ref().as_ref().unwrap().path();
        let database = fjall::Database::builder(path).open().unwrap();
        let storage = FjallRelations::open(&database, &DatabaseConfig::default(), path).unwrap();
        let object = Obj::mk_id(1);
        storage
            .object_name
            .put(Timestamp(7), &object, &StringHolder("before".into()))
            .unwrap();
        storage
            .object_parent
            .put(Timestamp(7), &object, &Obj::mk_id(0))
            .unwrap();
        let sequences = database
            .keyspace("sequences", fjall::KeyspaceCreateOptions::default)
            .unwrap();
        sequences
            .insert(0_usize.to_le_bytes(), 7_i64.to_le_bytes())
            .unwrap();
        let property = ObjAndUUIDHolder::new(&object, Uuid::from_u128(1));
        let partition = storage.object_propvalues.partition();
        let mut builder = planus::Builder::new();
        partition
            .insert(
                encode_property_value_record_key(&property, 1),
                encode_full_record(&mut builder, &v_list(&[v_int(1)]), Timestamp(7)).unwrap(),
            )
            .unwrap();
        let snapshot = lease(&database, directory.clone());
        storage
            .object_name
            .put(Timestamp(9), &object, &StringHolder("after".into()))
            .unwrap();
        storage
            .object_parent
            .put(Timestamp(9), &object, &Obj::mk_id(8))
            .unwrap();
        sequences
            .insert(0_usize.to_le_bytes(), 9_i64.to_le_bytes())
            .unwrap();
        partition
            .insert(
                encode_property_value_record_key(&property, 2),
                encode_list_append_record(
                    &mut builder,
                    &moor_var::List::from_iter([v_int(2)]),
                    Timestamp(9),
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            crate::provider::backend::read_sequences(&snapshot.snapshot, &sequences, path).unwrap()
                [0],
            7
        );
        let (root, chains) = storage.seed(&Relations::init(), &snapshot, path).unwrap();
        assert_eq!(
            chains[&property].record_versions().collect::<Vec<_>>(),
            vec![1]
        );
        assert_eq!(chains[&property].append_bytes(), 0);
        assert_eq!(
            root.object_propvalues
                .index_lookup(&property)
                .unwrap()
                .value,
            v_list(&[v_int(1)])
        );
        assert_eq!(root.committed_ts, Timestamp(7));
        assert_eq!(
            root.object_name.index_lookup(&object).unwrap().value.0,
            "before"
        );
        assert_eq!(
            root.object_parent.index_lookup(&object).unwrap().value,
            Obj::mk_id(0)
        );
        assert!(root.object_name.is_fully_resident());
        let readers = storage.readers(&snapshot);
        assert_eq!(
            readers.object_propvalues.get(&property).unwrap(),
            Some((Timestamp(7), v_list(&[v_int(1)])))
        );
        assert_eq!(
            readers.object_name.get(&object).unwrap().unwrap().1.0,
            "before"
        );
        assert_eq!(
            readers
                .object_parent
                .scan(ObjectScan::All)
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .2,
            Obj::mk_id(0)
        );
    }

    #[test]
    fn typed_prefixes_exclude_other_objects_and_entities() {
        let directory = Arc::new(Some(tempfile::TempDir::new().unwrap()));
        let path = directory.as_ref().as_ref().unwrap().path();
        let database = fjall::Database::builder(path).open().unwrap();
        let storage = FjallRelations::open(&database, &DatabaseConfig::default(), path).unwrap();
        let object = Obj::mk_id(1);
        let other = Obj::mk_id(257);
        let uuid = Uuid::from_u128(1);
        let uuid2 = Uuid::from_u128(2);
        for (index, key) in [
            EntityMetadataKey::object(object, Symbol::mk("name")),
            EntityMetadataKey::property(object, uuid, Symbol::mk("name")),
            EntityMetadataKey::property(object, uuid, Symbol::mk("note")),
            EntityMetadataKey::property(object, uuid2, Symbol::mk("name")),
            EntityMetadataKey::verb(object, uuid, Symbol::mk("name")),
            EntityMetadataKey::property(other, uuid, Symbol::mk("name")),
        ]
        .into_iter()
        .enumerate()
        {
            storage
                .entity_metadata
                .put(Timestamp(1), &key, &v_int(index as i64))
                .unwrap();
        }
        let readers = storage.readers(&lease(&database, directory.clone()));
        let rows = readers
            .entity_metadata
            .scan(MetadataScan::Entity(object, MetadataEntity::Property(uuid)))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows.iter()
                .map(|(_, _, value)| value.clone())
                .collect::<Vec<_>>(),
            vec![v_int(1), v_int(2)]
        );
        assert_eq!(
            readers
                .entity_metadata
                .scan(MetadataScan::Object(object))
                .unwrap()
                .count(),
            5
        );
        assert_eq!(
            readers
                .entity_metadata
                .scan(MetadataScan::Entity(object, MetadataEntity::Object))
                .unwrap()
                .count(),
            1
        );
        assert_eq!(
            readers
                .entity_metadata
                .scan(MetadataScan::Entity(object, MetadataEntity::Verb(uuid)))
                .unwrap()
                .count(),
            1
        );
    }

    #[test]
    fn property_cursor_reconstructs_one_chain_at_a_time_and_retains_its_lease() {
        let directory = Arc::new(Some(tempfile::TempDir::new().unwrap()));
        let path = directory.as_ref().as_ref().unwrap().path().to_path_buf();
        let database = fjall::Database::builder(&path).open().unwrap();
        let storage = FjallRelations::open(&database, &DatabaseConfig::default(), &path).unwrap();
        let object = Obj::mk_id(1);
        let key = ObjAndUUIDHolder::new(&object, Uuid::from_u128(1));
        let corrupt = ObjAndUUIDHolder::new(&object, Uuid::from_u128(2));
        let mut builder = planus::Builder::new();
        let partition = storage.object_propvalues.partition();
        partition
            .insert(
                encode_property_value_record_key(&key, 1),
                encode_full_record(&mut builder, &v_list(&[v_int(1)]), Timestamp(3)).unwrap(),
            )
            .unwrap();
        partition
            .insert(
                encode_property_value_record_key(&key, 2),
                encode_list_append_record(
                    &mut builder,
                    &moor_var::List::from_iter([v_int(2)]),
                    Timestamp(4),
                )
                .unwrap(),
            )
            .unwrap();
        partition
            .insert(encode_property_value_record_key(&corrupt, 1), b"corrupt")
            .unwrap();
        let reader =
            FjallPropertyReader::new(lease(&database, directory.clone()), partition.clone());
        assert_eq!(
            reader.get(&key).unwrap(),
            Some((Timestamp(4), v_list(&[v_int(1), v_int(2)])))
        );
        assert!(
            reader
                .scan(ObjectUuidScan::Object(Obj::mk_id(2)))
                .unwrap()
                .next()
                .is_none()
        );
        let mut cursor = reader.scan(ObjectUuidScan::Object(object)).unwrap();
        drop(reader);
        drop(storage);
        drop(database);
        drop(directory);
        assert!(path.exists());
        assert_eq!(
            cursor.next().unwrap().unwrap(),
            (Timestamp(4), key, v_list(&[v_int(1), v_int(2)]))
        );
        assert!(cursor.next().unwrap().is_err());
        drop(cursor);
        assert!(!path.exists());
    }

    #[test]
    fn ordinary_cursor_does_not_eagerly_decode_later_rows() {
        let directory = Arc::new(Some(tempfile::TempDir::new().unwrap()));
        let path = directory.as_ref().as_ref().unwrap().path();
        let database = fjall::Database::builder(path).open().unwrap();
        let storage = FjallRelations::open(&database, &DatabaseConfig::default(), path).unwrap();
        storage
            .object_parent
            .put(Timestamp(1), &Obj::mk_id(1), &Obj::mk_id(0))
            .unwrap();
        storage
            .object_parent
            .partition()
            .insert(FjallCodec.encode(&Obj::mk_id(2)).unwrap(), b"bad")
            .unwrap();
        let reader = FjallReader::<Obj, Obj>::new(
            lease(&database, directory.clone()),
            storage.object_parent.partition().clone(),
        );
        let mut cursor = reader.scan(ObjectScan::All).unwrap();
        assert_eq!(cursor.next().unwrap().unwrap().1, Obj::mk_id(1));
        assert!(cursor.next().unwrap().is_err());
    }
}
