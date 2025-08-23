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

//! Complex object/string matching functionality with ordinal support

use moor_var::Var;
use strsim::damerau_levenshtein;

/// Error type for ordinal parsing failures
#[derive(thiserror::Error, Debug, Clone, PartialEq)]
#[error("Failed to parse ordinal")]
pub struct OrdinalParseError;

/// Simple ordinals - just use a lookup table that returns the correct value directly
fn find_ordinal_value(word: &str) -> Option<i64> {
    match word.to_lowercase().as_str() {
        "first" => Some(1),
        "second" => Some(2),
        "third" => Some(3),
        "fourth" => Some(4),
        "fifth" => Some(5),
        "sixth" => Some(6),
        "seventh" => Some(7),
        "eighth" => Some(8),
        "ninth" => Some(9),
        "tenth" => Some(10),
        "eleventh" => Some(11),
        "twelfth" => Some(12),
        "thirteenth" => Some(13),
        "fourteenth" => Some(14),
        "fifteenth" => Some(15),
        "sixteenth" => Some(16),
        "seventeenth" => Some(17),
        "eighteenth" => Some(18),
        "nineteenth" => Some(19),
        "twenty" | "twentieth" => Some(20),
        "thirty" | "thirtieth" => Some(30),
        "forty" | "fortieth" => Some(40),
        "fifty" | "fiftieth" => Some(50),
        "sixty" | "sixtieth" => Some(60),
        "seventy" | "seventieth" => Some(70),
        "eighty" | "eightieth" => Some(80),
        "ninety" | "ninetieth" => Some(90),
        _ => None,
    }
}

/// Result of complex matching operation
#[derive(Debug, Clone, PartialEq)]
pub enum ComplexMatchResult<T> {
    /// No matches found
    NoMatch,
    /// Single match (for ordinal selection)
    Single(T),
    /// Multiple matches (for non-ordinal cases)
    Multiple(Vec<T>),
}

/// Parse an ordinal from a word, supporting various formats
pub fn parse_ordinal(word: &str) -> Result<i64, OrdinalParseError> {
    // Split on hyphens for compound ordinals like "twenty-first"
    let tokens: Vec<&str> = word.split('-').collect();
    let mut ordinal_values = Vec::new();

    for token in tokens {
        // Try numeric patterns first: "1.", "2.", etc.
        if token.len() > 1 && token.ends_with('.') {
            let num_str = &token[..token.len() - 1];
            if let Ok(num) = num_str.parse::<i64>() {
                ordinal_values.push(num);
                continue;
            }
        }

        // Try ordinal word matching
        if let Some(ordinal) = find_ordinal_value(token) {
            ordinal_values.push(ordinal);
            continue;
        }

        // Try numeric ordinals: "1st", "2nd", "3rd", "4th", etc.
        if token.len() > 2 {
            let (num_part, suffix) = token.split_at(token.len() - 2);
            if matches!(suffix, "st" | "nd" | "rd" | "th")
                && let Ok(num) = num_part.parse::<i64>() {
                    ordinal_values.push(num);
                    continue;
                }
        }

        // If we can't parse any token, fail
        return Err(OrdinalParseError);
    }

    match ordinal_values.len() {
        0 => Err(OrdinalParseError),
        1 => Ok(ordinal_values[0]),
        2 => {
            // Handle compound ordinals like "twenty-first" -> 21
            // First value should be a multiple of 10 (twenty=20), second should be 1-9 (first=1)
            Ok(ordinal_values[0] + ordinal_values[1])
        }
        _ => Err(OrdinalParseError), // Too many parts
    }
}

/// Parse the input token to extract ordinal and subject
pub fn parse_input_token(token: &str) -> (i64, String) {
    let words: Vec<&str> = token.split_whitespace().collect();
    if words.is_empty() {
        return (0, String::new());
    }

    // Try to parse ordinal from first word
    if let Ok(ordinal) = parse_ordinal(words[0]) {
        if words.len() > 1 {
            (ordinal, words[1..].join(" "))
        } else {
            (ordinal, String::new())
        }
    } else {
        (0, token.to_string())
    }
}

