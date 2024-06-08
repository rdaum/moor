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

use moor_db::Database;
use std::sync::Arc;

mod rb_worldstate;
mod rel_transaction;

pub use rb_worldstate::RelBoxWorldState;
pub use rel_transaction::RelboxTransaction;

pub struct RelBoxDatabaseBuilder {
    path: Option<std::path::PathBuf>,
    memory_size: Option<usize>,
}

impl RelBoxDatabaseBuilder {
    pub fn new() -> Self {
        Self {
            path: None,
            memory_size: None,
        }
    }

    pub fn with_path(mut self, path: std::path::PathBuf) -> Self {
        self.path = Some(path);
        self
    }

    pub fn with_memory_size(mut self, memory_size_bytes: usize) -> Self {
        self.memory_size = Some(memory_size_bytes);
        self
    }

    /// Returns a new database instance. The second value in the result tuple is true if the
    /// database was newly created, and false if it was already present.
    pub fn open_db(&self) -> Result<(Arc<dyn Database + Send + Sync>, bool), String> {
        let (db, fresh) =
            RelBoxWorldState::open(self.path.clone(), self.memory_size.unwrap_or(1 << 40));
        Ok((Arc::new(db), fresh))
    }
}

impl Default for RelBoxDatabaseBuilder {
    fn default() -> Self {
        Self::new()
    }
}
