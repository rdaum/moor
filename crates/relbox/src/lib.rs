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

//! In-memory database that provides transactional consistency through copy-on-write maps
//! Base relations are `im` hashmaps -- persistent / functional / copy-on-writish hashmaps, which
//! transactions obtain a fork of from `canonical`. At commit timestamps are checked and reconciled
//! if possible, and the whole set of relations is swapped out for the set of modified tuples.
//!
//! The tuples themselves are written out at commit time to a backing store, and then re-read at
//! system initialization.
//!
//! TLDR Transactions continue to see a fully snapshot isolated view of the world.

pub use index::AttrType;
pub use index::IndexType;
pub use relbox::{RelBox, RelationInfo};
use std::fmt::Display;
use std::str::FromStr;
use strum::EnumProperty;
use thiserror::Error;
pub use tx::{CommitError, Transaction};

mod base_relation;
mod paging;
mod pool;
mod relbox;
mod tuples;
mod tx;

// Note: this is 'pub' just to shut dead_code compiler warnings up,
// and can be removed once the ART index is actually being used
pub mod index;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct RelationId(pub usize);

impl RelationId {
    // If the top bit (63rd) bit is not set, then this is a base relation.
    pub fn is_base_relation(&self) -> bool {
        self.0 & (1 << 63) == 0
    }
    pub fn is_transient_relation(&self) -> bool {
        !self.is_base_relation()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Error)]
pub enum RelationError {
    #[error("Tuple not found")]
    TupleNotFound,
    #[error("Tuple already exists for unique domain value")]
    UniqueConstraintViolation,
    #[error("Ambiguous tuple found; more than one tuple found for presumed-unique domain value")]
    AmbiguousTuple,
    #[error("Invalid key type")]
    BadKey,
}

/// Convert an enum schema description into RelationInfo (see WorldStateRelation for example)
pub fn relation_info_for<E: EnumProperty + Display>(relation: E) -> RelationInfo {
    let domain_type = relation
        .get_str("DomainType")
        .unwrap_or_else(|| panic!("DomainType not found for declared relation {}", relation));
    let codomain_type = relation
        .get_str("CodomainType")
        .unwrap_or_else(|| panic!("CodomainType not found for declared relation {}", relation));

    let domain_type = AttrType::from_str(domain_type).unwrap_or_else(|_| {
        panic!(
            "DomainType {} invalid for declared relation {}",
            domain_type, relation
        )
    });
    let codomain_type = AttrType::from_str(codomain_type).unwrap_or_else(|_| {
        panic!(
            "CodomainType {} invalid for declared relation {}",
            codomain_type, relation
        )
    });
    let secondary_indexed = relation
        .get_str("SecondaryIndexed")
        .map(|it| it == "true")
        .unwrap_or(false);

    let index_type = relation
        .get_str("IndexType")
        .map(|it| {
            IndexType::from_str(it).unwrap_or_else(|_| {
                panic!(
                    "Invalid index type: {} for declared relation {}",
                    it, relation
                )
            })
        })
        .unwrap_or(IndexType::Hash);

    let codomain_index_type = if secondary_indexed {
        Some(
            relation
                .get_str("CodomainIndexType")
                .map(|it| {
                    IndexType::from_str(it).unwrap_or_else(|_| {
                        panic!(
                            "Invalid index type: {} for declared relation {}",
                            it, relation
                        )
                    })
                })
                .unwrap_or(IndexType::Hash),
        )
    } else {
        None
    };

    RelationInfo {
        name: relation.to_string(),
        domain_type,
        codomain_type,
        secondary_indexed,
        unique_domain: true,
        index_type,
        codomain_index_type,
    }
}