/// Perform complex matching on a list of strings, returning the matching strings
pub fn complex_match_strings(token: &str, strings: &[Var]) -> ComplexMatchResult<Var> {
    complex_match_strings_with_fuzzy(token, strings, true)
}

/// Perform complex matching on a list of strings with optional fuzzy matching
pub fn complex_match_strings_with_fuzzy(
    token: &str,
    strings: &[Var],
    use_fuzzy: bool,
) -> ComplexMatchResult<Var> {
    let (ordinal, subject) = parse_input_token(token);

    if subject.is_empty() {
        return ComplexMatchResult::NoMatch;
    }

    let mut exact_matches = Vec::new();
    let mut start_matches = Vec::new();
    let mut contain_matches = Vec::new();
    let mut fuzzy_matches = Vec::new();

    let subject_lower = subject.to_lowercase();

    // Match against strings directly
    for string_var in strings.iter() {
        let Some(string_val) = string_var.as_string() else {
            continue;
        };

        let string_lower = string_val.to_lowercase();

        // Exact match
        if string_lower == subject_lower {
            if ordinal > 0 && ordinal == (exact_matches.len() as i64 + 1) {
                return ComplexMatchResult::Single(string_var.clone());
            }
            exact_matches.push(string_var.clone());
        }
        // Prefix match
        else if string_lower.starts_with(&subject_lower) {
            start_matches.push(string_var.clone());
        }
        // Substring match
        else if string_lower.contains(&subject_lower) {
            contain_matches.push(string_var.clone());
        }
        // Fuzzy match using Damerau-Levenshtein distance
        else if use_fuzzy {
            let distance = damerau_levenshtein(&subject_lower, &string_lower);
            let max_distance = if subject_lower.len() <= 3 { 1 } else { 2 };
            if distance <= max_distance {
                fuzzy_matches.push(string_var.clone());
            }
        }
    }

    // Handle ordinal selection
    if ordinal > 0 {
        if ordinal <= exact_matches.len() as i64 {
            return ComplexMatchResult::Single(exact_matches[(ordinal - 1) as usize].clone());
        }
        if ordinal <= start_matches.len() as i64 {
            return ComplexMatchResult::Single(start_matches[(ordinal - 1) as usize].clone());
        }
        if ordinal <= contain_matches.len() as i64 {
            return ComplexMatchResult::Single(contain_matches[(ordinal - 1) as usize].clone());
        }
        if use_fuzzy && ordinal <= fuzzy_matches.len() as i64 {
            return ComplexMatchResult::Single(fuzzy_matches[(ordinal - 1) as usize].clone());
        }
        return ComplexMatchResult::NoMatch;
    }

    // Return best match tier - for 2-arg form, return the first match or all matches
    if !exact_matches.is_empty() {
        return match exact_matches.len() {
            1 => ComplexMatchResult::Single(exact_matches[0].clone()),
            _ => ComplexMatchResult::Multiple(exact_matches),
        };
    }

    if !start_matches.is_empty() {
        return match start_matches.len() {
            1 => ComplexMatchResult::Single(start_matches[0].clone()),
            _ => ComplexMatchResult::Multiple(start_matches),
        };
    }

    if !contain_matches.is_empty() {
        return match contain_matches.len() {
            1 => ComplexMatchResult::Single(contain_matches[0].clone()),
            _ => ComplexMatchResult::Multiple(contain_matches),
        };
    }

    if use_fuzzy && !fuzzy_matches.is_empty() {
        return match fuzzy_matches.len() {
            1 => ComplexMatchResult::Single(fuzzy_matches[0].clone()),
            _ => ComplexMatchResult::Multiple(fuzzy_matches),
        };
    }

    ComplexMatchResult::NoMatch
}

/// Perform complex matching with separate objects and keys lists
pub fn complex_match_objects_keys(
    token: &str,
    objects: &[Var],
    keys: &[Var],
) -> ComplexMatchResult<Var> {
    complex_match_objects_keys_with_fuzzy(token, objects, keys, true)
}

