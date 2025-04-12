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

/// Check `names` for matches with wildcard prefixes.
/// e.g. "dname*c" will match for any of 'dname', 'dnamec'
#[must_use]
pub fn verbname_cmp(vname: &str, candidate: &str) -> bool {
    let mut v_iter = vname.chars().peekable();
    let mut w_iter = candidate.chars().peekable();

    let mut had_wildcard = false;
    while let Some(v_c) = v_iter.next() {
        if v_c == '*' {
            if v_iter.peek().is_none() || w_iter.peek().is_none() {
                return true;
            }
            had_wildcard = true;
        } else {
            match w_iter.next() {
                None => {
                    return had_wildcard;
                }
                Some(w_c) if w_c != v_c => {
                    return false;
                }
                _ => {}
            }
        }
    }
    if w_iter.peek().is_some() || v_iter.peek().is_some() {
        return false;
    }
    true
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
    use crate::util::{quote_str, verbname_cmp};

    #[test]
    fn test_string_quote() {
        assert_eq!(quote_str("foo"), r#""foo""#);
        assert_eq!(quote_str(r#"foo"bar"#), r#""foo\"bar""#);
        assert_eq!(quote_str("foo\\bar"), r#""foo\\bar""#);
    }

    #[test]
    fn test_verb_match() {
        // full match
        assert!(verbname_cmp("give", "give"));
        // * matches anything
        assert!(verbname_cmp("*", "give"));
        // full match w wildcard
        assert!(verbname_cmp("g*ive", "give"));

        // partial match after wildcard, this is permitted in MOO
        assert!(verbname_cmp("g*ive", "giv"));

        // negative
        assert!(!verbname_cmp("g*ive", "gender"));

        // From reference manual
        // If the name contains a single star, however, then the name matches any prefix of itself
        // that is at least as long as the part before the star. For example, the verb-name
        // `foo*bar' matches any of the strings `foo', `foob', `fooba', or `foobar'; note that the
        // star itself is not considered part of the name.
        let foobar = "foo*bar";
        assert!(verbname_cmp(foobar, "foo"));
        assert!(verbname_cmp(foobar, "foob"));
        assert!(verbname_cmp(foobar, "fooba"));
        assert!(verbname_cmp(foobar, "foobar"));
        assert!(!verbname_cmp(foobar, "fo"));
        assert!(!verbname_cmp(foobar, "foobaar"));

        // If the verb name ends in a star, then it matches any string that begins with the part
        // before the star. For example, the verb-name `foo*' matches any of the strings `foo',
        // `foobar', `food', or `foogleman', among many others. As a special case, if the verb-name
        // is `*' (i.e., a single star all by itself), then it matches anything at all.
        let foostar = "foo*";
        assert!(verbname_cmp(foostar, "foo"));
        assert!(verbname_cmp(foostar, "foobar"));
        assert!(verbname_cmp(foostar, "food"));
        assert!(!verbname_cmp(foostar, "fo"));

        // Regression for 'do_object' matching 'do'
        assert!(!verbname_cmp("do", "do_object"));
    }
}
