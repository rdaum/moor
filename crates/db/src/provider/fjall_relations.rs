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

//! Fjall resource binding at open, seed, encode, and maintenance boundaries.
use super::{fjall_provider::FjallProvider, read::SnapshotReaders};
use crate::{
    AnonymousObjectMetadata, DatabaseConfig, DatabaseOpenError, EntityMetadataKey,
    ObjAndUUIDHolder, StringHolder,
    engine::moor_db::{Caches, DatabaseRelation, RelationChanges, Relations, WorldStateSnapshot},
    tx::{Error, Timestamp},
};
use moor_common::{
    model::{ObjFlag, PropDefs, PropPerms, VerbDefs},
    util::BitEnum,
};
use moor_var::{Obj, Var, program::ProgramType};
use std::sync::Arc;

type SeedResult = (
    WorldStateSnapshot,
    ahash::AHashMap<ObjAndUUIDHolder, super::property_value_store::PropertyValueChain>,
);

macro_rules! define_fjall_relations {
    ($( $field:ident $category:ident $policy:ident $arrow:tt $domain:ty, $codomain:ty ),* $(,)?) => {
        /// Physical relation handles used only by the Fjall adapter and encoder workers.
        pub(crate) struct FjallRelations {
            $( pub(crate) $field: FjallProvider<$domain, $codomain>, )*
        }

        impl FjallRelations {
            pub(crate) fn open(
                database: &fjall::Database,
                config: &DatabaseConfig,
                path: &std::path::Path,
            ) -> Result<Self, DatabaseOpenError> {
                Ok(Self {
                    $( $field: FjallProvider::new(
                        stringify!($field),
                        database.keyspace(
                            stringify!($field),
                            || config.$field.clone().unwrap_or_default().keyspace_options(),
                        ).map_err(|error| DatabaseOpenError::Keyspace {
                            path: path.to_path_buf(),
                            keyspace: stringify!($field),
                            detail: error.to_string(),
                        })?,
                    ), )*
                })
            }

            pub(crate) fn readers(
                &self,
                snapshot: &Arc<super::backend::FjallReadSnapshot>,
            ) -> SnapshotReaders {
                SnapshotReaders {
                    $( $field: Arc::new(define_fjall_relations!(
                        @reader $category, self.$field, snapshot
                    )), )*
                }
            }

            /// Stream one snapshot into resident indexes and the writer's private chain state.
            pub(crate) fn seed(
                &self,
                relations: &Relations,
                snapshot: &Arc<super::backend::FjallReadSnapshot>,
                path: &std::path::Path,
            ) -> Result<SeedResult, DatabaseOpenError> {
                let readers = self.readers(snapshot);
                let mut committed_ts = Timestamp(0);
                let mut chains = ahash::AHashMap::new();
                $(
                    let $field = {
                        let (index, max_ts) = define_fjall_relations!(
                            @seed $category, self.$field, relations.$field,
                            readers.$field, snapshot, chains
                        ).map_err(|error| DatabaseOpenError::SeedRelation {
                            path: path.to_path_buf(),
                            relation: stringify!($field),
                            detail: error.to_string(),
                        })?;
                        if !index.is_fully_resident() {
                            return Err(DatabaseOpenError::SeedRelation {
                                path: path.to_path_buf(),
                                relation: stringify!($field),
                                detail: Error::IncompleteIndex(
                                    moor_var::Symbol::mk(stringify!($field)),
                                ).to_string(),
                            });
                        }
                        committed_ts = committed_ts.max(max_ts);
                        Arc::from(index)
                    };
                )*
                Ok((WorldStateSnapshot {
                    version: 0,
                    committed_ts,
                    caches: Arc::new(Caches::new()),
                    $( $field, )*
                    commit_bloom: None,
                    bloom_since_version: 0,
                }, chains))
            }

            /// Bind accepted logical mutations to Fjall keyspaces on an encoder worker.
            pub(crate) fn working_sets_to_batch(
                &self,
                changes: RelationChanges,
                version: u64,
                timestamp: Timestamp,
            ) -> Result<super::batch_writer::CommitBatch, Error> {
                let mut ops = Vec::new();
                $(
                    if !changes.$field.is_empty() {
                        ops.extend(define_fjall_relations!(
                            @encode $category, self.$field, changes.$field
                        )?);
                    }
                )*
                Ok(super::batch_writer::CommitBatch::from_ops(version, timestamp, ops))
            }

            pub(crate) fn compact_relations(
                &self,
                relations: &[DatabaseRelation],
            ) -> Vec<crate::RelationCompactionResult> {
                pastey::paste! {
                    relations.iter().copied().map(|relation| match relation {
                        $( DatabaseRelation::[<$field:camel>] => {
                            super::fjall_maintenance::major_compact(
                                relation, self.$field.partition(),
                            )
                        }, )*
                    }).collect()
                }
            }
        }
    };
    (@reader Ordinary, $provider:expr, $snapshot:expr) => {
        super::fjall_reader::FjallReader::new($snapshot.clone(), $provider.partition().clone())
    };
    (@reader PropertyValueChain, $provider:expr, $snapshot:expr) => {
        super::fjall_reader::FjallPropertyReader::new($snapshot.clone(), $provider.partition().clone())
    };
    (@seed Ordinary, $provider:expr, $relation:expr, $reader:expr, $snapshot:expr, $chains:ident) => {
        (|| { $relation.seeded_index($reader.scan(super::read::ScanRequest::all())?) })()
    };
    (@seed PropertyValueChain, $provider:expr, $relation:expr, $reader:expr, $snapshot:expr, $chains:ident) => {
        $provider.seeded_property_value_index(&$snapshot.snapshot, &$relation)
            .map(|(index, ts, state)| {
                $chains.extend(state);
                (index, ts)
            })
    };
    (@encode Ordinary, $provider:expr, $changes:expr) => {
        $provider.encode_changes($changes)
    };
    (@encode PropertyValueChain, $provider:expr, $changes:expr) => {
        Ok::<_, Error>($provider.encode_property_value_changes($changes))
    };
}
crate::relation_registry::relation_registry!(define_fjall_relations);
