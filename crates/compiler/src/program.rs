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

use crate::labels::{JumpLabel, Label};
use crate::names::{Name, Names};
use crate::opcode::{ForSequenceOperand, ListComprehend, Op, RangeComprehend, ScatterArgs};
use crate::unparse::to_literal;
use bincode::{Decode, Encode};
use byteview::ByteView;
use lazy_static::lazy_static;
use moor_var::{AsByteBuffer, BINCODE_CONFIG, CountingWriter, DecodingError, EncodingError};
use moor_var::{ErrorCode, Var};
use std::fmt::{Display, Formatter};
use std::sync::Arc;

lazy_static! {
    pub static ref EMPTY_PROGRAM: Program = Program::new();
}

/// The result of compilation. The set of instructions, fork vectors, variable offsets, literals.
#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub struct Program {
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
    /// The actual program code.
    pub main_vector: Arc<Vec<Op>>,
    /// The program code for each fork.
    pub fork_vectors: Vec<Vec<Op>>,
    /// As each statement is pushed, the line number is recorded, along with its offset in the main
    /// vector.
    /// TODO: fork vector offsets... Have to think about that one.
    pub line_number_spans: Vec<(usize, usize)>,
}

impl Program {
    pub fn new() -> Self {
        Program {
            literals: Vec::new(),
            jump_labels: Vec::new(),
            var_names: Names::new(0),
            scatter_tables: vec![],
            for_sequence_operands: vec![],
            range_comprehensions: vec![],
            list_comprehensions: vec![],
            main_vector: Arc::new(Vec::new()),
            fork_vectors: Vec::new(),
            line_number_spans: Vec::new(),
            error_operands: vec![],
        }
    }

    pub fn find_var(&self, v: &str) -> Name {
        self.var_names
            .find_name(v)
            .unwrap_or_else(|| panic!("variable not found: {}", v))
    }

    pub fn find_literal(&self, l: Var) -> Label {
        Label(
            self.literals
                .iter()
                .position(|x| *x == l)
                .expect("literal not found") as u16,
        )
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
        for (i, l) in self.literals.iter().enumerate() {
            writeln!(f, "L{}: {}", i, to_literal(l))?;
        }

        // Write jump labels indexed by their offset & showing position & optional name
        for (i, l) in self.jump_labels.iter().enumerate() {
            write!(f, "J{}: {}", i, l.position.0)?;
            if let Some(name) = &l.name {
                write!(f, " ({})", self.var_names.name_of(name).unwrap())?;
            }
            writeln!(f)?;
        }

        // Write variable names indexed by their offset
        for (i, v) in self.var_names.symbols().iter().enumerate() {
            writeln!(f, "V{}: {}", i, v)?;
        }

        // TODO: print fork vectors

        // Display main vector (program); opcodes are indexed by their offset
        for (i, op) in self.main_vector.iter().enumerate() {
            writeln!(f, "{}: {:?}", i, op)?;
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
