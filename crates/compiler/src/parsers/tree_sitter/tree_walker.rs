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

//! Semantic tree walker for robust tree-sitter to CST conversion
//! 
//! This module implements a three-phase conversion strategy:
//! 1. Semantic Discovery - Walk entire tree and catalog all nodes
//! 2. Semantic Analysis - Resolve field mappings and validate structure  
//! 3. Ordered Conversion - Convert in dependency order with full context

use std::collections::HashMap;
use tree_sitter::Node;

use crate::cst::{CSTNode, CSTNodeKind, CSTSpan};
use crate::parsers::parse::moo::Rule;
use moor_common::model::CompileError;

pub type NodeId = usize;

/// Semantic tree walker for robust tree-sitter conversion
pub struct SemanticTreeWalker<'a> {
    source: &'a str,
    semantic_map: HashMap<NodeId, SemanticNode>,
    next_id: NodeId,
    root_id: Option<NodeId>,
}

/// Semantic representation of a tree-sitter node with field relationships
#[derive(Debug, Clone)]
pub struct SemanticNode {
    pub node_type: String,
    pub fields: HashMap<String, NodeId>,
    pub children: Vec<NodeId>,
    pub span: CSTSpan,
    pub text: String,
    pub conversion_state: ConversionState,
    pub semantic_info: SemanticInfo,
}

/// State tracking for conversion process
#[derive(Debug, Clone, PartialEq)]
pub enum ConversionState {
    Pending,
    InProgress,
    Completed(CSTNode),
    Failed(String),
}

/// Semantic analysis results for a node
#[derive(Debug, Clone)]
pub struct SemanticInfo {
    pub field_mappings: Vec<FieldMapping>,
    pub structural_issues: Vec<StructuralIssue>,
    pub conversion_strategy: ConversionStrategy,
}


/// Field mapping information
#[derive(Debug, Clone)]
pub struct FieldMapping {
    pub field_name: String,
    pub target_node: NodeId,
}


/// Structural validation issues
#[derive(Debug, Clone)]
pub struct StructuralIssue {
    pub description: String,
    pub node_id: NodeId,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum IssueType {
    MissingRequiredField,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum Severity {
    Error,
}

/// Strategy for converting a node
#[derive(Debug, Clone)]
pub enum ConversionStrategy {
    FieldBased,
    ChildrenBased,
    TextBased,
}

impl<'a> SemanticTreeWalker<'a> {
    /// Create a new semantic tree walker
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            semantic_map: HashMap::new(),
            next_id: 0,
            root_id: None,
        }
    }

    /// Phase 1: Semantic Discovery - Walk entire tree and catalog all nodes
    pub fn discover_semantics(&mut self, root: &Node) -> Result<(), CompileError> {
        let root_id = self.walk_tree(root, None)?;
        self.root_id = Some(root_id);
        Ok(())
    }

    /// Phase 2: Semantic Analysis - Resolve field mappings and validate structure
    pub fn analyze_semantics(&mut self) -> Result<(), CompileError> {
        let node_ids: Vec<NodeId> = self.semantic_map.keys().cloned().collect();
        
        for node_id in node_ids {
            self.analyze_node_semantics(node_id)?;
        }
        
        Ok(())
    }

    /// Phase 3: Ordered Conversion - Convert with full semantic context
    pub fn convert_with_semantics(&mut self) -> Result<CSTNode, CompileError> {
        let root_id = self.root_id.ok_or_else(|| self.error("No root node found"))?;
        self.convert_semantic_node(root_id)
    }

    /// Walk the tree recursively and build semantic map
    fn walk_tree(&mut self, node: &Node, _parent_id: Option<NodeId>) -> Result<NodeId, CompileError> {
        let node_id = self.next_id;
        self.next_id += 1;

        let span = self.node_span(node);
        let text = self.node_text(node);
        
        // Create semantic node with initial state
        let semantic_node = SemanticNode {
            node_type: node.kind().to_string(),
            fields: HashMap::new(),
            children: Vec::new(),
            span,
            text,
            conversion_state: ConversionState::Pending,
            semantic_info: SemanticInfo {
                field_mappings: Vec::new(),
                structural_issues: Vec::new(),
                conversion_strategy: ConversionStrategy::FieldBased,
            },
        };

        self.semantic_map.insert(node_id, semantic_node);

        // Walk children and build field mappings
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                let child_id = self.walk_tree(&child, Some(node_id))?;
                
                // Add to children list
                if let Some(semantic_node) = self.semantic_map.get_mut(&node_id) {
                    semantic_node.children.push(child_id);
                }

                // Try to get field name for this child
                if let Some(field_name) = cursor.field_name() {
                    if let Some(semantic_node) = self.semantic_map.get_mut(&node_id) {
                        semantic_node.fields.insert(field_name.to_string(), child_id);
                    }
                }
                
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        Ok(node_id)
    }

