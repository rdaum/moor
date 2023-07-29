use crate::compiler::ast::Stmt;
use crate::compiler::labels::Names;

pub mod ast;
pub mod builtins;
pub mod codegen;
pub mod decompile;
pub mod labels;
pub mod parse;

/// The emitted parse tree from the parse phase of the compiler.
pub struct Parse {
    pub stmts: Vec<Stmt>,
    pub names: Names,
}
