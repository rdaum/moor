/// Kicks off the Pest parser and converts it into our AST.
/// This is the main entry point for parsing.
use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;

use moor_values::SYSTEM_OBJECT;
use pest::pratt_parser::{Assoc, Op, PrattParser};
pub use pest::Parser as PestParser;
use tracing::instrument;

use moor_values::var::error::Error::{
    E_ARGS, E_DIV, E_FLOAT, E_INVARG, E_INVIND, E_MAXREC, E_NACC, E_NONE, E_PERM, E_PROPNF,
    E_QUOTA, E_RANGE, E_RECMOVE, E_TYPE, E_VARNF, E_VERBNF,
};
use moor_values::var::objid::Objid;
use moor_values::var::{v_err, v_float, v_int, v_objid, v_str, v_string};

use crate::compiler::ast::Arg::{Normal, Splice};
use crate::compiler::ast::{
    Arg, BinaryOp, CatchCodes, CondArm, ExceptArm, Expr, ScatterItem, ScatterKind, Stmt, StmtNode,
    UnaryOp,
};
use crate::compiler::labels::Names;
use crate::compiler::parse::moo::{MooParser, Rule};
use crate::compiler::unparse::annotate_line_numbers;
use crate::compiler::CompileError;

pub mod moo {
    #[derive(Parser)]
    #[grammar = "src/compiler/moo.pest"]
    pub struct MooParser;
}

/// The emitted parse tree from the parse phase of the compiler.
pub struct Parse {
    pub stmts: Vec<Stmt>,
    pub names: Names,
}

