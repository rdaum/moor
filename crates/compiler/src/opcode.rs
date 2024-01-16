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
use moor_values::var::Error;
use moor_values::var::Objid;

use crate::labels::{Label, Name, Offset};

#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Encode, Decode)]
pub enum Op {
    If(Label),
    Eif(Label),
    IfQues(Label),
    While(Label),
    Jump { label: Label },
    ForList { id: Name, end_label: Label },
    ForRange { id: Name, end_label: Label },
    Pop,
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
    GPut { id: Name },
    GPush { id: Name },
    GetProp,
    PushGetProp,
    PutProp,
    Fork { fv_offset: Offset, id: Option<Name> },
    CallVerb,
    Return,
    Return0,
    Done,
    FuncCall { id: Name },
    Pass,
    RangeSet,
    Length(Offset),
    Exp,
    Scatter(Box<ScatterArgs>),
    PushLabel(Label),
    TryFinally(Label),
    Catch(Label),
    TryExcept { num_excepts: usize },
    EndCatch(Label),
    EndExcept(Label),
    EndFinally,
    WhileId { id: Name, end_label: Label },
    Continue,
    ExitId(Label),
    Exit { stack: Offset, label: Label },
}

#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Encode, Decode)]
pub enum ScatterLabel {
    Required(Name),
    Rest(Name),
    Optional(Name, Option<Label>),
}

#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Encode, Decode)]
pub struct ScatterArgs {
    pub nargs: usize,
    pub nreq: usize,
    pub rest: usize,
    pub labels: Vec<ScatterLabel>,
    pub done: Label,
}

#[cfg(test)]
mod tests {
    use crate::{Label, Name, Offset};

    /// Verify we don't go over our 16 byte budget for opcodes.
    // TODO: This is still rather bloated.
    #[test]
    fn size_opcode() {
        use crate::opcode::Op;
        use std::mem::size_of;
        assert_eq!(size_of::<Op>(), 16);
        assert_eq!(size_of::<Name>(), 2);
        assert_eq!(size_of::<Offset>(), 2);
        assert_eq!(size_of::<Label>(), 2);
    }
}
