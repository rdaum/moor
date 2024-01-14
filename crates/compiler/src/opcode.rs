// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use bincode::{Decode, Encode};
use moor_values::var::error::Error;
use moor_values::var::objid::Objid;

use moor_values::var::Var;

use crate::labels::{Label, Name, Offset};

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
    ImmNone,
    ImmBigInt(i64),
    ImmInt(i32),
    ImmErr(Error),
    ImmObjid(Objid),
    ImmEmptyList,
    Imm(Label),
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
