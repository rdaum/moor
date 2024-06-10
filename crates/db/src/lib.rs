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

use moor_values::model::WorldStateError;
use moor_values::model::WorldStateSource;

use crate::loader::LoaderInterface;

mod db_loader_client;
pub mod db_worldstate;
pub mod loader;
mod relational_transaction;
mod relational_worldstate;
mod worldstate_tables;
pub mod worldstate_transaction;

mod worldstate_tests;

pub use relational_transaction::{RelationalError, RelationalTransaction};
pub use relational_worldstate::RelationalWorldStateTransaction;
pub use worldstate_tables::{WorldStateSequence, WorldStateTable};
pub use worldstate_tests::*;

pub trait Database {
    fn loader_client(self: Arc<Self>) -> Result<Box<dyn LoaderInterface>, WorldStateError>;
    fn world_state_source(self: Arc<Self>) -> Result<Arc<dyn WorldStateSource>, WorldStateError>;
}

/// Possible backend storage engines.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DatabaseFlavour {
    /// WiredTiger, a high-performance, scalable, transactional storage engine, also used in MongoDB.
    /// Adaptation still under development.
    WiredTiger,
    /// In-house in-memory MVCC transactional store based on copy-on-write hashes and trees and
    /// custom buffer pool management. Consider experimental.
    RelBox,
}

impl From<&str> for DatabaseFlavour {
    fn from(s: &str) -> Self {
        match s {
            "wiredtiger" => DatabaseFlavour::WiredTiger,
            "relbox" => DatabaseFlavour::RelBox,
            _ => panic!("Unknown database flavour: {}", s),
        }
    }
}
