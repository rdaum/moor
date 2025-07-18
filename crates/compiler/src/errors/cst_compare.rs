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

use crate::cst::{CSTNode, CSTNodeKind};
use crate::parsers::parse::moo::Rule;
use std::fmt::Write;

#[derive(Debug, Clone)]
pub struct CSTDifference {
    /// Path to the node where the difference was found (e.g., "program.statements[0].expr[1]")
    pub path: String,
    /// Description of the difference
    pub description: String,
    /// Expected value (from PEST)
    pub expected: Option<String>,
    /// Actual value (from tree-sitter)
    pub actual: Option<String>,
}

#[derive(Default)]
pub struct CSTComparator {
    differences: Vec<CSTDifference>,
}

impl CSTComparator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Compare two CST trees and return a list of differences
    pub fn compare(&mut self, pest_cst: &CSTNode, ts_cst: &CSTNode) -> Vec<CSTDifference> {
        self.differences.clear();
        self.compare_nodes(pest_cst, ts_cst, "root");
        self.differences.clone()
    }

    /// Recursively compare two CST nodes
    fn compare_nodes(&mut self, pest: &CSTNode, ts: &CSTNode, path: &str) {
        // Compare rules
        if pest.rule != ts.rule {
            self.differences.push(CSTDifference {
                path: path.to_string(),
                description: "Different rules".to_string(),
                expected: Some(format!("{:?}", pest.rule)),
                actual: Some(format!("{:?}", ts.rule)),
            });
            // Don't continue comparing if rules differ
            return;
        }

        // Compare node kinds
        match (&pest.kind, &ts.kind) {
            (
                CSTNodeKind::Terminal { text: pest_text },
                CSTNodeKind::Terminal { text: ts_text },
            ) => {
                if pest_text != ts_text {
                    self.differences.push(CSTDifference {
                        path: path.to_string(),
                        description: "Different terminal text".to_string(),
                        expected: Some(pest_text.clone()),
                        actual: Some(ts_text.clone()),
                    });
                }
            }
            (
                CSTNodeKind::NonTerminal {
                    children: pest_children,
                },
                CSTNodeKind::NonTerminal {
                    children: ts_children,
                },
            ) => {
                // Compare children counts
                if pest_children.len() != ts_children.len() {
                    self.differences.push(CSTDifference {
                        path: path.to_string(),
                        description: "Different number of children".to_string(),
                        expected: Some(format!("{} children", pest_children.len())),
                        actual: Some(format!("{} children", ts_children.len())),
                    });

                    // Show what's missing or extra
                    if pest_children.len() > ts_children.len() {
                        for i in ts_children.len()..pest_children.len() {
                            let child_path = format!("{path}.children[{i}]");
                            self.differences.push(CSTDifference {
                                path: child_path,
                                description: "Missing child in tree-sitter".to_string(),
                                expected: Some(format!("{:?}", pest_children[i].rule)),
                                actual: None,
                            });
                        }
                    } else {
                        for i in pest_children.len()..ts_children.len() {
                            let child_path = format!("{path}.children[{i}]");
                            self.differences.push(CSTDifference {
                                path: child_path,
                                description: "Extra child in tree-sitter".to_string(),
                                expected: None,
                                actual: Some(format!("{:?}", ts_children[i].rule)),
                            });
                        }
                    }
                }

                // Compare children that exist in both
                let min_children = pest_children.len().min(ts_children.len());
                for i in 0..min_children {
                    let child_path = self.build_child_path(path, &pest_children[i], i);
                    self.compare_nodes(&pest_children[i], &ts_children[i], &child_path);
                }
            }
            _ => {
                self.differences.push(CSTDifference {
                    path: path.to_string(),
                    description: "Different node kinds".to_string(),
                    expected: Some(self.kind_description(&pest.kind)),
                    actual: Some(self.kind_description(&ts.kind)),
                });
            }
        }
    }

    /// Build a descriptive path for a child node
    fn build_child_path(&self, parent_path: &str, child: &CSTNode, index: usize) -> String {
        // Use rule name if it's meaningful, otherwise use index
        match child.rule {
            Rule::ident => format!("{parent_path}.ident"),
            Rule::expr => format!("{parent_path}.expr[{index}]"),
            Rule::statement => format!("{parent_path}.statement[{index}]"),
            Rule::statements => format!("{parent_path}.statements"),
            Rule::assign => format!("{parent_path}.assign"),
            Rule::list => format!("{parent_path}.list"),
            Rule::map => format!("{parent_path}.map"),
            Rule::exprlist => format!("{parent_path}.exprlist"),
            Rule::argument => format!("{parent_path}.argument[{index}]"),
            Rule::while_statement => format!("{parent_path}.while_statement"),
            Rule::if_statement => format!("{parent_path}.if_statement"),
            _ => format!("{parent_path}.children[{index}]"),
        }
    }

    fn kind_description(&self, kind: &CSTNodeKind) -> String {
        match kind {
            CSTNodeKind::Terminal { .. } => "Terminal".to_string(),
            CSTNodeKind::NonTerminal { .. } => "NonTerminal".to_string(),
            CSTNodeKind::Comment { .. } => "Comment".to_string(),
            CSTNodeKind::Whitespace { .. } => "Whitespace".to_string(),
        }
    }
}

/// Format differences for display
pub fn format_cst_differences(differences: &[CSTDifference]) -> String {
    let mut output = String::new();

    if differences.is_empty() {
        writeln!(&mut output, "No differences found - CSTs match!").unwrap();
        return output;
    }

    writeln!(&mut output, "Found {} differences:", differences.len()).unwrap();
    writeln!(&mut output).unwrap();

    for diff in differences {
        writeln!(&mut output, "Path: {}", diff.path).unwrap();
        writeln!(&mut output, "  Issue: {}", diff.description).unwrap();
        if let Some(expected) = &diff.expected {
            writeln!(&mut output, "  Expected (PEST): {expected}").unwrap();
        }
        if let Some(actual) = &diff.actual {
            writeln!(&mut output, "  Actual (TS):     {actual}").unwrap();
        }
        writeln!(&mut output).unwrap();
    }

    output
}

/// Create a detailed tree representation of a CST for debugging
pub fn cst_to_tree_string(node: &CSTNode, indent: usize) -> String {
    let mut output = String::new();
    let indent_str = "  ".repeat(indent);

    match &node.kind {
        CSTNodeKind::Terminal { text } => {
            writeln!(&mut output, "{}{:?}: '{}'", indent_str, node.rule, text).unwrap();
        }
        CSTNodeKind::NonTerminal { children } => {
            writeln!(
                &mut output,
                "{}{:?}: {} children",
                indent_str,
                node.rule,
                children.len()
            )
            .unwrap();
            for child in children {
                output.push_str(&cst_to_tree_string(child, indent + 1));
            }
        }
        _ => {
            writeln!(
                &mut output,
                "{}{:?}: {:?}",
                indent_str, node.rule, node.kind
            )
            .unwrap();
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cst_comparison() {
        // Test identical CSTs
        let cst1 = CSTNode {
            rule: Rule::expr,
            span: crate::cst::CSTSpan {
                start: 0,
                end: 1,
                line_col: (1, 1),
            },
            kind: CSTNodeKind::Terminal {
                text: "x".to_string(),
            },
        };

        let cst2 = cst1.clone();

        let mut comparator = CSTComparator::new();
        let differences = comparator.compare(&cst1, &cst2);
        assert!(differences.is_empty());
    }
}
