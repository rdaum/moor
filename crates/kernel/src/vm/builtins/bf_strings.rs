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

//! Builtin functions for string manipulation, hashing, and encoding operations.

use argon2::{
    Algorithm, Argon2, Params, PasswordHasher, PasswordVerifier, Version,
    password_hash::{SaltString, rand_core::OsRng},
};
use base64::{Engine, engine::general_purpose};
use hmac::{Hmac, Mac};
use lazy_static::lazy_static;
use md5::Digest;
use moor_compiler::{offset_for_builtin, to_literal};
use moor_var::{
    E_ARGS, E_INVARG, E_TYPE, Sequence, Symbol, Variant, v_binary, v_int, v_list, v_str, v_string,
};
use rand::{Rng, distr::Alphanumeric};
use sha1::Sha1;
use sha2::Sha256;
use tracing::warn;

use crate::vm::builtins::{
    BfCallState, BfErr, BfRet, BfRet::Ret, BuiltinFunction, world_state_bf_err,
};

lazy_static! {
    static ref SHA1_SYM: Symbol = Symbol::mk("sha1");
    static ref SHA256_SYM: Symbol = Symbol::mk("sha256");
}

/// Internal helper for string substitution with case sensitivity control.
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

/// MOO: `str strsub(str subject, str what, str with [, bool case_matters])`
/// Substitutes all occurrences of 'what' in 'subject' with 'with'.
fn bf_strsub(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    let case_matters = if bf_args.args.len() == 3 {
        false
    } else if bf_args.args.len() == 4 {
        bf_args.args[3].is_true()
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

/// Internal helper for finding first occurrence of substring.
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

/// Internal helper for finding last occurrence of substring.
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

/// MOO: `int index(str subject, str what [, bool case_matters])`
/// Returns the position of the first occurrence of 'what' in 'subject' (1-based), or 0 if not found.
fn bf_index(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    let case_matters = if bf_args.args.len() == 2 {
        false
    } else if bf_args.args.len() == 3 {
        bf_args.args[2].is_true()
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

/// MOO: `int rindex(str subject, str what [, bool case_matters])`
/// Returns the position of the last occurrence of 'what' in 'subject' (1-based), or 0 if not found.
fn bf_rindex(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    let case_matters = if bf_args.args.len() == 2 {
        false
    } else if bf_args.args.len() == 3 {
        bf_args.args[2].is_true()
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

/// MOO: `int strcmp(str str1, str str2)`
/// Compares two strings lexicographically. Returns -1, 0, or 1.
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

/// MOO: `str salt()`
/// Generates a random cryptographically secure salt string for use with crypt & argon2.
/// Note: Not compatible with ToastStunt's salt function which takes two arguments.
fn bf_salt(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(BfErr::Code(E_ARGS));
    }

    let mut rng_core = OsRng;
    let salt = SaltString::generate(&mut rng_core);
    let salt = v_str(salt.as_str());
    Ok(Ret(salt))
}

/// MOO: `str crypt(str text [, str salt])`
/// Encrypts text using standard UNIX encryption method.
/// If salt is provided, uses first two characters as encryption salt.
/// If no salt provided, uses random pair. Salt is returned as first two characters of result.
fn bf_crypt(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.is_empty() || bf_args.args.len() > 2 {
        return Err(BfErr::Code(E_ARGS));
    }

    let salt = if bf_args.args.len() == 1 {
        // Provide a random 2-letter salt.
        let mut rng = rand::rng();
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

/// MOO: `str string_hash(str text)`
/// Returns MD5 hash of the given string in uppercase hexadecimal format.
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

/// MOO: `str binary_hash(binary data)`
/// Returns MD5 hash of the given binary data in uppercase hexadecimal format.
fn bf_binary_hash(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Variant::Binary(b) = bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_INVARG));
    };
    let hash_digest = md5::Md5::digest(b.as_bytes());
    Ok(Ret(v_str(
        format!("{hash_digest:x}").to_uppercase().as_str(),
    )))
}

/// MOO: `str argon2(str password, str salt [, int iterations] [, int memory] [, int parallelism])`
/// Generates Argon2 hash with specified parameters. Wizard-only function.
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

/// MOO: `bool argon2_verify(str hashed_password, str password)`
/// Verifies a password against an Argon2 hash. Wizard-only function.
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

/// MOO: `str encode_base64(str|binary data [, bool url_safe] [, bool no_padding])`
/// Encodes the given string or binary data using Base64 encoding.
/// - url_safe: If true, uses URL-safe Base64 alphabet (- and _ instead of + and /). Defaults to false.
/// - no_padding: If true, omits trailing = padding characters. Defaults to false.
fn bf_encode_base64(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.is_empty() || bf_args.args.len() > 3 {
        return Err(BfErr::Code(E_ARGS));
    }

    let bytes = match bf_args.args[0].variant() {
        Variant::Str(s) => s.as_str().as_bytes().to_vec(),
        Variant::Binary(b) => b.as_bytes().to_vec(),
        _ => return Err(BfErr::Code(E_TYPE)),
    };

    let url_safe = bf_args.args.len() >= 2 && bf_args.args[1].is_true();
    let no_padding = bf_args.args.len() >= 3 && bf_args.args[2].is_true();

    let encoded = if url_safe && no_padding {
        general_purpose::URL_SAFE_NO_PAD.encode(&bytes)
    } else if url_safe {
        general_purpose::URL_SAFE.encode(&bytes)
    } else if no_padding {
        general_purpose::STANDARD_NO_PAD.encode(&bytes)
    } else {
        general_purpose::STANDARD.encode(&bytes)
    };

    Ok(Ret(v_string(encoded)))
}

/// MOO: `binary decode_base64(str encoded_text [, bool url_safe])`
/// Decodes Base64-encoded string to binary data.
/// - url_safe: If true, uses URL-safe Base64 alphabet (- and _ instead of + and /). Defaults to false.
fn bf_decode_base64(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() < 1 || bf_args.args.len() > 2 {
        return Err(BfErr::Code(E_ARGS));
    }

    let Some(encoded_text) = bf_args.args[0].as_string() else {
        return Err(BfErr::Code(E_TYPE));
    };

    let url_safe = bf_args.args.len() >= 2 && bf_args.args[1].is_true();

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

    Ok(Ret(v_binary(decoded_bytes)))
}

/// MOO: `str string_hmac(str text, str key [, str algorithm] [, bool binary_output])`
/// Computes HMAC of text using key with specified algorithm (SHA1, SHA256).
fn bf_string_hmac(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    let arg_count = bf_args.args.len();
    if !(2..=4).contains(&arg_count) {
        return Err(BfErr::Code(E_ARGS));
    }

    let Some(text) = bf_args.args[0].as_string() else {
        return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
            format!(
                "Invalid type {} for text argument for string_hmac",
                bf_args.args[0].type_code().to_literal()
            )
        })));
    };

    let Some(key) = bf_args.args[1].as_string() else {
        return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
            format!(
                "Invalid type {} for key argument for string_hmac",
                bf_args.args[1].type_code().to_literal()
            )
        })));
    };

    let algo = if arg_count > 2 {
        let Ok(kind) = bf_args.args[2].as_symbol() else {
            return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
                format!(
                    "Invalid type for algorithm argument in string_hmac: {})",
                    to_literal(&bf_args.args[2])
                )
            })));
        };
        kind
    } else {
        *SHA256_SYM
    };

    let binary_output = arg_count > 3 && bf_args.args[3].is_true();

    let result_bytes = if algo == *SHA1_SYM {
        let mut mac =
            Hmac::<Sha1>::new_from_slice(key.as_bytes()).map_err(|_| BfErr::Code(E_INVARG))?;
        mac.update(text.as_bytes());
        mac.finalize().into_bytes().to_vec()
    } else if algo == *SHA256_SYM {
        let mut mac =
            Hmac::<Sha256>::new_from_slice(key.as_bytes()).map_err(|_| BfErr::Code(E_INVARG))?;
        mac.update(text.as_bytes());
        mac.finalize().into_bytes().to_vec()
    } else {
        return Err(BfErr::ErrValue(
            E_INVARG.with_msg(|| format!("Invalid algorithm for string_hmac: {algo}")),
        ));
    };

    if binary_output {
        Ok(Ret(v_binary(result_bytes)))
    } else {
        let hex_string = result_bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        Ok(Ret(v_str(&hex_string)))
    }
}

