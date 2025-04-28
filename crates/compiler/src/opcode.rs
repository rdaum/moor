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

use crate::builtins::BuiltinId;
use crate::labels::{Label, Offset};
use crate::names::Name;
use bincode::{Decode, Encode};
use moor_var::Obj;
use moor_var::{Error, VarType};

#[derive(Clone, Debug, PartialEq, PartialOrd, Encode, Decode)]
pub enum Op {
    Add,
    And(Label),
    CallVerb,
    CheckListForSplice,
    FinallyContinue,
    Div,
    Done,
    EndCatch(Label),
    EndExcept(Label),
    EndFinally,
    Eq,
    Exit {
        stack: Offset,
        label: Label,
    },
    ExitId(Label),
    Exp,
    ForSequence {
        value_bind: Name,
        key_bind: Option<Name>,
        end_label: Label,
        environment_width: u16,
    },
    ForRange {
        id: Name,
        end_label: Label,
        environment_width: u16,
    },
    Fork {
        fv_offset: Offset,
        id: Option<Name>,
    },
    FuncCall {
        id: BuiltinId,
    },
    Ge,
    GetProp,
    Gt,
    IfQues(Label),
    Imm(Label),
    ImmBigInt(i64),
    ImmFloat(f64),
    ImmEmptyList,
    ImmErr(Error),
    ImmInt(i32),
    ImmType(VarType),
    ImmNone,
    ImmObjid(Obj),
    In,
    IndexSet,
    Jump {
        label: Label,
    },
    Le,
    Length(Offset),
    ListAddTail,
    ListAppend,
    Lt,
    MakeSingletonList,
    MakeMap,
    MapInsert,
    MakeFlyweight(usize),
    Mod,
    Mul,
    Ne,
    Not,
    Or(Label),
    Pass,
    Pop,
    Push(Name),
    PushGetProp,
    PushRef,
    PushTemp,
    Put(Name),
    PutProp,
    PutTemp,
    RangeRef,
    RangeSet,
    Ref,
    Return,
    Return0,
    Scatter(Box<ScatterArgs>),
    Sub,
    PushCatchLabel(Label),
    TryCatch {
        handler_label: Label,
        end_label: Label,
    },
    TryExcept {
        num_excepts: u16,
        environment_width: u16,
        end_label: Label,
    },
    TryFinally {
        end_label: Label,
        environment_width: u16,
    },
    /// Begin a lexical scope, expanding the Environment by `num_bindings`
    BeginScope {
        num_bindings: u16,
        end_label: Label,
    },
    /// End a lexical scope, contracting the Environment by `num_bindings`
    EndScope {
        num_bindings: u16,
    },
    UnaryMinus,
    While {
        jump_label: Label,
        environment_width: u16,
    },
    WhileId {
        id: Name,
        end_label: Label,
        environment_width: u16,
    },
    If(Label, u16),
    Eif(Label, u16),
    BeginComprehension(ComprehensionType, Label, Label),
    ComprehendRange {
        /// The variable to populate with the result of the current range iteration, which is
        /// declared in the current scope.
        position: Name,
        end_of_range_register: Name,
        /// Where to jump after done producing
        end_label: Label,
    },
    ComprehendList {
        /// The register (unnamed variable) which holds the current offset into the list
        position_register: Name,
        /// The register holding the evaluated list we're iterating.
        list_register: Name,
        /// The variable we assign with the value indexed from the list.
        item_variable: Name,
        /// Where to jump after done producing
        end_label: Label,
    },
    ContinueComprehension(Name),
}

#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Encode, Decode)]
pub enum ComprehensionType {
    Range,
    List,
}

#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Encode, Decode)]
pub enum ScatterLabel {
    Optional(Name, Option<Label>),
    Required(Name),
    Rest(Name),
}

#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Encode, Decode)]
pub struct ScatterArgs {
    pub labels: Vec<ScatterLabel>,
    pub done: Label,
}

#[cfg(test)]
mod tests {
    use crate::names::Name;
    use crate::{Label, Offset};

    /// Verify we don't go over our 24 byte budget for opcodes.
    // TODO: This is still rather bloated.
    //  and was 16, until we widened ForSequence
    //  we need to come up with a more compact representation
    #[test]
    fn size_opcode() {
        use crate::opcode::Op;
        use std::mem::size_of;
        assert_eq!(size_of::<Op>(), 24);
        assert_eq!(size_of::<Name>(), 2);
        assert_eq!(size_of::<Offset>(), 2);
        assert_eq!(size_of::<Label>(), 2);
    }
}
