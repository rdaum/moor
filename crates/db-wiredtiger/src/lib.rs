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

use std::sync::Arc;

pub use crate::wtrel::relation::WiredTigerRelation;
use moor_db::Database;

pub use crate::worldstate::wt_worldstate::WiredTigerDB;
pub use crate::wtrel::rel_db::WiredTigerRelDb;
pub use crate::wtrel::rel_transaction::WiredTigerRelTransaction;

#[allow(dead_code, unused_imports)]
mod bindings;
mod worldstate;
mod wtrel;

pub struct WiredTigerDatabaseBuilder {
    path: Option<std::path::PathBuf>,
}

impl WiredTigerDatabaseBuilder {
    pub fn new() -> Self {
        Self { path: None }
    }

    pub fn with_path(mut self, path: std::path::PathBuf) -> Self {
        self.path = Some(path);
        self
    }

    /// Returns a new database instance. The second value in the result tuple is true if the
    /// database was newly created, and false if it was already present.
    pub fn open_db(&self) -> Result<(Arc<dyn Database>, bool), String> {
        let (db, fresh) = WiredTigerDB::open(self.path.as_ref());
        Ok((Arc::new(db), fresh))
    }
}

impl Default for WiredTigerDatabaseBuilder {
    fn default() -> Self {
        Self::new()
    }
}
