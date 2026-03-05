#![recursion_limit = "256"]

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

use moor_common::model::loader::{LoaderInterface, SnapshotInterface};
use moor_common::model::{WorldState, WorldStateError, WorldStateSource};
use moor_common::threading::spawn_efficient;
use std::{path::Path, sync::Arc};

mod api;
mod cache;
mod config;
mod engine;
mod model;
mod provider;
mod tx;

use crate::engine::MoorDB;

pub use api::world_state::DbWorldState;
pub use api::{
    gc::{GCError, GCInterface},
    world_state::db_counters,
};
pub use cache::property_pic_stats::{
    PropertyPicSnapshot, PropertyPicStats, record_vm_property_hint_get_prop,
    record_vm_property_hint_push_get_prop, record_vm_property_hint_put_prop,
    record_vm_property_hint_put_prop_at,
};
pub use cache::stats::CacheStats;
pub use cache::verb_pic_stats::{
    VerbPicSnapshot, VerbPicStats, record_vm_verb_hint_call_verb, record_vm_verb_hint_pass,
};
pub use cache::{
    ANCESTRY_CACHE_STATS, PROP_CACHE_STATS, PROPERTY_PIC_STATS, VERB_CACHE_STATS, VERB_PIC_STATS,
};
pub use cache::{
    ancestry_cache::AncestryCache, prop_cache::PropResolutionCache, verb_cache::VerbResolutionCache,
};
pub use config::{DatabaseConfig, TableConfig};
pub use model::{
    AnonymousObjectMetadata, BytesHolder, ObjAndUUIDHolder, StringHolder, SystemTimeHolder,
    UUIDHolder,
};
pub use provider::Provider;
pub use tx::{
    AcceptIdentical, CheckRelation, ConflictResolver, Error, FailOnConflict, PotentialConflict,
    ProposedOp, Relation, RelationCodomain, RelationCodomainHashable, RelationDomain,
    RelationIndex, RelationTransaction, SmartMergeResolver, Timestamp, Tx, WorkingSet,
};

pub type SnapshotCallback = Box<
    dyn FnOnce(Result<Box<dyn SnapshotInterface>, WorldStateError>) -> Result<(), WorldStateError>
        + Send,
>;

// Re-export sequence constants for use in VM
pub use engine::SEQUENCE_MAX_OBJECT;

pub trait Database: Send + WorldStateSource {
    fn loader_client(&self) -> Result<Box<dyn LoaderInterface>, WorldStateError>;
    fn create_snapshot(&self) -> Result<Box<dyn SnapshotInterface>, WorldStateError>;
    fn create_snapshot_async(&self, callback: SnapshotCallback) -> Result<(), WorldStateError>;
    fn gc_interface(&self) -> Result<Box<dyn GCInterface>, WorldStateError>;
}

#[derive(Clone)]
pub struct TxDB {
    storage: Arc<MoorDB>,
}

impl TxDB {
    pub fn open(path: Option<&Path>, database_config: DatabaseConfig) -> (Self, bool) {
        let (storage, fresh) = MoorDB::open(path, database_config);
        (Self { storage }, fresh)
    }

    /// Mark all relations as fully loaded from their backing providers.
    /// Call this after bulk import operations to enable optimized reads.
    pub fn mark_all_fully_loaded(&self) {
        self.storage.mark_all_fully_loaded();
    }
}

impl WorldStateSource for TxDB {
    fn new_world_state(&self) -> Result<Box<dyn WorldState>, WorldStateError> {
        let tx = self.storage.start_transaction();
        let tx = api::world_state::DbWorldState { tx };
        Ok(Box::new(tx))
    }

    fn checkpoint(&self) -> Result<(), WorldStateError> {
        // TODO: noop for now... but this should probably do a sync of sequences to disk and make
        //   sure all data is durable.
        Ok(())
    }
}

impl Database for TxDB {
    fn loader_client(&self) -> Result<Box<dyn LoaderInterface>, WorldStateError> {
        let tx = self.storage.start_transaction();
        let tx = api::world_state::DbWorldState { tx };
        Ok(Box::new(tx))
    }

    fn create_snapshot(&self) -> Result<Box<dyn SnapshotInterface>, WorldStateError> {
        self.storage
            .create_snapshot()
            .map_err(|e| WorldStateError::DatabaseError(e.to_string()))
    }

    fn create_snapshot_async(&self, callback: SnapshotCallback) -> Result<(), WorldStateError> {
        let storage = self.storage.clone();
        spawn_efficient("moor-snapshot", move || {
            let snapshot_result = storage
                .create_snapshot()
                .map_err(|e| WorldStateError::DatabaseError(e.to_string()));

            if let Err(e) = callback(snapshot_result) {
                tracing::error!("Snapshot callback failed: {}", e);
            }
        })
        .map_err(|e| {
            WorldStateError::DatabaseError(format!("Failed to spawn snapshot thread: {e}"))
        })?;
        Ok(())
    }

    fn gc_interface(&self) -> Result<Box<dyn GCInterface>, WorldStateError> {
        let tx = self.storage.start_transaction();
        let tx = api::world_state::DbWorldState { tx };
        Ok(Box::new(tx))
    }
}
