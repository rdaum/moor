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
mod perf_counter;

pub use bitarray::BitArray;
pub use bitenum::BitEnum;
pub use bitset::*;
pub use perf_counter::{PerfCounter, PerfTimerGuard};

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

// Simple MOO quasi-C style string quoting.
// In MOO, there's just \" and \\, no \n, \t, etc.
// So no need to produce anything else.
#[must_use]
pub fn quote_str(s: &str) -> String {
    let output = String::from("\"");
    let mut output = s.chars().fold(output, |mut acc, c| {
        match c {
            '"' => acc.push_str("\\\""),
            '\\' => acc.push_str("\\\\"),
            _ => acc.push(c),
        }
        acc
    });
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
            if let Some(last_word) = acc.last() {
                if !last_word.is_empty() {
                    acc.push(String::new());
                }
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
    use crate::util::{quote_str, verbcasecmp};

    #[test]
    fn test_string_quote() {
        assert_eq!(quote_str("foo"), r#""foo""#);
        assert_eq!(quote_str(r#"foo"bar"#), r#""foo\"bar""#);
        assert_eq!(quote_str("foo\\bar"), r#""foo\\bar""#);
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
