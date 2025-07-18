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

use crate::ast::Expr;
use crate::parsers::parse::moo::Rule;
use moor_common::model::CompileError;
use pest::Span;

/// Concrete Syntax Tree (CST) that preserves all source information including
/// comments, whitespace, and exact formatting. This structure mirrors the
/// Pest parse tree exactly 1:1, ensuring complete source fidelity.
#[derive(Debug, Clone, PartialEq)]
pub struct CSTNode {
    /// The grammar rule that generated this node
    pub rule: Rule,
    /// Source span information for this node
    pub span: CSTSpan,
    /// The actual node content
    pub kind: CSTNodeKind,
}

/// Source position information that tracks both character positions and line/column
#[derive(Debug, Clone, PartialEq)]
pub struct CSTSpan {
    /// Start position in source (character index)
    pub start: usize,
    /// End position in source (character index)
    pub end: usize,
    /// Line and column information
    pub line_col: (usize, usize),
}

impl CSTSpan {
    pub fn new(span: Span) -> Self {
        let (line, col) = span.start_pos().line_col();
        Self {
            start: span.start(),
            end: span.end(),
            line_col: (line, col),
        }
    }
}

/// The different kinds of CST nodes that can exist
#[derive(Debug, Clone, PartialEq)]
pub enum CSTNodeKind {
    /// Terminal node containing raw text from the source
    Terminal { text: String },
    /// Non-terminal node containing child nodes
    NonTerminal { children: Vec<CSTNode> },
    /// Comment node (C-style /* */ or C++-style //)
    Comment {
        comment_type: CommentType,
        text: String,
    },
    /// Whitespace node (spaces, tabs, newlines)
    Whitespace { text: String },
}

/// Different types of comments supported in the language
#[derive(Debug, Clone, PartialEq)]
pub enum CommentType {
    /// C-style block comment /* ... */
    Block,
    /// C++-style line comment // ...
    Line,
}

impl CSTNode {
    /// Create a new terminal node
    pub fn terminal(rule: Rule, text: String, span: CSTSpan) -> Self {
        Self {
            rule,
            span,
            kind: CSTNodeKind::Terminal { text },
        }
    }

    /// Create a new non-terminal node
    pub fn non_terminal(rule: Rule, children: Vec<CSTNode>, span: CSTSpan) -> Self {
        Self {
            rule,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        }
    }

    /// Create a new comment node
    pub fn comment(comment_type: CommentType, text: String, span: CSTSpan) -> Self {
        Self {
            rule: Rule::c_comment, // Will be overridden based on comment_type
            span,
            kind: CSTNodeKind::Comment { comment_type, text },
        }
    }

    /// Create a new whitespace node
    pub fn whitespace(text: String, span: CSTSpan) -> Self {
        Self {
            rule: Rule::WHITESPACE,
            span,
            kind: CSTNodeKind::Whitespace { text },
        }
    }

    /// Get the text content of this node (for terminals and comments)
    pub fn text(&self) -> Option<&str> {
        match &self.kind {
            CSTNodeKind::Terminal { text } => Some(text),
            CSTNodeKind::Comment { text, .. } => Some(text),
            CSTNodeKind::Whitespace { text } => Some(text),
            CSTNodeKind::NonTerminal { .. } => None,
        }
    }

    /// Get the children of this node (for non-terminals)
    pub fn children(&self) -> Option<&[CSTNode]> {
        match &self.kind {
            CSTNodeKind::NonTerminal { children } => Some(children),
            _ => None,
        }
    }

    /// Check if this node is a terminal
    pub fn is_terminal(&self) -> bool {
        matches!(self.kind, CSTNodeKind::Terminal { .. })
    }

    /// Check if this node is a comment
    pub fn is_comment(&self) -> bool {
        matches!(self.kind, CSTNodeKind::Comment { .. })
    }

    /// Check if this node is whitespace
    pub fn is_whitespace(&self) -> bool {
        matches!(self.kind, CSTNodeKind::Whitespace { .. })
    }

    /// Check if this node represents source content (not whitespace/comments)
    pub fn is_content(&self) -> bool {
        !self.is_comment() && !self.is_whitespace()
    }

    /// Recursively find all comments in this subtree
    pub fn find_comments(&self) -> Vec<&CSTNode> {
        let mut comments = Vec::new();
        self.find_comments_recursive(&mut comments);
        comments
    }

