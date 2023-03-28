use serde_derive::{Deserialize, Serialize};

use crate::compiler::codegen::JumpLabel;
use crate::compiler::parse::Names;
use crate::model::var::Var;

#[derive(Clone, Deserialize, Serialize, Debug, PartialEq, Eq)]
pub enum ScatterLabel {
    Required(usize),
    Rest(usize),
    Optional(usize, Option<usize>),
}

#[derive(Clone, Deserialize, Serialize, Debug, PartialEq, Eq)]
pub enum Op {
    If(usize),
    Eif(usize),
    IfQues(usize),
    While(usize),
    Jump {
        label: usize,
    },
    ForList {
        id: usize,
        label: usize,
    },
    ForRange {
        id: usize,
        label: usize,
    },
    Pop,
    Val(Var),
    Imm(usize),
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
    And(usize),
    Or(usize),
    Not,
    UnaryMinus,
    Ref,
    Push(usize),
    PushRef,
    Put(usize),
    RangeRef,
    GPut {
        id: usize,
    },
    GPush {
        id: usize,
    },
    GetProp,
    PushGetProp,
    PutProp,
    Fork {
        f_index: usize,
        id: Option<usize>,
    },
    CallVerb,
    Return,
    Return0,
    Done,
    FuncCall {
        id: usize,
    },
    RangeSet,
    Length(usize),
    Exp,
    Scatter {
        nargs: usize,
        nreq: usize,
        nrest: usize,
        labels: Vec<ScatterLabel>,
        done: usize,
    },
    PushLabel(usize),
    TryFinally(usize),
    Catch,
    TryExcept(usize),
    EndCatch(usize),
    EndExcept(usize),
    EndFinally,
    WhileId {
        id: usize,
        label: usize,
    },
    Continue,
    ExitId(usize),
    Exit{stack: usize, label: usize}
}

/// The result of compilation. The set of instructions, fork vectors, variable offsets, literals.
#[derive(Clone, Deserialize, Serialize, Debug, PartialEq)]
pub struct Binary {
    pub(crate) literals: Vec<Var>,
    pub(crate) jump_labels: Vec<JumpLabel>,
    pub(crate) var_names: Names,
    pub(crate) main_vector: Vec<Op>,
    pub(crate) fork_vectors: Vec<Vec<Op>>,
}

impl Binary {
    pub fn find_var(&self, v: &str) -> usize {
        self.var_names.find_name(v).expect("variable not found").0
    }

    pub fn find_literal(&self, l: Var) -> usize {
        self.literals
            .iter()
            .position(|x| *x == l)
            .expect("literal not found")
    }
}