/// Perform complex matching with separate objects and keys lists with optional fuzzy matching
pub fn complex_match_objects_keys_with_fuzzy(
    token: &str,
    objects: &[Var],
    keys: &[Var],
    use_fuzzy: bool,
) -> ComplexMatchResult<Var> {
    let (ordinal, subject) = parse_input_token(token);

    if subject.is_empty() || objects.is_empty() || keys.is_empty() {
        return ComplexMatchResult::NoMatch;
    }

    let mut exact_matches = Vec::new();
    let mut start_matches = Vec::new();
    let mut contain_matches = Vec::new();
    let mut fuzzy_matches = Vec::new();

    let subject_lower = subject.to_lowercase();

    // Match against keys, return corresponding objects
    for (idx, key_set) in keys.iter().enumerate() {
        if idx >= objects.len() {
            break;
        }

        let obj_val = &objects[idx];

        // Handle both list of strings and single string for keys
        let key_strings: Vec<String> = match key_set.variant() {
            moor_var::Variant::List(key_list) => {
                let mut strings = Vec::new();
                for k in key_list.iter() {
                    if let Some(s) = k.as_string() {
                        strings.push(s.to_string());
                    }
                }
                strings
            }
            moor_var::Variant::Str(s) => vec![s.as_str().to_string()],
            _ => continue,
        };

        for key_str in &key_strings {
            let key_lower = key_str.to_lowercase();

            // Exact match
            if key_lower == subject_lower {
                if ordinal > 0 && ordinal == (exact_matches.len() as i64 + 1) {
                    return ComplexMatchResult::Single(obj_val.clone());
                }
                exact_matches.push(obj_val.clone());
                break; // Don't check other keys for this object
            }
            // Prefix match
            else if key_lower.starts_with(&subject_lower) {
                start_matches.push(obj_val.clone());
                break;
            }
            // Substring match
            else if key_lower.contains(&subject_lower) {
                contain_matches.push(obj_val.clone());
                break;
            }
        }

        // If no exact/prefix/substring match found, try fuzzy matching
        if use_fuzzy
            && !exact_matches.iter().any(|v| v == obj_val)
            && !start_matches.iter().any(|v| v == obj_val)
            && !contain_matches.iter().any(|v| v == obj_val)
        {
            for key_str in &key_strings {
                let key_lower = key_str.to_lowercase();
                let distance = damerau_levenshtein(&subject_lower, &key_lower);
                let max_distance = if subject_lower.len() <= 3 { 1 } else { 2 };
                if distance <= max_distance {
                    fuzzy_matches.push(obj_val.clone());
                    break;
                }
            }
        }
    }

    // Handle ordinal selection
    if ordinal > 0 {
        if ordinal <= exact_matches.len() as i64 {
            return ComplexMatchResult::Single(exact_matches[(ordinal - 1) as usize].clone());
        }
        if ordinal <= start_matches.len() as i64 {
            return ComplexMatchResult::Single(start_matches[(ordinal - 1) as usize].clone());
        }
        if ordinal <= contain_matches.len() as i64 {
            return ComplexMatchResult::Single(contain_matches[(ordinal - 1) as usize].clone());
        }
        if use_fuzzy && ordinal <= fuzzy_matches.len() as i64 {
            return ComplexMatchResult::Single(fuzzy_matches[(ordinal - 1) as usize].clone());
        }
        return ComplexMatchResult::NoMatch;
    }

    // Return best match tier
    if !exact_matches.is_empty() {
        return match exact_matches.len() {
            1 => ComplexMatchResult::Single(exact_matches[0].clone()),
            _ => ComplexMatchResult::Multiple(exact_matches),
        };
    }

    if !start_matches.is_empty() {
        return match start_matches.len() {
            1 => ComplexMatchResult::Single(start_matches[0].clone()),
            _ => ComplexMatchResult::Multiple(start_matches),
        };
    }

    if !contain_matches.is_empty() {
        return match contain_matches.len() {
            1 => ComplexMatchResult::Single(contain_matches[0].clone()),
            _ => ComplexMatchResult::Multiple(contain_matches),
        };
    }

    if use_fuzzy && !fuzzy_matches.is_empty() {
        return match fuzzy_matches.len() {
            1 => ComplexMatchResult::Single(fuzzy_matches[0].clone()),
            _ => ComplexMatchResult::Multiple(fuzzy_matches),
        };
    }

    ComplexMatchResult::NoMatch
}

