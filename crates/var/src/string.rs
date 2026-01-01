// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

use arcstr::ArcStr;
use num_traits::ToPrimitive;
use std::{
    cmp::max,
    fmt::{Display, Formatter},
    hash::Hash,
};

use crate::{
    Error, Sequence,
    error::ErrorCode::{E_INVARG, E_RANGE, E_TYPE},
    variant::Var,
};

#[derive(Clone, Debug)]
#[repr(transparent)]
pub struct Str(ArcStr);

impl Str {
    pub fn mk_str(s: &str) -> Self {
        Str(ArcStr::from(s))
    }

    pub fn mk_string(s: String) -> Self {
        Str(ArcStr::from(s))
    }

    pub fn mk_arc_str(s: ArcStr) -> Self {
        Str(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn as_arc_str(&self) -> ArcStr {
        self.0.clone()
    }

    pub fn str_append(&self, other: &Self) -> Var {
        let mut s = String::from(&*self.0);
        s.push_str(other.as_str());
        Var::from_str_type(Str(ArcStr::from(s)))
    }
}

// ============================================================================
// Case-insensitive string operations with ASCII fast path
// ============================================================================

/// Streaming comparison against pre-lowercased needle slice.
/// The needle has already been lowercased once; we only lowercase subject chars on the fly.
#[inline]
fn matches_ci_precomputed(subject: &str, start_byte: usize, needle_lower: &[char]) -> bool {
    let mut needle_iter = needle_lower.iter();
    let mut subject_lower = subject[start_byte..].chars().flat_map(|c| c.to_lowercase());

    loop {
        match needle_iter.next() {
            None => return true,
            Some(&needle_ch) => match subject_lower.next() {
                Some(subject_ch) if subject_ch == needle_ch => continue,
                _ => return false,
            },
        }
    }
}

/// Match against pre-lowercased needle, returns byte length consumed in subject.
#[inline]
#[allow(unused_assignments)] // subject_lower = None IS read by the if-let on next iteration
fn match_len_ci_precomputed(
    subject: &str,
    start_byte: usize,
    needle_lower: &[char],
) -> Option<usize> {
    let subject_slice = &subject[start_byte..];
    let mut subject_chars = subject_slice.char_indices();
    let mut subject_lower: Option<std::char::ToLowercase> = None;
    let mut last_byte_end = 0;

    for &needle_ch in needle_lower {
        let subject_ch = loop {
            if let Some(ref mut iter) = subject_lower {
                if let Some(ch) = iter.next() {
                    break ch;
                }
                subject_lower = None;
            }
            let (offset, ch) = subject_chars.next()?;
            last_byte_end = offset + ch.len_utf8();
            subject_lower = Some(ch.to_lowercase());
        };

        if subject_ch != needle_ch {
            return None;
        }
    }

    Some(last_byte_end)
}

/// ASCII case-insensitive find - no byte scanning when known_ascii=true.
pub(crate) fn ascii_find_ci(
    subject: &str,
    needle: &str,
    skip_chars: usize,
    known_ascii: bool,
) -> Result<Option<usize>, ()> {
    let subject_bytes = subject.as_bytes();
    let needle_bytes = needle.as_bytes();

    if !known_ascii {
        for &b in needle_bytes {
            if b > 127 {
                return Err(());
            }
        }
        for &b in subject_bytes {
            if b > 127 {
                return Err(());
            }
        }
    }

    if needle_bytes.is_empty() {
        return Ok(Some(skip_chars.min(subject_bytes.len())));
    }

    if skip_chars >= subject_bytes.len() {
        return Ok(None);
    }

    let search_slice = &subject_bytes[skip_chars..];
    if search_slice.len() < needle_bytes.len() {
        return Ok(None);
    }
    for i in 0..=search_slice.len() - needle_bytes.len() {
        if search_slice[i..i + needle_bytes.len()].eq_ignore_ascii_case(needle_bytes) {
            return Ok(Some(skip_chars + i));
        }
    }

    Ok(None)
}

/// ASCII case-insensitive rfind - no byte scanning when known_ascii=true.
pub(crate) fn ascii_rfind_ci(
    subject: &str,
    needle: &str,
    skip_from_end: usize,
    known_ascii: bool,
) -> Result<Option<usize>, ()> {
    let subject_bytes = subject.as_bytes();
    let needle_bytes = needle.as_bytes();

    if !known_ascii {
        for &b in needle_bytes {
            if b > 127 {
                return Err(());
            }
        }
        for &b in subject_bytes {
            if b > 127 {
                return Err(());
            }
        }
    }

    if needle_bytes.is_empty() {
        return Ok(Some(subject_bytes.len().saturating_sub(skip_from_end)));
    }

    let search_end = subject_bytes.len().saturating_sub(skip_from_end);
    if search_end < needle_bytes.len() {
        return Ok(None);
    }

    for i in (0..=search_end - needle_bytes.len()).rev() {
        if subject_bytes[i..i + needle_bytes.len()].eq_ignore_ascii_case(needle_bytes) {
            return Ok(Some(i));
        }
    }

    Ok(None)
}

/// ASCII case-insensitive strsub - no byte scanning when known_ascii=true.
pub(crate) fn ascii_strsub_ci(
    subject: &str,
    what: &str,
    with: &str,
    known_ascii: bool,
) -> Result<String, ()> {
    let subject_bytes = subject.as_bytes();
    let what_bytes = what.as_bytes();

    if !known_ascii {
        for &b in what_bytes {
            if b > 127 {
                return Err(());
            }
        }
        for &b in subject_bytes {
            if b > 127 {
                return Err(());
            }
        }
    }

    if what_bytes.is_empty() || subject_bytes.len() < what_bytes.len() {
        return Ok(subject.to_string());
    }

    let mut result = String::with_capacity(subject.len());
    let mut i = 0;

    while i + what_bytes.len() <= subject_bytes.len() {
        if subject_bytes[i..i + what_bytes.len()].eq_ignore_ascii_case(what_bytes) {
            result.push_str(with);
            i += what_bytes.len();
        } else {
            result.push(subject_bytes[i] as char);
            i += 1;
        }
    }

    for &b in &subject_bytes[i..] {
        result.push(b as char);
    }

    Ok(result)
}

fn needle_first_lower(needle: &str) -> Option<char> {
    needle.chars().next().and_then(|c| c.to_lowercase().next())
}

/// Unicode case-insensitive find - streaming match with a first-char filter.
pub(crate) fn unicode_find_ci(subject: &str, needle: &str, skip_chars: usize) -> Option<usize> {
    if needle.is_empty() {
        return Some(skip_chars.min(subject.chars().count()));
    }

    let Some(first_needle_char) = needle_first_lower(needle) else {
        return Some(skip_chars.min(subject.chars().count()));
    };
    let needle_lower: Vec<char> = needle.chars().flat_map(|c| c.to_lowercase()).collect();

    for (char_idx, (byte_pos, subject_char)) in subject.char_indices().enumerate() {
        if char_idx < skip_chars {
            continue;
        }

        // First-char filter: skip positions where first char can't match
        let first_subject_lower = subject_char.to_lowercase().next().unwrap_or('\0');
        if first_subject_lower != first_needle_char {
            continue;
        }

        // Full match against pre-lowercased needle
        if matches_ci_precomputed(subject, byte_pos, &needle_lower) {
            return Some(char_idx);
        }
    }

    None
}

/// Unicode case-insensitive rfind - streaming match with a first-char filter.
pub(crate) fn unicode_rfind_ci(subject: &str, needle: &str, skip_from_end: usize) -> Option<usize> {
    let total_chars = subject.chars().count();
    let search_chars = total_chars.saturating_sub(skip_from_end);

    if needle.is_empty() {
        return Some(search_chars);
    }

    let Some(first_needle_char) = needle_first_lower(needle) else {
        return Some(search_chars);
    };
    let needle_lower: Vec<char> = needle.chars().flat_map(|c| c.to_lowercase()).collect();

    let mut last_match: Option<usize> = None;

    for (char_idx, (byte_pos, subject_char)) in subject.char_indices().enumerate() {
        if char_idx >= search_chars {
            break;
        }

        // First-char filter
        let first_subject_lower = subject_char.to_lowercase().next().unwrap_or('\0');
        if first_subject_lower != first_needle_char {
            continue;
        }

        if matches_ci_precomputed(subject, byte_pos, &needle_lower) {
            last_match = Some(char_idx);
        }
    }

    last_match
}

/// Unicode case-insensitive strsub - streaming match with a first-char filter.
pub(crate) fn unicode_strsub_ci(subject: &str, what: &str, with: &str) -> String {
    if what.is_empty() {
        return subject.to_string();
    }

    let Some(first_what_char) = needle_first_lower(what) else {
        return subject.to_string();
    };
    let what_lower: Vec<char> = what.chars().flat_map(|c| c.to_lowercase()).collect();

    let mut result = String::with_capacity(subject.len());
    let mut cursor = 0;

    while cursor < subject.len() {
        let ch = subject[cursor..].chars().next().unwrap();

        // First-char filter + full match
        let first_subject_lower = ch.to_lowercase().next().unwrap_or('\0');
        if first_subject_lower == first_what_char
            && let Some(match_len) = match_len_ci_precomputed(subject, cursor, &what_lower)
        {
            result.push_str(with);
            cursor += match_len;
            continue;
        }

        result.push(ch);
        cursor += ch.len_utf8();
    }

    result
}

// ============================================================================
// High-level string operations (used by Var methods)
// ============================================================================

/// Find first occurrence of needle in subject. Returns 0-based char index or None.
pub(crate) fn str_find(
    subject: &str,
    needle: &str,
    case_matters: bool,
    skip: usize,
    is_ascii: bool,
) -> Option<usize> {
    if case_matters {
        let search_start: usize = subject
            .char_indices()
            .nth(skip)
            .map(|(i, _)| i)
            .unwrap_or(subject.len());

        if search_start >= subject.len() {
            return if needle.is_empty() {
                Some(skip.min(subject.chars().count()))
            } else {
                None
            };
        }

        return subject[search_start..].find(needle).map(|byte_offset| {
            let byte_pos = search_start + byte_offset;
            subject[..byte_pos].chars().count()
        });
    }

    // Case-insensitive: try ASCII fast path
    if let Ok(result) = ascii_find_ci(subject, needle, skip, is_ascii) {
        return result;
    }

    // Fall back to Unicode streaming
    unicode_find_ci(subject, needle, skip)
}

/// Find last occurrence of needle in subject. Returns 0-based char index or None.
pub(crate) fn str_rfind(
    subject: &str,
    needle: &str,
    case_matters: bool,
    skip_from_end: usize,
    is_ascii: bool,
) -> Option<usize> {
    // For ASCII strings, char count equals byte count - avoid O(n) scan
    let total_chars = if is_ascii {
        subject.len()
    } else {
        subject.chars().count()
    };
    let search_chars = total_chars.saturating_sub(skip_from_end);

    if case_matters {
        // For ASCII, byte offset equals char offset
        let search_end: usize = if is_ascii {
            search_chars
        } else {
            subject
                .char_indices()
                .nth(search_chars)
                .map(|(i, _)| i)
                .unwrap_or(subject.len())
        };

        if needle.is_empty() {
            return Some(search_chars);
        }

        return subject[..search_end].rfind(needle).map(|byte_offset| {
            if is_ascii {
                byte_offset
            } else {
                subject[..byte_offset].chars().count()
            }
        });
    }

    // Case-insensitive: try ASCII fast path
    if let Ok(result) = ascii_rfind_ci(subject, needle, skip_from_end, is_ascii) {
        return result;
    }

    // Fall back to Unicode streaming
    unicode_rfind_ci(subject, needle, skip_from_end)
}

/// Replace all occurrences of what with with in subject.
pub(crate) fn str_replace(
    subject: &str,
    what: &str,
    with: &str,
    case_matters: bool,
    is_ascii: bool,
) -> String {
    if what.is_empty() {
        return subject.to_string();
    }

    if case_matters {
        return subject.replace(what, with);
    }

    // Case-insensitive: try ASCII fast path
    if let Ok(result) = ascii_strsub_ci(subject, what, with, is_ascii) {
        return result;
    }

    // Fall back to Unicode streaming
    unicode_strsub_ci(subject, what, with)
}

impl Sequence for Str {
    fn is_empty(&self) -> bool {
        self.as_str().is_empty()
    }

    fn len(&self) -> usize {
        self.as_str().chars().count()
    }

    fn index_in(&self, value: &Var, case_sensitive: bool) -> Result<Option<usize>, Error> {
        let Some(needle_str) = value.as_str() else {
            return Err(E_TYPE.with_msg(|| {
                format!(
                    "Cannot index string with {}",
                    value.type_code().to_literal()
                )
            }));
        };

        // Compute ASCII flag on the fly (enables ASCII fast path for case-insensitive)
        let is_ascii = self.as_str().is_ascii() && needle_str.as_str().is_ascii();
        Ok(str_find(
            self.as_str(),
            needle_str.as_str(),
            case_sensitive,
            0,
            is_ascii,
        ))
    }

    fn contains(&self, value: &Var, case_sensitive: bool) -> Result<bool, Error> {
        let Some(needle_str) = value.as_str() else {
            return Err(E_TYPE.with_msg(|| {
                format!(
                    "Cannot check if string contains {}",
                    value.type_code().to_literal()
                )
            }));
        };

        // Compute ASCII flag on the fly (enables ASCII fast path for case-insensitive)
        let is_ascii = self.as_str().is_ascii() && needle_str.as_str().is_ascii();
        Ok(str_find(
            self.as_str(),
            needle_str.as_str(),
            case_sensitive,
            0,
            is_ascii,
        )
        .is_some())
    }

    fn index(&self, index: usize) -> Result<Var, Error> {
        if index >= self.len() {
            return Err(E_RANGE.with_msg(|| {
                format!(
                    "Index {} out of range for string of length {}",
                    index,
                    self.len()
                )
            }));
        }
        let c = self.as_str().chars().nth(index).unwrap();
        let c_str = c.to_string();
        Ok(Var::from_str_type(Str(ArcStr::from(c_str))))
    }

    fn index_set(&self, index: usize, value: &Var) -> Result<Var, Error> {
        if index >= self.len() {
            return Err(E_RANGE.with_msg(|| {
                format!(
                    "Index {} out of range for string of length {}",
                    index,
                    self.len()
                )
            }));
        }

        // Index set for strings requires that the `value` being set is a string, otherwise it's.
        // E_TYPE.
        // And it must be a single character character, otherwise, E_INVARG is returned.
        let Some(value) = value.as_str() else {
            return Err(E_TYPE.with_msg(|| {
                format!(
                    "Cannot set string index {} with {}",
                    index,
                    value.type_code().to_literal()
                )
            }));
        };

        if value.as_str().chars().count() != 1 {
            return Err(E_INVARG.msg("String index set value must be a single character"));
        }

        // Convert character index to byte indices
        let mut chars = self.as_str().char_indices();
        let (start_byte, _) = chars.nth(index).unwrap();
        let end_byte = chars.next().map(|(i, _)| i).unwrap_or(self.as_str().len());

        let mut s = self.as_str().to_string();
        s.replace_range(start_byte..end_byte, value.as_str());
        Ok(Var::from_str_type(Str(ArcStr::from(s))))
    }

    fn push(&self, value: &Var) -> Result<Var, Error> {
        let Some(value) = value.as_str() else {
            return Err(E_TYPE
                .with_msg(|| format!("Cannot push {} to string", value.type_code().to_literal())));
        };

        let mut new_copy = self.as_str().to_string();
        new_copy.push_str(value.as_str());
        Ok(Var::from_str_type(Str(ArcStr::from(new_copy))))
    }

    fn insert(&self, index: usize, value: &Var) -> Result<Var, Error> {
        // If value is not a string, return E_TYPE.
        let Some(value) = value.as_str() else {
            return Err(E_TYPE.with_msg(|| {
                format!(
                    "Cannot insert {} into string",
                    value.type_code().to_literal()
                )
            }));
        };

        // Convert character index to byte index
        let byte_index = if index == 0 {
            0
        } else if index >= self.len() {
            self.as_str().len()
        } else {
            self.as_str()
                .char_indices()
                .nth(index)
                .map(|(i, _)| i)
                .unwrap_or(self.as_str().len())
        };

        let mut new_copy = self.as_str().to_string();
        new_copy.insert_str(byte_index, value.as_str());
        Ok(Var::from_str_type(Str(ArcStr::from(new_copy))))
    }

    fn range(&self, from: isize, to: isize) -> Result<Var, Error> {
        if to < from {
            return Ok(Var::mk_str(""));
        }
        let s = self.as_str();
        let char_len = self.len();
        let start = max(from, 0) as usize;
        let to = to as usize;
        if start >= char_len || to >= char_len {
            return Err(E_RANGE.with_msg(|| {
                format!("Range {from}..{to} out of bounds for string of length {char_len}")
            }));
        }

        // Extract characters directly using iterator
        let result: String = s.chars().skip(start).take(to - start + 1).collect();
        Ok(Var::mk_str(&result))
    }

    fn range_set(&self, from: isize, to: isize, with: &Var) -> Result<Var, Error> {
        let Some(with_val) = with.as_str() else {
            return Err(E_TYPE.with_msg(|| {
                format!(
                    "Cannot set string range with {}",
                    with.type_code().to_literal()
                )
            }));
        };

        let base_str = self.as_str();
        let from = max(from, 0) as usize;

        let mut result_str = if from > 0 {
            base_str[..from].to_string()
        } else {
            "".to_string()
        };
        result_str.push_str(with_val.as_str());

        match to.to_usize() {
            Some(to) => {
                result_str.push_str(&base_str[to + 1..]);
            }
            None => {
                result_str.push_str(base_str);
            }
        }

        Ok(Var::from_str_type(Str(ArcStr::from(result_str))))
    }

    fn append(&self, other: &Var) -> Result<Var, Error> {
        let Some(other) = other.as_str() else {
            return Err(E_TYPE.with_msg(|| {
                format!("Cannot append {} to string", other.type_code().to_literal())
            }));
        };

        let mut new_copy = self.as_str().to_string();
        new_copy.push_str(other.as_str());
        Ok(Var::from_str_type(Str(ArcStr::from(new_copy))))
    }

    fn remove_at(&self, index: usize) -> Result<Var, Error> {
        if index >= self.len() {
            return Err(E_RANGE.with_msg(|| {
                format!(
                    "Index {} out of range for string of length {}",
                    index,
                    self.len()
                )
            }));
        }

        // Convert character index to byte indices
        let mut chars = self.as_str().char_indices();
        let (start_byte, _) = chars.nth(index).unwrap();
        let end_byte = chars.next().map(|(i, _)| i).unwrap_or(self.as_str().len());

        let mut new_copy = self.as_str().to_string();
        new_copy.replace_range(start_byte..end_byte, "");
        Ok(Var::from_str_type(Str(ArcStr::from(new_copy))))
    }
}

impl Display for Str {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
impl PartialEq for Str {
    // MOO strings are case-insensitive on comparison unless an explicit case sensitive comparison
    // is needed.
    fn eq(&self, other: &Self) -> bool {
        self.as_str().to_lowercase() == other.as_str().to_lowercase()
    }
}

impl Eq for Str {}

impl PartialOrd for Str {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Str {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str()
            .to_lowercase()
            .cmp(&other.as_str().to_lowercase())
    }
}

impl Hash for Str {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_str().to_lowercase().hash(state)
    }
}

impl From<&str> for Str {
    fn from(s: &str) -> Self {
        Str::mk_str(s)
    }
}

impl From<String> for Str {
    fn from(s: String) -> Self {
        Str::mk_string(s)
    }
}

#[cfg(test)]
mod tests {
    use super::Str;
    use crate::{
        IndexMode, Sequence,
        error::ErrorCode::E_RANGE,
        v_bool_int,
        variant::Variant,
        variant::{Var, v_int, v_str},
    };

    #[test]
    fn test_str_pack_unpack() {
        let s = Var::mk_str("hello");
        match s.variant() {
            Variant::Str(s) => assert_eq!(s.as_str(), "hello"),
            _ => panic!("Expected string"),
        }
    }

    #[test]
    fn test_string_is_funcs() {
        let s = Var::mk_str("hello");
        assert!(s.is_true());
        assert!(s.is_sequence());
        assert!(!s.is_associative());
        assert!(!s.is_scalar());
        assert_eq!(s.len().unwrap(), 5);
        assert!(!s.is_empty().unwrap());

        let s = Var::mk_str("");
        assert!(!s.is_true());
        assert!(s.is_sequence());
        assert!(!s.is_associative());
        assert!(!s.is_scalar());
        assert_eq!(s.len().unwrap(), 0);
        assert!(s.is_empty().unwrap());
    }

    #[test]
    fn test_string_cached_len_utf8() {
        let s = Var::mk_str("aé😊");
        assert_eq!(s.len().unwrap(), 3);
    }

    #[test]
    fn test_index_in_case_insensitive_utf8_expansion() {
        let s = Str::mk_str("AİB");
        let idx = s.index_in(&Var::mk_str("b"), false).unwrap();
        assert_eq!(idx, Some(2));
    }

    #[test]
    fn test_string_equality_inquality() {
        let s1 = Var::mk_str("hello");
        let s2 = Var::mk_str("hello");
        let s3 = Var::mk_str("world");
        let s4 = Var::mk_str("hello world");

        assert_eq!(s1, s2);
        assert_ne!(s1, s3);
        assert_ne!(s1, s4);
    }

    #[test]
    fn test_string_index() {
        let s = Var::mk_str("hello");
        let r = s.index(&Var::mk_integer(1), IndexMode::ZeroBased).unwrap();
        let r = r.variant();
        match r {
            Variant::Str(s) => assert_eq!(s.as_str(), "e"),
            _ => panic!("Expected string, got {r:?}"),
        }
    }

    #[test]
    fn test_string_index_set() {
        let s = Var::mk_str("hello");
        let r = s
            .index_set(&Var::mk_integer(1), &Var::mk_str("a"), IndexMode::ZeroBased)
            .unwrap();
        let r = r.variant();
        match r {
            Variant::Str(s) => assert_eq!(s.as_str(), "hallo"),
            _ => panic!("Expected string, got {r:?}"),
        }

        let fail_bad_index = s.index_set(
            &Var::mk_integer(10),
            &Var::mk_str("a"),
            IndexMode::ZeroBased,
        );
        assert!(fail_bad_index.is_err());
        assert_eq!(fail_bad_index.unwrap_err(), E_RANGE);
    }

    #[test]
    fn test_one_index_slice() {
        let s = v_str("hello world");
        let result = s.range(&v_int(2), &v_int(7), IndexMode::OneBased).unwrap();
        assert_eq!(result, v_str("ello w"));
    }

    #[test]
    fn test_zero_index_slice() {
        let s = v_str("hello world");
        let result = s.range(&v_int(1), &v_int(6), IndexMode::ZeroBased).unwrap();
        assert_eq!(result, v_str("ello w"));
    }

    #[test]
    fn test_string_range_set() {
        // Test a one-indexed assignment, comparing against a known MOO behaviour.
        let base = v_str("mandalorian");
        let (start, end) = (v_int(4), v_int(7));
        let replace = v_str("bozo");
        let expected = v_str("manbozorian");
        let result = base.range_set(&start, &end, &replace, IndexMode::OneBased);
        assert_eq!(result, Ok(expected));

        // Test interior insertion
        let base = v_str("12345");
        let value = v_str("abc");
        let expected = v_str("1abc45");
        let result = base.range_set(&v_int(2), &v_int(3), &value, IndexMode::OneBased);
        assert_eq!(result, Ok(expected));

        // Test interior replacement
        let base = v_str("12345");
        let value = v_str("ab");
        let expected = v_str("1ab45");
        let result = base.range_set(&v_int(1), &v_int(2), &value, IndexMode::ZeroBased);
        assert_eq!(result, Ok(expected));

        // Test interior deletion
        let base = v_str("12345");
        let value = v_str("");
        let expected = v_str("145");
        let result = base.range_set(&v_int(1), &v_int(2), &value, IndexMode::ZeroBased);
        assert_eq!(result, Ok(expected));

        // Test interior subtraction
        let base = v_str("12345");
        let value = v_str("z");
        let expected = v_str("1z45");
        let result = base.range_set(&v_int(1), &v_int(2), &value, IndexMode::ZeroBased);
        assert_eq!(result, Ok(expected));
    }

    /// Moo supports this weird behavior
    #[test]
    fn test_string_range_set_odd_range_end() {
        let base = v_str("me:words");
        let value = v_str("");
        let expected = v_str("me:words");
        let result = base.range_set(&v_int(1), &v_int(0), &value, IndexMode::OneBased);
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn test_string_push() {
        let s = Var::mk_str("hello");
        let r = s.push(&Var::mk_str(" world")).unwrap();
        let r = r.variant();
        match r {
            Variant::Str(s) => assert_eq!(s.as_str(), "hello world"),
            _ => panic!("Expected string, got {r:?}"),
        }
    }

    #[test]
    fn test_string_append() {
        let s1 = Var::mk_str("hello");
        let s2 = Var::mk_str(" world");
        let r = s1.append(&s2).unwrap();
        let r = r.variant();
        match r {
            Variant::Str(s) => assert_eq!(s.as_str(), "hello world"),
            _ => panic!("Expected string, got {r:?}"),
        }
    }

    #[test]
    fn test_string_remove_at() {
        let s = Var::mk_str("hello");
        let r = s
            .remove_at(&Var::mk_integer(1), IndexMode::ZeroBased)
            .unwrap();
        let r = r.variant();
        match r {
            Variant::Str(s) => assert_eq!(s.as_str(), "hllo"),
            _ => panic!("Expected string, got {r:?}"),
        }

        let fail_bad_index = s.remove_at(&Var::mk_integer(10), IndexMode::ZeroBased);
        assert!(fail_bad_index.is_err());
        assert_eq!(fail_bad_index.unwrap_err(), E_RANGE);
    }

    #[test]
    fn test_string_contains() {
        // Check both case-sensitive and case-insensitive
        let s = Var::mk_str("hello");
        assert_eq!(
            s.contains(&Var::mk_str("ell"), true).unwrap(),
            v_bool_int(true)
        );
        assert_eq!(
            s.contains(&Var::mk_str("Ell"), false).unwrap(),
            v_bool_int(true)
        );
        assert_eq!(
            s.contains(&Var::mk_str("world"), true).unwrap(),
            v_bool_int(false)
        );
    }

    #[test]
    fn test_string_case_sensitive() {
        let s = Var::mk_str("hello");
        let s2 = Var::mk_str("Hello");
        assert_eq!(s, s2);
        assert!(!s.eq_case_sensitive(&s2));
    }

    #[test]
    fn test_range_assignment_regression() {
        let base = v_str("testing\"");
        let value = v_str("");
        let expected = v_str("esting\"");

        let result = base
            .range_set(&v_int(1), &v_int(1), &value, IndexMode::OneBased)
            .unwrap();

        assert_eq!(result, expected);
    }

    // ============================================================================
    // Comprehensive tests for str_find, str_rfind, str_replace
    // ============================================================================

    mod string_search_tests {
        use crate::variant::Var;

        // Helper to test str_find (returns 0-based index or None)
        fn find(subject: &str, needle: &str, case_matters: bool, skip: usize) -> Option<usize> {
            let s = Var::mk_str(subject);
            let n = Var::mk_str(needle);
            s.str_find(&n, case_matters, skip)
        }

        // Helper to test str_rfind (returns 0-based index or None)
        fn rfind(
            subject: &str,
            needle: &str,
            case_matters: bool,
            skip_from_end: usize,
        ) -> Option<usize> {
            let s = Var::mk_str(subject);
            let n = Var::mk_str(needle);
            s.str_rfind(&n, case_matters, skip_from_end)
        }

        // Helper to test str_replace
        fn replace(subject: &str, what: &str, with: &str, case_matters: bool) -> String {
            let s = Var::mk_str(subject);
            let w = Var::mk_str(what);
            let r = Var::mk_str(with);
            s.str_replace(&w, &r, case_matters)
                .and_then(|v| v.as_str().map(|s| s.as_str().to_string()))
                .unwrap_or_default()
        }

        // ===== str_find tests =====

        #[test]
        fn test_find_basic_ascii() {
            assert_eq!(find("hello world", "world", true, 0), Some(6));
            assert_eq!(find("hello world", "hello", true, 0), Some(0));
            assert_eq!(find("hello world", "o", true, 0), Some(4));
        }

        #[test]
        fn test_find_case_insensitive_ascii() {
            assert_eq!(find("Hello World", "world", false, 0), Some(6));
            assert_eq!(find("HELLO WORLD", "hello", false, 0), Some(0));
            assert_eq!(find("FoObAr", "foobar", false, 0), Some(0));
        }

        #[test]
        fn test_find_with_skip() {
            assert_eq!(find("abcabc", "abc", true, 0), Some(0));
            assert_eq!(find("abcabc", "abc", true, 1), Some(3));
            assert_eq!(find("abcabc", "abc", true, 4), None);
        }

        #[test]
        fn test_find_not_found() {
            assert_eq!(find("hello", "xyz", true, 0), None);
            assert_eq!(find("hello", "HELLO", true, 0), None); // case sensitive
        }

        #[test]
        fn test_find_empty_needle() {
            assert_eq!(find("hello", "", true, 0), Some(0));
            assert_eq!(find("hello", "", true, 3), Some(3));
            assert_eq!(find("hello", "", true, 10), Some(5)); // clamped to length
        }

        #[test]
        fn test_find_empty_subject() {
            assert_eq!(find("", "hello", true, 0), None);
            assert_eq!(find("", "", true, 0), Some(0));
        }

        #[test]
        fn test_find_unicode_case_fold() {
            // İ (U+0130, Latin Capital Letter I With Dot Above) lowercases to 'i' + '\u{307}'
            // Searching for 'b' in "AİB" should find it at position 2 (0-based)
            assert_eq!(find("AİB", "b", false, 0), Some(2));

            // Searching for 'i' should match İ case-insensitively
            assert_eq!(find("AİB", "i", false, 0), Some(1));
        }

        #[test]
        fn test_find_unicode_subject_ascii_needle() {
            // Subject has UTF-8 but needle is ASCII
            assert_eq!(find("héllo wörld", "wor", false, 0), None); // 'ö' != 'o'
            assert_eq!(find("héllo world", "world", true, 0), Some(6));
        }

        // ===== str_rfind tests =====

        #[test]
        fn test_rfind_basic_ascii() {
            assert_eq!(rfind("hello world world", "world", true, 0), Some(12));
            assert_eq!(rfind("hello world", "hello", true, 0), Some(0));
            assert_eq!(rfind("hello", "o", true, 0), Some(4));
        }

        #[test]
        fn test_rfind_case_insensitive_ascii() {
            assert_eq!(rfind("Hello WORLD world", "world", false, 0), Some(12));
            assert_eq!(rfind("FOOBAR foobar", "foobar", false, 0), Some(7));
        }

        #[test]
        fn test_rfind_with_skip() {
            assert_eq!(rfind("abcabc", "abc", true, 0), Some(3)); // finds second "abc"
            assert_eq!(rfind("abcabc", "abc", true, 3), Some(0)); // skip last 3 chars, finds first "abc"
            assert_eq!(rfind("abcabc", "abc", true, 4), None); // skip last 4 chars, only "ab" left - no match
        }

        #[test]
        fn test_rfind_not_found() {
            assert_eq!(rfind("hello", "xyz", true, 0), None);
            assert_eq!(rfind("hello", "HELLO", true, 0), None); // case sensitive
        }

        #[test]
        fn test_rfind_empty_needle() {
            assert_eq!(rfind("hello", "", true, 0), Some(5)); // returns length
            assert_eq!(rfind("hello", "", true, 2), Some(3)); // length - skip
        }

        #[test]
        fn test_rfind_unicode_case_fold() {
            // İ lowercases to 'i' + combining character
            assert_eq!(rfind("AİB", "b", false, 0), Some(2));
            assert_eq!(rfind("BİA", "i", false, 0), Some(1));
        }

        // ===== str_replace tests =====

        #[test]
        fn test_replace_basic_ascii() {
            assert_eq!(
                replace("hello world", "world", "there", true),
                "hello there"
            );
            assert_eq!(replace("foo foo foo", "foo", "bar", true), "bar bar bar");
        }

        #[test]
        fn test_replace_case_insensitive_ascii() {
            assert_eq!(
                replace("Hello World", "world", "there", false),
                "Hello there"
            );
            assert_eq!(replace("FOO foo FoO", "foo", "bar", false), "bar bar bar");
        }

        #[test]
        fn test_replace_no_match() {
            assert_eq!(replace("hello", "xyz", "abc", true), "hello");
            assert_eq!(replace("hello", "HELLO", "hi", true), "hello"); // case sensitive
        }

        #[test]
        fn test_replace_empty_what() {
            assert_eq!(replace("hello", "", "x", true), "hello");
        }

        #[test]
        fn test_replace_empty_subject() {
            assert_eq!(replace("", "foo", "bar", true), "");
        }

        #[test]
        fn test_replace_empty_with() {
            assert_eq!(replace("hello world", "world", "", true), "hello ");
            assert_eq!(replace("foofoofoo", "foo", "", true), "");
        }

        #[test]
        fn test_replace_unicode_case_fold() {
            // İ (U+0130) lowercases to 'i' + '\u{307}'
            // Case-insensitive: 'i' should match İ
            assert_eq!(replace("İB", "i", "x", false), "xB");
            assert_eq!(replace("AİAİA", "i", "x", false), "AxAxA");
        }

        #[test]
        fn test_replace_longer_replacement() {
            assert_eq!(replace("ab", "a", "xyz", true), "xyzb");
            assert_eq!(replace("aaa", "a", "bb", true), "bbbbbb");
        }

        #[test]
        fn test_replace_shorter_replacement() {
            assert_eq!(replace("hello", "ll", "l", true), "helo");
            assert_eq!(replace("aaaa", "aa", "a", true), "aa");
        }

        // ===== ASCII flag caching tests =====

        #[test]
        fn test_ascii_flag_caching() {
            // Pure ASCII string should have is_ascii flag set
            let ascii = Var::mk_str("hello world");
            assert!(ascii.str_is_ascii());

            // String with non-ASCII should not have flag set
            let unicode = Var::mk_str("héllo");
            assert!(!unicode.str_is_ascii());

            // Empty string is ASCII
            let empty = Var::mk_str("");
            assert!(empty.str_is_ascii());

            // String with emoji is not ASCII
            let emoji = Var::mk_str("hello 😀");
            assert!(!emoji.str_is_ascii());
        }

        #[test]
        fn test_ascii_fast_path_correctness() {
            // These should all use ASCII fast path
            let s = Var::mk_str("HELLO WORLD");
            let n = Var::mk_str("world");
            assert!(s.str_is_ascii() && n.str_is_ascii());
            assert_eq!(s.str_find(&n, false, 0), Some(6));

            // These should fall back to Unicode
            let s = Var::mk_str("HÉLLO WÖRLD");
            let n = Var::mk_str("world");
            assert!(!s.str_is_ascii());
            assert_eq!(s.str_find(&n, false, 0), None); // ö != o
        }
    }
}
