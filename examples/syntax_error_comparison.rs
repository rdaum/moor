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

//! Syntax Error Comparison Tool
//!
//! This tool compares syntax error reporting quality across the three available parsers:
//! - CST (Concrete Syntax Tree) parser
//! - PEST parser  
//! - Tree-sitter parser
//!
//! It tests various types of syntax errors and compares how well each parser
//! reports the location and nature of the errors.

#[cfg(feature = "tree-sitter-parser")]
use moor_compiler::compile_with_tree_sitter;
use moor_compiler::{compile, CompileOptions};
use std::collections::HashMap;

#[derive(Debug, Clone)]
struct ErrorTest {
    name: String,
    description: String,
    code: String,
    expected_error_type: String,
    expected_location: Option<(usize, usize)>, // (line, column)
}

#[derive(Debug, Clone)]
struct ErrorResult {
    parser_name: String,
    test_name: String,
    success: bool,
    error_message: Option<String>,
    error_location: Option<(usize, usize)>,
    error_quality_score: u8, // 0-10 scale
}

fn create_error_test_cases() -> Vec<ErrorTest> {
    vec![
        // Basic syntax errors
        ErrorTest {
            name: "unclosed_string".to_string(),
            description: "Unclosed string literal".to_string(),
            code: r#"return "hello world;"#.to_string(),
            expected_error_type: "UnclosedString".to_string(),
            expected_location: Some((1, 8)),
        },
        ErrorTest {
            name: "missing_semicolon".to_string(),
            description: "Missing semicolon at end of statement".to_string(),
            code: r#"x = 42
return x;"#
                .to_string(),
            expected_error_type: "MissingSemicolon".to_string(),
            expected_location: Some((1, 6)),
        },
        ErrorTest {
            name: "unmatched_paren".to_string(),
            description: "Unmatched opening parenthesis".to_string(),
            code: r#"if (x > 0
    return x;
endif"#
                .to_string(),
            expected_error_type: "UnmatchedParen".to_string(),
            expected_location: Some((1, 4)),
        },
        ErrorTest {
            name: "unmatched_bracket".to_string(),
            description: "Unmatched opening bracket".to_string(),
            code: r#"list = [1, 2, 3;
return list;"#
                .to_string(),
            expected_error_type: "UnmatchedBracket".to_string(),
            expected_location: Some((1, 8)),
        },
        ErrorTest {
            name: "unmatched_brace".to_string(),
            description: "Unmatched opening brace".to_string(),
            code: r#"map = ["key" -> "value";
return map;"#
                .to_string(),
            expected_error_type: "UnmatchedBrace".to_string(),
            expected_location: Some((1, 7)),
        },
        ErrorTest {
            name: "invalid_assignment".to_string(),
            description: "Invalid assignment target".to_string(),
            code: r#"42 = x;
return x;"#
                .to_string(),
            expected_error_type: "InvalidAssignment".to_string(),
            expected_location: Some((1, 1)),
        },
        ErrorTest {
            name: "invalid_operator".to_string(),
            description: "Invalid operator usage".to_string(),
            code: r#"x = 1 ++ 2;
return x;"#
                .to_string(),
            expected_error_type: "InvalidOperator".to_string(),
            expected_location: Some((1, 7)),
        },
        ErrorTest {
            name: "malformed_if".to_string(),
            description: "Malformed if statement".to_string(),
            code: r#"if x > 0 then
    return x;
end"#
                .to_string(),
            expected_error_type: "MalformedIf".to_string(),
            expected_location: Some((3, 1)),
        },
        ErrorTest {
            name: "malformed_for".to_string(),
            description: "Malformed for loop".to_string(),
            code: r#"for x in (1, 2, 3)
    return x;
endfor"#
                .to_string(),
            expected_error_type: "MalformedFor".to_string(),
            expected_location: Some((1, 19)),
        },
        ErrorTest {
            name: "malformed_while".to_string(),
            description: "Malformed while loop".to_string(),
            code: r#"while x > 0
    x = x - 1;
end"#
                .to_string(),
            expected_error_type: "MalformedWhile".to_string(),
            expected_location: Some((3, 1)),
        },
        ErrorTest {
            name: "invalid_scatter".to_string(),
            description: "Invalid scatter assignment".to_string(),
            code: r#"{x, y, z} = [1, 2];
return x;"#
                .to_string(),
            expected_error_type: "InvalidScatter".to_string(),
            expected_location: Some((1, 1)),
        },
        ErrorTest {
            name: "invalid_try_catch".to_string(),
            description: "Invalid try-catch block".to_string(),
            code: r#"try
    x = dangerous_operation();
catch e
    return e;
end"#
                .to_string(),
            expected_error_type: "InvalidTryCatch".to_string(),
            expected_location: Some((5, 1)),
        },
        ErrorTest {
            name: "invalid_property_access".to_string(),
            description: "Invalid property access syntax".to_string(),
            code: r#"x = obj.;
return x;"#
                .to_string(),
            expected_error_type: "InvalidPropertyAccess".to_string(),
            expected_location: Some((1, 8)),
        },
        ErrorTest {
            name: "invalid_verb_call".to_string(),
            description: "Invalid verb call syntax".to_string(),
            code: r#"x = obj:();
return x;"#
                .to_string(),
            expected_error_type: "InvalidVerbCall".to_string(),
            expected_location: Some((1, 9)),
        },
        ErrorTest {
            name: "nested_error".to_string(),
            description: "Nested syntax error in complex expression".to_string(),
            code: r#"if (x > 0 && (y < 10 || z == "test")
    for item in items
        if item.valid
            return item.process();
        endif
    endfor
endif"#
                .to_string(),
            expected_error_type: "UnmatchedParen".to_string(),
            expected_location: Some((1, 4)),
        },
    ]
}

