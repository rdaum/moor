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

use moor_common::model::{PropAttrs, PropFlag, prop_flags_string};
use moor_common::util::BitEnum;
use moor_compiler::offset_for_builtin;
use moor_var::Sequence;
use moor_var::Variant;
use moor_var::{E_ARGS, E_INVARG, E_TYPE};
use moor_var::{List, v_empty_list};
use moor_var::{v_list, v_none, v_obj, v_string};

use crate::bf_declare;
use crate::builtins::BfErr::{Code, ErrValue};
use crate::builtins::BfRet::Ret;
use crate::builtins::{BfCallState, BfErr, BfRet, BuiltinFunction, world_state_bf_err};

// property_info (obj <object>, str <prop-name>)              => list\
//  {<owner>, <perms> }
fn bf_property_info(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(Code(E_ARGS));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Err(Code(E_TYPE));
    };
    let prop_name = bf_args.args[1].as_symbol().map_err(ErrValue)?;
    let (_, perms) = bf_args
        .world_state
        .get_property_info(&bf_args.task_perms_who(), obj, prop_name)
        .map_err(world_state_bf_err)?;
    let owner = perms.owner();
    let flags = perms.flags();

    // Turn perm flags into string: r w c
    let perms = prop_flags_string(flags);

    Ok(Ret(v_list(&[v_obj(owner), v_string(perms)])))
}
bf_declare!(property_info, bf_property_info);

enum InfoParseResult {
    Fail(moor_var::Error),
    Success(PropAttrs),
}

fn info_to_prop_attrs(info: &List) -> InfoParseResult {
    if info.len() < 2 || info.len() > 3 {
        return InfoParseResult::Fail(E_ARGS.msg("Invalid property info length"));
    }

    let owner = info.index(0).unwrap();
    let Variant::Obj(owner) = owner.variant() else {
        return InfoParseResult::Fail(E_TYPE.msg("Invalid property info owner"));
    };
    let perms = info.index(1).unwrap();
    let Variant::Str(perms) = perms.variant() else {
        return InfoParseResult::Fail(E_TYPE.msg("Invalid property info perms"));
    };
    let name = if info.len() == 3 {
        let name = info.index(2).unwrap();
        let Variant::Str(name) = name.variant() else {
            return InfoParseResult::Fail(E_TYPE.msg("Invalid property info name"));
        };
        Some(name.as_str().to_string())
    } else {
        None
    };

    let mut flags = BitEnum::new();
    for c in perms.as_str().chars() {
        match c {
            'r' => flags |= PropFlag::Read,
            'w' => flags |= PropFlag::Write,
            'c' => flags |= PropFlag::Chown,
            _ => return InfoParseResult::Fail(E_INVARG.msg("Invalid property info perms")),
        }
    }

    InfoParseResult::Success(PropAttrs {
        name,
        value: None,
        location: None,
        owner: Some(owner.clone()),
        flags: Some(flags),
    })
}

fn bf_set_property_info(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 3 {
        return Err(ErrValue(
            E_ARGS.msg("set_property_info requires 3 arguments"),
        ));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Err(ErrValue(E_TYPE.msg("set_property_info requires an object")));
    };
    let prop_name = bf_args.args[1].as_symbol().map_err(ErrValue)?;
    let Variant::List(info) = bf_args.args[2].variant() else {
        return Err(ErrValue(E_TYPE.msg("set_property_info requires a list")));
    };

    let attrs = match info_to_prop_attrs(info) {
        InfoParseResult::Fail(e) => {
            return Err(ErrValue(e));
        }
        InfoParseResult::Success(a) => a,
    };

    bf_args
        .world_state
        .set_property_info(&bf_args.task_perms_who(), obj, prop_name, attrs)
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
    let prop_name = bf_args.args[1].as_symbol().map_err(ErrValue)?;
    let is_clear = bf_args
        .world_state
        .is_property_clear(&bf_args.task_perms_who(), obj, prop_name)
        .map_err(world_state_bf_err)?;
    Ok(Ret(bf_args.v_bool(is_clear)))
}
bf_declare!(is_clear_property, bf_is_clear_property);

fn bf_clear_property(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(Code(E_ARGS));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Err(Code(E_TYPE));
    };
    let prop_name = bf_args.args[1].as_symbol().map_err(ErrValue)?;
    bf_args
        .world_state
        .clear_property(&bf_args.task_perms_who(), obj, prop_name)
        .map_err(world_state_bf_err)?;
    Ok(Ret(v_empty_list()))
}
bf_declare!(set_clear_property, bf_clear_property);

// add_property (obj <object>, str <prop-name>, <value>, list <info>) => none
fn bf_add_property(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 4 {
        return Err(Code(E_ARGS));
    }

    let (Variant::Obj(location), name, value, Variant::List(info)) = (
        bf_args.args[0].variant(),
        bf_args.args[1].clone(),
        bf_args.args[2].clone(),
        bf_args.args[3].variant(),
    ) else {
        return Err(Code(E_ARGS));
    };

    let prop_name = name.as_symbol().map_err(ErrValue)?;

    let attrs = match info_to_prop_attrs(info) {
        InfoParseResult::Fail(e) => {
            return Err(ErrValue(e));
        }
        InfoParseResult::Success(a) => a,
    };

    bf_args
        .world_state
        .define_property(
            &bf_args.task_perms_who(),
            location,
            location,
            prop_name,
            &attrs.owner.unwrap(),
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
    let prop_name = bf_args.args[1].as_symbol().map_err(ErrValue)?;
    bf_args
        .world_state
        .delete_property(&bf_args.task_perms_who(), obj, prop_name)
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
