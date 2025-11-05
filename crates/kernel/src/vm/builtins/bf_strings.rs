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

use base64::{Engine, engine::general_purpose};
use md5::Digest;
use moor_compiler::offset_for_builtin;
use moor_var::{
    E_ARGS, E_INVARG, E_TYPE, Sequence, Variant, v_binary, v_int, v_list, v_str, v_string,
};

use crate::vm::builtins::{BfCallState, BfErr, BfRet, BfRet::Ret, BuiltinFunction};

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

pub(crate) fn register_bf_strings(builtins: &mut [BuiltinFunction]) {
    builtins[offset_for_builtin("strsub")] = bf_strsub;
    builtins[offset_for_builtin("index")] = bf_index;
    builtins[offset_for_builtin("rindex")] = bf_rindex;
    builtins[offset_for_builtin("strcmp")] = bf_strcmp;
    builtins[offset_for_builtin("string_hash")] = bf_string_hash;
    builtins[offset_for_builtin("binary_hash")] = bf_binary_hash;
    builtins[offset_for_builtin("encode_base64")] = bf_encode_base64;
    builtins[offset_for_builtin("decode_base64")] = bf_decode_base64;
    builtins[offset_for_builtin("binary_to_str")] = bf_binary_to_str;
    builtins[offset_for_builtin("binary_from_str")] = bf_binary_from_str;
    builtins[offset_for_builtin("explode")] = bf_explode;
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
