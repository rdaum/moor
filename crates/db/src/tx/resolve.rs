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

use crate::{
    api::world_state::db_counters,
    tx::{Error, RelationCodomain, RelationDomain},
};
use super::check::{PotentialConflict, ProposedOp};

/// The result of a conflict resolution attempt.
pub enum Resolution<Codomain> {
    /// Accept the proposed operation as-is (continue checking)
    Accept,
    /// Rewrite the proposed operation with a new value
    Rewrite(Codomain),
}

/// Trait for conflict resolution strategies.
///
/// Implement this to provide custom conflict resolution logic. The resolver
/// is called for each detected conflict and can either:
/// - Return `Ok(Resolution::Accept)` to accept/resolve the conflict and continue checking
/// - Return `Ok(Resolution::Rewrite(new_val))` to resolve by changing the written value
/// - Return `Err(Error::Conflict(...))` to abort with that conflict
pub trait ConflictResolver<Domain, Codomain>
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
{
    /// Attempt to resolve a conflict.
    ///
    /// Called when a conflict is detected during the check phase.
    fn resolve(
        &mut self,
        conflict: &PotentialConflict<Domain, Codomain>,
    ) -> Result<Resolution<Codomain>, Error>;
}

/// A resolver that always fails on conflict (the default behavior).
pub struct FailOnConflict;

impl<Domain, Codomain> ConflictResolver<Domain, Codomain> for FailOnConflict
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
{
    fn resolve(
        &mut self,
        conflict: &PotentialConflict<Domain, Codomain>,
    ) -> Result<Resolution<Codomain>, Error> {
        Err(Error::Conflict(conflict.info.clone()))
    }
}

/// A resolver that accepts conflicts where theirs == mine (identical values).
/// If both transactions wrote the same value, there's no real conflict.
pub struct AcceptIdentical;

impl<Domain, Codomain> ConflictResolver<Domain, Codomain> for AcceptIdentical
where
    Domain: RelationDomain,
    Codomain: RelationCodomain, // PartialEq is already required by RelationCodomain
{
    fn resolve(
        &mut self,
        conflict: &PotentialConflict<Domain, Codomain>,
    ) -> Result<Resolution<Codomain>, Error> {
        // Check if theirs == mine (using PartialEq)
        let identical = match (&conflict.theirs, &conflict.mine) {
            // Both have values - compare them
            (
                Some((_, theirs_val)),
                ProposedOp::Insert(mine_val) | ProposedOp::Update(mine_val),
            ) => theirs_val == mine_val,
            // Both are effectively "no value" (theirs doesn't exist, we're deleting)
            (None, ProposedOp::Delete) => true,
            // Otherwise, real conflict
            _ => false,
        };

        if identical {
            Ok(Resolution::Accept)
        } else {
            Err(Error::Conflict(conflict.info.clone()))
        }
    }
}

/// A resolver that attempts smart merges using RelationCodomain::try_merge.
/// If merging fails, it falls back to AcceptIdentical logic.
pub struct SmartMergeResolver;

impl<Domain, Codomain> ConflictResolver<Domain, Codomain> for SmartMergeResolver
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
{
    fn resolve(
        &mut self,
        conflict: &PotentialConflict<Domain, Codomain>,
    ) -> Result<Resolution<Codomain>, Error> {
        let counters = db_counters();

        // 1. Try AcceptIdentical logic (idempotency)
        let identical = match (&conflict.theirs, &conflict.mine) {
            (
                Some((_, theirs_val)),
                ProposedOp::Insert(mine_val) | ProposedOp::Update(mine_val),
            ) => theirs_val == mine_val,
            (None, ProposedOp::Delete) => true,
            _ => false,
        };

        if identical {
            counters.crdt_resolve_success.invocations().add(1);
            return Ok(Resolution::Accept);
        }

        // 2. Try Smart Merge (CRDT-like)
        // Only applicable if we have Base, Theirs, and Mine (Update/Insert conflict)
        if let Some((_, base_val)) = &conflict.base
            && let Some((_, theirs_val)) = &conflict.theirs
            && let Some(mine_val) = conflict.mine.value()
            && let Some(merged) = mine_val.try_merge(base_val, theirs_val)
        {
            counters.crdt_resolve_success.invocations().add(1);
            return Ok(Resolution::Rewrite(merged));
        }

        // 3. Fail
        counters.crdt_resolve_fail.invocations().add(1);
        Err(Error::Conflict(conflict.info.clone()))
    }
}

