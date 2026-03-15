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

use crate::task_context::with_current_transaction_mut;
use moor_common::model::WorldStateError;
use moor_var::{Obj, Symbol, Var};
use moor_vm::WorldStateCallback;

/// Bridges VM property access to the kernel's TLS-based transaction context.
pub(crate) struct KernelHost;

impl WorldStateCallback for KernelHost {
    fn retrieve_property(
        &mut self,
        perms: &Obj,
        obj: &Obj,
        prop: Symbol,
    ) -> Result<Var, WorldStateError> {
        with_current_transaction_mut(|ws| ws.retrieve_property(perms, obj, prop))
    }

    fn update_property(
        &mut self,
        perms: &Obj,
        obj: &Obj,
        prop: Symbol,
        value: &Var,
    ) -> Result<(), WorldStateError> {
        with_current_transaction_mut(|ws| ws.update_property(perms, obj, prop, value))
    }
}
