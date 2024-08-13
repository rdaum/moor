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

/// Kicks off the Pest parser and converts it into our AST.
/// This is the main entry point for parsing.
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::str::FromStr;

use moor_values::var::{v_none, Symbol};
use moor_values::SYSTEM_OBJECT;
use pest::pratt_parser::{Assoc, Op, PrattParser};
pub use pest::Parser as PestParser;
use tracing::{instrument, warn};

use moor_values::var::Error::{
    E_ARGS, E_DIV, E_FLOAT, E_INVARG, E_INVIND, E_MAXREC, E_NACC, E_NONE, E_PERM, E_PROPNF,
    E_QUOTA, E_RANGE, E_RECMOVE, E_TYPE, E_VARNF, E_VERBNF,
};
use moor_values::var::Objid;
use moor_values::var::{v_err, v_float, v_int, v_objid, v_str, v_string};

use crate::ast::Arg::{Normal, Splice};
use crate::ast::StmtNode::Scope;
use crate::ast::{
    Arg, BinaryOp, CatchCodes, CondArm, ElseArm, ExceptArm, Expr, ScatterItem, ScatterKind, Stmt,
    StmtNode, UnaryOp,
};
use crate::names::{Names, UnboundName, UnboundNames};
use crate::parse::moo::{MooParser, Rule};
use crate::unparse::annotate_line_numbers;
use crate::Name;
use moor_values::model::CompileError;

pub mod moo {
    #[derive(Parser)]
    #[grammar = "src/moo.pest"]
    pub struct MooParser;
}

#[derive(Debug, Clone)]
pub struct CompileOptions {
    /// Whether we allow lexical scope blocks. begin/end blocks and 'let' and 'global' statements
    pub lexical_scopes: bool,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            lexical_scopes: true,
        }
    }
}

pub struct TreeTransformer {
    // TODO: this is Rc<RefCell because PrattParser has some API restrictions which result in
    //   borrowing issues, see: https://github.com/pest-parser/pest/discussions/1030
    names: RefCell<UnboundNames>,
    options: CompileOptions,
}

impl TreeTransformer {
    pub fn new(options: CompileOptions) -> Rc<Self> {
        Rc::new(Self {
            names: RefCell::new(UnboundNames::new()),
            options,
        })
    }

    fn parse_atom(
        self: Rc<Self>,
        pairs: pest::iterators::Pair<Rule>,
    ) -> Result<Expr, CompileError> {
        match pairs.as_rule() {
            Rule::ident => {
                let name = self
                    .names
                    .borrow_mut()
                    .find_or_add_name_global(pairs.as_str().trim());
                Ok(Expr::Id(name))
            }
            Rule::object => {
                let ostr = &pairs.as_str()[1..];
                let oid = i64::from_str(ostr).unwrap();
                let objid = Objid(oid);
                Ok(Expr::Value(v_objid(objid)))
            }
            Rule::integer => match pairs.as_str().parse::<i64>() {
                Ok(int) => Ok(Expr::Value(v_int(int))),
                Err(e) => {
                    warn!("Failed to parse '{}' to i64: {}", pairs.as_str(), e);
                    Ok(Expr::Value(v_err(E_INVARG)))
                }
            },
            Rule::float => {
                let float = pairs.as_str().parse::<f64>().unwrap();
                Ok(Expr::Value(v_float(float)))
            }
            Rule::string => {
                let string = pairs.as_str();
                let parsed = unquote_str(string)?;
                Ok(Expr::Value(v_str(&parsed)))
            }
            Rule::err => {
                let e = pairs.as_str();
                Ok(Expr::Value(match e.to_lowercase().as_str() {
                    "e_args" => v_err(E_ARGS),
                    "e_div" => v_err(E_DIV),
                    "e_float" => v_err(E_FLOAT),
                    "e_invarg" => v_err(E_INVARG),
                    "e_invind" => v_err(E_INVIND),
                    "e_maxrec" => v_err(E_MAXREC),
                    "e_nacc" => v_err(E_NACC),
                    "e_none" => v_err(E_NONE),
                    "e_perm" => v_err(E_PERM),
                    "e_propnf" => v_err(E_PROPNF),
                    "e_quota" => v_err(E_QUOTA),
                    "e_range" => v_err(E_RANGE),
                    "e_recmove" => v_err(E_RECMOVE),
                    "e_type" => v_err(E_TYPE),
                    "e_varnf" => v_err(E_VARNF),
                    "e_verbnf" => v_err(E_VERBNF),
                    &_ => {
                        panic!("unknown error")
                    }
                }))
            }
            _ => {
                panic!("Unimplemented atom: {:?}", pairs);
            }
        }
    }

