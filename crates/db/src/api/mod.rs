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

//! Public integration surface for the transactional DB engine.
//!
//! - `world_state`: adapter exposing `WorldState` operations over a DB transaction.
//! - `loader_adapter`: loader/snapshot traits implemented on the same adapter.
//! - `gc`: garbage-collection trait and error types.

pub mod gc;
pub mod world_state;

#[cfg(test)]
mod gc_tests;
mod loader_adapter;
#[cfg(test)]
mod loader_tests;
