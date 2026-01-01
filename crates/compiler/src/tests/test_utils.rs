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

//! Testing utilities for parser validation and debugging

use crate::{CompileOptions, compile};
use moor_common::model::CompileError;

/// Result of parsing for test comparisons
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ParseTestResult {
    pub code: String,
    pub result: Result<(), CompileError>,
}

impl ParseTestResult {
    /// Create a new parse test result
    pub fn new(code: &str) -> Self {
        let result = compile(code, CompileOptions::default()).map(|_| ());
        Self {
            code: code.to_string(),
            result,
        }
    }

    /// Check if parsing succeeded
    pub fn is_ok(&self) -> bool {
        self.result.is_ok()
    }

    /// Check if parsing failed
    pub fn is_err(&self) -> bool {
        self.result.is_err()
    }

    /// Get the error if parsing failed
    pub fn error(&self) -> Option<&CompileError> {
        self.result.as_ref().err()
    }

    /// Print a formatted test result
    pub fn print_result(&self, test_name: &str) {
        println!("{test_name}: {}", self.code);
        match &self.result {
            Ok(_) => println!("  ✓ Success"),
            Err(e) => println!("  ✗ Error: {e}"),
        }
    }
}

/// Test multiple code samples and return results
#[allow(dead_code)]
pub fn batch_parse_test(samples: &[(&str, &str)]) -> Vec<(String, ParseTestResult)> {
    samples
        .iter()
        .map(|(name, code)| (name.to_string(), ParseTestResult::new(code)))
        .collect()
}

/// Print results of batch parsing tests
#[allow(dead_code)]
pub fn print_batch_results(results: &[(String, ParseTestResult)]) {
    let total = results.len();
    let passed = results.iter().filter(|(_, r)| r.is_ok()).count();
    let failed = total - passed;

    println!("\nTest Results Summary:");
    println!("Total: {total}, Passed: {passed}, Failed: {failed}");
    println!("\nDetailed Results:");

    for (name, result) in results {
        result.print_result(name);
    }

    if failed > 0 {
        println!("\nFailed Tests:");
        for (name, result) in results.iter().filter(|(_, r)| r.is_err()) {
            println!("  - {name}: {:?}", result.error());
        }
    }
}

/// Common test cases for operand parsing
#[allow(dead_code)]
pub fn operand_test_cases() -> Vec<(&'static str, &'static str)> {
    vec![
        ("builtin_call_scatter", "{_, _, perms, @_} = callers()[2];"),
        ("builtin_call_simple", "result = callers()[2];"),
        ("builtin_call_args", "x = length(args);"),
        ("builtin_call_no_args", "{a, b} = time();"),
        (
            "sysprop_call_scatter",
            "{msg, parties} = $pronoun_sub:flatten_message(msg, parties);",
        ),
        (
            "sysprop_call_simple",
            "result = $pronoun_sub:flatten_message(msg, parties);",
        ),
        ("sysprop_simple", "x = $some_prop;"),
        ("sysprop_scatter", "{a, b} = $some_prop;"),
        ("mixed_expression", "result = callers()[2] + $some_prop;"),
        ("mixed_operands", "{a, b} = $obj:verb(callers()[1]);"),
    ]
}
