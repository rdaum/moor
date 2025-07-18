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

//! Testing utilities and test modules

pub mod codegen_tests;
pub mod operand_parsing_tests;
pub mod test_macros;
pub mod test_utils;

#[cfg(feature = "tree-sitter-parser")]
pub mod parse_treesitter_tests;

#[cfg(test)]
pub mod tree_traits_integration_tests;

// Re-export testing utilities
// pub use test_utils::{setup_test_logging, assert_successful_parse};
