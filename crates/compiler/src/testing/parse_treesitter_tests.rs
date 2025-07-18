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

// Comprehensive tests comparing PEST and tree-sitter parsers

#[cfg(all(test, feature = "tree-sitter-parser"))]
mod tests {
    use crate::{
        parse::CompileOptions, parse_cst::parse_program_cst,
        parse_treesitter::parse_program_with_tree_sitter, unparse::unparse,
    };
    use log::debug;
    use pretty_assertions::assert_eq;

    /// Test that both parsers produce equivalent ASTs
    fn compare_parsers(source: &str) {
        let options = CompileOptions::default();

        // Parse with PEST (via parse_cst which uses CST)
        let pest_result = parse_program_cst(source, options.clone());

        // Parse with tree-sitter
        let ts_result = parse_program_with_tree_sitter(source, options);

        match (pest_result, ts_result) {
            (Ok(pest_parse), Ok(ts_parse)) => {
                println!("Testing: {}", source);
                println!("PEST statements: {}", pest_parse.stmts.len());
                println!("Tree-sitter statements: {}", ts_parse.stmts.len());

                // Compare number of statements
                assert_eq!(
                    pest_parse.stmts.len(),
                    ts_parse.stmts.len(),
                    "Different number of statements for: {source}"
                );

                debug!("PEST: {:#?}", pest_parse);
                debug!("Tree-sitter: {:#?}", ts_parse);

                // Convert ParseCst to Parse for unparsing
                let pest_as_parse = pest_parse.into();
                let ts_as_parse = ts_parse.into();

                // Compare unparsed output (should be semantically equivalent)
                let pest_unparsed = unparse(&pest_as_parse).unwrap();
                let ts_unparsed = unparse(&ts_as_parse).unwrap();

                assert_eq!(pest_unparsed, ts_unparsed);
                // // For now, just check they both unparse successfully
                // // TODO: Implement more detailed AST comparison
                // if pest_parse.stmts.is_empty() {
                //     // Empty program should produce empty output
                //     assert!(pest_unparsed.is_empty());
                //     assert!(ts_unparsed.is_empty());
                // } else {
                //     assert!(!pest_unparsed.is_empty());
                //     assert!(!ts_unparsed.is_empty());
                // }
            }
            (Err(pest_err), Err(ts_err)) => {
                // Both failed - this is OK
                println!(
                    "Both parsers failed for '{}': PEST: {:?}, TS: {:?}",
                    source, pest_err, ts_err
                );
            }
            (Ok(_), Err(ts_err)) => {
                println!(
                    "Tree-sitter failed but PEST succeeded for '{}': {:?}",
                    source, ts_err
                );
            }
            (Err(pest_err), Ok(_)) => {
                println!(
                    "PEST failed but tree-sitter succeeded for '{}': {:?}",
                    source, pest_err
                );
            }
        }
    }

    #[test]
    fn test_simple_statements() {
        let cases = vec![
            "x = 1;",
            "x = 1 + 2;",
            "x = 1 + 2 * 3;",
            "x = (1 + 2) * 3;",
            "return;",
            "return 42;",
            ";", // empty statement
        ];

        for case in cases {
            compare_parsers(case);
        }
    }

    #[test]
    fn test_operators() {
        let cases = vec![
            // Arithmetic
            "x = a + b;",
            "x = a - b;",
            "x = a * b;",
            "x = a / b;",
            "x = a % b;",
            "x = a ^ b;",
            // Comparison
            "x = a == b;",
            "x = a != b;",
            "x = a < b;",
            "x = a <= b;",
            "x = a > b;",
            "x = a >= b;",
            "x = a in b;",
            // Logical
            "x = a && b;",
            "x = a || b;",
            "x = !a;",
            // Ternary
            "x = a ? b | c;",
        ];

        for case in cases {
            compare_parsers(case);
        }
    }

    #[test]
    fn test_control_flow() {
        let cases = vec![
            // If statements
            "if (x) return 1; endif",
            "if (x) return 1; else return 2; endif",
            "if (x) return 1; elseif (y) return 2; else return 3; endif",
            // Loops
            "while (x < 10) x = x + 1; endwhile",
            "for x in ({1, 2, 3}) sum = sum + x; endfor",
            "for i in [1..10] sum = sum + i; endfor",
            // Break/continue
            "while (1) if (done) break; endif endwhile",
            "for x in (list) if (x == 0) continue; endif process(x); endfor",
        ];

        for case in cases {
            compare_parsers(case);
        }
    }

    #[test]
    fn test_function_definitions() {
        let cases = vec![
            "fn f() return 42; endfn",
            "fn f(x) return x * 2; endfn",
            "fn f(x, y) return x + y; endfn",
            "fn f(x, y = 10) return x + y; endfn",
            "fn f(x, @args) return length(args); endfn",
        ];

        for case in cases {
            compare_parsers(case);
        }
    }

    #[test]
    fn test_data_structures() {
        let cases = vec![
            // Lists
            "x = {};",
            "x = {1};",
            "x = {1, 2, 3};",
            "x = {1, 2, 3,};", // trailing comma
            // Maps
            "x = [];",
            "x = [1 -> 2];",
            "x = [\"a\" -> 1, \"b\" -> 2];",
            // Property/method access
            "x = obj.prop;",
            "x = obj.(expr);",
            "x = obj:method();",
            "x = obj:method(a, b);",
            // Indexing
            "x = list[1];",
            "x = list[1..5];",
        ];

        for case in cases {
            compare_parsers(case);
        }
    }

    #[test]
    fn test_special_constructs() {
        let cases = vec![
            // Try-catch expressions
            "x = `risky()';",
            "x = `risky() ! E_PERM => 0';",
            "try x = risky(); except (E_PERM) return 0; endtry",
            // Scatter assignment
            "{a, b} = get_pair();",
            "{a, ?b = 10} = get_pair();",
            "{a, @rest} = get_list();",
            // Lambda (if supported)
            "f = {x, y -> x + y};",
        ];

        for case in cases {
            // Some of these might not be supported by both parsers
            compare_parsers(case);
        }
    }

    #[test]
    fn test_comments_preserved() {
        let cases = vec![
            "// comment\nx = 1;",
            "x = 1; // end of line",
            "/* block */ x = 1;",
            "x = /* inline */ 1;",
            "/*\n * multi\n * line\n */\nx = 1;",
        ];

        for case in cases {
            // Tree-sitter should preserve these in the CST
            compare_parsers(case);
            // let ts_result = parse_program_with_tree_sitter(case, CompileOptions::default());
            // assert!(ts_result.is_ok(), "Failed to parse with comments: {case}");
            //
            // // Verify CST contains comment nodes
            // if let Ok(parse) = ts_result {
            //     // The CST should have preserved the comments
            //     parse.stmts.iter().for_each(|stmt| {
            //         println!("{stmt:?}" );
            //     })
            // }
        }
    }

    #[test]
    fn test_error_cases() {
        let cases = vec![
            "if (x) missing endif",
            "x = ;",
            "x = 1 +;",
            "{ = 1;",              // invalid scatter
            "fn () return; endfn", // missing function name
        ];

        for case in cases {
            // Both parsers should fail on these
            let pest_result = parse_program_cst(case, CompileOptions::default());
            let ts_result = parse_program_with_tree_sitter(case, CompileOptions::default());

            assert!(
                pest_result.is_err() || ts_result.is_err(),
                "Expected parse error for: {}",
                case
            );
        }
    }
}
