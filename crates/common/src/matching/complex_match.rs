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
        if token.chars().count() > 2 {
            let char_count = token.chars().count();
            let split_pos = token
                .char_indices()
                .nth(char_count - 2)
                .map(|(i, _)| i)
                .unwrap_or(0);
            let (num_part, suffix) = token.split_at(split_pos);
            if matches!(suffix, "st" | "nd" | "rd" | "th")
                && let Ok(num) = num_part.parse::<i64>()
            {
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
    // Handle "N.subject" pattern (e.g., "2.foo") before whitespace split
    if let Some(dot_pos) = token.find('.') {
        let (num_part, rest) = token.split_at(dot_pos);
        if let Ok(ordinal) = num_part.parse::<i64>() {
            let subject = &rest[1..]; // skip the dot
            if !subject.is_empty() {
                return (ordinal, subject.to_string());
            }
        }
    }

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

/// Check if the token has an "all " or "*." prefix for all-tier matching.
/// Returns None if no prefix, or Some(subject) if prefix found and subject is non-empty.
/// When the token is exactly "all" or "*." with no subject, returns None to treat it as a literal.
pub fn parse_all_tiers_prefix(token: &str) -> Option<String> {
    if let Some(rest) = token.strip_prefix("all ") {
        if !rest.is_empty() {
            return Some(rest.to_string());
        }
    }
    if let Some(rest) = token.strip_prefix("*.") {
        if !rest.is_empty() {
            return Some(rest.to_string());
        }
    }
    None
}

/// Perform complex matching on a list of strings, returning the matching strings
pub fn complex_match_strings(token: &str, strings: &[Var]) -> ComplexMatchResult<Var> {
    complex_match_strings_with_fuzzy_threshold(token, strings, 0.0)
}

/// Perform complex matching on a list of strings with optional fuzzy matching
pub fn complex_match_strings_with_fuzzy(
    token: &str,
    strings: &[Var],
    use_fuzzy: bool,
) -> ComplexMatchResult<Var> {
    let fuzzy_threshold = if use_fuzzy { 0.5 } else { 0.0 };
    complex_match_strings_with_fuzzy_threshold(token, strings, fuzzy_threshold)
}

/// Perform complex matching on a list of strings with configurable fuzzy threshold
///
/// fuzzy_threshold: 0.0 = no fuzzy matching, 0.5 = reasonable default, 1.0 = very permissive
pub fn complex_match_strings_with_fuzzy_threshold(
    token: &str,
    strings: &[Var],
    fuzzy_threshold: f64,
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
        else if fuzzy_threshold > 0.0 {
            let distance = damerau_levenshtein(&subject_lower, &string_lower);
            let max_distance = (subject_lower.len() as f64 * fuzzy_threshold).ceil() as usize;
            if distance <= max_distance {
                fuzzy_matches.push(string_var.clone());
            }
        }
    }

    // Handle ordinal selection - count across all tiers combined
    if ordinal > 0 {
        let mut all_matches = Vec::new();
        all_matches.extend(exact_matches.iter().cloned());
        all_matches.extend(start_matches.iter().cloned());
        all_matches.extend(contain_matches.iter().cloned());
        if fuzzy_threshold > 0.0 {
            all_matches.extend(fuzzy_matches.iter().cloned());
        }
        if ordinal <= all_matches.len() as i64 {
            return ComplexMatchResult::Single(all_matches[(ordinal - 1) as usize].clone());
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

    if fuzzy_threshold > 0.0 && !fuzzy_matches.is_empty() {
        return match fuzzy_matches.len() {
            1 => ComplexMatchResult::Single(fuzzy_matches[0].clone()),
            _ => ComplexMatchResult::Multiple(fuzzy_matches),
        };
    }

    ComplexMatchResult::NoMatch
}

/// Return all matches from the best (highest priority) non-empty tier
///
/// If an ordinal is provided (e.g., "1.foo", "2nd foo"), returns only that
/// specific match as a single-element list. Otherwise returns all matches
/// from the first non-empty tier (exact > prefix > substring > fuzzy).
pub fn complex_match_strings_all(token: &str, strings: &[Var], fuzzy_threshold: f64) -> Vec<Var> {
    let (ordinal, subject) = parse_input_token(token);

    if subject.is_empty() {
        return Vec::new();
    }

    let mut exact_matches = Vec::new();
    let mut start_matches = Vec::new();
    let mut contain_matches = Vec::new();
    let mut fuzzy_matches = Vec::new();

    let subject_lower = subject.to_lowercase();

    for string_var in strings.iter() {
        let Some(string_val) = string_var.as_string() else {
            continue;
        };

        let string_lower = string_val.to_lowercase();

        if string_lower == subject_lower {
            exact_matches.push(string_var.clone());
        } else if string_lower.starts_with(&subject_lower) {
            start_matches.push(string_var.clone());
        } else if string_lower.contains(&subject_lower) {
            contain_matches.push(string_var.clone());
        } else if fuzzy_threshold > 0.0 {
            let distance = damerau_levenshtein(&subject_lower, &string_lower);
            let max_distance = (subject_lower.len() as f64 * fuzzy_threshold).ceil() as usize;
            if distance <= max_distance {
                fuzzy_matches.push(string_var.clone());
            }
        }
    }

    // Get all matches from the best non-empty tier
    let matches = if !exact_matches.is_empty() {
        exact_matches
    } else if !start_matches.is_empty() {
        start_matches
    } else if !contain_matches.is_empty() {
        contain_matches
    } else if fuzzy_threshold > 0.0 && !fuzzy_matches.is_empty() {
        fuzzy_matches
    } else {
        return Vec::new();
    };

    // If ordinal specified, return only that match (as single-element list)
    if ordinal > 0 {
        if ordinal <= matches.len() as i64 {
            return vec![matches[(ordinal - 1) as usize].clone()];
        }
        return Vec::new(); // Ordinal out of range
    }

    matches
}

/// Return all matches from ALL tiers concatenated in priority order.
///
/// Unlike `complex_match_strings_all` which returns only the best tier,
/// this function returns matches from every tier (exact, prefix, contains, fuzzy)
/// in that order. Used when the "all " or "*." prefix is specified.
pub fn complex_match_strings_all_tiers(
    token: &str,
    strings: &[Var],
    fuzzy_threshold: f64,
) -> Vec<Var> {
    // Strip any leading ordinal from the token
    let (_, subject) = parse_input_token(token);

    if subject.is_empty() {
        return Vec::new();
    }

    let mut exact_matches = Vec::new();
    let mut start_matches = Vec::new();
    let mut contain_matches = Vec::new();
    let mut fuzzy_matches = Vec::new();

    let subject_lower = subject.to_lowercase();

    for string_var in strings.iter() {
        let Some(string_val) = string_var.as_string() else {
            continue;
        };

        let string_lower = string_val.to_lowercase();

        if string_lower == subject_lower {
            exact_matches.push(string_var.clone());
        } else if string_lower.starts_with(&subject_lower) {
            start_matches.push(string_var.clone());
        } else if string_lower.contains(&subject_lower) {
            contain_matches.push(string_var.clone());
        } else if fuzzy_threshold > 0.0 {
            let distance = damerau_levenshtein(&subject_lower, &string_lower);
            let max_distance = (subject_lower.len() as f64 * fuzzy_threshold).ceil() as usize;
            if distance <= max_distance {
                fuzzy_matches.push(string_var.clone());
            }
        }
    }

    // Concatenate all tiers in priority order
    let mut result = Vec::new();
    result.append(&mut exact_matches);
    result.append(&mut start_matches);
    result.append(&mut contain_matches);
    if fuzzy_threshold > 0.0 {
        result.append(&mut fuzzy_matches);
    }
    result
}

/// Perform complex matching with separate objects and keys lists
pub fn complex_match_objects_keys(
    token: &str,
    objects: &[Var],
    keys: &[Var],
) -> ComplexMatchResult<Var> {
    complex_match_objects_keys_with_fuzzy_threshold(token, objects, keys, 0.0)
}

/// Perform complex matching with separate objects and keys lists with optional fuzzy matching
pub fn complex_match_objects_keys_with_fuzzy(
    token: &str,
    objects: &[Var],
    keys: &[Var],
    use_fuzzy: bool,
) -> ComplexMatchResult<Var> {
    let fuzzy_threshold = if use_fuzzy { 0.5 } else { 0.0 };
    complex_match_objects_keys_with_fuzzy_threshold(token, objects, keys, fuzzy_threshold)
}

/// Perform complex matching with separate objects and keys lists with configurable fuzzy threshold
///
/// fuzzy_threshold: 0.0 = no fuzzy matching, 0.5 = reasonable default, 1.0 = very permissive
pub fn complex_match_objects_keys_with_fuzzy_threshold(
    token: &str,
    objects: &[Var],
    keys: &[Var],
    fuzzy_threshold: f64,
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

        // Find the best match type across ALL keys for this object
        // Priority: exact (0) > prefix (1) > substring (2) > none (3)
        let mut best_match: u8 = 3; // none

        for key_str in &key_strings {
            let key_lower = key_str.to_lowercase();

            if key_lower == subject_lower {
                best_match = 0; // exact - can't do better, stop checking
                break;
            } else if key_lower.starts_with(&subject_lower) {
                best_match = best_match.min(1); // prefix
            } else if key_lower.contains(&subject_lower) {
                best_match = best_match.min(2); // substring
            }
        }

        // Add object to the appropriate category based on best match found
        match best_match {
            0 => exact_matches.push(obj_val.clone()),
            1 => start_matches.push(obj_val.clone()),
            2 => contain_matches.push(obj_val.clone()),
            _ => {}
        }

        // If no exact/prefix/substring match found, try fuzzy matching
        if fuzzy_threshold > 0.0
            && !exact_matches.iter().any(|v| v == obj_val)
            && !start_matches.iter().any(|v| v == obj_val)
            && !contain_matches.iter().any(|v| v == obj_val)
        {
            for key_str in &key_strings {
                let key_lower = key_str.to_lowercase();
                let distance = damerau_levenshtein(&subject_lower, &key_lower);
                let max_distance = (subject_lower.len() as f64 * fuzzy_threshold).ceil() as usize;
                if distance <= max_distance {
                    fuzzy_matches.push(obj_val.clone());
                    break;
                }
            }
        }
    }

    // Handle ordinal selection - count across all tiers combined
    if ordinal > 0 {
        let mut all_matches = Vec::new();
        all_matches.extend(exact_matches.iter().cloned());
        all_matches.extend(start_matches.iter().cloned());
        all_matches.extend(contain_matches.iter().cloned());
        if fuzzy_threshold > 0.0 {
            all_matches.extend(fuzzy_matches.iter().cloned());
        }
        if ordinal <= all_matches.len() as i64 {
            return ComplexMatchResult::Single(all_matches[(ordinal - 1) as usize].clone());
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

    if fuzzy_threshold > 0.0 && !fuzzy_matches.is_empty() {
        return match fuzzy_matches.len() {
            1 => ComplexMatchResult::Single(fuzzy_matches[0].clone()),
            _ => ComplexMatchResult::Multiple(fuzzy_matches),
        };
    }

    ComplexMatchResult::NoMatch
}

/// Return all matches from the best non-empty tier when matching with keys.
/// Similar to `complex_match_strings_all` but for object/key matching.
///
/// If an ordinal is provided (e.g., "1.foo", "2nd foo"), returns only that
/// specific match as a single-element list. Otherwise returns all matching
/// objects from the first non-empty tier (exact > prefix > substring > fuzzy).
pub fn complex_match_objects_keys_all(
    token: &str,
    objects: &[Var],
    keys: &[Var],
    fuzzy_threshold: f64,
) -> Vec<Var> {
    let (ordinal, subject) = parse_input_token(token);

    if subject.is_empty() || objects.is_empty() || keys.is_empty() {
        return Vec::new();
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

        // Find the best match type across ALL keys for this object
        // Priority: exact (0) > prefix (1) > substring (2) > none (3)
        let mut best_match: u8 = 3; // none

        for key_str in &key_strings {
            let key_lower = key_str.to_lowercase();

            if key_lower == subject_lower {
                best_match = 0; // exact - can't do better, stop checking
                break;
            } else if key_lower.starts_with(&subject_lower) {
                best_match = best_match.min(1); // prefix
            } else if key_lower.contains(&subject_lower) {
                best_match = best_match.min(2); // substring
            }
        }

        // Add object to the appropriate category based on best match found
        match best_match {
            0 => exact_matches.push(obj_val.clone()),
            1 => start_matches.push(obj_val.clone()),
            2 => contain_matches.push(obj_val.clone()),
            _ => {}
        }

        // If no exact/prefix/substring match found, try fuzzy matching
        if fuzzy_threshold > 0.0
            && !exact_matches.iter().any(|v| v == obj_val)
            && !start_matches.iter().any(|v| v == obj_val)
            && !contain_matches.iter().any(|v| v == obj_val)
        {
            for key_str in &key_strings {
                let key_lower = key_str.to_lowercase();
                let distance = damerau_levenshtein(&subject_lower, &key_lower);
                let max_distance = (subject_lower.len() as f64 * fuzzy_threshold).ceil() as usize;
                if distance <= max_distance {
                    fuzzy_matches.push(obj_val.clone());
                    break;
                }
            }
        }
    }

    // Get all matches from the best non-empty tier
    let matches = if !exact_matches.is_empty() {
        exact_matches
    } else if !start_matches.is_empty() {
        start_matches
    } else if !contain_matches.is_empty() {
        contain_matches
    } else if fuzzy_threshold > 0.0 && !fuzzy_matches.is_empty() {
        fuzzy_matches
    } else {
        return Vec::new();
    };

    // If ordinal specified, return only that match (as single-element list)
    if ordinal > 0 {
        if ordinal <= matches.len() as i64 {
            return vec![matches[(ordinal - 1) as usize].clone()];
        }
        return Vec::new(); // Ordinal out of range
    }

    matches
}

/// Return all matches from ALL tiers when matching with keys.
/// Unlike `complex_match_objects_keys_all` which returns only the best tier,
/// this function returns matches from every tier (exact, prefix, contains, fuzzy)
/// in that order. Used when the "all " or "*." prefix is specified.
pub fn complex_match_objects_keys_all_tiers(
    token: &str,
    objects: &[Var],
    keys: &[Var],
    fuzzy_threshold: f64,
) -> Vec<Var> {
    // Strip any leading ordinal from the token
    let (_, subject) = parse_input_token(token);

    if subject.is_empty() || objects.is_empty() || keys.is_empty() {
        return Vec::new();
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

        // Find the best match type across ALL keys for this object
        // Priority: exact (0) > prefix (1) > substring (2) > none (3)
        let mut best_match: u8 = 3; // none

        for key_str in &key_strings {
            let key_lower = key_str.to_lowercase();

            if key_lower == subject_lower {
                best_match = 0; // exact - can't do better, stop checking
                break;
            } else if key_lower.starts_with(&subject_lower) {
                best_match = best_match.min(1); // prefix
            } else if key_lower.contains(&subject_lower) {
                best_match = best_match.min(2); // substring
            }
        }

        // Add object to the appropriate category based on best match found
        match best_match {
            0 => exact_matches.push(obj_val.clone()),
            1 => start_matches.push(obj_val.clone()),
            2 => contain_matches.push(obj_val.clone()),
            _ => {}
        }

        // If no exact/prefix/substring match found, try fuzzy matching
        if fuzzy_threshold > 0.0
            && !exact_matches.iter().any(|v| v == obj_val)
            && !start_matches.iter().any(|v| v == obj_val)
            && !contain_matches.iter().any(|v| v == obj_val)
        {
            for key_str in &key_strings {
                let key_lower = key_str.to_lowercase();
                let distance = damerau_levenshtein(&subject_lower, &key_lower);
                let max_distance = (subject_lower.len() as f64 * fuzzy_threshold).ceil() as usize;
                if distance <= max_distance {
                    fuzzy_matches.push(obj_val.clone());
                    break;
                }
            }
        }
    }

    // Concatenate all tiers in priority order
    let mut result = Vec::new();
    result.append(&mut exact_matches);
    result.append(&mut start_matches);
    result.append(&mut contain_matches);
    if fuzzy_threshold > 0.0 {
        result.append(&mut fuzzy_matches);
    }
    result
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
    fn test_complex_match_objects_keys_best_match_across_aliases() {
        // Regression test: ensure we find the best match type across ALL keys,
        // not just the first match type found.
        // For "heated pool" with keys ["heated pool", "pool"], matching "pool"
        // should be an exact match (via "pool" alias), not a substring match.
        let objects = vec![v_int(1)];
        let keys = vec![v_list(&[v_str("heated pool"), v_str("pool")])];
        let result = complex_match_objects_keys("pool", &objects, &keys);
        assert_eq!(result, ComplexMatchResult::Single(v_int(1)));

        // Verify it goes in exact_matches, not contain_matches, by testing
        // that it beats a prefix match from another object
        let objects = vec![v_int(1), v_int(2)];
        let keys = vec![
            v_list(&[v_str("heated pool"), v_str("pool")]), // exact via "pool"
            v_list(&[v_str("poolside")]),                   // prefix match
        ];
        let result = complex_match_objects_keys("pool", &objects, &keys);
        // Should return only the exact match, not both
        assert_eq!(result, ComplexMatchResult::Single(v_int(1)));
    }

    #[test]
    fn test_fuzzy_match_simple_typos() {
        let strings = vec![v_str("lamp"), v_str("table"), v_str("chair")];

        // Single character substitution (with fuzzy enabled)
        let result = complex_match_strings_with_fuzzy_threshold("lammp", &strings, 0.5);
        assert_eq!(result, ComplexMatchResult::Single(v_str("lamp")));

        // Single character deletion
        let result = complex_match_strings_with_fuzzy_threshold("lmp", &strings, 0.5);
        assert_eq!(result, ComplexMatchResult::Single(v_str("lamp")));

        // Single character insertion
        let result = complex_match_strings_with_fuzzy_threshold("tabel", &strings, 0.5);
        assert_eq!(result, ComplexMatchResult::Single(v_str("table")));
    }

    #[test]
    fn test_fuzzy_match_transposition() {
        let strings = vec![v_str("chair"), v_str("table")];

        // Adjacent character swap (with fuzzy enabled)
        let result = complex_match_strings_with_fuzzy_threshold("chaor", &strings, 0.5);
        assert_eq!(result, ComplexMatchResult::Single(v_str("chair")));

        let result = complex_match_strings_with_fuzzy_threshold("talbe", &strings, 0.5);
        assert_eq!(result, ComplexMatchResult::Single(v_str("table")));
    }

    #[test]
    fn test_fuzzy_match_precedence() {
        let strings = vec![v_str("foobar"), v_str("foo"), v_str("fooo")];

        // Exact match should still win over fuzzy matches
        let result = complex_match_strings_with_fuzzy_threshold("foo", &strings, 0.5);
        assert_eq!(result, ComplexMatchResult::Single(v_str("foo")));

        // But fuzzy should work when no exact/prefix/substring match
        let result = complex_match_strings_with_fuzzy_threshold("foox", &strings, 0.5);
        // Both "foo" and "fooo" are distance 1 from "foox", so we get multiple matches
        assert_eq!(
            result,
            ComplexMatchResult::Multiple(vec![v_str("foo"), v_str("fooo")])
        );
    }

    #[test]
    fn test_fuzzy_match_with_ordinal() {
        let strings = vec![v_str("lamp"), v_str("lump"), v_str("limp")];

        // Should find second fuzzy match (with fuzzy enabled)
        let result = complex_match_strings_with_fuzzy_threshold("second lmop", &strings, 0.5);
        // "lmop" should fuzzy match multiple items, so "second" should pick the second one
        assert!(matches!(result, ComplexMatchResult::Single(_)));
    }

    #[test]
    fn test_fuzzy_match_distance_threshold() {
        let strings = vec![v_str("lamp"), v_str("table")];

        // Too many changes - should not match even with fuzzy
        let result = complex_match_strings_with_fuzzy_threshold("xyz", &strings, 0.5);
        assert_eq!(result, ComplexMatchResult::NoMatch);

        // Just within threshold for short words (distance 1) - this is a prefix match
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

        // Should fuzzy match "lamp" with "lammp" (with fuzzy enabled)
        let result = complex_match_objects_keys_with_fuzzy_threshold("lammp", &objects, &keys, 0.5);
        assert_eq!(result, ComplexMatchResult::Single(v_int(1)));

        // Should fuzzy match "table" with "tabel"
        let result = complex_match_objects_keys_with_fuzzy_threshold("tabel", &objects, &keys, 0.5);
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

        // Test new threshold function
        let result = complex_match_strings_with_fuzzy_threshold("lammp", &strings, 0.5);
        assert_eq!(result, ComplexMatchResult::Single(v_str("lamp")));

        let result = complex_match_strings_with_fuzzy_threshold("lammp", &strings, 0.0);
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

        // Test new threshold function
        let result = complex_match_objects_keys_with_fuzzy_threshold("lammp", &objects, &keys, 0.5);
        assert_eq!(result, ComplexMatchResult::Single(v_int(1)));

        let result = complex_match_objects_keys_with_fuzzy_threshold("lammp", &objects, &keys, 0.0);
        assert_eq!(result, ComplexMatchResult::NoMatch);
    }

    #[test]
    fn test_ordinal_across_tiers() {
        // Test that ordinals count across all tiers combined
        // "foo" = exact match (position 1), "foobar"/"foobaz" = prefix matches (positions 2-3)
        let strings = vec![v_str("foo"), v_str("foobar"), v_str("foobaz")];

        // "2.foo" should return "foobar" (2nd overall: 1 exact + 1st prefix)
        let result = complex_match_strings("2.foo", &strings);
        assert_eq!(result, ComplexMatchResult::Single(v_str("foobar")));

        // "second foo" should return "foobar" (same as "2.foo")
        let result = complex_match_strings("second foo", &strings);
        assert_eq!(result, ComplexMatchResult::Single(v_str("foobar")));

        // "2nd foo" should return "foobar" (same as "2.foo")
        let result = complex_match_strings("2nd foo", &strings);
        assert_eq!(result, ComplexMatchResult::Single(v_str("foobar")));

        // "3.foo" should return "foobaz" (3rd overall: 1 exact + 2nd prefix)
        let result = complex_match_strings("3.foo", &strings);
        assert_eq!(result, ComplexMatchResult::Single(v_str("foobaz")));
    }

    #[test]
    fn test_ordinal_out_of_range() {
        // Test that ordinal beyond available matches returns NoMatch
        let strings = vec![v_str("foo"), v_str("bar"), v_str("baz")];

        // "2.foo" should return NoMatch since there's only 1 match for "foo"
        let result = complex_match_strings("2.foo", &strings);
        assert_eq!(result, ComplexMatchResult::NoMatch);

        // "4.foo" with three matches should return NoMatch
        let strings = vec![v_str("foo"), v_str("foobar"), v_str("foobaz")];
        let result = complex_match_strings("4.foo", &strings);
        assert_eq!(result, ComplexMatchResult::NoMatch);
    }

    #[test]
    fn test_dot_notation_parsing() {
        // Test that "N.subject" format is parsed correctly
        assert_eq!(parse_input_token("2.foo"), (2, "foo".to_string()));
        assert_eq!(parse_input_token("10.lamp"), (10, "lamp".to_string()));
        assert_eq!(parse_input_token("1.bar baz"), (1, "bar baz".to_string()));

        // Test that non-ordinal dots are not misinterpreted
        assert_eq!(parse_input_token("foo.bar"), (0, "foo.bar".to_string()));
        assert_eq!(parse_input_token("a.b"), (0, "a.b".to_string()));
    }

    #[test]
    fn test_complex_match_strings_all() {
        let strings = vec![v_str("foo"), v_str("bar"), v_str("foobar")];

        // Should return all matches from the best tier
        let result = complex_match_strings_all("foo", &strings, 0.5);
        assert_eq!(result, vec![v_str("foo")]);

        // When there's no exact match, return prefix matches
        let strings = vec![v_str("foobar"), v_str("food"), v_str("bar")];
        let result = complex_match_strings_all("foo", &strings, 0.5);
        assert_eq!(result, vec![v_str("foobar"), v_str("food")]);

        // Test with multiple exact matches
        let strings = vec![v_str("foo"), v_str("foo"), v_str("bar")];
        let result = complex_match_strings_all("foo", &strings, 0.5);
        assert_eq!(result, vec![v_str("foo"), v_str("foo")]);

        // Test ordinal selection - returns Nth match as single-element list
        let strings = vec![v_str("foobar"), v_str("foobaz")];
        let result = complex_match_strings_all("1.foo", &strings, 0.0);
        assert_eq!(result, vec![v_str("foobar")]);

        let result = complex_match_strings_all("2.foo", &strings, 0.0);
        assert_eq!(result, vec![v_str("foobaz")]);

        // Ordinal with word form
        let result = complex_match_strings_all("first foo", &strings, 0.0);
        assert_eq!(result, vec![v_str("foobar")]);

        let result = complex_match_strings_all("2nd foo", &strings, 0.0);
        assert_eq!(result, vec![v_str("foobaz")]);

        // Ordinal out of range returns empty list
        let result = complex_match_strings_all("3.foo", &strings, 0.0);
        assert_eq!(result, Vec::<Var>::new());
    }

    #[test]
    fn test_parse_all_tiers_prefix() {
        // "all " prefix should return the subject
        assert_eq!(parse_all_tiers_prefix("all foo"), Some("foo".to_string()));
        assert_eq!(
            parse_all_tiers_prefix("all bar baz"),
            Some("bar baz".to_string())
        );

        // "*." prefix should return the subject
        assert_eq!(parse_all_tiers_prefix("*.foo"), Some("foo".to_string()));
        assert_eq!(
            parse_all_tiers_prefix("*.bar baz"),
            Some("bar baz".to_string())
        );

        // Just "all" or "*." with no subject should return None (treat as literal)
        assert_eq!(parse_all_tiers_prefix("all"), None);
        assert_eq!(parse_all_tiers_prefix("all "), None); // Empty after stripping
        assert_eq!(parse_all_tiers_prefix("*."), None);

        // No prefix should return None
        assert_eq!(parse_all_tiers_prefix("foo"), None);
        assert_eq!(parse_all_tiers_prefix("allover"), None); // Not "all " prefix
    }

    #[test]
    fn test_complex_match_strings_all_tiers() {
        // Test that "all foo" returns matches from ALL tiers in order
        // foo = exact, foobar = prefix, bofooer = contains
        let strings = vec![v_str("foo"), v_str("foobar"), v_str("bofooer")];
        let result = complex_match_strings_all_tiers("foo", &strings, 0.0);
        // Should return exact, then prefix, then contains
        assert_eq!(result, vec![v_str("foo"), v_str("foobar"), v_str("bofooer")]);

        // Test with no exact match - should still get prefix and contains
        let strings = vec![v_str("foobar"), v_str("bofooer"), v_str("bar")];
        let result = complex_match_strings_all_tiers("foo", &strings, 0.0);
        assert_eq!(result, vec![v_str("foobar"), v_str("bofooer")]);

        // Test with fuzzy matching enabled
        let strings = vec![v_str("foo"), v_str("foobar"), v_str("bofooer"), v_str("foi")];
        let result = complex_match_strings_all_tiers("foo", &strings, 0.5);
        // foi is 1 edit from foo, so fuzzy should match it
        assert_eq!(
            result,
            vec![v_str("foo"), v_str("foobar"), v_str("bofooer"), v_str("foi")]
        );

        // Test empty subject returns empty
        let result = complex_match_strings_all_tiers("", &strings, 0.0);
        assert_eq!(result, Vec::<Var>::new());
    }

    #[test]
    fn test_all_as_literal_matches_allover() {
        // When token is just "all" (no space after), treat as literal
        let strings = vec![v_str("allover"), v_str("foobar"), v_str("bar")];

        // "all" should match "allover" as a prefix match (literal matching)
        let result = complex_match_strings_all("all", &strings, 0.0);
        assert_eq!(result, vec![v_str("allover")]);

        // Same for all_tiers
        let result = complex_match_strings_all_tiers("all", &strings, 0.0);
        assert_eq!(result, vec![v_str("allover")]);
    }

    #[test]
    fn test_star_dot_prefix() {
        // "*.foo" should behave same as "all foo"
        let strings = vec![v_str("foo"), v_str("foobar"), v_str("bofooer")];
        let result = complex_match_strings_all_tiers("foo", &strings, 0.0);
        assert_eq!(result, vec![v_str("foo"), v_str("foobar"), v_str("bofooer")]);
    }

    #[test]
    fn test_complex_match_objects_keys_all_tiers() {
        let objects = vec![v_int(1), v_int(2), v_int(3)];
        let keys = vec![v_str("foo"), v_str("foobar"), v_str("bofooer")];

        // Should return all matching objects from all tiers
        let result = complex_match_objects_keys_all_tiers("foo", &objects, &keys, 0.0);
        assert_eq!(result, vec![v_int(1), v_int(2), v_int(3)]);

        // With no exact match
        let objects = vec![v_int(1), v_int(2), v_int(3)];
        let keys = vec![v_str("foobar"), v_str("bofooer"), v_str("bar")];
        let result = complex_match_objects_keys_all_tiers("foo", &objects, &keys, 0.0);
        assert_eq!(result, vec![v_int(1), v_int(2)]);
    }

    #[test]
    fn test_complex_match_objects_keys_all_ordinal() {
        let objects = vec![v_int(1), v_int(2), v_int(3)];
        let keys = vec![v_str("foobar"), v_str("foobaz"), v_str("bar")];

        // Ordinal selects Nth match
        let result = complex_match_objects_keys_all("1.foo", &objects, &keys, 0.0);
        assert_eq!(result, vec![v_int(1)]);

        let result = complex_match_objects_keys_all("2.foo", &objects, &keys, 0.0);
        assert_eq!(result, vec![v_int(2)]);

        // Ordinal out of range returns empty list
        let result = complex_match_objects_keys_all("3.foo", &objects, &keys, 0.0);
        assert_eq!(result, Vec::<Var>::new());

        // Without ordinal returns all matches
        let result = complex_match_objects_keys_all("foo", &objects, &keys, 0.0);
        assert_eq!(result, vec![v_int(1), v_int(2)]);
    }
}
