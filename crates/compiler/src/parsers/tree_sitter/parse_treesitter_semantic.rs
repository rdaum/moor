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

//! Semantic tree-sitter parser using the tree walker approach

use tree_sitter::Parser;
use tree_sitter_moo;

use super::tree_walker::SemanticTreeWalker;
use crate::cst::CSTNode;
use moor_common::model::CompileError;

/// Parse source code using semantic tree walker approach
pub fn parse_with_semantic_walker(source: &str) -> Result<CSTNode, CompileError> {
    // Parse with tree-sitter
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_moo::language())
        .map_err(|e| CompileError::ParseError {
            error_position: moor_common::model::CompileContext::new((1, 1)),
            context: "tree-sitter language setup".to_string(),
            end_line_col: None,
            message: format!("Failed to set language: {}", e),
        })?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| CompileError::ParseError {
            error_position: moor_common::model::CompileContext::new((1, 1)),
            context: "tree-sitter parsing".to_string(),
            end_line_col: None,
            message: "Failed to parse source".to_string(),
        })?;

    let root = tree.root_node();

    // Check for parse errors
    if root.has_error() {
        return Err(find_tree_sitter_error(&root, source));
    }

    // Use semantic tree walker for conversion
    let mut walker = SemanticTreeWalker::new(source);

    // Phase 1: Semantic Discovery
    walker.discover_semantics(&root)?;

    // Phase 2: Semantic Analysis
    walker.analyze_semantics()?;

    // Phase 3: Conversion with semantic context
    walker.convert_with_semantics()
}

/// Find and report tree-sitter parse errors
fn find_tree_sitter_error(node: &tree_sitter::Node, source: &str) -> CompileError {
    if node.kind() == "ERROR" {
        let pos = node.start_position();
        let text = &source[node.start_byte()..node.end_byte()];
        return CompileError::ParseError {
            error_position: moor_common::model::CompileContext::new((pos.row + 1, pos.column + 1)),
            context: "tree-sitter parsing".to_string(),
            end_line_col: None,
            message: format!("Syntax error: {}", text),
        };
    }

    for child in node.children(&mut node.walk()) {
        if child.has_error() {
            return find_tree_sitter_error(&child, source);
        }
    }

    let pos = node.start_position();
    CompileError::ParseError {
        error_position: moor_common::model::CompileContext::new((pos.row + 1, pos.column + 1)),
        context: "tree-sitter parsing".to_string(),
        end_line_col: None,
        message: "Unknown parse error".to_string(),
    }
}

/// Debug function to analyze semantic structure
pub fn debug_semantic_analysis(source: &str) -> Result<String, CompileError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_moo::language())
        .map_err(|e| CompileError::ParseError {
            error_position: moor_common::model::CompileContext::new((1, 1)),
            context: "tree-sitter language setup".to_string(),
            end_line_col: None,
            message: format!("Failed to set language: {}", e),
        })?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| CompileError::ParseError {
            error_position: moor_common::model::CompileContext::new((1, 1)),
            context: "tree-sitter parsing".to_string(),
            end_line_col: None,
            message: "Failed to parse source".to_string(),
        })?;

    let root = tree.root_node();

    let mut walker = SemanticTreeWalker::new(source);
    walker.discover_semantics(&root)?;
    walker.analyze_semantics()?;

    Ok(walker.debug_semantic_analysis())
}
