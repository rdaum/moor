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

use super::{Unparse, write_literal};
use crate::{
    ast::{self, CallTarget, Expr},
    decompile::DecompileError,
    precedence::{PrecedenceLevel, expr_precedence_level},
};
use moor_common::util::quote_str;
use moor_var::{Obj, VarType};
pub(super) enum ParenPosition {
    Left,
    Right,
}

pub(super) fn needs_parens(
    expr: &Expr,
    parent: &Expr,
    fully_paren: bool,
    paren_position: ParenPosition,
) -> bool {
    let current_precedence = expr_precedence_level(expr);
    let parent_precedence = expr_precedence_level(parent);

    if fully_paren {
        return current_precedence != PrecedenceLevel::Atomic;
    }

    if current_precedence < parent_precedence {
        return true;
    }
    if current_precedence > parent_precedence {
        return false;
    }

    if current_precedence == PrecedenceLevel::Conditional {
        return true;
    }

    if current_precedence == PrecedenceLevel::Exponent {
        return matches!(paren_position, ParenPosition::Left);
    }

    matches!(paren_position, ParenPosition::Right)
}

impl<'a> Unparse<'a> {
    pub(super) fn write_expr<W: std::fmt::Write>(
        &self,
        expr: &Expr,
        writer: &mut W,
    ) -> Result<(), DecompileError> {
        self.write_expr_in_context(expr, writer, None, ParenPosition::Left)
    }

    fn write_expr_in_context<W: std::fmt::Write>(
        &self,
        expr: &Expr,
        writer: &mut W,
        parent: Option<&Expr>,
        paren_position: ParenPosition,
    ) -> Result<(), DecompileError> {
        let needs_parens = parent
            .is_some_and(|parent| needs_parens(expr, parent, self.fully_paren, paren_position));

        if needs_parens {
            write!(writer, "(")?;
        }

        self.write_expr_inner(expr, writer)?;

        if needs_parens {
            write!(writer, ")")?;
        }
        Ok(())
    }

