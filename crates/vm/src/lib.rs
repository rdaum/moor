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

use moor_common::model::{ObjFlag, VerbDef, VerbDispatch, VerbDispatchResult, WorldStateError};
use moor_common::util::BitEnum;
use moor_var::{Obj, Symbol, Var, program::ProgramType};
use uuid::Uuid;

pub(crate) mod activation;
pub(crate) mod config;
pub(crate) mod environment;
pub(crate) mod moo_execute;
pub(crate) mod moo_frame;
pub(crate) mod scatter_assign;
pub(crate) mod vm_unwind;

pub use activation::{Activation, BuiltinFrame, CallProgram, Frame};
pub use config::FeaturesConfig;
pub use environment::Environment;
pub use moo_execute::{
    CommandVerbExecutionRequest, ExecutionResult, Fork, TaskSuspend, VerbExecutionRequest,
    moo_frame_execute,
};
pub use moo_frame::{CatchType, MooStackFrame, PcType, ProgramSlot, Scope, ScopeType};
pub use scatter_assign::{ScatterResult, scatter_assign};
pub use vm_unwind::FinallyReason;

pub(crate) mod exec_state;

pub use exec_state::{Caller, ExecState, ProgramCacheLocalSnapshot};

/// A phantom type for explicitly marking types as !Sync
pub type PhantomUnsync = PhantomData<Cell<()>>;

/// Services the VM needs from its host environment.
/// Monomorphized at the call site for zero-cost dispatch.
pub trait VmHost {
    fn retrieve_property(
        &mut self,
        perms: &Obj,
        obj: &Obj,
        prop: Symbol,
    ) -> Result<Var, WorldStateError>;

    fn update_property(
        &mut self,
        perms: &Obj,
        obj: &Obj,
        prop: Symbol,
        value: &Var,
    ) -> Result<(), WorldStateError>;

    fn flags_of(&mut self, obj: &Obj) -> Result<BitEnum<ObjFlag>, WorldStateError>;

    fn valid(&mut self, obj: &Obj) -> Result<bool, WorldStateError>;

    fn dispatch_verb(
        &mut self,
        perms: &Obj,
        dispatch: VerbDispatch<'_>,
    ) -> Result<Option<VerbDispatchResult>, WorldStateError>;

    fn parent_of(&mut self, perms: &Obj, obj: &Obj) -> Result<Obj, WorldStateError>;

    /// Resolve a verb's program by UUID. Used by the program cache.
    fn retrieve_verb(
        &mut self,
        perms: &Obj,
        obj: &Obj,
        uuid: Uuid,
    ) -> Result<(ProgramType, VerbDef), WorldStateError>;
}
