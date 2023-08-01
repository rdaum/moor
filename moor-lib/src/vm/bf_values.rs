use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use async_trait::async_trait;

use crate::bf_declare;
use crate::compiler::builtins::offset_for_builtin;
use crate::values::error::Error::{E_INVARG, E_TYPE};
use crate::values::var::{v_bool, v_err, v_float, v_int, v_obj, v_str, Var};
use crate::values::variant::Variant;
use crate::vm::vm::BfCallState;
use crate::vm::vm::{BuiltinFunction, VM};

async fn bf_typeof<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    let arg = &bf_args.args[0];
    Ok(v_int(arg.type_id() as i64))
}
bf_declare!(typeof, bf_typeof);

async fn bf_tostr<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    let mut result = String::new();
    for arg in &bf_args.args {
        match arg.variant() {
            Variant::None => result.push_str("None"),
            Variant::Int(i) => result.push_str(&i.to_string()),
            Variant::Float(f) => result.push_str(&f.to_string()),
            Variant::Str(s) => result.push_str(s.as_str()),
            Variant::Obj(o) => result.push_str(&o.to_string()),
            Variant::List(_) => result.push_str("{list}"),
            Variant::Err(e) => result.push_str(e.name()),
            _ => {}
        }
    }
    Ok(v_str(result.as_str()))
}
bf_declare!(tostr, bf_tostr);

async fn bf_toliteral<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }
    let literal = bf_args.args[0].to_literal();
    Ok(v_str(literal.as_str()))
}
bf_declare!(toliteral, bf_toliteral);

async fn bf_toint<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }
    match bf_args.args[0].variant() {
        Variant::Int(i) => Ok(v_int(*i)),
        Variant::Float(f) => Ok(v_int(*f as i64)),
        Variant::Str(s) => {
            let i = s.as_str().parse::<i64>();
            match i {
                Ok(i) => Ok(v_int(i)),
                Err(_) => Ok(v_int(0)),
            }
        }
        Variant::Err(e) => Ok(v_int(*e as i64)),
        _ => Ok(v_err(E_INVARG)),
    }
}
bf_declare!(toint, bf_toint);

async fn bf_toobj<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }
    match bf_args.args[0].variant() {
        Variant::Int(i) => Ok(v_obj(*i)),
        Variant::Float(f) => Ok(v_obj(*f as i64)),
        Variant::Str(s) if s.as_str().starts_with('#') => {
            let i = s.as_str()[1..].parse::<i64>();
            match i {
                Ok(i) => Ok(v_obj(i)),
                Err(_) => Ok(v_obj(0)),
            }
        }
        Variant::Str(s) => {
            let i = s.as_str().parse::<i64>();
            match i {
                Ok(i) => Ok(v_obj(i)),
                Err(_) => Ok(v_obj(0)),
            }
        }
        _ => Ok(v_err(E_INVARG)),
    }
}
bf_declare!(toobj, bf_toobj);

async fn bf_tofloat<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }
    match bf_args.args[0].variant() {
        Variant::Int(i) => Ok(v_float(*i as f64)),
        Variant::Float(f) => Ok(v_float(*f)),
        Variant::Str(s) => {
            let f = s.as_str().parse::<f64>();
            match f {
                Ok(f) => Ok(v_float(f)),
                Err(_) => Ok(v_float(0.0)),
            }
        }
        Variant::Err(e) => Ok(v_float(*e as u8 as f64)),
        _ => Ok(v_err(E_INVARG)),
    }
}
bf_declare!(tofloat, bf_tofloat);

async fn bf_equal<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 2 {
        return Ok(v_err(E_INVARG));
    }
    let result = match (bf_args.args[0].variant(), bf_args.args[1].variant()) {
        (Variant::Str(s1), Variant::Str(s2)) => {
            s1.as_str().to_lowercase() == s2.as_str().to_lowercase()
        }
        _ => bf_args.args[0] == bf_args.args[1],
    };
    Ok(v_bool(result))
}
bf_declare!(equal, bf_equal);

async fn bf_value_bytes<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }
    unimplemented!("value_bytes");
}
bf_declare!(value_bytes, bf_value_bytes);

async fn bf_value_hash<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }
    let mut s = DefaultHasher::new();
    bf_args.args[0].hash(&mut s);
    Ok(v_int(s.finish() as i64))
}
bf_declare!(value_hash, bf_value_hash);

async fn bf_length<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }

    match bf_args.args[0].variant() {
        Variant::Str(s) => Ok(v_int(s.len() as i64)),
        Variant::List(l) => Ok(v_int(l.len() as i64)),
        _ => Ok(v_err(E_TYPE)),
    }
}
bf_declare!(length, bf_length);

impl VM {
    pub(crate) fn register_bf_values(&mut self) -> Result<(), anyhow::Error> {
        self.builtins[offset_for_builtin("typeof")] = Arc::new(Box::new(BfTypeof {}));
        self.builtins[offset_for_builtin("tostr")] = Arc::new(Box::new(BfTostr {}));
        self.builtins[offset_for_builtin("toliteral")] = Arc::new(Box::new(BfToliteral {}));
        self.builtins[offset_for_builtin("toint")] = Arc::new(Box::new(BfToint {}));
        self.builtins[offset_for_builtin("tonum")] = Arc::new(Box::new(BfToint {}));
        self.builtins[offset_for_builtin("toobj")] = Arc::new(Box::new(BfToobj {}));
        self.builtins[offset_for_builtin("tofloat")] = Arc::new(Box::new(BfTofloat {}));
        self.builtins[offset_for_builtin("equal")] = Arc::new(Box::new(BfEqual {}));
        self.builtins[offset_for_builtin("value_bytes")] = Arc::new(Box::new(BfValueBytes {}));
        self.builtins[offset_for_builtin("value_hash")] = Arc::new(Box::new(BfValueHash {}));

        self.builtins[offset_for_builtin("length")] = Arc::new(Box::new(BfLength {}));
        Ok(())
    }
}