    /// Analyze semantics for a specific node
    fn analyze_node_semantics(&mut self, node_id: NodeId) -> Result<(), CompileError> {
        let node_type = self.semantic_map[&node_id].node_type.clone();
        
        let (field_mappings, issues, strategy) = match node_type.as_str() {
            "if_statement" => self.analyze_if_statement(node_id)?,
            "method_call" => self.analyze_method_call(node_id)?,
            "function_call" | "call" => self.analyze_function_call(node_id)?,
            "for_statement" | "for_in_statement" => self.analyze_for_statement(node_id)?,
            "system_property" => self.analyze_system_property(node_id)?,
            "assignment_operation" | "assignment_expr" => self.analyze_assignment(node_id)?,
            "scatter_assignment" | "local_assign_scatter" | "scatter_assign" => self.analyze_scatter_assignment(node_id)?,
            "scatter_pattern" => self.analyze_scatter_pattern(node_id)?,
            "scatter_target" | "scatter_optional" | "scatter_rest" => self.analyze_scatter_item(node_id)?,
            "binary_operation" | "binary_expr" => self.analyze_binary_operation(node_id)?,
            "unary_operation" | "unary_expr" => self.analyze_unary_operation(node_id)?,
            "conditional_operation" | "conditional_expr" => self.analyze_conditional_operation(node_id)?,
            "property_access" | "property" => self.analyze_property_access(node_id)?,
            "index_access" | "index" => self.analyze_index_access(node_id)?,
            _ => (Vec::new(), Vec::new(), self.default_conversion_strategy(&node_type)),
        };

        // Update semantic info
        if let Some(semantic_node) = self.semantic_map.get_mut(&node_id) {
            semantic_node.semantic_info.field_mappings = field_mappings;
            semantic_node.semantic_info.structural_issues = issues;
            semantic_node.semantic_info.conversion_strategy = strategy;
        }

        Ok(())
    }

    /// Analyze if statement semantics
    fn analyze_if_statement(&mut self, node_id: NodeId) -> Result<(Vec<FieldMapping>, Vec<StructuralIssue>, ConversionStrategy), CompileError> {
        let semantic_node = &self.semantic_map[&node_id];
        let mut field_mappings = Vec::new();
        let mut issues = Vec::new();

        // Required: condition
        if let Some(&condition_id) = semantic_node.fields.get("condition") {
            field_mappings.push(FieldMapping {
                field_name: "condition".to_string(),
                target_node: condition_id,
            });
        } else {
            issues.push(StructuralIssue {
                description: "If statement missing condition field".to_string(),
                node_id: 0, // Will be set properly by caller
            });
        }

        // Required: then_body (the correct field name from grammar)
        if let Some(&body_id) = semantic_node.fields.get("then_body") {
            field_mappings.push(FieldMapping {
                field_name: "then_body".to_string(),
                target_node: body_id,
            });
        } else {
            issues.push(StructuralIssue {
                description: "If statement missing then_body field".to_string(),
                node_id: 0, // Will be set properly by caller
            });
        }

        // Optional: else_clause
        if let Some(&else_id) = semantic_node.fields.get("else_clause") {
            field_mappings.push(FieldMapping {
                field_name: "else_clause".to_string(),
                target_node: else_id,
            });
        }

        let strategy = ConversionStrategy::FieldBased;

        Ok((field_mappings, issues, strategy))
    }

    /// Analyze method call semantics  
    fn analyze_method_call(&mut self, node_id: NodeId) -> Result<(Vec<FieldMapping>, Vec<StructuralIssue>, ConversionStrategy), CompileError> {
        let semantic_node = &self.semantic_map[&node_id];
        let field_mappings = Vec::new();
        let issues = Vec::new();

        // Try to identify if this is a system property call
        let _is_sysprop_call = if let Some(&object_id) = semantic_node.fields.get("object") {
            self.is_system_property_node(object_id)
        } else {
            false
        };

        let strategy = ConversionStrategy::FieldBased;

        Ok((field_mappings, issues, strategy))
    }

    /// Analyze function call semantics
    fn analyze_function_call(&mut self, _node_id: NodeId) -> Result<(Vec<FieldMapping>, Vec<StructuralIssue>, ConversionStrategy), CompileError> {
        let strategy = ConversionStrategy::FieldBased {
        };

        Ok((Vec::new(), Vec::new(), strategy))
    }

    /// Analyze for statement semantics
    fn analyze_for_statement(&mut self, _node_id: NodeId) -> Result<(Vec<FieldMapping>, Vec<StructuralIssue>, ConversionStrategy), CompileError> {
        let strategy = ConversionStrategy::FieldBased {
        };

        Ok((Vec::new(), Vec::new(), strategy))
    }

    /// Analyze system property semantics
    fn analyze_system_property(&mut self, _node_id: NodeId) -> Result<(Vec<FieldMapping>, Vec<StructuralIssue>, ConversionStrategy), CompileError> {
        let strategy = ConversionStrategy::FieldBased {
        };

        Ok((Vec::new(), Vec::new(), strategy))
    }

