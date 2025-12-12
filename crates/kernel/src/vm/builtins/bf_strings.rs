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

/// Usage: `str strsub(str subject, str what, str with [, bool case_matters])`
/// Replaces all occurrences of 'what' in 'subject' with 'with'. Occurrences are found
/// left to right and all substitutions happen simultaneously. By default, the search
/// ignores case; if case_matters is true, case is significant.
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
/// skip: number of characters to skip from the beginning (must be >= 0)
fn str_index(subject: &str, what: &str, case_matters: bool, skip: usize) -> i64 {
    // Skip characters from the beginning
    let search_start: usize = subject
        .char_indices()
        .nth(skip)
        .map(|(i, _)| i)
        .unwrap_or(subject.len());
    let search_slice = &subject[search_start..];

    let found_in_slice = if case_matters {
        search_slice.find(what)
    } else {
        search_slice.to_lowercase().find(&what.to_lowercase())
    };

    // Convert byte offset in slice back to 1-based character index in original
    found_in_slice
        .map(|byte_offset| {
            let byte_pos = search_start + byte_offset;
            subject[..byte_pos].chars().count() as i64 + 1
        })
        .unwrap_or(0)
}

/// Internal helper for finding last occurrence of substring.
/// skip: number of characters to skip from the end (as negative, e.g., -4 means skip last 4)
fn str_rindex(subject: &str, what: &str, case_matters: bool, skip: i64) -> i64 {
    // skip is negative for rindex - skip last N characters
    let chars_to_keep = if skip < 0 {
        let total_chars = subject.chars().count();
        total_chars.saturating_sub((-skip) as usize)
    } else {
        subject.chars().count()
    };

    // Find byte position after chars_to_keep characters
    let search_end: usize = subject
        .char_indices()
        .nth(chars_to_keep)
        .map(|(i, _)| i)
        .unwrap_or(subject.len());
    let search_slice = &subject[..search_end];

    let found_in_slice = if case_matters {
        search_slice.rfind(what)
    } else {
        search_slice.to_lowercase().rfind(&what.to_lowercase())
    };

    // Convert byte offset to 1-based character index
    found_in_slice
        .map(|byte_offset| subject[..byte_offset].chars().count() as i64 + 1)
        .unwrap_or(0)
}

/// Usage: `int index(str subject, str what [, bool case_matters [, int skip]])`
/// Returns the index of the first character of the first occurrence of 'what' in 'subject',
/// or 0 if not found. By default, the search ignores case; if case_matters is true, case
/// is significant. If skip is provided (positive integer), that many characters are skipped
/// from the beginning before searching.
fn bf_index(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() < 2 || bf_args.args.len() > 4 {
        return Err(BfErr::Code(E_ARGS));
    }

    let case_matters = if bf_args.args.len() >= 3 {
        bf_args.args[2].is_true()
    } else {
        false
    };

    let skip = if bf_args.args.len() == 4 {
        let skip_val = bf_args.args[3].as_integer().ok_or(BfErr::Code(E_TYPE))?;
        if skip_val < 0 {
            return Err(BfErr::ErrValue(
                E_INVARG.msg("index() skip must be non-negative"),
            ));
        }
        skip_val as usize
    } else {
        0
    };

    let (subject, what) = (bf_args.args[0].variant(), bf_args.args[1].variant());
    match (subject, what) {
        (Variant::Str(subject), Variant::Str(what)) => Ok(Ret(v_int(str_index(
            subject.as_str(),
            what.as_str(),
            case_matters,
            skip,
        )))),
        _ => Err(BfErr::Code(E_TYPE)),
    }
}

