use moor_value::var::variant::Variant;
use moor_value::var::{v_float, v_int, v_list, v_none, v_string, Var};
use serde_derive::{Deserialize, Serialize};
use serde_json::{json, Number, Value};

pub mod routes;
pub mod ws_server;
mod ws_sessions;

#[derive(Serialize, Deserialize)]
struct OID(i64);

#[derive(Serialize, Deserialize)]
struct Error {
    code: u8,
    msg: String,
}

pub fn var_as_json(v: &Var) -> Value {
    match v.variant() {
        Variant::None => Value::Null,
        Variant::Str(s) => Value::String(s.to_string()),
        Variant::Obj(o) => json!(OID(o.0)),
        Variant::Int(i) => Value::Number(Number::from(*i)),
        Variant::Float(f) => json!(*f),
        Variant::Err(e) => json!(Error {
            code: (*e) as u8,
            msg: e.message().to_string(),
        }),
        Variant::List(l) => {
            let mut v = Vec::new();
            for e in l.iter() {
                v.push(var_as_json(e));
            }
            Value::Array(v)
        }
    }
}

#[allow(dead_code)]
pub fn json_as_var(v: &Value) -> Result<Var, anyhow::Error> {
    match v {
        Value::Null => Ok(v_none()),
        Value::Bool(b) => Ok(v_int(if *b { 1 } else { 0 })),
        Value::Number(n) => {
            if n.is_f64() {
                Ok(v_float(n.as_f64().unwrap()))
            } else {
                Ok(v_int(n.as_i64().unwrap()))
            }
        }
        Value::String(s) => Ok(v_string(s.clone())),
        Value::Array(a) => {
            let mut l = Vec::new();
            for e in a {
                l.push(json_as_var(e)?);
            }
            Ok(v_list(l))
        }
        Value::Object(_) => Err(anyhow::anyhow!("Object not supported yet")),
    }
}
