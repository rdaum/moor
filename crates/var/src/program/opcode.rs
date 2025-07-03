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

use crate::program::labels::{Label, Offset};
use crate::program::names::Name;
use crate::{ErrorCode, Obj, VarType};
use bincode::{Decode, Encode};

#[derive(Clone, Copy, Debug, PartialOrd, PartialEq, Eq, Hash, Encode, Decode)]
pub struct BuiltinId(pub u16);

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Encode, Decode)]
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
    ForSequence(Offset),
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
    /// Pushes just an error code, and is used when the literal has no message portion.
    ImmErr(ErrorCode),
    /// Operand is used because not doing so blew us over our 16-byte limit for some reason.
    /// Expects stack to contain a message portion of the error, immediately after.
    MakeError(Offset),
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
    Scatter(Offset),
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
    ComprehendRange(Offset),
    ComprehendList(Offset),
    ContinueComprehension(Name),
    /// Create lambda value from pre-compiled Program and parameter specification
    /// The lambda Program is compiled at compile-time and stored in lambda_programs table
    MakeLambda {
        scatter_offset: Offset, // Reference to parameter spec in scatter_tables
        program_offset: Offset, // Reference to pre-compiled Program in lambda_programs table
    },
    /// Call a lambda value with arguments from stack
    /// Expects stack: [lambda_value, args_list]
    /// Uses existing scatter assignment for parameter binding
    CallLambda,
}

#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Encode, Decode)]
pub struct ForSequenceOperand {
    pub value_bind: Name,
    pub key_bind: Option<Name>,
    pub end_label: Label,
    pub environment_width: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Encode, Decode)]
pub struct RangeComprehend {
    /// The variable to populate with the result of the current range iteration, which is
    /// declared in the current scope.
    pub position: Name,
    pub end_of_range_register: Name,
    /// Where to jump after done producing
    pub end_label: Label,
}

#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Encode, Decode)]
pub struct ListComprehend {
    /// The register (unnamed variable) which holds the current offset into the list
    pub position_register: Name,
    /// The register holding the evaluated list we're iterating.
    pub list_register: Name,
    /// The variable we assign with the value indexed from the list.
    pub item_variable: Name,
    /// Where to jump after done producing
    pub end_label: Label,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Encode, Decode)]
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
    use crate::program::labels::{Label, Offset};
    use crate::program::names::Name;
    use crate::program::opcode::Op;

    #[test]
    fn size_opcode() {
        use std::mem::size_of;
        assert_eq!(size_of::<Op>(), 16);
        assert_eq!(size_of::<Name>(), 6);
        assert_eq!(size_of::<Offset>(), 2);
        assert_eq!(size_of::<Label>(), 2);
    }
}