    fn parse_exprlist(
        self: Rc<Self>,
        pairs: pest::iterators::Pairs<Rule>,
    ) -> Result<Vec<Arg>, CompileError> {
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

    fn parse_arglist(
        self: Rc<Self>,
        pairs: pest::iterators::Pairs<Rule>,
    ) -> Result<Vec<Arg>, CompileError> {
        let Some(first) = pairs.peek() else {
            return Ok(vec![]);
        };

        let Rule::exprlist = first.as_rule() else {
            panic!("Unimplemented arglist: {:?}", first);
        };

        return self.parse_exprlist(first.into_inner());
    }

    fn parse_except_codes(
        self: Rc<Self>,
        pairs: pest::iterators::Pair<Rule>,
    ) -> Result<CatchCodes, CompileError> {
        match pairs.as_rule() {
            Rule::anycode => Ok(CatchCodes::Any),
            Rule::exprlist => Ok(CatchCodes::Codes(self.parse_exprlist(pairs.into_inner())?)),
            _ => {
                panic!("Unimplemented except_codes: {:?}", pairs);
            }
        }
    }

    fn parse_expr(
        self: Rc<Self>,
        pairs: pest::iterators::Pairs<Rule>,
    ) -> Result<Expr, CompileError> {
        let pratt = PrattParser::new()
            // Generally following C-like precedence order as described:
            //   https://en.cppreference.com/w/c/language/operator_precedence
            // Precedence from lowest to highest.
            // 14. Assignments are lowest precedence.
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
        let prefix_self = self.clone();
        let postfix_self = self.clone();

        return pratt
            .map_primary(|primary| match primary.as_rule() {
                Rule::atom => {
                    let mut inner = primary.into_inner();
                    let expr = primary_self.clone().parse_atom(inner.next().unwrap())?;
                    Ok(expr)
                }
                Rule::sysprop => {
                    let mut inner = primary.into_inner();
                    let property = inner.next().unwrap().as_str();
                    Ok(Expr::Prop {
                        location: Box::new(Expr::Value(v_objid(SYSTEM_OBJECT))),
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
                        location: Box::new(Expr::Value(v_objid(SYSTEM_OBJECT))),
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
                Rule::builtin_call => {
                    let mut inner = primary.into_inner();
                    let bf = inner.next().unwrap().as_str();
                    let args = primary_self
                        .clone()
                        .parse_arglist(inner.next().unwrap().into_inner())?;
                    Ok(Expr::Call {
                        function: Symbol::mk_case_insensitive(bf),
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
                    Err(e) => {
                        warn!("Failed to parse '{}' to i64: {}", primary.as_str(), e);
                        Ok(Expr::Value(v_err(E_INVARG)))
                    }
                },
                _ => todo!("Unimplemented primary: {:?}", primary.as_rule()),
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
                    let inner = op.into_inner();
                    let mut items = vec![];
                    for scatter_item in inner {
                        match scatter_item.as_rule() {
                            Rule::scatter_optional => {
                                let mut inner = scatter_item.into_inner();
                                let id = inner.next().unwrap().as_str();
                                let id = primary_self
                                    .clone()
                                    .names
                                    .borrow_mut()
                                    .find_or_add_name_global(id);
                                let expr = inner.next().map(|e| {
                                    primary_self.clone().parse_expr(e.into_inner()).unwrap()
                                });
                                items.push(ScatterItem {
                                    kind: ScatterKind::Optional,
                                    id,
                                    expr,
                                });
                            }
                            Rule::scatter_target => {
                                let mut inner = scatter_item.into_inner();
                                let id = inner.next().unwrap().as_str();
                                let id = primary_self
                                    .clone()
                                    .names
                                    .borrow_mut()
                                    .find_or_add_name_global(id);
                                items.push(ScatterItem {
                                    kind: ScatterKind::Required,
                                    id,
                                    expr: None,
                                });
                            }
                            Rule::scatter_rest => {
                                let mut inner = scatter_item.into_inner();
                                let id = inner.next().unwrap().as_str();
                                let id = prefix_self
                                    .clone()
                                    .names
                                    .borrow_mut()
                                    .find_or_add_name_global(id);
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
                    Ok(Expr::Scatter(items, Box::new(rhs?)))
                }
                Rule::not => Ok(Expr::Unary(UnaryOp::Not, Box::new(rhs?))),
                Rule::neg => Ok(Expr::Unary(UnaryOp::Neg, Box::new(rhs?))),
                _ => todo!("Unimplemented prefix: {:?}", op.as_rule()),
            })
            .map_postfix(|lhs, op| match op.as_rule() {
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
                    let mut parts = op.into_inner();
                    let right = postfix_self
                        .clone()
                        .parse_expr(parts.next().unwrap().into_inner())?;
                    Ok(Expr::Assign {
                        left: Box::new(lhs?),
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
            })
            .parse(pairs);
    }

    fn parse_statement(
        self: Rc<Self>,
        pair: pest::iterators::Pair<Rule>,
    ) -> Result<Option<Stmt>, CompileError> {
        let line = pair.line_col().0;
        match pair.as_rule() {
            Rule::expr_statement => {
                let mut inner = pair.into_inner();
                if let Some(rule) = inner.next() {
                    let expr = self.parse_expr(rule.into_inner())?;
                    return Ok(Some(Stmt::new(StmtNode::Expr(expr), line)));
                }
                Ok(None)
            }
            Rule::while_statement => {
                self.enter_scope();
                let mut parts = pair.into_inner();
                let condition = self
                    .clone()
                    .parse_expr(parts.next().unwrap().into_inner())?;
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
                    line,
                )))
            }
            Rule::labelled_while_statement => {
                self.enter_scope();
                let mut parts = pair.into_inner();
                let id = self
                    .clone()
                    .names
                    .borrow_mut()
                    .find_or_add_name_global(parts.next().unwrap().as_str());
                let condition = self
                    .clone()
                    .parse_expr(parts.next().unwrap().into_inner())?;
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
                    line,
                )))
            }
            Rule::if_statement => {
                self.enter_scope();
                let mut parts = pair.into_inner();
                let mut arms = vec![];
                let mut otherwise = None;
                let condition = self
                    .clone()
                    .parse_expr(parts.next().unwrap().into_inner())?;
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
                            {
                                self.clone().names.borrow_mut().push_scope();
                            }
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
                            self.clone().names.borrow_mut().push_scope();
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
                Ok(Some(Stmt::new(StmtNode::Cond { arms, otherwise }, line)))
            }
            Rule::break_statement => {
                let mut parts = pair.into_inner();
                let label = match parts.next() {
                    None => None,
                    Some(s) => {
                        let label = s.as_str();
                        let Some(label) = self.names.borrow_mut().find_name(label) else {
                            return Err(CompileError::UnknownLoopLabel(label.to_string()));
                        };
                        Some(label)
                    }
                };
                Ok(Some(Stmt::new(StmtNode::Break { exit: label }, line)))
            }
            Rule::continue_statement => {
                let mut parts = pair.into_inner();
                let label = match parts.next() {
                    None => None,
                    Some(s) => {
                        let label = s.as_str();
                        let Some(label) = self.names.borrow_mut().find_name(label) else {
                            return Err(CompileError::UnknownLoopLabel(label.to_string()));
                        };
                        Some(label)
                    }
                };
                Ok(Some(Stmt::new(StmtNode::Continue { exit: label }, line)))
            }
            Rule::return_statement => {
                let mut parts = pair.into_inner();
                let expr = parts
                    .next()
                    .map(|expr| self.parse_expr(expr.into_inner()).unwrap());
                Ok(Some(Stmt::new(StmtNode::Return(expr), line)))
            }
            Rule::for_statement => {
                self.enter_scope();
                let mut parts = pair.into_inner();
                let id = self
                    .clone()
                    .names
                    .borrow_mut()
                    .find_or_add_name_global(parts.next().unwrap().as_str());
                let clause = parts.next().unwrap();
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
                                id,
                                from,
                                to,
                                body,
                                environment_width,
                            },
                            line,
                        )))
                    }
                    Rule::for_in_clause => {
                        let mut clause_inner = clause.into_inner();
                        let in_rule = clause_inner.next().unwrap();
                        let expr = self.clone().parse_expr(in_rule.into_inner())?;
                        let environment_width = self.exit_scope();
                        Ok(Some(Stmt::new(
                            StmtNode::ForList {
                                id,
                                expr,
                                body,
                                environment_width,
                            },
                            line,
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
                    line,
                )))
            }
            Rule::try_except_statement => {
                self.enter_scope();
                let mut parts = pair.into_inner();
                let body = self
                    .clone()
                    .parse_statements(parts.next().unwrap().into_inner())?;
                let mut excepts = vec![];
                for except in parts {
                    match except.as_rule() {
                        Rule::except => {
                            let mut except_clause_parts = except.into_inner();
                            let clause = except_clause_parts.next().unwrap();
                            let (id, codes) = match clause.as_rule() {
                                Rule::labelled_except => {
                                    let mut my_parts = clause.into_inner();
                                    let exception = my_parts.next().map(|id| {
                                        self.names.borrow_mut().find_or_add_name_global(id.as_str())
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
                let environment_width = self.exit_scope();
                Ok(Some(Stmt::new(
                    StmtNode::TryExcept {
                        body,
                        excepts,
                        environment_width,
                    },
                    line,
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
                    line,
                )))
            }
            Rule::labelled_fork_statement => {
                let mut parts = pair.into_inner();
                let id = self
                    .names
                    .borrow_mut()
                    .find_or_add_name_global(parts.next().unwrap().as_str());
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
                    line,
                )))
            }
            Rule::begin_statement => {
                if !self.options.lexical_scopes {
                    return Err(CompileError::ParseError(
                        "begin block when lexical scopes not enabled".to_string(),
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
                    line,
                )))
            }
            Rule::local_assignment => {
                if !self.options.lexical_scopes {
                    return Err(CompileError::ParseError(
                        "local assignment when lexical scopes not enabled".to_string(),
                    ));
                }

                // An assignment declaration that introduces a locally lexically scoped variable.
                // May be of form `let x = expr` or just `let x`
                let mut parts = pair.into_inner();
                let id = self
                    .names
                    .borrow_mut()
                    .declare_name(parts.next().unwrap().as_str());
                let expr = parts
                    .next()
                    .map(|e| self.parse_expr(e.into_inner()).unwrap());

                // Just becomes an assignment expression.
                // But that means the decompiler will need to know what to do with it.
                // Which is: if assignment is on its own in statement, and variable assigned to is
                //   restricted to the scope of the block, then it's a let.
                Ok(Some(Stmt::new(
                    StmtNode::Expr(Expr::Assign {
                        left: Box::new(Expr::Id(id)),
                        right: Box::new(expr.unwrap_or(Expr::Value(v_none()))),
                    }),
                    line,
                )))
            }
            Rule::global_assignment => {
                if !self.options.lexical_scopes {
                    return Err(CompileError::ParseError(
                        "global assignment when lexical scopes not enabled".to_string(),
                    ));
                }

                // An explicit global-declaration.
                // global x, or global x = y
                let mut parts = pair.into_inner();
                let id = self
                    .names
                    .borrow_mut()
                    .find_or_add_name_global(parts.next().unwrap().as_str());
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
                    line,
                )))
            }
            _ => panic!("Unimplemented statement: {:?}", pair.as_rule()),
        }
    }

    fn parse_statements(
        self: Rc<Self>,
        pairs: pest::iterators::Pairs<Rule>,
    ) -> Result<Vec<Stmt>, CompileError> {
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

    fn compile(self: Rc<Self>, pairs: pest::iterators::Pairs<Rule>) -> Result<Parse, CompileError> {
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
        let names = self.names.borrow_mut();
        // Annotate the "true" line numbers of the AST nodes.
        annotate_line_numbers(1, &mut program);

        let (bound_names, names_mapping) = names.bind();

        Ok(Parse {
            stmts: program,
            unbound_names: names.clone(),
            names: bound_names,
            names_mapping,
        })
    }

    fn enter_scope(&self) {
        if self.options.lexical_scopes {
            self.names.borrow_mut().push_scope();
        }
    }

    fn exit_scope(&self) -> usize {
        if self.options.lexical_scopes {
            return self.names.borrow_mut().pop_scope();
        }
        0
    }
}

/// The emitted parse tree from the parse phase of the compiler.
#[derive(Debug)]
pub struct Parse {
    pub stmts: Vec<Stmt>,
    pub unbound_names: UnboundNames,
    pub names: Names,
    pub names_mapping: HashMap<UnboundName, Name>,
}

#[instrument(skip(program_text))]
pub fn parse_program(program_text: &str, options: CompileOptions) -> Result<Parse, CompileError> {
    let pairs = match MooParser::parse(Rule::program, program_text) {
        Ok(pairs) => pairs,
        Err(e) => {
            let msg = format!("Parse error: {}", e);
            return Err(CompileError::ParseError(msg));
        }
    };

    // TODO: this is in Rc because of borrowing issues in the Pratt parser
    let tree_transform = TreeTransformer::new(options);
    tree_transform.compile(pairs)
}

// Lex a simpe MOO string literal.  Expectation is:
//   " and " at beginning and end
//   \" is "
//   \\ is \
//   \n is just n
// That's it. MOO has no tabs, newlines, etc. quoting.
pub fn unquote_str(s: &str) -> Result<String, CompileError> {
    let mut output = String::new();
    let mut chars = s.chars().peekable();
    let Some('"') = chars.next() else {
        return Err(CompileError::StringLexError(
            "Expected \" at beginning of string".to_string(),
        ));
    };
    // Proceed until second-last. Last has to be '"'
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some('\\') => output.push('\\'),
                Some('"') => output.push('"'),
                Some(c) => output.push(c),
                None => {
                    return Err(CompileError::StringLexError(
                        "Unexpected end of string".to_string(),
                    ))
                }
            },
            '"' => {
                if chars.peek().is_some() {
                    return Err(CompileError::StringLexError(
                        "Unexpected \" in string".to_string(),
                    ));
                }
                return Ok(output);
            }
            c => output.push(c),
        }
    }
    Err(CompileError::StringLexError(
        "Unexpected end of string".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use moor_values::var::Error::{E_INVARG, E_PROPNF, E_VARNF};
    use moor_values::var::{v_err, v_float, v_int, v_obj, v_str};
    use moor_values::var::{v_none, Symbol};

    use crate::ast::Arg::{Normal, Splice};
    use crate::ast::Expr::{Call, Id, Prop, Value, Verb};
    use crate::ast::{
        BinaryOp, CatchCodes, CondArm, ElseArm, ExceptArm, Expr, ScatterItem, ScatterKind, Stmt,
        StmtNode, UnaryOp,
    };
    use crate::parse::{parse_program, unquote_str};
    use crate::CompileOptions;
    use moor_values::model::CompileError;

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
            vec![StmtNode::Return(Some(Value(v_float(1e-9))))]
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
                location: Box::new(Value(v_obj(0))),
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
        let a = parse.unbound_names.find_name("a").unwrap();

        assert_eq!(parse.stmts.len(), 1);
        assert_eq!(
            stripped_stmts(&parse.stmts)[0],
            StmtNode::Expr(Expr::Assign {
                left: Box::new(Id(a)),
                right: Box::new(Expr::Binary(
                    BinaryOp::Add,
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

    #[test]
    fn test_parse_if_stmt() {
        let program = "if (1 == 2) return 5; elseif (2 == 3) return 3; else return 6; endif";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(parse.stmts.len(), 1);
        assert_eq!(
            stripped_stmts(&parse.stmts)[0],
            StmtNode::Cond {
                arms: vec![
                    CondArm {
                        condition: Expr::Binary(
                            BinaryOp::Eq,
                            Box::new(Value(v_int(1))),
                            Box::new(Value(v_int(2))),
                        ),
                        statements: vec![Stmt {
                            node: StmtNode::Return(Some(Value(v_int(5)))),
                            parser_line_no: 1,
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
                            node: StmtNode::Return(Some(Value(v_int(3)))),
                            parser_line_no: 1,
                            tree_line_no: 4,
                        }],
                    },
                ],

                otherwise: Some(ElseArm {
                    statements: vec![Stmt {
                        node: StmtNode::Return(Some(Value(v_int(6)))),
                        parser_line_no: 1,
                        tree_line_no: 6,
                    }],
                    environment_width: 0,
                }),
            }
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
        assert_eq!(
            stripped_stmts(&parse.stmts)[0],
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
                            node: StmtNode::Return(Some(Value(v_int(5)))),
                            parser_line_no: 3,
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
                            node: StmtNode::Return(Some(Value(v_int(3)))),
                            parser_line_no: 5,
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
                            node: StmtNode::Return(Some(Value(v_int(4)))),
                            parser_line_no: 7,
                            tree_line_no: 6,
                        }],
                    },
                ],

                otherwise: Some(ElseArm {
                    statements: vec![Stmt {
                        node: StmtNode::Return(Some(Value(v_int(6)))),
                        parser_line_no: 9,
                        tree_line_no: 8,
                    }],
                    environment_width: 0,
                }),
            }
        );
    }

    #[test]
    fn test_not_precedence() {
        let program = "return !(#2:move(5));";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(parse.stmts.len(), 1);
        assert_eq!(
            stripped_stmts(&parse.stmts)[0],
            StmtNode::Return(Some(Expr::Unary(
                UnaryOp::Not,
                Box::new(Verb {
                    location: Box::new(Value(v_obj(2))),
                    verb: Box::new(Value(v_str("move"))),
                    args: vec![Normal(Value(v_int(5)))],
                })
            )))
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
        assert_eq!(parse.stmts.len(), 1);
        assert_eq!(
            stripped_stmts(&parse.stmts)[0],
            StmtNode::Cond {
                arms: vec![CondArm {
                    environment_width: 0,
                    condition: Expr::Unary(
                        UnaryOp::Not,
                        Box::new(Verb {
                            location: Box::new(Prop {
                                location: Box::new(Value(v_obj(0))),
                                property: Box::new(Value(v_str("network"))),
                            }),
                            verb: Box::new(Value(v_str("is_connected"))),
                            args: vec![Normal(Id(parse.unbound_names.find_name("this").unwrap())),],
                        })
                    ),
                    statements: vec![Stmt {
                        node: StmtNode::Return(None),
                        parser_line_no: 3,
                        tree_line_no: 2,
                    }],
                }],
                otherwise: None,
            }
        );
    }

    #[test]
    fn test_parse_for_loop() {
        let program = "for x in ({1,2,3}) b = x + 5; endfor";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        let x = parse.unbound_names.find_name("x").unwrap();
        let b = parse.unbound_names.find_name("b").unwrap();
        assert_eq!(parse.stmts.len(), 1);
        assert_eq!(
            stripped_stmts(&parse.stmts)[0],
            StmtNode::ForList {
                environment_width: 0,
                id: x,
                expr: Expr::List(vec![
                    Normal(Value(v_int(1))),
                    Normal(Value(v_int(2))),
                    Normal(Value(v_int(3))),
                ]),
                body: vec![Stmt {
                    node: StmtNode::Expr(Expr::Assign {
                        left: Box::new(Id(b)),
                        right: Box::new(Expr::Binary(
                            BinaryOp::Add,
                            Box::new(Id(x)),
                            Box::new(Value(v_int(5))),
                        )),
                    }),
                    parser_line_no: 1,
                    tree_line_no: 2,
                }],
            }
        )
    }

    #[test]
    fn test_parse_for_range() {
        let program = "for x in [1..5] b = x + 5; endfor";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        assert_eq!(parse.stmts.len(), 1);
        let x = parse.unbound_names.find_name("x").unwrap();
        let b = parse.unbound_names.find_name("b").unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts)[0],
            StmtNode::ForRange {
                environment_width: 0,
                id: x,
                from: Value(v_int(1)),
                to: Value(v_int(5)),
                body: vec![Stmt {
                    node: StmtNode::Expr(Expr::Assign {
                        left: Box::new(Id(b)),
                        right: Box::new(Expr::Binary(
                            BinaryOp::Add,
                            Box::new(Id(x)),
                            Box::new(Value(v_int(5))),
                        )),
                    }),
                    parser_line_no: 1,
                    tree_line_no: 2,
                }],
            }
        )
    }

    #[test]
    fn test_indexed_range_len() {
        let program = "a = {1, 2, 3}; b = a[2..$];";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        let (a, b) = (
            parse.unbound_names.find_name("a").unwrap(),
            parse.unbound_names.find_name("b").unwrap(),
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
        let x = parse.unbound_names.find_name("x").unwrap();

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
                                BinaryOp::Add,
                                Box::new(Id(x)),
                                Box::new(Value(v_int(1))),
                            )),
                        }),
                        parser_line_no: 1,
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
                                    parser_line_no: 1,
                                    tree_line_no: 4,
                                }],
                            }],
                            otherwise: None,
                        },
                        parser_line_no: 1,
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
        let chuckles = parse.unbound_names.find_name("chuckles").unwrap();
        let x = parse.unbound_names.find_name("x").unwrap();

        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::While {
                environment_width: 0,
                id: Some(chuckles),
                condition: Value(v_int(1)),
                body: vec![
                    Stmt {
                        node: StmtNode::Expr(Expr::Assign {
                            left: Box::new(Id(x)),
                            right: Box::new(Expr::Binary(
                                BinaryOp::Add,
                                Box::new(Id(x)),
                                Box::new(Value(v_int(1))),
                            )),
                        }),
                        parser_line_no: 1,
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
                                        exit: Some(chuckles)
                                    },
                                    parser_line_no: 1,
                                    tree_line_no: 4,
                                }],
                            }],
                            otherwise: None,
                        },
                        parser_line_no: 1,
                        tree_line_no: 3,
                    },
                ],
            }]
        )
    }

    #[test]
    fn test_sysobjref() {
        let program = "$string_utils:from_list(test_string);";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        let test_string = parse.unbound_names.find_name("test_string").unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Verb {
                location: Box::new(Prop {
                    location: Box::new(Value(v_obj(0))),
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
        let connection = parse.unbound_names.find_name("connection").unwrap();
        let args = parse.unbound_names.find_name("args").unwrap();

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
        let connection = parse.unbound_names.find_name("connection").unwrap();
        let args = parse.unbound_names.find_name("args").unwrap();

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
        let a = parse.unbound_names.find_name("a").unwrap();
        let b = parse.unbound_names.find_name("b").unwrap();
        let c = parse.unbound_names.find_name("c").unwrap();
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
        let a = parse.unbound_names.find_name("a").unwrap();
        let b = parse.unbound_names.find_name("b").unwrap();
        let c = parse.unbound_names.find_name("c").unwrap();
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
        let this = parse.unbound_names.find_name("this").unwrap();
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
        let i = parse.unbound_names.find_name("i").unwrap();
        // Verify the structure of the syntax tree for a for-list loop.
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![
                StmtNode::ForList {
                    environment_width: 0,
                    id: i,
                    expr: Expr::List(vec![
                        Normal(Value(v_int(1))),
                        Normal(Value(v_int(2))),
                        Normal(Value(v_int(3))),
                    ]),
                    body: vec![],
                },
                StmtNode::Return(Some(Id(i))),
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
                        id: parse.unbound_names.find_name("a").unwrap(),
                        expr: None,
                    },
                    ScatterItem {
                        kind: ScatterKind::Required,
                        id: parse.unbound_names.find_name("b").unwrap(),
                        expr: None,
                    },
                    ScatterItem {
                        kind: ScatterKind::Required,
                        id: parse.unbound_names.find_name("c").unwrap(),
                        expr: None,
                    },
                ],
                Box::new(Id(parse.unbound_names.find_name("args").unwrap())),
            ))]
        );
    }

    #[test]
    fn test_valid_underscore_and_no_underscore_ident() {
        let program = "_house == home;";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        let house = parse.unbound_names.find_name("_house").unwrap();
        let home = parse.unbound_names.find_name("home").unwrap();
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
        let program = "return {@results, frozzbozz(@args)};";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        let results = parse.unbound_names.find_name("results").unwrap();
        let args = parse.unbound_names.find_name("args").unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Return(Some(Expr::List(vec![
                Splice(Id(results)),
                Normal(Call {
                    function: Symbol::mk("frozzbozz"),
                    args: vec![Splice(Id(args))],
                }),
            ])))]
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
        let a = parse.unbound_names.find_name("a").unwrap();
        let info = parse.unbound_names.find_name("info").unwrap();
        let forgotten = parse.unbound_names.find_name("forgotten").unwrap();

        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![
                StmtNode::ForList {
                    environment_width: 0,
                    id: a,
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
                            node: StmtNode::Return(Some(Value(v_int(5)))),
                            parser_line_no: 2,
                            tree_line_no: 2,
                        }],
                    }],
                    otherwise: Some(ElseArm {
                        statements: vec![Stmt {
                            node: StmtNode::Return(Some(Value(v_int(3)))),
                            parser_line_no: 4,
                            tree_line_no: 4,
                        }],
                        environment_width: 0,
                    }),
                },
                parser_line_no: 1,
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
                            node: StmtNode::Return(Some(Value(v_int(5)))),
                            parser_line_no: 2,
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
                            node: StmtNode::Return(Some(Value(v_int(2)))),
                            parser_line_no: 4,
                            tree_line_no: 4,
                        }],
                    },
                ],
                otherwise: Some(ElseArm {
                    statements: vec![Stmt {
                        node: StmtNode::Return(Some(Value(v_int(3)))),
                        parser_line_no: 6,
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
                    parser_line_no: 2,
                    tree_line_no: 2,
                }],
                excepts: vec![ExceptArm {
                    id: None,
                    codes: CatchCodes::Codes(vec![Normal(Value(v_err(E_PROPNF)))]),
                    statements: vec![Stmt {
                        node: StmtNode::Return(None),
                        parser_line_no: 4,
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
        let a = parse.unbound_names.find_name("a").unwrap();
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
                location: Box::new(Id(parse.unbound_names.find_name("this").unwrap())),
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
                location: Box::new(Id(parse.unbound_names.find_name("this").unwrap())),
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
        let program = "(2 <= (len = length(text)));";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        let len = parse.unbound_names.find_name("len").unwrap();
        let text = parse.unbound_names.find_name("text").unwrap();
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
                left: Box::new(Id(parse.unbound_names.find_name("a").unwrap())),
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
                    .unbound_names
                    .find_name("what")
                    .unwrap())),])),
                Box::new(Id(parse.unbound_names.find_name("args").unwrap())),
            ))]
        );
    }

    #[test]
    fn test_raise_bf_call_incorrect_err() {
        // detect ambiguous match on E_PERMS != E_PERM
        let program = "raise(E_PERMS);";
        let parse = parse_program(program, CompileOptions::default()).unwrap();

        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Call {
                function: Symbol::mk("raise"),
                args: vec![Normal(Id(parse
                    .unbound_names
                    .find_name("E_PERMS")
                    .unwrap()))]
            })]
        );
    }

    #[test]
    fn test_keyword_disambig_call() {
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
                    id: parse.unbound_names.find_name("line").unwrap(),
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

        let varnf = Normal(Value(v_err(E_VARNF)));
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Return(Some(Expr::List(vec![Normal(
                Expr::TryCatch {
                    trye: Box::new(Id(parse.unbound_names.find_name("x").unwrap())),
                    codes: CatchCodes::Codes(vec![varnf]),
                    except: Some(Box::new(Value(v_int(666)))),
                }
            )],)))]
        )
    }

    #[test]
    fn try_catch_any_expr() {
        let program = "`raise(E_INVARG) ! ANY';";
        let parse = parse_program(program, CompileOptions::default()).unwrap();
        let invarg = Normal(Value(v_err(E_INVARG)));

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
                        location: Box::new(Value(v_obj(0))),
                        property: Box::new(Value(v_str("ftp_client"))),
                    }),
                    verb: Box::new(Value(v_str("finish_get"))),
                    args: vec![Normal(Prop {
                        location: Box::new(Id(parse.unbound_names.find_name("this").unwrap())),
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
                    left: Box::new(Id(parse.unbound_names.find_name("result").unwrap())),
                    right: Box::new(Expr::Pass {
                        args: vec![Splice(Id(parse.unbound_names.find_name("args").unwrap()))],
                    }),
                }),
                StmtNode::Expr(Expr::Assign {
                    left: Box::new(Id(parse.unbound_names.find_name("result").unwrap())),
                    right: Box::new(Expr::Pass { args: vec![] }),
                }),
                StmtNode::Expr(Expr::Assign {
                    left: Box::new(Id(parse.unbound_names.find_name("result").unwrap())),
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
                    left: Box::new(Id(parse.unbound_names.find_name("pass").unwrap())),
                    right: Box::new(Id(parse.unbound_names.find_name("blop").unwrap())),
                }),
                StmtNode::Return(Some(Id(parse.unbound_names.find_name("pass").unwrap()))),
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
        assert!(matches!(parse, Err(CompileError::UnknownLoopLabel(_))));

        let program = r#"
            while (1)
                continue unknown;
            endwhile"#;
        let parse = parse_program(program, CompileOptions::default());
        assert!(matches!(parse, Err(CompileError::UnknownLoopLabel(_))));
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
                    node: StmtNode::Return(Some(Value(v_int(5)))),
                    parser_line_no: 2,
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
        let x_names = parse.unbound_names.find_named("x");
        let y_names = parse.unbound_names.find_named("y");
        let z_names = parse.unbound_names.find_named("z");
        let o_names = parse.unbound_names.find_named("o");
        let inner_y = y_names[0];
        let inner_z = z_names[0];
        let inner_o = o_names[0];
        let global_a = parse.unbound_names.find_named("a")[0];
        assert_eq!(x_names.len(), 2);
        let global_x = x_names[1];
        // Declared first, so appears in unbound names first, though in the bound names it will
        // appear second.
        let inner_x = x_names[0];
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![
                StmtNode::Scope {
                    num_bindings: 4,
                    body: vec![
                        // Declaration of X
                        Stmt {
                            node: StmtNode::Expr(Expr::Assign {
                                left: Box::new(Id(inner_x)),
                                right: Box::new(Value(v_int(5))),
                            }),
                            parser_line_no: 2,
                            tree_line_no: 2,
                        },
                        // Declaration of y
                        Stmt {
                            node: StmtNode::Expr(Expr::Assign {
                                left: Box::new(Id(inner_y)),
                                right: Box::new(Value(v_int(6))),
                            }),
                            parser_line_no: 3,
                            tree_line_no: 3,
                        },
                        Stmt {
                            node: StmtNode::Expr(Expr::Assign {
                                left: Box::new(Id(inner_x)),
                                right: Box::new(Expr::Binary(
                                    BinaryOp::Add,
                                    Box::new(Id(inner_x)),
                                    Box::new(Value(v_int(6))),
                                )),
                            }),
                            parser_line_no: 4,
                            tree_line_no: 4,
                        },
                        // Asssignment to z.
                        Stmt {
                            node: StmtNode::Expr(Expr::Assign {
                                left: Box::new(Id(inner_z)),
                                right: Box::new(Value(v_int(7))),
                            }),
                            parser_line_no: 5,
                            tree_line_no: 5,
                        },
                        // Declaration of o (o = v_none)
                        Stmt {
                            node: StmtNode::Expr(Expr::Assign {
                                left: Box::new(Id(inner_o)),
                                right: Box::new(Value(v_none())),
                            }),
                            parser_line_no: 6,
                            tree_line_no: 6,
                        },
                        // Assignment to global a
                        Stmt {
                            node: StmtNode::Expr(Expr::Assign {
                                left: Box::new(Id(global_a)),
                                right: Box::new(Value(v_int(1))),
                            }),
                            parser_line_no: 7,
                            tree_line_no: 7,
                        },
                    ],
                },
                StmtNode::Return(Some(Id(global_x)))
            ]
        );
    }
}