    fn write_expr_inner<W: std::fmt::Write>(
        &self,
        expr: &Expr,
        writer: &mut W,
    ) -> Result<(), DecompileError> {
        match expr {
            Expr::Assign { left, right } => {
                self.write_expr_in_context(left, writer, Some(expr), ParenPosition::Left)?;
                write!(writer, " = ")?;
                self.write_expr_in_context(right, writer, Some(expr), ParenPosition::Right)?;
                Ok(())
            }
            Expr::Binary(op, left, right) => {
                self.write_expr_in_context(left, writer, Some(expr), ParenPosition::Left)?;
                write!(writer, " {op} ")?;
                self.write_expr_in_context(right, writer, Some(expr), ParenPosition::Right)?;
                Ok(())
            }
            Expr::And(left, right) => {
                self.write_expr_in_context(left, writer, Some(expr), ParenPosition::Left)?;
                write!(writer, " && ")?;
                self.write_expr_in_context(right, writer, Some(expr), ParenPosition::Right)?;
                Ok(())
            }
            Expr::Or(left, right) => {
                self.write_expr_in_context(left, writer, Some(expr), ParenPosition::Left)?;
                write!(writer, " || ")?;
                self.write_expr_in_context(right, writer, Some(expr), ParenPosition::Right)?;
                Ok(())
            }
            Expr::Unary(op, operand) => {
                write!(writer, "{op}")?;
                let operand_needs_parens = self.fully_paren || unary_operand_needs_parens(operand);
                write_maybe_parenthesized_expr(self, operand, writer, operand_needs_parens)?;
                Ok(())
            }
            Expr::Pass { args } => {
                write!(writer, "pass(")?;
                self.write_args(args, writer)?;
                write!(writer, ")")?;
                Ok(())
            }
            Expr::Error(code, message) => {
                write!(writer, "{code}")?;
                if let Some(message) = message {
                    write!(writer, "(")?;
                    self.write_expr(message, writer)?;
                    write!(writer, ")")?;
                }
                Ok(())
            }
            Expr::Value(value) => write_literal(value, writer),
            Expr::TypeConstant(typ) => {
                write!(writer, "{}", unparse_type_constant(*typ)).map_err(Into::into)
            }
            Expr::Return(expr) => {
                write!(writer, "return")?;
                if let Some(expr) = expr {
                    write!(writer, " ")?;
                    self.write_expr(expr, writer)?;
                }
                Ok(())
            }
            Expr::Length => write!(writer, "$").map_err(Into::into),
            Expr::List(values) => {
                write!(writer, "{{")?;
                self.write_args(values, writer)?;
                write!(writer, "}}")?;
                Ok(())
            }
            Expr::Map(pairs) => {
                write!(writer, "[")?;
                for (i, (key, value)) in pairs.iter().enumerate() {
                    if i > 0 {
                        write!(writer, ", ")?;
                    }
                    self.write_expr(key, writer)?;
                    write!(writer, " -> ")?;
                    self.write_expr(value, writer)?;
                }
                write!(writer, "]")?;
                Ok(())
            }
            Expr::Decl { id, is_const, expr } => {
                let prefix = if *is_const { "const" } else { "let" };
                write!(
                    writer,
                    "{prefix} {}",
                    self.unparse_variable(id).as_arc_str()
                )?;
                if let Some(expr) = expr {
                    write!(writer, " = ")?;
                    self.write_expr(expr, writer)?;
                }
                Ok(())
            }
            Expr::Scatter(items, value) => {
                write!(writer, "{{")?;
                self.write_scatter_items(items, writer)?;
                write!(writer, "}} = ")?;
                self.write_expr(value, writer)?;
                Ok(())
            }
            Expr::Call { function, args } => {
                match function {
                    CallTarget::Builtin(name) => {
                        write!(writer, "{}(", name.as_arc_str())?;
                    }
                    CallTarget::Expr(expr) => {
                        let needs_parens = self.fully_paren
                            || expr_precedence_level(expr) < PrecedenceLevel::Postfix;
                        write_maybe_parenthesized_expr(self, expr, writer, needs_parens)?;
                        write!(writer, "(")?;
                    }
                }
                self.write_args(args, writer)?;
                write!(writer, ")")?;
                Ok(())
            }
            Expr::Prop { location, property } => {
                if is_system_object(location)
                    && let Some(name) = literal_property_name(property)
                {
                    write!(writer, "${name}")?;
                    return Ok(());
                }

                let location_needs_parens =
                    self.fully_paren || expr_precedence_level(location) < PrecedenceLevel::Postfix;
                write_maybe_parenthesized_expr(self, location, writer, location_needs_parens)?;
                write!(writer, ".{}", unparse_property_access(self, property)?)?;
                Ok(())
            }
            Expr::Index(base, index) => {
                let base_needs_parens =
                    self.fully_paren || expr_precedence_level(base) < PrecedenceLevel::Postfix;
                write_maybe_parenthesized_expr(self, base, writer, base_needs_parens)?;
                write!(writer, "[")?;
                self.write_expr(index, writer)?;
                write!(writer, "]")?;
                Ok(())
            }
            Expr::Verb {
                location,
                verb,
                args,
            } => {
                if is_system_object(location)
                    && let Some(name) = system_verb_name(self, verb)
                {
                    write!(writer, "${name}(")?;
                    self.write_args(args, writer)?;
                    write!(writer, ")")?;
                    return Ok(());
                }

                let location_needs_parens =
                    self.fully_paren || expr_precedence_level(location) < PrecedenceLevel::Postfix;
                write_maybe_parenthesized_expr(self, location, writer, location_needs_parens)?;
                write!(writer, ":{}(", unparse_verb_access(self, verb)?)?;
                self.write_args(args, writer)?;
                write!(writer, ")")?;
                Ok(())
            }
            Expr::Range { base, from, to } => {
                let base_needs_parens =
                    self.fully_paren || expr_precedence_level(base) < PrecedenceLevel::Postfix;
                write_maybe_parenthesized_expr(self, base, writer, base_needs_parens)?;
                write!(writer, "[")?;
                self.write_expr(from, writer)?;
                write!(writer, "..")?;
                self.write_expr(to, writer)?;
                write!(writer, "]")?;
                Ok(())
            }
            Expr::Cond {
                condition,
                consequence,
                alternative,
            } => {
                self.write_expr_in_context(condition, writer, Some(expr), ParenPosition::Left)?;
                write!(writer, " ? ")?;
                self.write_expr_in_context(consequence, writer, Some(expr), ParenPosition::Left)?;
                write!(writer, " | ")?;
                self.write_expr_in_context(alternative, writer, Some(expr), ParenPosition::Right)?;
                Ok(())
            }
            Expr::TryCatch {
                trye,
                codes,
                except,
            } => {
                write!(writer, "`")?;
                self.write_expr(trye, writer)?;
                write!(writer, " ! ")?;
                self.write_catch_codes(codes, writer)?;
                if let Some(except) = except {
                    write!(writer, " => ")?;
                    self.write_expr(except, writer)?;
                }
                write!(writer, "'")?;
                Ok(())
            }
            Expr::Flyweight(delegate, slots, contents) => {
                write!(writer, "<")?;
                self.write_expr(delegate, writer)?;
                for (slot, value) in slots {
                    write!(writer, ", .{} = ", slot.as_arc_str())?;
                    self.write_expr(value, writer)?;
                }
                if let Some(contents) = contents {
                    write!(writer, ", ")?;
                    self.write_expr(contents, writer)?;
                }
                write!(writer, ">")?;
                Ok(())
            }
            Expr::ComprehendRange {
                variable,
                producer_expr,
                from,
                to,
                ..
            } => {
                write!(writer, "{{ ")?;
                self.write_expr(producer_expr, writer)?;
                write!(
                    writer,
                    " for {} in [",
                    self.unparse_variable(variable).as_arc_str()
                )?;
                self.write_expr(from, writer)?;
                write!(writer, "..")?;
                self.write_expr(to, writer)?;
                write!(writer, "] }}")?;
                Ok(())
            }
            Expr::ComprehendList {
                variable,
                producer_expr,
                list,
                ..
            } => {
                write!(writer, "{{ ")?;
                self.write_expr(producer_expr, writer)?;
                write!(
                    writer,
                    " for {} in (",
                    self.unparse_variable(variable).as_arc_str()
                )?;
                self.write_expr(list, writer)?;
                write!(writer, ") }}")?;
                Ok(())
            }
            Expr::Lambda {
                params,
                body,
                self_name,
            } => {
                if let Some(name) = self_name {
                    let name = self.unparse_variable(name);
                    let mut function_buffer = String::new();
                    self.unparse_named_function(params, body, &name, &mut function_buffer, 0)?;
                    write!(writer, "{}", function_buffer.trim_end())?;
                    return Ok(());
                }

                if let ast::StmtNode::Expr(Expr::Return(Some(expr))) = &body.node {
                    write!(writer, "{{")?;
                    self.write_lambda_params(params, writer)?;
                    write!(writer, "}} => ")?;
                    self.write_expr(expr, writer)?;
                    return Ok(());
                }

                write!(writer, "fn (")?;
                self.write_lambda_params(params, writer)?;
                write!(writer, ") ")?;
                self.write_lambda_body_inline(std::slice::from_ref(body), writer)?;
                write!(writer, "endfn")?;
                Ok(())
            }
        }
    }
}

