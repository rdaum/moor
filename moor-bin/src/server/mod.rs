use moor_value::var::variant::Variant;
use moor_value::var::Var;
use serde_derive::{Deserialize, Serialize};
use serde_json::{json, Number};

pub mod routes;
pub mod ws_server;

#[derive(Serialize, Deserialize)]
struct OID(i64);

#[derive(Serialize, Deserialize)]
struct Error {
    code: u8,
    msg: String,
}

pub fn var_as_json(v: &Var) -> serde_json::Value {
    match v.variant() {
        Variant::None => serde_json::Value::Null,
        Variant::Str(s) => serde_json::Value::String(s.to_string()),
        Variant::Obj(o) => json!(OID(o.0)),
        Variant::Int(i) => serde_json::Value::Number(Number::from(*i)),
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
            serde_json::Value::Array(v)
        }
    }
}
