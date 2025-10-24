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

mod bitarray;
mod bitenum;
mod bitset;
mod cache_padded;
mod counter;
mod perf_counter;
mod trace_events;
mod xdg;

pub use bitarray::BitArray;
pub use bitenum::BitEnum;
pub use bitset::*;
pub use cache_padded::CachePadded;
pub use counter::ConcurrentCounter;
pub use perf_counter::{PerfCounter, PerfTimerGuard};
pub use trace_events::*;
pub use xdg::*;

/// Check single verb pattern for matches following LambdaMOO semantics.
/// This exactly tries to mirror the C verbcasecmp() state machine from LambdaMOO's utils.c.
/// (Unlike LambdaMOO, multiple names are handled further up the call chain, not inside this match function.)
///
/// Wildcard behavior:
/// - `*` at the end: matches any string that begins with the prefix (e.g., "foo*" matches "foo", "foobar")
/// - `*` in the middle: matches any prefix of the full pattern that's at least as long as the part before the star
///   (e.g., "foo*bar" matches "foo", "foob", "fooba", "foobar")
/// - Leading `*` are consumed but do NOT act as wildcards - exact matching resumes after them
#[must_use]
pub fn verbcasecmp(pattern: &str, word: &str) -> bool {
    if pattern == word {
        return true;
    }

    let mut pattern_chars = pattern.chars().peekable();
    let mut word_chars = word.chars().peekable();

    #[derive(PartialEq)]
    enum StarType {
        None,
        Inner, // * in the middle of pattern
        End,   // * at the end of pattern
    }

    let mut star = StarType::None;
    let mut has_matched_non_star = false;

    // Main matching loop - mirrors C verbcasecmp state machine
    loop {
        // Handle consecutive asterisks
        while pattern_chars.peek() == Some(&'*') {
            pattern_chars.next();
            star = if pattern_chars.peek().is_none() {
                StarType::End
            } else {
                // Only treat as inner wildcard if we've matched non-star characters before
                if has_matched_non_star {
                    StarType::Inner
                } else {
                    StarType::None // Leading asterisks don't count as wildcards
                }
            };
        }

        // Check if we can continue matching
        match (pattern_chars.peek(), word_chars.peek()) {
            (None, _) => break,       // End of pattern
            (Some(_), None) => break, // End of word but pattern continues
            (Some(p), Some(w)) if chars_match_case_insensitive(*p, *w) => {
                // Characters match, advance both
                pattern_chars.next();
                word_chars.next();
                has_matched_non_star = true;
            }
            _ => break, // Characters don't match
        }
    }

    // Determine if we have a match based on what's left
    match (word_chars.peek(), star) {
        (None, StarType::None) => pattern_chars.peek().is_none(), // Exact match
        (None, _) => true,                // Word consumed and we had a wildcard
        (Some(_), StarType::End) => true, // Trailing wildcard matches remaining word
        _ => false,                       // No match
    }
}

/// Helper function for case-insensitive character comparison
fn chars_match_case_insensitive(a: char, b: char) -> bool {
    a.eq_ignore_ascii_case(&b)
}

