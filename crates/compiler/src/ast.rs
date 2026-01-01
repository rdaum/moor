// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

/// The abstract syntax tree produced by the parser and converted by codegen into opcodes.
use moor_var::program::names::Variable;
use moor_var::{ErrorCode, Symbol, Var, VarType, program::opcode::Op};
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
    BitAnd,
    BitOr,
    BitShl,
    BitShr,
    BitLShr,
    BitXor,
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
            Op::BitAnd => Self::BitAnd,
            Op::BitOr => Self::BitOr,
            Op::BitShl => Self::BitShl,
            Op::BitShr => Self::BitShr,
            Op::BitLShr => Self::BitLShr,
            Op::BitXor => Self::BitXor,
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
            Self::BitAnd => write!(f, "&."),
            Self::BitOr => write!(f, "|."),
            Self::BitShl => write!(f, "<<"),
            Self::BitShr => write!(f, ">>"),
            Self::BitLShr => write!(f, ">>>"),
            Self::BitXor => write!(f, "^."),
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
    BitNot,
}

impl Display for UnaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Neg => write!(f, "-"),
            Self::Not => write!(f, "!"),
            Self::BitNot => write!(f, "~"),
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum CatchCodes {
    Codes(Vec<Arg>),
    Any,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum CallTarget {
    Builtin(Symbol), // Compile-time known builtin function
    Expr(Box<Expr>), // Runtime expression that evaluates to callable
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
        function: CallTarget,
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
        self_name: Option<Variable>, // For recursive lambdas, the variable to assign self to
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
//
// These helpers are used by decompile tests to compare parsed vs decompiled ASTs.
// They handle cosmetic differences that arise because:
// 1. The parser records actual source positions; the decompiler uses placeholder (0,0)
// 2. The parser knows the exact num_bindings; the decompiler cannot determine this from bytecode
// 3. Scopes with num_bindings=0 are elided by codegen, so decompile may produce them when
//    parse doesn't (or vice versa for single-statement scopes)

/// Compare two expressions for equality, handling Lambdas specially by recursively
/// comparing their bodies with assert_stmts_match instead of direct equality.
/// This ignores line_col and num_bindings differences in lambda bodies.
/// Panics with a detailed message if expressions don't match.
#[cfg(test)]
pub fn assert_exprs_match(e1: &Expr, e2: &Expr) {
    match (e1, e2) {
        (
            Expr::Lambda {
                params: p1,
                body: b1,
                self_name: s1,
            },
            Expr::Lambda {
                params: p2,
                body: b2,
                self_name: s2,
            },
        ) => {
            assert_eq!(p1, p2, "Lambda params mismatch");
            assert_eq!(s1, s2, "Lambda self_name mismatch");
            assert_stmts_match(b1, b2);
        }
        (
            Expr::Decl {
                id: id1,
                is_const: c1,
                expr: e1,
            },
            Expr::Decl {
                id: id2,
                is_const: c2,
                expr: e2,
            },
        ) => {
            assert_eq!(id1, id2, "Decl id mismatch");
            assert_eq!(c1, c2, "Decl is_const mismatch");
            match (e1, e2) {
                (Some(e1), Some(e2)) => assert_exprs_match(e1, e2),
                (None, None) => {}
                _ => panic!("Decl expr mismatch: {e1:?} vs {e2:?}"),
            }
        }
        (
            Expr::Assign {
                left: l1,
                right: r1,
            },
            Expr::Assign {
                left: l2,
                right: r2,
            },
        ) => {
            assert_exprs_match(l1, l2);
            assert_exprs_match(r1, r2);
        }
        (Expr::List(items1), Expr::List(items2)) => {
            assert_eq!(items1.len(), items2.len(), "List length mismatch");
            for (a1, a2) in items1.iter().zip(items2.iter()) {
                assert_args_match(a1, a2);
            }
        }
        _ => assert_eq!(e1, e2),
    }
}

/// Compare two Args for equality, recursively handling Lambdas.
#[cfg(test)]
fn assert_args_match(a1: &Arg, a2: &Arg) {
    match (a1, a2) {
        (Arg::Normal(e1), Arg::Normal(e2)) => assert_exprs_match(e1, e2),
        (Arg::Splice(e1), Arg::Splice(e2)) => assert_exprs_match(e1, e2),
        _ => assert_eq!(a1, a2, "Arg type mismatch"),
    }
}

/// Compare two statements for equality, handling Lambdas specially.
/// Also handles the case where a Scope with num_bindings=0 and single statement
/// is equivalent to just that statement (codegen elides empty scopes).
#[cfg(test)]
fn assert_stmts_match(s1: &Stmt, s2: &Stmt) {
    // Unwrap single-statement scopes with no bindings (they're elided by codegen)
    // We need to handle both directions: decompiled may have Scope, parsed may not (or vice versa)
    let (node1, node2) = match (&s1.node, &s2.node) {
        // If one side is an empty scope with single statement, unwrap it
        (
            StmtNode::Scope {
                num_bindings: 0,
                body,
            },
            other,
        ) if body.len() == 1 => (&body[0].node, other),
        (
            other,
            StmtNode::Scope {
                num_bindings: 0,
                body,
            },
        ) if body.len() == 1 => (other, &body[0].node),
        (n1, n2) => (n1, n2),
    };

    match (node1, node2) {
        (StmtNode::Expr(e1), StmtNode::Expr(e2)) => {
            assert_exprs_match(e1, e2);
        }
        (StmtNode::Scope { body: b1, .. }, StmtNode::Scope { body: b2, .. }) => {
            assert_trees_match_recursive(b1, b2);
        }
        _ => {
            // For other statement types, use the regular recursive matching
            assert_trees_match_recursive(std::slice::from_ref(s1), std::slice::from_ref(s2));
        }
    }
}

#[cfg(test)]
pub fn assert_trees_match_recursive(a: &[Stmt], b: &[Stmt]) {
    assert_eq!(a.len(), b.len());
    for (left, right) in a.iter().zip(b.iter()) {
        assert_eq!(left.tree_line_no, right.tree_line_no);

        match (&left.node, &right.node) {
            (StmtNode::Expr(e1), StmtNode::Expr(e2)) => {
                assert_exprs_match(e1, e2);
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

// AST Visitor Pattern for traversing the entire AST
pub trait AstVisitor {
    fn visit_expr(&mut self, expr: &Expr);
    fn visit_stmt(&mut self, stmt: &Stmt);
    fn visit_stmt_node(&mut self, stmt_node: &StmtNode);

    // Default implementations that traverse all children
    fn walk_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Assign { left, right } => {
                self.visit_expr(left);
                self.visit_expr(right);
            }
            Expr::Pass { args } => {
                for arg in args {
                    self.walk_arg(arg);
                }
            }
            Expr::TypeConstant(_) => {}
            Expr::Value(_) => {}
            Expr::Error(_, opt_expr) => {
                if let Some(expr) = opt_expr {
                    self.visit_expr(expr);
                }
            }
            Expr::Id(_) => {
                // This is where variable references happen!
            }
            Expr::Binary(_, left, right) => {
                self.visit_expr(left);
                self.visit_expr(right);
            }
            Expr::And(left, right) | Expr::Or(left, right) => {
                self.visit_expr(left);
                self.visit_expr(right);
            }
            Expr::Unary(_, expr) => {
                self.visit_expr(expr);
            }
            Expr::Prop { location, property } => {
                self.visit_expr(location);
                self.visit_expr(property);
            }
            Expr::Call { function: _, args } => {
                for arg in args {
                    self.walk_arg(arg);
                }
            }
            Expr::Verb {
                location,
                verb,
                args,
            } => {
                self.visit_expr(location);
                self.visit_expr(verb);
                for arg in args {
                    self.walk_arg(arg);
                }
            }
            Expr::Range { base, from, to } => {
                self.visit_expr(base);
                self.visit_expr(from);
                self.visit_expr(to);
            }
            Expr::Cond {
                condition,
                consequence,
                alternative,
            } => {
                self.visit_expr(condition);
                self.visit_expr(consequence);
                self.visit_expr(alternative);
            }
            Expr::TryCatch {
                trye,
                codes: _,
                except,
            } => {
                self.visit_expr(trye);
                if let Some(except_expr) = except {
                    self.visit_expr(except_expr);
                }
            }
            Expr::Index(base, index) => {
                self.visit_expr(base);
                self.visit_expr(index);
            }
            Expr::List(args) => {
                for arg in args {
                    self.walk_arg(arg);
                }
            }
            Expr::Map(pairs) => {
                for (key, value) in pairs {
                    self.visit_expr(key);
                    self.visit_expr(value);
                }
            }
            Expr::Flyweight(delegate, slots, contents) => {
                self.visit_expr(delegate);
                for (_, slot_expr) in slots {
                    self.visit_expr(slot_expr);
                }
                if let Some(contents_expr) = contents {
                    self.visit_expr(contents_expr);
                }
            }
            Expr::Scatter(items, expr) => {
                for item in items {
                    if let Some(default_expr) = &item.expr {
                        self.visit_expr(default_expr);
                    }
                }
                self.visit_expr(expr);
            }
            Expr::Length => {}
            Expr::ComprehendList {
                variable: _,
                position_register: _,
                list_register: _,
                producer_expr,
                list,
            } => {
                self.visit_expr(producer_expr);
                self.visit_expr(list);
            }
            Expr::ComprehendRange {
                variable: _,
                end_of_range_register: _,
                producer_expr,
                from,
                to,
            } => {
                self.visit_expr(producer_expr);
                self.visit_expr(from);
                self.visit_expr(to);
            }
            Expr::Decl {
                id: _,
                is_const: _,
                expr,
            } => {
                if let Some(init_expr) = expr {
                    self.visit_expr(init_expr);
                }
            }
            Expr::Return(opt_expr) => {
                if let Some(expr) = opt_expr {
                    self.visit_expr(expr);
                }
            }
            Expr::Lambda {
                params,
                body,
                self_name: _,
            } => {
                // For lambda parameters, we visit them but they don't count as "captures"
                for param in params {
                    if let Some(default_expr) = &param.expr {
                        self.visit_expr(default_expr);
                    }
                }
                self.visit_stmt(body);
            }
        }
    }

    fn walk_arg(&mut self, arg: &Arg) {
        match arg {
            Arg::Normal(expr) | Arg::Splice(expr) => {
                self.visit_expr(expr);
            }
        }
    }

    fn walk_stmt(&mut self, stmt: &Stmt) {
        self.visit_stmt_node(&stmt.node);
    }

    fn walk_stmt_node(&mut self, stmt_node: &StmtNode) {
        match stmt_node {
            StmtNode::Cond { arms, otherwise } => {
                for arm in arms {
                    self.visit_expr(&arm.condition);
                    for stmt in &arm.statements {
                        self.visit_stmt(stmt);
                    }
                }
                if let Some(else_arm) = otherwise {
                    for stmt in &else_arm.statements {
                        self.visit_stmt(stmt);
                    }
                }
            }
            StmtNode::ForList {
                value_binding: _,
                key_binding: _,
                expr,
                body,
                environment_width: _,
            } => {
                self.visit_expr(expr);
                for stmt in body {
                    self.visit_stmt(stmt);
                }
            }
            StmtNode::ForRange {
                id: _,
                from,
                to,
                body,
                environment_width: _,
            } => {
                self.visit_expr(from);
                self.visit_expr(to);
                for stmt in body {
                    self.visit_stmt(stmt);
                }
            }
            StmtNode::While {
                id: _,
                condition,
                body,
                environment_width: _,
            } => {
                self.visit_expr(condition);
                for stmt in body {
                    self.visit_stmt(stmt);
                }
            }
            StmtNode::Fork { id: _, time, body } => {
                self.visit_expr(time);
                for stmt in body {
                    self.visit_stmt(stmt);
                }
            }
            StmtNode::TryExcept {
                body,
                excepts,
                environment_width: _,
            } => {
                for stmt in body {
                    self.visit_stmt(stmt);
                }
                for except in excepts {
                    // except.codes would need more analysis for CatchCodes::Codes
                    for stmt in &except.statements {
                        self.visit_stmt(stmt);
                    }
                }
            }
            StmtNode::TryFinally {
                body,
                handler,
                environment_width: _,
            } => {
                for stmt in body {
                    self.visit_stmt(stmt);
                }
                for stmt in handler {
                    self.visit_stmt(stmt);
                }
            }
            StmtNode::Scope {
                num_bindings: _,
                body,
            } => {
                for stmt in body {
                    self.visit_stmt(stmt);
                }
            }
            StmtNode::Break { exit: _ } | StmtNode::Continue { exit: _ } => {}
            StmtNode::Expr(expr) => {
                self.visit_expr(expr);
            }
        }
    }
}
