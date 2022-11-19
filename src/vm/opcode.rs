use crate::model::var::{Objid, Var};
use crate::model::verbs::Program;
use anyhow::anyhow;
use bytecheck::CheckBytes;
use rkyv::vec::ArchivedVec;
use rkyv::{Archive, Deserialize, Serialize};

#[derive(Clone, Archive, Deserialize, Serialize, Debug, PartialEq)]
#[archive_attr(derive(CheckBytes, Debug))]
pub enum Op {
    If,
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
    Imm,
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
    And,
    Or,
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
        done: usize,
    },
    PushLabel,
    TryFinally,
    Catch,
    TryExcept,
    EndCatch,
    EndExcept,
    EndFinally,
    Continue,
    WhileId {
        id: usize,
    },
    ExitId {
        id: usize,
    },
    Exit,
}

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
#[archive_attr(derive(CheckBytes, Debug))]
pub struct Binary {
    first_lineno: usize,
    ref_count: usize,
    num_literals: usize,
    pub(crate) var_names: Vec<String>,
    pub(crate) main_vector: Vec<Op>,
}