/// Parse a MOO string literal, converting escape sequences to their actual characters.
///
/// Supports the following escape sequences:
/// - Standard escapes: `\"`, `\\`, `\n`, `\r`, `\t`, `\0`, `\'`
/// - Hex escapes: `\xNN` (where NN are hex digits)
/// - Unicode escapes: `\uNNNN` (where NNNN are hex digits)
/// - Unknown escape sequences pass through the character for backward compatibility
///
/// The input string must be surrounded by double quotes.
///
/// # Examples
///
/// ```
/// use moor_common::util::unquote_str;
///
/// assert_eq!(unquote_str(r#""hello""#).unwrap(), "hello");
/// assert_eq!(unquote_str(r#""hello\nworld""#).unwrap(), "hello\nworld");
/// assert_eq!(unquote_str(r#""A is \x41""#).unwrap(), "A is A");
/// assert_eq!(unquote_str(r#""Hello \u0041""#).unwrap(), "Hello A");
/// ```
///
/// # Errors
///
/// Returns an error if:
/// - The string doesn't start and end with quotes
/// - Hex or unicode escape sequences are malformed
/// - The string ends unexpectedly
pub fn unquote_str(s: &str) -> Result<String, String> {
    fn parse_hex_escape(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<char, String> {
        let mut hex_str = String::new();
        for _ in 0..2 {
            match chars.next() {
                Some(c) if c.is_ascii_hexdigit() => hex_str.push(c),
                Some(c) => {
                    return Err(format!("Invalid hex escape: expected hex digit, got '{c}'"));
                }
                None => return Err("Incomplete hex escape: expected 2 hex digits".to_string()),
            }
        }
        let hex_value =
            u8::from_str_radix(&hex_str, 16).map_err(|_| "Invalid hex escape value".to_string())?;
        Ok(hex_value as char)
    }

    fn parse_unicode_escape(
        chars: &mut std::iter::Peekable<std::str::Chars>,
    ) -> Result<char, String> {
        let mut hex_str = String::new();
        for _ in 0..4 {
            match chars.next() {
                Some(c) if c.is_ascii_hexdigit() => hex_str.push(c),
                Some(c) => {
                    return Err(format!(
                        "Invalid unicode escape: expected hex digit, got '{c}'"
                    ));
                }
                None => return Err("Incomplete unicode escape: expected 4 hex digits".to_string()),
            }
        }
        let unicode_value = u32::from_str_radix(&hex_str, 16)
            .map_err(|_| "Invalid unicode escape value".to_string())?;
        char::from_u32(unicode_value).ok_or_else(|| "Invalid unicode code point".to_string())
    }

    let mut output = String::new();
    let mut chars = s.chars().peekable();
    let Some('"') = chars.next() else {
        return Err("Expected \" at beginning of string".to_string());
    };

    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.peek() {
                Some('"') => {
                    // Check if this is the closing quote - if so, this is a backslash at the end
                    chars.next(); // consume the quote
                    if chars.peek().is_some() {
                        // Not the end, there are more characters, so this is an escaped quote
                        output.push('"');
                        continue;
                    } else {
                        // This is the closing quote, backslash at end is ignored
                        return Ok(output);
                    }
                }
                _ => {
                    match chars.next() {
                        Some('\\') => output.push('\\'),
                        Some('\'') => output.push('\''),
                        Some('n') => output.push('\n'),
                        Some('r') => output.push('\r'),
                        Some('t') => output.push('\t'),
                        Some('0') => output.push('\0'),
                        Some('x') => {
                            let hex_char = parse_hex_escape(&mut chars)?;
                            output.push(hex_char);
                        }
                        Some('u') => {
                            let unicode_char = parse_unicode_escape(&mut chars)?;
                            output.push(unicode_char);
                        }
                        Some(c) => {
                            // Backward compatibility: unknown escapes just pass through the character
                            output.push(c);
                        }
                        None => {
                            return Err("Unexpected end of string".to_string());
                        }
                    }
                }
            },
            '"' => {
                if chars.peek().is_some() {
                    return Err("Unexpected \" in string".to_string());
                }
                return Ok(output);
            }
            c => output.push(c),
        }
    }
    Err("Unexpected end of string".to_string())
}

/// Convert a string into a properly escaped MOO string literal.
///
/// This function wraps the input string in double quotes and escapes characters
/// that have special meaning in MOO string literals:
/// - `"` becomes `\"`
/// - `\` becomes `\\`
/// - `\n` becomes `\\n`
/// - `\r` becomes `\\r`
/// - `\t` becomes `\\t`
/// - `\0` becomes `\\0`
/// - Control characters become hex escapes (`\xNN`)
/// - Non-ASCII characters become unicode escapes (`\uNNNN`)
///
/// This function is the inverse of [`unquote_str`].
///
/// # Examples
///
/// ```
/// use moor_common::util::quote_str;
///
/// assert_eq!(quote_str("hello"), r#""hello""#);
/// assert_eq!(quote_str("hello\nworld"), r#""hello\nworld""#);
/// assert_eq!(quote_str(r#"say "hi""#), r#""say \"hi\"""#);
/// assert_eq!(quote_str("path\\file"), r#""path\\file""#);
/// ```
#[must_use]
pub fn quote_str(s: &str) -> String {
    let mut output = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            '\0' => output.push_str("\\0"),
            // For control characters and non-printable characters, use hex escapes
            c if c.is_control() && c != '\n' && c != '\r' && c != '\t' && c != '\0' => {
                output.push_str(&format!("\\x{:02X}", c as u8));
            }
            // For non-ASCII characters that might cause issues, use unicode escapes
            c if !c.is_ascii() && (c as u32) <= 0xFFFF => {
                output.push_str(&format!("\\u{:04X}", c as u32));
            }
            // For characters above the BMP, we could use extended unicode, but for now just pass through
            c => output.push(c),
        }
    }
    output.push('"');
    output
}

