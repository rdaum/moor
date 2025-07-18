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

use moor_compiler::{parse_program_cst, parse_with_tree_sitter, CompileOptions};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <code>", args[0]);
        std::process::exit(1);
    }

    let code = &args[1];
    println!("Parsing: {}", code);
    println!();

    // Parse with PEST and show CST structure
    match parse_program_cst(code, CompileOptions::default()) {
        Ok(parse) => {
            println!("✅ PEST CST structure:");
            print_cst_node(&parse.cst, 0);
        }
        Err(e) => {
            println!("❌ PEST parse failed: {}", e);
        }
    }

    println!();
    println!("{}", "=".repeat(80));
    println!();

    // Parse with tree-sitter and show CST structure
    match parse_with_tree_sitter(code) {
        Ok(cst) => {
            println!("✅ Tree-sitter CST structure:");
            print_cst_node(&cst, 0);
        }
        Err(e) => {
            println!("❌ Tree-sitter parse failed: {}", e);
        }
    }
}

fn print_cst_node(node: &moor_compiler::CSTNode, depth: usize) {
    let indent = "  ".repeat(depth);
    println!("{}{:?} -> {:?}", indent, node.rule, node.kind);

    if let Some(children) = node.children() {
        for child in children {
            print_cst_node(child, depth + 1);
        }
    }
}
