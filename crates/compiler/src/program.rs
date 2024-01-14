use crate::labels::{JumpLabel, Label, Name, Names};
use crate::opcode::Op;
use bincode::{Decode, Encode};
use lazy_static::lazy_static;
use moor_values::var::Var;
use std::fmt::{Display, Formatter};

lazy_static! {
    pub static ref EMPTY_PROGRAM: Program = Program::new();
}

/// The result of compilation. The set of instructions, fork vectors, variable offsets, literals.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct Program {
    /// All the literals referenced in this program.
    pub literals: Vec<Var>,
    /// All the jump offsets used in this program.
    pub jump_labels: Vec<JumpLabel>,
    /// All the variable names used in this program.
    pub var_names: Names,
    /// The actual program code.
    pub main_vector: Vec<Op>,
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
            var_names: Default::default(),
            main_vector: Vec::new(),
            fork_vectors: Vec::new(),
            line_number_spans: Vec::new(),
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
                .expect("literal not found") as u32,
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
            writeln!(f, "L{}: {}", i, l.to_literal())?;
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
        for (i, v) in self.var_names.names.iter().enumerate() {
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
