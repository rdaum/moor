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

use crate::program::labels::{JumpLabel, Label, Offset};
use crate::program::names::{Name, Names};
use crate::program::opcode::{
    ForSequenceOperand, ListComprehend, Op, RangeComprehend, ScatterArgs,
};
use crate::{AsByteBuffer, BINCODE_CONFIG, CountingWriter, DecodingError, EncodingError, Symbol};
use crate::{ErrorCode, Var};
use bincode::{Decode, Encode};
use byteview::ByteView;
use lazy_static::lazy_static;
use std::fmt::{Display, Formatter};
use std::sync::Arc;

lazy_static! {
    pub static ref EMPTY_PROGRAM: Program = Program::new();
}

/// The result of compilation. The set of instructions, fork vectors, variable offsets, literals.
#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub struct Program(pub Arc<PrgInner>);

#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub struct PrgInner {
    /// All the literals referenced in this program.
    pub literals: Vec<Var>,
    /// All the jump offsets used in this program.
    pub jump_labels: Vec<JumpLabel>,
    /// All the variable names used in this program.
    pub var_names: Names,
    /// Scatter assignment tables, referenced by the scatter opcode.
    pub scatter_tables: Vec<ScatterArgs>,
    /// Table of the operands for the ForSequence opcode.
    pub for_sequence_operands: Vec<ForSequenceOperand>,
    /// Range comprehensions, referenced by the range comprehension opcode.
    pub range_comprehensions: Vec<RangeComprehend>,
    /// List comprehensions, referenced by the list comprehension opcode.
    pub list_comprehensions: Vec<ListComprehend>,
    /// All the error operands referenced in by MakeError in the program.
    pub error_operands: Vec<ErrorCode>,
    /// Lambda programs, referenced by the MakeLambda opcode.
    pub lambda_programs: Vec<Program>,
    /// The actual program code.
    pub main_vector: Vec<Op>,
    /// The program code for each fork as (offset_in_main_vector, fork_opcodes).
    pub fork_vectors: Vec<(usize, Vec<Op>)>,
    /// As each statement is pushed, the line number is recorded, along with its offset in the main
    /// vector.
    pub line_number_spans: Vec<(usize, usize)>,
    /// Line number spans for each fork vector.
    pub fork_line_number_spans: Vec<Vec<(usize, usize)>>,
}
impl Program {
    pub fn new() -> Self {
        Program(Arc::new(PrgInner {
            literals: Vec::new(),
            jump_labels: Vec::new(),
            var_names: Names::new(0),
            scatter_tables: vec![],
            for_sequence_operands: vec![],
            range_comprehensions: vec![],
            list_comprehensions: vec![],
            error_operands: vec![],
            lambda_programs: vec![],
            main_vector: vec![],
            fork_vectors: vec![],
            line_number_spans: vec![],
            fork_line_number_spans: vec![],
        }))
    }

    pub fn find_var(&self, v: &str) -> Name {
        let v = Symbol::mk(v);
        self.0
            .var_names
            .name_for_ident(v)
            .unwrap_or_else(|| panic!("variable not found: {v}"))
    }

    pub fn find_label_for_literal(&self, l: Var) -> Label {
        Label(
            self.0
                .literals
                .iter()
                .position(|x| *x == l)
                .expect("literal not found") as u16,
        )
    }

    pub fn var_names(&self) -> &Names {
        &self.0.var_names
    }

    pub fn line_number_spans(&self) -> &[(usize, usize)] {
        &self.0.line_number_spans
    }

    pub fn literals(&self) -> &[Var] {
        &self.0.literals
    }

    pub fn jump_labels(&self) -> &[JumpLabel] {
        &self.0.jump_labels
    }

    pub fn find_literal(&self, label: &Label) -> Option<Var> {
        self.0.literals.get(label.0 as usize).cloned()
    }

    pub fn error_operand(&self, offset: Offset) -> &ErrorCode {
        &self.0.error_operands[offset.0 as usize]
    }

    pub fn scatter_table(&self, offset: Offset) -> &ScatterArgs {
        &self.0.scatter_tables[offset.0 as usize]
    }

    pub fn for_sequence_operand(&self, offset: Offset) -> &ForSequenceOperand {
        &self.0.for_sequence_operands[offset.0 as usize]
    }

