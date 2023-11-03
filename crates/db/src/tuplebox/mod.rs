//! In-memory database that provides transactional consistency through copy-on-write maps
//! Base relations are `im` hashmaps -- persistent / functional / copy-on-writish hashmaps, which
//! transactions obtain a fork of from `canonical`. At commit timestamps are checked and reconciled
//! if possible, and the whole set of relations is swapped out for the set of modified tuples.
//!
//! The tuples themselves are written out at commit time to a backing store, and then re-read at
//! system initialization.
//!
//! TLDR Transactions continue to see a fully snapshot isolated view of the world.

mod backing;
mod base_relation;

pub mod rocks_backing;
mod slots;
pub mod tb;
mod tuples;
mod tx;

pub use tuples::TupleError;
pub use tx::transaction::{CommitError, Transaction};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct RelationId(pub usize);

impl RelationId {
    pub fn transient(id: usize) -> Self {
        RelationId(id | (1 << 63))
    }

    // If the top bit (63rd) bit is not set, then this is a base relation.
    pub fn is_base_relation(&self) -> bool {
        self.0 & (1 << 63) == 0
    }
    pub fn is_transient_relation(&self) -> bool {
        !self.is_base_relation()
    }
}
