use crate::tuplebox::transaction::{Transaction, TupleError};
use crate::tuplebox::RelationId;
use moor_values::model::objset::ObjSet;
use moor_values::model::WorldStateError;
use moor_values::util::slice_ref::SliceRef;
use moor_values::var::objid::Objid;
use moor_values::AsByteBuffer;
use strum::{Display, EnumCount, EnumIter};
use uuid::Uuid;

/// The set of binary relations that are used to represent the world state in the moor system.
#[repr(usize)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, EnumIter, EnumCount, Display)]
pub enum WorldStateRelation {
    /// Object<->Parent
    ObjectParent = 0,
    /// Object<->Location
    ObjectLocation = 1,
    /// Object->Flags (BitEnum<ObjFlag>)
    ObjectFlags = 2,
    /// Object->Name
    ObjectName = 3,
    /// Object->Owner
    ObjectOwner = 4,

    /// Object->Verbs (Verbdefs)
    ObjectVerbs = 5,
    /// Verb UUID->VerbProgram (Binary)
    VerbProgram = 6,

    /// Object->Properties (Propdefs)
    ObjectPropDefs = 7,
    /// Property UUID->PropertyValue (Var)
    ObjectPropertyValue = 8,
}

impl Into<RelationId> for WorldStateRelation {
    fn into(self) -> RelationId {
        RelationId(self as usize)
    }
}

#[repr(usize)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, EnumIter, EnumCount)]
pub enum WorldStateSequences {
    MaximumObject = 0,
}

pub async fn upsert_object_value<Codomain: Clone + Eq + PartialEq + AsByteBuffer>(
    tx: &Transaction,
    rel: WorldStateRelation,
    oid: Objid,
    value: Codomain,
) -> Result<(), WorldStateError> {
    let key_bytes = oid.0.to_le_bytes();

    // TODO: copy might not be needed here.
    let value = SliceRef::from_vec(value.make_copy_as_vec());

    let relation = tx.relation(RelationId(rel as usize)).await;
    if let Err(e) = relation.upsert_tuple(&key_bytes, value).await {
        panic!("Unexpected error: {:?}", e)
    }
    Ok(())
}

#[allow(dead_code)]
pub async fn insert_object_value<Codomain: Clone + Eq + PartialEq + AsByteBuffer>(
    tx: &Transaction,
    rel: WorldStateRelation,
    oid: Objid,
    value: Codomain,
) -> Result<(), WorldStateError> {
    let key_bytes = oid.0.to_le_bytes();
    let value = SliceRef::from_vec(value.make_copy_as_vec());
    let relation = tx.relation(RelationId(rel as usize)).await;
    match relation.insert_tuple(&key_bytes, value).await {
        Ok(_) => Ok(()),
        Err(TupleError::Duplicate) => {
            Err(WorldStateError::DatabaseError("Duplicate key".to_string()))
        }
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

pub async fn get_object_value<Codomain: Clone + Eq + PartialEq + AsByteBuffer>(
    tx: &Transaction,
    rel: WorldStateRelation,
    oid: Objid,
) -> Option<Codomain> {
    let key_bytes = oid.0.to_le_bytes();
    let relation = tx.relation(RelationId(rel as usize)).await;
    match relation.seek_by_domain(&key_bytes).await {
        Ok(v) => Some(Codomain::from_sliceref(v)),
        Err(TupleError::NotFound) => None,
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

pub async fn get_object_codomain<Codomain: Clone + Eq + PartialEq + AsByteBuffer>(
    tx: &Transaction,
    rel: WorldStateRelation,
    codomain: Codomain,
) -> ObjSet {
    let relation = tx.relation(RelationId(rel as usize)).await;
    let objs = relation
        .seek_by_codomain(&codomain.make_copy_as_vec())
        .await
        .expect("Unable to seek by codomain")
        .into_iter()
        .map(|v| {
            let oid = i64::from_le_bytes(v[0..8].try_into().unwrap());
            Objid(oid)
        });
    ObjSet::from_oid_iter(objs)
}

pub async fn get_composite_value<Codomain: Clone + Eq + PartialEq + AsByteBuffer>(
    tx: &Transaction,
    rel: WorldStateRelation,
    oid: Objid,
    uuid: Uuid,
) -> Option<Codomain> {
    let key_bytes = composite_key_for(oid, &uuid);
    let relation = tx.relation(RelationId(rel as usize)).await;
    match relation.seek_by_domain(&key_bytes).await {
        Ok(v) => Some(Codomain::from_sliceref(v)),
        Err(TupleError::NotFound) => None,
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[allow(dead_code)]
async fn insert_composite_value<Codomain: Clone + Eq + PartialEq + AsByteBuffer>(
    tx: &Transaction,
    rel: WorldStateRelation,
    oid: Objid,
    uuid: Uuid,
    value: Codomain,
) -> Result<(), WorldStateError> {
    let key_bytes = composite_key_for(oid, &uuid);
    let value = SliceRef::from_vec(value.make_copy_as_vec());
    let relation = tx.relation(RelationId(rel as usize)).await;
    match relation.insert_tuple(&key_bytes, value).await {
        Ok(_) => Ok(()),
        Err(TupleError::Duplicate) => Err(WorldStateError::ObjectNotFound(oid)),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[allow(dead_code)]
async fn delete_if_exists<Codomain: Clone + Eq + PartialEq + AsByteBuffer>(
    tx: &Transaction,
    rel: WorldStateRelation,
    oid: Objid,
) -> Result<(), WorldStateError> {
    let key_bytes = oid.0.to_le_bytes();
    let relation = tx.relation(RelationId(rel as usize)).await;
    match relation.remove_by_domain(&key_bytes).await {
        Ok(_) => Ok(()),
        Err(TupleError::NotFound) => Ok(()),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

pub async fn delete_composite_if_exists<Codomain: Clone + Eq + PartialEq + AsByteBuffer>(
    tx: &Transaction,
    rel: WorldStateRelation,
    oid: Objid,
    uuid: Uuid,
) -> Result<(), WorldStateError> {
    let key_bytes = composite_key_for(oid, &uuid);
    let relation = tx.relation(RelationId(rel as usize)).await;
    match relation.remove_by_domain(&key_bytes).await {
        Ok(_) => Ok(()),
        Err(TupleError::NotFound) => Ok(()),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

pub async fn upsert_obj_uuid_value<Codomain: Clone + Eq + PartialEq + AsByteBuffer>(
    tx: &Transaction,
    rel: WorldStateRelation,
    oid: Objid,
    uuid: Uuid,
    value: Codomain,
) -> Result<(), WorldStateError> {
    let key_bytes = composite_key_for(oid, &uuid);
    let value = SliceRef::from_vec(value.make_copy_as_vec());
    let relation = tx.relation(RelationId(rel as usize)).await;
    if let Err(e) = relation.upsert_tuple(&key_bytes, value).await {
        panic!("Unexpected error: {:?}", e)
    }
    Ok(())
}

pub fn composite_key_for(o: Objid, u: &Uuid) -> Vec<u8> {
    let mut key = o.0.to_le_bytes().to_vec();
    key.extend_from_slice(u.as_bytes());
    key
}
