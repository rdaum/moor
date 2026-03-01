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

use super::{Caches, WorldStateSnapshot};
use crate::tx_management::Tx;
use arc_swap::ArcSwap;
use moor_common::util::CachePadded;
use std::sync::Arc;
use std::sync::atomic::AtomicI64;

/// Read-mostly cache publication metadata.
///
/// `version` tracks the world snapshot version that the cache publication is
/// valid for, so startup can avoid mixing caches across snapshot versions.
struct CachePublication {
    version: u64,
    caches: Arc<Caches>,
}

/// Coordinates publication and consumption of world snapshot state.
///
/// We keep root indexes/version and read-mostly resolution caches in separate
/// atomic planes so read-only cache commits can publish without rewriting the
/// root snapshot pointer.
pub(super) struct SnapshotPlanes {
    root_state: ArcSwap<WorldStateSnapshot>,
    cache_publication: ArcSwap<CachePublication>,
}

impl SnapshotPlanes {
    /// Initialize both publication planes from the same initial root snapshot.
    pub(super) fn new(initial_root: Arc<WorldStateSnapshot>) -> Self {
        let initial_cache_publication = Arc::new(CachePublication {
            version: initial_root.version,
            caches: initial_root.caches.clone(),
        });
        Self {
            root_state: ArcSwap::new(initial_root),
            cache_publication: ArcSwap::new(initial_cache_publication),
        }
    }

    /// Load a consistent startup snapshot and forked caches for a new transaction.
    ///
    /// If the cache sidecar matches the root snapshot version, use it. Otherwise,
    /// fall back to caches embedded in the root snapshot.
    pub(super) fn acquire_seed_caches(&self) -> (Arc<WorldStateSnapshot>, Caches) {
        let snapshot = self.root_state.load();
        let cache_publication = self.cache_publication.load();
        let base_caches = if cache_publication.version == snapshot.version {
            &cache_publication.caches
        } else {
            &snapshot.caches
        };
        (Arc::clone(&snapshot), base_caches.fork())
    }

    /// Publish read-only cache updates for the given snapshot version.
    pub(super) fn publish_read_only_cache(&self, snapshot_version: u64, combined_caches: Caches) {
        if !combined_caches.has_changed() {
            return;
        }

        let current_root = self.root_state.load();
        if current_root.version != snapshot_version {
            return;
        }

        self.cache_publication.store(Arc::new(CachePublication {
            version: snapshot_version,
            caches: Arc::new(combined_caches),
        }));
    }

    /// Publish a new committed root snapshot and synchronize cache sidecar.
    pub(super) fn publish_write_root(&self, next_root: Arc<WorldStateSnapshot>) {
        let next_cache_publication = Arc::new(CachePublication {
            version: next_root.version,
            caches: next_root.caches.clone(),
        });
        self.root_state.store(next_root);
        self.cache_publication.store(next_cache_publication);
    }

    pub(super) fn load_root(&self) -> Arc<WorldStateSnapshot> {
        self.root_state.load_full()
    }

    pub(super) fn update_root<F>(&self, f: F)
    where
        F: FnMut(&Arc<WorldStateSnapshot>) -> Arc<WorldStateSnapshot>,
    {
        self.root_state.rcu(f);
    }
}

/// Startup context for constructing a `WorldStateTransaction`.
pub(crate) struct TxSeed {
    pub(crate) tx: Tx,
    pub(crate) snapshot: Arc<WorldStateSnapshot>,
    pub(crate) sequences: Arc<[CachePadded<AtomicI64>; 16]>,
    pub(crate) caches: Caches,
}
