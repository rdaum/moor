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

//! AST builder implementations

pub mod generic_ast_builder;
pub mod ast_builder;

// Re-export builder types
#[cfg(feature = "tree-sitter-parser")]
pub use generic_ast_builder::GenericASTBuilder;
#[cfg(feature = "tree-sitter-parser")]
pub use ast_builder::ASTBuilder;
