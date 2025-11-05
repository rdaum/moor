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

//! Builtin functions for flyweight manipulation and introspection.

use crate::vm::builtins::{BfCallState, BfErr, BfRet, BfRet::Ret, BuiltinFunction};
use moor_compiler::offset_for_builtin;
use moor_var::{E_ARGS, E_PERM, E_TYPE, Sequence, v_flyweight, v_map, v_sym};

/// MOO: `map slots(flyweight f)`
/// Returns the set of slots on the flyweight as a map.
fn bf_slots(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.config.flyweight_type {
        return Err(BfErr::ErrValue(E_PERM.msg("Flyweights not enabled")));
    }

    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(E_ARGS.with_msg(|| {
            format!("slots() takes 1 argument, got {}", bf_args.args.len())
        })));
    }

    let Some(f) = bf_args.args[0].as_flyweight() else {
        return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
            format!(
                "slots() expects a flyweight as the first argument, got {}",
                bf_args.args[0].type_code().to_literal()
            )
        })));
    };

    let slots_map = f.slots_as_map();
    let slots: Vec<_> = slots_map
        .iter()
        .map(|(k, v)| (v_sym(*k), v.clone()))
        .collect();
    let map = v_map(&slots);

    Ok(Ret(map))
}

/// MOO: `flyweight remove_slot(flyweight f, symbol slot_name)`
/// Returns copy of flyweight with the specified slot removed.
fn bf_remove_slot(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.config.flyweight_type {
        return Err(BfErr::ErrValue(E_PERM.msg("Flyweights not enabled")));
    }

    if bf_args.args.len() != 2 {
        return Err(BfErr::ErrValue(E_ARGS.with_msg(|| {
            format!(
                "remove_slot() takes 2 arguments, got {}",
                bf_args.args.len()
            )
        })));
    }

    let Some(f) = bf_args.args[0].as_flyweight() else {
        return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
            format!(
                "remove_slot() expects a flyweight as the first argument, got {}",
                bf_args.args[0].type_code().to_literal()
            )
        })));
    };

    let Ok(s) = bf_args.args[1].as_symbol() else {
        return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
            format!(
                "remove_slot() expects a symbol as the second argument, got {}",
                bf_args.args[1].type_code().to_literal()
            )
        })));
    };

    let new_f = f.remove_slot(s);
    Ok(Ret(v_flyweight(
        *new_f.delegate(),
        &new_f.slots(),
        new_f.contents().clone(),
    )))
}

/// MOO: `flyweight add_slot(flyweight f, symbol key, any value)`
/// Returns copy of flyweight with the slot added or updated.
fn bf_add_slot(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.config.flyweight_type {
        return Err(BfErr::ErrValue(E_PERM.msg("Flyweights not enabled")));
    }

    if bf_args.args.len() != 3 {
        return Err(BfErr::ErrValue(E_ARGS.with_msg(|| {
            format!("add_slot() takes 3 arguments, got {}", bf_args.args.len())
        })));
    }

    let Some(f) = bf_args.args[0].as_flyweight() else {
        return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
            format!(
                "add_slot() expects a flyweight as the first argument, got {}",
                bf_args.args[0].type_code().to_literal()
            )
        })));
    };

    let Ok(key) = bf_args.args[1].as_symbol() else {
        return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
            format!(
                "add_slot() expects a symbol as the second argument, got {}",
                bf_args.args[1].type_code().to_literal()
            )
        })));
    };

    let value = bf_args.args[2].clone();

    let new_f = f.add_slot(key, value);
    Ok(Ret(v_flyweight(
        *new_f.delegate(),
        &new_f.slots(),
        new_f.contents().clone(),
    )))
}

pub(crate) fn register_bf_flyweights(builtins: &mut [BuiltinFunction]) {
    builtins[offset_for_builtin("slots")] = bf_slots;
    builtins[offset_for_builtin("remove_slot")] = bf_remove_slot;
    builtins[offset_for_builtin("add_slot")] = bf_add_slot;
}
