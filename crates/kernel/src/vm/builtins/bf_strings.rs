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

use argon2::password_hash::SaltString;
use argon2::{Algorithm, Argon2, Params, PasswordHasher, PasswordVerifier, Version};
use base64::Engine;
use base64::engine::general_purpose;
use md5::Digest;
use moor_compiler::offset_for_builtin;
use moor_var::{E_ARGS, E_INVARG, E_TYPE};
use moor_var::{Sequence, Variant};
use moor_var::{v_int, v_map, v_str, v_string};
use rand::distributions::Alphanumeric;
use rand::{Rng, thread_rng};
use serde_json::{self, Value as JsonValue};
use tracing::warn;

use crate::vm::builtins::BfRet::Ret;
use crate::vm::builtins::{BfCallState, BfErr, BfRet, BuiltinFunction, world_state_bf_err};

fn strsub(subject: &str, what: &str, with: &str, case_matters: bool) -> String {
    let mut result = String::new();
    let mut source = subject;

    if what.is_empty() {
        return subject.to_string();
    }

    while let Some(index) = if case_matters {
        source.find(what)
    } else {
        source.to_lowercase().find(&what.to_lowercase())
    } {
        result.push_str(&source[..index]);
        result.push_str(with);
        let next = index + what.len();
        source = &source[next..];
    }

    result.push_str(source);

    result
}

//Function: str strsub (str subject, str what, str with [, case-matters])
fn bf_strsub(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    let case_matters = if bf_args.args.len() == 3 {
        false
    } else if bf_args.args.len() == 4 {
        let Some(case_matters) = bf_args.args[3].as_integer() else {
            return Err(BfErr::Code(E_TYPE));
        };
        case_matters == 1
    } else {
        return Err(BfErr::Code(E_ARGS));
    };
    let (subject, what, with) = (
        bf_args.args[0].variant(),
        bf_args.args[1].variant(),
        bf_args.args[2].variant(),
    );
    match (subject, what, with) {
        (Variant::Str(subject), Variant::Str(what), Variant::Str(with)) => Ok(Ret(v_str(
            strsub(subject.as_str(), what.as_str(), with.as_str(), case_matters).as_str(),
        ))),
        _ => Err(BfErr::Code(E_TYPE)),
    }
}

fn str_index(subject: &str, what: &str, case_matters: bool) -> i64 {
    if case_matters {
        subject.find(what).map(|i| i as i64 + 1).unwrap_or(0)
    } else {
        subject
            .to_lowercase()
            .find(&what.to_lowercase())
            .map(|i| i as i64 + 1)
            .unwrap_or(0)
    }
}

fn str_rindex(subject: &str, what: &str, case_matters: bool) -> i64 {
    if case_matters {
        subject.rfind(what).map(|i| i as i64 + 1).unwrap_or(0)
    } else {
        subject
            .to_lowercase()
            .rfind(&what.to_lowercase())
            .map(|i| i as i64 + 1)
            .unwrap_or(0)
    }
}

fn bf_index(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    let case_matters = if bf_args.args.len() == 2 {
        false
    } else if bf_args.args.len() == 3 {
        let Some(case_matters) = bf_args.args[2].as_integer() else {
            return Err(BfErr::Code(E_TYPE));
        };
        case_matters == 1
    } else {
        return Err(BfErr::Code(E_ARGS));
    };

    let (subject, what) = (bf_args.args[0].variant(), bf_args.args[1].variant());
    match (subject, what) {
        (Variant::Str(subject), Variant::Str(what)) => Ok(Ret(v_int(str_index(
            subject.as_str(),
            what.as_str(),
            case_matters,
        )))),
        _ => Err(BfErr::Code(E_TYPE)),
    }
}

