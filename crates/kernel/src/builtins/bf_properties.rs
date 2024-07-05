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

use moor_compiler::offset_for_builtin;
use moor_values::model::{PropAttrs, PropFlag};
use moor_values::util::BitEnum;
use moor_values::var::Error::{E_ARGS, E_INVARG, E_TYPE};
use moor_values::var::Symbol;
use moor_values::var::Variant;
use moor_values::var::{v_bool, v_list, v_none, v_objid, v_string};
use moor_values::var::{v_empty_list, List};

use crate::bf_declare;
use crate::builtins::BfErr::Code;
use crate::builtins::BfRet::Ret;
use crate::builtins::{world_state_bf_err, BfCallState, BfErr, BfRet, BuiltinFunction};

// property_info (obj <object>, str <prop-name>)              => list\
//  {<owner>, <perms> }
fn bf_property_info(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(Code(E_ARGS));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Err(Code(E_TYPE));
    };
    let Variant::Str(prop_name) = bf_args.args[1].variant() else {
        return Err(Code(E_TYPE));
    };
    let (_, perms) = bf_args
        .world_state
        .get_property_info(
            bf_args.task_perms_who(),
            *obj,
            Symbol::mk_case_insensitive(prop_name.as_str()),
        )
        .map_err(world_state_bf_err)?;
    let owner = perms.owner();
    let flags = perms.flags();

    // Turn perm flags into string: r w c
    let mut perms = String::new();
    if flags.contains(PropFlag::Read) {
        perms.push('r');
    }
    if flags.contains(PropFlag::Write) {
        perms.push('w');
    }
    if flags.contains(PropFlag::Chown) {
        perms.push('c');
    }

    Ok(Ret(v_list(&[v_objid(owner), v_string(perms)])))
}
bf_declare!(property_info, bf_property_info);

enum InfoParseResult {
    Fail(moor_values::var::Error),
    Success(PropAttrs),
}

fn info_to_prop_attrs(info: &List) -> InfoParseResult {
    if info.len() < 2 || info.len() > 3 {
        return InfoParseResult::Fail(E_ARGS);
    }

    let owner = info.get(0).unwrap();
    let Variant::Obj(owner) = owner.variant() else {
        return InfoParseResult::Fail(E_TYPE);
    };
    let perms = info.get(1).unwrap();
    let Variant::Str(perms) = perms.variant() else {
        return InfoParseResult::Fail(E_TYPE);
    };
    let name = if info.len() == 3 {
        let name = info.get(2).unwrap();
        let Variant::Str(name) = name.variant() else {
            return InfoParseResult::Fail(E_TYPE);
        };
        Some(name.to_string())
    } else {
        None
    };

    let mut flags = BitEnum::new();
    for c in perms.as_str().chars() {
        match c {
            'r' => flags |= PropFlag::Read,
            'w' => flags |= PropFlag::Write,
            'c' => flags |= PropFlag::Chown,
            _ => return InfoParseResult::Fail(E_INVARG),
        }
    }

    InfoParseResult::Success(PropAttrs {
        name,
        value: None,
        location: None,
        owner: Some(*owner),
        flags: Some(flags),
    })
}

fn bf_set_property_info(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 3 {
        return Err(Code(E_ARGS));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Err(Code(E_TYPE));
    };
    let Variant::Str(prop_name) = bf_args.args[1].variant() else {
        return Err(Code(E_TYPE));
    };
    let Variant::List(info) = bf_args.args[2].variant() else {
        return Err(Code(E_TYPE));
    };

    let attrs = match info_to_prop_attrs(info) {
        InfoParseResult::Fail(e) => {
            return Err(Code(e));
        }
        InfoParseResult::Success(a) => a,
    };

    bf_args
        .world_state
        .set_property_info(
            bf_args.task_perms_who(),
            *obj,
            Symbol::mk_case_insensitive(prop_name.as_str()),
            attrs,
        )
        .map_err(world_state_bf_err)?;
    Ok(Ret(v_empty_list()))
}
bf_declare!(set_property_info, bf_set_property_info);

fn bf_is_clear_property(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(Code(E_ARGS));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Err(Code(E_TYPE));
    };
    let Variant::Str(prop_name) = bf_args.args[1].variant() else {
        return Err(Code(E_TYPE));
    };
    let is_clear = bf_args
        .world_state
        .is_property_clear(
            bf_args.task_perms_who(),
            *obj,
            Symbol::mk_case_insensitive(prop_name.as_str()),
        )
        .map_err(world_state_bf_err)?;
    Ok(Ret(v_bool(is_clear)))
}
bf_declare!(is_clear_property, bf_is_clear_property);

fn bf_clear_property(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(Code(E_ARGS));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Err(Code(E_TYPE));
    };
    let Variant::Str(prop_name) = bf_args.args[1].variant() else {
        return Err(Code(E_TYPE));
    };
    bf_args
        .world_state
        .clear_property(
            bf_args.task_perms_who(),
            *obj,
            Symbol::mk_case_insensitive(prop_name.as_str()),
        )
        .map_err(world_state_bf_err)?;
    Ok(Ret(v_empty_list()))
}
bf_declare!(set_clear_property, bf_clear_property);

// add_property (obj <object>, str <prop-name>, <value>, list <info>) => none
fn bf_add_property(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 4 {
        return Err(Code(E_ARGS));
    }

    let (Variant::Obj(location), Variant::Str(name), value, Variant::List(info)) = (
        bf_args.args[0].variant(),
        bf_args.args[1].variant(),
        bf_args.args[2].clone(),
        bf_args.args[3].variant(),
    ) else {
        return Err(Code(E_ARGS));
    };

    let attrs = match info_to_prop_attrs(info) {
        InfoParseResult::Fail(e) => {
            return Err(Code(e));
        }
        InfoParseResult::Success(a) => a,
    };

    bf_args
        .world_state
        .define_property(
            bf_args.task_perms_who(),
            *location,
            *location,
            Symbol::mk_case_insensitive(name.as_str()),
            attrs.owner.unwrap(),
            attrs.flags.unwrap(),
            Some(value),
        )
        .map_err(world_state_bf_err)?;
    Ok(Ret(v_none()))
}
bf_declare!(add_property, bf_add_property);

fn bf_delete_property(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(Code(E_ARGS));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Err(Code(E_TYPE));
    };
    let Variant::Str(prop_name) = bf_args.args[1].variant() else {
        return Err(Code(E_TYPE));
    };
    bf_args
        .world_state
        .delete_property(
            bf_args.task_perms_who(),
            *obj,
            Symbol::mk_case_insensitive(prop_name.as_str()),
        )
        .map_err(world_state_bf_err)?;
    Ok(Ret(v_empty_list()))
}
bf_declare!(delete_property, bf_delete_property);

pub(crate) fn register_bf_properties(builtins: &mut [Box<dyn BuiltinFunction>]) {
    builtins[offset_for_builtin("property_info")] = Box::new(BfPropertyInfo {});
    builtins[offset_for_builtin("set_property_info")] = Box::new(BfSetPropertyInfo {});
    builtins[offset_for_builtin("is_clear_property")] = Box::new(BfIsClearProperty {});
    builtins[offset_for_builtin("clear_property")] = Box::new(BfSetClearProperty {});
    builtins[offset_for_builtin("add_property")] = Box::new(BfAddProperty {});
    builtins[offset_for_builtin("delete_property")] = Box::new(BfDeleteProperty {});
}
