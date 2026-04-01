// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

//! Syntax kinds for the handwritten frontend rewrite.
//!
//! Phase 1 only uses the token variants directly. Composite node variants are
//! included so later phases can build on the same enum without renumbering it.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum SyntaxKind {
    // Special
    Error,
    Eof,

    // Trivia
    Whitespace,
    Newline,
    LineComment,
    BlockComment,

    // Literals and identifiers
    Ident,
    IntLit,
    FloatLit,
    StringLit,
    ObjectLit,
    ErrorLit,
    SymbolLit,
    BinaryLit,
    TypeConstant,

    // Keywords reserved in normal MOO mode.
    IfKw,
    ElseKw,
    ElseIfKw,
    EndIfKw,
    ForKw,
    EndForKw,
    WhileKw,
    EndWhileKw,
    ForkKw,
    EndForkKw,
    InKw,
    ReturnKw,
    BreakKw,
    ContinueKw,
    TryKw,
    ExceptKw,
    FinallyKw,
    EndTryKw,
    FnKw,
    EndFnKw,
    LetKw,
    ConstKw,
    GlobalKw,
    PassKw,
    AnyKw,
    TrueKw,
    FalseKw,

    // Operators and punctuation
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    AmpDot,
    PipeDot,
    CaretDot,
    Tilde,
    Shl,
    Shr,
    LShr,
    AmpAmp,
    PipePipe,
    Bang,
    EqEq,
    BangEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    Eq,
    Arrow,
    FatArrow,
    DotDot,
    Semi,
    Comma,
    Dot,
    Colon,
    At,
    Hash,
    Dollar,
    Backtick,
    Apostrophe,
    Question,
    Pipe,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,

    // Composite nodes for later CST phases.
    Program,
    StmtList,
    IfStmt,
    ElseIfClause,
    ElseClause,
    ForInStmt,
    ForRangeStmt,
    WhileStmt,
    ForkStmt,
    TryExceptStmt,
    TryFinallyStmt,
    ExceptClause,
    ReturnStmt,
    BreakStmt,
    ContinueStmt,
    ExprStmt,
    BeginStmt,
    FnStmt,
    LetStmt,
    ConstStmt,
    GlobalStmt,
    BinExpr,
    UnaryExpr,
    ParenExpr,
    CondExpr,
    IndexExpr,
    RangeExpr,
    CallExpr,
    VerbCallExpr,
    PropExpr,
    AssignExpr,
    ScatterExpr,
    ListExpr,
    MapExpr,
    FlyweightExpr,
    LambdaExpr,
    TryExpr,
    PassExpr,
    SysPropExpr,
    ComprehensionExpr,
    ArgList,
    ParamList,
    ScatterItem,
    ObjectsFile,
    ObjectDef,
    VerbDecl,
    PropDef,
    PropSet,
    Literal,
    ConstantDecl,
}

impl SyntaxKind {
    pub fn is_trivia(self) -> bool {
        matches!(
            self,
            Self::Whitespace | Self::Newline | Self::LineComment | Self::BlockComment
        )
    }

    pub fn can_end_expr(self) -> bool {
        matches!(
            self,
            Self::Ident
                | Self::IntLit
                | Self::FloatLit
                | Self::StringLit
                | Self::ObjectLit
                | Self::ErrorLit
                | Self::SymbolLit
                | Self::BinaryLit
                | Self::TypeConstant
                | Self::TrueKw
                | Self::FalseKw
                | Self::PassKw
                | Self::AnyKw
                | Self::RParen
                | Self::RBracket
                | Self::RBrace
        )
    }
}
