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

pub mod common;
pub mod event_log;
pub mod program;
pub mod rpc;

pub mod convert {
    pub use crate::schema::{convert_common::*, convert_errors::*, convert_events::*};
}

// Generated schemas
mod convert_common;
mod convert_events;
// Made public for event_log usage
mod convert_errors;

#[allow(dead_code, clippy::all)]
mod schemas_generated;