fn test_parser_error_reporting(parser_name: &str, test_case: &ErrorTest) -> ErrorResult {
    let compile_options = CompileOptions::default();

    // Set parser environment variable for consistency
    unsafe {
        std::env::set_var("MOO_PARSER", parser_name);
    }

    let result = match parser_name {
        "cst" | "pest" => {
            // Both cst and pest are handled by the compile function
            // The parser selection is done via the MOO_PARSER environment variable
            compile(&test_case.code, compile_options)
        }
        #[cfg(feature = "tree-sitter-parser")]
        "tree-sitter" => compile_with_tree_sitter(&test_case.code, compile_options),
        #[cfg(not(feature = "tree-sitter-parser"))]
        "tree-sitter" => {
            return ErrorResult {
                parser_name: parser_name.to_string(),
                test_name: test_case.name.clone(),
                success: false,
                error_message: Some("Tree-sitter parser not available".to_string()),
                error_location: None,
                error_quality_score: 0,
            };
        }
        _ => {
            return ErrorResult {
                parser_name: parser_name.to_string(),
                test_name: test_case.name.clone(),
                success: false,
                error_message: Some(format!("Unknown parser: {}", parser_name)),
                error_location: None,
                error_quality_score: 0,
            };
        }
    };

    match result {
        Ok(_) => {
            // Test case should have failed but didn't
            ErrorResult {
                parser_name: parser_name.to_string(),
                test_name: test_case.name.clone(),
                success: false,
                error_message: Some("Expected error but parsing succeeded".to_string()),
                error_location: None,
                error_quality_score: 0,
            }
        }
        Err(compile_error) => {
            let error_msg = format!("{}", compile_error);
            let error_location = extract_error_location(&error_msg);
            let quality_score = evaluate_error_quality(&error_msg, test_case, error_location);

            ErrorResult {
                parser_name: parser_name.to_string(),
                test_name: test_case.name.clone(),
                success: true,
                error_message: Some(error_msg),
                error_location,
                error_quality_score: quality_score,
            }
        }
    }
}

