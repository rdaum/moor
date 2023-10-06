//! Implements the phys-DB world-state backend, but for RocksDB.
//! This implementation spawns a separate thread per transaction, and uses RocksDB's
//! OptimisticTransaction mechanism to provide isolation.
//! Performance on the whole isn't great.
//! Prefer the in-memory implementation in TupleBox unless your world exceeds main-memory size.

use strum::{EnumString, EnumVariantNames};

pub mod db_server;
mod tx_db_impl;
mod tx_db_impl_objects;
mod tx_db_impl_props;
mod tx_db_impl_verbs;
pub mod tx_server;

#[derive(Debug, PartialEq, EnumString, EnumVariantNames)]
#[repr(u8)]
pub(crate) enum ColumnFamilies {
    // Incrementing current object id. TODO: exterminate
    ObjectIds,

    // Object->Parent
    ObjectParent,
    // Object->Children (ObjSet)
    ObjectChildren,
    // Object->Location
    ObjectLocation,
    // Object->Contents (ObjSet)
    ObjectContents,
    // Object->Flags (BitEnum<ObjFlag>)
    ObjectFlags,
    // Object->Name
    ObjectName,
    // Object->Owner
    ObjectOwner,

    // Object->Verbs (Vec<VerbHandle>)
    ObjectVerbs,
    // Verb UUID->VerbProgram (Binary)
    VerbProgram,

    // Object->Properties (Vec<PropHandle>)
    ObjectPropDefs,
    // Property UUID->PropertyValue (Var)
    ObjectPropertyValue,
}
