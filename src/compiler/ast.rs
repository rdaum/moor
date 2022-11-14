use crate::compiler::parse::Name;
use crate::model::Var;

pub enum Arg {
    Normal(Expr),
    Splice(Expr)
}

pub enum ScatterKind {
    Required, Optional, Rest
}

pub struct Scatter {
    kind: ScatterKind,
    id: Name,
    expr: Expr,
}

pub enum Expr {
    VarExpr(Var),
    Id(i64),
    Binary(Box<Expr>, Box<Expr>),
    Call{function: i64, args: Vec<Arg>},
    Verb{obj: Box<Expr>, verb: Box<Expr>, args: Vec<Arg>},
    Range{base: Box<Expr>, from: Box<Expr>, to: Box<Expr>},
    Cond{condition: Box<Expr>, consequence: Box<Expr>, alternative: Box<Expr>},
    Catch{trye: Box<Expr>, code: Vec<Arg>, except: Box<Expr>},
    Expr(Box<Expr>),
    List(Vec<Arg>),
    Scatter(Vec<Scatter>)
}

pub struct CondArm {
    pub condition: Expr,
    pub statements: Vec<Stmt>,
}

pub struct ExceptArm {
    pub id: Option<Name>,
    pub codes: Vec<Arg>,
    pub statements: Vec<Stmt>,
}

pub enum LoopKind {
    While
}

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
