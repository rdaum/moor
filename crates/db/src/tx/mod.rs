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

mod apply;
mod check;
pub(crate) mod commit_bloom;
mod indexes;
mod relation;
mod resolve;
mod transaction;

pub use check::{CheckRelation, PotentialConflict, ProposedOp};
pub use commit_bloom::CommitBloom;
pub use indexes::RelationIndex;
pub use relation::Relation;
pub(crate) use resolve::Resolution;
pub use resolve::{ConflictResolver, FailOnConflict};
pub(crate) use transaction::{OpType, WorkingSetTuples};
pub use transaction::{RelationTransaction, WorkingSet};

use std::fmt::{Debug, Display};
use std::hash::Hash;

use crate::model::{AnonymousObjectMetadata, EntityMetadataKey, ObjAndUUIDHolder, StringHolder};
use moor_common::model::{ObjFlag, PropDefs, PropPerms, VerbDefs};
use moor_common::util::BitEnum;
use moor_var::{Obj, Var, program::ProgramType};

// ============================================================================
// Trait Bounds for Relation Domain and Codomain Types
// ============================================================================
//
// These traits reduce boilerplate in type parameter bounds throughout the
// tx and provider modules. They use the blanket impl pattern since
// Rust stable doesn't have native trait aliases.

/// Trait alias for types that can be used as a domain (key) in a relation.
///
/// Domain types must support:
/// - `Hash + Eq`: For use in hash-based indexes
/// - `Clone`: For copying keys during operations
/// - `Debug`: For error messages and conflict reporting
/// - `Send + Sync + 'static`: For thread-safe, owned storage
pub trait RelationDomain: Hash + Eq + Clone + Debug + Display + Send + Sync + 'static {}

impl<T> RelationDomain for T where T: Hash + Eq + Clone + Debug + Display + Send + Sync + 'static {}

/// Trait alias for types that can be used as a codomain (value) in a relation.
///
/// Codomain types must support:
/// - `Clone`: For copying values during operations
/// - `PartialEq`: For conflict detection and comparison
/// - `Send + Sync + 'static`: For thread-safe, owned storage
pub trait RelationCodomain: Clone + PartialEq + Send + Sync + 'static {
    /// Clone the value for the committed relation index.
    fn clone_for_commit(&self) -> Self {
        self.clone()
    }
}

impl RelationCodomain for Var {
    fn clone_for_commit(&self) -> Self {
        self.clone().with_cleared_hint()
    }
}

// Macro to implement RelationCodomain for other types
macro_rules! impl_relation_codomain {
    ($($t:ty),*) => {
        $(
            impl RelationCodomain for $t {}
        )*
    };
}

impl_relation_codomain!(
    Obj,
    BitEnum<ObjFlag>,
    StringHolder,
    VerbDefs,
    ProgramType,
    PropDefs,
    PropPerms,
    AnonymousObjectMetadata,
    ObjAndUUIDHolder,
    EntityMetadataKey
);

// We also need to implement for TestCodomain used in tests
// (TestCodomain is defined in tests)

/// Extended trait alias for codomain types that can be used with secondary indexes.
///
/// In addition to `RelationCodomain` bounds, these types must also support:
/// - `Hash + Eq`: For reverse lookups in secondary indexes
pub trait RelationCodomainHashable: RelationCodomain + Hash + Eq {}

impl<T> RelationCodomainHashable for T where T: RelationCodomain + Hash + Eq {}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Ord, PartialOrd)]
pub struct Timestamp(pub u64);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Tx {
    pub ts: Timestamp,
    pub visible_ts: Timestamp,
    pub snapshot_version: u64,
}

pub use moor_common::model::{ConflictInfo, ConflictTarget, ConflictType};

/// Build conflict metadata while retaining structured information for known key types.
pub(crate) fn make_conflict_info<Domain: RelationDomain>(
    relation_name: moor_var::Symbol,
    domain: &Domain,
    conflict_type: ConflictType,
) -> ConflictInfo {
    use std::any::Any;

    let any_domain = domain as &dyn Any;
    let relation = relation_name.as_string();
    let target = if matches!(relation.as_str(), "object_propvalues" | "object_propflags") {
        any_domain
            .downcast_ref::<crate::model::ObjAndUUIDHolder>()
            .map(|holder| ConflictTarget::Property {
                object: holder.obj(),
                uuid: holder.uuid(),
                name: None,
            })
    } else {
        any_domain
            .downcast_ref::<moor_var::Obj>()
            .copied()
            .map(ConflictTarget::Object)
    };

    ConflictInfo {
        relation_name,
        domain_key: format!("{domain}"),
        target,
        conflict_type,
    }
}

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("Incomplete resident index: {0}")]
    IncompleteIndex(moor_var::Symbol),
    #[error("Duplicate key")]
    Duplicate,
    #[error("Conflict detected: {0}")]
    Conflict(ConflictInfo),
    #[error("Retrieval error from backing store")]
    RetrievalFailure(String),
    /// Snapshot reader context already redacted by the storage adapter.
    #[error("Snapshot read failed: {0}")]
    SnapshotFailure(String),
    #[error("Store failure when writing to backing store: #{0}")]
    StorageFailure(String),
    #[error("Encoding error")]
    EncodingFailure,
}

/// Trait for handling persistence of a specific type T.
/// Provider implementations implement this trait multiple times for different types,
/// allowing per-type encoding and storage decisions.
///
/// This trait does NOT assume a universal byte representation - each type's impl
/// can encode and persist however it wants.
pub trait EncodeFor<T> {
    /// Type representing the stored form - could be bytes, SQL row, etc.
    type Stored;

    /// Encode a value to its stored representation
    fn encode(&self, value: &T) -> Result<Self::Stored, Error>;

    /// Decode from stored representation
    fn decode(&self, stored: Self::Stored) -> Result<T, Error>;
}

/// Represents a "canonical" source for some domain/codomain pair, to be supplied to a
/// transaction.
#[cfg(test)]
#[allow(dead_code)]
pub trait Canonical<Domain, Codomain> {
    fn get(&self, domain: &Domain) -> Result<Option<(Timestamp, Codomain)>, Error>;
    fn scan<F>(&self, f: &F) -> Result<Vec<(Timestamp, Domain, Codomain)>, Error>
    where
        F: Fn(&Domain, &Codomain) -> bool;
    fn get_by_codomain(&self, codomain: &Codomain) -> Vec<Domain>;
}