/// Implement ConflictResolver for closures for convenience.
impl<Domain, Codomain, F> ConflictResolver<Domain, Codomain> for F
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
    F: FnMut(&PotentialConflict<Domain, Codomain>) -> Result<Resolution<Codomain>, Error>,
{
    fn resolve(
        &mut self,
        conflict: &PotentialConflict<Domain, Codomain>,
    ) -> Result<Resolution<Codomain>, Error> {
        self(conflict)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tx::{ConflictInfo, ConflictType, Timestamp};

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct MergeCodomain(u64);

    impl RelationCodomain for MergeCodomain {
        fn try_merge(&self, base: &Self, theirs: &Self) -> Option<Self> {
            Some(MergeCodomain(
                self.0.wrapping_add(theirs.0).wrapping_sub(base.0),
            ))
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct PlainCodomain(u64);
    impl RelationCodomain for PlainCodomain {}

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct TestDomain(u64);

    impl std::fmt::Display for TestDomain {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "TestDomain({})", self.0)
        }
    }

    #[test]
    fn test_accept_identical_accepts() {
        let conflict = PotentialConflict {
            info: ConflictInfo {
                relation_name: "test".into(),
                domain_key: "k1".to_string(),
                conflict_type: ConflictType::ConcurrentWrite,
            },
            domain: TestDomain(1),
            base: Some((Timestamp(1), PlainCodomain(1))),
            theirs: Some((Timestamp(2), PlainCodomain(10))),
            mine: ProposedOp::Update(PlainCodomain(10)),
            read_ts: Timestamp(1),
            write_ts: Timestamp(2),
        };

        let mut resolver = AcceptIdentical;
        let resolution = resolver.resolve(&conflict).unwrap();
        assert!(matches!(resolution, Resolution::Accept));
    }

    #[test]
    fn test_smart_merge_rewrite_increments_success_counter() {
        let counters = db_counters();
        let success_before = counters.crdt_resolve_success.invocations().sum();

        let conflict = PotentialConflict {
            info: ConflictInfo {
                relation_name: "test".into(),
                domain_key: "k1".to_string(),
                conflict_type: ConflictType::ConcurrentWrite,
            },
            domain: TestDomain(1),
            base: Some((Timestamp(1), MergeCodomain(10))),
            theirs: Some((Timestamp(2), MergeCodomain(20))),
            mine: ProposedOp::Update(MergeCodomain(11)),
            read_ts: Timestamp(1),
            write_ts: Timestamp(2),
        };

        let mut resolver = SmartMergeResolver;
        let resolution = resolver.resolve(&conflict).unwrap();
        assert!(matches!(resolution, Resolution::Rewrite(MergeCodomain(21))));
        assert!(counters.crdt_resolve_success.invocations().sum() >= success_before + 1);
    }

    #[test]
    fn test_smart_merge_fail_increments_fail_counter() {
        let counters = db_counters();
        let fail_before = counters.crdt_resolve_fail.invocations().sum();

        let conflict = PotentialConflict {
            info: ConflictInfo {
                relation_name: "test".into(),
                domain_key: "k1".to_string(),
                conflict_type: ConflictType::ConcurrentWrite,
            },
            domain: TestDomain(1),
            base: Some((Timestamp(1), PlainCodomain(10))),
            theirs: Some((Timestamp(2), PlainCodomain(20))),
            mine: ProposedOp::Update(PlainCodomain(11)),
            read_ts: Timestamp(1),
            write_ts: Timestamp(2),
        };

        let mut resolver = SmartMergeResolver;
        assert!(matches!(resolver.resolve(&conflict), Err(Error::Conflict(_))));
        assert!(counters.crdt_resolve_fail.invocations().sum() >= fail_before + 1);
    }
}
