use crate::compiler::parse::Name;
use crate::model::var::Var;

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
pub struct Scatter {
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
pub enum Expr {
    Assign{left: Box<Expr>, right :Box<Expr>},
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
        function: Name,
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
        codes: Vec<Arg>,
        except: Option<Box<Expr>>,
    },
    Index(Box<Expr>, Box<Expr>),
    List(Vec<Arg>),
    Scatter(Vec<Scatter>),
    Length,
    This,
}

#[derive(Debug, Eq, PartialEq)]
pub struct CondArm {
    pub condition: Expr,
    pub statements: Vec<Stmt>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ExceptArm {
    pub id: Option<Name>,
    pub codes: Vec<Arg>,
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
    Catch {
        body: Vec<Stmt>,
        excepts: Vec<ExceptArm>,
    },
    Finally {
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
    Exit(i64),
}