fn bf_rindex(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    let case_matters = if bf_args.args.len() == 2 {
        false
    } else if bf_args.args.len() == 3 {
        let Some(case_matters) = bf_args.args[2].as_integer() else {
            return Err(BfErr::Code(E_TYPE));
        };
        case_matters == 1
    } else {
        return Err(BfErr::Code(E_ARGS));
    };

    let (subject, what) = (bf_args.args[0].variant(), bf_args.args[1].variant());
    match (subject, what) {
        (Variant::Str(subject), Variant::Str(what)) => Ok(Ret(v_int(str_rindex(
            subject.as_str(),
            what.as_str(),
            case_matters,
        )))),
        _ => Err(BfErr::Code(E_TYPE)),
    }
}

fn bf_strcmp(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::Code(E_ARGS));
    }
    let (str1, str2) = (bf_args.args[0].variant(), bf_args.args[1].variant());
    match (str1, str2) {
        (Variant::Str(str1), Variant::Str(str2)) => {
            Ok(Ret(v_int(str1.as_str().cmp(str2.as_str()) as i64)))
        }
        _ => Err(BfErr::Code(E_TYPE)),
    }
}

/// Generate a random cryptographically secure salt string, for use with crypt & argon2
/// Note: This is not (for now) compatible with the `salt` function in ToastStunt, which takes
/// two arguments.
fn bf_salt(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(BfErr::Code(E_ARGS));
    }

    let mut rng_core = thread_rng();
    let salt = SaltString::generate(&mut rng_core);
    let salt = v_str(salt.as_str());
    Ok(Ret(salt))
}

/*
str crypt (str text [, str salt])

Encrypts the given text using the standard UNIX encryption method. If provided, salt should be a
string at least two characters long, the first two characters of which will be used as the extra
encryption "salt" in the algorithm. If salt is not provided, a random pair of characters is used.
 In any case, the salt used is also returned as the first two characters of the resulting encrypted
 string.
*/
fn bf_crypt(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.is_empty() || bf_args.args.len() > 2 {
        return Err(BfErr::Code(E_ARGS));
    }

    let salt = if bf_args.args.len() == 1 {
        // Provide a random 2-letter salt.
        let mut rng = rand::thread_rng();
        let mut salt = String::new();

        salt.push(char::from(rng.sample(Alphanumeric)));
        salt.push(char::from(rng.sample(Alphanumeric)));
        salt
    } else {
        let Some(salt) = bf_args.args[1].as_string() else {
            return Err(BfErr::Code(E_TYPE));
        };
        String::from(salt)
    };
    if let Some(text) = bf_args.args[0].as_string() {
        let crypted = pwhash::unix::crypt(text, salt.as_str()).unwrap();
        Ok(Ret(v_string(crypted)))
    } else {
        Err(BfErr::Code(E_TYPE))
    }
}

fn bf_string_hash(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }
    match bf_args.args[0].variant() {
        Variant::Str(s) => {
            let hash_digest = md5::Md5::digest(s.as_str().as_bytes());
            Ok(Ret(v_str(
                format!("{:x}", hash_digest).to_uppercase().as_str(),
            )))
        }
        _ => Err(BfErr::Code(E_INVARG)),
    }
}

fn bf_binary_hash(_bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    Err(BfErr::Code(E_INVARG))
}

// password (string), salt (string), iterations, memory, parallelism
fn bf_argon2(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // Must be wizard.
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    if bf_args.args.len() > 5 || bf_args.args.len() < 2 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Some(password) = bf_args.args[0].as_string() else {
        return Err(BfErr::Code(E_TYPE));
    };
    let Some(salt) = bf_args.args[1].as_string() else {
        return Err(BfErr::Code(E_TYPE));
    };

    let iterations = if bf_args.args.len() > 2 {
        let Some(iterations) = bf_args.args[2].as_integer() else {
            return Err(BfErr::Code(E_TYPE));
        };
        iterations as u32
    } else {
        3
    };
    let memory = if bf_args.args.len() > 3 {
        let Some(memory) = bf_args.args[3].as_integer() else {
            return Err(BfErr::Code(E_TYPE));
        };
        memory as u32
    } else {
        4096
    };

    let parallelism = if bf_args.args.len() > 4 {
        let Some(parallelism) = bf_args.args[4].as_integer() else {
            return Err(BfErr::Code(E_TYPE));
        };
        parallelism as u32
    } else {
        1
    };

    let params = Params::new(memory, iterations, parallelism, None).map_err(|e| {
        warn!("Failed to create argon2 params: {}", e);
        BfErr::Code(E_INVARG)
    })?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let salt_string = SaltString::encode_b64(salt.as_bytes()).map_err(|e| {
        warn!("Failed to encode salt: {}", e);
        BfErr::Code(E_INVARG)
    })?;

    let hash = argon2
        .hash_password(password.as_bytes(), &salt_string)
        .map_err(|e| {
            warn!("Failed to hash password: {}", e);
            BfErr::Code(E_INVARG)
        })?;

    Ok(Ret(v_string(hash.to_string())))
}

