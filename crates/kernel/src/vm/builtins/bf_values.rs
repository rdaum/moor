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

//! Builtin functions for value inspection, conversion, and manipulation.

use crate::vm::builtins::BfRet::Ret;
use crate::vm::builtins::{BfCallState, BfErr, BfRet, BuiltinFunction, world_state_bf_err};
use md5::Digest;
use moor_compiler::{offset_for_builtin, to_literal};
use moor_var::{AsByteBuffer, Sequence};
use moor_var::{E_ARGS, E_INVARG, E_RANGE, E_TYPE};
use moor_var::{Variant, v_err};
use moor_var::{v_float, v_int, v_obj, v_objid, v_str, v_sym};

/// MOO: `int typeof(any value)`
/// Returns the type code of the given value.
fn bf_typeof(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    let arg = &bf_args.args[0];
    Ok(Ret(v_int(arg.type_code() as i64)))
}

/// MOO: `str tostr(any ...)`
/// Converts arguments to their string representation and concatenates them.
fn bf_tostr(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    let mut result = String::new();
    for arg in bf_args.args.iter() {
        match arg.variant() {
            Variant::None => result.push_str("None"),
            Variant::Bool(b) => result.push_str(format!("{b}").as_str()),
            Variant::Int(i) => result.push_str(&i.to_string()),
            Variant::Float(f) => result.push_str(format!("{f:?}").as_str()),
            Variant::Str(s) => result.push_str(s.as_str()),
            Variant::Binary(b) => result.push_str(&format!("<binary {} bytes>", b.len())),
            Variant::Obj(o) => result.push_str(&o.to_string()),
            Variant::List(_) => result.push_str("{list}"),
            Variant::Map(_) => result.push_str("[map]"),
            Variant::Sym(s) => result.push_str(&s.to_string()),
            Variant::Err(e) => result.push_str(&e.name().as_arc_string()),
            Variant::Flyweight(_) => result.push_str("<flyweight>"),
            Variant::Lambda(l) => {
                use moor_var::program::opcode::ScatterLabel;
                let param_str =
                    l.0.params
                        .labels
                        .iter()
                        .map(|label| match label {
                            ScatterLabel::Required(_) => "x".to_string(),
                            ScatterLabel::Optional(_, _) => "?x".to_string(),
                            ScatterLabel::Rest(_) => "@x".to_string(),
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                result.push_str(&format!("<lambda:({param_str})>"));
            }
        }
    }
    Ok(Ret(v_str(result.as_str())))
}

/// MOO: `symbol tosym(str|bool|error|symbol value)`
/// Converts scalar values to symbols.
fn bf_tosym(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // Convert scalar values to symbols.
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("tosym() requires exactly 1 argument"),
        ));
    }

    match bf_args.args[0].variant() {
        Variant::Bool(b) => {
            let s = format!("{b}");
            Ok(Ret(v_sym(s.as_str())))
        }
        Variant::Str(s) => Ok(Ret(v_sym(s.as_str()))),
        Variant::Err(e) => Ok(Ret(v_sym(e.name()))),
        Variant::Sym(s) => Ok(Ret(v_sym(*s))),
        _ => Err(BfErr::ErrValue(E_TYPE.msg(
            "tosym() requires a string, boolean, error, or symbol argument",
        ))),
    }
}

/// MOO: `str toliteral(any value)`
/// Converts a value to its literal representation (cannot convert closures with captured variables).
fn bf_toliteral(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("toliteral() requires exactly 1 argument"),
        ));
    }

    // Check if this is a lambda with captures - if so, raise an error
    if let Some(lambda) = bf_args.args[0].as_lambda()
        && !lambda.0.captured_env.is_empty() {
            return Err(BfErr::ErrValue(
                E_INVARG.msg("Cannot convert closure with captured variables to literal"),
            ));
        }

    let literal = to_literal(&bf_args.args[0]);
    Ok(Ret(v_str(literal.as_str())))
}