/// MOO: `str|binary binary_hmac(binary data, str key [, symbol algorithm] [, bool binary_output])`
/// Computes HMAC of binary data using key with specified algorithm (SHA1, SHA256).
/// Note: Takes mooR's native Binary type, NOT ToastStunt's bin-string format.
fn bf_binary_hmac(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    let arg_count = bf_args.args.len();
    if !(2..=4).contains(&arg_count) {
        return Err(BfErr::Code(E_ARGS));
    }

    let Some(data) = bf_args.args[0].as_binary() else {
        return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
            format!(
                "Invalid type {} for data argument for binary_hmac (requires mooR Binary type, not string)",
                bf_args.args[0].type_code().to_literal()
            )
        })));
    };

    let Some(key) = bf_args.args[1].as_string() else {
        return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
            format!(
                "Invalid type {} for key argument for binary_hmac",
                bf_args.args[1].type_code().to_literal()
            )
        })));
    };

    let algo = if arg_count > 2 {
        let Ok(kind) = bf_args.args[2].as_symbol() else {
            return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
                format!(
                    "Invalid type for algorithm argument in binary_hmac: {})",
                    to_literal(&bf_args.args[2])
                )
            })));
        };
        kind
    } else {
        *SHA256_SYM
    };

    let binary_output = arg_count > 3 && bf_args.args[3].is_true();

    let result_bytes = if algo == *SHA1_SYM {
        let mut mac =
            Hmac::<Sha1>::new_from_slice(key.as_bytes()).map_err(|_| BfErr::Code(E_INVARG))?;
        mac.update(data.as_bytes());
        mac.finalize().into_bytes().to_vec()
    } else if algo == *SHA256_SYM {
        let mut mac =
            Hmac::<Sha256>::new_from_slice(key.as_bytes()).map_err(|_| BfErr::Code(E_INVARG))?;
        mac.update(data.as_bytes());
        mac.finalize().into_bytes().to_vec()
    } else {
        return Err(BfErr::ErrValue(
            E_INVARG.with_msg(|| format!("Invalid algorithm for binary_hmac: {algo}")),
        ));
    };

    if binary_output {
        Ok(Ret(v_binary(result_bytes)))
    } else {
        let hex_string = result_bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        Ok(Ret(v_str(&hex_string)))
    }
}

