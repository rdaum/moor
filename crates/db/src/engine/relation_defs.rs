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

/// Result of checking whether prepared writes can be rebased onto a CAS winner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RebaseCheck {
    /// The cumulative bloom filter proves the write sets are disjoint.
    BloomDisjoint,
    /// Snapshot indexes prove the write sets are disjoint after a bloom hit.
    ExactlyDisjoint,
    /// At least one written key changed after the transaction was prepared.
    ActualOverlap(moor_common::model::ConflictInfo),
}

/// Return whether a key has the same authoritative state in two relation indexes.
pub(crate) fn relation_key_unchanged<Domain, Codomain>(
    checked: &dyn crate::tx::RelationIndex<Domain, Codomain>,
    winner: &dyn crate::tx::RelationIndex<Domain, Codomain>,
    key: &Domain,
) -> bool
where
    Domain: crate::tx::RelationDomain,
    Codomain: crate::tx::RelationCodomain,
{
    // Snapshot indexes are normally fully loaded. If that invariant is ever
    // relaxed, absence is not authoritative and exact rebase must fail safe.
    if !checked.is_fully_resident() || !winner.is_fully_resident() {
        return false;
    }

    let checked_ts = checked.index_lookup(key).map(|entry| entry.ts);
    let winner_ts = winner.index_lookup(key).map(|entry| entry.ts);
    checked_ts == winner_ts
}

