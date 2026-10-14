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

use crate::SyntaxKind;

pub(crate) fn is_name_like_token(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Ident
            | SyntaxKind::IfKw
            | SyntaxKind::ElseKw
            | SyntaxKind::ElseIfKw
            | SyntaxKind::EndIfKw
            | SyntaxKind::ForKw
            | SyntaxKind::EndForKw
            | SyntaxKind::WhileKw
            | SyntaxKind::EndWhileKw
            | SyntaxKind::ForkKw
            | SyntaxKind::EndForkKw
            | SyntaxKind::InKw
            | SyntaxKind::ReturnKw
            | SyntaxKind::BreakKw
            | SyntaxKind::ContinueKw
            | SyntaxKind::TryKw
            | SyntaxKind::ExceptKw
            | SyntaxKind::FinallyKw
            | SyntaxKind::EndTryKw
            | SyntaxKind::FnKw
            | SyntaxKind::EndFnKw
            | SyntaxKind::LetKw
            | SyntaxKind::ConstKw
            | SyntaxKind::GlobalKw
            | SyntaxKind::PassKw
            | SyntaxKind::AnyKw
            | SyntaxKind::TrueKw
            | SyntaxKind::FalseKw
            | SyntaxKind::NoneLit
    )
}

pub(crate) fn is_atom_token(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Ident
            | SyntaxKind::IntLit
            | SyntaxKind::FloatLit
            | SyntaxKind::StringLit
            | SyntaxKind::ObjectLit
            | SyntaxKind::ErrorLit
            | SyntaxKind::SymbolLit
            | SyntaxKind::BinaryLit
            | SyntaxKind::NoneLit
            | SyntaxKind::TypeConstant
            | SyntaxKind::TrueKw
            | SyntaxKind::FalseKw
            | SyntaxKind::AnyKw
            | SyntaxKind::GlobalKw
    )
}

pub(crate) fn is_expr_start(kind: SyntaxKind) -> bool {
    is_atom_token(kind)
        || matches!(
            kind,
            SyntaxKind::PassKw
                | SyntaxKind::ReturnKw
                | SyntaxKind::FnKw
                | SyntaxKind::Dollar
                | SyntaxKind::LParen
                | SyntaxKind::LBrace
                | SyntaxKind::LBracket
                | SyntaxKind::Lt
                | SyntaxKind::Backtick
                | SyntaxKind::Minus
                | SyntaxKind::Bang
                | SyntaxKind::Tilde
        )
}
