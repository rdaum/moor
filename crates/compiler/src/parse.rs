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

/// Kicks off the Pest parser and converts it into our AST.
/// This is the main entry point for parsing.
use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;

use itertools::Itertools;
use moor_var::{ErrorCode, SYSTEM_OBJECT, Var, VarType};
use moor_var::{Symbol, v_none};
pub use pest::Parser as PestParser;
use pest::error::LineColLocation;
use pest::iterators::{Pair, Pairs};
use pest::pratt_parser::{Assoc, Op, PrattParser};

use moor_var::Obj;
use moor_var::{v_binary, v_float, v_int, v_obj, v_str, v_string};

use crate::ast::Arg::{Normal, Splice};
use crate::ast::StmtNode::Scope;
use crate::ast::{
    Arg, BinaryOp, CatchCodes, CondArm, ElseArm, ExceptArm, Expr, ScatterItem, ScatterKind, Stmt,
    StmtNode, UnaryOp,
};
use crate::parse::moo::{MooParser, Rule};
use crate::unparse::annotate_line_numbers;
use crate::var_scope::VarScope;
use base64::{Engine, engine::general_purpose};
use moor_common::model::CompileError::{DuplicateVariable, UnknownTypeConstant};
use moor_common::model::{CompileContext, CompileError};
use moor_common::program::DeclType;
use moor_common::program::names::Names;

pub mod moo {
    #[derive(Parser)]
    #[grammar = "src/moo.pest"]
    pub struct MooParser;
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CompileOptions {
    /// Whether we allow lexical scope blocks. begin/end blocks and 'let' and 'global' statements
    pub lexical_scopes: bool,
    /// Whether to support a Map datatype ([ k -> v, .. ]) compatible with Stunt/ToastStunt
    pub map_type: bool,
    /// Whether to support the flyweight type (a delegate object with slots and contents)
    pub flyweight_type: bool, // TODO: future options:
    //      - symbol types
    //      - disable "#" style object references (obscure_references)
    /// Whether to support list and range comprehensions in the compiler
    pub list_comprehensions: bool,
    /// Whether to support boolean types in compilation
    pub bool_type: bool,
    /// Whether to support symbol types ('sym) in compilation
    pub symbol_type: bool,
    /// Whether to support non-stanard custom error values.
    pub custom_errors: bool,
    /// Whether to turn unsupported builtins into `call_function` invocations.
    /// Useful for textdump imports from other MOO dialects.
    pub call_unsupported_builtins: bool,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            lexical_scopes: true,
            map_type: true,
            flyweight_type: true,
            list_comprehensions: true,
            bool_type: true,
            symbol_type: true,
            custom_errors: true,
            call_unsupported_builtins: false,
        }
    }
}

pub struct TreeTransformer {
    // TODO: this is RefCell because PrattParser has some API restrictions which result in
    //   borrowing issues, see: https://github.com/pest-parser/pest/discussions/1030
    names: RefCell<VarScope>,
    options: CompileOptions,
}

impl TreeTransformer {
    pub fn new(options: CompileOptions) -> Rc<Self> {
        Rc::new(Self {
            names: RefCell::new(VarScope::new()),
            options,
        })
    }

    fn compile_context(&self, pair: &Pair<Rule>) -> CompileContext {
        CompileContext::new(pair.line_col())
    }

