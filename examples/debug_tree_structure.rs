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

fn print_tree_simple(source: &str) {
    // Simple approach: just try to parse with our converter
    use moor_compiler::{compile_with_tree_sitter, CompileOptions};

    match compile_with_tree_sitter(source, CompileOptions::default()) {
        Ok(_) => {
            println!("✅ Tree-sitter compiled successfully!");
        }
        Err(e) => {
            println!("❌ Tree-sitter compile failed: {}", e);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <code>", args[0]);
        std::process::exit(1);
    }

    let code = &args[1];
    println!("Parsing: {}", code);

    print_tree_simple(code);

    // Also try PEST for comparison
    use moor_compiler::{compile, CompileOptions};

    println!("\n--- PEST comparison ---");
    match compile(code, CompileOptions::default()) {
        Ok(_) => println!("✅ PEST parsed successfully!"),
        Err(e) => println!("❌ PEST parse failed: {}", e),
    }
}
