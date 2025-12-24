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

/// Compare two statement trees for semantic equality, ignoring line position metadata.
///
/// This is used for roundtrip testing: parse → compile → decompile → unparse → parse again.
/// Both trees come from the parser, so they have consistent structure. We only need to
/// ignore `line_col` and `tree_line_no` which differ due to different source text.
///
/// Panics with a detailed diff message if trees don't match.
/// Macro for comparing AST nodes while ignoring position metadata.
/// Provides detailed mismatch diagnostics with path tracking.
macro_rules! assert_ast_eq {
    // Entry point for statement slices
    (stmts: $a:expr, $b:expr) => {{
        let a = $a;
        let b = $b;
        if a.len() != b.len() {
            panic!(
                "Statement count mismatch: {} vs {}\nLeft:  {a:?}\nRight: {b:?}",
                a.len(),
                b.len()
            );
        }
        for (i, (left, right)) in a.iter().zip(b.iter()).enumerate() {
            assert_ast_eq!(stmt: left, right, format!("stmt[{i}]"));
        }
    }};

    // Compare single statements with path tracking
    (stmt: $a:expr, $b:expr, $path:expr) => {{
        if !stmt_nodes_equal(&$a.node, &$b.node) {
            panic!(
                "Mismatch at {}:\nLeft:  {:?}\nRight: {:?}",
                $path, $a.node, $b.node
            );
        }
    }};
}

pub fn assert_stmts_equal_ignoring_pos(a: &[Stmt], b: &[Stmt]) {
    assert_ast_eq!(stmts: a, b);
}

/// Deep equality check for StmtNode, recursively checking children but ignoring
/// position metadata in nested Stmts.
fn stmt_nodes_equal(a: &StmtNode, b: &StmtNode) -> bool {
    match (a, b) {
        (StmtNode::Expr(e1), StmtNode::Expr(e2)) => exprs_equal(e1, e2),
        (StmtNode::Break { exit }, StmtNode::Break { exit: exit2 }) => exit == exit2,
        (StmtNode::Continue { exit }, StmtNode::Continue { exit: exit2 }) => exit == exit2,
        (
            StmtNode::Cond { arms: a1, otherwise: o1 },
            StmtNode::Cond { arms: a2, otherwise: o2 },
        ) => {
            a1.len() == a2.len()
                && a1.iter().zip(a2.iter()).all(|(arm1, arm2)| {
                    exprs_equal(&arm1.condition, &arm2.condition)
                        && stmts_equal(&arm1.statements, &arm2.statements)
                })
                && opts_equal(o1, o2, |e1, e2| stmts_equal(&e1.statements, &e2.statements))
        }
        (
            StmtNode::ForList { value_binding: v1, key_binding: k1, expr: e1, body: b1, .. },
            StmtNode::ForList { value_binding: v2, key_binding: k2, expr: e2, body: b2, .. },
        ) => v1 == v2 && k1 == k2 && exprs_equal(e1, e2) && stmts_equal(b1, b2),
        (
            StmtNode::ForRange { id: id1, from: f1, to: t1, body: b1, .. },
            StmtNode::ForRange { id: id2, from: f2, to: t2, body: b2, .. },
        ) => id1 == id2 && exprs_equal(f1, f2) && exprs_equal(t1, t2) && stmts_equal(b1, b2),
        (
            StmtNode::While { id: id1, condition: c1, body: b1, .. },
            StmtNode::While { id: id2, condition: c2, body: b2, .. },
        ) => id1 == id2 && exprs_equal(c1, c2) && stmts_equal(b1, b2),
        (
            StmtNode::Fork { id: id1, time: t1, body: b1 },
            StmtNode::Fork { id: id2, time: t2, body: b2 },
        ) => id1 == id2 && exprs_equal(t1, t2) && stmts_equal(b1, b2),
        (
            StmtNode::TryExcept { body: b1, excepts: e1, environment_width: w1 },
            StmtNode::TryExcept { body: b2, excepts: e2, environment_width: w2 },
        ) => {
            w1 == w2
                && stmts_equal(b1, b2)
                && e1.len() == e2.len()
                && e1.iter().zip(e2.iter()).all(|(ex1, ex2)| {
                    ex1.id == ex2.id
                        && ex1.codes == ex2.codes
                        && stmts_equal(&ex1.statements, &ex2.statements)
                })
        }
        (
            StmtNode::TryFinally { body: b1, handler: h1, environment_width: w1 },
            StmtNode::TryFinally { body: b2, handler: h2, environment_width: w2 },
        ) => w1 == w2 && stmts_equal(b1, b2) && stmts_equal(h1, h2),
        (
            StmtNode::Scope { num_bindings: n1, body: b1 },
            StmtNode::Scope { num_bindings: n2, body: b2 },
        ) => n1 == n2 && stmts_equal(b1, b2),
        _ => false,
    }
}

fn stmts_equal(a: &[Stmt], b: &[Stmt]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b.iter())
            .all(|(s1, s2)| stmt_nodes_equal(&s1.node, &s2.node))
}

fn exprs_equal(a: &Expr, b: &Expr) -> bool {
    match (a, b) {
        (
            Expr::Lambda { params: p1, body: b1, self_name: s1 },
            Expr::Lambda { params: p2, body: b2, self_name: s2 },
        ) => p1 == p2 && s1 == s2 && stmt_nodes_equal(&b1.node, &b2.node),
        (Expr::Assign { left: l1, right: r1 }, Expr::Assign { left: l2, right: r2 }) => {
            exprs_equal(l1, l2) && exprs_equal(r1, r2)
        }
        (
            Expr::Decl { id: id1, is_const: c1, expr: e1 },
            Expr::Decl { id: id2, is_const: c2, expr: e2 },
        ) => id1 == id2 && c1 == c2 && opts_equal(e1, e2, |a, b| exprs_equal(a, b)),
        (Expr::List(items1), Expr::List(items2)) => {
            items1.len() == items2.len()
                && items1.iter().zip(items2.iter()).all(|(a1, a2)| args_equal(a1, a2))
        }
        (Expr::Map(pairs1), Expr::Map(pairs2)) => {
            pairs1.len() == pairs2.len()
                && pairs1
                    .iter()
                    .zip(pairs2.iter())
                    .all(|((k1, v1), (k2, v2))| exprs_equal(k1, k2) && exprs_equal(v1, v2))
        }
        _ => a == b,
    }
}

fn args_equal(a: &Arg, b: &Arg) -> bool {
    match (a, b) {
        (Arg::Normal(e1), Arg::Normal(e2)) | (Arg::Splice(e1), Arg::Splice(e2)) => exprs_equal(e1, e2),
        _ => false,
    }
}

/// Helper for comparing Option<T> with a custom equality function.
fn opts_equal<T, F: Fn(&T, &T) -> bool>(a: &Option<T>, b: &Option<T>, eq: F) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => eq(a, b),
        (None, None) => true,
        _ => false,
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
