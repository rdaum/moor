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

use tracing::debug;

use moor_values::model::WorldState;
use moor_values::var::Error;
use moor_values::var::Error::{E_INVIND, E_TYPE};
use moor_values::var::Var;
use moor_values::var::Variant;

use crate::vm::{VMExecState, VM};

impl VM {
    /// VM-level property resolution.
    pub(crate) async fn resolve_property(
        &self,
        state: &mut VMExecState,
        world_state: &mut dyn WorldState,
        propname: Var,
        obj: Var,
    ) -> Result<Var, Error> {
        let Variant::Str(propname) = propname.variant() else {
            return Err(E_TYPE);
        };

        let Variant::Obj(obj) = obj.variant() else {
            return Err(E_INVIND);
        };

        let result = world_state
            .retrieve_property(state.top().permissions, *obj, propname.as_str())
            .await;
        let v = match result {
            Ok(v) => v,
            Err(e) => {
                debug!(obj = ?obj, propname = propname.as_str(), "Error resolving property");
                return Err(e.to_error_code());
            }
        };
        Ok(v)
    }

    /// VM-level property assignment
    pub(crate) async fn set_property(
        &self,
        state: &mut VMExecState,
        world_state: &mut dyn WorldState,
        propname: Var,
        obj: Var,
        value: Var,
    ) -> Result<Var, Error> {
        let (propname, obj) = match (propname.variant(), obj.variant()) {
            (Variant::Str(propname), Variant::Obj(obj)) => (propname, obj),
            (_, _) => {
                return Err(E_TYPE);
            }
        };

        let update_result = world_state
            .update_property(state.top().permissions, *obj, propname.as_str(), &value)
            .await;

        match update_result {
            Ok(()) => Ok(value),
            Err(e) => Err(e.to_error_code()),
        }
    }
}