fn extract_error_location(error_msg: &str) -> Option<(usize, usize)> {
    // Try to extract line and column from error message
    // Look for patterns like "line 1, column 8" or "1:8" or "at line 1"

    // Pattern: "line X, column Y"
    if let Some(line_start) = error_msg.find("line ") {
        if let Some(line_end) = error_msg[line_start + 5..].find(',') {
            if let Ok(line) = error_msg[line_start + 5..line_start + 5 + line_end].parse::<usize>()
            {
                if let Some(col_start) = error_msg[line_start + 5 + line_end..].find("column ") {
                    let col_start = line_start + 5 + line_end + col_start + 7;
                    if let Some(col_end) =
                        error_msg[col_start..].find(|c: char| !c.is_ascii_digit())
                    {
                        if let Ok(col) = error_msg[col_start..col_start + col_end].parse::<usize>()
                        {
                            return Some((line, col));
                        }
                    }
                }
            }
        }
    }

    // Pattern: "X:Y" - check first for the standard format at the beginning
    if let Some(first_colon) = error_msg.find(':') {
        // Check if there's another colon right after (for line:col: message format)
        if let Some(second_colon) = error_msg[first_colon + 1..].find(':') {
            let line_part = &error_msg[..first_colon];
            let col_part = &error_msg[first_colon + 1..first_colon + 1 + second_colon];
            if let (Ok(line), Ok(col)) = (
                line_part.trim().parse::<usize>(),
                col_part.trim().parse::<usize>(),
            ) {
                return Some((line, col));
            }
        }
    }

    // Pattern: "X:Y" in parts
    for part in error_msg.split_whitespace() {
        if let Some(colon_pos) = part.find(':') {
            let line_part = &part[..colon_pos];
            let col_part = &part[colon_pos + 1..];
            if let (Ok(line), Ok(col)) = (line_part.parse::<usize>(), col_part.parse::<usize>()) {
                return Some((line, col));
            }
        }
    }

    // Pattern: "at line X"
    if let Some(line_start) = error_msg.find("at line ") {
        let line_str = &error_msg[line_start + 8..];
        if let Some(space_pos) = line_str.find(' ') {
            if let Ok(line) = line_str[..space_pos].parse::<usize>() {
                return Some((line, 0));
            }
        }
    }

    // Pattern: "@ X/Y"
    if let Some(at_pos) = error_msg.find("@ ") {
        let location_str = &error_msg[at_pos + 2..];
        if let Some(slash_pos) = location_str.find('/') {
            let line_part = &location_str[..slash_pos];
            let rest = &location_str[slash_pos + 1..];
            // Find where the column number ends
            let col_end = rest
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(rest.len());
            let col_part = &rest[..col_end];

            if let (Ok(line), Ok(col)) = (line_part.parse::<usize>(), col_part.parse::<usize>()) {
                return Some((line, col));
            }
        }
    }

    None
}

fn evaluate_error_quality(
    error_msg: &str,
    test_case: &ErrorTest,
    location: Option<(usize, usize)>,
) -> u8 {
    let mut score = 0u8;

    // Base score for having an error message
    score += 2;

    // Score for error message clarity (length and informativeness)
    if error_msg.len() > 20 {
        score += 1;
    }
    if error_msg.len() > 50 {
        score += 1;
    }

    // Score for location accuracy
    if let (Some((actual_line, actual_col)), Some((expected_line, expected_col))) =
        (location, test_case.expected_location)
    {
        // Perfect location match
        if actual_line == expected_line && actual_col == expected_col {
            score += 4;
        }
        // Close location match (within 2 lines)
        else if actual_line.abs_diff(expected_line) <= 2 {
            score += 2;
        }
        // At least has some location
        else {
            score += 1;
        }
    } else if location.is_some() {
        // Has location but we don't know expected
        score += 1;
    }

    // Score for error type relevance
    let error_lower = error_msg.to_lowercase();
    let expected_lower = test_case.expected_error_type.to_lowercase();

    if error_lower.contains(&expected_lower) {
        score += 2;
    } else if error_lower.contains("syntax")
        || error_lower.contains("parse")
        || error_lower.contains("unexpected")
    {
        score += 1;
    }

    score.min(10)
}

