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

//! Compile entry points for the bytecode backend.

pub use crate::backend::state::CodegenState;

use moor_common::model::CompileError;
use moor_var::program::{opcode::Op, program::Program};

use crate::{compile_options::CompileOptions, frontend::lower::parse_program_frontend, parse_tree::Parse};

fn do_compile(parse: Parse, compile_options: CompileOptions) -> Result<Program, CompileError> {
    let mut cg_state = CodegenState::new(compile_options, parse.names);
    for x in parse.stmts {
        cg_state.generate_stmt(&x)?;
    }
    cg_state.emit(Op::Done);

    if cg_state.stack.depth() != 0 || cg_state.stack.saved_top().is_some() {
        panic!(
            "Stack is not empty at end of compilation: cur_stack#: {} stack: {:?}",
            cg_state.stack.depth(),
            cg_state.stack.saved_top()
        )
    }

    Ok(cg_state.operands.take_program_parts().build_program(
        cg_state.var_names,
        cg_state.emitter.take_jumps(),
        cg_state.emitter.take_ops(),
        cg_state.line_number_spans,
    ))
}

/// Compile from a program string using the handwritten frontend parser and lowering path.
pub fn compile(program: &str, options: CompileOptions) -> Result<Program, CompileError> {
    let parse = parse_program_frontend(program, options.clone())?;

    do_compile(parse, options)
}