#[cfg(test)]
mod tests {
    use super::*;
    use moor_var::{v_int, v_list, v_str};

    #[test]
    fn test_parse_ordinal_words() {
        assert_eq!(parse_ordinal("first"), Ok(1));
        assert_eq!(parse_ordinal("second"), Ok(2));
        assert_eq!(parse_ordinal("twelfth"), Ok(12));
        assert_eq!(parse_ordinal("twentieth"), Ok(20));
        assert_eq!(parse_ordinal("twenty"), Ok(20));
    }

    #[test]
    fn test_parse_ordinal_numeric() {
        assert_eq!(parse_ordinal("1st"), Ok(1));
        assert_eq!(parse_ordinal("2nd"), Ok(2));
        assert_eq!(parse_ordinal("3rd"), Ok(3));
        assert_eq!(parse_ordinal("4th"), Ok(4));
        assert_eq!(parse_ordinal("21st"), Ok(21));
    }

    #[test]
    fn test_parse_ordinal_dots() {
        assert_eq!(parse_ordinal("1."), Ok(1));
        assert_eq!(parse_ordinal("10."), Ok(10));
    }

    #[test]
    fn test_parse_ordinal_compound() {
        assert_eq!(parse_ordinal("twenty-first"), Ok(21));

        // Let's debug this case
        assert_eq!(find_ordinal_value("thirty"), Some(30));
        assert_eq!(find_ordinal_value("second"), Some(2));
        // So thirty-second should be 30 + 2 = 32
        assert_eq!(parse_ordinal("thirty-second"), Ok(32));
    }

    #[test]
    fn test_parse_input_token() {
        assert_eq!(parse_input_token("foo"), (0, "foo".to_string()));
        assert_eq!(parse_input_token("first foo"), (1, "foo".to_string()));
        assert_eq!(parse_input_token("2nd foo bar"), (2, "foo bar".to_string()));
        assert_eq!(
            parse_input_token("twenty-first lamp"),
            (21, "lamp".to_string())
        );
    }

    #[test]
    fn test_complex_match_strings_exact() {
        let strings = vec![v_str("bar"), v_str("baz"), v_str("foobar")];
        let result = complex_match_strings("foobar", &strings);
        assert_eq!(result, ComplexMatchResult::Single(v_str("foobar")));
    }

    #[test]
    fn test_complex_match_strings_substring() {
        let strings = vec![v_str("bar"), v_str("baz"), v_str("foobar")];
        let result = complex_match_strings("foo", &strings);
        assert_eq!(result, ComplexMatchResult::Single(v_str("foobar")));
    }

    #[test]
    fn test_complex_match_strings_precedence() {
        let strings = vec![v_str("foobar"), v_str("foo")];
        let result = complex_match_strings("foo", &strings);
        // Exact match "foo" should win over substring match "foobar"
        assert_eq!(result, ComplexMatchResult::Single(v_str("foo")));
    }

    #[test]
    fn test_complex_match_strings_ordinal() {
        let strings = vec![v_str("foobar"), v_str("food"), v_str("foot")];
        let result = complex_match_strings("second foo", &strings);
        assert_eq!(result, ComplexMatchResult::Single(v_str("food")));
    }

    #[test]
    fn test_complex_match_objects_keys() {
        let objects = vec![v_int(3), v_int(4), v_int(5)];
        let keys = vec![
            v_list(&[v_str("foobar"), v_str("foo")]),
            v_list(&[v_str("zed")]),
            v_str("zonk"),
        ];
        let result = complex_match_objects_keys("foo", &objects, &keys);
        assert_eq!(result, ComplexMatchResult::Single(v_int(3)));
    }

