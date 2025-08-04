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

//! Builtin functions for property manipulation and introspection.

use moor_common::model::{PropAttrs, PropFlag, prop_flags_string};
use moor_common::util::BitEnum;
use moor_compiler::offset_for_builtin;
use moor_var::Variant;
use moor_var::{E_ARGS, E_INVARG, E_TYPE};
use moor_var::{List, v_empty_list};
use moor_var::{Sequence, Symbol};
use moor_var::{v_list, v_obj, v_string};

use crate::vm::builtins::BfErr::{Code, ErrValue};
use crate::vm::builtins::BfRet::{Ret, RetNil};
use crate::vm::builtins::{BfCallState, BfErr, BfRet, BuiltinFunction, world_state_bf_err};

/// MOO: `list property_info(obj object, symbol prop_name)`
/// Returns property information as `{owner, perms}`.
fn bf_property_info(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(Code(E_ARGS));
    }
    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(Code(E_TYPE));
    };
    let prop_name = bf_args.args[1].as_symbol().map_err(ErrValue)?;
    let (_, perms) = bf_args
        .world_state
        .get_property_info(&bf_args.task_perms_who(), &obj, prop_name)
        .map_err(world_state_bf_err)?;
    let owner = perms.owner();
    let flags = perms.flags();

    // Turn perm flags into string: r w c
    let perms = prop_flags_string(flags);

    Ok(Ret(v_list(&[v_obj(owner), v_string(perms)])))
}

enum InfoParseResult {
    Fail(moor_var::Error),
    Success(PropAttrs),
}

fn info_to_prop_attrs(info: &List) -> InfoParseResult {
    if info.len() < 2 || info.len() > 3 {
        return InfoParseResult::Fail(E_ARGS.msg("Invalid property info length"));
    }

    let owner = info.index(0).unwrap();
    let Some(owner) = owner.as_object() else {
        return InfoParseResult::Fail(E_TYPE.msg("Invalid property info owner"));
    };
    let perms = info.index(1).unwrap();
    let Some(perms) = perms.as_string() else {
        return InfoParseResult::Fail(E_TYPE.msg("Invalid property info perms"));
    };
    let name = if info.len() == 3 {
        let name = info.index(2).unwrap();
        let Some(name) = name.as_string() else {
            return InfoParseResult::Fail(E_TYPE.msg("Invalid property info name"));
        };
        Some(name.to_string())
    } else {
        None
    };

    let mut flags = BitEnum::new();
    for c in perms.chars() {
        match c {
            'r' => flags |= PropFlag::Read,
            'w' => flags |= PropFlag::Write,
            'c' => flags |= PropFlag::Chown,
            _ => return InfoParseResult::Fail(E_INVARG.msg("Invalid property info perms")),
        }
    }

    InfoParseResult::Success(PropAttrs {
        name: name.as_deref().map(Symbol::mk),
        value: None,
        location: None,
        owner: Some(owner),
        flags: Some(flags),
    })
}

/// MOO: `none set_property_info(obj object, symbol prop_name, list info)`
/// Sets property information from a `{owner, perms}` list.
fn bf_set_property_info(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 3 {
        return Err(ErrValue(
            E_ARGS.msg("set_property_info requires 3 arguments"),
        ));
    }
    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(ErrValue(E_TYPE.msg("set_property_info requires an object")));
    };
    let prop_name = bf_args.args[1].as_symbol().map_err(ErrValue)?;
    let Some(info) = bf_args.args[2].as_list() else {
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
        .set_property_info(&bf_args.task_perms_who(), &obj, prop_name, attrs)
        .map_err(world_state_bf_err)?;
    Ok(Ret(v_empty_list()))
}

/// MOO: `bool is_clear_property(obj object, symbol prop_name)`
/// Returns true if the property is clear (has no local value).
fn bf_is_clear_property(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(Code(E_ARGS));
    }
    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(Code(E_TYPE));
    };
    let prop_name = bf_args.args[1].as_symbol().map_err(ErrValue)?;
    let is_clear = bf_args
        .world_state
        .is_property_clear(&bf_args.task_perms_who(), &obj, prop_name)
        .map_err(world_state_bf_err)?;
    Ok(Ret(bf_args.v_bool(is_clear)))
}

/// MOO: `none clear_property(obj object, symbol prop_name)`
/// Clears the local value of a property, reverting to inherited value.
fn bf_clear_property(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(Code(E_ARGS));
    }
    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(Code(E_TYPE));
    };
    let prop_name = bf_args.args[1].as_symbol().map_err(ErrValue)?;
    bf_args
        .world_state
        .clear_property(&bf_args.task_perms_who(), &obj, prop_name)
        .map_err(world_state_bf_err)?;
    Ok(Ret(v_empty_list()))
}

/// MOO: `none add_property(obj object, symbol prop_name, any value, list info)`
/// Adds a new property with the given value and info.
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
    Ok(RetNil)
}

/// MOO: `none delete_property(obj object, symbol prop_name)`
/// Removes a property from an object.
fn bf_delete_property(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(Code(E_ARGS));
    }
    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(Code(E_TYPE));
    };
    let prop_name = bf_args.args[1].as_symbol().map_err(ErrValue)?;
    bf_args
        .world_state
        .delete_property(&bf_args.task_perms_who(), &obj, prop_name)
        .map_err(world_state_bf_err)?;
    Ok(Ret(v_empty_list()))
}

pub(crate) fn register_bf_properties(builtins: &mut [Box<BuiltinFunction>]) {
    builtins[offset_for_builtin("property_info")] = Box::new(bf_property_info);
    builtins[offset_for_builtin("set_property_info")] = Box::new(bf_set_property_info);
    builtins[offset_for_builtin("is_clear_property")] = Box::new(bf_is_clear_property);
    builtins[offset_for_builtin("clear_property")] = Box::new(bf_clear_property);
    builtins[offset_for_builtin("add_property")] = Box::new(bf_add_property);
    builtins[offset_for_builtin("delete_property")] = Box::new(bf_delete_property);
}
