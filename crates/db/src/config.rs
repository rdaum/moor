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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[derive(Default)]
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
