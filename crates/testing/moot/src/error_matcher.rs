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

/// Flexible error matching for MOOT tests
pub fn match_error_flexibly(expected: &str, actual: &str) -> bool {
    // If they match exactly, great!
    if expected == actual {
        return true;
    }

    // Check if both are eval error results {0, "message"}
    if let (Some(expected_msg), Some(actual_msg)) = (
        extract_eval_error_message(expected),
        extract_eval_error_message(actual)
    ) {
        return match_error_messages(&expected_msg, &actual_msg);
    }

    false
}

/// Extract error message from eval result format {0, "message"}
fn extract_eval_error_message(s: &str) -> Option<String> {
    let s = s.trim();
    if s.starts_with("{0, \"") && s.ends_with("\"}") {
        let msg = &s[5..s.len()-2];
        Some(msg.to_string())
    } else {
        None
    }
}

/// Match error messages with some flexibility
fn match_error_messages(expected: &str, actual: &str) -> bool {
    // Check for exact match first
    if expected == actual {
        return true;
    }

    // Both should be parse errors
    if !expected.starts_with("Failure to parse program @") ||
       !actual.starts_with("Failure to parse program @") {
        return false;
    }

    // Extract position and message parts
    if let (Some((exp_pos, exp_msg)), Some((act_pos, act_msg))) = (
        parse_error_parts(expected),
        parse_error_parts(actual)
    ) {
        // Positions should match exactly
        if exp_pos != act_pos {
            return false;
        }

        // Messages can have minor variations
        return messages_equivalent(&exp_msg, &act_msg);
    }

    false
}

/// Parse "Failure to parse program @ pos: message" into parts
fn parse_error_parts(error: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = error.splitn(2, ": ").collect();
    if parts.len() != 2 {
        return None;
    }

    let pos_part = parts[0].strip_prefix("Failure to parse program @ ")?;
    Some((pos_part.to_string(), parts[1].to_string()))
}

/// Check if two error messages are semantically equivalent
fn messages_equivalent(expected: &str, actual: &str) -> bool {
    // Exact match
    if expected == actual {
        return true;
    }

    // Common equivalences
    let equivalences = [
        ("expected ident", "expected identifier"),
        ("unexpected token", "syntax error"),
        // Add more as needed
    ];

    for (e1, e2) in &equivalences {
        if (expected == *e1 && actual == *e2) || (expected == *e2 && actual == *e1) {
            return true;
        }
    }

    // Check if messages are similar enough (e.g., both contain "ident" or "identifier")
    let exp_lower = expected.to_lowercase();
    let act_lower = actual.to_lowercase();

    // Both mention identifier/ident
    if (exp_lower.contains("ident") && act_lower.contains("ident")) {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        assert!(match_error_flexibly(
            "{0, \"Failure to parse program @ 1/13: expected ident\"}",
            "{0, \"Failure to parse program @ 1/13: expected ident\"}"
        ));
    }

    #[test]
    fn test_equivalent_messages() {
        assert!(match_error_flexibly(
            "{0, \"Failure to parse program @ 1/13: expected ident\"}",
            "{0, \"Failure to parse program @ 1/13: expected identifier\"}"
        ));
    }

    #[test]
    fn test_different_positions() {
        assert!(!match_error_flexibly(
            "{0, \"Failure to parse program @ 1/13: expected ident\"}",
            "{0, \"Failure to parse program @ 1/14: expected ident\"}"
        ));
    }
}
