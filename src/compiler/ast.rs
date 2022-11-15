use crate::compiler::parse::Name;
use crate::model::{Var};

#[derive(Debug)]
pub enum Arg {
    Normal(Expr),
    Splice(Expr),
}

#[derive(Debug)]
pub enum ScatterKind {
    Required, Optional, Rest
}

#[derive(Debug)]
pub struct Scatter {
    pub kind: ScatterKind,
    pub id: Name,
    pub expr: Option<Expr>,
}

#[derive(Debug)]
pub enum BinaryOp {
    Add, Sub, Mul, Div, Mod, Eq, NEq, Gt, GtE, Lt, LtE, And, Or, Xor, In, Arrow,
    Index, IndexRange
}

#[derive(Debug)]
pub enum UnaryOp {
    Neg, Not
}

#[derive(Debug)]
pub enum Expr {
    VarExpr(Var),
    Id(usize),
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
    Unary(UnaryOp, Box<Expr>),
    Prop{location: Box<Expr>, property: Box<Expr>},
    Call{function: i64, args: Vec<Arg>},
    Verb{location: Box<Expr>, verb: Box<Expr>, args: Vec<Arg>},
    Range{base: Box<Expr>, from: Box<Expr>, to: Box<Expr>},
    Cond{condition: Box<Expr>, consequence: Box<Expr>, alternative: Box<Expr>},
    Catch{trye: Box<Expr>, codes: Vec<Arg>, except: Option<Box<Expr>>},
    Expr(Box<Expr>),
    List(Vec<Arg>),
    Scatter(Vec<Scatter>),
    Length,
}

#[derive(Debug)]
pub struct CondArm {
    pub condition: Expr,
    pub statements: Vec<Stmt>,
}

#[derive(Debug)]
pub struct ExceptArm {
    pub id: Option<Name>,
    pub codes: Vec<Arg>,
    pub statements: Vec<Stmt>,
}

#[derive(Debug)]
pub enum LoopKind {
    While
}

#[derive(Debug)]
pub enum Stmt {
    Cond{arms: Vec<CondArm>, otherwise:Vec<Stmt>},
    List{expr: Expr, body: Vec<Stmt>},
    Range{id: Name, from: Expr, to: Expr, body: Vec<Stmt>},
    Loop{kind: LoopKind, id :Option<Name>, condition: Expr, body: Vec<Stmt>},
    Fork{id: Option<Name>, time: Expr, body: Vec<Stmt>},
    Catch{body: Vec<Stmt>, excepts: Vec<ExceptArm>},
    Finally{body: Vec<Stmt>, handler: Vec<Stmt>},
    Break{exit: Option<Name>},
    Continue{exit: Option<Name>},
    Return{expr: Option<Expr>},
    Expr(Expr),
    Exit(i64)
}
