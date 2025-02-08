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

use crate::bf_declare;
use crate::builtins::{BfCallState, BfErr, BfRet, BuiltinFunction};
use moor_compiler::offset_for_builtin;
use moor_values::Error::{E_ARGS, E_RANGE, E_TYPE};
use moor_values::{v_list, Var, Variant};
use moor_values::{Associative, Sequence};
/// Returns a copy of map with the value corresponding to key removed. If key is not a valid key, then E_RANGE is raised.
fn bf_mapdelete(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::Code(E_ARGS));
    }

    let Variant::Map(m) = &bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };

    if matches!(
        bf_args.args[1].variant(),
        Variant::Map(_) | Variant::List(_)
    ) {
        return Err(BfErr::Code(E_TYPE));
    }

    let (nm, Some(_)) = m.remove(&bf_args.args[1], false) else {
        return Err(BfErr::Code(E_RANGE));
    };

    Ok(BfRet::Ret(nm))
}
bf_declare!(mapdelete, bf_mapdelete);

fn bf_mapkeys(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let Variant::Map(m) = &bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };

    let keys: Vec<Var> = m.iter().map(|kv| kv.0.clone()).collect();

    Ok(BfRet::Ret(v_list(&keys)))
}
bf_declare!(mapkeys, bf_mapkeys);

fn bf_mapvalues(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let Variant::Map(m) = &bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };

    let values: Vec<Var> = m.iter().map(|kv| kv.1.clone()).collect();

    Ok(BfRet::Ret(v_list(&values)))
}
bf_declare!(mapvalues, bf_mapvalues);

fn bf_maphaskey(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::Code(E_ARGS));
    }

    let Variant::Map(m) = &bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };

    if matches!(
        bf_args.args[1].variant(),
        Variant::Map(_) | Variant::List(_)
    ) {
        return Err(BfErr::Code(E_TYPE));
    }

    let contains = m
        .contains_key(&bf_args.args[1], false)
        .map_err(BfErr::Code)?;
    Ok(BfRet::Ret(bf_args.v_bool(contains)))
}
bf_declare!(maphaskey, bf_maphaskey);

pub(crate) fn register_bf_maps(builtins: &mut [Box<dyn BuiltinFunction>]) {
    builtins[offset_for_builtin("mapdelete")] = Box::new(BfMapdelete {});
    builtins[offset_for_builtin("mapkeys")] = Box::new(BfMapkeys {});
    builtins[offset_for_builtin("mapvalues")] = Box::new(BfMapvalues {});
    builtins[offset_for_builtin("maphaskey")] = Box::new(BfMaphaskey {});
}