    /// Analyze assignment semantics
    fn analyze_assignment(&mut self, _node_id: NodeId) -> Result<(Vec<FieldMapping>, Vec<StructuralIssue>, ConversionStrategy), CompileError> {
        let strategy = ConversionStrategy::FieldBased {
        };

        Ok((Vec::new(), Vec::new(), strategy))
    }

    /// Analyze binary operation semantics
    fn analyze_binary_operation(&mut self, _node_id: NodeId) -> Result<(Vec<FieldMapping>, Vec<StructuralIssue>, ConversionStrategy), CompileError> {
        let strategy = ConversionStrategy::FieldBased {
        };

        Ok((Vec::new(), Vec::new(), strategy))
    }

    /// Analyze unary operation semantics
    fn analyze_unary_operation(&mut self, _node_id: NodeId) -> Result<(Vec<FieldMapping>, Vec<StructuralIssue>, ConversionStrategy), CompileError> {
        let strategy = ConversionStrategy::FieldBased {
        };

        Ok((Vec::new(), Vec::new(), strategy))
    }

    /// Analyze conditional operation semantics
    fn analyze_conditional_operation(&mut self, _node_id: NodeId) -> Result<(Vec<FieldMapping>, Vec<StructuralIssue>, ConversionStrategy), CompileError> {
        let strategy = ConversionStrategy::FieldBased {
        };

        Ok((Vec::new(), Vec::new(), strategy))
    }

    /// Analyze property access semantics
    fn analyze_property_access(&mut self, _node_id: NodeId) -> Result<(Vec<FieldMapping>, Vec<StructuralIssue>, ConversionStrategy), CompileError> {
        let strategy = ConversionStrategy::FieldBased {
        };

        Ok((Vec::new(), Vec::new(), strategy))
    }

    /// Analyze index access semantics
    fn analyze_index_access(&mut self, _node_id: NodeId) -> Result<(Vec<FieldMapping>, Vec<StructuralIssue>, ConversionStrategy), CompileError> {
        let strategy = ConversionStrategy::FieldBased;
        Ok((Vec::new(), Vec::new(), strategy))
    }

    /// Analyze scatter assignment semantics
    fn analyze_scatter_assignment(&mut self, node_id: NodeId) -> Result<(Vec<FieldMapping>, Vec<StructuralIssue>, ConversionStrategy), CompileError> {
        let semantic_node = &self.semantic_map[&node_id];
        let mut field_mappings = Vec::new();
        let mut issues = Vec::new();
        
        // Find pattern and rhs in children
        for (field_name, &child_id) in &semantic_node.fields {
            let child_type = &self.semantic_map[&child_id].node_type;
            if child_type == "scatter_assign" || child_type == "scatter_pattern" {
                field_mappings.push(FieldMapping {
                    field_name: "pattern".to_string(),
                    target_node: child_id,
                });
            } else if field_name == "rhs" || child_type.contains("expr") {
                field_mappings.push(FieldMapping {
                    field_name: "rhs".to_string(),
                    target_node: child_id,
                });
            }
        }
        
        if field_mappings.len() < 2 {
            issues.push(StructuralIssue {
                description: "Scatter assignment missing pattern or expression".to_string(),
                node_id: 0,
            });
        }
        
        Ok((field_mappings, issues, ConversionStrategy::FieldBased))
    }

    /// Analyze scatter pattern semantics
    fn analyze_scatter_pattern(&mut self, node_id: NodeId) -> Result<(Vec<FieldMapping>, Vec<StructuralIssue>, ConversionStrategy), CompileError> {
        let semantic_node = &self.semantic_map[&node_id];
        let mut field_mappings = Vec::new();
        let issues = Vec::new();
        
        // Map all scatter items
        let mut item_count = 0;
        for &child_id in &semantic_node.children {
            let child_type = &self.semantic_map[&child_id].node_type;
            if child_type == "scatter_target" || child_type == "scatter_optional" || child_type == "scatter_rest" {
                field_mappings.push(FieldMapping {
                    field_name: format!("item_{}", item_count),
                    target_node: child_id,
                });
                item_count += 1;
            }
        }
        
        Ok((field_mappings, issues, ConversionStrategy::FieldBased))
    }

    /// Analyze scatter item semantics
    fn analyze_scatter_item(&mut self, node_id: NodeId) -> Result<(Vec<FieldMapping>, Vec<StructuralIssue>, ConversionStrategy), CompileError> {
        let semantic_node = &self.semantic_map[&node_id];
        let mut field_mappings = Vec::new();
        let mut issues = Vec::new();
        
        // Find identifier and optional default expression
        for &child_id in &semantic_node.children {
            let child_type = &self.semantic_map[&child_id].node_type;
            if child_type == "identifier" || child_type == "ident" {
                field_mappings.push(FieldMapping {
                    field_name: "identifier".to_string(),
                    target_node: child_id,
                });
            } else if child_type.contains("expr") {
                field_mappings.push(FieldMapping {
                    field_name: "default".to_string(),
                    target_node: child_id,
                });
            }
        }
        
        // Check that we have at least an identifier
        if !field_mappings.iter().any(|m| m.field_name == "identifier") {
            issues.push(StructuralIssue {
                description: "Scatter item missing identifier".to_string(),
                node_id: 0,
            });
        }
        
        Ok((field_mappings, issues, ConversionStrategy::FieldBased))
    }