/// Generate resident transactions, indexes, and logical mutations from the relation registry.
/// Storage resources are bound separately by each adapter.
macro_rules! define_relations {
    (@change_type PropertyValueChain, $domain:ty, $codomain:ty) => {
        Vec<crate::provider::logical::PreparedPropertyValueOp>
    };
    (@change_type Ordinary, $domain:ty, $codomain:ty) => {
        crate::tx::WorkingSetTuples<$domain, $codomain>
    };
    (@prepare_changes PropertyValueChain, $ws:expr) => {
        crate::provider::logical::prepare_property_value_working_set($ws)
    };
    (@prepare_changes Ordinary, $ws:expr) => { $ws.tuples() };

    // Entry point: parse all items
    (
        $(
            $field:ident $category:ident $policy:ident $arrow:tt $domain:ty, $codomain:ty
        ),* $(,)?
    ) => {
        define_relations!(@process [ $( ($field, $domain, $codomain, $arrow, $category, $policy) ),* ]);
    };

    // Main processing rule
    (@process [ $( ($field:ident, $domain:ty, $codomain:ty, $arrow:tt, $category:ident, $policy:ident) ),* ]) => {
        pastey::paste! {
            /// Type alias for Relations to reduce verbosity in macro.
            type R<Domain, Codomain> = Relation<Domain, Codomain>;

            /// Stable identifier for a persisted database relation.
            #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
            pub enum DatabaseRelation {
                $( [<$field:camel>], )*
            }

            impl DatabaseRelation {
                pub const ALL: &'static [Self] = &[
                    $( Self::[<$field:camel>], )*
                ];

                #[must_use]
                pub const fn as_str(self) -> &'static str {
                    match self {
                        $( Self::[<$field:camel>] => stringify!($field), )*
                    }
                }

                #[must_use]
                pub fn named(name: &str) -> Option<Self> {
                    match name {
                        $( stringify!($field) => Some(Self::[<$field:camel>]), )*
                        _ => None,
                    }
                }
            }

            /// Wrapper struct containing all database relations.
            ///
            /// This struct groups all relations together and provides convenience
            /// methods for operations that need to be performed across all relations.
            pub(crate) struct Relations {
                $( pub(crate) $field: R<$domain, $codomain>, )*
            }

            /// Wrapper struct for relation checkers during transaction commit.
            ///
            /// This struct holds the checking state for all relations during the
            /// commit process, allowing batch operations across all relations.
            pub(crate) struct RelationCheckers {
                $( $field: Option<CheckRelation<$domain, $codomain>>, )*
            }

            #[derive(Clone)]
            pub(crate) struct WorldStateSnapshot {
                pub(crate) version: u64,
                pub(crate) committed_ts: crate::tx::Timestamp,
                pub(crate) caches: std::sync::Arc<crate::engine::moor_db::Caches>,
                $( pub(crate) $field: std::sync::Arc<dyn crate::tx::RelationIndex<$domain, $codomain>>, )*
                /// Cumulative bloom filter of keys modified since `bloom_since_version`.
                /// Used for fast conflict detection: if a transaction's written keys
                /// don't intersect this bloom, check_all can be skipped entirely.
                /// Also used for key-level rebase after CAS failure.
                pub(crate) commit_bloom: Option<crate::tx::CommitBloom>,
                /// The snapshot version from which `commit_bloom` has been accumulating.
                /// A transaction with `snapshot_version >= bloom_since_version` can trust
                /// the bloom filter covers all intervening commits. Otherwise, fall back
                /// to check_all.
                pub(crate) bloom_since_version: u64,
            }

            /// Trait defining the interface for processing transaction commits.
            ///
            /// This trait decouples the WorldStateTransaction from the concrete
            /// database implementation, allowing for different commit backends
            /// (e.g., real database vs. mock/in-memory for testing).
            pub(crate) trait TransactionContext: Send + Sync {
                /// Commit a write transaction with its working sets.
                fn commit_writes(
                    &self,
                    ws: Box<WorkingSets>,
                    enqueued_at: Instant,
                ) -> Result<moor_common::model::CommitResult, moor_common::model::WorldStateError>;
                /// Commit a read-only transaction, potentially updating caches.
                fn commit_read_only(&self, snapshot_version: u64, caches: crate::engine::moor_db::Caches);
                /// Get the current database disk usage in bytes.
                fn usage_bytes(&self) -> usize;
            }

            // Instantiate these private methods inside the commit state module.
            // The outer pipeline cannot check and build snapshots independently.
            macro_rules! define_relation_commit_methods {
                () => {
                    impl RelationCheckers {
                        /// Check all relations for conflicts with the given working sets.
                        ///
                        /// Returns `Ok(())` if all relations pass conflict checking,
                        /// An invariant error is distinct from a retryable conflict.
                        fn check_all(&mut self, ws: &mut RelationWorkingSets) -> Result<(), crate::tx::Error> {
                            $(
                                if !ws.$field.is_empty() {
                                    let checker = self.$field.as_mut().expect("nonempty working set must have a checker");
                                    define_relations!(@check_relation $policy, $field, checker, ws)?;
                                }
                            )*
                            Ok(())
                        }

                        /// Update imbl indexes from working sets without touching providers.
                        /// Returns a bloom filter of modified keys for fast rebase conflict detection.
                        fn prepare_apply_all(
                            &mut self,
                            ws: &RelationWorkingSets,
                        ) -> crate::tx::CommitBloom {
                            let mut bloom = crate::tx::CommitBloom::new();
                            $(
                                if !ws.$field.is_empty() {
                                    // Insert all keys from this relation into the bloom filter
                                    for key in ws.$field.tuples_ref().keys() {
                                        bloom.insert(key);
                                    }
                                    self.$field
                                        .as_mut()
                                        .expect("nonempty working set must have a checker")
                                        .prepare_indexes(&ws.$field);
                                }
                            )*
                            bloom
                        }

                        /// Build a candidate snapshot from the updated indexes without consuming self.
                        /// The bloom filter is cumulative: this commit's keys OR'd with the
                        /// previous snapshot's bloom, covering all commits since `bloom_since_version`.
                        /// Resets when the bloom has accumulated too many versions (saturation guard).
                        fn build_snapshot(
                            &self,
                            current_root: &std::sync::Arc<WorldStateSnapshot>,
                            committed_ts: crate::tx::Timestamp,
                            combined_caches: crate::engine::moor_db::Caches,
                            mut bloom: crate::tx::CommitBloom,
                        ) -> std::sync::Arc<WorldStateSnapshot> {
                            // Decide whether to accumulate or reset the bloom.
                            // Reset if the previous bloom covers more than 32 versions —
                            // beyond that, most bits are set and the filter is useless.
                            // After reset, bloom_since_version advances to current_root.version,
                            // meaning only transactions newer than that can use the bloom skip.
                            const MAX_BLOOM_SPAN: u64 = 32;
                            let bloom_since_version = if let Some(ref prev_bloom) = current_root.commit_bloom {
                                let span = current_root.version.saturating_sub(current_root.bloom_since_version);
                                if span < MAX_BLOOM_SPAN {
                                    bloom.merge(prev_bloom);
                                    current_root.bloom_since_version
                                } else {
                                    // Reset: bloom covers only this commit
                                    current_root.version
                                }
                            } else {
                                // No previous bloom (initial snapshot). Start fresh.
                                current_root.version
                            };

                            let caches = if combined_caches.has_changed() {
                                std::sync::Arc::new(combined_caches)
                            } else {
                                current_root.caches.clone()
                            };

                            std::sync::Arc::new(WorldStateSnapshot {
                                version: current_root.version + 1,
                                committed_ts: current_root.committed_ts.max(committed_ts),
                                caches,
                                $( $field: self.$field.as_ref().map_or_else(
                                    || current_root.$field.clone(),
                                    |checker| checker.snapshot_index_or(&current_root.$field),
                                ), )*
                                commit_bloom: Some(bloom),
                                bloom_since_version,
                            })
                        }

                        /// Determine whether prepared operations can be rebased after a CAS loss.
                        ///
                        /// A bloom miss proves disjointness without index lookups. On a bloom hit
                        /// or unavailable coverage, compare the written keys in the snapshot that
                        /// was checked with the CAS winner. This exact fallback is read-only and
                        /// does not clone or rewrite the working set.
                        fn rebase_check(
                            &self,
                            ws: &RelationWorkingSets,
                            checked: &std::sync::Arc<WorldStateSnapshot>,
                            winner: &std::sync::Arc<WorldStateSnapshot>,
                        ) -> $crate::engine::relation_defs::RebaseCheck {
                            if let Err(info) = ws.check_property_policies(winner) {
                                return $crate::engine::relation_defs::RebaseCheck::ActualOverlap(info);
                            }
                            let bloom_proves_disjoint = checked.version >= winner.bloom_since_version
                                && winner.commit_bloom.as_ref().is_some_and(|winner_bloom| {
                                    true $(&& ws.$field.tuples_ref().keys().all(|key| {
                                        !winner_bloom.might_contain(key)
                                    }))*
                                });

                            if bloom_proves_disjoint {
                                return $crate::engine::relation_defs::RebaseCheck::BloomDisjoint;
                            }

                            $(
                                for key in ws.$field.tuples_ref().keys() {
                                    if define_relations!(@can_clobber $policy, ws, key) {
                                        continue;
                                    }
                                    if !$crate::engine::relation_defs::relation_key_unchanged(
                                        &*checked.$field,
                                        &*winner.$field,
                                        key,
                                    ) {
                                        return $crate::engine::relation_defs::RebaseCheck::ActualOverlap(
                                            $crate::tx::make_conflict_info(
                                                self.$field
                                                    .as_ref()
                                                    .expect("nonempty working set must have a checker")
                                                    .relation_name(),
                                                key,
                                                moor_common::model::ConflictType::ConcurrentWrite,
                                            ),
                                        );
                                    }
                                }
                            )*

                            $crate::engine::relation_defs::RebaseCheck::ExactlyDisjoint
                        }

                        /// Rebuild prepared indexes on top of a winner proven disjoint from the
                        /// transaction's working set.
                        fn build_rebased_snapshot(
                            &self,
                            ws: &RelationWorkingSets,
                            winner: &std::sync::Arc<WorldStateSnapshot>,
                            committed_ts: crate::tx::Timestamp,
                            combined_caches: crate::engine::moor_db::Caches,
                            our_bloom: &crate::tx::CommitBloom,
                        ) -> std::sync::Arc<WorldStateSnapshot> {
                            // Apply the same span guard as build_snapshot: if the winner's
                            // bloom has accumulated too many versions, reset instead of merging.
                            const MAX_BLOOM_SPAN: u64 = 32;
                            let span = winner.version.saturating_sub(winner.bloom_since_version);
                            let (merged_bloom, bloom_since_version) = if span < MAX_BLOOM_SPAN {
                                let mut merged = our_bloom.clone();
                                if let Some(ref winner_bloom) = winner.commit_bloom {
                                    merged.merge(winner_bloom);
                                }
                                (merged, winner.bloom_since_version)
                            } else {
                                // Reset: bloom covers only our commit
                                (our_bloom.clone(), winner.version)
                            };

                            let caches = if combined_caches.has_changed() {
                                std::sync::Arc::new(combined_caches)
                            } else {
                                winner.caches.clone()
                            };

                            std::sync::Arc::new(WorldStateSnapshot {
                                version: winner.version + 1,
                                committed_ts: winner.committed_ts.max(committed_ts),
                                caches,
                                $( $field: self.$field.as_ref().map_or_else(
                                    || winner.$field.clone(),
                                    |checker| checker.rebased_snapshot_index(&winner.$field, &ws.$field),
                                ), )*
                                commit_bloom: Some(merged_bloom),
                                bloom_since_version,
                            })
                        }
                    }

                };
            }

            impl Relations {
                pub(crate) fn init() -> Self {
                    Self { $( $field: define_relations!(@create_relation $arrow, $field), )* }
                }

                /// Begin the checking phase for all relations.
                ///
                /// Creates RelationCheckers for all relations, which can then be used
                /// to check for conflicts during transaction commit.
                fn begin_check_all(
                    &self,
                    snapshot: &WorldStateSnapshot,
                    working_sets: &RelationWorkingSets,
                ) -> RelationCheckers {
                    RelationCheckers {
                        $( $field: (!working_sets.$field.is_empty()).then(|| {
                            self.$field.begin_check_from_index(&*snapshot.$field)
                        }), )*
                    }
                }

                /// Start a new transaction across all relations.
                ///
                /// Creates a WorldStateTransaction with relation transactions for all
                /// defined relations, along with the necessary caches.
                ///
                /// # Parameters
                /// - `db`: Database handle used for direct commit processing
                /// - `seed`: Transaction startup context with tx metadata, snapshot, sequences,
                ///   and forked resolution caches.
                fn start_transaction(&self,
                    db: std::sync::Arc<dyn TransactionContext>,
                    seed: crate::engine::moor_db::TxSeed,
                ) -> WorldStateTransaction {
                    let crate::engine::moor_db::TxSeed {
                        tx,
                        snapshot,
                        sequences,
                        caches,
                    } = seed;
                    let crate::engine::moor_db::Caches {
                        verb_resolution_cache,
                        prop_resolution_cache,
                        ancestry_cache,
                    } = caches;
                    WorldStateTransaction {
                        tx,
                        db,
                        $( $field: self.$field.start_from_snapshot(&tx, snapshot.$field.clone()), )*
                        sequences,
                        verb_resolution_cache: std::cell::RefCell::new(verb_resolution_cache),
                        prop_resolution_cache: std::cell::RefCell::new(prop_resolution_cache),
                        ancestry_cache: std::cell::RefCell::new(ancestry_cache),
                        prop_perm_memo: crate::engine::ws_transaction::PropertyPermMemo::new(),
                        inherited_policy_reads: std::collections::HashSet::new(),
                        owner_change_reads: std::collections::HashSet::new(),
                    }
                }
            }

            /// Working sets for all relations, including caches.
            ///
            /// This struct contains the working sets for all relations along with
            /// the resolution caches used during transaction processing.
            pub(crate) struct WorkingSets {
                pub(crate) owner_change_reads: std::collections::HashSet<moor_var::Obj>,
                pub(crate) inherited_policy_reads: std::collections::HashSet<crate::model::ObjAndUUIDHolder>,
                #[allow(dead_code)]
                pub(crate) tx: Tx,
                $( pub(crate) $field: WorkingSet<$domain, $codomain>, )*
                pub(crate) verb_resolution_cache: VerbResolutionCache,
                pub(crate) prop_resolution_cache: PropResolutionCache,
                pub(crate) ancestry_cache: AncestryCache,
                pub(crate) has_mutations: bool,
                /// Bloom filter of all keys written in this transaction.
                pub(crate) tx_bloom: crate::tx::CommitBloom,
            }

            impl WorkingSets {
                /// Count the total number of tuples across all working sets.
                ///
                /// This is useful for logging and performance monitoring during commits.
                pub fn total_tuples(&self) -> usize {
                    0 $( + self.$field.len() )*
                }

                /// Extract relation working sets from caches.
                ///
                /// Separates the relation working sets from the resolution caches,
                /// returning them as separate values to handle ownership properly
                /// during the commit process.
                ///
                /// # Returns
                /// A tuple containing:
                /// - `RelationWorkingSets`: Working sets for all relations
                /// - `VerbResolutionCache`: Verb resolution cache
                /// - `PropResolutionCache`: Property resolution cache
                /// - `AncestryCache`: Ancestry cache
                fn extract_relation_working_sets(self) -> (RelationWorkingSets, VerbResolutionCache, PropResolutionCache, AncestryCache) {
                    let ws = RelationWorkingSets {
                        owner_change_reads: self.owner_change_reads,
                        inherited_policy_reads: self.inherited_policy_reads,
                        $( $field: self.$field, )*
                    };
                    (ws, self.verb_resolution_cache, self.prop_resolution_cache, self.ancestry_cache)
                }
            }

            /// Working sets for relations only, without caches.
            ///
            /// This struct contains only the working sets for relations, with caches
            /// separated out to handle ownership during commit processing.
            #[derive(Clone)]
            pub(crate) struct RelationWorkingSets {
                pub(crate) owner_change_reads: std::collections::HashSet<moor_var::Obj>,
                pub(crate) inherited_policy_reads: std::collections::HashSet<crate::model::ObjAndUUIDHolder>,
                $( $field: WorkingSet<$domain, $codomain>, )*
            }

            /// Transaction state for all database relations.
            ///
            /// This struct represents an active transaction that can read from and write to
            /// all defined database relations. It contains relation transactions for each
            /// relation, along with caches needed for transaction processing.
            pub struct WorldStateTransaction {
                pub(crate) owner_change_reads: std::collections::HashSet<moor_var::Obj>,
                pub(crate) inherited_policy_reads: std::collections::HashSet<crate::model::ObjAndUUIDHolder>,
                #[allow(dead_code)]
                pub(crate) tx: Tx,
                /// Database handle used for direct commit processing.
                pub(crate) db: std::sync::Arc<dyn TransactionContext>,
                /// Relation transactions for each defined relation
                $( pub(crate) $field: RelationTransaction<$domain, $codomain>, )*
                /// Array of sequence counters for object ID generation
                pub(crate) sequences: Arc<crate::engine::moor_db::SequenceState>,
                /// Local fork of the verb resolution cache
                pub(crate) verb_resolution_cache: std::cell::RefCell<VerbResolutionCache>,
                /// Local fork of the property resolution cache
                pub(crate) prop_resolution_cache: std::cell::RefCell<PropResolutionCache>,
                /// Local fork of the ancestry cache
                pub(crate) ancestry_cache: std::cell::RefCell<AncestryCache>,
                /// Per-transaction memo state for property permission lookups.
                pub(crate) prop_perm_memo: crate::engine::ws_transaction::PropertyPermMemo,
            }

            /// A mutation view cannot perform cached resolution while its caches are borrowed.
            pub(crate) struct WorldStateEdit<'a> {
                // Generate the whole relation view, including relations without cached readers.
                $( #[allow(dead_code)] pub(crate) $field: &'a mut RelationTransaction<$domain, $codomain>, )*
                _invalidation: crate::engine::ws_transaction::edit::EditInvalidation<'a>,
            }

            impl WorldStateTransaction {
                pub(crate) fn has_mutations(&self) -> bool {
                    false $( || self.$field.has_mutations() )*
                }

                pub(crate) fn edit(&mut self, scope: crate::engine::ws_transaction::edit::EditScope) -> WorldStateEdit<'_> {
                    WorldStateEdit {
                        $( $field: &mut self.$field, )*
                        _invalidation: crate::engine::ws_transaction::edit::EditInvalidation::new(
                            scope,
                            self.verb_resolution_cache.get_mut(),
                            self.prop_resolution_cache.get_mut(),
                            self.ancestry_cache.get_mut(),
                            &mut self.prop_perm_memo,
                        ),
                    }
                }

                /// Extract working sets from all relation transactions.
                ///
                /// This method collects the working sets from all relation transactions
                /// and packages them into a WorkingSets struct for commit processing.
                ///
                /// # Errors
                /// Returns an error if any relation transaction fails to produce a working set.
                pub(crate) fn into_working_sets(self) -> Result<Box<WorkingSets>, moor_common::model::WorldStateError> {
                    let has_mutations = self.has_mutations();
                    $(
                        let $field = self.$field.working_set()?;
                    )*

                    // Build bloom filter from all written keys across all relations.
                    let mut tx_bloom = crate::tx::CommitBloom::new();
                    $(
                        for key in $field.tuples_ref().keys() {
                            tx_bloom.insert(key);
                        }
                    )*

                    let ws = Box::new(WorkingSets {
                        owner_change_reads: self.owner_change_reads,
                        inherited_policy_reads: self.inherited_policy_reads,
                        tx: self.tx,
                        $( $field, )*
                        verb_resolution_cache: self.verb_resolution_cache.into_inner(),
                        prop_resolution_cache: self.prop_resolution_cache.into_inner(),
                        ancestry_cache: self.ancestry_cache.into_inner(),
                        has_mutations,
                        tx_bloom,
                    });

                    Ok(ws)
                }
            }

            /// Accepted logical mutations, without transaction indexes or backend resources.
            pub(crate) struct RelationChanges {
                $( pub(crate) $field: define_relations!(@change_type $category, $domain, $codomain), )*
            }

            impl RelationWorkingSets {
                /// Prove append candidates and release base indexes before asynchronous encoding.
                pub(crate) fn into_changes(self) -> RelationChanges {
                    RelationChanges {
                        $( $field: define_relations!(@prepare_changes $category, self.$field), )*
                    }
                }
            }

            /// Sequence constant for maximum object ID tracking.
            ///
            /// This constant identifies the sequence used to track the highest object ID
            /// that has been allocated, used for generating new unique object IDs.
            pub const SEQUENCE_MAX_OBJECT: usize = 0;
        }
    };

    // Only property values can opt out of write-conflict rejection.
    (@check_relation PropertyPermissions, $field:ident, $checker:ident, $ws:ident) => {{
        let flags = &$ws.object_propflags;
        $checker.check_with_resolver(&mut $ws.$field, |conflict: &$crate::tx::PotentialConflict<crate::model::ObjAndUUIDHolder, moor_var::Var>| {
            if property_can_clobber(flags, &conflict.domain) {
                Ok($crate::tx::Resolution::Accept)
            } else {
                Err($crate::tx::Error::Conflict(conflict.info.clone()))
            }
        })
    }};
    (@check_relation Strict, $field:ident, $checker:ident, $ws:ident) => {
        $checker.check(&mut $ws.$field)
    };
    (@can_clobber PropertyPermissions, $ws:ident, $key:ident) => {
        property_can_clobber(&$ws.object_propflags, $key)
    };
    (@can_clobber Strict, $ws:ident, $key:ident) => { false };

    // Helper rule to create a relation based on arrow type
    (@create_relation =>, $field:ident) => {
        Relation::new(Symbol::mk(stringify!($field)))
    };

    (@create_relation ==, $field:ident) => {
        Relation::new_with_secondary(
            Symbol::mk(stringify!($field))
        )
    };
}

// Re-export the macro for use in other modules
pub(crate) use define_relations;
