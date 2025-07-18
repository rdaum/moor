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

#[cfg(feature = "tree-sitter-parser")]
use moor_compiler::compile_with_tree_sitter;
use moor_compiler::{compile, CompileOptions};

fn main() {
    println!("Parser Error Reporting Comparison");
    println!("=================================\n");

    let test_source = "x = @invalid_token";

    println!("Testing with source: {test_source}");
    println!();

    // Test PEST CST parser
    println!("--- PEST CST Parser ---");
    match compile(test_source, CompileOptions::default()) {
        Ok(_) => println!("✓ Unexpectedly succeeded!"),
        Err(error) => {
            println!("✗ Error Report:");
            println!("{error}");
        }
    }

    println!();

    // Test Tree-sitter parser
    #[cfg(feature = "tree-sitter-parser")]
    {
        println!("--- Tree-sitter Parser ---");
        match compile_with_tree_sitter(test_source, CompileOptions::default()) {
            Ok(_) => println!("✓ Unexpectedly succeeded!"),
            Err(error) => {
                println!("✗ Enhanced Error Report:");
                println!("{}", error);
            }
        }
    }

    #[cfg(not(feature = "tree-sitter-parser"))]
    {
        println!("--- Tree-sitter Parser ---");
        println!("Tree-sitter parser not available. Enable with --features tree-sitter-parser");
    }
}
