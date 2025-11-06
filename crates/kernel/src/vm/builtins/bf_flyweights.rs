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
use moor_var::{
    E_ARGS, E_PERM, E_TYPE, Flyweight, List, Sequence, Symbol, Var, Variant, v_map, v_sym,
};

fn ensure_enabled(bf_args: &BfCallState<'_>) -> Result<(), BfErr> {
    if !bf_args.config.flyweight_type {
        return Err(BfErr::ErrValue(E_PERM.msg("Flyweights not enabled")));
    }
    Ok(())
}

fn contents_list_from_var(var: &Var) -> Result<List, BfErr> {
    let Some(list) = var.as_list() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("flyweight contents must be a list"),
        ));
    };
    Ok(list.clone())
}

fn flyweight_to_var(f: Flyweight) -> Var {
    Var::from_variant(Variant::Flyweight(f))
}

/// MOO: `toflyweight(delegate [, slots_map [, contents_list]])`
fn bf_mk_flyweight(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    ensure_enabled(bf_args)?;

    if !(1..=3).contains(&bf_args.args.len()) {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("toflyweight() takes 1 to 3 arguments"),
        ));
    }

    let delegate = match bf_args.args[0].as_object() {
        Some(obj) => obj,
        None => {
            return Err(BfErr::ErrValue(
                E_TYPE.msg("toflyweight() expects object as first argument"),
            ));
        }
    };

    let mut slot_pairs: Vec<(Symbol, Var)> = Vec::new();
    if bf_args.args.len() >= 2 {
        let map = bf_args.map_or_alist_to_map(&bf_args.args[1])?;
        for (key, value) in map.iter() {
            let key_sym = key.as_symbol().map_err(|_| {
                BfErr::ErrValue(
                    E_TYPE.msg("toflyweight() slot keys must be symbols or string values"),
                )
            })?;
            slot_pairs.push((key_sym, value.clone()));
        }
    }

    let contents = if bf_args.args.len() == 3 {
        contents_list_from_var(&bf_args.args[2])?
    } else {
        List::mk_list(&[])
    };

    let fly = Flyweight::mk_flyweight(delegate, &slot_pairs, contents);
    Ok(Ret(flyweight_to_var(fly)))
}

/// MOO: `flyslots(flyweight f)`
fn bf_flyslots(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    ensure_enabled(bf_args)?;

    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(E_ARGS.msg("flyslots() takes 1 argument")));
    }

    let Some(f) = bf_args.args[0].as_flyweight() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("flyslots() expects flyweight as first argument"),
        ));
    };

    let slots = f
        .slots_as_map()
        .iter()
        .map(|(k, v)| (v_sym(*k), v.clone()))
        .collect::<Vec<_>>();
    Ok(Ret(v_map(&slots)))
}

/// MOO: `flycontents(flyweight f)`
fn bf_flycontents(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    ensure_enabled(bf_args)?;

    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("flycontents() takes 1 argument"),
        ));
    }

    let Some(f) = bf_args.args[0].as_flyweight() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("flycontents() expects flyweight as first argument"),
        ));
    };

    Ok(Ret(Var::from_variant(Variant::List(f.contents().clone()))))
}

/// MOO: `flyslotset(flyweight f, symbol key, any value)`
fn bf_flyslotset(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    ensure_enabled(bf_args)?;

    if bf_args.args.len() != 3 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("flyslotset() takes 3 arguments"),
        ));
    }

    let Some(f) = bf_args.args[0].as_flyweight() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("flyslotset() expects flyweight as first argument"),
        ));
    };

    let key = bf_args.args[1]
        .as_symbol()
        .map_err(|_| BfErr::ErrValue(E_TYPE.msg("flyslotset() expects symbol key")))?;

    let value = bf_args.args[2].clone();
    let new_f = f.add_slot(key, value);
    Ok(Ret(flyweight_to_var(new_f)))
}

/// MOO: `flyslotremove(flyweight f, symbol key)`
fn bf_flyslotremove(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    ensure_enabled(bf_args)?;

    if bf_args.args.len() != 2 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("flyslotremove() takes 2 arguments"),
        ));
    }

    let Some(f) = bf_args.args[0].as_flyweight() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("flyslotremove() expects flyweight as first argument"),
        ));
    };

    let key = bf_args.args[1]
        .as_symbol()
        .map_err(|_| BfErr::ErrValue(E_TYPE.msg("flyslotremove() expects symbol key")))?;

    let new_f = f.remove_slot(key);
    Ok(Ret(flyweight_to_var(new_f)))
}

pub(crate) fn register_bf_flyweights(builtins: &mut [BuiltinFunction]) {
    builtins[offset_for_builtin("toflyweight")] = bf_mk_flyweight;
    builtins[offset_for_builtin("flyslots")] = bf_flyslots;
    builtins[offset_for_builtin("flycontents")] = bf_flycontents;
    builtins[offset_for_builtin("flyslotset")] = bf_flyslotset;
    builtins[offset_for_builtin("flyslotremove")] = bf_flyslotremove;
}
