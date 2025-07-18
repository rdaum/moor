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

//! Error handling and recovery modules

pub mod enhanced_errors;
pub mod cst_compare;
pub mod tree_error_recovery;

// Re-export error types and utilities
pub use enhanced_errors::{ParseContext, ErrorPosition, ErrorSpan};
pub use cst_compare::{CSTComparator, format_cst_differences};
pub use tree_error_recovery::{TreeErrorInfo, TreeErrorType, ErrorFix};