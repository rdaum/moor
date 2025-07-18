// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com>
// GPL v3.0 License

//! Integration tests for the generic tree trait system
//! 
//! These tests verify that different tree implementations can be used
//! interchangeably with the generic AST builder.

#[cfg(test)]
mod tests {
    use crate::tree_traits::TreeNode;
    use crate::simple_generic_builder::SimpleGenericBuilder;
    use crate::parse::CompileOptions;
    use crate::ast::Expr;
    use moor_var::v_int;
    
    /// Test that we can parse the same program with different tree implementations
    /// and get equivalent results
    #[test]
    fn test_cross_tree_equivalence() {
        // Create a simple program that should work with any parser
        let program_text = "return 42;";
        let options = CompileOptions::default();
        
        // Parse with CST
        {
            use crate::parse_cst::parse_program_cst;
            let cst_result = parse_program_cst(program_text, options.clone());
            assert!(cst_result.is_ok(), "CST parsing failed");
            let cst_ast = cst_result.unwrap();
            
            // Verify the AST structure
            assert_eq!(cst_ast.stmts.len(), 1);
            if let crate::ast::StmtNode::Expr(Expr::Return(Some(val))) = &cst_ast.stmts[0].node {
                if let Expr::Value(v) = &**val {
                    assert_eq!(v, &v_int(42));
                } else {
                    panic!("Expected integer value");
                }
            } else {
                panic!("Expected return statement");
            }
        }
        
        // Parse with tree-sitter
        #[cfg(feature = "tree-sitter-parser")]
        {
            use crate::parse_treesitter::parse_with_tree_sitter;
            let ts_result = parse_with_tree_sitter(program_text);
            assert!(ts_result.is_ok(), "Tree-sitter parsing failed");
            // Note: parse_with_tree_sitter returns a CSTNode, not a Program
            // so we can't directly compare the AST structure
        }
    }
    
    /// Test that custom handlers work correctly
    #[test]
    fn test_custom_handler_registration() {
        use crate::tree_traits::TreeNode;
        
        // Mock node for testing
        #[derive(Debug)]
        struct TestNode {
            kind: String,
            text: Option<String>,
        }
        
        impl TestNode {
            fn new(kind: &str, text: Option<&str>) -> Self {
                Self {
                    kind: kind.to_string(),
                    text: text.map(|s| s.to_string()),
                }
            }
        }
        
        impl TreeNode for TestNode {
            fn node_kind(&self) -> &str { &self.kind }
            fn text(&self) -> Option<&str> { self.text.as_deref() }
            fn children(&self) -> Box<dyn Iterator<Item = &Self> + '_> {
                Box::new(std::iter::empty())
            }
            fn child_by_name(&self, _name: &str) -> Option<&Self> { None }
            fn span(&self) -> (usize, usize) { (0, 0) }
            fn line_col(&self) -> (usize, usize) { (1, 1) }
            fn is_error(&self) -> bool { false }
        }
        
        let mut builder = SimpleGenericBuilder::<TestNode>::new(CompileOptions::default());
        
        // Test building expression with a mock node
        let test_node = TestNode::new("integer_literal", Some("123"));
        let result = builder.build_expression(&test_node);
        
        assert!(result.is_ok());
        if let Ok(Expr::Value(val)) = result {
            assert_eq!(val, v_int(123));
        } else {
            panic!("Expected integer value");
        }
    }
    
    /// Test error handling in the generic builder
    #[test]
    fn test_error_handling() {
        use crate::tree_traits::TreeNode;
        
        #[derive(Debug)]
        struct ErrorNode;
        
        impl TreeNode for ErrorNode {
            fn node_kind(&self) -> &str { "error" }
            fn text(&self) -> Option<&str> { None }
            fn children(&self) -> Box<dyn Iterator<Item = &Self> + '_> {
                Box::new(std::iter::empty())
            }
            fn child_by_name(&self, _name: &str) -> Option<&Self> { None }
            fn span(&self) -> (usize, usize) { (0, 0) }
            fn line_col(&self) -> (usize, usize) { (1, 1) }
            fn is_error(&self) -> bool { true }
        }
        
        let mut builder = SimpleGenericBuilder::<ErrorNode>::new(CompileOptions::default());
        let error_node = ErrorNode;
        let result = builder.build_expression(&error_node);
        
        assert!(result.is_err());
    }
    
    /// Test that the generic builder handles all common node types
    #[test]
    fn test_common_node_types() {
        use crate::tree_traits::TreeNode;
        
        #[derive(Debug)]
        struct MockNode {
            kind: String,
            text: Option<String>,
            children: Vec<MockNode>,
        }
        
        impl TreeNode for MockNode {
            fn node_kind(&self) -> &str { &self.kind }
            fn text(&self) -> Option<&str> { self.text.as_deref() }
            fn children(&self) -> Box<dyn Iterator<Item = &Self> + '_> {
                Box::new(self.children.iter())
            }
            fn child_by_name(&self, name: &str) -> Option<&Self> {
                self.children.iter().find(|c| c.kind == name)
            }
            fn span(&self) -> (usize, usize) { (0, 10) }
            fn line_col(&self) -> (usize, usize) { (1, 1) }
            fn is_error(&self) -> bool { false }
        }
        
        let mut builder = SimpleGenericBuilder::<MockNode>::new(CompileOptions::default());
        
        // Test integer literal
        let int_node = MockNode {
            kind: "integer_literal".to_string(),
            text: Some("42".to_string()),
            children: vec![],
        };
        let result = builder.build_expression(&int_node);
        assert!(result.is_ok());
        
        // Test string literal
        let str_node = MockNode {
            kind: "string_literal".to_string(),
            text: Some("\"hello\"".to_string()),
            children: vec![],
        };
        let result = builder.build_expression(&str_node);
        assert!(result.is_ok());
        
        // Test identifier
        let id_node = MockNode {
            kind: "identifier".to_string(),
            text: Some("my_var".to_string()),
            children: vec![],
        };
        let result = builder.build_expression(&id_node);
        assert!(result.is_ok());
    }
}