/// MOO: `int toint(int|float|obj|str|error value)`
/// Converts a value to an integer.
fn bf_toint(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("toint() requires exactly 1 argument"),
        ));
    }
    match bf_args.args[0].variant() {
        Variant::Int(i) => Ok(Ret(v_int(*i))),
        Variant::Float(f) => Ok(Ret(v_int(*f as i64))),
        Variant::Obj(o) => Ok(Ret(v_int(o.id().0 as i64))),
        Variant::Str(s) => {
            let i = s.as_str().parse::<f64>();
            match i {
                Ok(i) => Ok(Ret(v_int(i as i64))),
                Err(_) => Ok(Ret(v_int(0))),
            }
        }
        Variant::Err(e) => {
            let Some(v) = e.to_int() else {
                return Err(BfErr::ErrValue(
                    E_INVARG.msg("cannot convert this error to integer"),
                ));
            };

            Ok(Ret(v_int(v as i64)))
        }
        _ => Err(BfErr::ErrValue(
            E_INVARG.msg("cannot convert this type to integer"),
        )),
    }
}

/// MOO: `obj toobj(int|float|str|obj value)`
/// Converts a value to an object reference.
fn bf_toobj(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("toobj() requires exactly 1 argument"),
        ));
    }
    match bf_args.args[0].variant() {
        Variant::Int(i) => {
            let i = if *i < i32::MIN as i64 || *i > i32::MAX as i64 {
                return Err(BfErr::ErrValue(
                    E_RANGE.msg("integer value outside valid object ID range"),
                ));
            } else {
                *i as i32
            };
            Ok(Ret(v_objid(i)))
        }
        Variant::Float(f) => {
            let f = if *f < i32::MIN as f64 || *f > i32::MAX as f64 {
                return Err(BfErr::ErrValue(
                    E_RANGE.msg("float value outside valid object ID range"),
                ));
            } else {
                *f as i32
            };
            Ok(Ret(v_objid(f)))
        }
        Variant::Str(s) if s.as_str().starts_with('#') => {
            let i = s.as_str()[1..].parse::<i32>();
            match i {
                Ok(i) => Ok(Ret(v_objid(i))),
                Err(_) => Ok(Ret(v_objid(0))),
            }
        }
        Variant::Str(s) => {
            let i = s.as_str().parse::<i32>();
            match i {
                Ok(i) => Ok(Ret(v_objid(i))),
                Err(_) => Ok(Ret(v_objid(0))),
            }
        }
        Variant::Obj(o) => Ok(Ret(v_obj(*o))),
        _ => Err(BfErr::ErrValue(
            E_INVARG.msg("cannot convert this type to object"),
        )),
    }
}

/// MOO: `float tofloat(int|float|str|error value)`
/// Converts a value to a floating-point number.
fn bf_tofloat(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("tofloat() requires exactly 1 argument"),
        ));
    }
    match bf_args.args[0].variant() {
        Variant::Int(i) => Ok(Ret(v_float(*i as f64))),
        Variant::Float(f) => Ok(Ret(v_float(*f))),
        Variant::Str(s) => {
            let f = s.as_str().parse::<f64>();
            match f {
                Ok(f) => Ok(Ret(v_float(f))),
                Err(_) => Ok(Ret(v_float(0.0))),
            }
        }

        Variant::Err(e) => {
            let Some(v) = e.to_int() else {
                return Err(BfErr::ErrValue(
                    E_INVARG.msg("cannot convert this error to float"),
                ));
            };

            Ok(Ret(v_float(v as f64)))
        }
        _ => Err(BfErr::ErrValue(
            E_INVARG.msg("cannot convert this type to float"),
        )),
    }
}

/// MOO: `bool equal(any a, any b)`
/// Returns true if the two values are equal (case-sensitive comparison).
fn bf_equal(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("equal() requires exactly 2 arguments"),
        ));
    }
    let (a1, a2) = (&bf_args.args[0], &bf_args.args[1]);
    let result = a1.eq_case_sensitive(a2);
    Ok(Ret(bf_args.v_bool(result)))
}

