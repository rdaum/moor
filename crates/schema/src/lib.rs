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

//! FlatBuffer schema types organized by domain
//!
//! This module provides a clean, organized interface to all FlatBuffer types.
//! The actual generated code is kept private and accessed through these
//! domain-specific submodules.

// Re-export proc macros
pub use moor_schema_macros::{EnumFlatbuffer, define_enum_mapping};

/// Extension trait to convert any Display error to String
pub trait StrErr<T> {
    fn str_err(self) -> Result<T, String>;
}

impl<T, E: std::fmt::Display> StrErr<T> for Result<T, E> {
    fn str_err(self) -> Result<T, String> {
        self.map_err(|e| e.to_string())
    }
}

pub mod common;
pub mod event_log;
pub mod program;
pub mod rpc;
pub mod task;
pub mod var;
pub mod convert {
    pub use crate::{
        convert_common::*, convert_defs::*, convert_errors::*, convert_events::*,
        convert_program::*, convert_var::*,
    };
}

// Helper macros and utilities
#[macro_use]
pub mod macros;
pub mod packed_id;

// Generated schemas
mod convert_common;
mod convert_defs;
mod convert_events;
// Made public for event_log usage
mod convert_errors;
mod convert_var;

pub mod convert_program;
pub mod opcode_stream;
#[allow(dead_code, clippy::all)]
mod schemas_generated;
