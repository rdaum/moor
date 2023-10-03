use crate::inmemtransient::transaction::{Transaction, TupleError};
use moor_values::model::WorldStateError;
use moor_values::util::slice_ref::SliceRef;
use moor_values::var::objid::Objid;
use moor_values::AsByteBuffer;
use strum::{EnumCount, EnumIter};
use uuid::Uuid;

/// The set of binary relations that are used to represent the world state in the moor system.
#[repr(usize)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, EnumIter, EnumCount)]
pub enum WorldStateRelation {
    /// Object->Parent
    ObjectParent = 0,
    /// Object->Children (ObjSet)
    // TODO: this could be re-formulated using a secondary index on the ObjectParent relation.
    ObjectChildren = 1,
    /// Object->Location
    ObjectLocation = 2,
    /// Object->Contents (ObjSet)
    // TODO: this could be re-formulated using a secondary index on the ObjectLocation relation.
    ObjectContents = 3,
    /// Object->Flags (BitEnum<ObjFlag>)
    ObjectFlags = 4,
    /// Object->Name
    ObjectName = 5,
    /// Object->Owner
    ObjectOwner = 6,

    /// Object->Verbs (Verbdefs)
    ObjectVerbs = 7,
    /// Verb UUID->VerbProgram (Binary)
    VerbProgram = 8,

    /// Object->Properties (Propdefs)
    ObjectPropDefs = 9,
    /// Property UUID->PropertyValue (Var)
    ObjectPropertyValue = 10,
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

    if let Err(e) = tx.upsert_tuple(rel as usize, &key_bytes, value).await {
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

    match tx.insert_tuple(rel as usize, &key_bytes, value).await {
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
    match tx.get_tuple(rel as usize, &key_bytes).await {
        Ok(v) => Some(Codomain::from_sliceref(v)),
        Err(TupleError::NotFound) => None,
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

pub async fn get_composite_value<Codomain: Clone + Eq + PartialEq + AsByteBuffer>(
    tx: &Transaction,
    rel: WorldStateRelation,
    oid: Objid,
    uuid: Uuid,
) -> Option<Codomain> {
    let key_bytes = composite_key_for(oid, &uuid);
    match tx.get_tuple(rel as usize, &key_bytes).await {
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

    match tx.insert_tuple(rel as usize, &key_bytes, value).await {
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
    match tx.delete_tuple(rel as usize, &key_bytes).await {
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
    match tx.delete_tuple(rel as usize, &key_bytes).await {
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

    if let Err(e) = tx.upsert_tuple(rel as usize, &key_bytes, value).await {
        panic!("Unexpected error: {:?}", e)
    }
    Ok(())
}

pub fn composite_key_for(o: Objid, u: &Uuid) -> Vec<u8> {
    let mut key = o.0.to_le_bytes().to_vec();
    key.extend_from_slice(u.as_bytes());
    key
}
