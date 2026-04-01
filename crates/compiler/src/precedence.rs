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

//! Shared operator precedence used by both parsing and unparsing.

use crate::ast::{BinaryOp, Expr, UnaryOp};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PrecedenceLevel {
    Assignment,
    Conditional,
    Logical,
    BitwiseOr,
    BitwiseXor,
    BitwiseAnd,
    Comparison,
    BitwiseShift,
    Arithmetic,
    Multiplicative,
    Exponent,
    Unary,
    Postfix,
    Atomic,
}

pub fn expr_precedence_level(expr: &Expr) -> PrecedenceLevel {
    match expr {
        Expr::Assign { .. } | Expr::Scatter(_, _) => PrecedenceLevel::Assignment,
        Expr::Cond { .. } => PrecedenceLevel::Conditional,
        Expr::Or(..) | Expr::And(..) => PrecedenceLevel::Logical,
        Expr::Binary(BinaryOp::BitOr, _, _) => PrecedenceLevel::BitwiseOr,
        Expr::Binary(BinaryOp::BitXor, _, _) => PrecedenceLevel::BitwiseXor,
        Expr::Binary(BinaryOp::BitAnd, _, _) => PrecedenceLevel::BitwiseAnd,
        Expr::Binary(
            BinaryOp::Eq
            | BinaryOp::NEq
            | BinaryOp::Gt
            | BinaryOp::Lt
            | BinaryOp::GtE
            | BinaryOp::LtE
            | BinaryOp::In,
            _,
            _,
        ) => PrecedenceLevel::Comparison,
        Expr::Binary(BinaryOp::BitShl | BinaryOp::BitLShr | BinaryOp::BitShr, _, _) => {
            PrecedenceLevel::BitwiseShift
        }
        Expr::Binary(BinaryOp::Add | BinaryOp::Sub, _, _) => PrecedenceLevel::Arithmetic,
        Expr::Binary(BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod, _, _) => {
            PrecedenceLevel::Multiplicative
        }
        Expr::Binary(BinaryOp::Exp, _, _) => PrecedenceLevel::Exponent,
        Expr::Unary(UnaryOp::Neg | UnaryOp::Not | UnaryOp::BitNot, _) => PrecedenceLevel::Unary,
        Expr::Index(..) | Expr::Verb { .. } | Expr::Prop { .. } | Expr::Call { .. } => {
            PrecedenceLevel::Postfix
        }
        _ => PrecedenceLevel::Atomic,
    }
}
