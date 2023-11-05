pub mod web_host;
mod ws_connection;

use moor_values::var::variant::Variant;
use moor_values::var::Var;
use serde_derive::{Deserialize, Serialize};
use serde_json::{json, Number};

pub use web_host::WebHost;
pub use web_host::{
    connect_auth_handler, create_auth_handler, eval_handler, welcome_message_handler,
    ws_connect_attach_handler, ws_create_attach_handler,
};

#[derive(Serialize, Deserialize)]
struct OID {
    oid: i64,
}

#[derive(Serialize, Deserialize)]
struct Error {
    error_code: u8,
    error_name: String,
    error_msg: String,
}

pub fn var_as_json(v: &Var) -> serde_json::Value {
    match v.variant() {
        Variant::None => serde_json::Value::Null,
        Variant::Str(s) => serde_json::Value::String(s.to_string()),
        Variant::Obj(o) => json!(OID { oid: o.0 }),
        Variant::Int(i) => serde_json::Value::Number(Number::from(*i)),
        Variant::Float(f) => json!(*f),
        Variant::Err(e) => json!(Error {
            error_code: (*e) as u8,
            error_name: e.name().to_string(),
            error_msg: e.message().to_string(),
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