    /// Convert a semantic node to CST using semantic analysis
    fn convert_semantic_node(&mut self, node_id: NodeId) -> Result<CSTNode, CompileError> {
        // Check if already converted
        if let ConversionState::Completed(cst_node) = &self.semantic_map[&node_id].conversion_state {
            return Ok(cst_node.clone());
        }

        // Mark as in progress
        if let Some(semantic_node) = self.semantic_map.get_mut(&node_id) {
            semantic_node.conversion_state = ConversionState::InProgress;
        }

        let result = self.perform_semantic_conversion(node_id);

        // Update conversion state
        match &result {
            Ok(cst_node) => {
                if let Some(semantic_node) = self.semantic_map.get_mut(&node_id) {
                    semantic_node.conversion_state = ConversionState::Completed(cst_node.clone());
                }
            }
            Err(error) => {
                if let Some(semantic_node) = self.semantic_map.get_mut(&node_id) {
                    semantic_node.conversion_state = ConversionState::Failed(error.to_string());
                }
            }
        }

        result
    }

    /// Perform the actual conversion based on semantic analysis
    fn perform_semantic_conversion(&mut self, node_id: NodeId) -> Result<CSTNode, CompileError> {
        let semantic_node = self.semantic_map[&node_id].clone();
        let span = semantic_node.span.clone();

        match semantic_node.node_type.as_str() {
            "program" | "source_file" => self.convert_semantic_program(node_id),
            "if_statement" => self.convert_semantic_if_statement(node_id),
            "method_call" => self.convert_semantic_method_call(node_id),
            "function_call" | "call" => self.convert_semantic_function_call(node_id),
            "system_property" => self.convert_semantic_system_property(node_id),
            "assignment_operation" | "assignment_expr" => self.convert_semantic_assignment(node_id),
            "let_statement" | "scatter_assignment" | "local_assign_scatter" | "scatter_assign" => self.convert_semantic_scatter_assignment(node_id),
            "binding_pattern" | "scatter_pattern" => self.convert_semantic_scatter_pattern(node_id),
            "binding_optional" | "scatter_target" | "scatter_optional" | "scatter_rest" => self.convert_semantic_scatter_item(node_id),
            "binary_operation" | "binary_expr" => self.convert_semantic_binary_operation(node_id),
            "unary_operation" | "unary_expr" => self.convert_semantic_unary_operation(node_id),
            "conditional_operation" | "conditional_expr" => self.convert_semantic_conditional_operation(node_id),
            "property_access" | "property" => self.convert_semantic_property_access(node_id),
            "index_access" | "index" => self.convert_semantic_index_access(node_id),
            "for_statement" => self.convert_semantic_for_statement(node_id),
            "return_expression" => self.convert_semantic_return_expression(node_id),
            // Literals
            "identifier" => Ok(self.create_terminal(Rule::ident, &semantic_node.text, span)),
            "integer" | "INTEGER" => Ok(self.create_terminal(Rule::integer, &semantic_node.text, span)),
            "float" => Ok(self.create_terminal(Rule::float, &semantic_node.text, span)),
            "string" => Ok(self.create_terminal(Rule::string, &semantic_node.text, span)),
            "boolean" => Ok(self.create_terminal(Rule::boolean, &semantic_node.text, span)),
            // Fallback to children conversion
            _ => self.convert_semantic_children(node_id),
        }
    }

