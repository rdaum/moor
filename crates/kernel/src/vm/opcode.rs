use std::fmt::{Display, Formatter};

use bincode::{Decode, Encode};
use lazy_static::lazy_static;

use moor_values::var::Var;

use crate::compiler::labels::{JumpLabel, Label, Name, Names, Offset};

#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Encode, Decode)]
pub enum ScatterLabel {
    Required(Name),
    Rest(Name),
    Optional(Name, Option<Label>),
}

#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Encode, Decode)]
pub enum Op {
    If(Label),
    Eif(Label),
    IfQues(Label),
    While(Label),
    Jump {
        label: Label,
    },
    ForList {
        id: Name,
        end_label: Label,
    },
    ForRange {
        id: Name,
        end_label: Label,
    },
    Pop,
    Val(Var),
    Imm(Label),
    MkEmptyList,
    ListAddTail,
    ListAppend,
    IndexSet,
    MakeSingletonList,
    CheckListForSplice,
    PutTemp,
    PushTemp,
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
    In,
    Mul,
    Sub,
    Div,
    Mod,
    Add,
    And(Label),
    Or(Label),
    Not,
    UnaryMinus,
    Ref,
    Push(Name),
    PushRef,
    Put(Name),
    RangeRef,
    GPut {
        id: Name,
    },
    GPush {
        id: Name,
    },
    GetProp,
    PushGetProp,
    PutProp,
    Fork {
        fv_offset: Offset,
        id: Option<Name>,
    },
    CallVerb,
    Return,
    Return0,
    Done,
    FuncCall {
        id: Name,
    },
    Pass,
    RangeSet,
    Length(Offset),
    Exp,
    Scatter {
        nargs: usize,
        nreq: usize,
        rest: usize,
        labels: Vec<ScatterLabel>,
        done: Label,
    },
    PushLabel(Label),
    TryFinally(Label),
    Catch(Label),
    TryExcept {
        num_excepts: usize,
    },
    EndCatch(Label),
    EndExcept(Label),
    EndFinally,
    WhileId {
        id: Name,
        end_label: Label,
    },
    Continue,
    ExitId(Label),
    Exit {
        stack: Offset,
        label: Label,
    },
}

lazy_static! {
    pub static ref EMPTY_PROGRAM: Program = Program::new();
}

/// The result of compilation. The set of instructions, fork vectors, variable offsets, literals.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct Program {
    /// All the literals referenced in this program.
    pub(crate) literals: Vec<Var>,
    /// All the jump offsets used in this program.
    pub(crate) jump_labels: Vec<JumpLabel>,
    /// All the variable names used in this program.
    pub(crate) var_names: Names,
    /// The actual program code.
    pub(crate) main_vector: Vec<Op>,
    /// The program code for each fork.
    pub(crate) fork_vectors: Vec<Vec<Op>>,
    /// As each statement is pushed, the line number is recorded, along with its offset in the main
    /// vector.
    /// TODO: fork vector offsets... Have to think about that one.
    pub(crate) line_number_spans: Vec<(usize, usize)>,
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
