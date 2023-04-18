use crate::compiler::ast::Stmt;
use crate::compiler::labels::Names;

pub mod ast;
pub mod builtins;
pub mod codegen;
pub mod labels;
pub mod parse;

pub struct Parse {
    pub stmts: Vec<Stmt>,
    pub names: Names,
}
