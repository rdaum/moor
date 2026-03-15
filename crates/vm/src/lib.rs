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

use std::{cell::Cell, marker::PhantomData};

pub mod activation;
pub mod environment;
pub mod moo_frame;
pub mod scatter_assign;
pub mod vm_unwind;

pub use activation::{Activation, BfFrame, Frame};
pub use moo_frame::{CatchType, MooStackFrame, PcType, Scope, ScopeType};
pub use scatter_assign::{ScatterResult, scatter_assign};
pub use vm_unwind::FinallyReason;

/// A phantom type for explicitly marking types as !Sync
pub type PhantomUnsync = PhantomData<Cell<()>>;

/// Stable handle to a program residing in a task-local program cache.
/// The pointer is valid for the duration of the owning task's transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgramSlot {
    pub program_ptr: usize,
    pub global_width: usize,
}
