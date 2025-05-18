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

use fjall::PartitionCreateOptions;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const DEFAULT_EVICTION_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// The rate to run cache eviction cycles at.
    pub cache_eviction_interval: Option<Duration>,
    /// The default eviction threshold for each transaction-global cache. If a value is not specified
    /// for a specific table, this value will be used.
    /// Every `cache_eviction_interval` seconds, the total memory usage of the cache will be checked,
    /// and if it exceeds this threshold, random entries will be put onto the eviction queue.
    /// If they are still there, untouched, by the next eviction cycle, they will be removed.
    pub default_eviction_threshold: Option<usize>,

    /// Per-table configurations
    pub object_location: Option<TableConfig>,
    pub object_contents: Option<TableConfig>,
    pub object_flags: Option<TableConfig>,
    pub object_parent: Option<TableConfig>,
    pub object_children: Option<TableConfig>,
    pub object_owner: Option<TableConfig>,
    pub object_name: Option<TableConfig>,
    pub object_verbdefs: Option<TableConfig>,
    pub object_verbs: Option<TableConfig>,
    pub object_propdefs: Option<TableConfig>,
    pub object_propvalues: Option<TableConfig>,
    pub object_propflags: Option<TableConfig>,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            cache_eviction_interval: Some(DEFAULT_EVICTION_INTERVAL),
            // 64MB.
            default_eviction_threshold: Some(1 << 26),
            object_location: None,
            object_contents: None,
            object_flags: None,
            object_parent: None,
            object_children: None,
            object_owner: None,
            object_name: None,
            object_verbdefs: None,
            object_verbs: None,
            object_propdefs: None,
            object_propvalues: None,
            object_propflags: None,
        }
    }
}

/// Per-table configuration.
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct TableConfig {
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
