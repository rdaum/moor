// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use strum::{AsRefStr, Display, EnumCount, EnumIter, EnumProperty};
use uuid::Uuid;

use moor_values::model::ObjSet;
use moor_values::model::WorldStateError;
use moor_values::util::SliceRef;
use moor_values::var::Objid;
use moor_values::AsByteBuffer;

use crate::rdb::RelationError;
use crate::rdb::{RelationId, Transaction};

/// The set of binary relations that are used to represent the world state in the moor system.
#[repr(usize)]
#[derive(
    Copy, Clone, Debug, Eq, PartialEq, EnumIter, EnumCount, Display, EnumProperty, AsRefStr,
)]
pub enum WorldStateRelation {
    /// Object<->Parent
    #[strum(props(
        DomainType = "Integer",
        CodomainType = "Integer",
        SecondaryIndexed = "true",
        IndexType = "Hash",
        SecondaryIndexType = "Hash",
    ))]
    ObjectParent = 0,
    /// Object<->Location
    #[strum(props(
        DomainType = "Integer",
        CodomainType = "Integer",
        SecondaryIndexed = "true",
        IndexType = "Hash",
        SecondaryIndexType = "Hash",
    ))]
    ObjectLocation = 1,
    /// Object->Flags (BitEnum<ObjFlag>)
    #[strum(props(DomainType = "Integer", CodomainType = "Bytes", IndexType = "Hash"))]
    ObjectFlags = 2,
    /// Object->Name
    #[strum(props(DomainType = "Integer", CodomainType = "String", IndexType = "Hash"))]
    ObjectName = 3,
    /// Object->Owner
    #[strum(props(DomainType = "Integer", CodomainType = "Integer", IndexType = "Hash"))]
    ObjectOwner = 4,
    /// Object->Verbs (Verbdefs)
    #[strum(props(DomainType = "Integer", CodomainType = "Bytes", IndexType = "Hash"))]
    ObjectVerbs = 5,
    /// Verb UUID->VerbProgram (Binary)
    #[strum(props(DomainType = "Bytes", CodomainType = "Bytes", IndexType = "Hash"))]
    VerbProgram = 6,
    /// Object->Properties (Propdefs)
    #[strum(props(DomainType = "Integer", CodomainType = "Bytes", IndexType = "Hash"))]
    ObjectPropDefs = 7,
    /// Property UUID->PropertyValue (Var)
    #[strum(props(DomainType = "Bytes", CodomainType = "Bytes", IndexType = "Hash"))]
    ObjectPropertyValue = 8,
}

impl From<WorldStateRelation> for RelationId {
    fn from(val: WorldStateRelation) -> Self {
        RelationId(val as usize)
    }
}

pub fn composite_key_for(o: Objid, u: &Uuid) -> SliceRef {
    let mut key = o.0.to_le_bytes().to_vec();
    key.extend_from_slice(u.as_bytes());
    SliceRef::from_vec(key)
}

#[repr(usize)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, EnumIter, EnumCount)]
pub enum WorldStateSequences {
    MaximumObject = 0,
}

/// Just write OIDs out as signed ints.
pub fn encode_oid(oid: Objid) -> SliceRef {
    let bytes = oid.0.to_le_bytes();
    SliceRef::from_vec(bytes.to_vec())
}

fn decode_oid(sr: &SliceRef) -> Objid {
    let bytes = sr.as_slice();
    let oid_i = i64::from_le_bytes(bytes.try_into().expect("Could not decode OID"));
    Objid(oid_i)
}

pub fn upsert_object_value<Codomain: Clone + Eq + PartialEq + AsByteBuffer>(
    tx: &Transaction,
    rel: WorldStateRelation,
    oid: Objid,
    value: Codomain,
) -> Result<(), WorldStateError> {
    let relation = tx.relation(RelationId(rel as usize));
    if let Err(e) = relation.upsert_by_domain(
        encode_oid(oid),
        value.as_sliceref().expect("Could not encode value"),
    ) {
        panic!("Unexpected error: {:?}", e)
    }
    Ok(())
}

