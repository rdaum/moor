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

//! Test macros for easier parse tree testing and validation

/// Assert that a CST node has a specific rule
#[macro_export]
macro_rules! assert_cst_rule {
    ($node:expr, $expected_rule:expr) => {
        assert_eq!(
            $node.rule,
            $expected_rule,
            "Expected rule {:?}, but got {:?} at {}:{}",
            $expected_rule,
            $node.rule,
            file!(),
            line!()
        );
    };
}

/// Assert that a CST node is a terminal with specific text
#[macro_export]
macro_rules! assert_cst_terminal {
    ($node:expr, $expected_text:expr) => {
        match &$node.kind {
            $crate::cst::CSTNodeKind::Terminal { text, .. } => {
                assert_eq!(
                    text,
                    $expected_text,
                    "Expected terminal text '{}', but got '{}' at {}:{}",
                    $expected_text,
                    text,
                    file!(),
                    line!()
                );
            }
            _ => panic!(
                "Expected terminal node, but got non-terminal at {}:{}",
                file!(),
                line!()
            ),
        }
    };
}

/// Assert that a CST node is a non-terminal with specific number of children
#[macro_export]
macro_rules! assert_cst_children_count {
    ($node:expr, $expected_count:expr) => {
        match &$node.kind {
            $crate::cst::CSTNodeKind::NonTerminal { children, .. } => {
                assert_eq!(
                    children.len(),
                    $expected_count,
                    "Expected {} children, but got {} at {}:{}",
                    $expected_count,
                    children.len(),
                    file!(),
                    line!()
                );
            }
            _ => panic!(
                "Expected non-terminal node, but got terminal at {}:{}",
                file!(),
                line!()
            ),
        }
    };
}

/// Assert the structure of a CST node and its children
#[macro_export]
macro_rules! assert_cst_structure {
    ($node:expr, $rule:expr, children: [$($child_rule:expr),*]) => {
        assert_cst_rule!($node, $rule);
        match &$node.kind {
            $crate::cst::CSTNodeKind::NonTerminal { children, .. } => {
                let expected_rules = vec![$($child_rule),*];
                assert_eq!(
                    children.len(), expected_rules.len(),
                    "Expected {} children, but got {} at {}:{}",
                    expected_rules.len(), children.len(), file!(), line!()
                );
                for (i, (child, expected_rule)) in children.iter().zip(expected_rules.iter()).enumerate() {
                    assert_eq!(
                        child.rule, *expected_rule,
                        "Child {} - expected rule {:?}, but got {:?} at {}:{}",
                        i, expected_rule, child.rule, file!(), line!()
                    );
                }
            }
            _ => panic!("Expected non-terminal node, but got terminal at {}:{}", file!(), line!()),
        }
    };
    ($node:expr, $rule:expr, terminal: $text:expr) => {
        assert_cst_rule!($node, $rule);
        assert_cst_terminal!($node, $text);
    };
}

/// Get a child node by index with proper error handling
#[macro_export]
macro_rules! get_cst_child {
    ($node:expr, $index:expr) => {
        match &$node.kind {
            $crate::cst::CSTNodeKind::NonTerminal { children, .. } => {
                children.get($index).unwrap_or_else(|| {
                    panic!(
                        "Child index {} out of bounds (node has {} children) at {}:{}",
                        $index,
                        children.len(),
                        file!(),
                        line!()
                    )
                })
            }
            _ => panic!(
                "Expected non-terminal node, but got terminal at {}:{}",
                file!(),
                line!()
            ),
        }
    };
}

/// Assert that a parse tree matches expected AST structure
#[macro_export]
macro_rules! assert_parse_tree {
    ($parse:expr, statements: $stmt_count:expr) => {
        assert_eq!(
            $parse.stmts.len(), $stmt_count,
            "Expected {} statements, but got {} at {}:{}",
            $stmt_count, $parse.stmts.len(), file!(), line!()
        );
    };
    ($parse:expr, statements: $stmt_count:expr, variables: $var_count:expr) => {
        assert_parse_tree!($parse, statements: $stmt_count);
        assert_eq!(
            $parse.variables.len(), $var_count,
            "Expected {} variables, but got {} at {}:{}",
            $var_count, $parse.variables.len(), file!(), line!()
        );
    };
}

/// Compare two parse results for equality with better error messages
#[macro_export]
macro_rules! assert_parsers_agree {
    ($pest_parse:expr, $ts_parse:expr, $source:expr) => {
        assert_eq!(
            $pest_parse.stmts.len(),
            $ts_parse.stmts.len(),
            "Parsers disagree on statement count for '{}': PEST={}, TreeSitter={}",
            $source,
            $pest_parse.stmts.len(),
            $ts_parse.stmts.len()
        );

        // Convert to Parse for comparison
        let pest_as_parse: $crate::Parse = $pest_parse.into();
        let ts_as_parse: $crate::Parse = $ts_parse.into();

        // Compare unparsed output
        let pest_unparsed = $crate::unparse::unparse(&pest_as_parse).unwrap();
        let ts_unparsed = $crate::unparse::unparse(&ts_as_parse).unwrap();

        assert_eq!(
            pest_unparsed, ts_unparsed,
            "Parsers produced different unparsed output for '{}'",
            $source
        );
    };
}

#[cfg(test)]
mod test_macro_tests {
    use crate::cst::{CSTNode, CSTNodeKind, CSTSpan};
    use crate::parsers::parse::moo::Rule;

    #[test]
    fn test_assert_cst_rule() {
        let node = CSTNode {
            rule: Rule::program,
            span: CSTSpan {
                start: 0,
                end: 10,
                line_col: (1, 1),
            },
            kind: CSTNodeKind::Terminal {
                text: "test".to_string(),
            },
        };
        assert_cst_rule!(node, Rule::program);
    }

    #[test]
    fn test_assert_cst_terminal() {
        let node = CSTNode {
            rule: Rule::ident,
            span: CSTSpan {
                start: 0,
                end: 4,
                line_col: (1, 1),
            },
            kind: CSTNodeKind::Terminal {
                text: "test".to_string(),
            },
        };
        assert_cst_terminal!(node, "test");
    }

    #[test]
    fn test_assert_cst_structure() {
        let child1 = CSTNode {
            rule: Rule::ident,
            span: CSTSpan {
                start: 0,
                end: 1,
                line_col: (1, 1),
            },
            kind: CSTNodeKind::Terminal {
                text: "x".to_string(),
            },
        };
        let child2 = CSTNode {
            rule: Rule::integer,
            span: CSTSpan {
                start: 4,
                end: 5,
                line_col: (1, 5),
            },
            kind: CSTNodeKind::Terminal {
                text: "1".to_string(),
            },
        };
        let node = CSTNode {
            rule: Rule::assign,
            span: CSTSpan {
                start: 0,
                end: 5,
                line_col: (1, 1),
            },
            kind: CSTNodeKind::NonTerminal {
                children: vec![child1, child2],
            },
        };

        assert_cst_structure!(node, Rule::assign, children: [Rule::ident, Rule::integer]);
    }
}
