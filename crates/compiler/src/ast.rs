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

use moor_var::Var;
/// The abstract syntax tree produced by the parser and converted by codegen into opcodes.
use moor_var::program::names::Variable;
use moor_var::program::opcode::Op;
use moor_var::{ErrorCode, Symbol, VarType};
use std::fmt::Display;

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Arg {
    Normal(Expr),
    Splice(Expr),
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum ScatterKind {
    Required,
    Optional,
    Rest,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct ScatterItem {
    pub kind: ScatterKind,
    pub id: Variable,
    pub expr: Option<Expr>,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum BinaryOp {
    Add,
    Div,
    Eq,
    Exp,
    Gt,
    GtE,
    In,
    Lt,
    LtE,
    Mod,
    Mul,
    NEq,
    Sub,
}

impl BinaryOp {
    pub fn from_binary_opcode(opcode: Op) -> Self {
        match opcode {
            Op::Add => Self::Add,
            Op::Div => Self::Div,
            Op::Eq => Self::Eq,
            Op::Exp => Self::Exp,
            Op::Ge => Self::GtE,
            Op::Gt => Self::Gt,
            Op::In => Self::In,
            Op::Le => Self::LtE,
            Op::Lt => Self::Lt,
            Op::Mod => Self::Mod,
            Op::Mul => Self::Mul,
            Op::Ne => Self::NEq,
            Op::Sub => Self::Sub,
            _ => panic!("Invalid binary opcode: {opcode:?}"),
        }
    }
}

impl Display for BinaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Add => write!(f, "+"),
            Self::Div => write!(f, "/"),
            Self::Eq => write!(f, "=="),
            Self::Exp => write!(f, "^"),
            Self::Gt => write!(f, ">"),
            Self::GtE => write!(f, ">="),
            Self::In => write!(f, "in"),
            Self::Lt => write!(f, "<"),
            Self::LtE => write!(f, "<="),
            Self::Mod => write!(f, "%"),
            Self::Mul => write!(f, "*"),
            Self::NEq => write!(f, "!="),
            Self::Sub => write!(f, "-"),
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum UnaryOp {
    Neg,
    Not,
}

impl Display for UnaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Neg => write!(f, "-"),
            Self::Not => write!(f, "!"),
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum CatchCodes {
    Codes(Vec<Arg>),
    Any,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Expr {
    Assign {
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Pass {
        args: Vec<Arg>,
    },
    TypeConstant(VarType),
    Value(Var),
    Error(ErrorCode, Option<Box<Expr>>),
    Id(Variable),
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Unary(UnaryOp, Box<Expr>),
    Prop {
        location: Box<Expr>,
        property: Box<Expr>,
    },
    Call {
        function: Symbol,
        args: Vec<Arg>,
    },
    Verb {
        location: Box<Expr>,
        verb: Box<Expr>,
        args: Vec<Arg>,
    },
    Range {
        base: Box<Expr>,
        from: Box<Expr>,
        to: Box<Expr>,
    },
    Cond {
        condition: Box<Expr>,
        consequence: Box<Expr>,
        alternative: Box<Expr>,
    },
    TryCatch {
        trye: Box<Expr>,
        codes: CatchCodes,
        except: Option<Box<Expr>>,
    },
    Index(Box<Expr>, Box<Expr>),
    List(Vec<Arg>),
    Map(Vec<(Expr, Expr)>),
    Flyweight(Box<Expr>, Vec<(Symbol, Expr)>, Option<Box<Expr>>),
    Scatter(Vec<ScatterItem>, Box<Expr>),
    Length,
    ComprehendList {
        variable: Variable,
        position_register: Variable,
        list_register: Variable,
        producer_expr: Box<Expr>,
        list: Box<Expr>,
    },
    ComprehendRange {
        variable: Variable,
        end_of_range_register: Variable,
        producer_expr: Box<Expr>,
        from: Box<Expr>,
        to: Box<Expr>,
    },
    Decl {
        id: Variable,
        is_const: bool,
        expr: Option<Box<Expr>>,
    },
    Return(Option<Box<Expr>>),
    Lambda {
        params: Vec<ScatterItem>,
        body: Box<Stmt>,
    },
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct CondArm {
    pub condition: Expr,
    pub statements: Vec<Stmt>,
    pub environment_width: usize,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct ElseArm {
    pub statements: Vec<Stmt>,
    pub environment_width: usize,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct ExceptArm {
    pub id: Option<Variable>,
    pub codes: CatchCodes,
    pub statements: Vec<Stmt>,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Stmt {
    pub node: StmtNode,
    /// The position from the physical source code.
    /// Note that this is not necessarily the same as the line number that will be reported into
    /// codegen, and may not correspond to what shows as a result of `unparse`; that line number
    /// is derived from the AST, not the parser.
    pub line_col: (usize, usize),
    /// This line number is generated during a second pass over the tree, and is used to generate
    /// the line number spans in the bytecode.
    /// On first pass, this is set to 0.
    pub tree_line_no: usize,
}

impl Stmt {
    pub fn new(node: StmtNode, line_col: (usize, usize)) -> Self {
        Stmt {
            node,
            line_col,
            tree_line_no: 0,
        }
    }
}
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum StmtNode {
    Cond {
        arms: Vec<CondArm>,
        otherwise: Option<ElseArm>,
    },
    ForList {
        value_binding: Variable,
        key_binding: Option<Variable>,
        expr: Expr,
        body: Vec<Stmt>,
        environment_width: usize,
    },
    ForRange {
        id: Variable,
        from: Expr,
        to: Expr,
        body: Vec<Stmt>,
        environment_width: usize,
    },
    While {
        id: Option<Variable>,
        condition: Expr,
        body: Vec<Stmt>,
        environment_width: usize,
    },
    Fork {
        id: Option<Variable>,
        time: Expr,
        body: Vec<Stmt>,
    },
    TryExcept {
        body: Vec<Stmt>,
        excepts: Vec<ExceptArm>,
        environment_width: usize,
    },
    TryFinally {
        body: Vec<Stmt>,
        handler: Vec<Stmt>,
        environment_width: usize,
    },
    Scope {
        /// The number of non-upfront variables in the scope (e.g. let statements)
        num_bindings: usize,
        /// The body of the let scope, which is evaluated with the bindings in place.
        body: Vec<Stmt>,
    },
    Break {
        exit: Option<Variable>,
    },
    Continue {
        exit: Option<Variable>,
    },
    Expr(Expr),
}

impl StmtNode {
    pub fn mk_return(expr: Expr) -> Self {
        StmtNode::Expr(Expr::Return(Some(Box::new(expr))))
    }
    pub fn mk_return_none() -> Self {
        StmtNode::Expr(Expr::Return(None))
    }
}

// Recursive descent compare of two trees, ignoring the parser-provided line numbers, but
// validating equality for everything else.
#[cfg(test)]
pub fn assert_trees_match_recursive(a: &[Stmt], b: &[Stmt]) {
    assert_eq!(a.len(), b.len());
    for (left, right) in a.iter().zip(b.iter()) {
        assert_eq!(left.tree_line_no, right.tree_line_no);

        match (&left.node, &right.node) {
            (StmtNode::Expr(e1), StmtNode::Expr(e2)) => {
                assert_eq!(e1, e2);
            }
            (StmtNode::Break { .. }, StmtNode::Break { .. }) => {}
            (StmtNode::Continue { .. }, StmtNode::Continue { .. }) => {}
            (
                StmtNode::Cond {
                    otherwise: otherwise1,
                    arms: arms1,
                    ..
                },
                StmtNode::Cond {
                    otherwise: otherwise2,
                    arms: arms2,
                    ..
                },
            ) => {
                match (otherwise1, otherwise2) {
                    (
                        Some(ElseArm { statements, .. }),
                        Some(ElseArm {
                            statements: statements2,
                            ..
                        }),
                    ) => {
                        assert_trees_match_recursive(statements, statements2);
                    }
                    (None, None) => {}
                    _ => panic!("Mismatched otherwise: {otherwise1:?} vs {otherwise2:?}"),
                }
                for arms in arms1.iter().zip(arms2.iter()) {
                    assert_eq!(arms.0.condition, arms.1.condition);
                    assert_trees_match_recursive(&arms.0.statements, &arms.1.statements);
                }
            }
            (
                StmtNode::TryFinally {
                    body: body1,
                    handler: handler1,
                    environment_width: ew1,
                },
                StmtNode::TryFinally {
                    body: body2,
                    handler: handler2,
                    environment_width: ew2,
                },
            ) => {
                assert_trees_match_recursive(body1, body2);
                assert_trees_match_recursive(handler1, handler2);
                assert_eq!(ew1, ew2);
            }
            (StmtNode::TryExcept { body: body1, .. }, StmtNode::TryExcept { body: body2, .. })
            | (StmtNode::ForList { body: body1, .. }, StmtNode::ForList { body: body2, .. })
            | (StmtNode::ForRange { body: body1, .. }, StmtNode::ForRange { body: body2, .. })
            | (StmtNode::Fork { body: body1, .. }, StmtNode::Fork { body: body2, .. })
            | (StmtNode::Scope { body: body1, .. }, StmtNode::Scope { body: body2, .. })
            | (StmtNode::While { body: body1, .. }, StmtNode::While { body: body2, .. }) => {
                assert_trees_match_recursive(body1, body2);
            }
            _ => {
                panic!(
                    "Mismatched statements:\n\
                {left:?}\n\
                vs\n\
                {right:?}"
                );
            }
        }
    }
}
