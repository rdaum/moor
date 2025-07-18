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

//! Parser trait for abstracting different compilation backends
//!
//! This module provides a trait-based interface for different parser implementations.
//! The MooParser trait allows different parsing backends (PEST, Tree-sitter, etc.) to
//! provide a consistent interface for compilation.
//!
//! For adding new parsers that use different tree representations, the generic trait
//! system in tree_traits.rs provides TreeNode, ASTBuilder, and ASTBuilderContext traits
//! that allow parsers to share common AST building logic while using different underlying
//! tree structures.

use super::parse::CompileOptions;
use crate::Program;
use moor_common::model::CompileError;

/// Trait for different parser implementations
pub trait MooParser {
    /// The name of this parser (for debugging/testing)
    fn name(&self) -> &'static str;

    /// Compile a program string into a Program
    fn compile(&self, program: &str, options: CompileOptions) -> Result<Program, CompileError>;

    /// Optional method to check if this parser is available (e.g., for feature-gated parsers)
    fn is_available(&self) -> bool {
        true
    }
}

/// Original PEST parser implementation
pub struct OriginalPestParser;

impl MooParser for OriginalPestParser {
    fn name(&self) -> &'static str {
        "Original PEST"
    }

    fn compile(&self, program: &str, options: CompileOptions) -> Result<Program, CompileError> {
        crate::compile_legacy(program, options)
    }
}

/// CST parser implementation (uses PEST internally but with CST structure)
pub struct CstParser;

impl MooParser for CstParser {
    fn name(&self) -> &'static str {
        "CST (PEST-based)"
    }

    fn compile(&self, program: &str, options: CompileOptions) -> Result<Program, CompileError> {
        crate::compile(program, options)
    }
}

/// Tree-sitter parser implementation (feature-gated)
#[cfg(feature = "tree-sitter-parser")]
pub struct TreeSitterParser;

#[cfg(feature = "tree-sitter-parser")]
impl MooParser for TreeSitterParser {
    fn name(&self) -> &'static str {
        "Tree-sitter"
    }

    fn compile(&self, program: &str, options: CompileOptions) -> Result<Program, CompileError> {
        crate::compile_with_tree_sitter(program, options)
    }
}

/// Tree-sitter parser with moot-compatible errors (feature-gated)
#[cfg(feature = "tree-sitter-parser")]
pub struct TreeSitterMootParser;

#[cfg(feature = "tree-sitter-parser")]
impl MooParser for TreeSitterMootParser {
    fn name(&self) -> &'static str {
        "Tree-sitter (moot-compatible)"
    }

    fn compile(&self, program: &str, options: CompileOptions) -> Result<Program, CompileError> {
        crate::compile_with_tree_sitter_moot_compatible(program, options)
    }
}

/// Get all available parser implementations
pub fn get_available_parsers() -> Vec<Box<dyn MooParser>> {
    let mut parsers: Vec<Box<dyn MooParser>> =
        vec![Box::new(OriginalPestParser), Box::new(CstParser)];

    #[cfg(feature = "tree-sitter-parser")]
    {
        parsers.push(Box::new(TreeSitterParser));
        parsers.push(Box::new(TreeSitterMootParser));
    }

    parsers.into_iter().filter(|p| p.is_available()).collect()
}

/// Get a specific parser by name
pub fn get_parser_by_name(name: &str) -> Option<Box<dyn MooParser>> {
    match name {
        "original" | "pest" => Some(Box::new(OriginalPestParser)),
        "cst" => Some(Box::new(CstParser)),
        #[cfg(feature = "tree-sitter-parser")]
        "tree-sitter" | "ts" => Some(Box::new(TreeSitterParser)),
        #[cfg(feature = "tree-sitter-parser")]
        "tree-sitter-moot" | "ts-moot" => Some(Box::new(TreeSitterMootParser)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_availability() {
        let parsers = get_available_parsers();
        assert!(parsers.len() >= 2); // At least original and CST

        // Test that all parsers have names
        for parser in parsers {
            assert!(!parser.name().is_empty());
        }
    }

    #[test]
    fn test_get_parser_by_name() {
        assert!(get_parser_by_name("original").is_some());
        assert!(get_parser_by_name("pest").is_some());
        assert!(get_parser_by_name("cst").is_some());
        assert!(get_parser_by_name("nonexistent").is_none());
    }

    #[test]
    fn test_simple_compilation() {
        let program = "return 42;";
        let options = CompileOptions::default();

        for parser in get_available_parsers() {
            let result = parser.compile(program, options.clone());
            assert!(
                result.is_ok(),
                "Parser '{}' failed to compile simple program: {:?}",
                parser.name(),
                result.err()
            );
        }
    }

    #[test]
    fn test_for_loop_compilation() {
        let program = "z = 0; for i in [1..4] z = z + i; endfor return z;";
        let options = CompileOptions::default();

        for parser in get_available_parsers() {
            let result = parser.compile(program, options.clone());
            assert!(
                result.is_ok(),
                "Parser '{}' failed to compile for loop: {:?}",
                parser.name(),
                result.err()
            );
        }
    }

    #[test]
    fn test_parser_consistency() {
        let test_programs = vec![
            "return 1;",
            "x = 42; return x;",
            "if (1) return 2; else return 3; endif",
            "z = 0; for i in [1..3] z = z + i; endfor return z;",
            "for x in ({1,2,3}) return x; endfor",
        ];

        let parsers = get_available_parsers();
        if parsers.len() < 2 {
            return; // Skip test if we don't have multiple parsers
        }

        for program in test_programs {
            let mut results = Vec::new();

            // Compile with all parsers
            for parser in &parsers {
                let result = parser.compile(program, CompileOptions::default());
                results.push((parser.name(), result));
            }

            // Check that all parsers either all succeed or all fail
            let success_count = results.iter().filter(|(_, r)| r.is_ok()).count();
            assert!(
                success_count == 0 || success_count == parsers.len(),
                "Inconsistent parser results for '{}': {:?}",
                program,
                results
                    .iter()
                    .map(|(name, r)| (name, r.is_ok()))
                    .collect::<Vec<_>>()
            );
        }
    }
}
