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

use fjall::KeyspaceCreateOptions;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DatabaseConfig {
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
    pub object_last_move: Option<TableConfig>,
    pub anonymous_object_metadata: Option<TableConfig>,
}

const LARGE_MEMTABLE_SIZE: u64 = 512 * 1024 * 1024; // 512 MiB

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            object_location: None,
            object_contents: None,
            object_flags: None,
            object_parent: None,
            object_children: None,
            object_owner: None,
            object_name: None,
            object_verbdefs: None,
            // Verbs and propvalues are hot tables under write pressure - larger memtables
            // reduce L0 segment accumulation and fjall backpressure stalls
            object_verbs: Some(TableConfig {
                max_memtable_size: Some(LARGE_MEMTABLE_SIZE),
            }),
            object_propdefs: None,
            object_propvalues: Some(TableConfig {
                max_memtable_size: Some(LARGE_MEMTABLE_SIZE),
            }),
            object_propflags: None,
            object_last_move: None,
            anonymous_object_metadata: None,
        }
    }
}

/// Per-table configuration.
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct TableConfig {
    /// Various fjall keyspace creation options.
    /// Refer to the fjall documentation for more information.
    pub max_memtable_size: Option<u64>,
}

impl TableConfig {
    pub fn keyspace_options(&self) -> KeyspaceCreateOptions {
        let mut opts = KeyspaceCreateOptions::default();
        if let Some(max_memtable_size) = self.max_memtable_size {
            opts = opts.max_memtable_size(max_memtable_size);
        }
        opts
    }
}