    pub fn range_comprehension(&self, offset: Offset) -> &RangeComprehend {
        &self.0.range_comprehensions[offset.0 as usize]
    }

    pub fn list_comprehension(&self, offset: Offset) -> &ListComprehend {
        &self.0.list_comprehensions[offset.0 as usize]
    }

    pub fn lambda_program(&self, offset: Offset) -> &Program {
        &self.0.lambda_programs[offset.0 as usize]
    }

    pub fn jump_label(&self, offset: Label) -> &JumpLabel {
        &self.0.jump_labels[offset.0 as usize]
    }

    pub fn fork_vector(&self, offset: Offset) -> &Vec<Op> {
        &self.0.fork_vectors[offset.0 as usize].1
    }

    pub fn fork_vector_offset(&self, offset: Offset) -> usize {
        self.0.fork_vectors[offset.0 as usize].0
    }

    pub fn main_vector(&self) -> &Vec<Op> {
        &self.0.main_vector
    }

    pub fn find_jump(&self, label: &Label) -> Option<JumpLabel> {
        self.0.jump_labels.iter().find(|j| &j.id == label).cloned()
    }

    pub fn line_num_for_position(&self, position: usize, offset: usize) -> usize {
        let position = position + offset;
        let mut last_line_num = 1;
        for (off, line_no) in &self.0.line_number_spans {
            if *off >= position {
                return last_line_num;
            }
            last_line_num = *line_no
        }
        last_line_num
    }

    pub fn fork_line_num_for_position(&self, fork_offset: Offset, position: usize) -> usize {
        let fork_spans = &self.0.fork_line_number_spans[fork_offset.0 as usize];
        let mut last_line_num = 1;
        for (off, line_no) in fork_spans {
            if *off >= position {
                return last_line_num;
            }
            last_line_num = *line_no
        }
        last_line_num
    }

}

impl Default for Program {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for Program {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // Write literals indexed by their offset #
        for (i, l) in self.0.literals.iter().enumerate() {
            writeln!(f, "L{i}: {l:?}")?;
        }

        // Write jump labels indexed by their offset & showing position & optional name
        for (i, l) in self.0.jump_labels.iter().enumerate() {
            write!(f, "J{}: {}", i, l.position.0)?;
            if let Some(name) = &l.name {
                write!(f, " ({})", self.0.var_names.ident_for_name(name).unwrap())?;
            }
            writeln!(f)?;
        }

        // Write variable names indexed by their offset
        for (i, v) in self.0.var_names.symbols().iter().enumerate() {
            writeln!(f, "V{i}: {v}")?;
        }

        // TODO: print fork vectors

        // Display main vector (program); opcodes are indexed by their offset
        for (i, op) in self.0.main_vector.iter().enumerate() {
            writeln!(f, "{i}: {op:?}")?;
        }

        Ok(())
    }
}

// Byte buffer representation is just bincoded for now.
impl AsByteBuffer for Program {
    fn size_bytes(&self) -> usize
    where
        Self: Encode,
    {
        // For now be careful with this as we have to bincode the whole thing in order to calculate
        // this. In the long run with a zero-copy implementation we can just return the size of the
        // underlying bytes.
        let mut cw = CountingWriter { count: 0 };
        bincode::encode_into_writer(self, &mut cw, *BINCODE_CONFIG)
            .expect("bincode to bytes for counting size");
        cw.count
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> Result<R, EncodingError>
    where
        Self: Sized + Encode,
    {
        let v = bincode::encode_to_vec(self, *BINCODE_CONFIG)
            .map_err(|e| EncodingError::CouldNotEncode(e.to_string()))?;
        Ok(f(&v[..]))
    }

    fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError>
    where
        Self: Sized + Encode,
    {
        bincode::encode_to_vec(self, *BINCODE_CONFIG)
            .map_err(|e| EncodingError::CouldNotEncode(e.to_string()))
    }

    fn from_bytes(bytes: ByteView) -> Result<Self, DecodingError>
    where
        Self: Sized + Decode<()>,
    {
        Ok(bincode::decode_from_slice(bytes.as_ref(), *BINCODE_CONFIG)
            .map_err(|e| DecodingError::CouldNotDecode(e.to_string()))?
            .0)
    }

    fn as_bytes(&self) -> Result<ByteView, EncodingError> {
        Ok(ByteView::from(self.make_copy_as_vec()?))
    }
}
