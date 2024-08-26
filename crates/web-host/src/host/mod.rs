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

pub mod web_host;
mod ws_connection;

use moor_values::{v_float, v_int, v_list, v_none, v_str, Var, Variant};
use serde::Serialize;
use serde_derive::Deserialize;
use serde_json::{json, Number};

pub use web_host::WebHost;
pub use web_host::{
    connect_auth_handler, create_auth_handler, eval_handler, properties_handler,
    property_retrieval_handler, verb_program_handler, verb_retrieval_handler, verbs_handler,
    welcome_message_handler, ws_connect_attach_handler, ws_create_attach_handler,
};

#[derive(serde_derive::Serialize, Deserialize)]
struct Oid {
    oid: i64,
}

#[derive(serde_derive::Serialize, Deserialize)]
struct Error {
    error_code: u8,
    error_name: String,
    error_msg: String,
}

pub fn var_as_json(v: &Var) -> serde_json::Value {
    match v.variant() {
        Variant::None => serde_json::Value::Null,
        Variant::Str(s) => serde_json::Value::String(s.to_string()),
        Variant::Obj(o) => json!(Oid { oid: o.0 }),
        Variant::Int(i) => serde_json::Value::Number(Number::from(i)),
        Variant::Float(f) => json!(f),
        Variant::Err(e) => json!(Error {
            error_code: e as u8,
            error_name: e.name().to_string(),
            error_msg: e.message().to_string(),
        }),
        Variant::List(l) => {
            let mut v = Vec::new();
            for e in l.iter() {
                v.push(var_as_json(&e));
            }
            serde_json::Value::Array(v)
        }
        Variant::Map(_m) => {
            unimplemented!("Maps are not supported in JSON serialization");
        }
    }
}

pub fn json_as_var(j: &serde_json::Value) -> Option<Var> {
    match j {
        serde_json::Value::Null => Some(v_none()),
        serde_json::Value::String(s) => Some(v_str(&s)),
        serde_json::Value::Number(n) => Some(if n.is_i64() {
            v_int(n.as_i64().unwrap())
        } else {
            v_float(n.as_f64().unwrap())
        }),
        serde_json::Value::Object(_o) => {
            // Object references in JSON are encoded as one of:
            // { "oid": 1234 }
            // { "sysobj": "name[.name]" }
            // { "match": "name[.name]" }
            // Not valid.
            unimplemented!("Object references in JSON");
        }
        serde_json::Value::Array(a) => {
            let mut v = Vec::new();
            for e in a.iter() {
                v.push(json_as_var(e)?);
            }
            Some(v_list(&v))
        }
        _ => None,
    }
}

pub fn serialize_var<S>(v: &Var, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let j = var_as_json(v);
    j.serialize(s)
}
