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
use crate::builtins::BfRet::Ret;
use crate::builtins::{BfCallState, BfErr, BfRet, BuiltinFunction, world_state_bf_err};
use md5::Digest;
use moor_compiler::{offset_for_builtin, to_literal};
use moor_var::Variant;
use moor_var::{AsByteBuffer, Sequence};
use moor_var::{E_ARGS, E_INVARG, E_RANGE, E_TYPE};
use moor_var::{v_float, v_int, v_obj, v_objid, v_str, v_sym, v_sym_str};

fn bf_typeof(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    let arg = &bf_args.args[0];
    Ok(Ret(v_int(arg.type_code() as i64)))
}
bf_declare!(typeof, bf_typeof);

fn bf_tostr(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    let mut result = String::new();
    for arg in bf_args.args.iter() {
        match arg.variant() {
            Variant::None => result.push_str("None"),
            Variant::Bool(b) => result.push_str(format!("{}", b).as_str()),
            Variant::Int(i) => result.push_str(&i.to_string()),
            Variant::Float(f) => result.push_str(format!("{:?}", f).as_str()),
            Variant::Str(s) => result.push_str(s.as_str()),
            Variant::Obj(o) => result.push_str(&o.to_string()),
            Variant::List(_) => result.push_str("{list}"),
            Variant::Map(_) => result.push_str("[map]"),
            Variant::Sym(s) => result.push_str(&s.to_string()),
            Variant::Err(e) => result.push_str(e.name().as_str()),
            Variant::Flyweight(fl) => {
                if fl.is_sealed() {
                    result.push_str("<sealed flyweight>")
                } else {
                    result.push_str("<flyweight>")
                }
            }
        }
    }
    Ok(Ret(v_str(result.as_str())))
}
bf_declare!(tostr, bf_tostr);

fn bf_tosym(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // Convert scalar values to symbols.
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    match bf_args.args[0].variant() {
        Variant::Bool(b) => {
            let s = format!("{b}");
            Ok(Ret(v_sym_str(&s)))
        }
        Variant::Str(s) => Ok(Ret(v_sym_str(s.as_str()))),
        Variant::Err(e) => Ok(Ret(v_sym(e.name()))),
        Variant::Sym(s) => Ok(Ret(v_sym(*s))),
        _ => Err(BfErr::Code(E_TYPE)),
    }
}
bf_declare!(tosym, bf_tosym);

fn bf_toliteral(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }
    let literal = to_literal(&bf_args.args[0]);
    Ok(Ret(v_str(literal.as_str())))
}
bf_declare!(toliteral, bf_toliteral);

fn bf_toint(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
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
                return Err(BfErr::Code(E_INVARG));
            };

            Ok(Ret(v_int(v as i64)))
        }
        _ => Err(BfErr::Code(E_INVARG)),
    }
}
bf_declare!(toint, bf_toint);

fn bf_toobj(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }
    match bf_args.args[0].variant() {
        Variant::Int(i) => {
            let i = if *i < i32::MIN as i64 || *i > i32::MAX as i64 {
                return Err(BfErr::Code(E_RANGE));
            } else {
                *i as i32
            };
            Ok(Ret(v_objid(i)))
        }
        Variant::Float(f) => {
            let f = if *f < i32::MIN as f64 || *f > i32::MAX as f64 {
                return Err(BfErr::Code(E_RANGE));
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
        Variant::Obj(o) => Ok(Ret(v_obj(o.clone()))),
        _ => Err(BfErr::Code(E_INVARG)),
    }
}
bf_declare!(toobj, bf_toobj);

fn bf_tofloat(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
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
                return Err(BfErr::Code(E_INVARG));
            };

            Ok(Ret(v_float(v as f64)))
        }
        _ => Err(BfErr::Code(E_INVARG)),
    }
}
bf_declare!(tofloat, bf_tofloat);

fn bf_equal(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::Code(E_ARGS));
    }
    let (a1, a2) = (&bf_args.args[0], &bf_args.args[1]);
    let result = a1.eq_case_sensitive(a2);
    Ok(Ret(bf_args.v_bool(result)))
}
bf_declare!(equal, bf_equal);

fn bf_value_bytes(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }
    let count = bf_args.args[0].size_bytes();
    Ok(Ret(v_int(count as i64)))
}
bf_declare!(value_bytes, bf_value_bytes);

fn bf_value_hash(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }
    let s = to_literal(&bf_args.args[0]);
    let hash_digest = md5::Md5::digest(s.as_bytes());
    Ok(Ret(v_str(
        format!("{:x}", hash_digest).to_uppercase().as_str(),
    )))
}
bf_declare!(value_hash, bf_value_hash);

fn bf_length(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    match bf_args.args[0].len() {
        Ok(l) => Ok(Ret(v_int(l as i64))),
        Err(e) => Err(BfErr::ErrValue(e)),
    }
}
bf_declare!(length, bf_length);

fn bf_object_bytes(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Variant::Obj(o) = bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_INVARG));
    };
    if !bf_args.world_state.valid(o).map_err(world_state_bf_err)? {
        return Err(BfErr::Code(E_INVARG));
    };
    let size = bf_args
        .world_state
        .object_bytes(&bf_args.caller_perms(), o)
        .map_err(world_state_bf_err)?;
    Ok(Ret(v_int(size as i64)))
}
bf_declare!(object_bytes, bf_object_bytes);

pub(crate) fn register_bf_values(builtins: &mut [Box<dyn BuiltinFunction>]) {
    builtins[offset_for_builtin("typeof")] = Box::new(BfTypeof {});
    builtins[offset_for_builtin("tostr")] = Box::new(BfTostr {});
    builtins[offset_for_builtin("tosym")] = Box::new(BfTosym {});
    builtins[offset_for_builtin("toliteral")] = Box::new(BfToliteral {});
    builtins[offset_for_builtin("toint")] = Box::new(BfToint {});
    builtins[offset_for_builtin("tonum")] = Box::new(BfToint {});
    builtins[offset_for_builtin("toobj")] = Box::new(BfToobj {});
    builtins[offset_for_builtin("tofloat")] = Box::new(BfTofloat {});
    builtins[offset_for_builtin("equal")] = Box::new(BfEqual {});
    builtins[offset_for_builtin("value_bytes")] = Box::new(BfValueBytes {});
    builtins[offset_for_builtin("object_bytes")] = Box::new(BfObjectBytes {});
    builtins[offset_for_builtin("value_hash")] = Box::new(BfValueHash {});
    builtins[offset_for_builtin("length")] = Box::new(BfLength {});
}
