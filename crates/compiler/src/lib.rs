// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

#[macro_use]
extern crate pest_derive;
pub use moor_var::program::names::Names;

mod ast;
mod codegen;
mod compile_options;
mod decompile;
mod diagnostics;
pub mod frontend;
mod lexer;
mod parse_tree;
mod precedence;
mod pest_grammar;
mod syntax_kind;
mod unparse;

mod codegen_tests;
mod objdef;
mod var_scope;

#[cfg(test)]
mod tests;

pub use crate::diagnostics::{
    DiagnosticRenderOptions, DiagnosticVerbosity, compile_error_to_map, emit_compile_error,
    format_compile_error,
};
pub use crate::{
    codegen::compile,
    compile_options::CompileOptions,
    decompile::program_to_tree,
    frontend::cst::{
        AssignExpr, BeginStmt, BinExpr, BreakStmt, CallExpr, ComprehensionExpr, CondExpr,
        ConstStmt, ContinueStmt, ElseClause, ElseIfClause, ExceptClause, ExprStmt, Expression,
        FlyweightExpr, FnStmt, ForInStmt, ForRangeStmt, ForkStmt, GlobalStmt, IfStmt, IndexExpr,
        LambdaExpr, LetStmt, ListExpr, MapExpr, ParamList, ParenExpr, PassExpr,
        Program as CstProgram, PropExpr, RangeExpr, ReturnStmt, ScatterExpr, ScatterItem,
        Statement, StmtList, SysPropExpr, TryExceptStmt, TryExpr, TryFinallyStmt, UnaryExpr,
        VerbCallExpr, WhileStmt,
    },
    frontend::cursor::{ParseError as FrontendParseError, TokenCursor},
    frontend::lower::parse_program_frontend,
    frontend::parser::{parse_to_cst, parse_to_syntax_node},
    lexer::{Token, lex},
    objdef::{
        ObjDefParseError, ObjFileContext, ObjPropDef, ObjPropOverride, ObjVerbDef,
        ObjectDefinition, compile_object_definitions, parse_literal_value,
    },
    precedence::{PrecedenceLevel, expr_precedence_level},
    syntax_kind::SyntaxKind,
    unparse::{to_literal, to_literal_objsub, unparse},
};
// Re-export from var
pub use moor_common::builtins::{
    ArgCount, ArgType, BUILTINS, Builtin, BuiltinId, offset_for_builtin,
};
pub use moor_var::program::{
    labels::{JumpLabel, Label, Offset},
    opcode::{Op, ScatterLabel},
    program::{EMPTY_PROGRAM, Program},
    stored_program::StoredProgram,
};
pub use var_scope::VarScope;