// password, salt
fn bf_argon2_verify(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // Must be wizard.
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    if bf_args.args.len() != 2 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Some(hashed_password) = bf_args.args[0].as_string() else {
        return Err(BfErr::Code(E_TYPE));
    };
    let Some(password) = bf_args.args[1].as_string() else {
        return Err(BfErr::Code(E_TYPE));
    };

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, Params::default());
    let Ok(hashed_password) = argon2::PasswordHash::new(hashed_password) else {
        return Err(BfErr::Code(E_INVARG));
    };

    let validated = argon2
        .verify_password(password.as_bytes(), &hashed_password)
        .is_ok();
    Ok(Ret(bf_args.v_bool(validated)))
}

/// Function: str encode_base64(str text)
///
/// Encodes the given string using Base64 encoding.
/// Returns the Base64-encoded string.
fn bf_encode_base64(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let Some(text) = bf_args.args[0].as_string() else {
        return Err(BfErr::Code(E_TYPE));
    };

    let encoded = general_purpose::STANDARD.encode(text.as_bytes());
    Ok(Ret(v_string(encoded)))
}

/// Function: str decode_base64(str encoded_text)
///
/// Decodes the given Base64-encoded string.
/// Returns the decoded string. If the input is not valid Base64, E_INVARG is raised.
fn bf_decode_base64(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let Some(encoded_text) = bf_args.args[0].as_string() else {
        return Err(BfErr::Code(E_TYPE));
    };

    let decoded = match general_purpose::STANDARD.decode(encoded_text.as_bytes()) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(s) => s,
            Err(_) => return Err(BfErr::Code(E_INVARG)),
        },
        Err(_) => return Err(BfErr::Code(E_INVARG)),
    };

    Ok(Ret(v_string(decoded)))
}

/// Convert a MOO value to a JSON string
fn bf_generate_json(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let value = &bf_args.args[0];
    let json_value = moo_value_to_json(value)?;

    match serde_json::to_string(&json_value) {
        Ok(json_str) => Ok(Ret(v_string(json_str))),
        Err(_) => Err(BfErr::Code(E_INVARG)),
    }
}

/// Convert a MOO value to a JSON value
fn moo_value_to_json(value: &moor_var::Var) -> Result<JsonValue, BfErr> {
    match value.variant() {
        Variant::Int(i) => Ok(JsonValue::Number((*i).into())),
        Variant::Float(f) => {
            let num = serde_json::Number::from_f64(*f).ok_or_else(|| BfErr::Code(E_INVARG))?;
            Ok(JsonValue::Number(num))
        }
        Variant::Str(s) => Ok(JsonValue::String(s.as_str().to_string())),
        Variant::Obj(o) => Ok(JsonValue::String(format!("#{}", o))),
        Variant::List(list) => {
            let mut json_array = Vec::new();
            for item in list.iter() {
                json_array.push(moo_value_to_json(&item)?);
            }
            Ok(JsonValue::Array(json_array))
        }
        Variant::Map(map) => {
            let mut json_obj = serde_json::Map::new();
            for (k, v) in map.iter() {
                // JSON only allows string keys
                let key = match k.variant() {
                    Variant::Str(s) => s.as_str().to_string(),
                    Variant::Int(i) => i.to_string(),
                    Variant::Float(f) => f.to_string(),
                    Variant::Obj(o) => format!("#{}", o),
                    _ => return Err(BfErr::Code(E_TYPE)), // Complex keys not supported
                };
                json_obj.insert(key, moo_value_to_json(&v)?);
            }
            Ok(JsonValue::Object(json_obj))
        }
        _ => Err(BfErr::Code(E_TYPE)), // Other types not supported
    }
}

