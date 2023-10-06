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
mod object_relations;
pub mod rocks_backing;
pub mod tb;
pub mod tb_worldstate;

mod transaction;
