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

//! Testing utilities for parser validation and debugging
//!
//! This module provides comprehensive testing utilities for validating parser implementations,
//! comparing parser outputs, and debugging parsing issues. It includes utilities for:
//!
//! - Parser comparison and validation across different implementations
//! - Error analysis and debugging tools
//! - Test result formatting and reporting
//! - Performance and compatibility testing

#[cfg(feature = "tree-sitter-parser")]
use crate::parsers::parser_trait::get_parser_by_name;
#[cfg(feature = "tree-sitter-parser")]
use crate::{CompileOptions, compile, compile_with_tree_sitter};
use moor_common::model::CompileError;

/// Result of parsing with both parsers for comparison
#[derive(Debug, Clone)]
pub struct ParserComparisonResult {
    pub code: String,
    pub tree_sitter_result: Result<(), CompileError>,
    pub pest_result: Result<(), CompileError>,
    pub parsers_agree: bool,
}

impl ParserComparisonResult {
    /// Print a formatted comparison result
    pub fn print_result(&self, test_name: &str) {
        println!("{}: {}", test_name, self.code);

        match &self.tree_sitter_result {
            Ok(_) => println!("  Tree-sitter: ✓ Success"),
            Err(e) => println!("  Tree-sitter: ✗ Error: {e}"),
        }

        match &self.pest_result {
            Ok(_) => println!("  PEST: ✓ Success"),
            Err(e) => println!("  PEST: ✗ Error: {e}"),
        }

        if !self.parsers_agree {
            println!("  ⚠️  Parser disagreement detected!");
        }
    }
}

/// Compare tree-sitter and PEST parsing results
#[cfg(feature = "tree-sitter-parser")]
pub fn compare_parsers(code: &str) -> ParserComparisonResult {
    let tree_sitter_result = compile_with_tree_sitter(code, CompileOptions::default()).map(|_| ());
    let pest_result = compile(code, CompileOptions::default()).map(|_| ());

    let parsers_agree = match (&tree_sitter_result, &pest_result) {
        (Ok(_), Ok(_)) => true,
        (Err(_), Err(_)) => true,
        _ => false,
    };

    ParserComparisonResult {
        code: code.to_string(),
        tree_sitter_result,
        pest_result,
        parsers_agree,
    }
}

/// Test multiple code samples with both parsers
#[cfg(feature = "tree-sitter-parser")]
pub fn test_parser_compatibility(test_cases: &[&str]) -> Vec<ParserComparisonResult> {
    test_cases
        .iter()
        .map(|code| compare_parsers(code))
        .collect()
}

/// Generate character-by-character position map for error debugging
pub fn debug_char_positions(code: &str) -> String {
    code.chars()
        .enumerate()
        .map(|(i, c)| format!("{i}:{c}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Common MOOT test patterns
pub fn moot_test_cases() -> Vec<&'static str> {
    vec![
        "return 42;",
        "eval();",
        "return 1 + 2 + 3;",
        "return player;",
        "return {player.programmer, player.wizard, player.name};",
        r#"return "hello world";"#,
    ]
}

/// MOOT debug format test cases (with appended debug strings)
pub fn moot_debug_format_cases() -> Vec<&'static str> {
    vec![
        r#"return 42; "moot-line:2";"#,
        r#"eval(); "moot-line:8";"#,
        r#"return 1 + 2 + 3; "moot-line:12";"#,
        r#"create(); "moot-line:4";"#,
        r#"return 5 + 5; "moot-line:3";"#,
    ]
}

/// Validate tree-sitter parser against all available parser interfaces
#[cfg(feature = "tree-sitter-parser")]
pub fn test_all_tree_sitter_interfaces(
    code: &str,
) -> Vec<(&'static str, Result<(), CompileError>)> {
    let mut results = vec![];

    // Standard tree-sitter
    results.push((
        "tree-sitter",
        compile_with_tree_sitter(code, CompileOptions::default()).map(|_| ()),
    ));

    // Parser selection interface
    if let Some(parser) = get_parser_by_name("tree-sitter-moot") {
        results.push((
            "tree-sitter-moot",
            parser.compile(code, CompileOptions::default()).map(|_| ()),
        ));
    }

    results
}

/// Print detailed error position information for debugging
pub fn debug_error_position(code: &str, expected_position: Option<usize>) {
    println!("Code: {code}");
    if let Some(pos) = expected_position {
        println!(
            "Expected error position: {} (character '{}')",
            pos,
            code.chars().nth(pos).unwrap_or('?')
        );
    }
    println!("Character positions: {}", debug_char_positions(code));
}

/// Run comprehensive parser validation test suite
#[cfg(feature = "tree-sitter-parser")]
pub fn run_parser_validation_suite() {
    println!("=== Parser Validation Suite ===\n");

    println!("Testing basic MOOT commands:");
    let basic_results = test_parser_compatibility(&moot_test_cases());
    for (i, result) in basic_results.iter().enumerate() {
        result.print_result(&format!("Test {}", i + 1));
        println!();
    }

    println!("Testing MOOT debug format:");
    let debug_results = test_parser_compatibility(&moot_debug_format_cases());
    for (i, result) in debug_results.iter().enumerate() {
        result.print_result(&format!("MOOT Debug Test {}", i + 1));
        println!();
    }

    // Check for any disagreements
    let mut all_results = basic_results;
    all_results.extend(debug_results);
    let disagreements: Vec<_> = all_results.iter().filter(|r| !r.parsers_agree).collect();

    if disagreements.is_empty() {
        println!("✅ All tests passed - parsers agree on all test cases!");
    } else {
        println!("❌ Found {} parser disagreements:", disagreements.len());
        for disagreement in disagreements {
            println!("  - {}", disagreement.code);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "tree-sitter-parser")]
    fn test_moot_basic_compatibility() {
        let results = crate::test_utils::test_parser_compatibility(&moot_test_cases());

        for result in results {
            if !result.parsers_agree {
                panic!(
                    "Parser disagreement on: {}\nTree-sitter: {:?}\nPEST: {:?}",
                    result.code, result.tree_sitter_result, result.pest_result
                );
            }
        }
    }

    #[test]
    #[cfg(feature = "tree-sitter-parser")]
    fn test_moot_debug_format_compatibility() {
        let results = crate::test_utils::test_parser_compatibility(&moot_debug_format_cases());

        for result in results {
            if !result.parsers_agree {
                panic!(
                    "Parser disagreement on MOOT debug format: {}\nTree-sitter: {:?}\nPEST: {:?}",
                    result.code, result.tree_sitter_result, result.pest_result
                );
            }
        }
    }

    #[test]
    fn test_char_position_debugging() {
        let code = r#"return 42; "moot-line:2";"#;
        let positions = debug_char_positions(code);
        assert!(positions.contains("0:r"));
        assert!(positions.contains("11:\""));
        assert!(positions.contains("12:m"));
    }
}
