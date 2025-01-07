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

mod auth;
mod props;
mod verbs;
pub mod web_host;
mod ws_connection;

pub use auth::connect_auth_handler;
pub use auth::create_auth_handler;
use moor_values::{v_err, v_float, v_int, v_list, v_map, v_none, v_objid, v_str, Var, Variant};
pub use props::properties_handler;
pub use props::property_retrieval_handler;
use serde::Serialize;
use serde_derive::Deserialize;
use serde_json::{json, Number};
pub use verbs::verb_program_handler;
pub use verbs::verb_retrieval_handler;
pub use verbs::verbs_handler;
pub use web_host::WebHost;
pub use web_host::{
    eval_handler, resolve_objref_handler, welcome_message_handler, ws_connect_attach_handler,
    ws_create_attach_handler,
};

#[derive(serde_derive::Serialize, Deserialize)]
struct Oid {
    oid: i64,
}

#[derive(serde_derive::Serialize, Deserialize)]
struct Error {
    error: String,
    error_msg: Option<String>,
}

/// Construct a JSON representation of a Var.
/// This is not a straight-ahead representation because moo common have some semantic differences
/// from JSON common, in particular:
/// - Maps are not supported in JSON serialization, so we have to encode them as a list of pairs,
///   with a tag to indicate that it's a map.
/// - Object references are encoded as a JSON object with a tag to indicate the type of reference.
/// - Errors are encoded as a JSON object with a tag to indicate the type of error.
/// - Lists are encoded as JSON arrays.
/// - Strings are encoded as JSON strings.
/// - Integers & floats are encoded as JSON numbers, but there's a caveat here that JSON's spec
///   can't permit a full 64-bit integer, so we have to be careful about that.
/// - Future things like WAIFs, etc. will need to be encoded in a way that makes sense for JSON.
pub fn var_as_json(v: &Var) -> serde_json::Value {
    match v.variant() {
        Variant::None => serde_json::Value::Null,
        Variant::Str(s) => serde_json::Value::String(s.to_string()),
        Variant::Obj(o) => json!(Oid {
            oid: o.id().0 as i64
        }),
        Variant::Int(i) => serde_json::Value::Number(Number::from(*i)),
        Variant::Float(f) => json!(*f),
        Variant::Err(e) => json!(Error {
            error: e.name().to_string(),
            error_msg: Some(e.message().to_string()),
        }),
        Variant::List(l) => {
            let mut v = Vec::new();
            for e in l.iter() {
                v.push(var_as_json(&e));
            }
            serde_json::Value::Array(v)
        }
        Variant::Map(m) => {
            // A map is encoded as an object containing a tag and a list of key-value pairs.
            let mut v = Vec::new();
            for (k, e) in m.iter() {
                v.push(json!(&[var_as_json(&k), var_as_json(&e)]));
            }
            json!({ "map_pairs": v })
        }
        Variant::Flyweight(f) => {
            if f.is_sealed() {
                json!("sealed_flyweight")
            } else {
                let mut slotmap = serde_json::Map::new();
                for s in f.slots() {
                    slotmap.insert(s.0.to_string(), var_as_json(&s.1));
                }

                let json_map = serde_json::Value::Object(slotmap);
                json!({"flyweight": json_map})
            }
        }
    }
}

// Not used yet
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum JsonParseError {
    #[error("Unknown type")]
    UnknownType,
    #[error("Unknown error")]
    UnknownError,
    #[error("Invalid representation")]
    InvalidRepresentation,
}

