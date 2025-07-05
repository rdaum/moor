// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
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

/// Generates database relation boilerplate code.
///
/// This macro takes a list of relation definitions and generates all the necessary
/// boilerplate code including:
/// - `Relations` wrapper struct with helper methods
/// - `RelationCheckers` for transaction commit processing
/// - `WorkingSets` for transaction working sets
/// - `RelationWorkingSets` for separating caches from working sets
/// - `WorldStateTransaction` struct definition
///
/// # Syntax
///
/// ```rust,ignore
/// define_relations! {
///     field_name => DomainType, CodomainType,      // Normal relation (primary index only)
///     field_name == DomainType, CodomainType,      // Bidirectional secondary indexed relation
///     // ... more relations
/// }
/// ```
///
/// # Generated Code
///
/// For each relation `field_name: Domain => Codomain`, the macro generates:
/// - A field in the `Relations` struct of type `Relation<Domain, Codomain, FjallProvider<Domain, Codomain>>`
/// - A field in the `RelationCheckers` struct for commit checking
/// - A field in the `WorkingSets` struct for transaction working sets
/// - A field in the `WorldStateTransaction` struct for relation transactions
///
/// # Generated Methods
///
/// ## Relations
/// - `init(keyspace, config)` - Initialize all relations from keyspace and config
/// - `stop_all()` - Stop all relation providers
/// - `begin_check_all()` - Begin checking phase for all relations
/// - `start_transaction(...)` - Create a new WorldStateTransaction
///
/// ## RelationCheckers
/// - `check_all(ws)` - Check all relations for conflicts
/// - `all_clean()` - Check if any relations are dirty
/// - `apply_all(ws)` - Apply all working sets to relations
/// - `commit_all(relations)` - Commit all changes with appropriate locking
///
/// ## WorkingSets
/// - `total_tuples()` - Count total tuples across all working sets
/// - `extract_relation_working_sets()` - Separate relation working sets from caches
///
/// # Example
///
/// ```rust,ignore
/// define_relations! {
///     object_location == Obj, Obj,
///     object_contents => Obj, ObjSet,
///     object_flags => Obj, BitEnum<ObjFlag>,
/// }
/// ```
///
/// This generates all the necessary boilerplate for three relations, eliminating
/// hundreds of lines of repetitive code that would otherwise need to be maintained
/// manually.
///
/// # Type Aliases
///
/// The macro uses `R<Domain, Codomain>` as a type alias for
/// `Relation<Domain, Codomain, FjallProvider<Domain, Codomain>`.
///
/// # Dependencies
///
/// The macro requires the `paste` crate for token concatenation to generate
/// unique variable names for each relation during initialization.
macro_rules! define_relations {
    // Entry point: parse all items
    (
        $( 
            $field:ident $arrow:tt $domain:ty, $codomain:ty
        ),* $(,)?
    ) => {
        define_relations!(@process [ $( ($field, $domain, $codomain, $arrow) ),* ]);
    };
    
    // Main processing rule
    (@process [ $( ($field:ident, $domain:ty, $codomain:ty, $arrow:tt) ),* ]) => {
        paste::paste! {
            /// Type alias for Relations to reduce verbosity in macro.
            type R<Domain, Codomain> = Relation<Domain, Codomain, FjallProvider<Domain, Codomain>>;

            /// Wrapper struct containing all database relations.
            ///
            /// This struct groups all relations together and provides convenience
            /// methods for operations that need to be performed across all relations.
            pub(crate) struct Relations {
                $( $field: R<$domain, $codomain>, )*
            }

            /// Wrapper struct for relation checkers during transaction commit.
            ///
            /// This struct holds the checking state for all relations during the
            /// commit process, allowing batch operations across all relations.
            pub(crate) struct RelationCheckers {
                $( $field: CheckRelation<$domain, $codomain, FjallProvider<$domain, $codomain>>, )*
            }

            impl RelationCheckers {
                /// Check all relations for conflicts with the given working sets.
                ///
                /// Returns `true` if all relations pass conflict checking, `false` if any fail.
                fn check_all(&mut self, ws: &RelationWorkingSets) -> bool {
                    true $( && self.$field.check(&ws.$field).is_ok() )*
                }

                /// Check if all relation checkers are clean (no pending changes).
                ///
                /// Returns `true` if no relations have pending changes, `false` otherwise.
                fn all_clean(&self) -> bool {
                    true $( && !self.$field.dirty() )*
                }

                /// Apply all working sets to their respective relations.
                ///
                /// Consumes the working sets and applies them to the relations.
                /// Returns `Ok(self)` if all applications succeed, `Err(())` if any fail.
                fn apply_all(mut self, ws: RelationWorkingSets) -> Result<Self, ()> {
                    $(
                        if self.$field.apply(ws.$field).is_err() {
                            return Err(());
                        }
                    )*
                    Ok(self)
                }

                /// Commit all changes to the relations with appropriate write locking.
                ///
                /// This method takes write locks only on relations that have changes,
                /// minimizing lock contention during the commit process.
                fn commit_all(self, relations: &Relations) {
                    $(
                        let [<$field _lock>] = self.$field.dirty().then(|| relations.$field.write_lock());
                        self.$field.commit([<$field _lock>]);
                    )*
                }
            }

            impl Relations {
                /// Initialize all relations from the given keyspace and configuration.
                ///
                /// This method creates partitions, providers, and relations for each
                /// defined relation, and seeds them by scanning for existing data.
                ///
                /// # Parameters
                /// - `keyspace`: The fjall keyspace to create partitions in
                /// - `config`: Database configuration containing partition options
                ///
                /// # Panics
                /// Panics if any partition creation or relation seeding fails.
                fn init(keyspace: &fjall::Keyspace, config: &DatabaseConfig) -> Self {
                    $(
                        // Create partition using field name as partition name
                        let [<$field _partition>] = keyspace
                            .open_partition(
                                stringify!($field),
                                config
                                    .$field
                                    .clone()
                                    .unwrap_or_default()
                                    .partition_options(),
                            )
                            .unwrap();

                        // Create provider with field name as identifier
                        let [<$field _provider>] = FjallProvider::new(stringify!($field), [<$field _partition>]);

                        // Create relation with symbolized field name
                        let [<$field _relation>] = define_relations!(@create_relation $arrow, $field, [<$field _provider>]);

                        // Seed the relation by scanning all existing data
                        [<$field _relation>]
                            .scan(&|_, _| true)
                            .expect(concat!("Failed to seed ", stringify!($field)));
                    )*

                    Relations {
                        $( $field: [<$field _relation>], )*
                    }
                }

                /// Stop all relation providers.
                ///
                /// This method stops background processing for all relation providers.
                /// Should be called during database shutdown.
                fn stop_all(&self) {
                    $( self.$field.stop_provider().unwrap(); )*
                }

                /// Begin the checking phase for all relations.
                ///
                /// Creates RelationCheckers for all relations, which can then be used
                /// to check for conflicts during transaction commit.
                fn begin_check_all(&self) -> RelationCheckers {
                    RelationCheckers {
                        $( $field: self.$field.begin_check(), )*
                    }
                }

                /// Start a new transaction across all relations.
                ///
                /// Creates a WorldStateTransaction with relation transactions for all
                /// defined relations, along with the necessary channels and caches.
                ///
                /// # Parameters
                /// - `tx`: The transaction timestamp and metadata
                /// - `commit_channel`: Channel for sending commit requests
                /// - `usage_channel`: Channel for requesting database usage statistics
                /// - `sequences`: Array of sequence counters
                /// - `verb_resolution_cache`: Forked verb resolution cache
                /// - `prop_resolution_cache`: Forked property resolution cache
                /// - `ancestry_cache`: Forked ancestry cache
                #[allow(clippy::too_many_arguments)]
                fn start_transaction(&self,
                    tx: Tx,
                    commit_channel: Sender<CommitSet>,
                    usage_channel: Sender<oneshot::Sender<usize>>,
                    sequences: [Arc<CachePadded<AtomicI64>>; 16],
                    verb_resolution_cache: Box<VerbResolutionCache>,
                    prop_resolution_cache: Box<PropResolutionCache>,
                    ancestry_cache: Box<AncestryCache>
                ) -> WorldStateTransaction {
                    WorldStateTransaction {
                        tx: tx.clone(),
                        commit_channel,
                        usage_channel,
                        $( $field: self.$field.start(&tx), )*
                        sequences,
                        verb_resolution_cache,
                        prop_resolution_cache,
                        ancestry_cache,
                        has_mutations: false,
                    }
                }


                /// Send barrier messages to all providers to track transaction timestamp.
                ///
                /// This ensures transaction ordering consistency by recording the transaction
                /// timestamp in all provider logs after write transactions commit.
                pub fn send_barrier(&self, barrier_timestamp: Timestamp) -> Result<(), crate::tx_management::Error> {
                    $( self.$field.source().send_barrier(barrier_timestamp)?; )*
                    Ok(())
                }

                /// Wait for all writes up to the specified barrier timestamp to be completed in all providers.
                ///
                /// This ensures that all relation providers have fully processed writes up to
                /// the barrier before returning. Critical for ensuring snapshots capture a
                /// consistent view of the database at a specific point in time.
                pub fn wait_for_write_barrier(&self, barrier_timestamp: Timestamp, timeout: std::time::Duration) -> Result<(), crate::tx_management::Error> {
                    $( self.$field.source().wait_for_write_barrier(barrier_timestamp, timeout)?; )*
                    Ok(())
                }
            }

            /// Working sets for all relations, including caches.
            ///
            /// This struct contains the working sets for all relations along with
            /// the resolution caches used during transaction processing.
            pub(crate) struct WorkingSets {
                #[allow(dead_code)]
                pub(crate) tx: Tx,
                $( pub(crate) $field: WorkingSet<$domain, $codomain>, )*
                pub(crate) verb_resolution_cache: Box<VerbResolutionCache>,
                pub(crate) prop_resolution_cache: Box<PropResolutionCache>,
                pub(crate) ancestry_cache: Box<AncestryCache>,
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
                /// - `Box<VerbResolutionCache>`: Verb resolution cache
                /// - `Box<PropResolutionCache>`: Property resolution cache
                /// - `Box<AncestryCache>`: Ancestry cache
                fn extract_relation_working_sets(self) -> (RelationWorkingSets, Box<VerbResolutionCache>, Box<PropResolutionCache>, Box<AncestryCache>) {
                    let ws = RelationWorkingSets {
                        $( $field: self.$field, )*
                    };
                    (ws, self.verb_resolution_cache, self.prop_resolution_cache, self.ancestry_cache)
                }
            }

            /// Working sets for relations only, without caches.
            ///
            /// This struct contains only the working sets for relations, with caches
            /// separated out to handle ownership during commit processing.
            pub(crate) struct RelationWorkingSets {
                $( $field: WorkingSet<$domain, $codomain>, )*
            }

            /// Transaction state for all database relations.
            ///
            /// This struct represents an active transaction that can read from and write to
            /// all defined database relations. It contains relation transactions for each
            /// relation, along with caches and channels needed for transaction processing.
            pub struct WorldStateTransaction {
                #[allow(dead_code)]
                pub(crate) tx: Tx,
                /// Channel to send commit requests to the main database thread
                pub(crate) commit_channel: Sender<CommitSet>,
                /// Channel to request current database disk usage
                pub(crate) usage_channel: Sender<oneshot::Sender<usize>>,
                /// Relation transactions for each defined relation
                $( pub(crate) $field: RelationTransaction<$domain, $codomain, R<$domain, $codomain>>, )*
                /// Array of sequence counters for object ID generation
                pub(crate) sequences: [Arc<CachePadded<AtomicI64>>; 16],
                /// Local fork of the verb resolution cache
                pub(crate) verb_resolution_cache: Box<VerbResolutionCache>,
                /// Local fork of the property resolution cache
                pub(crate) prop_resolution_cache: Box<PropResolutionCache>,
                /// Local fork of the ancestry cache
                pub(crate) ancestry_cache: Box<AncestryCache>,
                /// Whether this transaction has performed any mutations
                pub(crate) has_mutations: bool,
            }

            impl WorldStateTransaction {
                /// Extract working sets from all relation transactions.
                ///
                /// This method collects the working sets from all relation transactions
                /// and packages them into a WorkingSets struct for commit processing.
                ///
                /// # Errors
                /// Returns an error if any relation transaction fails to produce a working set.
                pub(crate) fn into_working_sets(self) -> Result<Box<WorkingSets>, moor_common::model::WorldStateError> {
                    $(
                        let $field = self.$field.working_set()?;
                    )*

                    let ws = Box::new(WorkingSets {
                        tx: self.tx,
                        $( $field, )*
                        verb_resolution_cache: self.verb_resolution_cache,
                        prop_resolution_cache: self.prop_resolution_cache,
                        ancestry_cache: self.ancestry_cache,
                    });

                    Ok(ws)
                }
            }

            /// Sequence constant for maximum object ID tracking.
            ///
            /// This constant identifies the sequence used to track the highest object ID
            /// that has been allocated, used for generating new unique object IDs.
            pub const SEQUENCE_MAX_OBJECT: usize = 0;
        }
    };
    
    // Helper rule to create a relation based on arrow type
    (@create_relation =>, $field:ident, $provider:ident) => {
        Relation::new(Symbol::mk(stringify!($field)), Arc::new($provider))
    };
    
    (@create_relation ==, $field:ident, $provider:ident) => {
        Relation::new_with_secondary(
            Symbol::mk(stringify!($field)),
            Arc::new($provider)
        )
    };
}

// Re-export the macro for use in other modules
pub(crate) use define_relations;