    fn parse_atom(self: Rc<Self>, pair: Pair<Rule>) -> Result<Expr, CompileError> {
        match pair.as_rule() {
            Rule::ident => {
                let name = self
                    .names
                    .borrow_mut()
                    .find_or_add_name_global(pair.as_str().trim(), DeclType::Unknown)
                    .unwrap();
                Ok(Expr::Id(name))
            }
            Rule::type_constant => {
                let type_str = pair.as_str();
                let Some(type_id) = VarType::parse(type_str) else {
                    return Err(UnknownTypeConstant(
                        self.compile_context(&pair),
                        type_str.into(),
                    ));
                };

                Ok(Expr::TypeConstant(type_id))
            }
            Rule::object => {
                let ostr = &pair.as_str()[1..];
                let oid = i32::from_str(ostr).unwrap();
                let objid = Obj::mk_id(oid);
                Ok(Expr::Value(v_obj(objid)))
            }
            Rule::integer => match pair.as_str().parse::<i64>() {
                Ok(int) => Ok(Expr::Value(v_int(int))),
                Err(e) => Err(CompileError::StringLexError(
                    self.compile_context(&pair),
                    format!("invalid integer literal '{}': {e}", pair.as_str()),
                )),
            },
            Rule::boolean => {
                if !self.options.bool_type {
                    return Err(CompileError::DisabledFeature(
                        self.compile_context(&pair),
                        "Booleans".to_string(),
                    ));
                }
                let b = pair.as_str().trim() == "true";
                Ok(Expr::Value(Var::mk_bool(b)))
            }
            Rule::symbol => {
                if !self.options.symbol_type {
                    return Err(CompileError::DisabledFeature(
                        self.compile_context(&pair),
                        "Symbols".to_string(),
                    ));
                }
                let s = Symbol::mk(&pair.as_str()[1..]);
                Ok(Expr::Value(Var::mk_symbol(s)))
            }
            Rule::float => {
                let float = pair.as_str().parse::<f64>().unwrap();
                Ok(Expr::Value(v_float(float)))
            }
            Rule::string => {
                let string = pair.as_str();
                let parsed = match unquote_str(string) {
                    Ok(str) => str,
                    Err(e) => {
                        return Err(CompileError::StringLexError(
                            self.compile_context(&pair),
                            format!("invalid string literal '{}': {e}", string),
                        ));
                    }
                };
                Ok(Expr::Value(v_str(&parsed)))
            }
            Rule::literal_binary => {
                let binary_literal = pair.as_str();
                // Remove b" and " from the literal to get just the base64 content
                let base64_content = binary_literal
                    .strip_prefix("b\"")
                    .and_then(|s| s.strip_suffix("\""))
                    .ok_or_else(|| {
                        CompileError::StringLexError(
                            self.compile_context(&pair),
                            format!(
                                "invalid binary literal '{}': missing b\" prefix or \" suffix",
                                binary_literal
                            ),
                        )
                    })?;

                // Decode the base64 content
                let decoded = match general_purpose::URL_SAFE.decode(base64_content) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        return Err(CompileError::StringLexError(
                            self.compile_context(&pair),
                            format!(
                                "invalid binary literal '{}': invalid base64: {e}",
                                binary_literal
                            ),
                        ));
                    }
                };

                Ok(Expr::Value(v_binary(decoded)))
            }
            Rule::err => {
                let mut inner = pair.into_inner();
                let pair = inner.next().unwrap();
                let e = pair.as_str();
                let Some(e) = ErrorCode::parse_str(e) else {
                    panic!("invalid error value: {e}");
                };
                if let ErrorCode::ErrCustom(_) = &e {
                    if !self.options.custom_errors {
                        return Err(CompileError::DisabledFeature(
                            self.compile_context(&pair),
                            "CustomErrors".to_string(),
                        ));
                    }
                }
                let mut msg_part = None;
                if let Some(msg) = inner.next() {
                    msg_part = Some(Box::new(self.clone().parse_expr(msg.into_inner())?));
                }

                Ok(Expr::Error(e, msg_part))
            }
            _ => {
                panic!("Unimplemented atom: {:?}", pair);
            }
        }
    }

    fn parse_exprlist(self: Rc<Self>, pairs: Pairs<Rule>) -> Result<Vec<Arg>, CompileError> {
        let mut args = vec![];
        for pair in pairs {
            match pair.as_rule() {
                Rule::argument => {
                    let arg = if pair.as_str().starts_with('@') {
                        Splice(
                            self.clone()
                                .parse_expr(pair.into_inner().next().unwrap().into_inner())?,
                        )
                    } else {
                        Normal(
                            self.clone()
                                .parse_expr(pair.into_inner().next().unwrap().into_inner())?,
                        )
                    };
                    args.push(arg);
                }
                _ => {
                    panic!("Unimplemented exprlist: {:?}", pair);
                }
            }
        }
        Ok(args)
    }

    fn parse_arglist(self: Rc<Self>, pairs: Pairs<Rule>) -> Result<Vec<Arg>, CompileError> {
        let Some(first) = pairs.peek() else {
            return Ok(vec![]);
        };

        let Rule::exprlist = first.as_rule() else {
            panic!("Unimplemented arglist: {:?}", first);
        };

        self.parse_exprlist(first.into_inner())
    }

    fn parse_except_codes(self: Rc<Self>, pairs: Pair<Rule>) -> Result<CatchCodes, CompileError> {
        match pairs.as_rule() {
            Rule::anycode => Ok(CatchCodes::Any),
            Rule::exprlist => Ok(CatchCodes::Codes(self.parse_exprlist(pairs.into_inner())?)),
            _ => {
                panic!("Unimplemented except_codes: {:?}", pairs);
            }
        }
    }

    fn parse_expr(self: Rc<Self>, pairs: Pairs<Rule>) -> Result<Expr, CompileError> {
        let pratt = PrattParser::new()
            // Generally following C-like precedence order as described:
            //   https://en.cppreference.com/w/c/language/operator_precedence
            // Precedence from lowest to highest.
            // 14. Assignments & returns are lowest precedence.
            .op(Op::postfix(Rule::assign) | Op::prefix(Rule::scatter_assign))
            // 13. Ternary conditional
            .op(Op::postfix(Rule::cond_expr))
            // 12. Logical or.
            .op(Op::infix(Rule::lor, Assoc::Left))
            // 11. Logical and.
            .op(Op::infix(Rule::land, Assoc::Left))
            // TODO: bitwise operators here (| 10, ^ XOR 9, & 8) if we ever get them.
            // 7
            // Equality/inequality
            .op(Op::infix(Rule::eq, Assoc::Left) | Op::infix(Rule::neq, Assoc::Left))
            // 6. Relational operators
            .op(Op::infix(Rule::gt, Assoc::Left)
                | Op::infix(Rule::lt, Assoc::Left)
                | Op::infix(Rule::gte, Assoc::Left)
                | Op::infix(Rule::lte, Assoc::Left))
            // TODO 5 bitwise shiftleft/shiftright if we ever get them.
            // 5. In operator ended up above add
            .op(Op::infix(Rule::in_range, Assoc::Left))
            // 4. Add & subtract same precedence
            .op(Op::infix(Rule::add, Assoc::Left) | Op::infix(Rule::sub, Assoc::Left))
            // 3. * / % all same precedence
            .op(Op::infix(Rule::mul, Assoc::Left)
                | Op::infix(Rule::div, Assoc::Left)
                | Op::infix(Rule::modulus, Assoc::Left))
            // Exponent is higher than multiply/divide (not present in C)
            .op(Op::infix(Rule::pow, Assoc::Left))
            // 2. Unary negation & logical-not
            .op(Op::prefix(Rule::neg) | Op::prefix(Rule::not))
            // 1. Indexing/suffix operator generally.
            .op(Op::postfix(Rule::index_range)
                | Op::postfix(Rule::index_single)
                | Op::postfix(Rule::verb_call)
                | Op::postfix(Rule::verb_expr_call)
                | Op::postfix(Rule::prop)
                | Op::postfix(Rule::prop_expr));

        let primary_self = self.clone();
        let postfix_self = self.clone();

        pratt
            .map_primary(|primary| {
                match primary.as_rule() {
                    Rule::atom => {
                        let mut inner = primary.into_inner();
                        let expr = primary_self.clone().parse_atom(inner.next().unwrap())?;
                        Ok(expr)
                    }
                    Rule::sysprop => {
                        let mut inner = primary.into_inner();
                        let property = inner.next().unwrap().as_str();
                        Ok(Expr::Prop {
                            location: Box::new(Expr::Value(v_obj(SYSTEM_OBJECT))),
                            property: Box::new(Expr::Value(v_str(property))),
                        })
                    }
                    Rule::sysprop_call => {
                        let mut inner = primary.into_inner();
                        let verb = inner.next().unwrap().as_str()[1..].to_string();
                        let args = primary_self
                            .clone()
                            .parse_arglist(inner.next().unwrap().into_inner())?;
                        Ok(Expr::Verb {
                            location: Box::new(Expr::Value(v_obj(SYSTEM_OBJECT))),
                            verb: Box::new(Expr::Value(v_string(verb))),
                            args,
                        })
                    }
                    Rule::list => {
                        let mut inner = primary.into_inner();
                        if let Some(arglist) = inner.next() {
                            let args = primary_self.clone().parse_exprlist(arglist.into_inner())?;
                            Ok(Expr::List(args))
                        } else {
                            Ok(Expr::List(vec![]))
                        }
                    }
                    Rule::map => {
                        if !self.options.map_type {
                            return Err(CompileError::DisabledFeature(
                                self.compile_context(&primary),
                                "Maps".to_string(),
                            ));
                        }

                        let inner = primary.into_inner();
                        // Parse each key, value as a separate expression which we will pair-up later.
                        let mut elements = vec![];
                        for r in inner {
                            elements.push(primary_self.clone().parse_expr(r.into_inner()).unwrap());
                        }
                        let pairs = elements
                            .chunks(2)
                            .map(|pair| {
                                let key = pair[0].clone();
                                let value = pair[1].clone();
                                (key, value)
                            })
                            .collect();
                        Ok(Expr::Map(pairs))
                    }
                    Rule::flyweight => {
                        if !self.options.flyweight_type {
                            return Err(CompileError::DisabledFeature(
                                self.compile_context(&primary),
                                "Maps".to_string(),
                            ));
                        }
                        let mut parts = primary.into_inner();

                        // Three components:
                        // 1. The delegate object
                        // 2. The slots
                        // 3. The contents
                        let delegate = primary_self
                            .clone()
                            .parse_expr(parts.next().unwrap().into_inner())?;

                        let mut slots = vec![];
                        let mut contents = None;

                        // Parse the remaining parts: optional slots, optional contents
                        for next in parts {
                            match next.as_rule() {
                                Rule::flyweight_slots => {
                                    // Parse the slots, they're a sequence of ident, expr pairs.
                                    // Collect them into two iterators,
                                    let slot_pairs = next.clone().into_inner().chunks(2);
                                    for mut pair in &slot_pairs {
                                        let slot_name = Symbol::mk(pair.next().unwrap().as_str());

                                        // "delegate" and "slots" are forbidden slot names.
                                        if slot_name == Symbol::mk("delegate")
                                            || slot_name == Symbol::mk("slots")
                                        {
                                            return Err(CompileError::BadSlotName(
                                                self.compile_context(&next),
                                                slot_name.to_string(),
                                            ));
                                        }

                                        let slot_expr = primary_self
                                            .clone()
                                            .parse_expr(pair.next().unwrap().into_inner())?;
                                        slots.push((slot_name, slot_expr));
                                    }
                                }
                                Rule::expr => {
                                    // This is the contents expression
                                    let expr =
                                        primary_self.clone().parse_expr(next.into_inner())?;
                                    contents = Some(Box::new(expr));
                                }
                                _ => {
                                    panic!("Unexpected rule: {:?}", next.as_rule());
                                }
                            };
                        }
                        Ok(Expr::Flyweight(Box::new(delegate), slots, contents))
                    }
                    Rule::builtin_call => {
                        let mut inner = primary.into_inner();
                        let bf = inner.next().unwrap().as_str();
                        let args = primary_self
                            .clone()
                            .parse_arglist(inner.next().unwrap().into_inner())?;
                        Ok(Expr::Call {
                            function: Symbol::mk(bf),
                            args,
                        })
                    }
                    Rule::pass_expr => {
                        let mut inner = primary.into_inner();
                        let args = if let Some(arglist) = inner.next() {
                            primary_self.clone().parse_exprlist(arglist.into_inner())?
                        } else {
                            vec![]
                        };
                        Ok(Expr::Pass { args })
                    }
                    Rule::range_end => Ok(Expr::Length),
                    Rule::try_expr => {
                        let mut inner = primary.into_inner();
                        let try_expr = primary_self
                            .clone()
                            .parse_expr(inner.next().unwrap().into_inner())?;
                        let codes = inner.next().unwrap();
                        let catch_codes = primary_self
                            .clone()
                            .parse_except_codes(codes.into_inner().next().unwrap())?;
                        let except = inner.next().map(|e| {
                            Box::new(primary_self.clone().parse_expr(e.into_inner()).unwrap())
                        });
                        Ok(Expr::TryCatch {
                            trye: Box::new(try_expr),
                            codes: catch_codes,
                            except,
                        })
                    }

                    Rule::paren_expr => {
                        let mut inner = primary.into_inner();
                        let expr = primary_self
                            .clone()
                            .parse_expr(inner.next().unwrap().into_inner())?;
                        Ok(expr)
                    }
                    Rule::integer => match primary.as_str().parse::<i64>() {
                        Ok(int) => Ok(Expr::Value(v_int(int))),
                        Err(e) => Err(CompileError::StringLexError(
                            self.compile_context(&primary),
                            format!("invalid integer literal '{}': {e}", primary.as_str()),
                        )),
                    },
                    Rule::range_comprehension => {
                        if !self.options.list_comprehensions {
                            return Err(CompileError::DisabledFeature(
                                self.compile_context(&primary),
                                "ListComprehension".to_string(),
                            ));
                        }
                        let mut inner = primary.into_inner();

                        let producer_portion = inner.next().unwrap().into_inner();

                        let variable_ident = inner.next().unwrap();
                        let varname = variable_ident.as_str().trim();
                        let variable = match variable_ident.as_rule() {
                            Rule::ident => {
                                let mut names = self.names.borrow_mut();
                                let Some(name) = names.declare_name(varname, DeclType::For) else {
                                    return Err(DuplicateVariable(
                                        self.compile_context(&variable_ident),
                                        varname.into(),
                                    ));
                                };
                                name
                            }
                            _ => {
                                panic!("Unexpected rule: {:?}", variable_ident.as_rule());
                            }
                        };
                        let producer_expr = primary_self.clone().parse_expr(producer_portion)?;
                        let clause = inner.next().unwrap();

                        match clause.as_rule() {
                            Rule::for_range_clause => {
                                self.enter_scope();
                                let mut clause_inner = clause.into_inner();
                                let from_rule = clause_inner.next().unwrap();
                                let to_rule = clause_inner.next().unwrap();
                                let from = self.clone().parse_expr(from_rule.into_inner())?;
                                let to = self.clone().parse_expr(to_rule.into_inner())?;
                                let end_of_range_register =
                                    self.names.borrow_mut().declare_register()?;
                                self.exit_scope();
                                Ok(Expr::ComprehendRange {
                                    variable,
                                    end_of_range_register,
                                    producer_expr: Box::new(producer_expr),
                                    from: Box::new(from),
                                    to: Box::new(to),
                                })
                            }
                            Rule::for_in_clause => {
                                self.enter_scope();

                                let mut clause_inner = clause.into_inner();
                                let in_rule = clause_inner.next().unwrap();
                                let expr = self.clone().parse_expr(in_rule.into_inner())?;
                                let position_register =
                                    self.names.borrow_mut().declare_register()?;
                                let list_register = self.names.borrow_mut().declare_register()?;
                                self.exit_scope();
                                Ok(Expr::ComprehendList {
                                    list_register,
                                    variable,
                                    position_register,
                                    producer_expr: Box::new(producer_expr),
                                    list: Box::new(expr),
                                })
                            }
                            _ => {
                                todo!("unhandled rule: {:?}", clause.as_rule())
                            }
                        }
                    }
                    Rule::return_expr => {
                        let mut inner = primary.into_inner();
                        let rhs = match inner.next() {
                            Some(e) => Some(Box::new(self.clone().parse_expr(e.into_inner())?)),
                            None => None,
                        };
                        Ok(Expr::Return(rhs))
                    }

                    _ => todo!("Unimplemented primary: {:?}", primary.as_rule()),
                }
            })
            .map_infix(|lhs, op, rhs| match op.as_rule() {
                Rule::add => Ok(Expr::Binary(
                    BinaryOp::Add,
                    Box::new(lhs?),
                    Box::new(rhs.unwrap()),
                )),
                Rule::sub => Ok(Expr::Binary(
                    BinaryOp::Sub,
                    Box::new(lhs?),
                    Box::new(rhs.unwrap()),
                )),
                Rule::mul => Ok(Expr::Binary(
                    BinaryOp::Mul,
                    Box::new(lhs?),
                    Box::new(rhs.unwrap()),
                )),
                Rule::div => Ok(Expr::Binary(
                    BinaryOp::Div,
                    Box::new(lhs?),
                    Box::new(rhs.unwrap()),
                )),
                Rule::pow => Ok(Expr::Binary(
                    BinaryOp::Exp,
                    Box::new(lhs?),
                    Box::new(rhs.unwrap()),
                )),
                Rule::modulus => Ok(Expr::Binary(
                    BinaryOp::Mod,
                    Box::new(lhs?),
                    Box::new(rhs.unwrap()),
                )),
                Rule::eq => Ok(Expr::Binary(
                    BinaryOp::Eq,
                    Box::new(lhs?),
                    Box::new(rhs.unwrap()),
                )),
                Rule::neq => Ok(Expr::Binary(
                    BinaryOp::NEq,
                    Box::new(lhs?),
                    Box::new(rhs.unwrap()),
                )),
                Rule::lt => Ok(Expr::Binary(
                    BinaryOp::Lt,
                    Box::new(lhs?),
                    Box::new(rhs.unwrap()),
                )),
                Rule::lte => Ok(Expr::Binary(
                    BinaryOp::LtE,
                    Box::new(lhs?),
                    Box::new(rhs.unwrap()),
                )),
                Rule::gt => Ok(Expr::Binary(
                    BinaryOp::Gt,
                    Box::new(lhs?),
                    Box::new(rhs.unwrap()),
                )),
                Rule::gte => Ok(Expr::Binary(
                    BinaryOp::GtE,
                    Box::new(lhs?),
                    Box::new(rhs.unwrap()),
                )),

                Rule::land => Ok(Expr::And(Box::new(lhs?), Box::new(rhs.unwrap()))),
                Rule::lor => Ok(Expr::Or(Box::new(lhs?), Box::new(rhs.unwrap()))),
                Rule::in_range => Ok(Expr::Binary(
                    BinaryOp::In,
                    Box::new(lhs?),
                    Box::new(rhs.unwrap()),
                )),
                _ => todo!("Unimplemented infix: {:?}", op.as_rule()),
            })
            .map_prefix(|op, rhs| match op.as_rule() {
                Rule::scatter_assign => {
                    let rhs = rhs?;

                    self.clone().parse_scatter_assign(op, rhs, false, false)
                }
                Rule::not => Ok(Expr::Unary(UnaryOp::Not, Box::new(rhs?))),
                Rule::neg => Ok(Expr::Unary(UnaryOp::Neg, Box::new(rhs?))),
                _ => todo!("Unimplemented prefix: {:?}", op.as_rule()),
            })
            .map_postfix(|lhs, op| {
                match op.as_rule() {
                    Rule::verb_call => {
                        let mut parts = op.into_inner();
                        let ident = parts.next().unwrap().as_str();
                        let args_expr = parts.next().unwrap();
                        let args = postfix_self.clone().parse_arglist(args_expr.into_inner())?;
                        Ok(Expr::Verb {
                            location: Box::new(lhs?),
                            verb: Box::new(Expr::Value(v_str(ident))),
                            args,
                        })
                    }
                    Rule::verb_expr_call => {
                        let mut parts = op.into_inner();
                        let expr = postfix_self
                            .clone()
                            .parse_expr(parts.next().unwrap().into_inner())?;
                        let args_expr = parts.next().unwrap();
                        let args = postfix_self.clone().parse_arglist(args_expr.into_inner())?;
                        Ok(Expr::Verb {
                            location: Box::new(lhs?),
                            verb: Box::new(expr),
                            args,
                        })
                    }
                    Rule::prop => {
                        let mut parts = op.into_inner();
                        let ident = parts.next().unwrap().as_str();
                        Ok(Expr::Prop {
                            location: Box::new(lhs?),
                            property: Box::new(Expr::Value(v_str(ident))),
                        })
                    }
                    Rule::prop_expr => {
                        let mut parts = op.into_inner();
                        let expr = postfix_self
                            .clone()
                            .parse_expr(parts.next().unwrap().into_inner())?;
                        Ok(Expr::Prop {
                            location: Box::new(lhs?),
                            property: Box::new(expr),
                        })
                    }
                    Rule::assign => {
                        let mut parts = op.clone().into_inner();
                        let right = postfix_self
                            .clone()
                            .parse_expr(parts.next().unwrap().into_inner())?;
                        let lhs = lhs?;
                        if let Expr::Id(name) = &lhs {
                            let mut names = self.names.borrow_mut();
                            let decl = names.decl_for_mut(name);

                            // If the variable referenced was not introduced by a let/const clause,
                            // and doesn't have a prior declaration, mark this as its declaration.
                            if decl.decl_type == DeclType::Unknown {
                                decl.decl_type = DeclType::Assign;
                            }

                            // If the variable referenced in the LHS is const, we can't assign to it.
                            // TODO this likely needs to recurse down this tree.
                            if decl.constant {
                                return Err(CompileError::AssignToConst(
                                    self.compile_context(&op),
                                    decl.identifier.to_symbol(),
                                ));
                            }
                        }
                        Ok(Expr::Assign {
                            left: Box::new(lhs),
                            right: Box::new(right),
                        })
                    }
                    Rule::index_single => {
                        let mut parts = op.into_inner();
                        let index = postfix_self
                            .clone()
                            .parse_expr(parts.next().unwrap().into_inner())?;
                        Ok(Expr::Index(Box::new(lhs?), Box::new(index)))
                    }
                    Rule::index_range => {
                        let mut parts = op.into_inner();
                        let start = postfix_self
                            .clone()
                            .parse_expr(parts.next().unwrap().into_inner())?;
                        let end = postfix_self
                            .clone()
                            .parse_expr(parts.next().unwrap().into_inner())?;
                        Ok(Expr::Range {
                            base: Box::new(lhs?),
                            from: Box::new(start),
                            to: Box::new(end),
                        })
                    }
                    Rule::cond_expr => {
                        let mut parts = op.into_inner();
                        let true_expr = postfix_self
                            .clone()
                            .parse_expr(parts.next().unwrap().into_inner())?;
                        let false_expr = postfix_self
                            .clone()
                            .parse_expr(parts.next().unwrap().into_inner())?;
                        Ok(Expr::Cond {
                            condition: Box::new(lhs?),
                            consequence: Box::new(true_expr),
                            alternative: Box::new(false_expr),
                        })
                    }
                    _ => todo!("Unimplemented postfix: {:?}", op.as_rule()),
                }
            })
            .parse(pairs)
    }

    fn parse_statement(self: Rc<Self>, pair: Pair<Rule>) -> Result<Option<Stmt>, CompileError> {
        let line_col = pair.line_col();
        let context = self.compile_context(&pair);
        match pair.as_rule() {
            Rule::expr_statement => {
                let mut inner = pair.into_inner();
                if let Some(rule) = inner.next() {
                    let expr = self.parse_expr(rule.into_inner())?;
                    return Ok(Some(Stmt::new(StmtNode::Expr(expr), line_col)));
                }
                Ok(None)
            }
            Rule::while_statement => {
                let mut parts = pair.into_inner();
                let condition = self
                    .clone()
                    .parse_expr(parts.next().unwrap().into_inner())?;
                self.enter_scope();
                let body = self
                    .clone()
                    .parse_statements(parts.next().unwrap().into_inner())?;
                let environment_width = self.exit_scope();
                Ok(Some(Stmt::new(
                    StmtNode::While {
                        id: None,
                        condition,
                        body,
                        environment_width,
                    },
                    line_col,
                )))
            }
            Rule::labelled_while_statement => {
                let mut parts = pair.into_inner();
                let varname = parts.next().unwrap().as_str();
                let Some(id) = self
                    .names
                    .borrow_mut()
                    .declare_name(varname, DeclType::WhileLabel)
                else {
                    return Err(DuplicateVariable(context, varname.into()));
                };
                let condition = self
                    .clone()
                    .parse_expr(parts.next().unwrap().into_inner())?;
                self.enter_scope();
                let body = self
                    .clone()
                    .parse_statements(parts.next().unwrap().into_inner())?;
                let environment_width = self.exit_scope();
                Ok(Some(Stmt::new(
                    StmtNode::While {
                        id: Some(id),
                        condition,
                        body,
                        environment_width,
                    },
                    line_col,
                )))
            }
            Rule::if_statement => {
                let mut parts = pair.into_inner();
                let mut arms = vec![];
                let mut otherwise = None;
                let condition = self
                    .clone()
                    .parse_expr(parts.next().unwrap().into_inner())?;
                self.enter_scope();
                let body = self
                    .clone()
                    .parse_statements(parts.next().unwrap().into_inner())?;
                let environment_width = { self.exit_scope() };
                arms.push(CondArm {
                    condition,
                    statements: body,
                    environment_width,
                });
                for remainder in parts {
                    match remainder.as_rule() {
                        Rule::endif_clause => {
                            continue;
                        }
                        Rule::elseif_clause => {
                            self.enter_scope();
                            let mut parts = remainder.into_inner();
                            let condition = self
                                .clone()
                                .parse_expr(parts.next().unwrap().into_inner())?;
                            let body = self
                                .clone()
                                .parse_statements(parts.next().unwrap().into_inner())?;
                            let environment_width = self.exit_scope();
                            arms.push(CondArm {
                                condition,
                                statements: body,
                                environment_width,
                            });
                        }
                        Rule::else_clause => {
                            self.enter_scope();
                            let mut parts = remainder.into_inner();
                            let otherwise_statements = self
                                .clone()
                                .parse_statements(parts.next().unwrap().into_inner())?;
                            let otherwise_environment_width = self.exit_scope();
                            let otherwise_arm = ElseArm {
                                statements: otherwise_statements,
                                environment_width: otherwise_environment_width,
                            };
                            otherwise = Some(otherwise_arm);
                        }
                        _ => panic!("Unimplemented if clause: {:?}", remainder),
                    }
                }
                Ok(Some(Stmt::new(
                    StmtNode::Cond { arms, otherwise },
                    line_col,
                )))
            }
            Rule::break_statement => {
                let mut parts = pair.clone().into_inner();
                let label = match parts.next() {
                    None => None,
                    Some(s) => {
                        let label = s.as_str();
                        let Some(label) = self.names.borrow_mut().find_name(label) else {
                            return Err(CompileError::UnknownLoopLabel(
                                self.compile_context(&pair),
                                label.to_string(),
                            ));
                        };
                        Some(label)
                    }
                };
                Ok(Some(Stmt::new(StmtNode::Break { exit: label }, line_col)))
            }
            Rule::continue_statement => {
                let mut parts = pair.clone().into_inner();
                let label = match parts.next() {
                    None => None,
                    Some(s) => {
                        let label = s.as_str();
                        let Some(label) = self.names.borrow_mut().find_name(label) else {
                            return Err(CompileError::UnknownLoopLabel(
                                self.compile_context(&pair),
                                label.to_string(),
                            ));
                        };
                        Some(label)
                    }
                };
                Ok(Some(Stmt::new(
                    StmtNode::Continue { exit: label },
                    line_col,
                )))
            }
            Rule::for_range_statement => {
                let mut parts = pair.into_inner();

                let varname = parts.next().unwrap().as_str();
                let value_binding = {
                    let mut names = self.names.borrow_mut();

                    names.declare_or_use_name(varname, DeclType::For)
                };
                let clause = parts.next().unwrap();
                self.enter_scope();
                let body = self
                    .clone()
                    .parse_statements(parts.next().unwrap().into_inner())?;
                match clause.as_rule() {
                    Rule::for_range_clause => {
                        let mut clause_inner = clause.into_inner();
                        let from_rule = clause_inner.next().unwrap();
                        let to_rule = clause_inner.next().unwrap();
                        let from = self.clone().parse_expr(from_rule.into_inner())?;
                        let to = self.clone().parse_expr(to_rule.into_inner())?;
                        let environment_width = self.exit_scope();
                        Ok(Some(Stmt::new(
                            StmtNode::ForRange {
                                id: value_binding,
                                from,
                                to,
                                body,
                                environment_width,
                            },
                            line_col,
                        )))
                    }
                    _ => panic!("Unimplemented for clause: {:?}", clause),
                }
            }
            Rule::for_in_statement => {
                let mut parts = pair.into_inner();

                let mut index_clause = parts.next().unwrap().into_inner();
                // index_clause can have 1 or 2 elements, if there's 2 then there's a "key" as well as a "value" var
                let first_index_rule = index_clause.next().unwrap();
                let varname = first_index_rule.as_str();
                let value_binding = {
                    let mut names = self.names.borrow_mut();

                    names.declare_or_use_name(varname, DeclType::For)
                };
                let key_binding = match index_clause.next() {
                    None => None,
                    Some(s) => {
                        let varname = s.as_str();
                        let key_var = self
                            .names
                            .borrow_mut()
                            .declare_or_use_name(varname, DeclType::For);
                        Some(key_var)
                    }
                };
                self.enter_scope();

                let clause = parts.next().unwrap();
                let body = self
                    .clone()
                    .parse_statements(parts.next().unwrap().into_inner())?;
                match clause.as_rule() {
                    Rule::for_in_clause => {
                        let mut clause_inner = clause.into_inner();
                        let in_rule = clause_inner.next().unwrap();
                        let expr = self.clone().parse_expr(in_rule.into_inner())?;
                        let environment_width = self.exit_scope();
                        Ok(Some(Stmt::new(
                            StmtNode::ForList {
                                value_binding,
                                key_binding,
                                expr,
                                body,
                                environment_width,
                            },
                            line_col,
                        )))
                    }
                    _ => panic!("Unimplemented for clause: {:?}", clause),
                }
            }
            Rule::try_finally_statement => {
                self.enter_scope();
                let mut parts = pair.into_inner();
                let body = self
                    .clone()
                    .parse_statements(parts.next().unwrap().into_inner())?;
                let handler = self
                    .clone()
                    .parse_statements(parts.next().unwrap().into_inner())?;
                let environment_width = self.exit_scope();
                Ok(Some(Stmt::new(
                    StmtNode::TryFinally {
                        body,
                        handler,
                        environment_width,
                    },
                    line_col,
                )))
            }
            Rule::try_except_statement => {
                self.enter_scope();
                let mut parts = pair.into_inner();
                let body = self
                    .clone()
                    .parse_statements(parts.next().unwrap().into_inner())?;
                let mut excepts = vec![];
                let environment_width = self.exit_scope();

                for except in parts {
                    match except.as_rule() {
                        Rule::except => {
                            let mut except_clause_parts = except.into_inner();
                            let clause = except_clause_parts.next().unwrap();
                            let (id, codes) = match clause.as_rule() {
                                Rule::labelled_except => {
                                    let mut my_parts = clause.into_inner();
                                    let exception = my_parts.next().map(|id| {
                                        let mut names = self.names.borrow_mut();
                                        names.declare_or_use_name(id.as_str(), DeclType::Except)
                                    });

                                    let codes = self.clone().parse_except_codes(
                                        my_parts.next().unwrap().into_inner().next().unwrap(),
                                    )?;
                                    (exception, codes)
                                }
                                Rule::unlabelled_except => {
                                    let mut my_parts = clause.into_inner();
                                    let codes = self.clone().parse_except_codes(
                                        my_parts.next().unwrap().into_inner().next().unwrap(),
                                    )?;
                                    (None, codes)
                                }
                                _ => panic!("Unimplemented except clause: {:?}", clause),
                            };
                            let statements = self.clone().parse_statements(
                                except_clause_parts.next().unwrap().into_inner(),
                            )?;
                            excepts.push(ExceptArm {
                                id,
                                codes,
                                statements,
                            });
                        }
                        _ => panic!("Unimplemented except clause: {:?}", except),
                    }
                }

                Ok(Some(Stmt::new(
                    StmtNode::TryExcept {
                        body,
                        excepts,
                        environment_width,
                    },
                    line_col,
                )))
            }
            Rule::fork_statement => {
                let mut parts = pair.into_inner();
                let time = self
                    .clone()
                    .parse_expr(parts.next().unwrap().into_inner())?;
                let body = self.parse_statements(parts.next().unwrap().into_inner())?;
                Ok(Some(Stmt::new(
                    StmtNode::Fork {
                        id: None,
                        time,
                        body,
                    },
                    line_col,
                )))
            }
            Rule::labelled_fork_statement => {
                let mut parts = pair.into_inner();
                let varname = parts.next().unwrap().as_str();
                let Some(id) = self
                    .names
                    .borrow_mut()
                    .find_or_add_name_global(varname, DeclType::ForkLabel)
                else {
                    return Err(DuplicateVariable(context, varname.into()));
                };
                let time = self
                    .clone()
                    .parse_expr(parts.next().unwrap().into_inner())?;
                let body = self.parse_statements(parts.next().unwrap().into_inner())?;
                Ok(Some(Stmt::new(
                    StmtNode::Fork {
                        id: Some(id),
                        time,
                        body,
                    },
                    line_col,
                )))
            }
            Rule::begin_statement => {
                if !self.options.lexical_scopes {
                    return Err(CompileError::DisabledFeature(
                        self.compile_context(&pair),
                        "lexical_scopes".to_string(),
                    ));
                }
                let mut parts = pair.into_inner();

                self.enter_scope();

                let body = self
                    .clone()
                    .parse_statements(parts.next().unwrap().into_inner())?;
                let num_total_bindings = self.exit_scope();
                Ok(Some(Stmt::new(
                    Scope {
                        num_bindings: num_total_bindings,
                        body,
                    },
                    line_col,
                )))
            }
            Rule::local_assignment | Rule::const_assignment => {
                if !self.options.lexical_scopes {
                    return Err(CompileError::DisabledFeature(
                        self.compile_context(&pair),
                        "lexical_scopes".to_string(),
                    ));
                }

                // Scatter, or local, we'll then go match on that...
                let parts = pair.into_inner().next().unwrap();
                match parts.as_rule() {
                    Rule::local_assign_single | Rule::const_assign_single => Ok(Some(Stmt::new(
                        self.clone().parse_decl_assign(parts)?,
                        line_col,
                    ))),
                    Rule::local_assign_scatter | Rule::const_assign_scatter => {
                        let is_const = parts.as_rule() == Rule::const_assign_scatter;
                        let mut parts = parts.into_inner();
                        let op = parts.next().unwrap();
                        let rhs = parts.next().unwrap();
                        let rhs = self.clone().parse_expr(rhs.into_inner())?;
                        let expr = self.parse_scatter_assign(op, rhs, true, is_const)?;
                        Ok(Some(Stmt::new(StmtNode::Expr(expr), line_col)))
                    }
                    _ => {
                        unimplemented!("Unimplemented assignment: {:?}", parts.as_rule())
                    }
                }
            }

            Rule::global_assignment => {
                if !self.options.lexical_scopes {
                    return Err(CompileError::DisabledFeature(
                        self.compile_context(&pair),
                        "lexical_scopes".to_string(),
                    ));
                }

                // An explicit global-declaration.
                // global x, or global x = y
                let mut parts = pair.into_inner();
                let varname = parts.next().unwrap().as_str();
                let id = {
                    let mut names = self.names.borrow_mut();
                    let Some(id) = names.find_or_add_name_global(varname, DeclType::Global) else {
                        return Err(DuplicateVariable(context, varname.into()));
                    };
                    names.decl_for_mut(&id).decl_type = DeclType::Global;
                    id
                };
                let expr = parts
                    .next()
                    .map(|e| self.parse_expr(e.into_inner()).unwrap());

                // Produces an assignment expression as usual, but
                // the decompiler will need to look and see that
                //      a) the statement is just an assignment on its own
                //      b) the variable being assigned to is in scope 0 (global)
                // and then it's a global declaration.
                // Note that this well have the effect of turning most existing MOO decompilations
                // into global declarations, which is fine, if that feature is turned on.
                Ok(Some(Stmt::new(
                    StmtNode::Expr(Expr::Assign {
                        left: Box::new(Expr::Id(id)),
                        right: Box::new(expr.unwrap_or(Expr::Value(v_none()))),
                    }),
                    line_col,
                )))
            }
            Rule::empty_return => Ok(Some(Stmt::new(StmtNode::mk_return_none(), line_col))),
            _ => panic!("Unimplemented statement: {:?}", pair.as_rule()),
        }
    }

    fn parse_statements(self: Rc<Self>, pairs: Pairs<Rule>) -> Result<Vec<Stmt>, CompileError> {
        let mut statements = vec![];
        for pair in pairs {
            match pair.as_rule() {
                Rule::statement => {
                    let stmt = self
                        .clone()
                        .parse_statement(pair.into_inner().next().unwrap())?;
                    if let Some(stmt) = stmt {
                        statements.push(stmt);
                    }
                }
                _ => {
                    panic!("Unexpected rule: {:?}", pair.as_rule());
                }
            }
        }
        Ok(statements)
    }

    fn parse_decl_assign(self: Rc<Self>, pair: Pair<Rule>) -> Result<StmtNode, CompileError> {
        let context = self.compile_context(&pair);
        let is_const = pair.as_rule() == Rule::const_assign_single;

        // An assignment declaration that introduces a locally lexically scoped variable.
        // May be of form `let x = expr` or just `let x`
        let mut parts = pair.into_inner();

        let varname = parts.next().unwrap().as_str();
        let id = {
            let mut names = self.names.borrow_mut();
            let Some(id) = names.declare(varname, is_const, false, DeclType::Let) else {
                return Err(DuplicateVariable(context, varname.into()));
            };
            id
        };
        let expr = parts
            .next()
            .map(|e| self.parse_expr(e.into_inner()).unwrap());

        // Just becomes an assignment expression.
        // But that means the decompiler will need to know what to do with it.
        // Which is: if assignment is on its own in statement, and variable assigned to is
        //   restricted to the scope of the block, then it's a let.
        Ok(StmtNode::Expr(Expr::Assign {
            left: Box::new(Expr::Id(id)),
            right: Box::new(expr.unwrap_or(Expr::Value(v_none()))),
        }))
    }

    fn parse_scatter_assign(
        self: Rc<Self>,
        op: Pair<Rule>,
        rhs: Expr,
        local_scope: bool,
        is_const: bool,
    ) -> Result<Expr, CompileError> {
        let context = self.compile_context(&op);
        let inner = op.into_inner();
        let mut items = vec![];
        for scatter_item in inner {
            match scatter_item.as_rule() {
                Rule::scatter_optional => {
                    let mut inner = scatter_item.into_inner();
                    let id = inner.next().unwrap().as_str();
                    let Some(id) = self.clone().names.borrow_mut().declare(
                        id,
                        is_const,
                        !local_scope,
                        DeclType::Assign,
                    ) else {
                        return Err(DuplicateVariable(context, id.into()));
                    };

                    let expr = inner
                        .next()
                        .map(|e| self.clone().parse_expr(e.into_inner()).unwrap());
                    items.push(ScatterItem {
                        kind: ScatterKind::Optional,
                        id,
                        expr,
                    });
                }
                Rule::scatter_target => {
                    let mut inner = scatter_item.into_inner();
                    let id = inner.next().unwrap().as_str();
                    let Some(id) = self.clone().names.borrow_mut().declare(
                        id,
                        is_const,
                        !local_scope,
                        DeclType::Assign,
                    ) else {
                        return Err(DuplicateVariable(context, id.into()));
                    };
                    items.push(ScatterItem {
                        kind: ScatterKind::Required,
                        id,
                        expr: None,
                    });
                }
                Rule::scatter_rest => {
                    let mut inner = scatter_item.into_inner();
                    let id = inner.next().unwrap().as_str();
                    let Some(id) = self.clone().names.borrow_mut().declare(
                        id,
                        is_const,
                        !local_scope,
                        DeclType::Assign,
                    ) else {
                        return Err(DuplicateVariable(context, id.into()));
                    };
                    items.push(ScatterItem {
                        kind: ScatterKind::Rest,
                        id,
                        expr: None,
                    });
                }
                _ => {
                    panic!("Unimplemented scatter_item: {:?}", scatter_item);
                }
            }
        }
        Ok(Expr::Scatter(items, Box::new(rhs)))
    }

    fn transform_tree(self: Rc<Self>, pairs: Pairs<Rule>) -> Result<Parse, CompileError> {
        let mut program = Vec::new();
        for pair in pairs {
            match pair.as_rule() {
                Rule::program => {
                    let inna = pair.into_inner().next().unwrap();

                    match inna.as_rule() {
                        Rule::statements => {
                            let parsed_statements =
                                self.clone().parse_statements(inna.into_inner())?;
                            program.extend(parsed_statements);
                        }

                        _ => {
                            panic!("Unexpected rule: {:?}", inna.as_rule());
                        }
                    }
                }
                _ => {
                    panic!("Unexpected rule: {:?}", pair.as_rule());
                }
            }
        }

        self.do_transform(program)
    }

    fn transform_statements(self: Rc<Self>, pairs: Pairs<Rule>) -> Result<Parse, CompileError> {
        let mut program = Vec::new();
        for pair in pairs {
            match pair.as_rule() {
                Rule::statements => {
                    let statements = pair.into_inner();
                    let parsed_statements = self.clone().parse_statements(statements)?;
                    program.extend(parsed_statements);
                }

                _ => {
                    panic!("Unexpected rule: {:?}", pair.as_rule());
                }
            }
        }

        self.do_transform(program)
    }

    fn do_transform(self: Rc<Self>, mut program: Vec<Stmt>) -> Result<Parse, CompileError> {
        let unbound_names = self.names.borrow_mut();
        // Annotate the "true" line numbers of the AST nodes.
        annotate_line_numbers(1, &mut program);
        let names = unbound_names.bind();
        Ok(Parse {
            stmts: program,
            variables: unbound_names.clone(),
            names,
        })
    }

    fn enter_scope(&self) {
        if self.options.lexical_scopes {
            self.names.borrow_mut().enter_new_scope();
        }
    }

    fn exit_scope(&self) -> usize {
        if self.options.lexical_scopes {
            return self.names.borrow_mut().exit_scope();
        }
        0
    }
}

