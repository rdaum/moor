// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

mod indexes;
mod relation;
mod relation_tx;

pub use relation::{
    AcceptIdentical, CheckRelation, ConflictResolver, FailOnConflict, PotentialConflict,
    ProposedOp, Relation,
};
pub use relation_tx::{RelationTransaction, WorkingSet};

use std::fmt::{Debug, Display};
use std::hash::Hash;

use crate::{AnonymousObjectMetadata, ObjAndUUIDHolder, StringHolder};
use moor_common::model::{ObjFlag, PropDefs, PropPerms, VerbDefs};
use moor_common::util::BitEnum;
use moor_var::{Associative, Obj, Sequence, Var, program::ProgramType};

// ============================================================================
// Trait Bounds for Relation Domain and Codomain Types
// ============================================================================
//
// These traits reduce boilerplate in type parameter bounds throughout the
// tx_management and provider modules. They use the blanket impl pattern since
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
    /// Attempt to merge conflicting values.
    /// Returns Some(merged_value) if successful, None if conflict is unresolvable.
    fn try_merge(&self, _base: &Self, _theirs: &Self) -> Option<Self> {
        None
    }
}

// Implement for Var with smart merge logic
impl RelationCodomain for Var {
    fn try_merge(&self, base: &Self, theirs: &Self) -> Option<Self> {
        // Only merge if both operations provided a hint
        use moor_var::{
            OP_HINT_FLYWEIGHT_ADD_SLOT, OP_HINT_FLYWEIGHT_APPEND_CONTENTS, OP_HINT_LIST_APPEND,
            OP_HINT_MAP_INSERT, OP_HINT_STR_APPEND,
        };

        let my_hint = self.op_hint();
        let their_hint = theirs.op_hint();

        if my_hint != their_hint {
            return None;
        }

        match my_hint {
            OP_HINT_LIST_APPEND => {
                // List append merge: Base + (Theirs - Base) + (Mine - Base)
                let mine_list = self.as_list()?;
                let their_list = theirs.as_list()?;
                let base_list = base.as_list()?;

                // Both must be longer than base (appended)
                if mine_list.len() <= base_list.len() || their_list.len() <= base_list.len() {
                    return None;
                }

                let base_len = base_list.len();

                // Check prefixes
                if !mine_list.iter().take(base_len).eq(base_list.iter()) {
                    return None;
                }
                if !their_list.iter().take(base_len).eq(base_list.iter()) {
                    return None;
                }

                // Merge: Take `theirs` (which is Base + TheirSuffix) and append Mine's suffix
                let mut result = their_list.clone(); // Result starts as Theirs

                // Append Mine's suffix
                for item in mine_list.iter().skip(base_len) {
                    result = match result.push(&item) {
                        Ok(r) => match r.variant() {
                            moor_var::Variant::List(l) => l.clone(),
                            _ => return None,
                        },
                        Err(_) => return None,
                    };
                }

                // Return result wrapped in Var, WITHOUT hint - merged values shouldn't carry hints
                // as hints are only meaningful for the operation that created the value
                Some(Var::from_list(result))
            }
            OP_HINT_MAP_INSERT => {
                // Map insert merge: Two concurrent inserts of DIFFERENT keys.
                let mine_map = self.as_map()?;
                let their_map = theirs.as_map()?;
                let base_map = base.as_map()?;

                if mine_map.len() != base_map.len() + 1 || their_map.len() != base_map.len() + 1 {
                    // Only support single item insert for safety/speed for now
                    return None;
                }

                // Find the added key in Mine
                // Iterate Mine, check if in Base. The one that isn't is our key.
                let mut my_key = None;
                let mut my_val = None;
                for (k, v) in mine_map.iter() {
                    if !base_map.contains_key(&k, false).unwrap_or(false) {
                        my_key = Some(k);
                        my_val = Some(v);
                        break;
                    }
                }
                let (k_mine, v_mine) = (my_key?, my_val?);

                // Check if Theirs has this key
                if their_map.contains_key(&k_mine, false).unwrap_or(false) {
                    // Conflict! Both inserted same key (but different values, since AcceptIdentical failed)
                    return None;
                }

                // Merge: Take Theirs, insert My Key/Value
                // Clear hint - merged values shouldn't carry hints
                their_map
                    .set(&k_mine, &v_mine)
                    .ok()
                    .map(|v| v.with_cleared_hint())
            }
            OP_HINT_FLYWEIGHT_ADD_SLOT => {
                // Flyweight slot insert merge
                let mine_fw = self.as_flyweight()?;
                let their_fw = theirs.as_flyweight()?;
                let base_fw = base.as_flyweight()?;

                // Delegate must match
                if mine_fw.delegate() != base_fw.delegate()
                    || their_fw.delegate() != base_fw.delegate()
                {
                    return None;
                }

                // Verify sizes: both added exactly 1 item to slots
                let mine_slots = mine_fw.slots_storage();
                let their_slots = their_fw.slots_storage();
                let base_slots = base_fw.slots_storage();

                if mine_slots.len() != base_slots.len() + 1
                    || their_slots.len() != base_slots.len() + 1
                {
                    return None;
                }

                // Find the added slot in Mine
                let mut my_slot = None;
                for (k, v) in mine_slots.iter() {
                    if base_fw.get_slot(k).is_none() {
                        my_slot = Some((*k, v.clone()));
                        break;
                    }
                }
                let (k_mine, v_mine) = my_slot?;

                // Check if Theirs has this key
                if their_fw.get_slot(&k_mine).is_some() {
                    return None;
                }

                // Merge: Take Theirs, add My slot
                // No hint - merged values shouldn't carry hints
                Some(Var::from_flyweight(their_fw.add_slot(k_mine, v_mine)))
            }
            OP_HINT_FLYWEIGHT_APPEND_CONTENTS => {
                // Flyweight contents append merge
                let mine_fw = self.as_flyweight()?;
                let their_fw = theirs.as_flyweight()?;
                let base_fw = base.as_flyweight()?;

                // Slots and Delegate must match (only contents changed)
                if mine_fw.delegate() != base_fw.delegate()
                    || their_fw.delegate() != base_fw.delegate()
                    || mine_fw.slots_storage() != base_fw.slots_storage()
                    || their_fw.slots_storage() != base_fw.slots_storage()
                {
                    return None;
                }

                // Delegate to List merge for contents
                let base_contents = Var::from_list(base_fw.contents().clone());

                let mine_contents_hinted =
                    Var::from_list_with_hint(mine_fw.contents().clone(), OP_HINT_LIST_APPEND);
                let their_contents_hinted =
                    Var::from_list_with_hint(their_fw.contents().clone(), OP_HINT_LIST_APPEND);

                let merged_contents_var =
                    mine_contents_hinted.try_merge(&base_contents, &their_contents_hinted)?;
                let merged_contents = merged_contents_var.as_list()?.clone();

                // No hint - merged values shouldn't carry hints
                Some(Var::from_flyweight(their_fw.with_contents(merged_contents)))
            }
            OP_HINT_STR_APPEND => {
                // String append merge
                let mine_str = self.as_string()?;
                let their_str = theirs.as_string()?;
                let base_str = base.as_string()?;

                // Both must be longer than base
                if mine_str.len() <= base_str.len() || their_str.len() <= base_str.len() {
                    return None;
                }

                // Verify base prefix
                if !mine_str.starts_with(base_str) || !their_str.starts_with(base_str) {
                    return None;
                }

                // Merge: Take theirs, append mine's suffix
                let mut result = their_str.to_string();
                result.push_str(&mine_str[base_str.len()..]);

                // No hint - merged values shouldn't carry hints
                Some(Var::from_str_type(moor_var::Str::mk_string(result)))
            }
            _ => None,
        }
    }
}

// Macro to implement RelationCodomain for other types (no-op merge)
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
    ObjAndUUIDHolder
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
    pub snapshot_version: u64,
}

pub use moor_common::model::{ConflictInfo, ConflictType};

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("Duplicate key")]
    Duplicate,
    #[error("Conflict detected: {0}")]
    Conflict(ConflictInfo),
    #[error("Retrieval error from backing store")]
    RetrievalFailure(String),
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
pub trait Canonical<Domain, Codomain> {
    fn get(&self, domain: &Domain) -> Result<Option<(Timestamp, Codomain)>, Error>;
    fn scan<F>(&self, f: &F) -> Result<Vec<(Timestamp, Domain, Codomain)>, Error>
    where
        F: Fn(&Domain, &Codomain) -> bool;
    fn get_by_codomain(&self, codomain: &Codomain) -> Vec<Domain>;
}