/// Convert a JSON value to a MOO value
fn json_value_to_moo(json_value: &JsonValue) -> Result<moor_var::Var, BfErr> {
    match json_value {
        JsonValue::Null => Ok(moor_var::v_none()),
        JsonValue::Bool(b) => Ok(v_int(if *b { 1 } else { 0 })),
        JsonValue::Number(n) => {
            if n.is_i64() {
                Ok(v_int(n.as_i64().unwrap()))
            } else {
                Ok(moor_var::v_float(n.as_f64().unwrap()))
            }
        }
        JsonValue::String(s) => Ok(v_str(s)),
        JsonValue::Array(arr) => {
            let mut list_items = Vec::new();
            for item in arr {
                list_items.push(json_value_to_moo(item)?);
            }
            Ok(moor_var::v_list(&list_items))
        }
        JsonValue::Object(obj) => {
            let mut map_items = Vec::new();
            for (k, v) in obj {
                let key = v_str(k);
                let value = json_value_to_moo(v)?;
                map_items.push((key, value));
            }
            Ok(v_map(&map_items))
        }
    }
}
/// Parse a JSON string into a MOO value
fn bf_parse_json(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let Some(json_str) = bf_args.args[0].as_string() else {
        return Err(BfErr::Code(E_TYPE));
    };

    match serde_json::from_str::<JsonValue>(json_str) {
        Ok(json_value) => {
            let moo_value = json_value_to_moo(&json_value)?;
            Ok(Ret(moo_value))
        }
        Err(_) => Err(BfErr::Code(E_INVARG)),
    }
}

pub(crate) fn register_bf_strings(builtins: &mut [Box<BuiltinFunction>]) {
    builtins[offset_for_builtin("strsub")] = Box::new(bf_strsub);
    builtins[offset_for_builtin("index")] = Box::new(bf_index);
    builtins[offset_for_builtin("rindex")] = Box::new(bf_rindex);
    builtins[offset_for_builtin("strcmp")] = Box::new(bf_strcmp);
    builtins[offset_for_builtin("crypt")] = Box::new(bf_crypt);
    builtins[offset_for_builtin("argon2")] = Box::new(bf_argon2);
    builtins[offset_for_builtin("argon2_verify")] = Box::new(bf_argon2_verify);
    builtins[offset_for_builtin("string_hash")] = Box::new(bf_string_hash);
    builtins[offset_for_builtin("binary_hash")] = Box::new(bf_binary_hash);
    builtins[offset_for_builtin("salt")] = Box::new(bf_salt);
    builtins[offset_for_builtin("encode_base64")] = Box::new(bf_encode_base64);
    builtins[offset_for_builtin("decode_base64")] = Box::new(bf_decode_base64);
    builtins[offset_for_builtin("generate_json")] = Box::new(bf_generate_json);
    builtins[offset_for_builtin("parse_json")] = Box::new(bf_parse_json);
}

#[cfg(test)]
mod tests {
    use crate::vm::builtins::bf_strings::{json_value_to_moo, moo_value_to_json, strsub};
    use moor_var::{Associative, v_int, v_list, v_map, v_str};
    use serde_json::json;

    #[test]
    fn test_strsub_remove_piece() {
        let subject = "empty_message_integrate_room";
        assert_eq!(
            strsub(subject, "empty_message_", "", false),
            "integrate_room"
        );
    }

