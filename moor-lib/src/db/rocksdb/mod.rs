/// Implements the phys-DB backend, but for RocksDB.
/// This is essentially a stand-in for now until I have a better solution, because Rocks has a
/// number of disadvantages that I need to deal with.
///     * As an LSM-based system it's primarily designed for write-heavy workloads, not the
///       read heavy workloads that we have.
///     * But even for writes, multiple writers must block.
///     * It's not a good fit for the kind of data we have, which is mostly small, random-access
///       bits and pieces.
///     * The 0-copy solution through DBPinnableSlice requires leaking RocksDB details all the way
///       up the stack, because primarily of the way the bindings are written.
///
/// Advantages over Sled, which is some respects technically superior:
///     * Sled's Transaction pieces require execution within a closure, and it performs the retries
///       automatically, rather than letting me do it. This is a problem because I need to be able
///       to retry at a higher level, in the VM / scheduler, so I can manage the other side-effects
///       and operational aspects. Sled really wants to be "in charge" of the transaction, and I
///       can't countenance that.
///     * Sled's development is spotty, it appears to be in the midst of a major rewrite, and
///       I don't know how that's going to shake out.
///     * Its IVec type has the same issue as the Rocks DBPinnableSlice stuff.
///
/// There are other options: lmdb, redb, etc but they all have similar kinds of issues.
///
/// The likely final option is to build my own, perhaps based on sanakirja's btree implementation.
/// But I don't want to do that until I have a better idea of what the final process model is going
/// to look like.
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