    /// Convert program node semantically
    fn convert_semantic_program(&mut self, node_id: NodeId) -> Result<CSTNode, CompileError> {
        let child_ids = self.semantic_map[&node_id].children.clone();
        let span = self.semantic_map[&node_id].span.clone();
        let mut statements = Vec::new();

        // Convert all statement children
        for child_id in child_ids {
            let child_node_type = self.semantic_map[&child_id].node_type.clone();
            if child_node_type != "comment" && !self.is_punctuation(&child_node_type) {
                statements.push(self.convert_semantic_node(child_id)?);
            }
        }

        // Create program structure
        let mut children = Vec::new();
        if !statements.is_empty() {
            let statements_span = CSTSpan {
                start: statements.first().unwrap().span.start,
                end: statements.last().unwrap().span.end,
                line_col: statements.first().unwrap().span.line_col,
            };

            children.push(CSTNode {
                rule: Rule::statements,
                span: statements_span,
                kind: CSTNodeKind::NonTerminal { children: statements },
            });
        }

        // Add EOI
        children.push(CSTNode {
            rule: Rule::EOI,
            span: CSTSpan {
                start: span.end,
                end: span.end,
                line_col: (span.line_col.0, span.line_col.1 + 1),
            },
            kind: CSTNodeKind::Terminal { text: String::new() },
        });

        Ok(CSTNode {
            rule: Rule::program,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    /// Convert if statement semantically
    fn convert_semantic_if_statement(&mut self, node_id: NodeId) -> Result<CSTNode, CompileError> {
        let field_mappings = self.semantic_map[&node_id].semantic_info.field_mappings.clone();
        let span = self.semantic_map[&node_id].span.clone();
        let mut children = Vec::new();

        // Use semantic analysis to find required components
        for mapping in &field_mappings {
            if mapping.field_name == "condition" || mapping.field_name == "then_body" {
                children.push(self.convert_semantic_node(mapping.target_node)?);
            } else if mapping.field_name == "else_clause" {
                children.push(self.convert_semantic_node(mapping.target_node)?);
            }
        }

        if children.len() < 2 {
            return Err(self.error("If statement missing required components"));
        }

        Ok(CSTNode {
            rule: Rule::if_statement,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    /// Convert method call semantically
    fn convert_semantic_method_call(&mut self, node_id: NodeId) -> Result<CSTNode, CompileError> {
        let _conversion_strategy = self.semantic_map[&node_id].semantic_info.conversion_strategy.clone();
        let fields = self.semantic_map[&node_id].fields.clone();
        let span = self.semantic_map[&node_id].span.clone();
        
        // For now, use regular method call conversion

        // Regular method call
        let mut children = Vec::new();

        if let Some(&object_id) = fields.get("object") {
            children.push(self.convert_semantic_node(object_id)?);
        }

        if let Some(&method_id) = fields.get("method") {
            children.push(self.convert_semantic_node(method_id)?);
        }

        // Handle arguments
        let args = if let Some(&args_id) = fields.get("arguments") {
            self.extract_argument_list(args_id)?
        } else {
            Vec::new()
        };

        let arglist = self.create_arglist(args, span.clone());
        children.push(arglist);

        Ok(CSTNode {
            rule: Rule::verb_call,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    /// Convert system property call
    #[allow(dead_code)]
    fn convert_system_property_call(&mut self, node_id: NodeId) -> Result<CSTNode, CompileError> {
        let fields = self.semantic_map[&node_id].fields.clone();
        let span = self.semantic_map[&node_id].span.clone();
        let mut children = Vec::new();

        if let Some(&object_id) = fields.get("object") {
            children.push(self.convert_semantic_node(object_id)?);
        }

        if let Some(&method_id) = fields.get("method") {
            children.push(self.convert_semantic_node(method_id)?);
        }

        let args = if let Some(&args_id) = fields.get("arguments") {
            self.extract_argument_list(args_id)?
        } else {
            Vec::new()
        };

        let arglist = self.create_arglist(args, span.clone());
        children.push(arglist);

        Ok(CSTNode {
            rule: Rule::sysprop_call,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    /// Convert function call semantically
    fn convert_semantic_function_call(&mut self, node_id: NodeId) -> Result<CSTNode, CompileError> {
        let fields = self.semantic_map[&node_id].fields.clone();
        let span = self.semantic_map[&node_id].span.clone();
        let mut children = Vec::new();

        if let Some(&function_id) = fields.get("function") {
            children.push(self.convert_semantic_node(function_id)?);
        }

        let args = if let Some(&args_id) = fields.get("arguments") {
            self.extract_argument_list(args_id)?
        } else {
            Vec::new()
        };

        let arglist = self.create_arglist(args, span.clone());
        children.push(arglist);

        Ok(CSTNode {
            rule: Rule::builtin_call,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    /// Convert scatter assignment semantically
    fn convert_semantic_scatter_assignment(&mut self, node_id: NodeId) -> Result<CSTNode, CompileError> {
        let fields = self.semantic_map[&node_id].fields.clone();
        let span = self.semantic_map[&node_id].span.clone();
        let mut children = Vec::new();
        
        // Add scatter pattern
        if let Some(&pattern_id) = fields.get("pattern") {
            children.push(self.convert_semantic_node(pattern_id)?);
        }
        
        // Add assignment operator
        children.push(CSTNode {
            rule: Rule::assign,
            span: span.clone(),
            kind: CSTNodeKind::Terminal { text: "=".to_string() },
        });
        
        // Add right-hand side expression
        if let Some(&rhs_id) = fields.get("rhs") {
            children.push(self.convert_semantic_node(rhs_id)?);
        }
        
        Ok(CSTNode {
            rule: Rule::local_assign_scatter,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    /// Convert scatter pattern semantically
    fn convert_semantic_scatter_pattern(&mut self, node_id: NodeId) -> Result<CSTNode, CompileError> {
        let fields = self.semantic_map[&node_id].fields.clone();
        let span = self.semantic_map[&node_id].span.clone();
        let mut children = Vec::new();
        
        // Add opening brace
        children.push(CSTNode {
            rule: Rule::expr,
            span: span.clone(),
            kind: CSTNodeKind::Terminal { text: "{".to_string() },
        });
        
        // Add scatter items
        for (field_name, &item_id) in &fields {
            if field_name.starts_with("item_") {
                children.push(self.convert_semantic_node(item_id)?);
            }
        }
        
        // Add closing brace
        children.push(CSTNode {
            rule: Rule::expr,
            span: span.clone(),
            kind: CSTNodeKind::Terminal { text: "}".to_string() },
        });
        
        Ok(CSTNode {
            rule: Rule::scatter_assign,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    /// Convert scatter item semantically (required, optional, or rest)
    fn convert_semantic_scatter_item(&mut self, node_id: NodeId) -> Result<CSTNode, CompileError> {
        let semantic_node = &self.semantic_map[&node_id];
        let node_type = &semantic_node.node_type;
        
        match node_type.as_str() {
            "scatter_target" => self.convert_scatter_required(node_id),
            "scatter_optional" => self.convert_scatter_optional(node_id),
            "scatter_rest" => self.convert_scatter_rest(node_id),
            _ => Err(self.error(&format!("Unknown scatter item type: {}", node_type)))
        }
    }
    
    /// Convert required scatter item (plain identifier)
    fn convert_scatter_required(&mut self, node_id: NodeId) -> Result<CSTNode, CompileError> {
        let id_node = self.semantic_map[&node_id].fields.get("identifier")
            .copied()
            .ok_or_else(|| self.error("Scatter target missing identifier"))?;
            
        self.convert_semantic_node(id_node)
    }
    
    /// Convert optional scatter item (?identifier or ?identifier = default)
    fn convert_scatter_optional(&mut self, node_id: NodeId) -> Result<CSTNode, CompileError> {
        let (span, id_node, default_node) = {
            let semantic_node = &self.semantic_map[&node_id];
            (
                semantic_node.span.clone(),
                semantic_node.fields.get("identifier").copied(),
                semantic_node.fields.get("default").copied()
            )
        };
        
        let mut children = vec![
            self.create_terminal_node("?", span.clone())
        ];
        
        // Add identifier
        if let Some(id_node) = id_node {
            children.push(self.convert_semantic_node(id_node)?);
        }
        
        // Add default expression if present
        if let Some(default_node) = default_node {
            children.push(self.create_terminal_node("=", span.clone()));
            children.push(self.convert_semantic_node(default_node)?);
        }
        
        Ok(CSTNode {
            rule: Rule::scatter_optional,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }
    
    /// Convert rest scatter item (@identifier)
    fn convert_scatter_rest(&mut self, node_id: NodeId) -> Result<CSTNode, CompileError> {
        let (span, id_node) = {
            let semantic_node = &self.semantic_map[&node_id];
            (
                semantic_node.span.clone(),
                semantic_node.fields.get("identifier").copied()
            )
        };
        
        let mut children = vec![
            self.create_terminal_node("@", span.clone())
        ];
        
        // Add identifier
        if let Some(id_node) = id_node {
            children.push(self.convert_semantic_node(id_node)?);
        }
        
        Ok(CSTNode {
            rule: Rule::scatter_rest,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }
    
    /// Create a terminal node with given text and span
    fn create_terminal_node(&self, text: &str, span: CSTSpan) -> CSTNode {
        CSTNode {
            rule: Rule::expr,
            span,
            kind: CSTNodeKind::Terminal { text: text.to_string() },
        }
    }

    /// Convert system property semantically
    fn convert_semantic_system_property(&mut self, node_id: NodeId) -> Result<CSTNode, CompileError> {
        let fields = self.semantic_map[&node_id].fields.clone();
        let children = self.semantic_map[&node_id].children.clone();
        let span = self.semantic_map[&node_id].span.clone();
        
        let prop_name = if let Some(&name_id) = fields.get("name") {
            self.semantic_map[&name_id].text.clone()
        } else {
            // Fallback to finding identifier child
            for child_id in children {
                let child = &self.semantic_map[&child_id];
                if child.node_type == "identifier" {
                    return Ok(CSTNode {
                        rule: Rule::sysprop,
                        span,
                        kind: CSTNodeKind::Terminal { text: format!("${}", child.text) },
                    });
                }
            }
            return Err(self.error("System property has no identifier"));
        };

        Ok(CSTNode {
            rule: Rule::sysprop,
            span,
            kind: CSTNodeKind::Terminal { text: format!("${}", prop_name) },
        })
    }

    /// Convert assignment semantically
    fn convert_semantic_assignment(&mut self, node_id: NodeId) -> Result<CSTNode, CompileError> {
        let fields = self.semantic_map[&node_id].fields.clone();
        let span = self.semantic_map[&node_id].span.clone();
        let mut children = Vec::new();

        if let Some(&left_id) = fields.get("left") {
            children.push(self.convert_semantic_node(left_id)?);
        }

        // Add assignment operator
        children.push(CSTNode {
            rule: Rule::assign,
            span: span.clone(),
            kind: CSTNodeKind::Terminal { text: "=".to_string() },
        });

        if let Some(&right_id) = fields.get("right") {
            children.push(self.convert_semantic_node(right_id)?);
        }

        Ok(CSTNode {
            rule: Rule::assign,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    /// Convert binary operation semantically
    fn convert_semantic_binary_operation(&mut self, node_id: NodeId) -> Result<CSTNode, CompileError> {
        let fields = self.semantic_map[&node_id].fields.clone();
        let span = self.semantic_map[&node_id].span.clone();
        let mut children = Vec::new();

        if let Some(&left_id) = fields.get("left") {
            children.push(self.convert_semantic_node(left_id)?);
        }

        if let Some(&operator_id) = fields.get("operator") {
            children.push(self.convert_semantic_node(operator_id)?);
        }

        if let Some(&right_id) = fields.get("right") {
            children.push(self.convert_semantic_node(right_id)?);
        }

        Ok(CSTNode {
            rule: Rule::expr,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    /// Convert unary operation semantically
    fn convert_semantic_unary_operation(&mut self, node_id: NodeId) -> Result<CSTNode, CompileError> {
        let fields = self.semantic_map[&node_id].fields.clone();
        let span = self.semantic_map[&node_id].span.clone();
        let mut children = Vec::new();

        if let Some(&operator_id) = fields.get("operator") {
            children.push(self.convert_semantic_node(operator_id)?);
        }

        if let Some(&operand_id) = fields.get("operand") {
            children.push(self.convert_semantic_node(operand_id)?);
        }

        Ok(CSTNode {
            rule: Rule::expr,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    /// Convert conditional operation semantically
    fn convert_semantic_conditional_operation(&mut self, node_id: NodeId) -> Result<CSTNode, CompileError> {
        let fields = self.semantic_map[&node_id].fields.clone();
        let span = self.semantic_map[&node_id].span.clone();
        let mut children = Vec::new();

        if let Some(&condition_id) = fields.get("condition") {
            children.push(self.convert_semantic_node(condition_id)?);
        }

        if let Some(&consequence_id) = fields.get("consequence") {
            children.push(self.convert_semantic_node(consequence_id)?);
        }

        if let Some(&alternative_id) = fields.get("alternative") {
            children.push(self.convert_semantic_node(alternative_id)?);
        }

        Ok(CSTNode {
            rule: Rule::cond_expr,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    /// Convert property access semantically
    fn convert_semantic_property_access(&mut self, node_id: NodeId) -> Result<CSTNode, CompileError> {
        let fields = self.semantic_map[&node_id].fields.clone();
        let span = self.semantic_map[&node_id].span.clone();
        let mut children = Vec::new();

        if let Some(&object_id) = fields.get("object") {
            children.push(self.convert_semantic_node(object_id)?);
        }

        if let Some(&property_id) = fields.get("property") {
            children.push(self.convert_semantic_node(property_id)?);
        }

        Ok(CSTNode {
            rule: Rule::prop,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    /// Convert index access semantically
    fn convert_semantic_index_access(&mut self, node_id: NodeId) -> Result<CSTNode, CompileError> {
        let fields = self.semantic_map[&node_id].fields.clone();
        let span = self.semantic_map[&node_id].span.clone();
        let mut children = Vec::new();

        if let Some(&object_id) = fields.get("object") {
            children.push(self.convert_semantic_node(object_id)?);
        }

        if let Some(&index_id) = fields.get("index") {
            children.push(self.convert_semantic_node(index_id)?);
        }

        Ok(CSTNode {
            rule: Rule::index_single,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    /// Convert for statement semantically
    fn convert_semantic_for_statement(&mut self, node_id: NodeId) -> Result<CSTNode, CompileError> {
        let fields = self.semantic_map[&node_id].fields.clone();
        let span = self.semantic_map[&node_id].span.clone();
        let mut children = Vec::new();

        if let Some(&variable_id) = fields.get("variable") {
            children.push(self.convert_semantic_node(variable_id)?);
        }

        if let Some(&iterable_id) = fields.get("iterable") {
            children.push(self.convert_semantic_node(iterable_id)?);
        }

        if let Some(&body_id) = fields.get("body") {
            children.push(self.convert_semantic_node(body_id)?);
        }

        Ok(CSTNode {
            rule: Rule::for_in_statement,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    /// Convert return expression semantically
    fn convert_semantic_return_expression(&mut self, node_id: NodeId) -> Result<CSTNode, CompileError> {
        let fields = self.semantic_map[&node_id].fields.clone();
        let span = self.semantic_map[&node_id].span.clone();
        let mut children = Vec::new();

        // Add optional value
        if let Some(&value_id) = fields.get("value") {
            children.push(self.convert_semantic_node(value_id)?);
            Ok(CSTNode {
                rule: Rule::return_expr,
                span,
                kind: CSTNodeKind::NonTerminal { children },
            })
        } else {
            Ok(CSTNode {
                rule: Rule::empty_return,
                span,
                kind: CSTNodeKind::Terminal { text: "return".to_string() },
            })
        }
    }

    /// Convert children nodes when no specific strategy applies
    fn convert_semantic_children(&mut self, node_id: NodeId) -> Result<CSTNode, CompileError> {
        let child_ids = self.semantic_map[&node_id].children.clone();
        let span = self.semantic_map[&node_id].span.clone();
        let text = self.semantic_map[&node_id].text.clone();
        let mut children = Vec::new();

        for child_id in child_ids {
            let child_node_type = self.semantic_map[&child_id].node_type.clone();
            if child_node_type != "comment" && !self.is_punctuation(&child_node_type) {
                children.push(self.convert_semantic_node(child_id)?);
            }
        }

        if children.is_empty() {
            Ok(CSTNode {
                rule: Rule::expr,
                span,
                kind: CSTNodeKind::Terminal { text },
            })
        } else {
            Ok(CSTNode {
                rule: Rule::expr,
                span,
                kind: CSTNodeKind::NonTerminal { children },
            })
        }
    }

    /// Helper methods

    fn default_conversion_strategy(&self, node_type: &str) -> ConversionStrategy {
        match node_type {
            "identifier" | "integer" | "INTEGER" | "float" | "string" | "boolean" => {
                ConversionStrategy::TextBased
            }
            _ => ConversionStrategy::ChildrenBased
        }
    }

    fn is_system_property_node(&self, node_id: NodeId) -> bool {
        self.semantic_map[&node_id].node_type == "system_property"
    }

    fn is_punctuation(&self, node_type: &str) -> bool {
        matches!(node_type, "(" | ")" | "{" | "}" | "[" | "]" | ";" | "," | "." | ":" | "?" | "|" | "=" | "$")
    }

    fn extract_argument_list(&mut self, args_id: NodeId) -> Result<Vec<CSTNode>, CompileError> {
        let node_type = self.semantic_map[&args_id].node_type.clone();
        let child_ids = self.semantic_map[&args_id].children.clone();
        let mut args = Vec::new();

        if node_type == "argument_list" {
            for child_id in child_ids {
                let child_node_type = self.semantic_map[&child_id].node_type.clone();
                if child_node_type == "expression" {
                    args.push(self.convert_semantic_node(child_id)?);
                }
            }
        } else {
            // Single argument
            args.push(self.convert_semantic_node(args_id)?);
        }

        Ok(args)
    }

    fn create_arglist(&self, args: Vec<CSTNode>, span: CSTSpan) -> CSTNode {
        let wrapped_args: Vec<CSTNode> = args.into_iter().map(|arg| {
            CSTNode {
                rule: Rule::argument,
                span: arg.span.clone(),
                kind: CSTNodeKind::NonTerminal { children: vec![arg] },
            }
        }).collect();

        let exprlist = CSTNode {
            rule: Rule::exprlist,
            span: span.clone(),
            kind: CSTNodeKind::NonTerminal { children: wrapped_args },
        };

        CSTNode {
            rule: Rule::arglist,
            span,
            kind: CSTNodeKind::NonTerminal { children: vec![exprlist] },
        }
    }

    fn create_terminal(&self, rule: Rule, text: &str, span: CSTSpan) -> CSTNode {
        CSTNode {
            rule,
            span,
            kind: CSTNodeKind::Terminal { text: text.to_string() },
        }
    }

    fn node_span(&self, node: &Node) -> CSTSpan {
        let start_pos = node.start_position();
        CSTSpan {
            start: node.start_byte(),
            end: node.end_byte(),
            line_col: (start_pos.row + 1, start_pos.column + 1),
        }
    }

    fn node_text(&self, node: &Node) -> String {
        let start = node.start_byte();
        let end = node.end_byte();
        self.source[start..end].to_string()
    }

    fn error(&self, message: &str) -> CompileError {
        self.error_at_node(None, message)
    }

    /// Create a detailed error with node context and position information
    fn error_at_node(&self, node: Option<&tree_sitter::Node>, message: &str) -> CompileError {
        let (line, col, end_line_col) = if let Some(node) = node {
            let start_pos = node.start_position();
            let end_pos = node.end_position();
            (
                start_pos.row + 1,
                start_pos.column + 1,
                Some((end_pos.row + 1, end_pos.column + 1)),
            )
        } else {
            (1, 1, None)
        };

        CompileError::ParseError {
            error_position: moor_common::model::CompileContext::new((line, col)),
            context: format!("semantic tree walker: {}", message),
            end_line_col,
            message: message.to_string(),
        }
    }

    /// Debug: Print semantic analysis results
    pub fn debug_semantic_analysis(&self) -> String {
        let mut output = String::new();
        output.push_str("=== SEMANTIC ANALYSIS ===\n");
        
        for (node_id, semantic_node) in &self.semantic_map {
            output.push_str(&format!("\nNode {}: {} ({})\n", 
                node_id, semantic_node.node_type, semantic_node.text));
            
            if !semantic_node.fields.is_empty() {
                output.push_str("  Fields:\n");
                for (field_name, target_id) in &semantic_node.fields {
                    output.push_str(&format!("    {}: Node {}\n", field_name, target_id));
                }
            }
            
            if !semantic_node.semantic_info.structural_issues.is_empty() {
                output.push_str("  Issues:\n");
                for issue in &semantic_node.semantic_info.structural_issues {
                    output.push_str(&format!("    {}\n", issue.description));
                }
            }
        }
        
        output
    }
}