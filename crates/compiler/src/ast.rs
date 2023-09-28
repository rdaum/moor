use std::fmt::Display;

use moor_values::var::Var;

/// The abstract syntax tree produced by the parser and converted by codgen into opcodes.
use crate::labels::Name;
use crate::opcode::Op;

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
    pub id: Name,
    pub expr: Option<Expr>,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    NEq,
    Gt,
    GtE,
    Lt,
    LtE,
    Exp,
    In,
}

impl BinaryOp {
    pub fn from_binary_opcode(opcode: Op) -> Self {
        match opcode {
            Op::Add => Self::Add,
            Op::Sub => Self::Sub,
            Op::Mul => Self::Mul,
            Op::Div => Self::Div,
            Op::Mod => Self::Mod,
            Op::Eq => Self::Eq,
            Op::Ne => Self::NEq,
            Op::Gt => Self::Gt,
            Op::Ge => Self::GtE,
            Op::Lt => Self::Lt,
            Op::Le => Self::LtE,
            Op::Exp => Self::Exp,
            Op::In => Self::In,
            _ => panic!("Invalid binary opcode: {:?}", opcode),
        }
    }
}

impl Display for BinaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Add => write!(f, "+"),
            Self::Sub => write!(f, "-"),
            Self::Mul => write!(f, "*"),
            Self::Div => write!(f, "/"),
            Self::Mod => write!(f, "%"),
            Self::Eq => write!(f, "=="),
            Self::NEq => write!(f, "!="),
            Self::Gt => write!(f, ">"),
            Self::GtE => write!(f, ">="),
            Self::Lt => write!(f, "<"),
            Self::LtE => write!(f, "<="),
            Self::Exp => write!(f, "^"),
            Self::In => write!(f, "in"),
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
    VarExpr(Var),
    Id(Name),
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Unary(UnaryOp, Box<Expr>),
    Prop {
        location: Box<Expr>,
        property: Box<Expr>,
    },
    Call {
        function: String,
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
    Catch {
        trye: Box<Expr>,
        codes: CatchCodes,
        except: Option<Box<Expr>>,
    },
    Index(Box<Expr>, Box<Expr>),
    List(Vec<Arg>),
    Scatter(Vec<ScatterItem>, Box<Expr>),
    Length,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct CondArm {
    pub condition: Expr,
    pub statements: Vec<Stmt>,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct ExceptArm {
    pub id: Option<Name>,
    pub codes: CatchCodes,
    pub statements: Vec<Stmt>,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Stmt {
    pub node: StmtNode,
    /// The line number from the physical source code.
    /// Note that this is not necessarily the same as the line number that will be reported into
    /// codegen, and may not correspond to what shows as a result of `unparse`; that line number
    /// is derived from the AST, not the parser.
    /// TODO: I may or may not keep this field around.
    pub parser_line_no: usize,
    /// This line number is generated during a second pass over the tree, and is used to generate
    /// the line number spans in the bytecode.
    /// On first pass, this is set to 0.
    pub tree_line_no: usize,
}

impl Stmt {
    pub fn new(node: StmtNode, line: usize) -> Self {
        Stmt {
            node,
            parser_line_no: line,
            tree_line_no: 0,
        }
    }
}
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum StmtNode {
    Cond {
        arms: Vec<CondArm>,
        otherwise: Vec<Stmt>,
    },
    ForList {
        id: Name,
        expr: Expr,
        body: Vec<Stmt>,
    },
    ForRange {
        id: Name,
        from: Expr,
        to: Expr,
        body: Vec<Stmt>,
    },
    While {
        id: Option<Name>,
        condition: Expr,
        body: Vec<Stmt>,
    },
    Fork {
        id: Option<Name>,
        time: Expr,
        body: Vec<Stmt>,
    },
    TryExcept {
        body: Vec<Stmt>,
        excepts: Vec<ExceptArm>,
    },
    TryFinally {
        body: Vec<Stmt>,
        handler: Vec<Stmt>,
    },
    Break {
        exit: Option<Name>,
    },
    Continue {
        exit: Option<Name>,
    },
    Return {
        expr: Option<Expr>,
    },
    Expr(Expr),
}

// Recursive descent compare of two trees, ignoring the parser-provided line numbers, but
// validating equality for everything else.
#[cfg(test)]
pub fn assert_trees_match_recursive(a: &Vec<Stmt>, b: &Vec<Stmt>) {
    assert_eq!(a.len(), b.len());
    for (left, right) in a.iter().zip(b.iter()) {
        assert_eq!(left.tree_line_no, right.tree_line_no);

        match (&left.node, &right.node) {
            (StmtNode::Return { .. }, StmtNode::Return { .. }) => {}
            (StmtNode::Expr { .. }, StmtNode::Expr { .. }) => {}
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
                assert_trees_match_recursive(otherwise1, otherwise2);
                for arms in arms1.iter().zip(arms2.iter()) {
                    assert_eq!(arms.0.condition, arms.1.condition);
                    assert_trees_match_recursive(&arms.0.statements, &arms.1.statements);
                }
            }
            (
                StmtNode::TryFinally {
                    body: body1,
                    handler: handler1,
                },
                StmtNode::TryFinally {
                    body: body2,
                    handler: handler2,
                },
            ) => {
                assert_trees_match_recursive(body1, body2);
                assert_trees_match_recursive(handler1, handler2);
            }
            (StmtNode::TryExcept { body: body1, .. }, StmtNode::TryExcept { body: body2, .. })
            | (StmtNode::ForList { body: body1, .. }, StmtNode::ForList { body: body2, .. })
            | (StmtNode::ForRange { body: body1, .. }, StmtNode::ForRange { body: body2, .. })
            | (StmtNode::Fork { body: body1, .. }, StmtNode::Fork { body: body2, .. })
            | (StmtNode::While { body: body1, .. }, StmtNode::While { body: body2, .. }) => {
                assert_trees_match_recursive(body1, body2);
            }
            _ => {
                panic!("Mismatched statements: {:?} vs {:?}", left, right);
            }
        }
    }
}