/// Usage: `int rindex(str subject, str what [, bool case_matters [, int skip]])`
/// Returns the index of the first character of the last occurrence of 'what' in 'subject',
/// or 0 if not found. By default, the search ignores case; if case_matters is true, case
/// is significant. If skip is provided (negative integer), that many characters are skipped
/// from the end before searching.
fn bf_rindex(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() < 2 || bf_args.args.len() > 4 {
        return Err(BfErr::Code(E_ARGS));
    }

    let case_matters = if bf_args.args.len() >= 3 {
        bf_args.args[2].is_true()
    } else {
        false
    };

    let skip = if bf_args.args.len() == 4 {
        let skip_val = bf_args.args[3].as_integer().ok_or(BfErr::Code(E_TYPE))?;
        if skip_val > 0 {
            return Err(BfErr::ErrValue(
                E_INVARG.msg("rindex() skip must be non-positive"),
            ));
        }
        skip_val
    } else {
        0
    };

    let (subject, what) = (bf_args.args[0].variant(), bf_args.args[1].variant());
    match (subject, what) {
        (Variant::Str(subject), Variant::Str(what)) => Ok(Ret(v_int(str_rindex(
            subject.as_str(),
            what.as_str(),
            case_matters,
            skip,
        )))),
        _ => Err(BfErr::Code(E_TYPE)),
    }
}

/// Usage: `int strcmp(str str1, str str2)`
/// Performs a case-sensitive comparison of two strings. Returns a negative integer if
/// str1 < str2, zero if identical, or a positive integer if str1 > str2. Uses ASCII ordering.
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

/// Usage: `str string_hash(str text)`
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

/// Usage: `str binary_hash(binary data)`
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

/// Usage: `str encode_base64(str|binary data [, bool url_safe] [, bool no_padding])`
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

/// Usage: `binary decode_base64(str encoded_text [, bool url_safe])`
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

/// Usage: `str binary_to_str(binary data [, bool allow_lossy])`
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

/// Usage: `binary binary_from_str(str text)`
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

/// Usage: `list explode(str subject [, str break [, bool include_sequential]])`
/// Returns a list of substrings of subject separated by break. Only the first character
/// of break is used; it defaults to space. By default, empty strings from consecutive
/// separators are omitted; if include_sequential is true, they are included.
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

/// Internal helper for character translation.
fn strtr(source: &str, str1: &str, str2: &str, case_matters: bool) -> String {
    let from_chars: Vec<char> = str1.chars().collect();
    let to_chars: Vec<char> = str2.chars().collect();

    let mut result = String::with_capacity(source.len());

    for c in source.chars() {
        let pos = if case_matters {
            from_chars.iter().position(|&fc| fc == c)
        } else {
            from_chars
                .iter()
                .position(|&fc| fc.to_lowercase().eq(c.to_lowercase()))
        };

        match pos {
            Some(i) if i < to_chars.len() => {
                // Map to corresponding character in str2
                let replacement = to_chars[i];
                if case_matters || !c.is_alphabetic() {
                    // Case-sensitive mode or non-letter: use replacement as-is
                    result.push(replacement);
                } else {
                    // Case-insensitive with letter: preserve original case
                    if c.is_uppercase() {
                        for uc in replacement.to_uppercase() {
                            result.push(uc);
                        }
                    } else {
                        for lc in replacement.to_lowercase() {
                            result.push(lc);
                        }
                    }
                }
            }
            Some(_) => {
                // Character found in str1 but no corresponding char in str2 - delete it
            }
            None => {
                // Character not in str1 - keep it unchanged
                result.push(c);
            }
        }
    }

    result
}

/// Usage: `str strtr(str source, str str1, str str2 [, bool case_matters])`
/// Translates characters in source by mapping each character in str1 to the corresponding
/// character in str2. If str2 is shorter than str1, characters that map beyond str2's length
/// are deleted. By default the search is case-insensitive (case_matters = false).
fn bf_strtr(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    let case_matters = if bf_args.args.len() == 3 {
        false
    } else if bf_args.args.len() == 4 {
        bf_args.args[3].is_true()
    } else {
        return Err(BfErr::Code(E_ARGS));
    };

    let (source, str1, str2) = (
        bf_args.args[0].variant(),
        bf_args.args[1].variant(),
        bf_args.args[2].variant(),
    );

    match (source, str1, str2) {
        (Variant::Str(source), Variant::Str(str1), Variant::Str(str2)) => Ok(Ret(v_str(
            strtr(source.as_str(), str1.as_str(), str2.as_str(), case_matters).as_str(),
        ))),
        _ => Err(BfErr::Code(E_TYPE)),
    }
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
    builtins[offset_for_builtin("strtr")] = bf_strtr;
}

