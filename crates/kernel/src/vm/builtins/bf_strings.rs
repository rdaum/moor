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
use moor_var::{v_int, v_str, v_string};
use rand::distributions::Alphanumeric;
use rand::{Rng, thread_rng};
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
                format!("{hash_digest:x}").to_uppercase().as_str(),
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

    let bytes = match bf_args.args[0].variant() {
        Variant::Str(s) => s.as_str().as_bytes().to_vec(),
        Variant::Binary(b) => b.as_bytes().to_vec(),
        _ => return Err(BfErr::Code(E_TYPE)),
    };

    let encoded = general_purpose::STANDARD.encode(&bytes);
    Ok(Ret(v_string(encoded)))
}

/// Function: binary decode_base64(str encoded_text [, int url_safe])
///
/// Decodes the given Base64-encoded string.
/// Returns the decoded binary data. If the input is not valid Base64, E_INVARG is raised.
/// If url_safe is true (non-zero), uses URL-safe Base64 decoding. Defaults to true.
fn bf_decode_base64(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() < 1 || bf_args.args.len() > 2 {
        return Err(BfErr::Code(E_ARGS));
    }

    let Some(encoded_text) = bf_args.args[0].as_string() else {
        return Err(BfErr::Code(E_TYPE));
    };

    // Check if second argument specifies URL-safe decoding
    let url_safe = if bf_args.args.len() == 2 {
        bf_args.args[1].is_true()
    } else {
        true
    };

    let decoded_bytes = if url_safe {
        match general_purpose::URL_SAFE.decode(encoded_text.as_bytes()) {
            Ok(bytes) => bytes,
            Err(_) => return Err(BfErr::Code(E_INVARG)),
        }
    } else {
        match general_purpose::STANDARD.decode(encoded_text.as_bytes()) {
            Ok(bytes) => bytes,
            Err(_) => return Err(BfErr::Code(E_INVARG)),
        }
    };

    use moor_var::v_binary;
    Ok(Ret(v_binary(decoded_bytes)))
}

// str string_hmac(str text, str key [, str algo [, binary]])
fn bf_string_hmac(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    let arg_count = bf_args.args.len();
    if !(2..=4).contains(&arg_count) {
        return Err(BfErr::Code(E_ARGS));
    }

    let Variant::Str(text) = bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };
    let Variant::Str(key) = bf_args.args[1].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };

    let algo_str = if arg_count > 2 {
        let Variant::Str(s) = bf_args.args[2].variant() else {
            return Err(BfErr::Code(E_TYPE));
        };
        s.as_str().to_uppercase()
    } else {
        "SHA256".to_string()
    };

    let binary_output = if arg_count > 3 {
        let Variant::Int(b) = bf_args.args[3].variant() else {
            return Err(BfErr::Code(E_TYPE));
        };
        *b != 0
    } else {
        false
    };

    let result_bytes = match algo_str.as_str() {
        "SHA1" => {
            use hmac::{Hmac, Mac};
            use sha1::Sha1;
            let mut mac = Hmac::<Sha1>::new_from_slice(key.as_str().as_bytes())
                .map_err(|_| BfErr::Code(E_INVARG))?;
            mac.update(text.as_str().as_bytes());
            mac.finalize().into_bytes().to_vec()
        }
        "SHA256" => {
            use hmac::{Hmac, Mac};
            use sha2::Sha256;
            let mut mac = Hmac::<Sha256>::new_from_slice(key.as_str().as_bytes())
                .map_err(|_| BfErr::Code(E_INVARG))?;
            mac.update(text.as_str().as_bytes());
            mac.finalize().into_bytes().to_vec()
        }
        _ => return Err(BfErr::Code(E_INVARG)), // Unsupported algorithm
    };

    if binary_output {
        Err(BfErr::ErrValue(E_INVARG.with_msg(|| {
            "Binary support not implemented yet.".to_string()
        })))
    } else {
        let hex_string = result_bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        Ok(Ret(v_str(&hex_string)))
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
    builtins[offset_for_builtin("string_hmac")] = Box::new(bf_string_hmac);
    builtins[offset_for_builtin("salt")] = Box::new(bf_salt);
    builtins[offset_for_builtin("encode_base64")] = Box::new(bf_encode_base64);
    builtins[offset_for_builtin("decode_base64")] = Box::new(bf_decode_base64);
}

#[cfg(test)]
mod tests {
    use crate::vm::builtins::bf_strings::strsub;

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
}
