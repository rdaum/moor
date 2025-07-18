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

/// Shared operator precedence for parsing and unparsing.
/// Higher numbers = higher precedence (more tightly binding)
use crate::ast::{BinaryOp, Expr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Precedence {
    ScatterAssign = 1,   // = (lowest precedence)
    Cond = 2,            // ? |
    Or = 3,              // ||
    And = 4,             // &&
    Equality = 8,        // == !=
    Relational = 9,      // < <= > >= in
    Additive = 11,       // + -
    Multiplicative = 12, // * / %
    Exponential = 13,    // ^
    Unary = 14,          // ! - (prefix operators)
    Primary = 15,        // literals, identifiers, function calls, etc.
}

impl Precedence {
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Get the precedence for an expression
pub fn get_precedence(expr: &Expr) -> u8 {
    match expr {
        Expr::Scatter(_, _) | Expr::Assign { .. } => Precedence::ScatterAssign.as_u8(),
        Expr::Cond { .. } => Precedence::Cond.as_u8(),
        Expr::Or(_, _) => Precedence::Or.as_u8(),
        Expr::And(_, _) => Precedence::And.as_u8(),
        Expr::Binary(op, _, _) => match op {
            BinaryOp::Eq | BinaryOp::NEq => Precedence::Equality.as_u8(),
            BinaryOp::Gt | BinaryOp::GtE | BinaryOp::Lt | BinaryOp::LtE | BinaryOp::In => {
                Precedence::Relational.as_u8()
            }
            BinaryOp::Add | BinaryOp::Sub => Precedence::Additive.as_u8(),
            BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => Precedence::Multiplicative.as_u8(),
            BinaryOp::Exp => Precedence::Exponential.as_u8(),
        },
        Expr::Unary(_, _) => Precedence::Unary.as_u8(),
        Expr::Prop { .. }
        | Expr::Verb { .. }
        | Expr::Range { .. }
        | Expr::ComprehendRange { .. }
        | Expr::ComprehendList { .. }
        | Expr::Index(_, _)
        | Expr::Value(_)
        | Expr::Error(_, _)
        | Expr::Id(_)
        | Expr::TypeConstant(_)
        | Expr::List(_)
        | Expr::Map(_)
        | Expr::Flyweight(..)
        | Expr::Pass { .. }
        | Expr::Call { .. }
        | Expr::Length
        | Expr::Decl { .. }
        | Expr::Return(_)
        | Expr::TryCatch { .. }
        | Expr::Lambda { .. } => Precedence::Primary.as_u8(),
    }
}