/// The emitted parse tree from the parse phase of the compiler.
#[derive(Debug)]
pub struct Parse {
    pub stmts: Vec<Stmt>,
    pub variables: VarScope,
    pub names: Names,
}

pub fn parse_program(program_text: &str, options: CompileOptions) -> Result<Parse, CompileError> {
    let pairs = match MooParser::parse(Rule::program, program_text) {
        Ok(pairs) => pairs,
        Err(e) => {
            let ((line, column), end_line_col) = match e.line_col {
                LineColLocation::Pos(lc) => (lc, None),
                LineColLocation::Span(begin, end) => (begin, Some(end)),
            };

            let context = CompileContext::new((line, column));
            return Err(CompileError::ParseError {
                error_position: context,
                end_line_col,
                context: e.line().to_string(),
                message: e.variant.message().to_string(),
            });
        }
    };

    // TODO: this is in Rc because of borrowing issues in the Pratt parser
    let tree_transform = TreeTransformer::new(options);
    tree_transform.transform_tree(pairs)
}

pub fn parse_tree(pairs: Pairs<Rule>, options: CompileOptions) -> Result<Parse, CompileError> {
    let tree_transform = TreeTransformer::new(options);
    tree_transform.transform_statements(pairs)
}

// Lex a simple MOO string literal.  Expectation is:
//   " and " at beginning and end
//   \" is "
//   \\ is \
//   \n is just n
// That's it. MOO has no tabs, newlines, etc. quoting.
pub fn unquote_str(s: &str) -> Result<String, String> {
    let mut output = String::new();
    let mut chars = s.chars().peekable();
    let Some('"') = chars.next() else {
        return Err("Expected \" at beginning of string".to_string());
    };
    // Proceed until second-last. Last has to be '"'
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some('\\') => output.push('\\'),
                Some('"') => output.push('"'),
                Some(c) => output.push(c),
                None => {
                    return Err("Unexpected end of string".to_string());
                }
            },
            '"' => {
                if chars.peek().is_some() {
                    return Err("Unexpected \" in string".to_string());
                }
                return Ok(output);
            }
            c => output.push(c),
        }
    }
    Err("Unexpected end of string".to_string())
}