/// MOO: `int value_bytes(any value)`
/// Returns the number of bytes used to store the value in memory.
fn bf_value_bytes(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("value_bytes() requires exactly 1 argument"),
        ));
    }
    let count = bf_args.args[0].size_bytes();
    Ok(Ret(v_int(count as i64)))
}

/// MOO: `str value_hash(any value)`
/// Returns an MD5 hash of the value's literal representation.
fn bf_value_hash(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("value_hash() requires exactly 1 argument"),
        ));
    }
    let s = to_literal(&bf_args.args[0]);
    let hash_digest = md5::Md5::digest(s.as_bytes());
    Ok(Ret(v_str(
        format!("{hash_digest:x}").to_uppercase().as_str(),
    )))
}

/// MOO: `int length(list|map|str value)`
/// Returns the length of a list, map, or string.
fn bf_length(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("length() requires exactly 1 argument"),
        ));
    }

    match bf_args.args[0].len() {
        Ok(l) => Ok(Ret(v_int(l as i64))),
        Err(e) => Err(BfErr::ErrValue(e)),
    }
}

/// MOO: `int object_bytes(obj o)`
/// Returns the number of bytes used to store the object in the database.
fn bf_object_bytes(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("object_bytes() requires exactly 1 argument"),
        ));
    }
    let Some(o) = bf_args.args[0].as_object() else {
        return Err(BfErr::ErrValue(
            E_INVARG.msg("object_bytes() requires an object argument"),
        ));
    };
    if !bf_args.world_state.valid(&o).map_err(world_state_bf_err)? {
        return Err(BfErr::ErrValue(E_INVARG.msg("object is not valid")));
    };
    let size = bf_args
        .world_state
        .object_bytes(&bf_args.caller_perms(), &o)
        .map_err(world_state_bf_err)?;
    Ok(Ret(v_int(size as i64)))
}

/// MOO: `error error_code(error e)`
/// Returns the error code from an error object (strips off message and value).
fn bf_error_code(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("error_code() takes one argument"),
        ));
    }
    let Some(e) = bf_args.args[0].as_error() else {
        return Err(BfErr::ErrValue(
            E_INVARG.msg("error_code() takes an error object"),
        ));
    };
    let code = e.err_type;
    Ok(Ret(v_err(code)))
}

/// MOO: `str error_message(error e)`
/// Returns the message from an error object.
fn bf_error_message(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("error_message() takes one argument"),
        ));
    }
    let Some(e) = bf_args.args[0].as_error() else {
        return Err(BfErr::ErrValue(
            E_INVARG.msg("error_message() takes an error object"),
        ));
    };
    let msg = e.message();
    Ok(Ret(v_str(msg.as_str())))
}

pub(crate) fn register_bf_values(builtins: &mut [Box<BuiltinFunction>]) {
    builtins[offset_for_builtin("typeof")] = Box::new(bf_typeof);
    builtins[offset_for_builtin("tostr")] = Box::new(bf_tostr);
    builtins[offset_for_builtin("tosym")] = Box::new(bf_tosym);
    builtins[offset_for_builtin("toliteral")] = Box::new(bf_toliteral);
    builtins[offset_for_builtin("toint")] = Box::new(bf_toint);
    builtins[offset_for_builtin("tonum")] = Box::new(bf_toint);
    builtins[offset_for_builtin("toobj")] = Box::new(bf_toobj);
    builtins[offset_for_builtin("tofloat")] = Box::new(bf_tofloat);
    builtins[offset_for_builtin("equal")] = Box::new(bf_equal);
    builtins[offset_for_builtin("value_bytes")] = Box::new(bf_value_bytes);
    builtins[offset_for_builtin("object_bytes")] = Box::new(bf_object_bytes);
    builtins[offset_for_builtin("value_hash")] = Box::new(bf_value_hash);
    builtins[offset_for_builtin("length")] = Box::new(bf_length);
    builtins[offset_for_builtin("error_code")] = Box::new(bf_error_code);
    builtins[offset_for_builtin("error_message")] = Box::new(bf_error_message);
}
