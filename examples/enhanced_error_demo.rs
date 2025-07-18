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

use moor_compiler::{compile_with_tree_sitter, CompileOptions};

fn main() {
    println!("Enhanced Error Reporting Demo");
    println!("=============================\n");

    // Test cases with various syntax errors
    let test_cases = vec![
        ("Missing semicolon", "x = 42\ny = @invalid_token"),
        (
            "Invalid if statement",
            "if (x > 0 @error\n    return x;\nendif",
        ),
        (
            "Broken for loop",
            "for i in @broken_syntax\n    println(i);\nendfor",
        ),
        ("Invalid scatter assignment", "{a, b, c @error = {1, 2, 3}"),
        ("Malformed function call", "println(@invalid_arg, )"),
        ("Unclosed string", "x = \"this string is not closed"),
        ("Invalid operator", "x = 5 @@ 3"),
        ("Missing expression", "if () return 42; endif"),
    ];

    for (description, source) in test_cases {
        println!("Test: {}", description);
        println!("Source: {}", source);
        println!("------");

        #[cfg(feature = "tree-sitter-parser")]
        {
            match compile_with_tree_sitter(source, CompileOptions::default()) {
                Ok(_) => println!("✓ Unexpectedly succeeded!"),
                Err(error) => {
                    println!("✗ Enhanced Error Report:");
                    println!("{}", error);
                }
            }
        }

        #[cfg(not(feature = "tree-sitter-parser"))]
        {
            println!("Tree-sitter parser not available. Enable with --features tree-sitter-parser");
        }

        println!("\n{}\n", "=".repeat(60));
    }

    // Show a successful parse for comparison
    println!("Successful Parse Example:");
    println!("Source: x = 42; y = x + 1; return y;");
    println!("------");

    #[cfg(feature = "tree-sitter-parser")]
    {
        match compile_with_tree_sitter("x = 42; y = x + 1; return y;", CompileOptions::default()) {
            Ok(program) => {
                println!("✓ Successfully compiled!");
                println!("Generated {} opcodes", program.0.main_vector.len());
            }
            Err(error) => println!("✗ Unexpected error: {}", error),
        }
    }
}