pub fn parse_into_words(input: &str) -> Vec<String> {
    // Initialize state variables.
    let mut in_quotes = false;
    let mut previous_char_was_backslash = false;

    // Define the fold function's logic as a closure.
    let accumulate_words = |mut acc: Vec<String>, c| {
        if previous_char_was_backslash {
            // Handle escaped characters.
            if let Some(last_word) = acc.last_mut() {
                last_word.push(c);
            } else {
                acc.push(c.to_string());
            }
            previous_char_was_backslash = false;
        } else if c == '\\' {
            // Mark the next character as escaped.
            previous_char_was_backslash = true;
        } else if c == '"' {
            // Toggle whether we're inside quotes.
            in_quotes = !in_quotes;
        } else if c.is_whitespace() && !in_quotes {
            // Add a new empty string to the accumulator if we've reached a whitespace boundary.
            if let Some(last_word) = acc.last()
                && !last_word.is_empty()
            {
                acc.push(String::new());
            }
        } else {
            // Append the current character to the last word in the accumulator,
            // or create a new word if there isn't one yet.
            if let Some(last_word) = acc.last_mut() {
                last_word.push(c);
            } else {
                acc.push(c.to_string());
            }
        }
        acc
    };

    // Use the fold function to accumulate the words in the input string.
    let words = input.chars().fold(vec![], accumulate_words);

    // Filter out empty strings and return the result.
    words.into_iter().filter(|w| !w.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use crate::util::{quote_str, unquote_str, verbcasecmp};

    #[test]
    fn test_string_quote() {
        assert_eq!(quote_str("foo"), r#""foo""#);
        assert_eq!(quote_str(r#"foo"bar"#), r#""foo\"bar""#);
        assert_eq!(quote_str("foo\\bar"), r#""foo\\bar""#);
        assert_eq!(quote_str("hello\nworld"), r#""hello\nworld""#);
        assert_eq!(quote_str("hello\tworld"), r#""hello\tworld""#);
        assert_eq!(quote_str("hello\rworld"), r#""hello\rworld""#);
        assert_eq!(quote_str("hello\0world"), r#""hello\0world""#);
        assert_eq!(quote_str("hello'world"), r#""hello'world""#);
    }

    #[test]
    fn test_string_unquote() {
        assert_eq!(unquote_str(r#""foo""#).unwrap(), "foo");
        assert_eq!(unquote_str(r#""foo\"bar""#).unwrap(), r#"foo"bar"#);
        assert_eq!(unquote_str(r#""foo\\bar""#).unwrap(), r"foo\bar");
    }

    #[test]
    fn test_string_unquote_standard_escapes() {
        // Test standard escape sequences
        assert_eq!(unquote_str(r#""hello\nworld""#).unwrap(), "hello\nworld");
        assert_eq!(unquote_str(r#""hello\tworld""#).unwrap(), "hello\tworld");
        assert_eq!(unquote_str(r#""hello\rworld""#).unwrap(), "hello\rworld");
        assert_eq!(unquote_str(r#""hello\0world""#).unwrap(), "hello\0world");
        assert_eq!(unquote_str(r#""hello\'world""#).unwrap(), "hello'world");

        // Test combinations
        assert_eq!(
            unquote_str(r#""line1\nline2\tindented""#).unwrap(),
            "line1\nline2\tindented"
        );
    }

    #[test]
    fn test_string_unquote_hex_escapes() {
        // Test hex escape sequences
        assert_eq!(unquote_str(r#""A is \x41""#).unwrap(), "A is A");
        assert_eq!(unquote_str(r#""\x48\x65\x6C\x6C\x6F""#).unwrap(), "Hello");
        assert_eq!(unquote_str(r#""\x00\xFF""#).unwrap(), "\0\u{FF}");

        // Test lowercase and uppercase hex
        assert_eq!(unquote_str(r#""\x4a\x4A""#).unwrap(), "JJ");
    }

    #[test]
    fn test_string_unquote_unicode_escapes() {
        // Test unicode escape sequences
        assert_eq!(unquote_str(r#""Hello \u0041""#).unwrap(), "Hello A");
        assert_eq!(
            unquote_str(r#""\u0048\u0065\u006C\u006C\u006F""#).unwrap(),
            "Hello"
        );
        assert_eq!(unquote_str(r#""Smile: \u263A""#).unwrap(), "Smile: ☺");

        // Test various Unicode ranges
        assert_eq!(unquote_str(r#""\u00E9\u00E8\u00EA""#).unwrap(), "éèê"); // Latin
        assert_eq!(unquote_str(r#""\u03B1\u03B2\u03B3""#).unwrap(), "αβγ"); // Greek
    }

    #[test]
    fn test_string_roundtrip() {
        // Test that quote_str and unquote_str are inverses
        let test_cases = vec![
            "hello",
            "hello\nworld",
            "hello\tworld",
            "hello\rworld",
            "hello\0world",
            "hello\"world",
            "hello\\world",
            "hello'world",
            "hello\x01world", // control character
            "hello\x7fworld", // DEL character
        ];

        for test_case in test_cases {
            let quoted = quote_str(test_case);
            let unquoted = unquote_str(&quoted).unwrap();
            assert_eq!(test_case, unquoted, "Roundtrip failed for: {test_case:?}");

            // For single quote case, verify it's not escaped
            if test_case.contains('\'') {
                assert!(
                    !quoted.contains("\\'"),
                    "Single quotes should not be escaped: {quoted}"
                );
            }
        }
    }

    #[test]
    fn test_string_unquote_error_cases() {
        // Test malformed hex escapes
        assert!(unquote_str(r#""\x""#).is_err());
        assert!(unquote_str(r#""\x4""#).is_err());
        assert!(unquote_str(r#""\xGG""#).is_err());
        assert!(unquote_str(r#""\x4G""#).is_err());

        // Test malformed unicode escapes
        assert!(unquote_str(r#""\u""#).is_err());
        assert!(unquote_str(r#""\u123""#).is_err());
        assert!(unquote_str(r#""\uGGGG""#).is_err());
        assert!(unquote_str(r#""\u123G""#).is_err());

        // Test truncated escapes at end of string
        assert!(unquote_str(r#""\x4""#).is_err());
        assert!(unquote_str(r#""\u123""#).is_err());
    }

    #[test]
    fn test_string_unquote_backward_compatibility() {
        // Test that unknown escape sequences still work as before (pass through the character)
        assert_eq!(unquote_str(r#""foo\bbar""#).unwrap(), "foobbar");
        assert_eq!(unquote_str(r#""foo\fbar""#).unwrap(), "foofbar");
        assert_eq!(unquote_str(r#""foo\vbar""#).unwrap(), "foovbar");
        assert_eq!(unquote_str(r#""foo\zbar""#).unwrap(), "foozbar");

        // Test edge cases
        assert_eq!(unquote_str(r#""foo\""#).unwrap(), "foo"); // backslash at end becomes empty
    }

    #[test]
    fn test_single_quote_handling() {
        // Test that unescaped single quotes work fine
        assert_eq!(unquote_str(r#""hello'world""#).unwrap(), "hello'world");
        // Test that escaped single quotes also work (for backward compatibility)
        assert_eq!(unquote_str(r#""hello\'world""#).unwrap(), "hello'world");
    }

    #[test]
    fn test_verb_match() {
        // full match
        assert!(verbcasecmp("give", "give"));
        // * matches anything
        assert!(verbcasecmp("*", "give"));
        // full match w wildcard
        assert!(verbcasecmp("g*ive", "give"));

        // partial match after wildcard, this is permitted in MOO
        assert!(verbcasecmp("g*ive", "giv"));

        // negative
        assert!(!verbcasecmp("g*ive", "gender"));

        // From reference manual
        // If the name contains a single star, however, then the name matches any prefix of itself
        // that is at least as long as the part before the star. For example, the verb-name
        // `foo*bar' matches any of the strings `foo', `foob', `fooba', or `foobar'; note that the
        // star itself is not considered part of the name.
        let foobar = "foo*bar";
        assert!(verbcasecmp(foobar, "foo"));
        assert!(verbcasecmp(foobar, "foob"));
        assert!(verbcasecmp(foobar, "fooba"));
        assert!(verbcasecmp(foobar, "foobar"));
        assert!(!verbcasecmp(foobar, "fo"));
        assert!(!verbcasecmp(foobar, "foobaar"));

        // If the verb name ends in a star, then it matches any string that begins with the part
        // before the star. For example, the verb-name `foo*' matches any of the strings `foo',
        // `foobar', `food', or `foogleman', among many others. As a special case, if the verb-name
        // is `*' (i.e., a single star all by itself), then it matches anything at all.
        let foostar = "foo*";
        assert!(verbcasecmp(foostar, "foo"));
        assert!(verbcasecmp(foostar, "foobar"));
        assert!(verbcasecmp(foostar, "food"));
        assert!(!verbcasecmp(foostar, "fo"));

        // Regression for 'do_object' matching 'do'
        assert!(!verbcasecmp("do", "do_object"));
    }

    #[test]
    fn test_verb_match_basic_wildcard() {
        // First test the basic case that should work
        assert!(verbcasecmp("ps*c", "psc"), "ps*c should match psc");
    }

    #[test]
    fn test_verb_match_regressions() {
        // Regression test for pronoun verb patterns like "ps*c po*c pr*c pp*c pq*c"
        // These should match pronoun verbs like "psc", "Psc", "PSC", etc.
        assert!(
            verbcasecmp("ps*c", "Psc"),
            "ps*c should match Psc (case insensitive)"
        );
        assert!(
            verbcasecmp("ps*c", "PSC"),
            "ps*c should match PSC (case insensitive)"
        );
        assert!(
            verbcasecmp("ps*c", "psc"),
            "ps*c should match psc (full pattern)"
        );
        assert!(
            !verbcasecmp("ps*c", "psomc"),
            "ps*c should NOT match psomc (longer than pattern)"
        );
        assert!(
            !verbcasecmp("ps*c", "psc_extra"),
            "ps*c should not match psc_extra"
        );
        assert!(
            verbcasecmp("ps*c", "ps"),
            "ps*c should match ps (partial prefix)"
        );

        // Test other pronoun patterns
        assert!(verbcasecmp("po*c", "poc"), "po*c should match poc");
        assert!(
            verbcasecmp("po*c", "Poc"),
            "po*c should match Poc (case insensitive)"
        );
        assert!(verbcasecmp("pr*c", "prc"), "pr*c should match prc");
        assert!(verbcasecmp("pp*c", "ppc"), "pp*c should match ppc");
        assert!(verbcasecmp("pq*c", "pqc"), "pq*c should match pqc");

        // Test patterns without wildcards for case insensitivity
        assert!(verbcasecmp("psu", "psu"), "psu should match psu");
        assert!(
            verbcasecmp("psu", "PSU"),
            "psu should match PSU (case insensitive)"
        );
        assert!(
            verbcasecmp("psu", "Psu"),
            "psu should match Psu (case insensitive)"
        );

        // Mixed case pattern and candidate
        assert!(
            verbcasecmp("PS*C", "psc"),
            "PS*C should match psc (case insensitive)"
        );
        assert!(
            verbcasecmp("Ps*C", "PSC"),
            "Ps*C should match PSC (case insensitive)"
        );
    }

    #[test]
    fn test_verb_match_leading_asterisks() {
        // Test LambdaMOO behavior: leading asterisks don't work as wildcards
        // They are consumed but then exact matching resumes

        // Leading single asterisk - should only match exact suffix
        assert!(
            verbcasecmp("*p", "p"),
            "*p should match p (exact match after consuming *)"
        );
        assert!(
            !verbcasecmp("*p", "ap"),
            "*p should NOT match ap (leading * is not a wildcard)"
        );
        assert!(
            !verbcasecmp("*p", "anythingp"),
            "*p should NOT match anythingp"
        );

        // Leading multiple asterisks - same behavior
        assert!(verbcasecmp("**p", "p"), "**p should match p");
        assert!(!verbcasecmp("**p", "ap"), "**p should NOT match ap");

        // Leading asterisk with longer pattern
        assert!(verbcasecmp("*xyz", "xyz"), "*xyz should match xyz");
        assert!(
            !verbcasecmp("*xyz", "abcxyz"),
            "*xyz should NOT match abcxyz"
        );

        // Verify trailing asterisks still work as wildcards
        assert!(verbcasecmp("test*", "test"), "test* should match test");
        assert!(
            verbcasecmp("test*", "testfoo"),
            "test* should match testfoo"
        );

        // Verify internal asterisks still work as wildcards
        assert!(verbcasecmp("foo*bar", "foo"), "foo*bar should match foo");
        assert!(
            verbcasecmp("foo*bar", "foobar"),
            "foo*bar should match foobar"
        );

        // Mixed: leading asterisk followed by internal asterisk pattern
        assert!(
            verbcasecmp("*foo*bar", "foobar"),
            "*foo*bar should match foobar (leading * ignored, internal * works)"
        );
        assert!(
            verbcasecmp("*foo*bar", "foo"),
            "*foo*bar should match foo (leading * ignored, internal * works)"
        );
        assert!(
            !verbcasecmp("*foo*bar", "xfoobar"),
            "*foo*bar should NOT match xfoobar (leading * not a wildcard)"
        );
    }
}