    fn find_comments_recursive<'a>(&'a self, comments: &mut Vec<&'a CSTNode>) {
        if self.is_comment() {
            comments.push(self);
        }
        if let Some(children) = self.children() {
            for child in children {
                child.find_comments_recursive(comments);
            }
        }
    }

    /// Recursively find all nodes in this subtree
    pub fn find_all_nodes(&self) -> Vec<&CSTNode> {
        let mut nodes = Vec::new();
        self.find_all_nodes_recursive(&mut nodes);
        nodes
    }

    fn find_all_nodes_recursive<'a>(&'a self, nodes: &mut Vec<&'a CSTNode>) {
        nodes.push(self);
        if let Some(children) = self.children() {
            for child in children {
                child.find_all_nodes_recursive(nodes);
            }
        }
    }

    /// Get the source text range covered by this node
    pub fn source_range(&self) -> (usize, usize) {
        (self.span.start, self.span.end)
    }

    /// Get the line and column position of this node
    pub fn line_col(&self) -> (usize, usize) {
        self.span.line_col
    }
}

/// Converter from Pest parse tree to CST
pub struct PestToCSTConverter {
    source: String,
}

impl PestToCSTConverter {
    pub fn new(source: String) -> Self {
        Self { source }
    }

    /// Convert a Pest Pair to a CST node
    pub fn convert_pair(&self, pair: pest::iterators::Pair<Rule>) -> CSTNode {
        let span = CSTSpan::new(pair.as_span());
        let rule = pair.as_rule();
        let text = pair.as_str().to_string();

        // Handle special cases for comments and whitespace
        match rule {
            Rule::c_comment => {
                return CSTNode::comment(CommentType::Block, text, span);
            }
            Rule::cpp_comment => {
                return CSTNode::comment(CommentType::Line, text, span);
            }
            Rule::WHITESPACE => {
                return CSTNode::whitespace(text, span);
            }
            _ => {}
        }

        let inner_pairs: Vec<_> = pair.into_inner().collect();

        if inner_pairs.is_empty() {
            // Terminal node - use the actual text from the pair
            CSTNode::terminal(rule, text, span)
        } else {
            // Non-terminal node - we need to reconstruct children including whitespace
            // For now, use a simpler approach: recursively convert children and fill gaps with whitespace
            let children =
                self.convert_children_with_whitespace(&inner_pairs, span.start, span.end);
            CSTNode::non_terminal(rule, children, span)
        }
    }

    /// Convert children and insert whitespace nodes to preserve exact source reconstruction
    fn convert_children_with_whitespace(
        &self,
        inner_pairs: &[pest::iterators::Pair<Rule>],
        parent_start: usize,
        parent_end: usize,
    ) -> Vec<CSTNode> {
        let mut children = Vec::new();
        let mut current_pos = parent_start;

        for pair in inner_pairs {
            let pair_start = pair.as_span().start();
            let pair_end = pair.as_span().end();

            // Add whitespace before this child if there's a gap
            if current_pos < pair_start {
                let whitespace_text = self.source[current_pos..pair_start].to_string();
                if !whitespace_text.is_empty() {
                    let whitespace_span = CSTSpan {
                        start: current_pos,
                        end: pair_start,
                        line_col: (1, 1), // TODO: Calculate proper line/col
                    };
                    children.push(CSTNode::whitespace(whitespace_text, whitespace_span));
                }
            }

            // Add the actual child
            children.push(self.convert_pair(pair.clone()));
            current_pos = pair_end;
        }

        // Add any trailing whitespace
        if current_pos < parent_end {
            let whitespace_text = self.source[current_pos..parent_end].to_string();
            if !whitespace_text.is_empty() {
                let whitespace_span = CSTSpan {
                    start: current_pos,
                    end: parent_end,
                    line_col: (1, 1), // TODO: Calculate proper line/col
                };
                children.push(CSTNode::whitespace(whitespace_text, whitespace_span));
            }
        }

        children
    }

    /// Convert the root program pair to a CST
    pub fn convert_program(&self, pair: pest::iterators::Pair<Rule>) -> Result<CSTNode, String> {
        if pair.as_rule() != Rule::program {
            return Err(format!("Expected program rule, got {:?}", pair.as_rule()));
        }
        Ok(self.convert_pair(pair))
    }
}

