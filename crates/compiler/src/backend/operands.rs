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

use triomphe::Arc;

use moor_var::{
    ErrorCode, Var,
    program::{
        labels::{Label, Offset},
        opcode::{
            ForRangeOperand, ForSequenceOperand, ListComprehend, Op, RangeComprehend, ScatterArgs,
            ScatterLabel,
        },
        program::{PrgInner, Program},
    },
};

#[derive(Debug, Default)]
pub struct OperandState {
    literals: Vec<Var>,
    scatter_tables: Vec<ScatterArgs>,
    for_sequence_operands: Vec<ForSequenceOperand>,
    for_range_operands: Vec<ForRangeOperand>,
    range_comprehensions: Vec<RangeComprehend>,
    list_comprehensions: Vec<ListComprehend>,
    error_operands: Vec<ErrorCode>,
    lambda_programs: Vec<Program>,
    fork_vectors: Vec<(usize, Vec<Op>)>,
    fork_line_number_spans: Vec<Vec<(usize, usize)>>,
}

#[derive(Debug, Default)]
pub struct OperandSnapshot {
    literals: Vec<Var>,
    scatter_tables: Vec<ScatterArgs>,
    for_sequence_operands: Vec<ForSequenceOperand>,
    for_range_operands: Vec<ForRangeOperand>,
    range_comprehensions: Vec<RangeComprehend>,
    list_comprehensions: Vec<ListComprehend>,
    error_operands: Vec<ErrorCode>,
    lambda_programs: Vec<Program>,
    fork_vectors: Vec<(usize, Vec<Op>)>,
    fork_line_number_spans: Vec<Vec<(usize, usize)>>,
}