fn print_error_comparison_matrix(results: &[ErrorResult]) {
    let parsers = ["cst", "pest", "tree-sitter"];

    // Group results by test case
    let mut tests: HashMap<String, Vec<&ErrorResult>> = HashMap::new();
    for result in results {
        tests
            .entry(result.test_name.clone())
            .or_insert_with(Vec::new)
            .push(result);
    }

    println!("\n=== SYNTAX ERROR REPORTING COMPARISON ===\n");

    // Print header
    print!("{:<25}", "Test Case");
    for parser in &parsers {
        print!(
            "{:<12} {:<12} {:<12}",
            format!("{} Status", parser),
            format!("{} Quality", parser),
            format!("{} Location", parser)
        );
    }
    println!();

    // Print separator
    print!("{:-<25}", "");
    for _ in &parsers {
        print!("{:-<12} {:-<12} {:-<12}", "", "", "");
    }
    println!();

    // Print results for each test case
    for (test_name, test_results) in &tests {
        print!("{:<25}", test_name);

        for parser in &parsers {
            if let Some(result) = test_results.iter().find(|r| r.parser_name == *parser) {
                let status = if result.success { "DETECTED" } else { "MISSED" };
                let quality = format!("{}/10", result.error_quality_score);
                let location = if let Some((line, col)) = result.error_location {
                    format!("{}:{}", line, col)
                } else {
                    "N/A".to_string()
                };

                print!("{:<12} {:<12} {:<12}", status, quality, location);
            } else {
                print!("{:<12} {:<12} {:<12}", "N/A", "N/A", "N/A");
            }
        }
        println!();
    }

    println!("\n=== DETAILED ERROR MESSAGES ===\n");

    for (test_name, test_results) in &tests {
        println!("Test: {}", test_name);
        println!("Code: {}", test_results[0].test_name); // Get test case info

        for parser in &parsers {
            if let Some(result) = test_results.iter().find(|r| r.parser_name == *parser) {
                println!(
                    "  {}: {}",
                    parser,
                    result
                        .error_message
                        .as_deref()
                        .unwrap_or("No error message")
                );
            }
        }
        println!();
    }

    // Summary statistics
    println!("=== SUMMARY STATISTICS ===\n");

    for parser in &parsers {
        let parser_results: Vec<_> = results
            .iter()
            .filter(|r| r.parser_name == *parser)
            .collect();

        if !parser_results.is_empty() {
            let detection_rate = parser_results.iter().filter(|r| r.success).count() as f64
                / parser_results.len() as f64;
            let avg_quality = parser_results
                .iter()
                .map(|r| r.error_quality_score as f64)
                .sum::<f64>()
                / parser_results.len() as f64;
            let location_rate = parser_results
                .iter()
                .filter(|r| r.error_location.is_some())
                .count() as f64
                / parser_results.len() as f64;

            println!("{} parser:", parser);
            println!(
                "  Detection Rate: {:.1}% ({}/{})",
                detection_rate * 100.0,
                parser_results.iter().filter(|r| r.success).count(),
                parser_results.len()
            );
            println!("  Average Quality: {:.1}/10", avg_quality);
            println!(
                "  Location Rate: {:.1}% ({}/{})",
                location_rate * 100.0,
                parser_results
                    .iter()
                    .filter(|r| r.error_location.is_some())
                    .count(),
                parser_results.len()
            );
            println!();
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("MOO Parser Syntax Error Reporting Comparison");
    println!("===========================================\n");

    let test_cases = create_error_test_cases();
    let parsers = ["cst", "pest", "tree-sitter"];

    let mut results = Vec::new();

    println!(
        "Running {} test cases across {} parsers...\n",
        test_cases.len(),
        parsers.len()
    );

    for parser in &parsers {
        println!("Testing {} parser...", parser);

        // Create output file for this parser
        let filename = format!("error_output_{}.txt", parser);
        let mut file = std::fs::File::create(&filename)?;
        use std::io::Write;

        writeln!(file, "MOO Parser Error Output - {} Parser", parser)?;
        writeln!(file, "=========================================\n")?;

        for test_case in &test_cases {
            let result = test_parser_error_reporting(parser, test_case);

            if result.success {
                println!(
                    "  ✅ {}: Quality {}/10",
                    test_case.name, result.error_quality_score
                );
            } else {
                println!(
                    "  ❌ {}: {}",
                    test_case.name,
                    result.error_message.as_deref().unwrap_or("Unknown error")
                );
            }

            // Write error to file
            writeln!(
                file,
                "Test Case: {} ({})",
                test_case.name, test_case.description
            )?;
            writeln!(file, "Code:")?;
            writeln!(file, "{}", test_case.code)?;
            writeln!(file, "\nError Output:")?;
            writeln!(
                file,
                "{}",
                result
                    .error_message
                    .as_deref()
                    .unwrap_or("No error message")
            )?;
            writeln!(
                file,
                "\nError Location: {}",
                if let Some((line, col)) = result.error_location {
                    format!("{}:{}", line, col)
                } else {
                    "N/A".to_string()
                }
            )?;
            writeln!(file, "Quality Score: {}/10", result.error_quality_score)?;
            writeln!(file, "{}", "=".repeat(80))?;
            writeln!(file)?;

            results.push(result);
        }
        println!("  Output written to {}", filename);
        println!();
    }

    print_error_comparison_matrix(&results);

    Ok(())
}
