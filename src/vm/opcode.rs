use serde_derive::{Deserialize, Serialize};

use crate::compiler::codegen::JumpLabel;
use crate::model::var::Var;

#[derive(Clone, Deserialize, Serialize, Debug, PartialEq)]
pub enum Op {
    Label(usize),
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
        rest: usize,
        id: usize,
        label: usize,
        done : usize
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
    Continue(usize),
    Break(usize),
    This,
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct Label {
    pub (crate) offset : usize,
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct Binary {
    pub(crate) first_lineno: usize,
    pub(crate) literals: Vec<Var>,
    pub(crate) jump_labels: Vec<JumpLabel>,
    pub(crate) var_names: Vec<String>,
    pub(crate) main_vector: Vec<Op>,
}