impl OperandState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_literal(&mut self, v: &Var) -> Label {
        let lv_pos = self.literals.iter().position(|lv| lv.eq_case_sensitive(v));
        let pos = lv_pos.unwrap_or_else(|| {
            let idx = self.literals.len();
            self.literals.push(v.clone());
            idx
        });
        Label(pos as u16)
    }

    pub fn add_error_code_operand(&mut self, code: ErrorCode) -> Offset {
        let err_pos = self.error_operands.len();
        self.error_operands.push(code);
        Offset(err_pos as u16)
    }

    pub fn add_scatter_table(&mut self, labels: Vec<ScatterLabel>, done: Label) -> Offset {
        let st_pos = self.scatter_tables.len();
        self.scatter_tables.push(ScatterArgs { labels, done });
        Offset(st_pos as u16)
    }

    pub fn add_lambda_program(&mut self, mut program: Program, base_line_offset: usize) -> Offset {
        let adjusted_spans: Vec<(usize, usize)> = program
            .line_number_spans()
            .iter()
            .map(|(offset, line_num)| (*offset, line_num + base_line_offset))
            .collect();
        Arc::make_mut(&mut program.0).line_number_spans = adjusted_spans;

        let lp_pos = self.lambda_programs.len();
        self.lambda_programs.push(program);
        Offset(lp_pos as u16)
    }

    pub fn add_range_comprehension(&mut self, range_comprehension: RangeComprehend) -> Offset {
        let rc_pos = self.range_comprehensions.len();
        self.range_comprehensions.push(range_comprehension);
        Offset(rc_pos as u16)
    }

    pub fn add_list_comprehension(&mut self, list_comprehension: ListComprehend) -> Offset {
        let lc_pos = self.list_comprehensions.len();
        self.list_comprehensions.push(list_comprehension);
        Offset(lc_pos as u16)
    }

    pub fn add_for_sequence_operand(&mut self, operand: ForSequenceOperand) -> Offset {
        let fs_pos = self.for_sequence_operands.len();
        self.for_sequence_operands.push(operand);
        Offset(fs_pos as u16)
    }

    pub fn add_for_range_operand(&mut self, operand: ForRangeOperand) -> Offset {
        let fr_pos = self.for_range_operands.len();
        self.for_range_operands.push(operand);
        Offset(fr_pos as u16)
    }

    pub fn add_fork_vector(
        &mut self,
        offset: usize,
        opcodes: Vec<Op>,
        line_spans: Vec<(usize, usize)>,
    ) -> Offset {
        let fv = self.fork_vectors.len();
        self.fork_vectors.push((offset, opcodes));
        self.fork_line_number_spans.push(line_spans);
        Offset(fv as u16)
    }

    pub fn snapshot_and_reset(&mut self) -> OperandSnapshot {
        let snapshot = OperandSnapshot {
            literals: std::mem::take(&mut self.literals),
            scatter_tables: std::mem::take(&mut self.scatter_tables),
            for_sequence_operands: std::mem::take(&mut self.for_sequence_operands),
            for_range_operands: std::mem::take(&mut self.for_range_operands),
            range_comprehensions: std::mem::take(&mut self.range_comprehensions),
            list_comprehensions: std::mem::take(&mut self.list_comprehensions),
            error_operands: std::mem::take(&mut self.error_operands),
            lambda_programs: std::mem::take(&mut self.lambda_programs),
            fork_vectors: std::mem::take(&mut self.fork_vectors),
            fork_line_number_spans: std::mem::take(&mut self.fork_line_number_spans),
        };
        self.reset();
        snapshot
    }

    pub fn restore(&mut self, snapshot: OperandSnapshot) {
        self.literals = snapshot.literals;
        self.scatter_tables = snapshot.scatter_tables;
        self.for_sequence_operands = snapshot.for_sequence_operands;
        self.for_range_operands = snapshot.for_range_operands;
        self.range_comprehensions = snapshot.range_comprehensions;
        self.list_comprehensions = snapshot.list_comprehensions;
        self.error_operands = snapshot.error_operands;
        self.lambda_programs = snapshot.lambda_programs;
        self.fork_vectors = snapshot.fork_vectors;
        self.fork_line_number_spans = snapshot.fork_line_number_spans;
    }

    pub fn reset(&mut self) {
        self.literals.clear();
        self.scatter_tables.clear();
        self.for_sequence_operands.clear();
        self.for_range_operands.clear();
        self.range_comprehensions.clear();
        self.list_comprehensions.clear();
        self.error_operands.clear();
        self.lambda_programs.clear();
        self.fork_vectors.clear();
        self.fork_line_number_spans.clear();
    }

    pub fn take_program_parts(&mut self) -> ProgramOperandParts {
        ProgramOperandParts {
            literals: std::mem::take(&mut self.literals),
            scatter_tables: std::mem::take(&mut self.scatter_tables),
            for_sequence_operands: std::mem::take(&mut self.for_sequence_operands),
            for_range_operands: std::mem::take(&mut self.for_range_operands),
            range_comprehensions: std::mem::take(&mut self.range_comprehensions),
            list_comprehensions: std::mem::take(&mut self.list_comprehensions),
            error_operands: std::mem::take(&mut self.error_operands),
            lambda_programs: std::mem::take(&mut self.lambda_programs),
            fork_vectors: std::mem::take(&mut self.fork_vectors),
            fork_line_number_spans: std::mem::take(&mut self.fork_line_number_spans),
        }
    }
}

pub struct ProgramOperandParts {
    pub literals: Vec<Var>,
    pub scatter_tables: Vec<ScatterArgs>,
    pub for_sequence_operands: Vec<ForSequenceOperand>,
    pub for_range_operands: Vec<ForRangeOperand>,
    pub range_comprehensions: Vec<RangeComprehend>,
    pub list_comprehensions: Vec<ListComprehend>,
    pub error_operands: Vec<ErrorCode>,
    pub lambda_programs: Vec<Program>,
    pub fork_vectors: Vec<(usize, Vec<Op>)>,
    pub fork_line_number_spans: Vec<Vec<(usize, usize)>>,
}

impl ProgramOperandParts {
    pub fn build_program(
        self,
        var_names: moor_var::program::names::Names,
        jump_labels: Vec<moor_var::program::labels::JumpLabel>,
        main_vector: Vec<Op>,
        line_number_spans: Vec<(usize, usize)>,
    ) -> Program {
        Program(Arc::new(PrgInner {
            literals: self.literals,
            jump_labels,
            var_names,
            scatter_tables: self.scatter_tables,
            for_sequence_operands: self.for_sequence_operands,
            for_range_operands: self.for_range_operands,
            range_comprehensions: self.range_comprehensions,
            list_comprehensions: self.list_comprehensions,
            error_operands: self.error_operands,
            lambda_programs: self.lambda_programs,
            main_vector,
            fork_vectors: self.fork_vectors,
            line_number_spans,
            fork_line_number_spans: self.fork_line_number_spans,
        }))
    }
}
