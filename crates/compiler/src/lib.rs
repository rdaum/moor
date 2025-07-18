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

#[macro_use]
extern crate pest_derive;
pub use moor_var::program::names::Names;

// Core modules (keep at root level)
mod ast;
mod codegen;
mod cst;
mod decompile;
mod objdef;
mod precedence;
mod unparse;
mod var_scope;

#[cfg(test)]
mod tests;

// Organized modules
mod builders;
mod errors;
mod parsers;
pub mod testing;

pub use crate::codegen::{compile, compile_legacy};
#[cfg(feature = "tree-sitter-parser")]
pub use crate::codegen::{compile_with_tree_sitter, compile_with_tree_sitter_moot_compatible};
pub use crate::cst::{
    Associativity, CSTExpressionParser, CSTExpressionParserBuilder, CSTNode, CSTNodeKind, CSTSpan,
    CommentType, OperatorInfo, PestToCSTConverter,
};
pub use crate::decompile::program_to_tree;
pub use crate::errors::cst_compare::{
    CSTComparator, CSTDifference, cst_to_tree_string, format_cst_differences,
};
pub use crate::errors::enhanced_errors::{
    DefaultErrorReporter, EnhancedError, EnhancedErrorReporter, ErrorPosition, ErrorSpan,
    create_enhanced_error_message, infer_parse_context,
};
pub use crate::objdef::{
    ObjDefParseError, ObjFileContext, ObjPropDef, ObjPropOverride, ObjVerbDef, ObjectDefinition,
    compile_object_definitions,
};
// Core parsing interface - simpler paths for common usage
pub use crate::parsers::parse::{CompileOptions, Parse, parse_program, unquote_str};
pub use crate::parsers::parse_cst::{CSTTreeTransformer, ParseCst, parse_program_cst};
pub use crate::parsers::parser_trait::{
    CstParser, MooParser, OriginalPestParser, get_available_parsers, get_parser_by_name,
};
#[cfg(feature = "tree-sitter-parser")]
pub use crate::parsers::parser_trait::{TreeSitterMootParser, TreeSitterParser};
#[cfg(feature = "tree-sitter-parser")]
pub use crate::parsers::tree_sitter::parse_treesitter::{
    TreeSitterConverter, parse_program_with_tree_sitter, parse_with_tree_sitter,
};
#[cfg(feature = "tree-sitter-parser")]
pub use crate::parsers::tree_sitter::parse_treesitter_semantic::{
    debug_semantic_analysis, parse_with_semantic_walker,
};
pub use crate::unparse::{to_literal, to_literal_objsub, unparse};
pub use moor_common::builtins::{
    ArgCount, ArgType, BUILTINS, Builtin, BuiltinId, offset_for_builtin,
};
pub use moor_var::program::labels::{JumpLabel, Label, Offset};
pub use moor_var::program::opcode::{Op, ScatterLabel};
pub use moor_var::program::program::{EMPTY_PROGRAM, Program};
pub use var_scope::VarScope;
