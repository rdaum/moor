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
pub mod config;
pub mod environment;
pub mod execute;
pub mod moo_frame;
pub mod scatter_assign;
pub mod vm_unwind;

pub use activation::{Activation, BfFrame, Frame};
pub use config::FeaturesConfig;
pub use execute::{ExecutionResult, moo_frame_execute};
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

pub mod exec_state;

pub use exec_state::{Caller, Fork, ProgramCacheLocalSnapshot, VMExecState};

/// Services the VM needs from its host environment.
/// Monomorphized at the call site for zero-cost dispatch.
pub trait WorldStateCallback {
    fn retrieve_property(
        &mut self,
        perms: &moor_var::Obj,
        obj: &moor_var::Obj,
        prop: moor_var::Symbol,
    ) -> Result<moor_var::Var, moor_common::model::WorldStateError>;

    fn update_property(
        &mut self,
        perms: &moor_var::Obj,
        obj: &moor_var::Obj,
        prop: moor_var::Symbol,
        value: &moor_var::Var,
    ) -> Result<(), moor_common::model::WorldStateError>;

    fn flags_of(
        &self,
        obj: &moor_var::Obj,
    ) -> Result<
        moor_common::util::BitEnum<moor_common::model::ObjFlag>,
        moor_common::model::WorldStateError,
    >;
}