    #[test]
    fn test_fuzzy_match_simple_typos() {
        let strings = vec![v_str("lamp"), v_str("table"), v_str("chair")];

        // Single character substitution
        let result = complex_match_strings("lammp", &strings);
        assert_eq!(result, ComplexMatchResult::Single(v_str("lamp")));

        // Single character deletion
        let result = complex_match_strings("lmp", &strings);
        assert_eq!(result, ComplexMatchResult::Single(v_str("lamp")));

        // Single character insertion
        let result = complex_match_strings("tabel", &strings);
        assert_eq!(result, ComplexMatchResult::Single(v_str("table")));
    }

    #[test]
    fn test_fuzzy_match_transposition() {
        let strings = vec![v_str("chair"), v_str("table")];

        // Adjacent character swap
        let result = complex_match_strings("chaor", &strings);
        assert_eq!(result, ComplexMatchResult::Single(v_str("chair")));

        let result = complex_match_strings("talbe", &strings);
        assert_eq!(result, ComplexMatchResult::Single(v_str("table")));
    }

    #[test]
    fn test_fuzzy_match_precedence() {
        let strings = vec![v_str("foobar"), v_str("foo"), v_str("fooo")];

        // Exact match should still win over fuzzy matches
        let result = complex_match_strings("foo", &strings);
        assert_eq!(result, ComplexMatchResult::Single(v_str("foo")));

        // But fuzzy should work when no exact/prefix/substring match
        let result = complex_match_strings("foox", &strings);
        // Both "foo" and "fooo" are distance 1 from "foox", so we get multiple matches
        assert_eq!(
            result,
            ComplexMatchResult::Multiple(vec![v_str("foo"), v_str("fooo")])
        );
    }

    #[test]
    fn test_fuzzy_match_with_ordinal() {
        let strings = vec![v_str("lamp"), v_str("lump"), v_str("limp")];

        // Should find second fuzzy match
        let result = complex_match_strings("second lmop", &strings);
        // "lmop" should fuzzy match multiple items, so "second" should pick the second one
        assert!(matches!(result, ComplexMatchResult::Single(_)));
    }

    #[test]
    fn test_fuzzy_match_distance_threshold() {
        let strings = vec![v_str("lamp"), v_str("table")];

        // Too many changes - should not match
        let result = complex_match_strings("xyz", &strings);
        assert_eq!(result, ComplexMatchResult::NoMatch);

        // Just within threshold for short words (distance 1)
        let result = complex_match_strings("lam", &strings);
        assert_eq!(result, ComplexMatchResult::Single(v_str("lamp")));
    }

    #[test]
    fn test_fuzzy_match_objects_keys() {
        let objects = vec![v_int(1), v_int(2)];
        let keys = vec![
            v_list(&[v_str("lamp"), v_str("light")]),
            v_list(&[v_str("table"), v_str("desk")]),
        ];

        // Should fuzzy match "lamp" with "lammp"
        let result = complex_match_objects_keys("lammp", &objects, &keys);
        assert_eq!(result, ComplexMatchResult::Single(v_int(1)));

        // Should fuzzy match "table" with "tabel"
        let result = complex_match_objects_keys("tabel", &objects, &keys);
        assert_eq!(result, ComplexMatchResult::Single(v_int(2)));
    }

    #[test]
    fn test_fuzzy_parameter_control() {
        let strings = vec![v_str("lamp"), v_str("table")];

        // With fuzzy enabled (default)
        let result = complex_match_strings_with_fuzzy("lammp", &strings, true);
        assert_eq!(result, ComplexMatchResult::Single(v_str("lamp")));

        // With fuzzy disabled
        let result = complex_match_strings_with_fuzzy("lammp", &strings, false);
        assert_eq!(result, ComplexMatchResult::NoMatch);

        // Test objects/keys version
        let objects = vec![v_int(1), v_int(2)];
        let keys = vec![v_list(&[v_str("lamp")]), v_list(&[v_str("table")])];

        // With fuzzy enabled
        let result = complex_match_objects_keys_with_fuzzy("lammp", &objects, &keys, true);
        assert_eq!(result, ComplexMatchResult::Single(v_int(1)));

        // With fuzzy disabled
        let result = complex_match_objects_keys_with_fuzzy("lammp", &objects, &keys, false);
        assert_eq!(result, ComplexMatchResult::NoMatch);
    }
}
