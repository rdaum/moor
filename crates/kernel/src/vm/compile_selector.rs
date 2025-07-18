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

use moor_common::model::CompileError;
use moor_compiler::{Program, CompileOptions, get_parser_by_name};
use crate::config::Config;
use std::sync::Arc;

/// Compile MOO code using the parser specified in the config
pub fn compile_with_config(
    code: &str,
    compile_options: CompileOptions,
    config: &Arc<Config>,
) -> Result<Program, CompileError> {
    // Get the parser from config, defaulting to "cst" if not specified
    let parser_name = config.parser.as_deref().unwrap_or("cst");
    
    // Get the parser implementation
    let parser = get_parser_by_name(parser_name)
        .ok_or_else(|| CompileError::ParseError {
            error_position: moor_common::model::CompileContext::new((1, 1)),
            context: "parser selection".to_string(),
            end_line_col: None,
            message: format!("Unknown parser: {parser_name}"),
        })?;
    
    // Use the selected parser to compile
    parser.compile(code, compile_options)
}