#[cfg(test)]
mod tests {
    use moor_var::{E_INVARG, E_PROPNF, E_VARNF, ErrCustom, Symbol, v_none};
    use moor_var::{Var, v_binary, v_float, v_int, v_objid, v_str};

    use crate::CompileOptions;
    use crate::ast::Arg::{Normal, Splice};
    use crate::ast::BinaryOp::Add;
    use crate::ast::Expr::{Call, Error, Flyweight, Id, Prop, Value, Verb};
    use crate::ast::{
        BinaryOp, CatchCodes, CondArm, ElseArm, ExceptArm, Expr, ScatterItem, ScatterKind, Stmt,
        StmtNode, UnaryOp, assert_trees_match_recursive,
    };
    use crate::parse::{parse_program, unquote_str};
    use crate::unparse::annotate_line_numbers;
    use moor_common::model::CompileError;

    fn stripped_stmts(statements: &[Stmt]) -> Vec<StmtNode> {
        statements.iter().map(|s| s.node.clone()).collect()
    }

    #[test]
    fn test_string_unquote() {
        assert_eq!(unquote_str(r#""foo""#).unwrap(), "foo");
        assert_eq!(unquote_str(r#""foo\"bar""#).unwrap(), r#"foo"bar"#);
        assert_eq!(unquote_str(r#""foo\\bar""#).unwrap(), r"foo\bar");
        // Does not support \t, \n, etc.  They just turn into n, t, etc.
        assert_eq!(unquote_str(r#""foo\tbar""#).unwrap(), r#"footbar"#);
    }

    #[test]
    fn test_parse_flt_no_decimal() {
        let program = "return 1e-09;";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(parse.stmts.len(), 1);
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::Return(Some(Box::new(Value(
                v_float(1e-9)
            )))))]
        );
    }

    #[test]
    fn test_call_verb() {
        let program = r#"#0:test_verb(1,2,3,"test");"#;
        let parsed = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(parsed.stmts.len(), 1);
        assert_eq!(
            stripped_stmts(&parsed.stmts),
            vec![StmtNode::Expr(Verb {
                location: Box::new(Value(v_objid(0))),
                verb: Box::new(Value(v_str("test_verb"))),
                args: vec![
                    Normal(Value(v_int(1))),
                    Normal(Value(v_int(2))),
                    Normal(Value(v_int(3))),
                    Normal(Value(v_str("test")))
                ]
            })]
        );
    }

    #[test]
    fn test_parse_simple_var_assignment_precedence() {
        let program = "a = 1 + 2;";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        let a = parse.variables.find_name("a").unwrap();

        assert_eq!(parse.stmts.len(), 1);
        assert_eq!(
            stripped_stmts(&parse.stmts)[0],
            StmtNode::Expr(Expr::Assign {
                left: Box::new(Id(a)),
                right: Box::new(Expr::Binary(
                    Add,
                    Box::new(Value(v_int(1))),
                    Box::new(Value(v_int(2))),
                )),
            })
        );
    }

    #[test]
    fn test_parse_call_literal() {
        let program = "notify(\"test\");";
        let parse = parse_program(program, CompileOptions::default()).unwrap();

        assert_eq!(parse.stmts.len(), 1);
        assert_eq!(
            stripped_stmts(&parse.stmts)[0],
            StmtNode::Expr(Call {
                function: Symbol::mk("notify"),
                args: vec![Normal(Value(v_str("test")))],
            })
        );
    }

    fn assert_same_single(tree_a: &[Stmt], tree_b: StmtNode) {
        let mut tree_a = tree_a.to_vec();
        let mut tree_b = vec![Stmt::new(tree_b, (1, 1))];
        annotate_line_numbers(1, &mut tree_a);
        annotate_line_numbers(1, &mut tree_b);
        assert_trees_match_recursive(&tree_a, &tree_b);
    }

    fn assert_same(tree_a: &[Stmt], tree_b: &[StmtNode]) {
        let mut tree_a = tree_a.to_vec();
        let mut tree_b: Vec<_> = tree_b
            .iter()
            .map(|s| Stmt::new(s.clone(), (1, 1)))
            .collect();
        annotate_line_numbers(1, &mut tree_a);
        annotate_line_numbers(1, &mut tree_b);
        assert_trees_match_recursive(&tree_a, &tree_b);
    }

    #[test]
    fn test_parse_if_stmt() {
        let program = "if (1 == 2) return 5; elseif (2 == 3) return 3; else return 6; endif";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(parse.stmts.len(), 1);
        assert_same_single(
            &parse.stmts,
            StmtNode::Cond {
                arms: vec![
                    CondArm {
                        condition: Expr::Binary(
                            BinaryOp::Eq,
                            Box::new(Value(v_int(1))),
                            Box::new(Value(v_int(2))),
                        ),
                        statements: vec![Stmt {
                            node: StmtNode::mk_return(Value(v_int(5))),
                            line_col: (1, 0),
                            tree_line_no: 2,
                        }],
                        environment_width: 0,
                    },
                    CondArm {
                        environment_width: 0,
                        condition: Expr::Binary(
                            BinaryOp::Eq,
                            Box::new(Value(v_int(2))),
                            Box::new(Value(v_int(3))),
                        ),
                        statements: vec![Stmt {
                            node: StmtNode::mk_return(Value(v_int(3))),
                            line_col: (1, 0),
                            tree_line_no: 4,
                        }],
                    },
                ],

                otherwise: Some(ElseArm {
                    statements: vec![Stmt {
                        node: StmtNode::mk_return(Value(v_int(6))),
                        line_col: (1, 0),
                        tree_line_no: 6,
                    }],
                    environment_width: 0,
                }),
            },
        );
    }

    #[test]
    fn test_parse_if_elseif_chain() {
        let program = r#"
            if (1 == 2)
                return 5;
            elseif (2 == 3)
                return 3;
            elseif (3 == 4)
                return 4;
            else
                return 6;
            endif
        "#;
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(parse.stmts.len(), 1);
        assert_same_single(
            &parse.stmts,
            StmtNode::Cond {
                arms: vec![
                    CondArm {
                        environment_width: 0,
                        condition: Expr::Binary(
                            BinaryOp::Eq,
                            Box::new(Value(v_int(1))),
                            Box::new(Value(v_int(2))),
                        ),
                        statements: vec![Stmt {
                            node: StmtNode::mk_return(Value(v_int(5))),
                            line_col: (3, 0),
                            tree_line_no: 2,
                        }],
                    },
                    CondArm {
                        environment_width: 0,
                        condition: Expr::Binary(
                            BinaryOp::Eq,
                            Box::new(Value(v_int(2))),
                            Box::new(Value(v_int(3))),
                        ),
                        statements: vec![Stmt {
                            node: StmtNode::mk_return(Value(v_int(3))),
                            line_col: (5, 0),
                            tree_line_no: 4,
                        }],
                    },
                    CondArm {
                        environment_width: 0,
                        condition: Expr::Binary(
                            BinaryOp::Eq,
                            Box::new(Value(v_int(3))),
                            Box::new(Value(v_int(4))),
                        ),
                        statements: vec![Stmt {
                            node: StmtNode::mk_return(Value(v_int(4))),
                            line_col: (7, 0),
                            tree_line_no: 6,
                        }],
                    },
                ],

                otherwise: Some(ElseArm {
                    statements: vec![Stmt {
                        node: StmtNode::mk_return(Value(v_int(6))),
                        line_col: (9, 0),
                        tree_line_no: 8,
                    }],
                    environment_width: 0,
                }),
            },
        );
    }

    #[test]
    fn test_not_precedence() {
        let program = "return !(#2:move(5));";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_same_single(
            &parse.stmts,
            StmtNode::mk_return(Expr::Unary(
                UnaryOp::Not,
                Box::new(Verb {
                    location: Box::new(Value(v_objid(2))),
                    verb: Box::new(Value(v_str("move"))),
                    args: vec![Normal(Value(v_int(5)))],
                }),
            )),
        );
    }

    #[test]
    fn test_sys_obj_verb_regression() {
        // Precedence was wrong here, the not was being placed inside the verb call arguments.
        let program = r#"
            if (!$network:is_connected(this))
                return;
            endif
        "#;
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_same_single(
            &parse.stmts,
            StmtNode::Cond {
                arms: vec![CondArm {
                    environment_width: 0,
                    condition: Expr::Unary(
                        UnaryOp::Not,
                        Box::new(Verb {
                            location: Box::new(Prop {
                                location: Box::new(Value(v_objid(0))),
                                property: Box::new(Value(v_str("network"))),
                            }),
                            verb: Box::new(Value(v_str("is_connected"))),
                            args: vec![Normal(Id(parse.variables.find_name("this").unwrap()))],
                        }),
                    ),
                    statements: vec![Stmt {
                        node: StmtNode::mk_return_none(),
                        line_col: (3, 17),
                        tree_line_no: 2,
                    }],
                }],
                otherwise: None,
            },
        );
    }

    #[test]
    fn test_parse_for_loop() {
        let program = "for x in ({1,2,3}) b = x + 5; endfor";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        let x = parse.variables.find_name("x").unwrap();
        let b = parse.variables.find_name("b").unwrap();
        assert_same_single(
            &parse.stmts,
            StmtNode::ForList {
                environment_width: 0,
                value_binding: x,
                key_binding: None,
                expr: Expr::List(vec![
                    Normal(Value(v_int(1))),
                    Normal(Value(v_int(2))),
                    Normal(Value(v_int(3))),
                ]),
                body: vec![Stmt {
                    node: StmtNode::Expr(Expr::Assign {
                        left: Box::new(Id(b)),
                        right: Box::new(Expr::Binary(
                            Add,
                            Box::new(Id(x)),
                            Box::new(Value(v_int(5))),
                        )),
                    }),
                    line_col: (1, 20),
                    tree_line_no: 2,
                }],
            },
        )
    }

    #[test]
    fn test_parse_for_range() {
        let program = "for x in [1..5] b = x + 5; endfor";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(parse.stmts.len(), 1);
        let x = parse.variables.find_name("x").unwrap();
        let b = parse.variables.find_name("b").unwrap();
        assert_same_single(
            &parse.stmts,
            StmtNode::ForRange {
                environment_width: 0,
                id: x,
                from: Value(v_int(1)),
                to: Value(v_int(5)),
                body: vec![Stmt {
                    node: StmtNode::Expr(Expr::Assign {
                        left: Box::new(Id(b)),
                        right: Box::new(Expr::Binary(
                            Add,
                            Box::new(Id(x)),
                            Box::new(Value(v_int(5))),
                        )),
                    }),
                    line_col: (1, 17),
                    tree_line_no: 2,
                }],
            },
        )
    }

    #[test]
    fn test_indexed_range_len() {
        let program = "a = {1, 2, 3}; b = a[2..$];";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        let (a, b) = (
            parse.variables.find_name("a").unwrap(),
            parse.variables.find_name("b").unwrap(),
        );
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![
                StmtNode::Expr(Expr::Assign {
                    left: Box::new(Id(a)),
                    right: Box::new(Expr::List(vec![
                        Normal(Value(v_int(1))),
                        Normal(Value(v_int(2))),
                        Normal(Value(v_int(3))),
                    ])),
                }),
                StmtNode::Expr(Expr::Assign {
                    left: Box::new(Id(b)),
                    right: Box::new(Expr::Range {
                        base: Box::new(Id(a)),
                        from: Box::new(Value(v_int(2))),
                        to: Box::new(Expr::Length),
                    }),
                }),
            ]
        );
    }

    #[test]
    fn test_parse_while() {
        let program = "while (1) x = x + 1; if (x > 5) break; endif endwhile";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        let x = parse.variables.find_name("x").unwrap();

        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::While {
                environment_width: 0,
                id: None,
                condition: Value(v_int(1)),
                body: vec![
                    Stmt {
                        node: StmtNode::Expr(Expr::Assign {
                            left: Box::new(Id(x)),
                            right: Box::new(Expr::Binary(
                                Add,
                                Box::new(Id(x)),
                                Box::new(Value(v_int(1))),
                            )),
                        }),
                        line_col: (1, 11),
                        tree_line_no: 2,
                    },
                    Stmt {
                        node: StmtNode::Cond {
                            arms: vec![CondArm {
                                environment_width: 0,
                                condition: Expr::Binary(
                                    BinaryOp::Gt,
                                    Box::new(Id(x)),
                                    Box::new(Value(v_int(5))),
                                ),
                                statements: vec![Stmt {
                                    node: StmtNode::Break { exit: None },
                                    line_col: (1, 33),
                                    tree_line_no: 4,
                                }],
                            }],
                            otherwise: None,
                        },
                        line_col: (1, 22),
                        tree_line_no: 3,
                    },
                ],
            }]
        )
    }

    #[test]
    fn test_parse_labelled_while() {
        let program = "while chuckles (1) x = x + 1; if (x > 5) break chuckles; endif endwhile";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        let chuckles = parse.variables.find_name("chuckles").unwrap();
        let x = parse.variables.find_name("x").unwrap();
        assert_same_single(
            &parse.stmts,
            StmtNode::While {
                environment_width: 0,
                id: Some(chuckles),
                condition: Value(v_int(1)),
                body: vec![
                    Stmt {
                        node: StmtNode::Expr(Expr::Assign {
                            left: Box::new(Id(x)),
                            right: Box::new(Expr::Binary(
                                Add,
                                Box::new(Id(x)),
                                Box::new(Value(v_int(1))),
                            )),
                        }),
                        line_col: (1, 0),
                        tree_line_no: 2,
                    },
                    Stmt {
                        node: StmtNode::Cond {
                            arms: vec![CondArm {
                                environment_width: 0,
                                condition: Expr::Binary(
                                    BinaryOp::Gt,
                                    Box::new(Id(x)),
                                    Box::new(Value(v_int(5))),
                                ),
                                statements: vec![Stmt {
                                    node: StmtNode::Break {
                                        exit: Some(chuckles),
                                    },
                                    line_col: (1, 0),
                                    tree_line_no: 4,
                                }],
                            }],
                            otherwise: None,
                        },
                        line_col: (1, 0),
                        tree_line_no: 3,
                    },
                ],
            },
        )
    }

    #[test]
    fn test_sysobjref() {
        let program = "$string_utils:from_list(test_string);";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        let test_string = parse.variables.find_name("test_string").unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Verb {
                location: Box::new(Prop {
                    location: Box::new(Value(v_objid(0))),
                    property: Box::new(Value(v_str("string_utils"))),
                }),
                verb: Box::new(Value(v_str("from_list"))),
                args: vec![Normal(Id(test_string))],
            })]
        );
    }

    #[test]
    fn test_scatter_assign() {
        let program = "{connection} = args;";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        let connection = parse.variables.find_name("connection").unwrap();
        let args = parse.variables.find_name("args").unwrap();

        let scatter_items = vec![ScatterItem {
            kind: ScatterKind::Required,
            id: connection,
            expr: None,
        }];
        let scatter_right = Box::new(Id(args));
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::Scatter(scatter_items, scatter_right))]
        );
    }

    #[test]
    fn test_scatter_index_precedence() {
        // Regression test for a bug where the precedence of the right hand side of a scatter
        // assignment was incorrect.
        let program = "{connection} = args[1];";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        let connection = parse.variables.find_name("connection").unwrap();
        let args = parse.variables.find_name("args").unwrap();

        let scatter_items = vec![ScatterItem {
            kind: ScatterKind::Required,
            id: connection,
            expr: None,
        }];
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::Scatter(
                scatter_items,
                Box::new(Expr::Index(Box::new(Id(args)), Box::new(Value(v_int(1))),))
            ))]
        );
    }

    #[test]
    fn test_indexed_list() {
        let program = "{a,b,c}[1];";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        let a = parse.variables.find_name("a").unwrap();
        let b = parse.variables.find_name("b").unwrap();
        let c = parse.variables.find_name("c").unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::Index(
                Box::new(Expr::List(vec![
                    Normal(Id(a)),
                    Normal(Id(b)),
                    Normal(Id(c)),
                ])),
                Box::new(Value(v_int(1))),
            ))]
        );
    }

    #[test]
    fn test_assigned_indexed_list() {
        let program = "a = {a,b,c}[1];";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        let a = parse.variables.find_name("a").unwrap();
        let b = parse.variables.find_name("b").unwrap();
        let c = parse.variables.find_name("c").unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::Assign {
                left: Box::new(Id(a)),
                right: Box::new(Expr::Index(
                    Box::new(Expr::List(vec![
                        Normal(Id(a)),
                        Normal(Id(b)),
                        Normal(Id(c)),
                    ])),
                    Box::new(Value(v_int(1))),
                )),
            },)]
        );
    }

    #[test]
    fn test_indexed_assign() {
        let program = "this.stack[5] = 5;";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        let this = parse.variables.find_name("this").unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::Assign {
                left: Box::new(Expr::Index(
                    Box::new(Prop {
                        location: Box::new(Id(this)),
                        property: Box::new(Value(v_str("stack"))),
                    }),
                    Box::new(Value(v_int(5))),
                )),
                right: Box::new(Value(v_int(5))),
            })]
        );
    }

    #[test]
    fn test_for_list() {
        let program = "for i in ({1,2,3}) endfor return i;";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        let i = parse.variables.find_name("i").unwrap();
        // Verify the structure of the syntax tree for a for-list loop.
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![
                StmtNode::ForList {
                    environment_width: 0,
                    value_binding: i,
                    key_binding: None,
                    expr: Expr::List(vec![
                        Normal(Value(v_int(1))),
                        Normal(Value(v_int(2))),
                        Normal(Value(v_int(3))),
                    ]),
                    body: vec![],
                },
                StmtNode::mk_return(Id(i)),
            ]
        )
    }

    #[test]
    fn test_scatter_required() {
        let program = "{a, b, c} = args;";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::Scatter(
                vec![
                    ScatterItem {
                        kind: ScatterKind::Required,
                        id: parse.variables.find_name("a").unwrap(),
                        expr: None,
                    },
                    ScatterItem {
                        kind: ScatterKind::Required,
                        id: parse.variables.find_name("b").unwrap(),
                        expr: None,
                    },
                    ScatterItem {
                        kind: ScatterKind::Required,
                        id: parse.variables.find_name("c").unwrap(),
                        expr: None,
                    },
                ],
                Box::new(Id(parse.variables.find_name("args").unwrap())),
            ))]
        );
    }

    #[test]
    fn test_valid_underscore_and_no_underscore_ident() {
        let program = "_house == home;";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        let house = parse.variables.find_name("_house").unwrap();
        let home = parse.variables.find_name("home").unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::Binary(
                BinaryOp::Eq,
                Box::new(Id(house)),
                Box::new(Id(home)),
            ))]
        );
    }

    #[test]
    fn test_arg_splice() {
        let program = "return {@args, frozzbozz(@args)};";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        let args = parse.variables.find_name("args").unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::mk_return(Expr::List(vec![
                Splice(Id(args)),
                Normal(Call {
                    function: Symbol::mk("frozzbozz"),
                    args: vec![Splice(Id(args))],
                }),
            ]))]
        );
    }

    #[test]
    fn test_string_escape_codes() {
        // Just verify MOO's very limited string escape tokenizing, which does not support
        // anything other than \" and \\. \n, \t etc just become "n" "t".
        let program = r#"
            "\n \t \r \" \\";
        "#;
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Value(v_str(r#"n t r " \"#)))]
        );
    }

    #[test]
    fn test_empty_expr_stmt() {
        let program = r#"
            ;;;;
    "#;

        let parse = parse_program(program, CompileOptions::default()).unwrap();

        assert_eq!(stripped_stmts(&parse.stmts), vec![]);
    }
    #[test]
    fn test_assign_ambiguous_w_keyword_varname() {
        let program = r#"
            for a in ({1,2,3})
            endfor
            info = 5;
            forgotten = 3;
        "#;

        let parse = parse_program(program, CompileOptions::default()).unwrap();
        let a = parse.variables.find_name("a").unwrap();
        let info = parse.variables.find_name("info").unwrap();
        let forgotten = parse.variables.find_name("forgotten").unwrap();

        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![
                StmtNode::ForList {
                    environment_width: 0,
                    value_binding: a,
                    key_binding: None,
                    expr: Expr::List(vec![
                        Normal(Value(v_int(1))),
                        Normal(Value(v_int(2))),
                        Normal(Value(v_int(3))),
                    ]),
                    body: vec![],
                },
                StmtNode::Expr(Expr::Assign {
                    left: Box::new(Id(info)),
                    right: Box::new(Value(v_int(5))),
                }),
                StmtNode::Expr(Expr::Assign {
                    left: Box::new(Id(forgotten)),
                    right: Box::new(Value(v_int(3))),
                }),
            ]
        );
    }

    #[test]
    fn test_if_else() {
        let program = r#"if (5 == 5)
                        return 5;
                       else
                        return 3;
                       endif"#;
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(
            parse.stmts,
            vec![Stmt {
                node: StmtNode::Cond {
                    arms: vec![CondArm {
                        environment_width: 0,
                        condition: Expr::Binary(
                            BinaryOp::Eq,
                            Box::new(Value(v_int(5))),
                            Box::new(Value(v_int(5))),
                        ),
                        statements: vec![Stmt {
                            node: StmtNode::mk_return(Value(v_int(5))),
                            line_col: (2, 25),
                            tree_line_no: 2,
                        }],
                    }],
                    otherwise: Some(ElseArm {
                        statements: vec![Stmt {
                            node: StmtNode::mk_return(Value(v_int(3))),
                            line_col: (4, 25),
                            tree_line_no: 4,
                        }],
                        environment_width: 0,
                    }),
                },
                line_col: (1, 1),
                tree_line_no: 1,
            }]
        );
    }

    #[test]
    fn test_if_elseif_else() {
        let program = r#"if (5 == 5)
                        return 5;
                        elseif (2 == 2)
                        return 2;
                       else
                        return 3;
                       endif"#;
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Cond {
                arms: vec![
                    CondArm {
                        environment_width: 0,
                        condition: Expr::Binary(
                            BinaryOp::Eq,
                            Box::new(Value(v_int(5))),
                            Box::new(Value(v_int(5))),
                        ),
                        statements: vec![Stmt {
                            node: StmtNode::mk_return(Value(v_int(5))),
                            line_col: (2, 25),
                            tree_line_no: 2,
                        }],
                    },
                    CondArm {
                        environment_width: 0,
                        condition: Expr::Binary(
                            BinaryOp::Eq,
                            Box::new(Value(v_int(2))),
                            Box::new(Value(v_int(2))),
                        ),
                        statements: vec![Stmt {
                            node: StmtNode::mk_return(Value(v_int(2))),
                            line_col: (4, 25),
                            tree_line_no: 4,
                        }],
                    },
                ],

                otherwise: Some(ElseArm {
                    statements: vec![Stmt {
                        node: StmtNode::mk_return(Value(v_int(3))),
                        line_col: (6, 25),
                        tree_line_no: 6,
                    }],
                    environment_width: 0,
                }),
            }]
        );
    }

    #[test]
    fn test_if_in_range() {
        let program = r#"if (5 in {1,2,3})
                       endif"#;
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Cond {
                arms: vec![CondArm {
                    environment_width: 0,

                    condition: Expr::Binary(
                        BinaryOp::In,
                        Box::new(Value(v_int(5))),
                        Box::new(Expr::List(vec![
                            Normal(Value(v_int(1))),
                            Normal(Value(v_int(2))),
                            Normal(Value(v_int(3))),
                        ])),
                    ),
                    statements: vec![],
                }],
                otherwise: None,
            }]
        );
    }

    #[test]
    fn try_except() {
        let program = r#"try
                            5;
                         except (E_PROPNF)
                            return;
                         endtry"#;
        let parse = parse_program(program, CompileOptions::default()).unwrap();

        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::TryExcept {
                environment_width: 0,
                body: vec![Stmt {
                    node: StmtNode::Expr(Value(v_int(5))),
                    line_col: (2, 29),
                    tree_line_no: 2,
                }],
                excepts: vec![ExceptArm {
                    id: None,
                    codes: CatchCodes::Codes(vec![Normal(Error(E_PROPNF, None))]),
                    statements: vec![Stmt {
                        node: StmtNode::mk_return_none(),
                        line_col: (4, 29),
                        tree_line_no: 4,
                    }],
                }],
            }]
        );
    }

    #[test]
    fn test_float() {
        let program = "10000.0;";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Value(v_float(10000.0)))]
        );
    }

    #[test]
    fn test_in_range() {
        let program = "a in {1,2,3};";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        let a = parse.variables.find_name("a").unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::Binary(
                BinaryOp::In,
                Box::new(Id(a)),
                Box::new(Expr::List(vec![
                    Normal(Value(v_int(1))),
                    Normal(Value(v_int(2))),
                    Normal(Value(v_int(3))),
                ])),
            ))]
        );
    }

    #[test]
    fn test_empty_list() {
        let program = "{};";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::List(vec![]))]
        );
    }

    #[test]
    fn test_verb_expr() {
        let program = "this:(\"verb\")(1,2,3);";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Verb {
                location: Box::new(Id(parse.variables.find_name("this").unwrap())),
                verb: Box::new(Value(v_str("verb"))),
                args: vec![
                    Normal(Value(v_int(1))),
                    Normal(Value(v_int(2))),
                    Normal(Value(v_int(3))),
                ],
            })]
        );
    }

    #[test]
    fn test_prop_expr() {
        let program = "this.(\"prop\");";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Prop {
                location: Box::new(Id(parse.variables.find_name("this").unwrap())),
                property: Box::new(Value(v_str("prop"))),
            })]
        );
    }

    #[test]
    fn test_not_expr() {
        let program = "!2;";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::Unary(
                UnaryOp::Not,
                Box::new(Value(v_int(2))),
            ))]
        );
    }

    #[test]
    fn test_comparison_assign_chain() {
        let program = "(2 <= (len = length(player)));";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        let len = parse.variables.find_name("len").unwrap();
        let text = parse.variables.find_name("player").unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::Binary(
                BinaryOp::LtE,
                Box::new(Value(v_int(2))),
                Box::new(Expr::Assign {
                    left: Box::new(Id(len)),
                    right: Box::new(Call {
                        function: Symbol::mk("length"),
                        args: vec![Normal(Id(text))],
                    }),
                }),
            ))]
        );
    }

    #[test]
    fn test_cond_expr() {
        let program = "a = (1 == 2 ? 3 | 4);";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::Assign {
                left: Box::new(Id(parse.variables.find_name("a").unwrap())),
                right: Box::new(Expr::Cond {
                    condition: Box::new(Expr::Binary(
                        BinaryOp::Eq,
                        Box::new(Value(v_int(1))),
                        Box::new(Value(v_int(2))),
                    )),
                    consequence: Box::new(Value(v_int(3))),
                    alternative: Box::new(Value(v_int(4))),
                }),
            })]
        );
    }

    #[test]
    fn test_list_compare() {
        let program = "{what} == args;";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::Binary(
                BinaryOp::Eq,
                Box::new(Expr::List(vec![Normal(Id(parse
                    .variables
                    .find_name("what")
                    .unwrap())),])),
                Box::new(Id(parse.variables.find_name("args").unwrap())),
            ))]
        );
    }

    #[test]
    fn test_keyword_disambig_call() {
        // This is a regression test for a bug where the parser would incorrectly parse
        // this as "l in e ... " instead of "line in "
        let program = r#"
            for line in ({1,2,3})
            endfor(52);
        "#;
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![
                StmtNode::ForList {
                    environment_width: 0,
                    value_binding: parse.variables.find_name("line").unwrap(),
                    key_binding: None,
                    expr: Expr::List(vec![
                        Normal(Value(v_int(1))),
                        Normal(Value(v_int(2))),
                        Normal(Value(v_int(3))),
                    ]),
                    body: vec![],
                },
                StmtNode::Expr(Value(v_int(52))),
            ]
        );
    }

    #[test]
    fn try_catch_expr() {
        let program = "return {`x ! e_varnf => 666'};";
        let parse = parse_program(program, CompileOptions::default()).unwrap();

        let varnf = Normal(Error(E_VARNF, None));
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::mk_return(Expr::List(vec![Normal(
                Expr::TryCatch {
                    trye: Box::new(Id(parse.variables.find_name("x").unwrap())),
                    codes: CatchCodes::Codes(vec![varnf]),
                    except: Some(Box::new(Value(v_int(666)))),
                }
            )],))]
        )
    }

    #[test]
    fn try_catch_any_expr() {
        let program = r#"`raise(E_INVARG) ! ANY';"#;
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        let invarg = Normal(Error(E_INVARG, None));

        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::TryCatch {
                trye: Box::new(Call {
                    function: Symbol::mk("raise"),
                    args: vec![invarg]
                }),
                codes: CatchCodes::Any,
                except: None,
            })]
        );
    }

    #[test]
    fn test_try_any_expr() {
        let program = r#"`$ftp_client:finish_get(this.connection) ! ANY';"#;
        let parse = parse_program(program, CompileOptions::default()).unwrap();

        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::TryCatch {
                trye: Box::new(Verb {
                    location: Box::new(Prop {
                        location: Box::new(Value(v_objid(0))),
                        property: Box::new(Value(v_str("ftp_client"))),
                    }),
                    verb: Box::new(Value(v_str("finish_get"))),
                    args: vec![Normal(Prop {
                        location: Box::new(Id(parse.variables.find_name("this").unwrap())),
                        property: Box::new(Value(v_str("connection"))),
                    })],
                }),
                codes: CatchCodes::Any,
                except: None,
            })]
        )
    }

    #[test]
    fn test_paren_expr() {
        // Verify that parenthesized expressions end up with correct precedence and nesting.
        let program_a = r#"1 && (2 || 3);"#;
        let parse = parse_program(program_a, CompileOptions::default()).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::And(
                Box::new(Value(v_int(1))),
                Box::new(Expr::Or(
                    Box::new(Value(v_int(2))),
                    Box::new(Value(v_int(3))),
                )),
            ))]
        );
        let program_b = r#"1 && 2 || 3;"#;
        let parse = parse_program(program_b, CompileOptions::default()).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::Or(
                Box::new(Expr::And(
                    Box::new(Value(v_int(1))),
                    Box::new(Value(v_int(2))),
                )),
                Box::new(Value(v_int(3))),
            ))]
        );
    }

    #[test]
    fn test_pass_exprs() {
        let program = r#"
            result = pass(@args);
            result = pass();
            result = pass(1,2,3,4);
            pass = blop;
            return pass;
        "#;
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![
                StmtNode::Expr(Expr::Assign {
                    left: Box::new(Id(parse.variables.find_name("result").unwrap())),
                    right: Box::new(Expr::Pass {
                        args: vec![Splice(Id(parse.variables.find_name("args").unwrap()))],
                    }),
                }),
                StmtNode::Expr(Expr::Assign {
                    left: Box::new(Id(parse.variables.find_name("result").unwrap())),
                    right: Box::new(Expr::Pass { args: vec![] }),
                }),
                StmtNode::Expr(Expr::Assign {
                    left: Box::new(Id(parse.variables.find_name("result").unwrap())),
                    right: Box::new(Expr::Pass {
                        args: vec![
                            Normal(Value(v_int(1))),
                            Normal(Value(v_int(2))),
                            Normal(Value(v_int(3))),
                            Normal(Value(v_int(4))),
                        ],
                    }),
                }),
                StmtNode::Expr(Expr::Assign {
                    left: Box::new(Id(parse.variables.find_name("pass").unwrap())),
                    right: Box::new(Id(parse.variables.find_name("blop").unwrap())),
                }),
                StmtNode::mk_return(Id(parse.variables.find_name("pass").unwrap())),
            ]
        );
    }

    #[test]
    fn test_unknown_label() {
        let program = r#"
            while (1)
                break unknown;
            endwhile
        "#;
        let parse = parse_program(program, CompileOptions::default());
        assert!(matches!(parse, Err(CompileError::UnknownLoopLabel(_, _))));

        let program = r#"
            while (1)
                continue unknown;
            endwhile"#;
        let parse = parse_program(program, CompileOptions::default());
        assert!(matches!(parse, Err(CompileError::UnknownLoopLabel(_, _))));
    }

    #[test]
    fn test_begin_end() {
        let program = r#"begin
                return 5;
            end
        "#;
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Scope {
                num_bindings: 0,
                body: vec![Stmt {
                    node: StmtNode::mk_return(Value(v_int(5))),
                    line_col: (2, 17),
                    tree_line_no: 2,
                }],
            }]
        );
    }

    /// Test that lexical block scopes parse and that the inner scope variables can shadow outer scope
    #[test]
    fn test_parse_scoped_variables() {
        let program = r#"begin
                                 let x = 5;
                                 let y = 6;
                                 x = x + 6;
                                 let z = 7;
                                 let o;
                                 global a = 1;
                               end
                               return x;"#;
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        let x_names = parse.variables.find_named("x");
        let y_names = parse.variables.find_named("y");
        let z_names = parse.variables.find_named("z");
        let o_names = parse.variables.find_named("o");
        let inner_y = y_names[0];
        let inner_z = z_names[0];
        let inner_o = o_names[0];
        let global_a = parse.variables.find_named("a")[0];
        assert_eq!(x_names.len(), 2);
        let global_x = x_names[1];
        // Declared first, so appears in unbound names first, though in the bound names it will
        // appear second.
        let inner_x = x_names[0];
        assert_same(
            &parse.stmts,
            &[
                StmtNode::Scope {
                    num_bindings: 4,
                    body: vec![
                        // Declaration of X
                        Stmt {
                            node: StmtNode::Expr(Expr::Assign {
                                left: Box::new(Id(inner_x)),
                                right: Box::new(Value(v_int(5))),
                            }),
                            line_col: (2, 0),
                            tree_line_no: 2,
                        },
                        // Declaration of y
                        Stmt {
                            node: StmtNode::Expr(Expr::Assign {
                                left: Box::new(Id(inner_y)),
                                right: Box::new(Value(v_int(6))),
                            }),
                            line_col: (3, 0),
                            tree_line_no: 3,
                        },
                        Stmt {
                            node: StmtNode::Expr(Expr::Assign {
                                left: Box::new(Id(inner_x)),
                                right: Box::new(Expr::Binary(
                                    Add,
                                    Box::new(Id(inner_x)),
                                    Box::new(Value(v_int(6))),
                                )),
                            }),
                            line_col: (4, 0),
                            tree_line_no: 4,
                        },
                        // Asssignment to z.
                        Stmt {
                            node: StmtNode::Expr(Expr::Assign {
                                left: Box::new(Id(inner_z)),
                                right: Box::new(Value(v_int(7))),
                            }),
                            line_col: (5, 0),
                            tree_line_no: 5,
                        },
                        // Declaration of o (o = v_none)
                        Stmt {
                            node: StmtNode::Expr(Expr::Assign {
                                left: Box::new(Id(inner_o)),
                                right: Box::new(Value(v_none())),
                            }),
                            line_col: (6, 0),
                            tree_line_no: 6,
                        },
                        // Assignment to global a
                        Stmt {
                            node: StmtNode::Expr(Expr::Assign {
                                left: Box::new(Id(global_a)),
                                right: Box::new(Value(v_int(1))),
                            }),
                            line_col: (7, 0),
                            tree_line_no: 7,
                        },
                    ],
                },
                StmtNode::mk_return(Id(global_x)),
            ],
        );
    }

    /// "Lets = " was getting parsed as "let s ="
    #[test]
    fn test_lets_vs_let_s_regreesion() {
        let program = r#"
        lets = 5;
        "#;
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        parse.variables.find_name("lets").expect("lets not found");
    }

    #[test]
    fn test_const() {
        let program = r#"
        const x = 5;
        x = 6;
        "#;
        let parse = parse_program(program, CompileOptions::default());
        assert!(matches!(parse, Err(CompileError::AssignToConst(_, _))));
    }

    #[test]
    fn test_no_lexical_scopes() {
        let program = r#"
        begin
            let x = 5;
            begin
                let x = 6;
            end
        end
        "#;
        let parse = parse_program(
            program,
            CompileOptions {
                lexical_scopes: false,
                ..CompileOptions::default()
            },
        );
        assert!(matches!(parse, Err(CompileError::DisabledFeature(_, _))));
    }

    #[test]
    fn test_no_map() {
        let program = r#"
        [ 1 -> 2, 3 -> 4 ];
        "#;
        let parse = parse_program(
            program,
            CompileOptions {
                map_type: false,
                ..CompileOptions::default()
            },
        );
        assert!(matches!(parse, Err(CompileError::DisabledFeature(_, _))));
    }

    #[test]
    fn test_map() {
        let program = r#"
        [ 1 -> 2, 3 -> 4 ];
        "#;
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::Map(vec![
                (Value(v_int(1)), Value(v_int(2))),
                (Value(v_int(3)), Value(v_int(4))),
            ]))]
        );
    }

    #[test]
    fn test_local_scatter_assign() {
        let program = r#"begin
            let a = 3;
            begin
                let {a, b} = {1, 2};
            end
        end
        "#;
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        let a_outer = parse.variables.find_named("a")[0];
        let a_inner = parse.variables.find_named("a")[1];
        let b = parse.variables.find_named("b")[0];
        assert_eq!(parse.variables.decl_for(&a_inner).depth, 2);
        assert_eq!(parse.variables.decl_for(&b).depth, 2);
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Scope {
                num_bindings: 1,
                body: vec![
                    Stmt {
                        node: StmtNode::Expr(Expr::Assign {
                            left: Box::new(Id(a_outer)),
                            right: Box::new(Value(v_int(3))),
                        }),
                        line_col: (2, 13),
                        tree_line_no: 2,
                    },
                    Stmt {
                        node: StmtNode::Scope {
                            num_bindings: 2,
                            body: vec![Stmt {
                                node: StmtNode::Expr(Expr::Scatter(
                                    vec![
                                        ScatterItem {
                                            kind: ScatterKind::Required,
                                            id: a_inner,
                                            expr: None,
                                        },
                                        ScatterItem {
                                            kind: ScatterKind::Required,
                                            id: b,
                                            expr: None,
                                        },
                                    ],
                                    Box::new(Expr::List(vec![
                                        Normal(Value(v_int(1))),
                                        Normal(Value(v_int(2))),
                                    ])),
                                )),
                                line_col: (4, 17),
                                tree_line_no: 4,
                            }],
                        },
                        line_col: (3, 13),
                        tree_line_no: 3,
                    },
                ],
            },]
        );
    }

    /// Test that a const scatter assign can not be followed up with an additional assignment.
    #[test]
    fn test_const_scatter_assign() {
        let program = r#"begin
            const {a, b} = {1, 2};
            a = 3;
        end
        "#;
        let parse = parse_program(program, CompileOptions::default());
        assert!(matches!(parse, Err(CompileError::AssignToConst(_, _))));
    }

    /// And same for if two scatter assigns are done, though in this case the error takes the form of
    /// a duplicate variable error.
    #[test]
    fn test_const_scatter_assign_twice() {
        let program = r#"begin
            const {a, b} = {1, 2};
            const {a, b} = {2, 3};
        end
        "#;
        let parse = parse_program(program, CompileOptions::default());
        assert!(matches!(parse, Err(CompileError::DuplicateVariable(_, _))));
    }

    #[test]
    fn test_empty_flyweight() {
        let program = r#"<#1>;"#;
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Flyweight(
                Box::new(Value(v_objid(1))),
                vec![],
                None,
            ))]
        );
    }

    #[test]
    fn test_flyweight_no_slots_just_contents() {
        let program = r#"<#1, {2}>;"#;
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Flyweight(
                Box::new(Value(v_objid(1))),
                vec![],
                Some(Box::new(Expr::List(vec![Normal(Value(v_int(2)))]))),
            ))]
        );
    }
    #[test]
    fn test_flyweight_empty_slots_just_contents() {
        let program = r#"<#1, [], {2}>;"#;
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Flyweight(
                Box::new(Value(v_objid(1))),
                vec![],
                Some(Box::new(Expr::List(vec![Normal(Value(v_int(2)))]))),
            ))]
        );
    }

    #[test]
    fn test_flyweight_only_slots() {
        let program = r#"<#1, [a->1 , b->2]>;"#;
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Flyweight(
                Box::new(Value(v_objid(1))),
                vec![
                    (Symbol::mk("a"), Value(v_int(1))),
                    (Symbol::mk("b"), Value(v_int(2)))
                ],
                None,
            ))]
        );
    }

    #[test]
    fn test_flyweight_arbitrary_expression_contents() {
        // Test that flyweight contents can be any expression, not just lists
        let program = r#"<#1, [], a_list>;"#;
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        let a_list_var = parse.variables.find_name("a_list").unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Flyweight(
                Box::new(Value(v_objid(1))),
                vec![],
                Some(Box::new(Id(a_list_var))),
            ))]
        );
    }

    /// Modification to the MOO syntax which allows "return" to be an expression so as to allow
    /// the following syntax, as in Julia....
    #[test]
    fn test_return_as_expr() {
        let program = r#"true && return 5;"#;
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::And(
                Box::new(Value(Var::mk_bool(true))),
                Box::new(Expr::Return(Some(Box::new(Value(v_int(5))))))
            ))]
        );
    }

    /// Test both traditional and custom error parsing
    #[test]
    fn test_errors_parsing() {
        let program = r#"return {e_invarg, e_propnf, e_custom, e__ultra_long_custom, e_unknown};"#;
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::Return(Some(Box::new(Expr::List(
                vec![
                    Normal(Error(E_INVARG, None)),
                    Normal(Error(E_PROPNF, None)),
                    Normal(Error(ErrCustom("e_custom".into()), None)),
                    Normal(Error(ErrCustom("e__ultra_long_custom".into()), None)),
                    Normal(Error(ErrCustom("e_unknown".into()), None)),
                ]
            )))))]
        )
    }

    #[test]
    fn test_errors_args_parsing() {
        let program = r#"return {e_invarg("test"), e_propnf(5), e_custom("booo")};"#;
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::Return(Some(Box::new(Expr::List(
                vec![
                    Normal(Error(E_INVARG, Some(Box::new(Value(v_str("test")))))),
                    Normal(Error(E_PROPNF, Some(Box::new(Value(v_int(5)))))),
                    Normal(Error(
                        ErrCustom("e_custom".into()),
                        Some(Box::new(Value(v_str("booo"))))
                    )),
                ]
            )))))]
        )
    }

    #[test]
    fn test_return_x_regression() {
        // "returnval" was being parsed as "return val" not expecting whitespace after
        // (Was due to return as expr being parsed higher precedence than ident)
        let program = r#"return returnval;"#;
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::Return(Some(Box::new(Id(parse
                .variables
                .find_name("returnval")
                .unwrap())))))]
        );
    }

    #[test]
    fn test_scope_regression() {
        let program = r#"
        {dude, chamber, chamber_index} = args;
        if (chamber.mode == "package")
          reagent = $player_drug_reagent:create(chamber.product_name, chamber.reagents, chamber.quality);
          this.reagents[reagent] = `this.reagents[reagent] ! ANY => 0' + 5;
        endif
        for quantity, reagent in (received_reagents)
          this.reagents[reagent] = `this.reagents[reagent] ! ANY => 0' + quantity;
        endfor
        "#;

        let options = CompileOptions::default();
        parse_program(program, options).unwrap();
    }

    #[test]
    fn test_binary_literal() {
        // Test basic binary literal parsing
        let program = r#"return b"SGVsbG8gV29ybGQ=";"#;
        let parsed = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(parsed.stmts.len(), 1);

        // Extract the binary value and verify it decoded correctly
        if let StmtNode::Expr(Expr::Return(Some(expr))) = &parsed.stmts[0].node {
            if let Expr::Value(val) = expr.as_ref() {
                if let Some(binary) = val.as_binary() {
                    // "SGVsbG8gV29ybGQ=" is base64 for "Hello World"
                    assert_eq!(binary.as_bytes(), b"Hello World");
                } else {
                    panic!("Expected binary value, got: {:?}", val.variant());
                }
            } else {
                panic!("Expected Value expression, got: {:?}", expr);
            }
        } else {
            panic!(
                "Expected return statement with value, got: {:?}",
                parsed.stmts[0].node
            );
        }
    }

    #[test]
    fn test_binary_literal_empty() {
        // Test empty binary literal
        let program = r#"return b"";"#;
        let parsed = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(parsed.stmts.len(), 1);

        if let StmtNode::Expr(Expr::Return(Some(expr))) = &parsed.stmts[0].node {
            if let Expr::Value(val) = expr.as_ref() {
                if let Some(binary) = val.as_binary() {
                    assert_eq!(binary.as_bytes(), b"");
                } else {
                    panic!("Expected binary value, got: {:?}", val.variant());
                }
            }
        }
    }

    #[test]
    fn test_binary_literal_invalid_base64() {
        // Test that invalid base64 content produces an error
        // Using invalid base64 that would pass the grammar but fail decoding
        let program = r#"return b"SGVsbG8gV29ybGQ";"#; // Missing padding
        let result = parse_program(program, CompileOptions::default());
        assert!(result.is_err());

        if let Err(err) = result {
            // The error should mention invalid base64 or binary literal
            let error_str = err.to_string();
            assert!(error_str.contains("invalid base64") || error_str.contains("binary literal"));
        }
    }

    #[test]
    fn test_binary_literal_roundtrip() {
        use crate::unparse::to_literal;

        // Create a binary value
        let original_data = b"Hello, World! This is binary data.";
        let binary_var = v_binary(original_data.to_vec());

        // Convert to literal representation
        let literal_str = to_literal(&binary_var);

        // Parse it back
        let program = format!("return {};", literal_str);
        let parsed = parse_program(&program, CompileOptions::default()).unwrap();

        // Extract the parsed value
        if let StmtNode::Expr(Expr::Return(Some(expr))) = &parsed.stmts[0].node {
            if let Expr::Value(val) = expr.as_ref() {
                if let Some(binary) = val.as_binary() {
                    assert_eq!(binary.as_bytes(), original_data);
                } else {
                    panic!("Expected binary value, got: {:?}", val.variant());
                }
            }
        }
    }
}
