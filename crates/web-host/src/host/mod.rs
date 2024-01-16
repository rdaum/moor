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

use moor_values::var::Var;
use moor_values::var::Variant;
use serde_derive::{Deserialize, Serialize};
use serde_json::{json, Number};

pub use web_host::WebHost;
pub use web_host::{
    connect_auth_handler, create_auth_handler, eval_handler, welcome_message_handler,
    ws_connect_attach_handler, ws_create_attach_handler,
};

#[derive(Serialize, Deserialize)]
struct Oid {
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
        Variant::Obj(o) => json!(Oid { oid: o.0 }),
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
