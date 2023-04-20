use async_trait::async_trait;
use rkyv::ser::serializers::AllocSerializer;
use rkyv::ser::Serializer;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::bf_declare;
use crate::compiler::builtins::offset_for_builtin;
use crate::db::state::WorldState;
use crate::model::var::Error::{E_INVARG, E_TYPE};
use crate::model::var::{Objid, Var};
use crate::server::Sessions;
use crate::vm::activation::Activation;
use crate::vm::execute::{BfFunction, VM};

async fn bf_typeof(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<Mutex<dyn Sessions>>,
    args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    let arg = args[0].clone();
    Ok(Var::Int(arg.type_id() as i64))
}
bf_declare!(typeof, bf_typeof);

async fn bf_tostr(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<Mutex<dyn Sessions>>,
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
    Ok(Var::Str(result))
}
bf_declare!(tostr, bf_tostr);

fn to_literal(arg: &Var) -> String {
    match arg {
        Var::None => "None".to_string(),
        Var::Int(i) => i.to_string(),
        Var::Float(f) => f.to_string(),
        Var::Str(s) => format!("\"{}\"", s),
        Var::Obj(o) => format!("#{}", o),
        Var::List(l) => {
            let mut result = String::new();
            result.push('{');
            for (i, v) in l.iter().enumerate() {
                if i > 0 {
                    result.push(',');
                }
                result.push_str(&to_literal(v));
            }
            result.push('}');
            result
        }
        Var::Err(e) => e.name().to_string(),
        _ => "".to_string(),
    }
}

async fn bf_toliteral(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<Mutex<dyn Sessions>>,
    args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    if args.len() != 1 {
        return Ok(Var::Err(E_INVARG));
    }
    let literal = to_literal(&args[0]);
    Ok(Var::Str(literal))
}
bf_declare!(toliteral, bf_toliteral);

async fn bf_toint(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<Mutex<dyn Sessions>>,
    args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    if args.len() != 1 {
        return Ok(Var::Err(E_INVARG));
    }
    match &args[0] {
        Var::Int(i) => Ok(Var::Int(*i)),
        Var::Float(f) => Ok(Var::Int(*f as i64)),
        Var::Str(s) => {
            let i = s.parse::<i64>();
            match i {
                Ok(i) => Ok(Var::Int(i)),
                Err(_) => Ok(Var::Int(0)),
            }
        }
        Var::Err(e) => Ok(Var::Int(*e as i64)),
        _ => Ok(Var::Err(E_INVARG)),
    }
}
bf_declare!(toint, bf_toint);

async fn bf_toobj(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<Mutex<dyn Sessions>>,
    args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    if args.len() != 1 {
        return Ok(Var::Err(E_INVARG));
    }
    match &args[0] {
        Var::Int(i) => Ok(Var::Obj(Objid(*i))),
        Var::Float(f) => Ok(Var::Obj(Objid(*f as i64))),
        Var::Str(s) if s.starts_with('#') => {
            let i = s[1..].parse::<i64>();
            match i {
                Ok(i) => Ok(Var::Obj(Objid(i))),
                Err(_) => Ok(Var::Obj(Objid(0))),
            }
        }
        Var::Str(s) => {
            let i = s.parse::<i64>();
            match i {
                Ok(i) => Ok(Var::Obj(Objid(i))),
                Err(_) => Ok(Var::Obj(Objid(0))),
            }
        }
        _ => Ok(Var::Err(E_INVARG)),
    }
}
bf_declare!(toobj, bf_toobj);

async fn bf_tofloat(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<Mutex<dyn Sessions>>,
    args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    if args.len() != 1 {
        return Ok(Var::Err(E_INVARG));
    }
    match &args[0] {
        Var::Int(i) => Ok(Var::Float(*i as f64)),
        Var::Float(f) => Ok(Var::Float(*f)),
        Var::Str(s) => {
            let f = s.parse::<f64>();
            match f {
                Ok(f) => Ok(Var::Float(f)),
                Err(_) => Ok(Var::Float(0.0)),
            }
        }
        Var::Err(e) => Ok(Var::Float(*e as u8 as f64)),
        _ => Ok(Var::Err(E_INVARG)),
    }
}
bf_declare!(tofloat, bf_tofloat);

async fn bf_equal(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<Mutex<dyn Sessions>>,
    args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    if args.len() != 2 {
        return Ok(Var::Err(E_INVARG));
    }
    let result = match (&args[0], &args[1]) {
        (Var::Str(s1), Var::Str(s2)) => s1.to_lowercase() == s2.to_lowercase(),
        _ => args[0] == args[1],
    };
    Ok(Var::Int(if result { 1 } else { 0 }))
}
bf_declare!(equal, bf_equal);

async fn bf_value_bytes(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<Mutex<dyn Sessions>>,
    args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    if args.len() != 1 {
        return Ok(Var::Err(E_INVARG));
    }
    let mut serializer = AllocSerializer::<0>::default();
    serializer.serialize_value(&args[0]).unwrap();
    let bytes = serializer.into_serializer().into_inner();
    Ok(Var::Int(bytes.len() as i64))
}
bf_declare!(value_bytes, bf_value_bytes);

async fn bf_value_hash(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<Mutex<dyn Sessions>>,
    args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    if args.len() != 1 {
        return Ok(Var::Err(E_INVARG));
    }
    let mut s = DefaultHasher::new();
    args[0].hash(&mut s);
    Ok(Var::Int(s.finish() as i64))
}
bf_declare!(value_hash, bf_value_hash);

async fn bf_length(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<Mutex<dyn Sessions>>,
    args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    if args.len() != 1 {
        return Ok(Var::Err(E_INVARG));
    }

    match &args[0] {
        Var::Str(s) => Ok(Var::Int(s.len() as i64)),
        Var::List(l) => Ok(Var::Int(l.len() as i64)),
        _ => Ok(Var::Err(E_TYPE)),
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