pub fn upsert_object_object(
    tx: &Transaction,
    rel: WorldStateRelation,
    oid: Objid,
    value: Objid,
) -> Result<(), WorldStateError> {
    let relation = tx.relation(RelationId(rel as usize));
    if let Err(e) = relation.upsert_by_domain(encode_oid(oid), encode_oid(value)) {
        panic!("Unexpected error: {:?}", e)
    }
    Ok(())
}
#[allow(dead_code)]
pub fn insert_object_value<Codomain: Clone + Eq + PartialEq + AsByteBuffer>(
    tx: &Transaction,
    rel: WorldStateRelation,
    oid: Objid,
    value: Codomain,
) -> Result<(), WorldStateError> {
    let relation = tx.relation(RelationId(rel as usize));
    match relation.insert_tuple(
        encode_oid(oid),
        value.as_sliceref().expect("Could not encode value"),
    ) {
        Ok(_) => Ok(()),
        Err(RelationError::UniqueConstraintViolation) => {
            Err(WorldStateError::DatabaseError("Duplicate key".to_string()))
        }
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[allow(dead_code)]
pub fn insert_object_object(
    tx: &Transaction,
    rel: WorldStateRelation,
    oid: Objid,
    value: Objid,
) -> Result<(), WorldStateError> {
    let relation = tx.relation(RelationId(rel as usize));
    match relation.insert_tuple(encode_oid(oid), encode_oid(value)) {
        Ok(_) => Ok(()),
        Err(RelationError::UniqueConstraintViolation) => {
            Err(WorldStateError::DatabaseError("Duplicate key".to_string()))
        }
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}
/// Full scan an entire relation where the Domain is an Objid, and return the ones matching the
/// predicate.
pub fn get_all_object_keys_matching<P, Codomain>(
    tx: &Transaction,
    rel: WorldStateRelation,
    pred: P,
) -> Result<ObjSet, WorldStateError>
where
    P: Fn(Objid, Codomain) -> bool,
    Codomain: Clone + Eq + PartialEq + AsByteBuffer,
{
    let relation = tx.relation(RelationId(rel as usize));
    let Ok(all_tuples) = relation.predicate_scan(&|t| {
        let oid = Objid::from_sliceref(t.domain()).expect("Could not decode OID");
        pred(
            oid,
            Codomain::from_sliceref(t.codomain()).expect("Could not decode value"),
        )
    }) else {
        return Err(WorldStateError::DatabaseError(
            "Unable to scan relation".to_string(),
        ));
    };
    let objs = all_tuples.into_iter().map(|v| decode_oid(&v.domain()));
    Ok(ObjSet::from_oid_iter(objs))
}

pub fn get_object_object(tx: &Transaction, rel: WorldStateRelation, oid: Objid) -> Option<Objid> {
    let relation = tx.relation(RelationId(rel as usize));
    match relation.seek_unique_by_domain(encode_oid(oid)) {
        Ok(v) => Some(decode_oid(&v.codomain())),
        Err(RelationError::TupleNotFound) => None,
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

pub fn get_object_value<Codomain: Clone + Eq + PartialEq + AsByteBuffer>(
    tx: &Transaction,
    rel: WorldStateRelation,
    oid: Objid,
) -> Option<Codomain> {
    let relation = tx.relation(RelationId(rel as usize));
    match relation.seek_unique_by_domain(encode_oid(oid)) {
        Ok(v) => {
            Some(Codomain::from_sliceref(v.codomain()).expect("Could not decode codomain value"))
        }
        Err(RelationError::TupleNotFound) => None,
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

pub fn tuple_size_for_object_domain(
    tx: &Transaction,
    rel: WorldStateRelation,
    oid: Objid,
) -> Option<usize> {
    let relation = tx.relation(RelationId(rel as usize));
    match relation.seek_unique_by_domain(encode_oid(oid)) {
        Ok(t) => Some(t.slot_buffer().len()),
        Err(RelationError::TupleNotFound) => None,
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[allow(dead_code)]
pub fn get_object_by_object_codomain(
    tx: &Transaction,
    rel: WorldStateRelation,
    codomain: Objid,
) -> Objid {
    let relation = tx.relation(RelationId(rel as usize));
    let result = relation.seek_by_codomain(encode_oid(codomain));
    let objs = result
        .expect("Unable to seek by codomain")
        .into_iter()
        .map(|v| decode_oid(&v.domain()));
    if objs.len() != 1 {
        panic!("Expected exactly one object to match the codomain");
    }
    objs.into_iter().next().unwrap()
}

pub fn get_objects_by_object_codomain(
    tx: &Transaction,
    rel: WorldStateRelation,
    codomain: Objid,
) -> ObjSet {
    let relation = tx.relation(RelationId(rel as usize));
    let result = relation.seek_by_codomain(encode_oid(codomain));
    let objs = result
        .expect("Unable to seek by codomain")
        .into_iter()
        .map(|v| decode_oid(&v.domain()));
    ObjSet::from_oid_iter(objs)
}

pub fn tuple_size_for_object_codomain(
    tx: &Transaction,
    rel: WorldStateRelation,
    oid: Objid,
) -> Option<usize> {
    let relation = tx.relation(RelationId(rel as usize));
    match relation.seek_by_codomain(encode_oid(oid)) {
        Ok(ts) => Some(ts.iter().map(|t| t.slot_buffer().len()).sum()),
        Err(RelationError::TupleNotFound) => None,
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}
pub fn get_composite_value<Codomain: Clone + Eq + PartialEq + AsByteBuffer>(
    tx: &Transaction,
    rel: WorldStateRelation,
    oid: Objid,
    uuid: Uuid,
) -> Option<Codomain> {
    let key_bytes = composite_key_for(oid, &uuid);
    let relation = tx.relation(RelationId(rel as usize));
    match relation.seek_unique_by_domain(key_bytes) {
        Ok(v) => {
            Some(Codomain::from_sliceref(v.codomain()).expect("Could not decode codomain value"))
        }
        Err(RelationError::TupleNotFound) => None,
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

pub fn tuple_size_composite(
    tx: &Transaction,
    rel: WorldStateRelation,
    oid: Objid,
    uuid: Uuid,
) -> Option<usize> {
    let key_bytes = composite_key_for(oid, &uuid);
    let relation = tx.relation(RelationId(rel as usize));
    match relation.seek_unique_by_domain(key_bytes) {
        Ok(t) => Some(t.slot_buffer().len()),
        Err(RelationError::TupleNotFound) => None,
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[allow(dead_code)]
fn insert_composite_value<Codomain: Clone + Eq + PartialEq + AsByteBuffer>(
    tx: &Transaction,
    rel: WorldStateRelation,
    oid: Objid,
    uuid: Uuid,
    value: Codomain,
) -> Result<(), WorldStateError> {
    let key_bytes = composite_key_for(oid, &uuid);
    let relation = tx.relation(RelationId(rel as usize));
    match relation.insert_tuple(
        key_bytes,
        value.as_sliceref().expect("Could not encode value"),
    ) {
        Ok(_) => Ok(()),
        Err(RelationError::UniqueConstraintViolation) => Err(WorldStateError::ObjectNotFound(oid)),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[allow(dead_code)]
fn delete_if_exists(
    tx: &Transaction,
    rel: WorldStateRelation,
    oid: Objid,
) -> Result<(), WorldStateError> {
    let relation = tx.relation(RelationId(rel as usize));
    match relation.remove_by_domain(encode_oid(oid)) {
        Ok(_) => Ok(()),
        Err(RelationError::TupleNotFound) => Ok(()),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

pub fn delete_composite_if_exists(
    tx: &Transaction,
    rel: WorldStateRelation,
    oid: Objid,
    uuid: Uuid,
) -> Result<(), WorldStateError> {
    let key_bytes = composite_key_for(oid, &uuid);
    let relation = tx.relation(RelationId(rel as usize));
    match relation.remove_by_domain(key_bytes) {
        Ok(_) => Ok(()),
        Err(RelationError::TupleNotFound) => Ok(()),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

pub fn upsert_obj_uuid_value<Codomain: Clone + Eq + PartialEq + AsByteBuffer>(
    tx: &Transaction,
    rel: WorldStateRelation,
    oid: Objid,
    uuid: Uuid,
    value: Codomain,
) -> Result<(), WorldStateError> {
    let key_bytes = composite_key_for(oid, &uuid);
    let relation = tx.relation(RelationId(rel as usize));
    if let Err(e) = relation.upsert_by_domain(
        key_bytes,
        value.as_sliceref().expect("Could not encode value"),
    ) {
        panic!("Unexpected error: {:?}", e)
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use strum::{EnumCount, IntoEnumIterator};

    use moor_values::model::ObjSet;
    use moor_values::var::Objid;

    use crate::odb::object_relations::WorldStateRelation::ObjectParent;
    use crate::odb::object_relations::{
        get_object_by_object_codomain, get_object_object, get_objects_by_object_codomain,
        insert_object_object, upsert_object_object, WorldStateRelation, WorldStateSequences,
    };
    use crate::rdb::{relation_info_for, RelBox, RelationInfo};

    fn test_db() -> Arc<RelBox> {
        let relations: Vec<RelationInfo> =
            WorldStateRelation::iter().map(relation_info_for).collect();

        RelBox::new(1 << 24, None, &relations, WorldStateSequences::COUNT)
    }

    /// Test simple relations mapping oid->oid (with secondary index), independent of all other
    /// worldstate voodoo.
    #[test]
    fn test_simple_object() {
        let db = test_db();
        let tx = db.clone().start_tx();
        insert_object_object(&tx, ObjectParent, Objid(3), Objid(2)).unwrap();
        insert_object_object(&tx, ObjectParent, Objid(2), Objid(1)).unwrap();
        insert_object_object(&tx, ObjectParent, Objid(1), Objid(0)).unwrap();

        assert_eq!(
            get_object_object(&tx, ObjectParent, Objid(3)).unwrap(),
            Objid(2)
        );
        assert_eq!(
            get_object_object(&tx, ObjectParent, Objid(2)).unwrap(),
            Objid(1)
        );
        assert_eq!(
            get_object_object(&tx, ObjectParent, Objid(1)).unwrap(),
            Objid(0)
        );

        assert_eq!(
            get_objects_by_object_codomain(&tx, ObjectParent, Objid(3)),
            ObjSet::from(&[])
        );
        assert_eq!(
            get_objects_by_object_codomain(&tx, ObjectParent, Objid(2)),
            ObjSet::from(&[Objid(3)])
        );
        assert_eq!(
            get_objects_by_object_codomain(&tx, ObjectParent, Objid(1)),
            ObjSet::from(&[Objid(2)])
        );
        assert_eq!(
            get_objects_by_object_codomain(&tx, ObjectParent, Objid(0)),
            ObjSet::from(&[Objid(1)])
        );

        // Now commit and re-verify.
        tx.commit().unwrap();
        let tx = db.clone().start_tx();

        assert_eq!(
            get_object_object(&tx, ObjectParent, Objid(3)).unwrap(),
            Objid(2)
        );
        assert_eq!(
            get_object_object(&tx, ObjectParent, Objid(2)).unwrap(),
            Objid(1)
        );
        assert_eq!(
            get_object_object(&tx, ObjectParent, Objid(1)).unwrap(),
            Objid(0)
        );

        assert_eq!(
            get_objects_by_object_codomain(&tx, ObjectParent, Objid(3)),
            ObjSet::from(&[])
        );
        assert_eq!(
            get_objects_by_object_codomain(&tx, ObjectParent, Objid(2)),
            ObjSet::from(&[Objid(3)])
        );
        assert_eq!(
            get_objects_by_object_codomain(&tx, ObjectParent, Objid(1)),
            ObjSet::from(&[Objid(2)])
        );
        assert_eq!(
            get_objects_by_object_codomain(&tx, ObjectParent, Objid(0)),
            ObjSet::from(&[Objid(1)])
        );

        // And then update a value and verify.
        upsert_object_object(&tx, ObjectParent, Objid(1), Objid(2)).unwrap();
        assert_eq!(
            get_object_by_object_codomain(&tx, ObjectParent, Objid(1)),
            Objid(2)
        );
        // Verify that the secondary index is updated... First check for new value.
        let children = get_objects_by_object_codomain(&tx, ObjectParent, Objid(2));
        assert_eq!(children.len(), 2);
        assert!(
            children.contains(Objid(1)),
            "Expected children of 2 to contain 1"
        );
        assert!(
            !children.contains(Objid(0)),
            "Expected children of 2 to not contain 0"
        );
        // Now check the old value.
        let children = get_objects_by_object_codomain(&tx, ObjectParent, Objid(0));
        assert_eq!(children, ObjSet::from(&[]));
    }
}