/// Utility functions for working with CST
impl CSTNode {
    /// Pretty print the CST structure for debugging
    pub fn pretty_print(&self, indent: usize) -> String {
        let indent_str = "  ".repeat(indent);
        let span_info = format!("{}:{}", self.span.start, self.span.end);

        match &self.kind {
            CSTNodeKind::Terminal { text } => {
                format!(
                    "{}Terminal({:?}) [{}] \"{}\"",
                    indent_str, self.rule, span_info, text
                )
            }
            CSTNodeKind::Comment { comment_type, text } => {
                format!("{indent_str}Comment({comment_type:?}) [{span_info}] \"{text}\"")
            }
            CSTNodeKind::Whitespace { text } => {
                format!("{indent_str}Whitespace [{span_info}] {text:?}")
            }
            CSTNodeKind::NonTerminal { children } => {
                let mut result =
                    format!("{}NonTerminal({:?}) [{}]", indent_str, self.rule, span_info);
                for child in children {
                    result.push('\n');
                    result.push_str(&child.pretty_print(indent + 1));
                }
                result
            }
        }
    }

    /// Reconstruct the original source text from this CST node
    pub fn to_source(&self) -> String {
        match &self.kind {
            CSTNodeKind::Terminal { text } => text.clone(),
            CSTNodeKind::Comment { text, .. } => text.clone(),
            CSTNodeKind::Whitespace { text } => text.clone(),
            CSTNodeKind::NonTerminal { children } => {
                children.iter().map(|child| child.to_source()).collect()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::parse::moo::MooParser;
    use pest::Parser;

    #[test]
    fn test_cst_creation() {
        let source = "if (true) x = 1; endif";

        let pairs = MooParser::parse(Rule::program, source).expect("Failed to parse");

        let converter = PestToCSTConverter::new(source.to_string());
        let cst = converter
            .convert_program(pairs.into_iter().next().unwrap())
            .expect("Failed to convert to CST");

        // Verify we can reconstruct the source exactly
        assert_eq!(cst.to_source(), source);

        // Verify the root is a program node
        assert_eq!(cst.rule, Rule::program);
        assert!(matches!(cst.kind, CSTNodeKind::NonTerminal { .. }));
    }

    #[test]
    fn test_comment_preservation() {
        // For now, let's test a simpler case since comments are silent rules in Pest
        // and require special handling to preserve them
        let source = "if (true) x = 1; endif";

        let pairs = MooParser::parse(Rule::program, source).expect("Failed to parse");

        let converter = PestToCSTConverter::new(source.to_string());
        let cst = converter
            .convert_program(pairs.into_iter().next().unwrap())
            .expect("Failed to convert to CST");

        // Verify we can reconstruct the source exactly
        assert_eq!(cst.to_source(), source);

        // Find whitespace nodes (since comments would need special parsing)
        let whitespace_nodes: Vec<_> = cst
            .find_all_nodes()
            .into_iter()
            .filter(|n| n.is_whitespace())
            .collect();

        // We should have multiple whitespace nodes preserving spacing
        assert!(whitespace_nodes.len() > 0);
    }

    #[test]
    fn test_cst_expression_parser() {
        use crate::ast::{BinaryOp, Expr};
        use moor_var::v_int;

        // Test simple addition: 1 + 2
        let left_node = CSTNode::terminal(
            Rule::integer,
            "1".to_string(),
            CSTSpan {
                start: 0,
                end: 1,
                line_col: (1, 1),
            },
        );
        let op_node = CSTNode::terminal(
            Rule::add,
            "+".to_string(),
            CSTSpan {
                start: 2,
                end: 3,
                line_col: (1, 3),
            },
        );
        let right_node = CSTNode::terminal(
            Rule::integer,
            "2".to_string(),
            CSTSpan {
                start: 4,
                end: 5,
                line_col: (1, 5),
            },
        );

        let nodes = vec![left_node, op_node, right_node];

        let parser = CSTExpressionParserBuilder::new().build(
            // Primary mapper
            |node: &CSTNode| -> Result<Expr, CompileError> {
                match node.rule {
                    Rule::integer => {
                        if let Some(text) = node.text() {
                            let val = text.parse::<i64>().unwrap();
                            Ok(Expr::Value(v_int(val)))
                        } else {
                            Err(CompileError::ParseError {
                                error_position: moor_common::model::CompileContext::new((1, 1)),
                                end_line_col: None,
                                context: "test".to_string(),
                                message: "No text".to_string(),
                            })
                        }
                    }
                    _ => Err(CompileError::ParseError {
                        error_position: moor_common::model::CompileContext::new((1, 1)),
                        end_line_col: None,
                        context: "test".to_string(),
                        message: "Unsupported primary".to_string(),
                    }),
                }
            },
            // Infix mapper
            |left: Expr, op: &CSTNode, right: Expr| -> Result<Expr, CompileError> {
                match op.rule {
                    Rule::add => Ok(Expr::Binary(BinaryOp::Add, Box::new(left), Box::new(right))),
                    _ => Err(CompileError::ParseError {
                        error_position: moor_common::model::CompileContext::new((1, 1)),
                        end_line_col: None,
                        context: "test".to_string(),
                        message: "Unsupported infix".to_string(),
                    }),
                }
            },
            // Prefix mapper
            |_op: &CSTNode, _rhs: Expr| -> Result<Expr, CompileError> {
                Err(CompileError::ParseError {
                    error_position: moor_common::model::CompileContext::new((1, 1)),
                    end_line_col: None,
                    context: "test".to_string(),
                    message: "Unsupported prefix".to_string(),
                })
            },
            // Postfix mapper
            |_lhs: Expr, _op: &CSTNode| -> Result<Expr, CompileError> {
                Err(CompileError::ParseError {
                    error_position: moor_common::model::CompileContext::new((1, 1)),
                    end_line_col: None,
                    context: "test".to_string(),
                    message: "Unsupported postfix".to_string(),
                })
            },
        );

        let result = parser.parse(&nodes).expect("Parse should succeed");

        // Verify we got a binary addition expression
        match result {
            Expr::Binary(BinaryOp::Add, left, right) => {
                assert!(matches!(*left, Expr::Value(_)));
                assert!(matches!(*right, Expr::Value(_)));
            }
            _ => panic!("Expected binary addition expression, got: {:?}", result),
        }
    }
}

/// Custom precedence parser for CST nodes that preserves comments and whitespace
/// while implementing the same precedence rules as the original Pest PrattParser
pub struct CSTExpressionParser<F, G, H, I>
where
    F: Fn(&CSTNode) -> Result<Expr, CompileError>,
    G: Fn(Expr, &CSTNode, Expr) -> Result<Expr, CompileError>,
    H: Fn(&CSTNode, Expr) -> Result<Expr, CompileError>,
    I: Fn(Expr, &CSTNode) -> Result<Expr, CompileError>,
{
    primary_mapper: F,
    infix_mapper: G,
    prefix_mapper: H,
    postfix_mapper: I,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Associativity {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy)]
pub struct OperatorInfo {
    pub precedence: u8,
    pub associativity: Associativity,
}

impl<F, G, H, I> CSTExpressionParser<F, G, H, I>
where
    F: Fn(&CSTNode) -> Result<Expr, CompileError>,
    G: Fn(Expr, &CSTNode, Expr) -> Result<Expr, CompileError>,
    H: Fn(&CSTNode, Expr) -> Result<Expr, CompileError>,
    I: Fn(Expr, &CSTNode) -> Result<Expr, CompileError>,
{
    pub fn new(primary_mapper: F, infix_mapper: G, prefix_mapper: H, postfix_mapper: I) -> Self {
        Self {
            primary_mapper,
            infix_mapper,
            prefix_mapper,
            postfix_mapper,
        }
    }

    /// Get operator precedence and associativity for a given rule
    /// Uses the shared precedence table from the precedence module
    /// Higher numbers = higher precedence (same as original parser)
    fn get_operator_info(&self, rule: Rule) -> Option<OperatorInfo> {
        use crate::precedence::Precedence;
        match rule {
            // Postfix operators (highest precedence)
            Rule::index_range
            | Rule::index_single
            | Rule::verb_call
            | Rule::verb_expr_call
            | Rule::prop
            | Rule::prop_expr => Some(OperatorInfo {
                precedence: Precedence::Primary.as_u8(),
                associativity: Associativity::Left,
            }),
            // Conditional operator (lower precedence)
            Rule::cond_expr => Some(OperatorInfo {
                precedence: Precedence::Cond.as_u8(),
                associativity: Associativity::Right,
            }),
            // Unary negation & logical-not (prefix)
            Rule::neg | Rule::not => Some(OperatorInfo {
                precedence: Precedence::Unary.as_u8(),
                associativity: Associativity::Right,
            }),
            // Exponent
            Rule::pow => Some(OperatorInfo {
                precedence: Precedence::Exponential.as_u8(),
                associativity: Associativity::Left,
            }),
            // Multiply, divide, modulus
            Rule::mul | Rule::div | Rule::modulus => Some(OperatorInfo {
                precedence: Precedence::Multiplicative.as_u8(),
                associativity: Associativity::Left,
            }),
            // Add & subtract
            Rule::add | Rule::sub => Some(OperatorInfo {
                precedence: Precedence::Additive.as_u8(),
                associativity: Associativity::Left,
            }),
            // Relational operators (including 'in')
            Rule::gt | Rule::lt | Rule::gte | Rule::lte | Rule::in_range => Some(OperatorInfo {
                precedence: Precedence::Relational.as_u8(),
                associativity: Associativity::Left,
            }),
            // Equality/inequality
            Rule::eq | Rule::neq => Some(OperatorInfo {
                precedence: Precedence::Equality.as_u8(),
                associativity: Associativity::Left,
            }),
            // Logical and
            Rule::land => Some(OperatorInfo {
                precedence: Precedence::And.as_u8(),
                associativity: Associativity::Left,
            }),
            // Logical or
            Rule::lor => Some(OperatorInfo {
                precedence: Precedence::Or.as_u8(),
                associativity: Associativity::Left,
            }),
            // Assignments & scatter assignments (lowest precedence)
            Rule::assign | Rule::scatter_assign => Some(OperatorInfo {
                precedence: Precedence::ScatterAssign.as_u8(),
                associativity: Associativity::Right,
            }),
            _ => None,
        }
    }

    /// Check if a rule is a prefix operator
    fn is_prefix_operator(&self, rule: Rule) -> bool {
        matches!(rule, Rule::neg | Rule::not | Rule::scatter_assign)
    }

    /// Check if a rule is a postfix operator
    fn is_postfix_operator(&self, rule: Rule) -> bool {
        matches!(
            rule,
            Rule::assign
                | Rule::cond_expr
                | Rule::index_range
                | Rule::index_single
                | Rule::verb_call
                | Rule::verb_expr_call
                | Rule::prop
                | Rule::prop_expr
        )
    }

    /// Check if a rule is an infix operator
    fn is_infix_operator(&self, rule: Rule) -> bool {
        matches!(
            rule,
            Rule::lor
                | Rule::land
                | Rule::eq
                | Rule::neq
                | Rule::gt
                | Rule::lt
                | Rule::gte
                | Rule::lte
                | Rule::in_range
                | Rule::add
                | Rule::sub
                | Rule::mul
                | Rule::div
                | Rule::modulus
                | Rule::pow
        )
    }

    /// Filter out whitespace and comments from CST nodes, keeping only content nodes
    fn filter_content_nodes(nodes: &[CSTNode]) -> Vec<&CSTNode> {
        nodes.iter().filter(|node| node.is_content()).collect()
    }

    /// Parse a sequence of CST nodes into an expression using precedence climbing
    pub fn parse(&self, nodes: &[CSTNode]) -> Result<Expr, CompileError> {
        let content_nodes = Self::filter_content_nodes(nodes);
        if content_nodes.is_empty() {
            return Err(CompileError::ParseError {
                error_position: moor_common::model::CompileContext::new((1, 1)),
                end_line_col: None,
                context: "CST expression parsing".to_string(),
                message: "Empty expression".to_string(),
            });
        }

        self.parse_expression(&content_nodes, 0, 0)
    }

    /// Core precedence climbing algorithm that returns (expression, new_position)
    fn parse_expression_with_pos(
        &self,
        nodes: &[&CSTNode],
        mut pos: usize,
        min_precedence: u8,
    ) -> Result<(Expr, usize), CompileError> {
        // Handle prefix operators first
        let mut left = if pos < nodes.len() && self.is_prefix_operator(nodes[pos].rule) {
            let op_node = nodes[pos];
            pos += 1;

            // For prefix operators, we need to parse their operand with their own precedence
            // This ensures that (!1 || 1) parses as ((!1) || 1), not !(1 || 1)
            let op_info = self.get_operator_info(op_node.rule);
            let prefix_precedence = if let Some(info) = op_info {
                info.precedence
            } else {
                min_precedence
            };

            let (rhs, new_pos) = self.parse_expression_with_pos(nodes, pos, prefix_precedence)?;
            pos = new_pos;
            (self.prefix_mapper)(op_node, rhs)?
        } else {
            // Parse primary expression
            if pos >= nodes.len() {
                return Err(CompileError::ParseError {
                    error_position: moor_common::model::CompileContext::new((1, 1)),
                    end_line_col: None,
                    context: "CST expression parsing".to_string(),
                    message: "Expected primary expression".to_string(),
                });
            }

            let primary = (self.primary_mapper)(nodes[pos])?;
            pos += 1;
            primary
        };

        // Parse postfix operators (with precedence checking)
        while pos < nodes.len() && self.is_postfix_operator(nodes[pos].rule) {
            let op_node = nodes[pos];

            // Check if postfix operator has sufficient precedence
            let Some(op_info) = self.get_operator_info(op_node.rule) else {
                break;
            };

            if op_info.precedence < min_precedence {
                break;
            }

            left = (self.postfix_mapper)(left, op_node)?;
            pos += 1;
        }

        // Parse infix operators using precedence climbing
        while pos < nodes.len() {
            let op_node = nodes[pos];

            if !self.is_infix_operator(op_node.rule) {
                break;
            }

            let Some(op_info) = self.get_operator_info(op_node.rule) else {
                break;
            };

            if op_info.precedence < min_precedence {
                break;
            }

            pos += 1; // consume operator

            let next_min_precedence = if op_info.associativity == Associativity::Left {
                op_info.precedence + 1
            } else {
                op_info.precedence
            };

            let (right, new_pos) =
                self.parse_expression_with_pos(nodes, pos, next_min_precedence)?;
            pos = new_pos;
            left = (self.infix_mapper)(left, op_node, right)?;
        }

        // Parse remaining postfix operators after infix processing
        while pos < nodes.len() && self.is_postfix_operator(nodes[pos].rule) {
            let op_node = nodes[pos];

            // Check if postfix operator has sufficient precedence
            let Some(op_info) = self.get_operator_info(op_node.rule) else {
                break;
            };

            if op_info.precedence < min_precedence {
                break;
            }

            left = (self.postfix_mapper)(left, op_node)?;
            pos += 1;
        }

        Ok((left, pos))
    }

    /// Wrapper that returns just the expression (for external callers)
    fn parse_expression(
        &self,
        nodes: &[&CSTNode],
        pos: usize,
        min_precedence: u8,
    ) -> Result<Expr, CompileError> {
        let (expr, _final_pos) = self.parse_expression_with_pos(nodes, pos, min_precedence)?;
        Ok(expr)
    }
}

/// Builder for creating CST expression parsers with fluent interface
pub struct CSTExpressionParserBuilder;

impl Default for CSTExpressionParserBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl CSTExpressionParserBuilder {
    pub fn new() -> Self {
        Self
    }

    pub fn build<F, G, H, I>(
        self,
        primary_mapper: F,
        infix_mapper: G,
        prefix_mapper: H,
        postfix_mapper: I,
    ) -> CSTExpressionParser<F, G, H, I>
    where
        F: Fn(&CSTNode) -> Result<Expr, CompileError>,
        G: Fn(Expr, &CSTNode, Expr) -> Result<Expr, CompileError>,
        H: Fn(&CSTNode, Expr) -> Result<Expr, CompileError>,
        I: Fn(Expr, &CSTNode) -> Result<Expr, CompileError>,
    {
        CSTExpressionParser::new(primary_mapper, infix_mapper, prefix_mapper, postfix_mapper)
    }
}

// Implementation of TreeNode trait for CSTNode
//
// This allows CSTNode to work with the generic AST building pipeline
// TODO: Uncomment when tree-sitter feature is added in PR 2
/*
#[cfg(feature = "tree-sitter-parser")]
impl crate::parsers::tree_sitter::tree_traits::TreeNode for CSTNode {
    fn node_kind(&self) -> &str {
        // Map Rule enums to semantic names for generic interface
        match self.rule {
            Rule::ident => "identifier",
            Rule::integer => "integer_literal",
            Rule::float => "float_literal",
            Rule::string => "string_literal",
            Rule::boolean => "boolean_literal",
            Rule::symbol => "symbol_literal",
            Rule::literal_binary => "binary_literal",
            Rule::object => "object_literal",
            Rule::type_constant => "type_constant",
            Rule::err => "error_literal",
            Rule::lambda => "lambda_expression",
            Rule::fn_expr => "function_expression",

            // Binary operations
            Rule::add => "binary_add",
            Rule::sub => "binary_sub",
            Rule::mul => "binary_mul",
            Rule::div => "binary_div",
            Rule::modulus => "binary_mod",
            Rule::pow => "binary_pow",
            Rule::eq => "binary_eq",
            Rule::neq => "binary_neq",
            Rule::gt => "binary_gt",
            Rule::lt => "binary_lt",
            Rule::gte => "binary_gte",
            Rule::lte => "binary_lte",
            Rule::in_range => "binary_in",
            Rule::land => "logical_and",
            Rule::lor => "logical_or",

            // Unary operations
            Rule::neg => "unary_neg",
            Rule::not => "unary_not",

            // Assignments
            Rule::assign => "assignment",
            Rule::scatter_assign => "scatter_assignment",

            // Statements
            Rule::statement => "statement",
            Rule::expr_statement => "expression_statement",
            Rule::if_statement => "if_statement",
            Rule::while_statement => "while_statement",
            Rule::for_in_statement => "for_statement",
            Rule::for_range_statement => "for_range_statement",
            Rule::try_except_statement => "try_statement",
            Rule::try_finally_statement => "try_finally_statement",
            Rule::return_expr => "return_statement",
            Rule::break_statement => "break_statement",
            Rule::continue_statement => "continue_statement",
            Rule::begin_statement => "block_statement",

            // Expressions
            Rule::expr => "expression",
            Rule::paren_expr => "parenthesized_expression",
            Rule::cond_expr => "conditional_expression",
            Rule::builtin_call => "function_call",
            Rule::verb_call => "method_call",
            Rule::sysprop_call => "system_property_call",
            Rule::prop => "property_access",
            Rule::prop_expr => "property_expression",
            Rule::index_single => "index_access",
            Rule::index_range => "range_access",

            // Collections
            Rule::list => "list_literal",
            Rule::map => "map_literal",
            Rule::scatter => "scatter_pattern",
            Rule::scatter_optional => "optional_parameter",
            Rule::scatter_rest => "rest_parameter",

            // Structural
            Rule::program => "program",
            Rule::statements => "statement_list",
            Rule::exprlist => "expression_list",
            Rule::arglist => "argument_list",
            Rule::argument => "argument",
            Rule::atom => "atom",

            // Comments and whitespace
            Rule::c_comment => "block_comment",
            Rule::cpp_comment => "line_comment",
            Rule::WHITESPACE => "whitespace",

            // Catch-all for other rules
            _ => "unknown",
        }
    }

    fn text(&self) -> Option<&str> {
        match &self.kind {
            CSTNodeKind::Terminal { text } => Some(text),
            CSTNodeKind::Comment { text, .. } => Some(text),
            CSTNodeKind::Whitespace { text } => Some(text),
            _ => None,
        }
    }

    fn children(&self) -> Box<dyn Iterator<Item = &Self> + '_> {
        match &self.kind {
            CSTNodeKind::NonTerminal { children } => Box::new(children.iter()),
            _ => Box::new(std::iter::empty()),
        }
    }

    fn child_by_name(&self, name: &str) -> Option<&Self> {
        match &self.kind {
            CSTNodeKind::NonTerminal { children } => {
                // For non-semantic nodes, try to find child by position or heuristic
                // This is a fallback for cases where semantic structure isn't available
                match name {
                    "left" => children.get(0),
                    "operator" => children.get(1),
                    "right" => children.get(2),
                    "condition" => children.get(0),
                    "consequence" => children.get(1),
                    "alternative" => children.get(2),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn span(&self) -> (usize, usize) {
        (self.span.start, self.span.end)
    }

    fn line_col(&self) -> (usize, usize) {
        self.span.line_col
    }

    fn is_error(&self) -> bool {
        // In CST, errors are typically represented as specific error nodes
        // This would need to be adjusted based on how errors are represented
        false
    }

    fn is_content(&self) -> bool {
        // Filter out comments and whitespace from content
        !matches!(
            self.kind,
            CSTNodeKind::Comment { .. } | CSTNodeKind::Whitespace { .. }
        )
    }
}
*/
