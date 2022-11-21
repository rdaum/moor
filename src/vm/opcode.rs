use crate::model::var::{Objid, Var};
use crate::model::verbs::Program;
use anyhow::anyhow;
use serde_derive::{Deserialize, Serialize};

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
    PushRef,
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

    // extended
    Length {
        id: usize,
    },
    Exp,
    Scatter {
        nargs: usize,
        nreg: usize,
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
    Continue,
    WhileId {
        id: usize,
        label: usize,
    },
    ExitId {
        id: usize,
    },
    Exit,
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct Binary {
    pub(crate) first_lineno: usize,
    pub(crate) ref_count: usize,
    pub(crate) literals: Vec<Var>,
    pub(crate) var_names: Vec<String>,
    pub(crate) main_vector: Vec<Op>,
}
