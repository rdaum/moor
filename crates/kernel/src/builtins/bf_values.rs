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

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use async_trait::async_trait;

use moor_values::var::error::Error;
use moor_values::var::error::Error::{E_INVARG, E_TYPE};
use moor_values::var::variant::Variant;
use moor_values::var::{v_bool, v_float, v_int, v_obj, v_str};
use moor_values::AsByteBuffer;

use crate::bf_declare;
use crate::builtins::BfRet::Ret;
use crate::builtins::{BfCallState, BfRet, BuiltinFunction};
use crate::vm::VM;
use moor_compiler::offset_for_builtin;

async fn bf_typeof<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    let arg = &bf_args.args[0];
    Ok(Ret(v_int(arg.type_id() as i64)))
}
bf_declare!(typeof, bf_typeof);

async fn bf_tostr<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    let mut result = String::new();
    for arg in &bf_args.args {
        match arg.variant() {
            Variant::None => result.push_str("None"),
            Variant::Int(i) => result.push_str(&i.to_string()),
            Variant::Float(f) => result.push_str(format!("{:?}", f).as_str()),
            Variant::Str(s) => result.push_str(s.as_str()),
            Variant::Obj(o) => result.push_str(&o.to_string()),
            Variant::List(_) => result.push_str("{list}"),
            Variant::Err(e) => result.push_str(e.name()),
        }
    }
    Ok(Ret(v_str(result.as_str())))
}
bf_declare!(tostr, bf_tostr);

async fn bf_toliteral<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    if bf_args.args.len() != 1 {
        return Err(E_INVARG);
    }
    let literal = bf_args.args[0].to_literal();
    Ok(Ret(v_str(literal.as_str())))
}
bf_declare!(toliteral, bf_toliteral);

async fn bf_toint<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    if bf_args.args.len() != 1 {
        return Err(E_INVARG);
    }
    match bf_args.args[0].variant() {
        Variant::Int(i) => Ok(Ret(v_int(*i))),
        Variant::Float(f) => Ok(Ret(v_int(*f as i64))),
        Variant::Obj(o) => Ok(Ret(v_int(o.0))),
        Variant::Str(s) => {
            let i = s.as_str().parse::<f64>();
            match i {
                Ok(i) => Ok(Ret(v_int(i as i64))),
                Err(_) => Ok(Ret(v_int(0))),
            }
        }
        Variant::Err(e) => Ok(Ret(v_int(*e as i64))),
        _ => Err(E_INVARG),
    }
}
bf_declare!(toint, bf_toint);

async fn bf_toobj<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    if bf_args.args.len() != 1 {
        return Err(E_INVARG);
    }
    match bf_args.args[0].variant() {
        Variant::Int(i) => Ok(Ret(v_obj(*i))),
        Variant::Float(f) => Ok(Ret(v_obj(*f as i64))),
        Variant::Str(s) if s.as_str().starts_with('#') => {
            let i = s.as_str()[1..].parse::<i64>();
            match i {
                Ok(i) => Ok(Ret(v_obj(i))),
                Err(_) => Ok(Ret(v_obj(0))),
            }
        }
        Variant::Str(s) => {
            let i = s.as_str().parse::<i64>();
            match i {
                Ok(i) => Ok(Ret(v_obj(i))),
                Err(_) => Ok(Ret(v_obj(0))),
            }
        }
        _ => Err(E_INVARG),
    }
}
bf_declare!(toobj, bf_toobj);

async fn bf_tofloat<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    if bf_args.args.len() != 1 {
        return Err(E_INVARG);
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
        Variant::Err(e) => Ok(Ret(v_float(*e as u8 as f64))),
        _ => Err(E_INVARG),
    }
}
bf_declare!(tofloat, bf_tofloat);

async fn bf_equal<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    if bf_args.args.len() != 2 {
        return Err(E_INVARG);
    }
    let result = match (bf_args.args[0].variant(), bf_args.args[1].variant()) {
        (Variant::Str(s1), Variant::Str(s2)) => s1.as_str() == s2.as_str().to_lowercase(),
        _ => bf_args.args[0] == bf_args.args[1],
    };
    Ok(Ret(v_bool(result)))
}
bf_declare!(equal, bf_equal);

async fn bf_value_bytes<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    if bf_args.args.len() != 1 {
        return Err(E_INVARG);
    }
    let count = bf_args.args[0].size_bytes();
    Ok(Ret(v_int(count as i64)))
}
bf_declare!(value_bytes, bf_value_bytes);

async fn bf_value_hash<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    if bf_args.args.len() != 1 {
        return Err(E_INVARG);
    }
    let mut s = DefaultHasher::new();
    bf_args.args[0].hash(&mut s);
    Ok(Ret(v_int(s.finish() as i64)))
}
bf_declare!(value_hash, bf_value_hash);

async fn bf_length<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    if bf_args.args.len() != 1 {
        return Err(E_INVARG);
    }

    match bf_args.args[0].variant() {
        Variant::Str(s) => Ok(Ret(v_int(s.len() as i64))),
        Variant::List(l) => Ok(Ret(v_int(l.len() as i64))),
        _ => Err(E_TYPE),
    }
}
bf_declare!(length, bf_length);

async fn bf_object_bytes<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    if bf_args.args.len() != 1 {
        return Err(E_INVARG);
    }
    let Variant::Obj(o) = bf_args.args[0].variant() else {
        return Err(E_INVARG);
    };
    if !bf_args.world_state.valid(*o).await? {
        return Err(E_INVARG);
    };
    let size = bf_args
        .world_state
        .object_bytes(bf_args.caller_perms(), *o)
        .await?;
    Ok(Ret(v_int(size as i64)))
}
bf_declare!(object_bytes, bf_object_bytes);

impl VM {
    pub(crate) fn register_bf_values(&mut self) {
        self.builtins[offset_for_builtin("typeof")] = Arc::new(BfTypeof {});
        self.builtins[offset_for_builtin("tostr")] = Arc::new(BfTostr {});
        self.builtins[offset_for_builtin("toliteral")] = Arc::new(BfToliteral {});
        self.builtins[offset_for_builtin("toint")] = Arc::new(BfToint {});
        self.builtins[offset_for_builtin("tonum")] = Arc::new(BfToint {});
        self.builtins[offset_for_builtin("tonum")] = Arc::new(BfToint {});
        self.builtins[offset_for_builtin("toobj")] = Arc::new(BfToobj {});
        self.builtins[offset_for_builtin("tofloat")] = Arc::new(BfTofloat {});
        self.builtins[offset_for_builtin("equal")] = Arc::new(BfEqual {});
        self.builtins[offset_for_builtin("value_bytes")] = Arc::new(BfValueBytes {});
        self.builtins[offset_for_builtin("object_bytes")] = Arc::new(BfObjectBytes {});
        self.builtins[offset_for_builtin("value_hash")] = Arc::new(BfValueHash {});
        self.builtins[offset_for_builtin("length")] = Arc::new(BfLength {});
    }
}
