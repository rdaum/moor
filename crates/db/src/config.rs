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

use fjall::PartitionCreateOptions;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// The rate to run cache eviction cycles at.
    pub cache_eviction_interval: Duration,
    /// The default eviction threshold for each transaction-global cache. If a value is not specified
    /// for a specific table, this value will be used.
    /// Every `cache_eviction_interval` seconds, the total memory usage of the cache will be checked,
    /// and if it exceeds this threshold, random entries will be put onto the eviction queue.
    /// If they are still there, untouched, by the next eviction cycle, they will be removed.
    pub default_eviction_threshold: usize,

    /// Per-table configurations
    pub object_location: TableConfig,
    pub object_contents: TableConfig,
    pub object_flags: TableConfig,
    pub object_parent: TableConfig,
    pub object_children: TableConfig,
    pub object_owner: TableConfig,
    pub object_name: TableConfig,
    pub object_verbdefs: TableConfig,
    pub object_verbs: TableConfig,
    pub object_propdefs: TableConfig,
    pub object_propvalues: TableConfig,
    pub object_propflags: TableConfig,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            cache_eviction_interval: Duration::from_secs(60),
            // 4MB
            default_eviction_threshold: 1 << 22,
            object_location: TableConfig::default(),
            object_contents: TableConfig::default(),
            object_flags: TableConfig::default(),
            object_parent: TableConfig::default(),
            object_children: TableConfig::default(),
            object_owner: TableConfig::default(),
            object_name: TableConfig::default(),
            object_verbdefs: TableConfig::default(),
            object_verbs: TableConfig::default(),
            object_propdefs: TableConfig::default(),
            object_propvalues: TableConfig::default(),
            object_propflags: TableConfig::default(),
        }
    }
}

/// Per-table configuration.
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct TableConfig {
    /// The maximum number of bytes to keep in the global transactional cache for this table,
    /// before starting to evict entries.
    pub cache_eviction_threshold: Option<usize>,

    /// Various fjall partition creation options.
    /// Refer to the fjall documentation for more information.
    pub max_memtable_size: Option<u32>,
    pub block_size: Option<u32>,
}

impl TableConfig {
    pub fn partition_options(&self) -> PartitionCreateOptions {
        let mut opts = PartitionCreateOptions::default();
        if let Some(max_memtable_size) = self.max_memtable_size {
            opts = opts.max_memtable_size(max_memtable_size);
        }
        if let Some(block_size) = self.block_size {
            opts = opts.block_size(block_size);
        }
        opts
    }
}
