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

//! Tree-sitter parser implementation and related utilities

pub mod parse_treesitter;
pub mod parse_treesitter_semantic;
pub mod tree_traits;
pub mod tree_walker;

// Re-export main parser functionality
pub use parse_treesitter::parse_program_with_tree_sitter;
pub use tree_walker::SemanticTreeWalker;
