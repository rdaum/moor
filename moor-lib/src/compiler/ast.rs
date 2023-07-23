/// The abstract syntax tree produced by the parser and converted by codgen into opcodes.
use crate::compiler::labels::Name;
use crate::var::Var;

#[derive(Debug, Eq, PartialEq)]
pub enum Arg {
    Normal(Expr),
    Splice(Expr),
}

#[derive(Debug, Eq, PartialEq)]
pub enum ScatterKind {
    Required,
    Optional,
    Rest,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ScatterItem {
    pub kind: ScatterKind,
    pub id: Name,
    pub expr: Option<Expr>,
}

#[derive(Debug, Eq, PartialEq)]
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

#[derive(Debug, Eq, PartialEq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Eq, PartialEq)]
pub enum CatchCodes {
    Codes(Vec<Arg>),
    Any,
}

#[derive(Debug, Eq, PartialEq)]
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

#[derive(Debug, Eq, PartialEq)]
pub struct CondArm {
    pub condition: Expr,
    pub statements: Vec<Stmt>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ExceptArm {
    pub id: Option<Name>,
    pub codes: CatchCodes,
    pub statements: Vec<Stmt>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Stmt {
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
