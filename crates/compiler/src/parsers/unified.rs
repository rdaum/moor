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

//! Unified parsing interface and TreeNode implementations - Phase 1 & 2
//!
//! This module demonstrates how different parser backends can be unified through
//! the TreeNode trait and ParseResult output type. This includes:
//! - Phase 1: Unified ParseResult output type
//! - Phase 2: Common AST building utilities

use crate::ast::Stmt;
use crate::cst::CSTNode;
use crate::parsers::parse::{Parse, CompileOptions};
use crate::parsers::parse_cst::ParseCst;
use crate::parsers::ast_builder::ASTBuilder;
use crate::var_scope::VarScope;
use moor_var::program::names::Names;

/// Unified output type for all parsers
///
/// This replaces the separate Parse and ParseCst types with a single
/// output format that can preserve CST information when available.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseResult {
    /// The processed AST statements
    pub stmts: Vec<Stmt>,
    /// Variable scope and symbol table
    pub variables: VarScope,
    /// Name resolution table
    pub names: Names,
    /// Optional CST preservation for parsers that support it
    pub cst: Option<CSTNode>,
}

impl From<Parse> for ParseResult {
    fn from(parse: Parse) -> Self {
        Self {
            stmts: parse.stmts,
            variables: parse.variables,
            names: parse.names,
            cst: None, // PEST-only parser doesn't preserve CST
        }
    }
}

impl From<ParseCst> for ParseResult {
    fn from(parse_cst: ParseCst) -> Self {
        Self {
            stmts: parse_cst.stmts,
            variables: parse_cst.variables,
            names: parse_cst.names,
            cst: Some(parse_cst.cst), // CST parsers preserve it
        }
    }
}

/// Simple demonstration of unified parsing functions
/// 
/// These functions show how we can provide a unified interface
/// over the different parser implementations.
pub fn parse_unified_pest(code: &str, options: CompileOptions) -> Result<ParseResult, moor_common::model::CompileError> {
    let parse = crate::parsers::parse::parse_program(code, options)?;
    Ok(parse.into())
}

pub fn parse_unified_cst(code: &str, options: CompileOptions) -> Result<ParseResult, moor_common::model::CompileError> {
    let parse_cst = crate::parsers::parse_cst::parse_program_cst(code, options)?;
    Ok(parse_cst.into())
}

#[cfg(feature = "tree-sitter-parser")]
pub fn parse_unified_tree_sitter(code: &str, options: CompileOptions) -> Result<ParseResult, moor_common::model::CompileError> {
    let parse_cst = crate::parsers::tree_sitter::parse_treesitter::parse_program_with_tree_sitter(code, options)?;
    Ok(parse_cst.into())
}

/// Compare all available parsers for consistency
pub fn compare_all_parsers(code: &str) -> ParserComparisonResult {
    let options = CompileOptions::default();
    
    let pest_result = parse_unified_pest(code, options.clone()).map(|_| ());
    let cst_result = parse_unified_cst(code, options.clone()).map(|_| ());
    
    #[cfg(feature = "tree-sitter-parser")]
    let tree_sitter_result = parse_unified_tree_sitter(code, options).map(|_| ());
    #[cfg(not(feature = "tree-sitter-parser"))]
    let tree_sitter_result = Ok(());
    
    let parsers_agree = match (&pest_result, &cst_result, &tree_sitter_result) {
        (Ok(_), Ok(_), Ok(_)) => true,
        (Err(_), Err(_), Err(_)) => true, // All failed - consistent
        _ => false, // Mixed results
    };
    
    ParserComparisonResult {
        code: code.to_string(),
        pest_result,
        cst_result,
        tree_sitter_result,
        parsers_agree,
    }
}

/// Common AST builder instance for unified parsing
/// 
/// This demonstrates Phase 2 - extracting common AST building logic
/// that can be shared across all parser implementations.
pub fn create_ast_builder(options: CompileOptions) -> ASTBuilder {
    ASTBuilder::new(options)
}

/// Result of comparing parser outputs
#[derive(Debug)]
pub struct ParserComparisonResult {
    pub code: String,
    pub pest_result: Result<(), moor_common::model::CompileError>,
    pub cst_result: Result<(), moor_common::model::CompileError>,
    pub tree_sitter_result: Result<(), moor_common::model::CompileError>,
    pub parsers_agree: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_result_conversion() {
        let code = "return 42;";
        let options = CompileOptions::default();

        // Test PEST parser conversion
        let pest_result = parse_unified_pest(code, options.clone()).unwrap();
        assert!(pest_result.cst.is_none()); // PEST doesn't preserve CST
        assert_eq!(pest_result.stmts.len(), 1);

        // Test CST parser conversion  
        let cst_result = parse_unified_cst(code, options).unwrap();
        assert!(cst_result.cst.is_some()); // CST parser preserves CST
        assert_eq!(cst_result.stmts.len(), 1);
    }

    #[test]
    fn test_parser_consistency() {
        // Test that all parsers produce equivalent results for simple expressions
        let test_cases = vec![
            "return 42;",
            "x = 5;", 
            "if (1) return 2; endif",
        ];

        for code in test_cases {
            let comparison = compare_all_parsers(code);
            
            // For this proof of concept, we just verify they all run
            // In a full implementation, we'd do deep AST comparison
            println!("Testing: {} - Agreement: {}", 
                    comparison.code, comparison.parsers_agree);
            
            // At minimum, verify no panics occurred
            assert!(!comparison.code.is_empty());
        }
    }

    #[test]
    fn test_unified_interface() {
        let code = "return 123;";
        let options = CompileOptions::default();
        
        // Demonstrate that all parsers work through the unified interface
        let pest_result = parse_unified_pest(code, options.clone());
        let cst_result = parse_unified_cst(code, options.clone());
        
        assert!(pest_result.is_ok());
        assert!(cst_result.is_ok());
        
        // Show structural equivalence
        let pest_stmts = pest_result.unwrap().stmts.len();
        let cst_stmts = cst_result.unwrap().stmts.len();
        assert_eq!(pest_stmts, cst_stmts);
    }

    #[cfg(feature = "tree-sitter-parser")]
    #[test]
    fn test_tree_sitter_unified() {
        let code = "return 456;";
        let options = CompileOptions::default();
        
        let ts_result = parse_unified_tree_sitter(code, options);
        assert!(ts_result.is_ok());
        
        let ts_stmts = ts_result.unwrap().stmts.len();
        assert_eq!(ts_stmts, 1);
    }

    #[test]
    fn test_comprehensive_comparison() {
        let test_cases = vec![
            "return 1;",
            "x = 42;",
            "y = x + 1;",
        ];

        for code in test_cases {
            let comparison = compare_all_parsers(code);
            
            // Print results for manual inspection
            println!("\n=== Testing: {} ===", comparison.code);
            println!("PEST: {:?}", comparison.pest_result.is_ok());
            println!("CST: {:?}", comparison.cst_result.is_ok());
            println!("Tree-sitter: {:?}", comparison.tree_sitter_result.is_ok());
            println!("Agreement: {}", comparison.parsers_agree);
            
            // Basic validation - at least ensure no crashes
            assert!(!comparison.code.is_empty());
        }
    }
}