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

mod global_cache;
mod tx_table;

pub use global_cache::{GlobalCache, Provider};
pub use tx_table::{TransactionalTable, WorkingSet};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Ord, PartialOrd)]
pub struct Timestamp(pub u64);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Tx {
    pub(crate) ts: Timestamp,
}

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("Duplicate key")]
    Duplicate,
    #[error("Conflict detected")]
    Conflict,
    #[error("Retrieval error")]
    RetrievalFailure,
    #[error("Store error")]
    StorageFailure,
    #[error("Encoding error")]
    EncodingFailure,
}