fn write_maybe_parenthesized_expr<W: std::fmt::Write>(
    unparse: &Unparse<'_>,
    expr: &Expr,
    writer: &mut W,
    needs_parens: bool,
) -> Result<(), DecompileError> {
    if needs_parens {
        write!(writer, "(")?;
    }
    unparse.write_expr(expr, writer)?;
    if needs_parens {
        write!(writer, ")")?;
    }
    Ok(())
}

fn unparse_property_access(unparse: &Unparse<'_>, property: &Expr) -> Result<String, DecompileError> {
    if let Some(name) = property_name(unparse, property) {
        return Ok(name);
    }

    let mut buffer = String::from("(");
    unparse.write_expr(property, &mut buffer)?;
    buffer.push(')');
    Ok(buffer)
}

fn unparse_verb_access(unparse: &Unparse<'_>, verb: &Expr) -> Result<String, DecompileError> {
    if let Some(name) = property_name(unparse, verb) {
        return Ok(name);
    }

    let mut buffer = String::from("(");
    unparse.write_expr(verb, &mut buffer)?;
    buffer.push(')');
    Ok(buffer)
}

fn property_name(_unparse: &Unparse<'_>, expr: &Expr) -> Option<String> {
    match expr {
        Expr::Value(value) => {
            if let Ok(symbol) = value.as_symbol() {
                return Some(format_name_fragment(symbol.as_arc_str().as_ref()));
            }
            value
                .as_string()
                .map(|s| format_name_fragment(s.as_ref()))
        }
        _ => None,
    }
}

fn literal_property_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Value(value) => {
            if let Ok(symbol) = value.as_symbol() {
                return Some(symbol.as_arc_str().to_string());
            }
            value.as_string().map(|s| s.to_string())
        }
        _ => None,
    }
}

fn system_verb_name(unparse: &Unparse<'_>, expr: &Expr) -> Option<String> {
    match expr {
        Expr::Id(id) => Some(unparse.unparse_variable(id).as_arc_str().to_string()),
        _ => literal_property_name(expr),
    }
}

fn is_system_object(expr: &Expr) -> bool {
    matches!(expr, Expr::Value(value) if value.as_object() == Some(Obj::mk_id(0)))
}

fn unparse_type_constant(typ: VarType) -> String {
    typ.to_literal().to_string()
}

fn unary_operand_needs_parens(expr: &Expr) -> bool {
    match expr {
        Expr::Assign { .. }
        | Expr::Binary(..)
        | Expr::And(..)
        | Expr::Or(..)
        | Expr::Prop { .. }
        | Expr::Verb { .. }
        | Expr::Range { .. }
        | Expr::Cond { .. }
        | Expr::TryCatch { .. }
        | Expr::Index(..)
        | Expr::Scatter(..)
        | Expr::Decl { .. }
        | Expr::Return(..) => true,
        Expr::Call { function, .. } => matches!(function, CallTarget::Expr(..)),
        _ => false,
    }
}

fn format_name_fragment(name: &str) -> String {
    let needs_quotes = name.chars().any(|c| !c.is_alphanumeric() && c != '_')
        || (name.chars().next().is_some_and(|c| c.is_numeric()) && !name.starts_with('_'));
    if needs_quotes {
        format!("({})", quote_str(name))
    } else {
        name.to_string()
    }
}