#[allow(dead_code)]
pub fn json_as_var(j: &serde_json::Value) -> Result<Var, JsonParseError> {
    match j {
        serde_json::Value::Null => Ok(v_none()),
        serde_json::Value::String(s) => Ok(v_str(s)),
        serde_json::Value::Number(n) => Ok(if n.is_i64() {
            v_int(n.as_i64().unwrap())
        } else {
            v_float(n.as_f64().unwrap())
        }),
        serde_json::Value::Object(o) => {
            // An object can be one of three things (for now)
            // - An object reference, which can be oid: <number>. <TODO: sysrefs as their own type? somehow?>
            // - An error, which can be error_code: <number>, error_name: <string>, error_msg: <string>
            // - A map, which is a list of key-value pairs in the "pairs" field.
            if let Some(oid) = o.get("oid") {
                let Some(oid) = oid.as_number() else {
                    return Err(JsonParseError::InvalidRepresentation);
                };
                let Some(oid) = oid.as_i64() else {
                    return Err(JsonParseError::InvalidRepresentation);
                };
                let oid = if oid < i32::MIN as i64 || oid > i32::MAX as i64 {
                    return Err(JsonParseError::InvalidRepresentation);
                } else {
                    oid as i32
                };
                return Ok(v_objid(oid));
            }

            if let Some(pairs) = o.get("map_pairs") {
                let Some(pairs) = pairs.as_array() else {
                    return Err(JsonParseError::InvalidRepresentation);
                };
                let mut m = vec![];
                for pair in pairs.iter() {
                    let Some(pair) = pair.as_array() else {
                        return Err(JsonParseError::InvalidRepresentation);
                    };
                    if pair.len() != 2 {
                        return Err(JsonParseError::InvalidRepresentation);
                    }
                    let key = pair.first().ok_or(JsonParseError::InvalidRepresentation)?;
                    let value = pair.get(1).ok_or(JsonParseError::InvalidRepresentation)?;
                    m.push((json_as_var(key)?, json_as_var(value)?));
                }
                return Ok(v_map(&m));
            }

            if let Some(error_name) = o.get("error") {
                // Match against the symbols in Error
                let e = moor_values::Error::parse_str(
                    error_name
                        .as_str()
                        .ok_or(JsonParseError::InvalidRepresentation)?,
                )
                .ok_or(JsonParseError::UnknownError)?;

                return Ok(v_err(e));
            }

            Err(JsonParseError::UnknownType)
        }
        serde_json::Value::Array(a) => {
            let mut v = Vec::new();
            for e in a.iter() {
                v.push(json_as_var(e)?);
            }
            Ok(v_list(&v))
        }
        _ => Err(JsonParseError::UnknownType),
    }
}

pub fn serialize_var<S>(v: &Var, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let j = var_as_json(v);
    j.serialize(s)
}

#[cfg(test)]
mod tests {
    use moor_values::{v_err, v_float, v_int, v_str};

    #[test]
    fn test_int_to_fro() {
        let n = v_int(42);
        let j = super::var_as_json(&n);
        let n2 = super::json_as_var(&j).unwrap();
        assert_eq!(n, n2);
    }

    #[test]
    fn test_float_to_fro() {
        let n = v_float(42.0);
        let j = super::var_as_json(&n);
        let n2 = super::json_as_var(&j).unwrap();
        assert_eq!(n, n2);
    }

    #[test]
    fn test_string_to_fro() {
        let n = v_str("hello");
        let j = super::var_as_json(&n);
        let n2 = super::json_as_var(&j).unwrap();
        assert_eq!(n, n2);
    }

    #[test]
    fn test_error_to_fro() {
        let n = v_err(moor_values::Error::E_ARGS);
        let j = super::var_as_json(&n);
        let n2 = super::json_as_var(&j).unwrap();
        assert_eq!(n, n2);
    }

    #[test]
    fn test_list_to_fro() {
        let n = moor_values::v_list(&[v_int(42), v_float(42.0), v_str("hello")]);
        let j = super::var_as_json(&n);
        let n2 = super::json_as_var(&j).unwrap();
        assert_eq!(n, n2);
    }

    #[test]
    fn test_map_to_fro() {
        let n = moor_values::v_map(&[(v_int(42), v_float(42.0)), (v_str("hello"), v_str("world"))]);
        let j = super::var_as_json(&n);
        let n2 = super::json_as_var(&j).unwrap();
        assert_eq!(n, n2);
    }

    #[test]
    fn test_obj_to_fro() {
        let n = moor_values::v_objid(42);
        let j = super::var_as_json(&n);
        let n2 = super::json_as_var(&j).unwrap();
        assert_eq!(n, n2);
    }
}
