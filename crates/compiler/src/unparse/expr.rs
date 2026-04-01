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

use super::{Unparse, to_literal};
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
    pub(super) fn unparse_expr(&self, current_expr: &Expr) -> Result<String, DecompileError> {
        match current_expr {
            Expr::Assign { left, right } => {
                let left_needs_parens = needs_parens(
                    left,
                    current_expr,
                    self.fully_paren,
                    ParenPosition::Left,
                );
                let right_needs_parens = needs_parens(
                    right,
                    current_expr,
                    self.fully_paren,
                    ParenPosition::Right,
                );
                let left_frag = self.unparse_expr(left)?;
                let right_frag = self.unparse_expr(right)?;
                Ok(format!(
                    "{} = {}",
                    maybe_parenthesize(left_frag, left_needs_parens),
                    maybe_parenthesize(right_frag, right_needs_parens),
                ))
            }
            Expr::Pass { args } => Ok(format!("pass({})", self.unparse_args(args)?)),
            Expr::Error(code, message) => {
                let code_name = code.to_string();
                match message {
                    Some(message) => Ok(format!("{code_name}({})", self.unparse_expr(message)?)),
                    None => Ok(code_name),
                }
            }
            Expr::Value(value) => Ok(to_literal(value)),
            Expr::TypeConstant(typ) => Ok(unparse_type_constant(*typ)),
            Expr::Id(id) => Ok(self.unparse_variable(id).as_arc_str().to_string()),
            Expr::Binary(op, left, right) => {
                let left_needs_parens = needs_parens(
                    left,
                    current_expr,
                    self.fully_paren,
                    ParenPosition::Left,
                );
                let right_needs_parens = needs_parens(
                    right,
                    current_expr,
                    self.fully_paren,
                    ParenPosition::Right,
                );
                let left_frag = self.unparse_expr(left)?;
                let right_frag = self.unparse_expr(right)?;
                Ok(format!(
                    "{} {} {}",
                    maybe_parenthesize(left_frag, left_needs_parens),
                    op,
                    maybe_parenthesize(right_frag, right_needs_parens),
                ))
            }
            Expr::And(left, right) => {
                let left_needs_parens = needs_parens(
                    left,
                    current_expr,
                    self.fully_paren,
                    ParenPosition::Left,
                );
                let right_needs_parens = needs_parens(
                    right,
                    current_expr,
                    self.fully_paren,
                    ParenPosition::Right,
                );
                let left_frag = self.unparse_expr(left)?;
                let right_frag = self.unparse_expr(right)?;
                Ok(format!(
                    "{} && {}",
                    maybe_parenthesize(left_frag, left_needs_parens),
                    maybe_parenthesize(right_frag, right_needs_parens),
                ))
            }
            Expr::Or(left, right) => {
                let left_needs_parens = needs_parens(
                    left,
                    current_expr,
                    self.fully_paren,
                    ParenPosition::Left,
                );
                let right_needs_parens = needs_parens(
                    right,
                    current_expr,
                    self.fully_paren,
                    ParenPosition::Right,
                );
                let left_frag = self.unparse_expr(left)?;
                let right_frag = self.unparse_expr(right)?;
                Ok(format!(
                    "{} || {}",
                    maybe_parenthesize(left_frag, left_needs_parens),
                    maybe_parenthesize(right_frag, right_needs_parens),
                ))
            }
            Expr::Unary(op, expr) => {
                let expr_needs_parens = unary_operand_needs_parens(expr);
                let expr_frag = self.unparse_expr(expr)?;
                Ok(format!(
                    "{op}{}",
                    maybe_parenthesize(expr_frag, self.fully_paren || expr_needs_parens)
                ))
            }
            Expr::Prop { location, property } => {
                if is_system_object(location)
                    && let Some(name) = literal_property_name(property)
                {
                    return Ok(format!("${name}"));
                }
                let location_frag = self.unparse_expr(location)?;
                let location_needs_parens =
                    expr_precedence_level(location) < PrecedenceLevel::Postfix;
                Ok(format!(
                    "{}.{}",
                    maybe_parenthesize(location_frag, self.fully_paren || location_needs_parens),
                    unparse_property_access(self, property)?
                ))
            }
            Expr::Call { function, args } => {
                let args_frag = self.unparse_args(args)?;
                match function {
                    CallTarget::Builtin(name) => Ok(format!("{}({args_frag})", name.as_arc_str())),
                    CallTarget::Expr(expr) => {
                        let function_frag = self.unparse_expr(expr)?;
                        let function_needs_parens =
                            expr_precedence_level(expr) < PrecedenceLevel::Postfix;
                        Ok(format!(
                            "{}({args_frag})",
                            maybe_parenthesize(
                                function_frag,
                                self.fully_paren || function_needs_parens
                            )
                        ))
                    }
                }
            }
            Expr::Verb {
                location,
                verb,
                args,
            } => {
                if is_system_object(location)
                    && let Some(name) = system_verb_name(self, verb)
                {
                    return Ok(format!("${name}({})", self.unparse_args(args)?));
                }
                let location_frag = self.unparse_expr(location)?;
                let location_needs_parens =
                    expr_precedence_level(location) < PrecedenceLevel::Postfix;
                let args_frag = self.unparse_args(args)?;
                Ok(format!(
                    "{}:{}({args_frag})",
                    maybe_parenthesize(location_frag, self.fully_paren || location_needs_parens),
                    unparse_verb_access(self, verb)?
                ))
            }
            Expr::Range { base, from, to } => {
                let base_frag = self.unparse_expr(base)?;
                let from_frag = self.unparse_expr(from)?;
                let to_frag = self.unparse_expr(to)?;
                let base_needs_parens = expr_precedence_level(base) < PrecedenceLevel::Postfix;
                Ok(format!(
                    "{}[{from_frag}..{to_frag}]",
                    maybe_parenthesize(base_frag, self.fully_paren || base_needs_parens)
                ))
            }
            Expr::Cond {
                condition,
                consequence,
                alternative,
            } => {
                let condition_frag = self.unparse_expr(condition)?;
                let consequence_frag = self.unparse_expr(consequence)?;
                let alternative_frag = self.unparse_expr(alternative)?;
                Ok(format!(
                    "{} ? {} | {}",
                    maybe_parenthesize(
                        condition_frag,
                        needs_parens(
                            condition,
                            current_expr,
                            self.fully_paren,
                            ParenPosition::Left
                        )
                    ),
                    maybe_parenthesize(
                        consequence_frag,
                        needs_parens(
                            consequence,
                            current_expr,
                            self.fully_paren,
                            ParenPosition::Left
                        )
                    ),
                    maybe_parenthesize(
                        alternative_frag,
                        needs_parens(
                            alternative,
                            current_expr,
                            self.fully_paren,
                            ParenPosition::Right
                        )
                    ),
                ))
            }
            Expr::TryCatch {
                trye,
                codes,
                except,
            } => {
                let try_frag = self.unparse_expr(trye)?;
                let codes_frag = self.unparse_catch_codes(codes)?;
                if let Some(except) = except {
                    Ok(format!(
                        "`{} ! {} => {}'",
                        try_frag,
                        codes_frag,
                        self.unparse_expr(except)?
                    ))
                } else {
                    Ok(format!("`{} ! {}'", try_frag, codes_frag))
                }
            }
            Expr::Return(expr) => match expr {
                Some(expr) => Ok(format!("return {}", self.unparse_expr(expr)?)),
                None => Ok("return".to_string()),
            },
            Expr::Index(base, index) => {
                let base_frag = self.unparse_expr(base)?;
                let index_frag = self.unparse_expr(index)?;
                let base_needs_parens = expr_precedence_level(base) < PrecedenceLevel::Postfix;
                Ok(format!(
                    "{}[{index_frag}]",
                    maybe_parenthesize(base_frag, self.fully_paren || base_needs_parens)
                ))
            }
            Expr::List(values) => Ok(format!("{{{}}}", self.unparse_args(values)?)),
            Expr::Map(pairs) => {
                let pairs = pairs
                    .iter()
                    .map(|(key, value)| {
                        Ok(format!(
                            "{} -> {}",
                            self.unparse_expr(key)?,
                            self.unparse_expr(value)?
                        ))
                    })
                    .collect::<Result<Vec<_>, DecompileError>>()?;
                Ok(format!("[{}]", pairs.join(", ")))
            }
            Expr::Scatter(items, value) => {
                Ok(format!(
                    "{{{}}} = {}",
                    self.unparse_scatter_items(items)?,
                    self.unparse_expr(value)?
                ))
            }
            Expr::Length => Ok("$".to_string()),
            Expr::Decl { id, is_const, expr } => {
                let prefix = if *is_const { "const" } else { "let" };
                let name = self.unparse_variable(id);
                if let Some(expr) = expr {
                    Ok(format!(
                        "{prefix} {} = {}",
                        name.as_arc_str(),
                        self.unparse_expr(expr)?
                    ))
                } else {
                    Ok(format!("{prefix} {}", name.as_arc_str()))
                }
            }
            Expr::Flyweight(delegate, slots, contents) => {
                let mut parts = Vec::with_capacity(1 + slots.len() + usize::from(contents.is_some()));
                parts.push(self.unparse_expr(delegate)?);
                for (slot, value) in slots {
                    parts.push(format!(".{} = {}", slot.as_arc_str(), self.unparse_expr(value)?));
                }
                if let Some(contents) = contents {
                    parts.push(self.unparse_expr(contents)?);
                }
                Ok(format!("<{}>", parts.join(", ")))
            }
            Expr::ComprehendRange {
                variable,
                producer_expr,
                from,
                to,
                ..
            } => Ok(format!(
                "{{ {} for {} in [{}..{}] }}",
                self.unparse_expr(producer_expr)?,
                self.unparse_variable(variable).as_arc_str(),
                self.unparse_expr(from)?,
                self.unparse_expr(to)?
            )),
            Expr::ComprehendList {
                variable,
                producer_expr,
                list,
                ..
            } => Ok(format!(
                "{{ {} for {} in ({}) }}",
                self.unparse_expr(producer_expr)?,
                self.unparse_variable(variable).as_arc_str(),
                self.unparse_expr(list)?
            )),
            Expr::Lambda {
                params,
                body,
                self_name,
            } => {
                let params_frag = self.unparse_lambda_params(params)?;

                if let Some(name) = self_name {
                    let name = self.unparse_variable(name);
                    let mut writer = String::new();
                    self.unparse_named_function(params, body, &name, &mut writer, 0)?;
                    return Ok(writer.trim_end().to_string());
                }

                if let ast::StmtNode::Expr(Expr::Return(Some(expr))) = &body.node {
                    return Ok(format!(
                        "{{{params_frag}}} => {}",
                        self.unparse_expr(expr)?
                    ));
                }

                let body_frag = self.unparse_lambda_body_inline(std::slice::from_ref(body))?;
                Ok(format!("fn ({params_frag}) {}endfn", body_frag))
            }
        }
    }
}

fn maybe_parenthesize(fragment: String, needs_parens: bool) -> String {
    if needs_parens {
        format!("({fragment})")
    } else {
        fragment
    }
}

fn unparse_property_access(unparse: &Unparse<'_>, property: &Expr) -> Result<String, DecompileError> {
    if let Some(name) = property_name(unparse, property) {
        return Ok(name);
    }

    Ok(format!("({})", unparse.unparse_expr(property)?))
}

fn unparse_verb_access(unparse: &Unparse<'_>, verb: &Expr) -> Result<String, DecompileError> {
    if let Some(name) = property_name(unparse, verb) {
        return Ok(name);
    }

    Ok(format!("({})", unparse.unparse_expr(verb)?))
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