#[cfg(test)]
mod tests {
    use crate::vm::builtins::bf_strings::{str_index, str_rindex, strsub, strtr};

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
    fn test_strtr_simple_replace() {
        // strtr("foobar", "o", "i") => "fiibar"
        assert_eq!(strtr("foobar", "o", "i", true), "fiibar");
    }

    #[test]
    fn test_strtr_swap_chars() {
        // strtr("foobar", "ob", "bo") => "fbboar"
        assert_eq!(strtr("foobar", "ob", "bo", true), "fbboar");
    }

    #[test]
    fn test_strtr_delete_chars() {
        // strtr("foobar", "foba", "") => "r"
        assert_eq!(strtr("foobar", "foba", "", true), "r");
    }

    #[test]
    fn test_strtr_case_insensitive() {
        // strtr("5xX", "135x", "0aBB", 0) => "BbB"
        // case-insensitive: x and X both match "x", but output preserves case
        assert_eq!(strtr("5xX", "135x", "0aBB", false), "BbB");
    }

    #[test]
    fn test_strtr_case_sensitive() {
        // strtr("5xX", "135x", "0aBB", 1) => "BBX"
        // case-sensitive: only lowercase x matches, X unchanged
        assert_eq!(strtr("5xX", "135x", "0aBB", true), "BBX");
    }

    #[test]
    fn test_strtr_empty_source() {
        assert_eq!(strtr("", "abc", "xyz", true), "");
    }

    #[test]
    fn test_strtr_empty_str1() {
        assert_eq!(strtr("hello", "", "xyz", true), "hello");
    }

    #[test]
    fn test_strtr_utf8() {
        // UTF-8 character handling
        assert_eq!(strtr("héllo", "é", "e", true), "hello");
        assert_eq!(strtr("日本語", "日", "月", true), "月本語");
    }

    // Tests from the book documentation
    #[test]
    fn test_index_basic() {
        // index("foobar", "o") => 2
        assert_eq!(str_index("foobar", "o", false, 0), 2);
    }

    #[test]
    fn test_index_with_skip() {
        // index("foobar", "o", 0, 0) => 2
        assert_eq!(str_index("foobar", "o", false, 0), 2);
        // index("foobar", "o", 0, 2) => 3 (skip first 2 chars "fo", search in "obar", find "o" at position 3)
        assert_eq!(str_index("foobar", "o", false, 2), 3);
    }

    #[test]
    fn test_index_not_found() {
        // index("foobar", "x") => 0
        assert_eq!(str_index("foobar", "x", false, 0), 0);
    }

    #[test]
    fn test_index_case_sensitive() {
        // index("Foobar", "foo", 1) => 0 (case sensitive, "Foo" != "foo")
        assert_eq!(str_index("Foobar", "foo", true, 0), 0);
        // But case insensitive should find it
        assert_eq!(str_index("Foobar", "foo", false, 0), 1);
    }

    #[test]
    fn test_rindex_basic() {
        // rindex("foobar", "o") => 3
        assert_eq!(str_rindex("foobar", "o", false, 0), 3);
    }

    #[test]
    fn test_rindex_with_skip() {
        // rindex("foobar", "o", 0, 0) => 3
        assert_eq!(str_rindex("foobar", "o", false, 0), 3);
        // rindex("foobar", "o", 0, -4) => 2 (skip last 4 chars, search in "fo", find last "o" at position 2)
        assert_eq!(str_rindex("foobar", "o", false, -4), 2);
    }
}