/// MOO: `str binary_to_str(binary data [, bool allow_lossy])`
/// Converts binary data to a string.
/// If allow_lossy is false (default), returns E_INVARG on invalid UTF-8.
/// If allow_lossy is true, uses replacement character for invalid sequences.
fn bf_binary_to_str(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() < 1 || bf_args.args.len() > 2 {
        return Err(BfErr::Code(E_ARGS));
    }

    let Some(binary) = bf_args.args[0].as_binary() else {
        return Err(BfErr::Code(E_TYPE));
    };

    let allow_lossy = if bf_args.args.len() == 2 {
        bf_args.args[1].is_true()
    } else {
        false
    };

    let result = if allow_lossy {
        String::from_utf8_lossy(binary.as_bytes()).to_string()
    } else {
        match String::from_utf8(binary.as_bytes().to_vec()) {
            Ok(s) => s,
            Err(e) => {
                return Err(BfErr::ErrValue(
                    E_INVARG.with_msg(|| format!("Cannot convert to string: {e}")),
                ));
            }
        }
    };

    Ok(Ret(v_string(result)))
}

/// MOO: `binary binary_from_str(str text)`
/// Converts a string to binary data.
fn bf_binary_from_str(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let Some(text) = bf_args.args[0].as_string() else {
        return Err(BfErr::Code(E_TYPE));
    };

    let binary_data = text.as_bytes().to_vec();
    Ok(Ret(v_binary(binary_data)))
}

/// MOO: `list explode(str subject [, str break [, bool include-sequential-occurrences]])`
/// Splits subject into a list of substrings separated by break character.
/// Only the first character of break is used. Break defaults to space.
/// If include-sequential-occurrences is true, empty strings are included for consecutive breaks.
fn bf_explode(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.is_empty() || bf_args.args.len() > 3 {
        return Err(BfErr::Code(E_ARGS));
    }

    let Some(subject) = bf_args.args[0].as_string() else {
        return Err(BfErr::Code(E_TYPE));
    };

    // Get break character (default to space)
    let break_char = if bf_args.args.len() >= 2 {
        let Some(break_str) = bf_args.args[1].as_string() else {
            return Err(BfErr::Code(E_TYPE));
        };
        if break_str.is_empty() {
            ' '
        } else {
            break_str.chars().next().unwrap()
        }
    } else {
        ' '
    };

    // Get include-sequential-occurrences flag (default to false)
    let include_sequential = if bf_args.args.len() == 3 {
        bf_args.args[2].is_true()
    } else {
        false
    };

    let parts: Vec<_> = if include_sequential {
        // Include empty strings for consecutive separators
        subject.split(break_char).map(v_str).collect()
    } else {
        // Filter out empty strings
        subject
            .split(break_char)
            .filter(|s| !s.is_empty())
            .map(v_str)
            .collect()
    };

    Ok(Ret(v_list(&parts)))
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
    builtins[offset_for_builtin("binary_hmac")] = Box::new(bf_binary_hmac);
    builtins[offset_for_builtin("salt")] = Box::new(bf_salt);
    builtins[offset_for_builtin("encode_base64")] = Box::new(bf_encode_base64);
    builtins[offset_for_builtin("decode_base64")] = Box::new(bf_decode_base64);
    builtins[offset_for_builtin("binary_to_str")] = Box::new(bf_binary_to_str);
    builtins[offset_for_builtin("binary_from_str")] = Box::new(bf_binary_from_str);
    builtins[offset_for_builtin("explode")] = Box::new(bf_explode);
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