fn parse_atom(
    names: Rc<RefCell<Names>>,
    pairs: pest::iterators::Pair<Rule>,
) -> Result<Expr, CompileError> {
    match pairs.as_rule() {
        Rule::ident => {
            let name = names.borrow_mut().find_or_add_name(pairs.as_str().trim());
            Ok(Expr::Id(name))
        }
        Rule::object => {
            let ostr = &pairs.as_str()[1..];
            let oid = i64::from_str(ostr).unwrap();
            let objid = Objid(oid);
            Ok(Expr::VarExpr(v_objid(objid)))
        }
        Rule::integer => {
            let int = pairs.as_str().parse::<i64>().unwrap();
            Ok(Expr::VarExpr(v_int(int)))
        }
        Rule::float => {
            let float = pairs.as_str().parse::<f64>().unwrap();
            Ok(Expr::VarExpr(v_float(float)))
        }
        Rule::string => {
            let string = pairs.as_str();
            let parsed = unquote_str(string)?;
            Ok(Expr::VarExpr(v_str(&parsed)))
        }
        Rule::err => {
            let e = pairs.as_str();
            Ok(Expr::VarExpr(match e.to_lowercase().as_str() {
                "e_type" => v_err(E_TYPE),
                "e_div" => v_err(E_DIV),
                "e_perm" => v_err(E_PERM),
                "e_propnf" => v_err(E_PROPNF),
                "e_verbnf" => v_err(E_VERBNF),
                "e_varnf" => v_err(E_VARNF),
                "e_invind" => v_err(E_INVIND),
                "e_recmove" => v_err(E_RECMOVE),
                "e_maxrec" => v_err(E_MAXREC),
                "e_range" => v_err(E_RANGE),
                "e_args" => v_err(E_ARGS),
                "e_nacc" => v_err(E_NACC),
                "e_invarg" => v_err(E_INVARG),
                "e_quota" => v_err(E_QUOTA),
                "e_float" => v_err(E_FLOAT),
                "e_none" => v_err(E_NONE),
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
    names: Rc<RefCell<Names>>,
    pairs: pest::iterators::Pairs<Rule>,
) -> Result<Vec<Arg>, CompileError> {
    let mut args = vec![];
    for pair in pairs {
        match pair.as_rule() {
            Rule::argument => {
                let arg = if pair.as_str().starts_with('@') {
                    Splice(parse_expr(
                        names.clone(),
                        pair.into_inner().next().unwrap().into_inner(),
                    )?)
                } else {
                    Normal(parse_expr(
                        names.clone(),
                        pair.into_inner().next().unwrap().into_inner(),
                    )?)
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
    names: Rc<RefCell<Names>>,
    pairs: pest::iterators::Pairs<Rule>,
) -> Result<Vec<Arg>, CompileError> {
    for pair in pairs {
        match pair.as_rule() {
            Rule::exprlist => {
                return parse_exprlist(names, pair.into_inner());
            }
            _ => {
                panic!("Unimplemented arglist: {:?}", pair);
            }
        }
    }
    Ok(vec![])
}

fn parse_except_codes(
    names: Rc<RefCell<Names>>,
    pairs: pest::iterators::Pair<Rule>,
) -> Result<CatchCodes, CompileError> {
    match pairs.as_rule() {
        Rule::anycode => Ok(CatchCodes::Any),
        Rule::exprlist => Ok(CatchCodes::Codes(parse_exprlist(
            names,
            pairs.into_inner(),
        )?)),
        _ => {
            panic!("Unimplemented except_codes: {:?}", pairs);
        }
    }
}

fn parse_expr(
    names: Rc<RefCell<Names>>,
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
        // 4. Add & subtract same precedence
        .op(Op::infix(Rule::add, Assoc::Left) | Op::infix(Rule::sub, Assoc::Left))
        // 3. * / % all same precedence
        .op(Op::infix(Rule::mul, Assoc::Left)
            | Op::infix(Rule::div, Assoc::Left)
            | Op::infix(Rule::modulus, Assoc::Left))
        // Exponent is higher than multiply/divide (not present in C)
        .op(Op::infix(Rule::pow, Assoc::Left))
        // Not sure if this is correct
        .op(Op::infix(Rule::in_range, Assoc::Left))
        // 2. Unary negation & logical-not
        .op(Op::prefix(Rule::neg) | Op::prefix(Rule::not))
        // 1. Indexing/suffix operator generally.
        .op(Op::postfix(Rule::index_range)
            | Op::postfix(Rule::index_single)
            | Op::postfix(Rule::verb_call)
            | Op::postfix(Rule::verb_expr_call)
            | Op::postfix(Rule::prop)
            | Op::postfix(Rule::prop_expr));

    return pratt
        .map_primary(|primary| match primary.as_rule() {
            Rule::atom => {
                let mut inner = primary.into_inner();
                let expr = parse_atom(names.clone(), inner.next().unwrap())?;
                Ok(expr)
            }
            Rule::sysprop => {
                let mut inner = primary.into_inner();
                let property = inner.next().unwrap().as_str();
                Ok(Expr::Prop {
                    location: Box::new(Expr::VarExpr(v_objid(SYSTEM_OBJECT))),
                    property: Box::new(Expr::VarExpr(v_str(property))),
                })
            }
            Rule::sysprop_call => {
                let mut inner = primary.into_inner();
                let verb = inner.next().unwrap().as_str()[1..].to_string();
                let args = parse_arglist(names.clone(), inner.next().unwrap().into_inner())?;
                Ok(Expr::Verb {
                    location: Box::new(Expr::VarExpr(v_objid(SYSTEM_OBJECT))),
                    verb: Box::new(Expr::VarExpr(v_string(verb))),
                    args,
                })
            }
            Rule::list => {
                let mut inner = primary.into_inner();
                if let Some(arglist) = inner.next() {
                    let args = parse_exprlist(names.clone(), arglist.into_inner())?;
                    Ok(Expr::List(args))
                } else {
                    Ok(Expr::List(vec![]))
                }
            }
            Rule::builtin_call => {
                let mut inner = primary.into_inner();
                let bf = inner.next().unwrap().as_str();
                let args = parse_arglist(names.clone(), inner.next().unwrap().into_inner())?;
                Ok(Expr::Call {
                    function: bf.to_string(),
                    args,
                })
            }
            Rule::pass_expr => {
                let mut inner = primary.into_inner();
                let args = if let Some(arglist) = inner.next() {
                    parse_exprlist(names.clone(), arglist.into_inner())?
                } else {
                    vec![]
                };
                Ok(Expr::Pass { args })
            }
            Rule::range_end => Ok(Expr::Length),
            Rule::try_expr => {
                let mut inner = primary.into_inner();
                let try_expr = parse_expr(names.clone(), inner.next().unwrap().into_inner())?;
                let codes = inner.next().unwrap();
                let catch_codes =
                    parse_except_codes(names.clone(), codes.into_inner().next().unwrap())?;
                let except = inner
                    .next()
                    .map(|e| Box::new(parse_expr(names.clone(), e.into_inner()).unwrap()));
                Ok(Expr::Catch {
                    trye: Box::new(try_expr),
                    codes: catch_codes,
                    except,
                })
            }

            Rule::paren_expr => {
                let mut inner = primary.into_inner();
                let expr = parse_expr(names.clone(), inner.next().unwrap().into_inner())?;
                Ok(expr)
            }
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
                BinaryOp::Eq,
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
                            let id = names.borrow_mut().find_or_add_name(id);
                            let expr = inner
                                .next()
                                .map(|e| parse_expr(names.clone(), e.into_inner()).unwrap());
                            items.push(ScatterItem {
                                kind: ScatterKind::Optional,
                                id,
                                expr,
                            });
                        }
                        Rule::scatter_target => {
                            let mut inner = scatter_item.into_inner();
                            let id = inner.next().unwrap().as_str();
                            let id = names.borrow_mut().find_or_add_name(id);
                            items.push(ScatterItem {
                                kind: ScatterKind::Required,
                                id,
                                expr: None,
                            });
                        }
                        Rule::scatter_rest => {
                            let mut inner = scatter_item.into_inner();
                            let id = inner.next().unwrap().as_str();
                            let id = names.borrow_mut().find_or_add_name(id);
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
                let args = parse_arglist(names.clone(), args_expr.into_inner())?;
                Ok(Expr::Verb {
                    location: Box::new(lhs?),
                    verb: Box::new(Expr::VarExpr(v_str(ident))),
                    args,
                })
            }
            Rule::verb_expr_call => {
                let mut parts = op.into_inner();
                let expr = parse_expr(names.clone(), parts.next().unwrap().into_inner())?;
                let args_expr = parts.next().unwrap();
                let args = parse_arglist(names.clone(), args_expr.into_inner())?;
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
                    property: Box::new(Expr::VarExpr(v_str(ident))),
                })
            }
            Rule::prop_expr => {
                let mut parts = op.into_inner();
                let expr = parse_expr(names.clone(), parts.next().unwrap().into_inner())?;
                Ok(Expr::Prop {
                    location: Box::new(lhs?),
                    property: Box::new(expr),
                })
            }
            Rule::assign => {
                let mut parts = op.into_inner();
                let right = parse_expr(names.clone(), parts.next().unwrap().into_inner())?;
                Ok(Expr::Assign {
                    left: Box::new(lhs?),
                    right: Box::new(right),
                })
            }
            Rule::index_single => {
                let mut parts = op.into_inner();
                let index = parse_expr(names.clone(), parts.next().unwrap().into_inner())?;
                Ok(Expr::Index(Box::new(lhs?), Box::new(index)))
            }
            Rule::index_range => {
                let mut parts = op.into_inner();
                let start = parse_expr(names.clone(), parts.next().unwrap().into_inner())?;
                let end = parse_expr(names.clone(), parts.next().unwrap().into_inner())?;
                Ok(Expr::Range {
                    base: Box::new(lhs?),
                    from: Box::new(start),
                    to: Box::new(end),
                })
            }
            Rule::cond_expr => {
                let mut parts = op.into_inner();
                let true_expr = parse_expr(names.clone(), parts.next().unwrap().into_inner())?;
                let false_expr = parse_expr(names.clone(), parts.next().unwrap().into_inner())?;
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
    names: Rc<RefCell<Names>>,
    pair: pest::iterators::Pair<Rule>,
) -> Result<Option<Stmt>, CompileError> {
    let line = pair.line_col().0;
    match pair.as_rule() {
        Rule::expr_statement => {
            let mut inner = pair.into_inner();
            if let Some(rule) = inner.next() {
                let expr = parse_expr(names, rule.into_inner())?;
                return Ok(Some(Stmt::new(StmtNode::Expr(expr), line)));
            }
            Ok(None)
        }
        Rule::while_statement => {
            let mut parts = pair.into_inner();
            let condition = parse_expr(names.clone(), parts.next().unwrap().into_inner())?;
            let body = parse_statements(names, parts.next().unwrap().into_inner())?;
            Ok(Some(Stmt::new(
                StmtNode::While {
                    id: None,
                    condition,
                    body,
                },
                line,
            )))
        }
        Rule::labelled_while_statement => {
            let mut parts = pair.into_inner();
            let id = names
                .borrow_mut()
                .find_or_add_name(parts.next().unwrap().as_str());
            let condition = parse_expr(names.clone(), parts.next().unwrap().into_inner())?;
            let body = parse_statements(names, parts.next().unwrap().into_inner())?;
            Ok(Some(Stmt::new(
                StmtNode::While {
                    id: Some(id),
                    condition,
                    body,
                },
                line,
            )))
        }
        Rule::if_statement => {
            let mut parts = pair.into_inner();
            let mut arms = vec![];
            let mut otherwise = vec![];
            let condition = parse_expr(names.clone(), parts.next().unwrap().into_inner())?;
            let body = parse_statements(names.clone(), parts.next().unwrap().into_inner())?;
            arms.push(CondArm {
                condition,
                statements: body,
            });
            for remainder in parts {
                match remainder.as_rule() {
                    Rule::endif_clause => {
                        continue;
                    }
                    Rule::elseif_clause => {
                        let mut parts = remainder.into_inner();
                        let condition =
                            parse_expr(names.clone(), parts.next().unwrap().into_inner())?;
                        let body =
                            parse_statements(names.clone(), parts.next().unwrap().into_inner())?;
                        arms.push(CondArm {
                            condition,
                            statements: body,
                        });
                    }
                    Rule::else_clause => {
                        let mut parts = remainder.into_inner();
                        otherwise =
                            parse_statements(names.clone(), parts.next().unwrap().into_inner())?;
                    }
                    _ => panic!("Unimplemented if clause: {:?}", remainder),
                }
            }
            Ok(Some(Stmt::new(StmtNode::Cond { arms, otherwise }, line)))
        }
        Rule::break_statement => {
            let mut parts = pair.into_inner();
            let label = parts
                .next()
                .map(|id| names.borrow_mut().find_or_add_name(id.as_str()));
            Ok(Some(Stmt::new(StmtNode::Break { exit: label }, line)))
        }
        Rule::continue_statement => {
            let mut parts = pair.into_inner();
            let label = parts
                .next()
                .map(|id| names.borrow_mut().find_or_add_name(id.as_str()));
            Ok(Some(Stmt::new(StmtNode::Continue { exit: label }, line)))
        }
        Rule::return_statement => {
            let mut parts = pair.into_inner();
            let expr = parts
                .next()
                .map(|expr| parse_expr(names.clone(), expr.into_inner()).unwrap());
            Ok(Some(Stmt::new(StmtNode::Return { expr }, line)))
        }
        Rule::for_statement => {
            let mut parts = pair.into_inner();
            let id = names
                .borrow_mut()
                .find_or_add_name(parts.next().unwrap().as_str());
            let clause = parts.next().unwrap();
            let body = parse_statements(names.clone(), parts.next().unwrap().into_inner())?;
            match clause.as_rule() {
                Rule::for_range_clause => {
                    let mut clause_inner = clause.into_inner();
                    let from_rule = clause_inner.next().unwrap();
                    let to_rule = clause_inner.next().unwrap();
                    let from = parse_expr(names.clone(), from_rule.into_inner())?;
                    let to = parse_expr(names, to_rule.into_inner())?;
                    Ok(Some(Stmt::new(
                        StmtNode::ForRange { id, from, to, body },
                        line,
                    )))
                }
                Rule::for_in_clause => {
                    let mut clause_inner = clause.into_inner();
                    let in_rule = clause_inner.next().unwrap();
                    let expr = parse_expr(names, in_rule.into_inner())?;
                    Ok(Some(Stmt::new(StmtNode::ForList { id, expr, body }, line)))
                }
                _ => panic!("Unimplemented for clause: {:?}", clause),
            }
        }
        Rule::try_finally_statement => {
            let mut parts = pair.into_inner();
            let body = parse_statements(names.clone(), parts.next().unwrap().into_inner())?;
            let handler = parse_statements(names, parts.next().unwrap().into_inner())?;
            Ok(Some(Stmt::new(
                StmtNode::TryFinally { body, handler },
                line,
            )))
        }
        Rule::try_except_statement => {
            let mut parts = pair.into_inner();
            let body = parse_statements(names.clone(), parts.next().unwrap().into_inner())?;
            let mut excepts = vec![];
            for except in parts {
                match except.as_rule() {
                    Rule::except => {
                        let mut except_clause_parts = except.into_inner();
                        let clause = except_clause_parts.next().unwrap();
                        let (id, codes) = match clause.as_rule() {
                            Rule::labelled_except => {
                                let mut my_parts = clause.into_inner();
                                let exception = my_parts
                                    .next()
                                    .map(|id| names.borrow_mut().find_or_add_name(id.as_str()));

                                let codes = parse_except_codes(
                                    names.clone(),
                                    my_parts.next().unwrap().into_inner().next().unwrap(),
                                )?;
                                (exception, codes)
                            }
                            Rule::unlabelled_except => {
                                let mut my_parts = clause.into_inner();
                                let codes = parse_except_codes(
                                    names.clone(),
                                    my_parts.next().unwrap().into_inner().next().unwrap(),
                                )?;
                                (None, codes)
                            }
                            _ => panic!("Unimplemented except clause: {:?}", clause),
                        };
                        let statements = parse_statements(
                            names.clone(),
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
            Ok(Some(Stmt::new(StmtNode::TryExcept { body, excepts }, line)))
        }
        Rule::fork_statement => {
            let mut parts = pair.into_inner();
            let time = parse_expr(names.clone(), parts.next().unwrap().into_inner())?;
            let body = parse_statements(names, parts.next().unwrap().into_inner())?;
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
            let id = names
                .borrow_mut()
                .find_or_add_name(parts.next().unwrap().as_str());
            let time = parse_expr(names.clone(), parts.next().unwrap().into_inner())?;
            let body = parse_statements(names, parts.next().unwrap().into_inner())?;
            Ok(Some(Stmt::new(
                StmtNode::Fork {
                    id: Some(id),
                    time,
                    body,
                },
                line,
            )))
        }
        _ => panic!("Unimplemented statement: {:?}", pair.as_rule()),
    }
}

fn parse_statements(
    names: Rc<RefCell<Names>>,
    pairs: pest::iterators::Pairs<Rule>,
) -> Result<Vec<Stmt>, CompileError> {
    let mut statements = vec![];
    for pair in pairs {
        match pair.as_rule() {
            Rule::statement => {
                let stmt = parse_statement(names.clone(), pair.into_inner().next().unwrap())?;
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

#[instrument(skip(program_text))]
pub fn parse_program(program_text: &str) -> Result<Parse, CompileError> {
    let pairs = match MooParser::parse(Rule::program, program_text) {
        Ok(pairs) => pairs,
        Err(e) => {
            let msg = format!("Parse error: {}", e);
            return Err(CompileError::ParseError(msg));
        }
    };

    let names = Rc::new(RefCell::new(Names::new()));
    let mut program = Vec::new();
    for pair in pairs {
        match pair.as_rule() {
            Rule::program => {
                let inna = pair.into_inner().next().unwrap();

                match inna.as_rule() {
                    Rule::statements => {
                        let parsed_statements = parse_statements(names.clone(), inna.into_inner())?;
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
    let names = names.borrow_mut();
    let names = names.clone();

    // Annotate the "true" line numbers of the AST nodes.
    annotate_line_numbers(1, &mut program);

    Ok(Parse {
        stmts: program,
        names,
    })
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
    use moor_values::var::error::Error::{E_INVARG, E_PROPNF, E_VARNF};
    use moor_values::var::{v_err, v_float, v_int, v_obj, v_str};

    use crate::compiler::ast::Arg::{Normal, Splice};
    use crate::compiler::ast::Expr::{Call, Id, Prop, VarExpr, Verb};
    use crate::compiler::ast::{
        BinaryOp, CatchCodes, CondArm, ExceptArm, Expr, ScatterItem, ScatterKind, Stmt, StmtNode,
        UnaryOp,
    };
    use crate::compiler::labels::Names;
    use crate::compiler::parse::{parse_program, unquote_str};

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
        let parse = parse_program(program).unwrap();
        assert_eq!(parse.stmts.len(), 1);
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Return {
                expr: Some(VarExpr(v_float(1e-9))),
            }]
        );
    }

    #[test]
    fn test_call_verb() {
        let program = r#"#0:test_verb(1,2,3,"test");"#;
        let _names = Names::new();
        let parsed = parse_program(program).unwrap();
        assert_eq!(parsed.stmts.len(), 1);
        assert_eq!(
            stripped_stmts(&parsed.stmts),
            vec![StmtNode::Expr(Verb {
                location: Box::new(VarExpr(v_obj(0))),
                verb: Box::new(VarExpr(v_str("test_verb"))),
                args: vec![
                    Normal(VarExpr(v_int(1))),
                    Normal(VarExpr(v_int(2))),
                    Normal(VarExpr(v_int(3))),
                    Normal(VarExpr(v_str("test")))
                ]
            })]
        );
    }

    #[test]
    fn test_parse_simple_var_assignment_precedence() {
        let program = "a = 1 + 2;";
        let parse = parse_program(program).unwrap();
        let a = parse.names.find_name("a").unwrap();

        assert_eq!(parse.stmts.len(), 1);
        assert_eq!(
            stripped_stmts(&parse.stmts)[0],
            StmtNode::Expr(Expr::Assign {
                left: Box::new(Id(a)),
                right: Box::new(Expr::Binary(
                    BinaryOp::Add,
                    Box::new(VarExpr(v_int(1))),
                    Box::new(VarExpr(v_int(2))),
                )),
            })
        );
    }

    #[test]
    fn test_parse_call_literal() {
        let program = "notify(\"test\");";
        let parse = parse_program(program).unwrap();

        assert_eq!(parse.stmts.len(), 1);
        assert_eq!(
            stripped_stmts(&parse.stmts)[0],
            StmtNode::Expr(Call {
                function: "notify".to_string(),
                args: vec![Normal(VarExpr(v_str("test")))],
            })
        );
    }

    #[test]
    fn test_parse_if_stmt() {
        let program = "if (1 == 2) return 5; elseif (2 == 3) return 3; else return 6; endif";
        let parse = parse_program(program).unwrap();
        assert_eq!(parse.stmts.len(), 1);
        assert_eq!(
            stripped_stmts(&parse.stmts)[0],
            StmtNode::Cond {
                arms: vec![
                    CondArm {
                        condition: Expr::Binary(
                            BinaryOp::Eq,
                            Box::new(VarExpr(v_int(1))),
                            Box::new(VarExpr(v_int(2))),
                        ),
                        statements: vec![Stmt {
                            node: StmtNode::Return {
                                expr: Some(VarExpr(v_int(5))),
                            },
                            parser_line_no: 1,
                            tree_line_no: 2,
                        }],
                    },
                    CondArm {
                        condition: Expr::Binary(
                            BinaryOp::Eq,
                            Box::new(VarExpr(v_int(2))),
                            Box::new(VarExpr(v_int(3))),
                        ),
                        statements: vec![Stmt {
                            node: StmtNode::Return {
                                expr: Some(VarExpr(v_int(3))),
                            },
                            parser_line_no: 1,
                            tree_line_no: 4,
                        }],
                    },
                ],

                otherwise: vec![Stmt {
                    node: StmtNode::Return {
                        expr: Some(VarExpr(v_int(6))),
                    },
                    parser_line_no: 1,
                    tree_line_no: 6,
                }],
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
        let parse = parse_program(program).unwrap();
        assert_eq!(parse.stmts.len(), 1);
        assert_eq!(
            stripped_stmts(&parse.stmts)[0],
            StmtNode::Cond {
                arms: vec![
                    CondArm {
                        condition: Expr::Binary(
                            BinaryOp::Eq,
                            Box::new(VarExpr(v_int(1))),
                            Box::new(VarExpr(v_int(2))),
                        ),
                        statements: vec![Stmt {
                            node: StmtNode::Return {
                                expr: Some(VarExpr(v_int(5))),
                            },
                            parser_line_no: 3,
                            tree_line_no: 2,
                        }],
                    },
                    CondArm {
                        condition: Expr::Binary(
                            BinaryOp::Eq,
                            Box::new(VarExpr(v_int(2))),
                            Box::new(VarExpr(v_int(3))),
                        ),
                        statements: vec![Stmt {
                            node: StmtNode::Return {
                                expr: Some(VarExpr(v_int(3))),
                            },
                            parser_line_no: 5,
                            tree_line_no: 4,
                        }],
                    },
                    CondArm {
                        condition: Expr::Binary(
                            BinaryOp::Eq,
                            Box::new(VarExpr(v_int(3))),
                            Box::new(VarExpr(v_int(4))),
                        ),
                        statements: vec![Stmt {
                            node: StmtNode::Return {
                                expr: Some(VarExpr(v_int(4))),
                            },
                            parser_line_no: 7,
                            tree_line_no: 6,
                        }],
                    },
                ],

                otherwise: vec![Stmt {
                    node: StmtNode::Return {
                        expr: Some(VarExpr(v_int(6))),
                    },
                    parser_line_no: 9,
                    tree_line_no: 8,
                }],
            }
        );
    }

    #[test]
    fn test_not_precedence() {
        let program = "return !(#2:move(5));";
        let parse = parse_program(program).unwrap();
        assert_eq!(parse.stmts.len(), 1);
        assert_eq!(
            stripped_stmts(&parse.stmts)[0],
            StmtNode::Return {
                expr: Some(Expr::Unary(
                    UnaryOp::Not,
                    Box::new(Verb {
                        location: Box::new(VarExpr(v_obj(2))),
                        verb: Box::new(VarExpr(v_str("move"))),
                        args: vec![Normal(VarExpr(v_int(5)))],
                    })
                )),
            }
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
        let parse = parse_program(program).unwrap();
        assert_eq!(parse.stmts.len(), 1);
        assert_eq!(
            stripped_stmts(&parse.stmts)[0],
            StmtNode::Cond {
                arms: vec![CondArm {
                    condition: Expr::Unary(
                        UnaryOp::Not,
                        Box::new(Verb {
                            location: Box::new(Prop {
                                location: Box::new(VarExpr(v_obj(0))),
                                property: Box::new(VarExpr(v_str("network"))),
                            }),
                            verb: Box::new(VarExpr(v_str("is_connected"))),
                            args: vec![Normal(Id(parse.names.find_name("this").unwrap())),],
                        })
                    ),
                    statements: vec![Stmt {
                        node: StmtNode::Return { expr: None },
                        parser_line_no: 3,
                        tree_line_no: 2,
                    }],
                }],
                otherwise: vec![],
            }
        );
    }

    #[test]
    fn test_parse_for_loop() {
        let program = "for x in ({1,2,3}) b = x + 5; endfor";
        let parse = parse_program(program).unwrap();
        let x = parse.names.find_name("x").unwrap();
        let b = parse.names.find_name("b").unwrap();
        assert_eq!(parse.stmts.len(), 1);
        assert_eq!(
            stripped_stmts(&parse.stmts)[0],
            StmtNode::ForList {
                id: x,
                expr: Expr::List(vec![
                    Normal(VarExpr(v_int(1))),
                    Normal(VarExpr(v_int(2))),
                    Normal(VarExpr(v_int(3))),
                ]),
                body: vec![Stmt {
                    node: StmtNode::Expr(Expr::Assign {
                        left: Box::new(Id(b)),
                        right: Box::new(Expr::Binary(
                            BinaryOp::Add,
                            Box::new(Id(x)),
                            Box::new(VarExpr(v_int(5))),
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
        let parse = parse_program(program).unwrap();
        assert_eq!(parse.stmts.len(), 1);
        let x = parse.names.find_name("x").unwrap();
        let b = parse.names.find_name("b").unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts)[0],
            StmtNode::ForRange {
                id: x,
                from: VarExpr(v_int(1)),
                to: VarExpr(v_int(5)),
                body: vec![Stmt {
                    node: StmtNode::Expr(Expr::Assign {
                        left: Box::new(Id(b)),
                        right: Box::new(Expr::Binary(
                            BinaryOp::Add,
                            Box::new(Id(x)),
                            Box::new(VarExpr(v_int(5))),
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
        let parse = parse_program(program).unwrap();
        let (a, b) = (
            parse.names.find_name("a").unwrap(),
            parse.names.find_name("b").unwrap(),
        );
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![
                StmtNode::Expr(Expr::Assign {
                    left: Box::new(Id(a)),
                    right: Box::new(Expr::List(vec![
                        Normal(VarExpr(v_int(1))),
                        Normal(VarExpr(v_int(2))),
                        Normal(VarExpr(v_int(3))),
                    ])),
                }),
                StmtNode::Expr(Expr::Assign {
                    left: Box::new(Id(b)),
                    right: Box::new(Expr::Range {
                        base: Box::new(Id(a)),
                        from: Box::new(VarExpr(v_int(2))),
                        to: Box::new(Expr::Length),
                    }),
                }),
            ]
        );
    }

    #[test]
    fn test_parse_while() {
        let program = "while (1) x = x + 1; if (x > 5) break; endif endwhile";
        let parse = parse_program(program).unwrap();
        let x = parse.names.find_name("x").unwrap();

        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::While {
                id: None,
                condition: VarExpr(v_int(1)),
                body: vec![
                    Stmt {
                        node: StmtNode::Expr(Expr::Assign {
                            left: Box::new(Id(x)),
                            right: Box::new(Expr::Binary(
                                BinaryOp::Add,
                                Box::new(Id(x)),
                                Box::new(VarExpr(v_int(1))),
                            )),
                        }),
                        parser_line_no: 1,
                        tree_line_no: 2,
                    },
                    Stmt {
                        node: StmtNode::Cond {
                            arms: vec![CondArm {
                                condition: Expr::Binary(
                                    BinaryOp::Gt,
                                    Box::new(Id(x)),
                                    Box::new(VarExpr(v_int(5))),
                                ),
                                statements: vec![Stmt {
                                    node: StmtNode::Break { exit: None },
                                    parser_line_no: 1,
                                    tree_line_no: 4,
                                }],
                            }],
                            otherwise: vec![],
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
        let parse = parse_program(program).unwrap();
        let chuckles = parse.names.find_name("chuckles").unwrap();
        let x = parse.names.find_name("x").unwrap();

        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::While {
                id: Some(chuckles),
                condition: VarExpr(v_int(1)),
                body: vec![
                    Stmt {
                        node: StmtNode::Expr(Expr::Assign {
                            left: Box::new(Id(x)),
                            right: Box::new(Expr::Binary(
                                BinaryOp::Add,
                                Box::new(Id(x)),
                                Box::new(VarExpr(v_int(1))),
                            )),
                        }),
                        parser_line_no: 1,
                        tree_line_no: 2,
                    },
                    Stmt {
                        node: StmtNode::Cond {
                            arms: vec![CondArm {
                                condition: Expr::Binary(
                                    BinaryOp::Gt,
                                    Box::new(Id(x)),
                                    Box::new(VarExpr(v_int(5))),
                                ),
                                statements: vec![Stmt {
                                    node: StmtNode::Break {
                                        exit: Some(chuckles)
                                    },
                                    parser_line_no: 1,
                                    tree_line_no: 4,
                                }],
                            }],
                            otherwise: vec![],
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
        let parse = parse_program(program).unwrap();
        let test_string = parse.names.find_name("test_string").unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Verb {
                location: Box::new(Prop {
                    location: Box::new(VarExpr(v_obj(0))),
                    property: Box::new(VarExpr(v_str("string_utils"))),
                }),
                verb: Box::new(VarExpr(v_str("from_list"))),
                args: vec![Normal(Id(test_string))],
            })]
        );
    }

    #[test]
    fn test_scatter_assign() {
        let program = "{connection} = args;";
        let parse = parse_program(program).unwrap();
        let connection = parse.names.find_name("connection").unwrap();
        let args = parse.names.find_name("args").unwrap();

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
        let parse = parse_program(program).unwrap();
        let connection = parse.names.find_name("connection").unwrap();
        let args = parse.names.find_name("args").unwrap();

        let scatter_items = vec![ScatterItem {
            kind: ScatterKind::Required,
            id: connection,
            expr: None,
        }];
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::Scatter(
                scatter_items,
                Box::new(Expr::Index(Box::new(Id(args)), Box::new(VarExpr(v_int(1))),))
            ))]
        );
    }

    #[test]
    fn test_indexed_list() {
        let program = "{a,b,c}[1];";
        let parse = parse_program(program).unwrap();
        let a = parse.names.find_name("a").unwrap();
        let b = parse.names.find_name("b").unwrap();
        let c = parse.names.find_name("c").unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::Index(
                Box::new(Expr::List(vec![
                    Normal(Id(a)),
                    Normal(Id(b)),
                    Normal(Id(c)),
                ])),
                Box::new(VarExpr(v_int(1))),
            ))]
        );
    }

    #[test]
    fn test_assigned_indexed_list() {
        let program = "a = {a,b,c}[1];";
        let parse = parse_program(program).unwrap();
        let a = parse.names.find_name("a").unwrap();
        let b = parse.names.find_name("b").unwrap();
        let c = parse.names.find_name("c").unwrap();
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
                    Box::new(VarExpr(v_int(1))),
                )),
            },)]
        );
    }

    #[test]
    fn test_indexed_assign() {
        let program = "this.stack[5] = 5;";
        let parse = parse_program(program).unwrap();
        let this = parse.names.find_name("this").unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::Assign {
                left: Box::new(Expr::Index(
                    Box::new(Prop {
                        location: Box::new(Id(this)),
                        property: Box::new(VarExpr(v_str("stack"))),
                    }),
                    Box::new(VarExpr(v_int(5))),
                )),
                right: Box::new(VarExpr(v_int(5))),
            })]
        );
    }

    #[test]
    fn test_for_list() {
        let program = "for i in ({1,2,3}) endfor return i;";
        let parse = parse_program(program).unwrap();
        let i = parse.names.find_name("i").unwrap();
        // Verify the structure of the syntax tree for a for-list loop.
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![
                StmtNode::ForList {
                    id: i,
                    expr: Expr::List(vec![
                        Normal(VarExpr(v_int(1))),
                        Normal(VarExpr(v_int(2))),
                        Normal(VarExpr(v_int(3))),
                    ]),
                    body: vec![],
                },
                StmtNode::Return { expr: Some(Id(i)) },
            ]
        )
    }

    #[test]
    fn test_scatter_required() {
        let program = "{a, b, c} = args;";
        let parse = parse_program(program).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::Scatter(
                vec![
                    ScatterItem {
                        kind: ScatterKind::Required,
                        id: parse.names.find_name("a").unwrap(),
                        expr: None,
                    },
                    ScatterItem {
                        kind: ScatterKind::Required,
                        id: parse.names.find_name("b").unwrap(),
                        expr: None,
                    },
                    ScatterItem {
                        kind: ScatterKind::Required,
                        id: parse.names.find_name("c").unwrap(),
                        expr: None,
                    },
                ],
                Box::new(Id(parse.names.find_name("args").unwrap())),
            ))]
        );
    }

    #[test]
    fn test_valid_underscore_and_no_underscore_ident() {
        let program = "_house == home;";
        let parse = parse_program(program).unwrap();
        let house = parse.names.find_name("_house").unwrap();
        let home = parse.names.find_name("home").unwrap();
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
        let parse = parse_program(program).unwrap();
        let results = parse.names.find_name("results").unwrap();
        let args = parse.names.find_name("args").unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Return {
                expr: Some(Expr::List(vec![
                    Splice(Id(results)),
                    Normal(Call {
                        function: "frozzbozz".to_string(),
                        args: vec![Splice(Id(args))],
                    }),
                ])),
            }]
        );
    }

    #[test]
    fn test_string_escape_codes() {
        // Just verify MOO's very limited string escape tokenizing, which does not support
        // anything other than \" and \\. \n, \t etc just become "n" "t".
        let program = r#"
            "\n \t \r \" \\";
        "#;
        let parse = parse_program(program).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(VarExpr(v_str(r#"n t r " \"#)))]
        );
    }

    #[test]
    fn test_empty_expr_stmt() {
        let program = r#"
            ;;;;
    "#;

        let parse = parse_program(program).unwrap();

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

        let parse = parse_program(program).unwrap();
        let a = parse.names.find_name("a").unwrap();
        let info = parse.names.find_name("info").unwrap();
        let forgotten = parse.names.find_name("forgotten").unwrap();

        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![
                StmtNode::ForList {
                    id: a,
                    expr: Expr::List(vec![
                        Normal(VarExpr(v_int(1))),
                        Normal(VarExpr(v_int(2))),
                        Normal(VarExpr(v_int(3))),
                    ]),
                    body: vec![],
                },
                StmtNode::Expr(Expr::Assign {
                    left: Box::new(Id(info)),
                    right: Box::new(VarExpr(v_int(5))),
                }),
                StmtNode::Expr(Expr::Assign {
                    left: Box::new(Id(forgotten)),
                    right: Box::new(VarExpr(v_int(3))),
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
        let parse = parse_program(program).unwrap();
        assert_eq!(
            parse.stmts,
            vec![Stmt {
                node: StmtNode::Cond {
                    arms: vec![CondArm {
                        condition: Expr::Binary(
                            BinaryOp::Eq,
                            Box::new(VarExpr(v_int(5))),
                            Box::new(VarExpr(v_int(5))),
                        ),
                        statements: vec![Stmt {
                            node: StmtNode::Return {
                                expr: Some(VarExpr(v_int(5)))
                            },
                            parser_line_no: 2,
                            tree_line_no: 2,
                        }],
                    }],
                    otherwise: vec![Stmt {
                        node: StmtNode::Return {
                            expr: Some(VarExpr(v_int(3)))
                        },
                        parser_line_no: 4,
                        tree_line_no: 4,
                    }],
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
        let parse = parse_program(program).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Cond {
                arms: vec![
                    CondArm {
                        condition: Expr::Binary(
                            BinaryOp::Eq,
                            Box::new(VarExpr(v_int(5))),
                            Box::new(VarExpr(v_int(5))),
                        ),
                        statements: vec![Stmt {
                            node: StmtNode::Return {
                                expr: Some(VarExpr(v_int(5)))
                            },
                            parser_line_no: 2,
                            tree_line_no: 2,
                        }],
                    },
                    CondArm {
                        condition: Expr::Binary(
                            BinaryOp::Eq,
                            Box::new(VarExpr(v_int(2))),
                            Box::new(VarExpr(v_int(2))),
                        ),
                        statements: vec![Stmt {
                            node: StmtNode::Return {
                                expr: Some(VarExpr(v_int(2)))
                            },
                            parser_line_no: 4,
                            tree_line_no: 4,
                        }],
                    },
                ],
                otherwise: vec![Stmt {
                    node: StmtNode::Return {
                        expr: Some(VarExpr(v_int(3)))
                    },
                    parser_line_no: 6,
                    tree_line_no: 6,
                }],
            }]
        );
    }

    #[test]
    fn test_if_in_range() {
        let program = r#"if (5 in {1,2,3})
                       endif"#;
        let parse = parse_program(program).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Cond {
                arms: vec![CondArm {
                    condition: Expr::Binary(
                        BinaryOp::In,
                        Box::new(VarExpr(v_int(5))),
                        Box::new(Expr::List(vec![
                            Normal(VarExpr(v_int(1))),
                            Normal(VarExpr(v_int(2))),
                            Normal(VarExpr(v_int(3))),
                        ])),
                    ),
                    statements: vec![],
                }],
                otherwise: vec![],
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
        let parse = parse_program(program).unwrap();

        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::TryExcept {
                body: vec![Stmt {
                    node: StmtNode::Expr(VarExpr(v_int(5))),
                    parser_line_no: 2,
                    tree_line_no: 2,
                }],
                excepts: vec![ExceptArm {
                    id: None,
                    codes: CatchCodes::Codes(vec![Normal(VarExpr(v_err(E_PROPNF)))]),
                    statements: vec![Stmt {
                        node: StmtNode::Return { expr: None },
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
        let parse = parse_program(program).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(VarExpr(v_float(10000.0)))]
        );
    }

    #[test]
    fn test_in_range() {
        let program = "a in {1,2,3};";
        let parse = parse_program(program).unwrap();
        let a = parse.names.find_name("a").unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::Binary(
                BinaryOp::In,
                Box::new(Id(a)),
                Box::new(Expr::List(vec![
                    Normal(VarExpr(v_int(1))),
                    Normal(VarExpr(v_int(2))),
                    Normal(VarExpr(v_int(3))),
                ])),
            ))]
        );
    }

    #[test]
    fn test_empty_list() {
        let program = "{};";
        let parse = parse_program(program).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::List(vec![]))]
        );
    }

    #[test]
    fn test_verb_expr() {
        let program = "this:(\"verb\")(1,2,3);";
        let parse = parse_program(program).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Verb {
                location: Box::new(Id(parse.names.find_name("this").unwrap())),
                verb: Box::new(VarExpr(v_str("verb"))),
                args: vec![
                    Normal(VarExpr(v_int(1))),
                    Normal(VarExpr(v_int(2))),
                    Normal(VarExpr(v_int(3))),
                ],
            })]
        );
    }

    #[test]
    fn test_prop_expr() {
        let program = "this.(\"prop\");";
        let parse = parse_program(program).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Prop {
                location: Box::new(Id(parse.names.find_name("this").unwrap())),
                property: Box::new(VarExpr(v_str("prop"))),
            })]
        );
    }

    #[test]
    fn test_not_expr() {
        let program = "!2;";
        let parse = parse_program(program).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::Unary(
                UnaryOp::Not,
                Box::new(VarExpr(v_int(2))),
            ))]
        );
    }

    #[test]
    fn test_comparison_assign_chain() {
        let program = "(2 <= (len = length(text)));";
        let parse = parse_program(program).unwrap();
        let len = parse.names.find_name("len").unwrap();
        let text = parse.names.find_name("text").unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::Binary(
                BinaryOp::LtE,
                Box::new(VarExpr(v_int(2))),
                Box::new(Expr::Assign {
                    left: Box::new(Id(len)),
                    right: Box::new(Call {
                        function: "length".to_string(),
                        args: vec![Normal(Id(text))],
                    }),
                }),
            ))]
        );
    }

    #[test]
    fn test_cond_expr() {
        let program = "a = (1 == 2 ? 3 | 4);";
        let parse = parse_program(program).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::Assign {
                left: Box::new(Id(parse.names.find_name("a").unwrap())),
                right: Box::new(Expr::Cond {
                    condition: Box::new(Expr::Binary(
                        BinaryOp::Eq,
                        Box::new(VarExpr(v_int(1))),
                        Box::new(VarExpr(v_int(2))),
                    )),
                    consequence: Box::new(VarExpr(v_int(3))),
                    alternative: Box::new(VarExpr(v_int(4))),
                }),
            })]
        );
    }

    #[test]
    fn test_list_compare() {
        let program = "{what} == args;";
        let parse = parse_program(program).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::Binary(
                BinaryOp::Eq,
                Box::new(Expr::List(vec![Normal(Id(parse
                    .names
                    .find_name("what")
                    .unwrap())),])),
                Box::new(Id(parse.names.find_name("args").unwrap())),
            ))]
        );
    }

    #[test]
    fn test_raise_bf_call_incorrect_err() {
        // detect ambiguous match on E_PERMS != E_PERM
        let program = "raise(E_PERMS);";
        let parse = parse_program(program).unwrap();

        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Call {
                function: "raise".to_string(),
                args: vec![Normal(Id(parse.names.find_name("E_PERMS").unwrap()))]
            })]
        );
    }

    #[test]
    fn test_keyword_disambig_call() {
        let program = r#"
            for line in ({1,2,3})
            endfor(52);
        "#;
        let parse = parse_program(program).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![
                StmtNode::ForList {
                    id: parse.names.find_name("line").unwrap(),
                    expr: Expr::List(vec![
                        Normal(VarExpr(v_int(1))),
                        Normal(VarExpr(v_int(2))),
                        Normal(VarExpr(v_int(3))),
                    ]),
                    body: vec![],
                },
                StmtNode::Expr(VarExpr(v_int(52))),
            ]
        );
    }

    #[test]
    fn try_catch_expr() {
        let program = "return {`x ! e_varnf => 666'};";
        let parse = parse_program(program).unwrap();

        let varnf = Normal(VarExpr(v_err(E_VARNF)));
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Return {
                expr: Some(Expr::List(vec![Normal(Expr::Catch {
                    trye: Box::new(Id(parse.names.find_name("x").unwrap())),
                    codes: CatchCodes::Codes(vec![varnf]),
                    except: Some(Box::new(VarExpr(v_int(666)))),
                })],))
            }]
        )
    }

    #[test]
    fn try_catch_any_expr() {
        let program = "`raise(E_INVARG) ! ANY';";
        let parse = parse_program(program).unwrap();
        let invarg = Normal(VarExpr(v_err(E_INVARG)));

        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::Catch {
                trye: Box::new(Call {
                    function: "raise".to_string(),
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
        let parse = parse_program(program).unwrap();

        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::Catch {
                trye: Box::new(Verb {
                    location: Box::new(Prop {
                        location: Box::new(VarExpr(v_obj(0))),
                        property: Box::new(VarExpr(v_str("ftp_client"))),
                    }),
                    verb: Box::new(VarExpr(v_str("finish_get"))),
                    args: vec![Normal(Prop {
                        location: Box::new(Id(parse.names.find_name("this").unwrap())),
                        property: Box::new(VarExpr(v_str("connection"))),
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
        let parse = parse_program(program_a).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::And(
                Box::new(VarExpr(v_int(1))),
                Box::new(Expr::Or(
                    Box::new(VarExpr(v_int(2))),
                    Box::new(VarExpr(v_int(3))),
                )),
            ))]
        );
        let program_b = r#"1 && 2 || 3;"#;
        let parse = parse_program(program_b).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![StmtNode::Expr(Expr::Or(
                Box::new(Expr::And(
                    Box::new(VarExpr(v_int(1))),
                    Box::new(VarExpr(v_int(2))),
                )),
                Box::new(VarExpr(v_int(3))),
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
        let parse = parse_program(program).unwrap();
        assert_eq!(
            stripped_stmts(&parse.stmts),
            vec![
                StmtNode::Expr(Expr::Assign {
                    left: Box::new(Id(parse.names.find_name("result").unwrap())),
                    right: Box::new(Expr::Pass {
                        args: vec![Splice(Id(parse.names.find_name("args").unwrap()))],
                    }),
                }),
                StmtNode::Expr(Expr::Assign {
                    left: Box::new(Id(parse.names.find_name("result").unwrap())),
                    right: Box::new(Expr::Pass { args: vec![] }),
                }),
                StmtNode::Expr(Expr::Assign {
                    left: Box::new(Id(parse.names.find_name("result").unwrap())),
                    right: Box::new(Expr::Pass {
                        args: vec![
                            Normal(VarExpr(v_int(1))),
                            Normal(VarExpr(v_int(2))),
                            Normal(VarExpr(v_int(3))),
                            Normal(VarExpr(v_int(4))),
                        ],
                    }),
                }),
                StmtNode::Expr(Expr::Assign {
                    left: Box::new(Id(parse.names.find_name("pass").unwrap())),
                    right: Box::new(Id(parse.names.find_name("blop").unwrap())),
                }),
                StmtNode::Return {
                    expr: Some(Id(parse.names.find_name("pass").unwrap())),
                },
            ]
        );
    }
}
