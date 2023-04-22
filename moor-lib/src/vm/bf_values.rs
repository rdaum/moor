use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::bf_declare;
use crate::compiler::builtins::offset_for_builtin;
use crate::db::state::WorldState;
use crate::model::var::{v_err, v_int, v_obj, Objid, Var, v_float, v_str, v_bool};
use crate::model::var::Error::{E_INVARG, E_TYPE};
use crate::tasks::Sessions;
use crate::vm::activation::Activation;
use crate::vm::execute::{BfFunction, VM};

async fn bf_typeof(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    let arg = args[0].clone();
    Ok(v_int(arg.type_id() as i64))
}
bf_declare!(typeof, bf_typeof);

async fn bf_tostr(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    let mut result = String::new();
    for arg in args {
        match arg {
            Var::None => result.push_str("None"),
            Var::Int(i) => result.push_str(&i.to_string()),
            Var::Float(f) => result.push_str(&f.to_string()),
            Var::Str(s) => result.push_str(&s),
            Var::Obj(o) => result.push_str(&o.to_string()),
            Var::List(_) => result.push_str("{list}"),
            Var::Err(e) => result.push_str(e.name()),
            _ => {}
        }
    }
    Ok(v_str(result.as_str()))
}
bf_declare!(tostr, bf_tostr);

async fn bf_toliteral(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    if args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }
    let literal = args[0].to_literal();
    Ok(v_str(literal.as_str()))
}
bf_declare!(toliteral, bf_toliteral);

async fn bf_toint(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    if args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }
    match &args[0] {
        Var::Int(i) => Ok(v_int(*i)),
        Var::Float(f) => Ok(v_int(*f as i64)),
        Var::Str(s) => {
            let i = s.parse::<i64>();
            match i {
                Ok(i) => Ok(v_int(i)),
                Err(_) => Ok(v_int(0)),
            }
        }
        Var::Err(e) => Ok(v_int(*e as i64)),
        _ => Ok(v_err(E_INVARG)),
    }
}
bf_declare!(toint, bf_toint);

async fn bf_toobj(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    if args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }
    match &args[0] {
        Var::Int(i) => Ok(Var::Obj(Objid(*i))),
        Var::Float(f) => Ok(Var::Obj(Objid(*f as i64))),
        Var::Str(s) if s.starts_with('#') => {
            let i = s[1..].parse::<i64>();
            match i {
                Ok(i) => Ok(v_obj(i)),
                Err(_) => Ok(v_obj(0)),
            }
        }
        Var::Str(s) => {
            let i = s.parse::<i64>();
            match i {
                Ok(i) => Ok(v_obj(i)),
                Err(_) => Ok(v_obj(0)),
            }
        }
        _ => Ok(v_err(E_INVARG)),
    }
}
bf_declare!(toobj, bf_toobj);

async fn bf_tofloat(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    if args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }
    match &args[0] {
        Var::Int(i) => Ok(v_float(*i as f64)),
        Var::Float(f) => Ok(v_float(*f)),
        Var::Str(s) => {
            let f = s.parse::<f64>();
            match f {
                Ok(f) => Ok(v_float(f)),
                Err(_) => Ok(v_float(0.0)),
            }
        }
        Var::Err(e) => Ok(v_float(*e as u8 as f64)),
        _ => Ok(v_err(E_INVARG)),
    }
}
bf_declare!(tofloat, bf_tofloat);

async fn bf_equal(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    if args.len() != 2 {
        return Ok(v_err(E_INVARG));
    }
    let result = match (&args[0], &args[1]) {
        (Var::Str(s1), Var::Str(s2)) => s1.to_lowercase() == s2.to_lowercase(),
        _ => args[0] == args[1],
    };
    Ok(v_bool(result))
}
bf_declare!(equal, bf_equal);

async fn bf_value_bytes(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    if args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }
    unimplemented!("value_bytes");
}
bf_declare!(value_bytes, bf_value_bytes);

async fn bf_value_hash(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    if args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }
    let mut s = DefaultHasher::new();
    args[0].hash(&mut s);
    Ok(v_int(s.finish() as i64))
}
bf_declare!(value_hash, bf_value_hash);

async fn bf_length(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    if args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }

    match &args[0] {
        Var::Str(s) => Ok(v_int(s.len() as i64)),
        Var::List(l) => Ok(v_int(l.len() as i64)),
        _ => Ok(v_err(E_TYPE)),
    }
}
bf_declare!(length, bf_length);

impl VM {
    pub(crate) fn register_bf_values(&mut self) -> Result<(), anyhow::Error> {
        self.bf_funcs[offset_for_builtin("typeof")] = Arc::new(Box::new(BfTypeof {}));
        self.bf_funcs[offset_for_builtin("tostr")] = Arc::new(Box::new(BfTostr {}));
        self.bf_funcs[offset_for_builtin("toliteral")] = Arc::new(Box::new(BfToliteral {}));
        self.bf_funcs[offset_for_builtin("toint")] = Arc::new(Box::new(BfToint {}));
        self.bf_funcs[offset_for_builtin("tonum")] = Arc::new(Box::new(BfToint {}));
        self.bf_funcs[offset_for_builtin("toobj")] = Arc::new(Box::new(BfToobj {}));
        self.bf_funcs[offset_for_builtin("tofloat")] = Arc::new(Box::new(BfTofloat {}));
        self.bf_funcs[offset_for_builtin("equal")] = Arc::new(Box::new(BfEqual {}));
        self.bf_funcs[offset_for_builtin("value_bytes")] = Arc::new(Box::new(BfValueBytes {}));
        self.bf_funcs[offset_for_builtin("value_hash")] = Arc::new(Box::new(BfValueHash {}));

        self.bf_funcs[offset_for_builtin("length")] = Arc::new(Box::new(BfLength {}));
        Ok(())
    }
}
