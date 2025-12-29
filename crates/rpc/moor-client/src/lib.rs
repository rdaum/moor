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

//! Client interfaces for mooR world state access.
//!
//! This crate provides different client implementations for accessing
//! mooR world state:
//!
//! - [`in_memory`] - Direct in-memory access without network RPC,
//!   suitable for tooling like LSP servers.
//! - [`traits`] - Common traits for client implementations.

pub mod in_memory;
pub mod traits;

pub use in_memory::{InMemoryConfig, InMemoryWorldState};
pub use traits::{IntrospectionError, IntrospectionResult, MoorIntrospection};
