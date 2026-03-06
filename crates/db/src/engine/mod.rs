// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

//! Core transactional database engine internals.
//!
//! This module owns the lower-level execution path for transactions:
//! snapshot acquisition, relation commit/check plumbing, and durable write
//! coordination. The public crate API re-exports selected entry points, while
//! keeping these implementation details private.

pub(crate) mod moor_db;
#[cfg(test)]
mod moor_db_concurrent_tests;
#[cfg(test)]
mod moor_db_tests;
mod relation_defs;
mod ws_transaction;

pub(crate) use moor_db::MoorDB;
pub use moor_db::SEQUENCE_MAX_OBJECT;