    #[test]
    fn test_strsub_case_insensitive_substitution() {
        let subject = "foo bar baz";
        let expected = "fizz bar baz";
        assert_eq!(strsub(subject, "foo", "fizz", false), expected);
    }

    #[test]
    fn test_strsub_case_sensitive_substitution() {
        let subject = "foo bar baz";
        let expected = "foo bar fizz";
        assert_eq!(strsub(subject, "baz", "fizz", true), expected);
    }

    #[test]
    fn test_strsub_empty_subject() {
        let subject = "";
        let expected = "";
        assert_eq!(strsub(subject, "foo", "fizz", false), expected);
    }

    #[test]
    fn test_strsub_empty_what() {
        let subject = "foo bar baz";
        let expected = "foo bar baz";
        assert_eq!(strsub(subject, "", "fizz", false), expected);
    }

    #[test]
    fn test_strsub_multiple_occurrences() {
        let subject = "foo foo foo";
        let expected = "fizz fizz fizz";
        assert_eq!(strsub(subject, "foo", "fizz", false), expected);
    }

    #[test]
    fn test_strsub_no_occurrences() {
        let subject = "foo bar baz";
        let expected = "foo bar baz";
        assert_eq!(strsub(subject, "fizz", "buzz", false), expected);
    }

    #[test]
    fn test_moo_to_json_primitives() {
        // Test integer
        let int_val = v_int(42);
        assert_eq!(moo_value_to_json(&int_val).unwrap(), json!(42));

        // Test string
        let str_val = v_str("hello");
        assert_eq!(moo_value_to_json(&str_val).unwrap(), json!("hello"));
    }

    #[test]
    fn test_moo_to_json_complex() {
        // Test list
        let list_val = v_list(&[v_int(1), v_int(2), v_str("three")]);
        assert_eq!(
            moo_value_to_json(&list_val).unwrap(),
            json!([1, 2, "three"])
        );

        // Test map
        let map_val = v_map(&[(v_str("key1"), v_int(1)), (v_str("key2"), v_str("value"))]);
        assert_eq!(
            moo_value_to_json(&map_val).unwrap(),
            json!({"key1": 1, "key2": "value"})
        );
    }

    #[test]
    fn test_json_to_moo_primitives() {
        // Test null
        let null_json = json!(null);
        assert!(matches!(
            json_value_to_moo(&null_json).unwrap().variant(),
            moor_var::Variant::None
        ));

        // Test boolean
        let bool_json = json!(true);
        assert_eq!(json_value_to_moo(&bool_json).unwrap(), v_int(1));

        // Test number
        let num_json = json!(42);
        assert_eq!(json_value_to_moo(&num_json).unwrap(), v_int(42));

        // Test string
        let str_json = json!("hello");
        assert_eq!(json_value_to_moo(&str_json).unwrap(), v_str("hello"));
    }

    #[test]
    fn test_json_to_moo_complex() {
        // Test array
        let array_json = json!([1, "two", true]);
        let array_moo = json_value_to_moo(&array_json).unwrap();
        let list_items = match array_moo.variant() {
            moor_var::Variant::List(list) => list.iter().collect::<Vec<_>>(),
            _ => panic!("Expected list"),
        };
        assert_eq!(list_items.len(), 3);
        assert_eq!(list_items[0], v_int(1));
        assert_eq!(list_items[1], v_str("two"));
        assert_eq!(list_items[2], v_int(1)); // true becomes 1

        // Test object
        let obj_json = json!({"key1": 1, "key2": "value"});
        let obj_moo = json_value_to_moo(&obj_json).unwrap();
        match obj_moo.variant() {
            moor_var::Variant::Map(map) => {
                assert_eq!(map.len(), 2);
                // Check keys and values exist
                assert_eq!(map.get(&v_str("key1")).unwrap(), v_int(1));
                assert_eq!(map.get(&v_str("key2")).unwrap(), v_str("value"));
            }
            _ => panic!("Expected map"),
        };
    }
}
