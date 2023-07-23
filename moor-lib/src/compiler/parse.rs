use std::cell::RefCell;
use std::rc::Rc;
/// Kicks off the Pest parser and converts it into our AST.
/// This is the main entry point for parsing.
use std::str::FromStr;

use pest::pratt_parser::{Assoc, Op, PrattParser};
pub use pest::Parser as PestParser;

use crate::compiler::ast::{
    Arg, BinaryOp, CatchCodes, CondArm, ExceptArm, Expr, ScatterItem, ScatterKind, Stmt, UnaryOp,
};
use crate::compiler::labels::Names;
use crate::compiler::parse::moo::{MooParser, Rule};
use crate::compiler::Parse;
use crate::var::error::Error::{
    E_ARGS, E_DIV, E_FLOAT, E_INVARG, E_INVIND, E_MAXREC, E_NACC, E_PERM, E_PROPNF, E_QUOTA,
    E_RANGE, E_RECMOVE, E_TYPE, E_VARNF, E_VERBNF,
};
use crate::var::{v_err, v_float, v_int, v_objid, v_str, Objid, SYSTEM_OBJECT};

pub mod moo {
    #[derive(Parser)]
    #[grammar = "src/compiler/moo.pest"]
    pub struct MooParser;
}

fn parse_atom(
    names: Rc<RefCell<Names>>,
    pairs: pest::iterators::Pair<Rule>,
) -> Result<Expr, anyhow::Error> {
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
        Rule::hex => {
            let int = i64::from_str_radix(&pairs.as_str()[2..], 16).unwrap();
            Ok(Expr::VarExpr(v_int(int)))
        }
        Rule::float => {
            let float = pairs.as_str().parse::<f64>().unwrap();
            Ok(Expr::VarExpr(v_float(float)))
        }
        Rule::string => {
            let string = pairs.as_str();
            // Note we don't trim the start and end quotes, because snailquote is expecting them.
            let parsed = snailquote::unescape(string).unwrap();
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
) -> Result<Vec<Arg>, anyhow::Error> {
    let mut args = vec![];
    for pair in pairs {
        match pair.as_rule() {
            Rule::argument => {
                let arg = if pair.as_str().starts_with('@') {
                    Arg::Splice(parse_expr(
                        names.clone(),
                        pair.into_inner().next().unwrap().into_inner(),
                    )?)
                } else {
                    Arg::Normal(parse_expr(
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
) -> Result<Vec<Arg>, anyhow::Error> {
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
) -> Result<CatchCodes, anyhow::Error> {
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
) -> Result<Expr, anyhow::Error> {
    let pratt = PrattParser::new()
        // CondExpr is right-associative in C-ish languages, and should sit above all the infix rules
        // to make Pratt happy, it seems.
        .op(Op::postfix(Rule::assign))
        .op(Op::prefix(Rule::scatter_assign))
        .op(Op::postfix(Rule::cond_expr))
        .op(Op::prefix(Rule::neg))
        .op(Op::prefix(Rule::not))
        .op(Op::infix(Rule::add, Assoc::Left) | Op::infix(Rule::sub, Assoc::Left))
        .op(Op::infix(Rule::mul, Assoc::Left) | Op::infix(Rule::div, Assoc::Left))
        .op(Op::infix(Rule::gt, Assoc::Left) | Op::infix(Rule::lt, Assoc::Left))
        .op(Op::infix(Rule::land, Assoc::Left) | Op::infix(Rule::lor, Assoc::Left))
        .op(Op::infix(Rule::gte, Assoc::Left) | Op::infix(Rule::lte, Assoc::Left))
        .op(Op::infix(Rule::eq, Assoc::Left) | Op::infix(Rule::neq, Assoc::Left))
        .op(Op::infix(Rule::pow, Assoc::Left))
        .op(Op::infix(Rule::modulus, Assoc::Left))
        .op(Op::infix(Rule::in_range, Assoc::Left))
        .op(Op::postfix(Rule::index_range))
        .op(Op::postfix(Rule::index_single))
        .op(Op::postfix(Rule::verb_call))
        .op(Op::postfix(Rule::verb_expr_call))
        .op(Op::postfix(Rule::prop))
        .op(Op::postfix(Rule::prop_expr));

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
                let verb = inner.next().unwrap().as_str();
                let args = parse_arglist(names.clone(), inner.next().unwrap().into_inner())?;
                Ok(Expr::Verb {
                    location: Box::new(Expr::VarExpr(v_objid(SYSTEM_OBJECT))),
                    verb: Box::new(Expr::VarExpr(v_str(verb))),
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
) -> Result<Option<Stmt>, anyhow::Error> {
    match pair.as_rule() {
        Rule::expr_statement => {
            let mut inner = pair.into_inner();
            if let Some(rule) = inner.next() {
                let expr = parse_expr(names, rule.into_inner())?;
                return Ok(Some(Stmt::Expr(expr)));
            }
            Ok(None)
        }
        Rule::while_statement => {
            let mut parts = pair.into_inner();
            let condition = parse_expr(names.clone(), parts.next().unwrap().into_inner())?;
            let mut body = vec![];
            parse_statements(names, parts.next().unwrap().into_inner(), &mut body)?;
            Ok(Some(Stmt::While {
                id: None,
                condition,
                body,
            }))
        }
        Rule::labelled_while_statement => {
            let mut parts = pair.into_inner();
            let id = names
                .borrow_mut()
                .find_or_add_name(parts.next().unwrap().as_str());
            let condition = parse_expr(names.clone(), parts.next().unwrap().into_inner())?;
            let mut body = vec![];
            parse_statements(names, parts.next().unwrap().into_inner(), &mut body)?;
            Ok(Some(Stmt::While {
                id: Some(id),
                condition,
                body,
            }))
        }
        Rule::if_statement => {
            let mut parts = pair.into_inner();
            let mut arms = vec![];
            let mut otherwise = vec![];
            let condition = parse_expr(names.clone(), parts.next().unwrap().into_inner())?;
            let mut body = vec![];
            parse_statements(names.clone(), parts.next().unwrap().into_inner(), &mut body)?;
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
                        let mut body = vec![];
                        parse_statements(
                            names.clone(),
                            parts.next().unwrap().into_inner(),
                            &mut body,
                        )?;
                        arms.push(CondArm {
                            condition,
                            statements: body,
                        });
                    }
                    Rule::else_clause => {
                        let mut parts = remainder.into_inner();
                        parse_statements(
                            names.clone(),
                            parts.next().unwrap().into_inner(),
                            &mut otherwise,
                        )?;
                    }
                    _ => panic!("Unimplemented if clause: {:?}", remainder),
                }
            }
            Ok(Some(Stmt::Cond { arms, otherwise }))
        }
        Rule::break_statement => {
            let mut parts = pair.into_inner();
            let label = parts
                .next()
                .map(|id| names.borrow_mut().find_or_add_name(id.as_str()));
            Ok(Some(Stmt::Break { exit: label }))
        }
        Rule::continue_statement => {
            let mut parts = pair.into_inner();
            let label = parts
                .next()
                .map(|id| names.borrow_mut().find_or_add_name(id.as_str()));
            Ok(Some(Stmt::Continue { exit: label }))
        }
        Rule::return_statement => {
            let mut parts = pair.into_inner();
            let expr = parts
                .next()
                .map(|expr| parse_expr(names.clone(), expr.into_inner()).unwrap());
            Ok(Some(Stmt::Return { expr }))
        }
        Rule::for_statement => {
            let mut parts = pair.into_inner();
            let id = names
                .borrow_mut()
                .find_or_add_name(parts.next().unwrap().as_str());
            let clause = parts.next().unwrap();
            let mut body = vec![];
            parse_statements(names.clone(), parts.next().unwrap().into_inner(), &mut body)?;
            match clause.as_rule() {
                Rule::for_range_clause => {
                    let mut clause_inner = clause.into_inner();
                    let from_rule = clause_inner.next().unwrap();
                    let to_rule = clause_inner.next().unwrap();
                    let from = parse_expr(names.clone(), from_rule.into_inner())?;
                    let to = parse_expr(names, to_rule.into_inner())?;
                    Ok(Some(Stmt::ForRange { id, from, to, body }))
                }
                Rule::for_in_clause => {
                    let mut clause_inner = clause.into_inner();
                    let in_rule = clause_inner.next().unwrap();
                    let expr = parse_expr(names, in_rule.into_inner())?;
                    Ok(Some(Stmt::ForList { id, expr, body }))
                }
                _ => panic!("Unimplemented for clause: {:?}", clause),
            }
        }
        Rule::try_finally_statement => {
            let mut parts = pair.into_inner();
            let mut body = vec![];
            parse_statements(names.clone(), parts.next().unwrap().into_inner(), &mut body)?;
            let mut handler = vec![];
            parse_statements(names, parts.next().unwrap().into_inner(), &mut handler)?;
            Ok(Some(Stmt::TryFinally { body, handler }))
        }
        Rule::try_except_statement => {
            let mut parts = pair.into_inner();
            let mut body = vec![];
            parse_statements(names.clone(), parts.next().unwrap().into_inner(), &mut body)?;
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
                        let mut statements = vec![];
                        parse_statements(
                            names.clone(),
                            except_clause_parts.next().unwrap().into_inner(),
                            &mut statements,
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
            Ok(Some(Stmt::TryExcept { body, excepts }))
        }
        Rule::fork_statement => {
            let mut parts = pair.into_inner();
            let time = parse_expr(names.clone(), parts.next().unwrap().into_inner())?;
            let mut body = vec![];
            parse_statements(names, parts.next().unwrap().into_inner(), &mut body)?;
            Ok(Some(Stmt::Fork {
                id: None,
                time,
                body,
            }))
        }
        Rule::labelled_fork_statement => {
            let mut parts = pair.into_inner();
            let id = names
                .borrow_mut()
                .find_or_add_name(parts.next().unwrap().as_str());
            let time = parse_expr(names.clone(), parts.next().unwrap().into_inner())?;
            let mut body = vec![];
            parse_statements(names, parts.next().unwrap().into_inner(), &mut body)?;
            Ok(Some(Stmt::Fork {
                id: Some(id),
                time,
                body,
            }))
        }
        _ => panic!("Unimplemented statement: {:?}", pair.as_rule()),
    }
}

fn parse_statements(
    names: Rc<RefCell<Names>>,
    pairs: pest::iterators::Pairs<Rule>,
    statements: &mut Vec<Stmt>,
) -> Result<(), anyhow::Error> {
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
    Ok(())
}

pub fn parse_program(program_text: &str) -> Result<Parse, anyhow::Error> {
    let parse_program_span = tracing::trace_span!("parse_program");
    let _enter = parse_program_span.enter();

    let pairs = MooParser::parse(moo::Rule::program, program_text)?;

    let mut program = Vec::new();
    // This has to be tossed into an Arc because the precedence parser uses it in multiple closures,
    // causing multiple borrows.
    let names = Rc::new(RefCell::new(Names::new()));
    for pair in pairs {
        match pair.as_rule() {
            moo::Rule::program => {
                let inna = pair.into_inner().next().unwrap();

                match inna.as_rule() {
                    Rule::statements => {
                        parse_statements(names.clone(), inna.into_inner(), &mut program)?;
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
    Ok(Parse {
        stmts: program,
        names,
    })
}

#[cfg(test)]
mod tests {
    use crate::compiler::ast::Arg::Normal;
    use crate::compiler::ast::Expr::{Call, Id, Prop, VarExpr, Verb};
    use crate::compiler::ast::{
        Arg, BinaryOp, CatchCodes, CondArm, ExceptArm, Expr, ScatterItem, ScatterKind, Stmt,
        UnaryOp,
    };
    use crate::compiler::labels::Names;
    use crate::compiler::parse::parse_program;
    use crate::var::error::Error::{E_INVARG, E_PROPNF, E_VARNF};
    use crate::var::{v_err, v_float, v_int, v_obj, v_str};

    #[test]
    fn test_call_verb() {
        let program = r#"#0:test_verb(1,2,3,"test");"#;
        let _names = Names::new();
        let parsed = parse_program(program).unwrap();
        assert_eq!(parsed.stmts.len(), 1);
        assert_eq!(
            parsed.stmts,
            vec![Stmt::Expr(Expr::Verb {
                location: Box::new(Expr::VarExpr(v_obj(0))),
                verb: Box::new(Expr::VarExpr(v_str("test_verb"))),
                args: vec![
                    Arg::Normal(Expr::VarExpr(v_int(1))),
                    Arg::Normal(Expr::VarExpr(v_int(2))),
                    Arg::Normal(Expr::VarExpr(v_int(3))),
                    Arg::Normal(Expr::VarExpr(v_str("test")))
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
            parse.stmts[0],
            Stmt::Expr(Expr::Assign {
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
            parse.stmts[0],
            Stmt::Expr(Expr::Call {
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
            parse.stmts[0],
            Stmt::Cond {
                arms: vec![
                    CondArm {
                        condition: Expr::Binary(
                            BinaryOp::Eq,
                            Box::new(VarExpr(v_int(1))),
                            Box::new(VarExpr(v_int(2))),
                        ),
                        statements: vec![Stmt::Return {
                            expr: Some(VarExpr(v_int(5))),
                        }],
                    },
                    CondArm {
                        condition: Expr::Binary(
                            BinaryOp::Eq,
                            Box::new(VarExpr(v_int(2))),
                            Box::new(VarExpr(v_int(3))),
                        ),
                        statements: vec![Stmt::Return {
                            expr: Some(VarExpr(v_int(3))),
                        }],
                    },
                ],

                otherwise: vec![Stmt::Return {
                    expr: Some(VarExpr(v_int(6))),
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
            parse.stmts[0],
            Stmt::Cond {
                arms: vec![
                    CondArm {
                        condition: Expr::Binary(
                            BinaryOp::Eq,
                            Box::new(VarExpr(v_int(1))),
                            Box::new(VarExpr(v_int(2))),
                        ),
                        statements: vec![Stmt::Return {
                            expr: Some(VarExpr(v_int(5))),
                        }],
                    },
                    CondArm {
                        condition: Expr::Binary(
                            BinaryOp::Eq,
                            Box::new(VarExpr(v_int(2))),
                            Box::new(VarExpr(v_int(3))),
                        ),
                        statements: vec![Stmt::Return {
                            expr: Some(VarExpr(v_int(3))),
                        }],
                    },
                    CondArm {
                        condition: Expr::Binary(
                            BinaryOp::Eq,
                            Box::new(VarExpr(v_int(3))),
                            Box::new(VarExpr(v_int(4))),
                        ),
                        statements: vec![Stmt::Return {
                            expr: Some(VarExpr(v_int(4))),
                        }],
                    },
                ],

                otherwise: vec![Stmt::Return {
                    expr: Some(VarExpr(v_int(6))),
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
            parse.stmts[0],
            Stmt::Return {
                expr: Some(Expr::Unary(
                    UnaryOp::Not,
                    Box::new(Expr::Verb {
                        location: Box::new(Expr::VarExpr(v_obj(2))),
                        verb: Box::new(Expr::VarExpr(v_str("move"))),
                        args: vec![Normal(Expr::VarExpr(v_int(5)))],
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
            parse.stmts[0],
            Stmt::Cond {
                arms: vec![CondArm {
                    condition: Expr::Unary(
                        UnaryOp::Not,
                        Box::new(Expr::Verb {
                            location: Box::new(Expr::Prop {
                                location: Box::new(Expr::VarExpr(v_obj(0))),
                                property: Box::new(Expr::VarExpr(v_str("network"))),
                            }),
                            verb: Box::new(VarExpr(v_str("is_connected"))),
                            args: vec![Normal(Id(parse.names.find_name("this").unwrap())),],
                        })
                    ),
                    statements: vec![Stmt::Return { expr: None }],
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
            parse.stmts[0],
            Stmt::ForList {
                id: x,
                expr: Expr::List(vec![
                    Arg::Normal(VarExpr(v_int(1))),
                    Arg::Normal(VarExpr(v_int(2))),
                    Arg::Normal(VarExpr(v_int(3))),
                ]),
                body: vec![Stmt::Expr(Expr::Assign {
                    left: Box::new(Expr::Id(b)),
                    right: Box::new(Expr::Binary(
                        BinaryOp::Add,
                        Box::new(Expr::Id(x)),
                        Box::new(VarExpr(v_int(5))),
                    )),
                })],
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
            parse.stmts[0],
            Stmt::ForRange {
                id: x,
                from: VarExpr(v_int(1)),
                to: VarExpr(v_int(5)),
                body: vec![Stmt::Expr(Expr::Assign {
                    left: Box::new(Id(b)),
                    right: Box::new(Expr::Binary(
                        BinaryOp::Add,
                        Box::new(Id(x)),
                        Box::new(VarExpr(v_int(5))),
                    )),
                })],
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
            parse.stmts,
            vec![
                Stmt::Expr(Expr::Assign {
                    left: Box::new(Expr::Id(a)),
                    right: Box::new(Expr::List(vec![
                        Arg::Normal(VarExpr(v_int(1))),
                        Arg::Normal(VarExpr(v_int(2))),
                        Arg::Normal(VarExpr(v_int(3))),
                    ])),
                }),
                Stmt::Expr(Expr::Assign {
                    left: Box::new(Expr::Id(b)),
                    right: Box::new(Expr::Range {
                        base: Box::new(Expr::Id(a)),
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
            parse.stmts,
            vec![Stmt::While {
                id: None,
                condition: VarExpr(v_int(1)),
                body: vec![
                    Stmt::Expr(Expr::Assign {
                        left: Box::new(Expr::Id(x)),
                        right: Box::new(Expr::Binary(
                            BinaryOp::Add,
                            Box::new(Expr::Id(x)),
                            Box::new(VarExpr(v_int(1))),
                        )),
                    }),
                    Stmt::Cond {
                        arms: vec![CondArm {
                            condition: Expr::Binary(
                                BinaryOp::Gt,
                                Box::new(Expr::Id(x)),
                                Box::new(VarExpr(v_int(5))),
                            ),
                            statements: vec![Stmt::Break { exit: None }],
                        }],
                        otherwise: vec![],
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
            parse.stmts,
            vec![Stmt::While {
                id: Some(chuckles),
                condition: VarExpr(v_int(1)),
                body: vec![
                    Stmt::Expr(Expr::Assign {
                        left: Box::new(Id(x)),
                        right: Box::new(Expr::Binary(
                            BinaryOp::Add,
                            Box::new(Id(x)),
                            Box::new(VarExpr(v_int(1))),
                        )),
                    }),
                    Stmt::Cond {
                        arms: vec![CondArm {
                            condition: Expr::Binary(
                                BinaryOp::Gt,
                                Box::new(Id(x)),
                                Box::new(VarExpr(v_int(5))),
                            ),
                            statements: vec![Stmt::Break {
                                exit: Some(chuckles)
                            }],
                        }],
                        otherwise: vec![],
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
            parse.stmts,
            vec![Stmt::Expr(Expr::Verb {
                location: Box::new(Prop {
                    location: Box::new(VarExpr(v_obj(0))),
                    property: Box::new(VarExpr(v_str("string_utils"))),
                }),
                verb: Box::new(VarExpr(v_str("from_list"))),
                args: vec![Arg::Normal(Id(test_string))],
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
            parse.stmts,
            vec![Stmt::Expr(Expr::Scatter(scatter_items, scatter_right))]
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
            parse.stmts,
            vec![Stmt::Expr(Expr::Scatter(
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
            parse.stmts,
            vec![Stmt::Expr(Expr::Index(
                Box::new(Expr::List(vec![
                    Arg::Normal(Id(a)),
                    Arg::Normal(Id(b)),
                    Arg::Normal(Id(c)),
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
            parse.stmts,
            vec![Stmt::Expr(Expr::Assign {
                left: Box::new(Id(a)),
                right: Box::new(Expr::Index(
                    Box::new(Expr::List(vec![
                        Arg::Normal(Id(a)),
                        Arg::Normal(Id(b)),
                        Arg::Normal(Id(c)),
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
            parse.stmts,
            vec![Stmt::Expr(Expr::Assign {
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
            parse.stmts,
            vec![
                Stmt::ForList {
                    id: i,
                    expr: Expr::List(vec![
                        Arg::Normal(VarExpr(v_int(1))),
                        Arg::Normal(VarExpr(v_int(2))),
                        Arg::Normal(VarExpr(v_int(3))),
                    ]),
                    body: vec![],
                },
                Stmt::Return { expr: Some(Id(i)) },
            ]
        )
    }

    #[test]
    fn test_scatter_required() {
        let program = "{a, b, c} = args;";
        let parse = parse_program(program).unwrap();
        assert_eq!(
            parse.stmts,
            vec![Stmt::Expr(Expr::Scatter(
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
            parse.stmts,
            vec![Stmt::Expr(Expr::Binary(
                BinaryOp::Eq,
                Box::new(Id(house)),
                Box::new(Id(home)),
            ))]
        );
    }

    #[test]
    fn test_arg_splice() {
        let program = "return {@results, pass(@args)};";
        let parse = parse_program(program).unwrap();
        let results = parse.names.find_name("results").unwrap();
        let args = parse.names.find_name("args").unwrap();
        assert_eq!(
            parse.stmts,
            vec![Stmt::Return {
                expr: Some(Expr::List(vec![
                    Arg::Splice(Id(results)),
                    Arg::Normal(Expr::Call {
                        function: "pass".to_string(),
                        args: vec![Arg::Splice(Id(args))],
                    }),
                ])),
            }]
        );
    }

    #[test]
    fn test_string_escape_codes() {
        let program = r#"
            "\n \t \r \" \\ ";
        "#;
        let parse = parse_program(program).unwrap();
        assert_eq!(
            parse.stmts,
            vec![Stmt::Expr(Expr::VarExpr(v_str("\n \t \r \" \\ ")))]
        );
    }

    #[test]
    fn test_empty_expr_stmt() {
        let program = r#"
            ;;;;
    "#;

        let parse = parse_program(program).unwrap();

        assert_eq!(parse.stmts, vec![]);
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
            parse.stmts,
            vec![
                Stmt::ForList {
                    id: a,
                    expr: Expr::List(vec![
                        Arg::Normal(VarExpr(v_int(1))),
                        Arg::Normal(VarExpr(v_int(2))),
                        Arg::Normal(VarExpr(v_int(3))),
                    ]),
                    body: vec![],
                },
                Stmt::Expr(Expr::Assign {
                    left: Box::new(Id(info)),
                    right: Box::new(VarExpr(v_int(5))),
                }),
                Stmt::Expr(Expr::Assign {
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
            vec![Stmt::Cond {
                arms: vec![CondArm {
                    condition: Expr::Binary(
                        BinaryOp::Eq,
                        Box::new(VarExpr(v_int(5))),
                        Box::new(VarExpr(v_int(5))),
                    ),
                    statements: vec![Stmt::Return {
                        expr: Some(VarExpr(v_int(5)))
                    }],
                }],
                otherwise: vec![Stmt::Return {
                    expr: Some(VarExpr(v_int(3)))
                }],
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
            parse.stmts,
            vec![Stmt::Cond {
                arms: vec![
                    CondArm {
                        condition: Expr::Binary(
                            BinaryOp::Eq,
                            Box::new(VarExpr(v_int(5))),
                            Box::new(VarExpr(v_int(5))),
                        ),
                        statements: vec![Stmt::Return {
                            expr: Some(VarExpr(v_int(5)))
                        }],
                    },
                    CondArm {
                        condition: Expr::Binary(
                            BinaryOp::Eq,
                            Box::new(VarExpr(v_int(2))),
                            Box::new(VarExpr(v_int(2))),
                        ),
                        statements: vec![Stmt::Return {
                            expr: Some(VarExpr(v_int(2)))
                        }],
                    },
                ],
                otherwise: vec![Stmt::Return {
                    expr: Some(VarExpr(v_int(3)))
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
            parse.stmts,
            vec![Stmt::Cond {
                arms: vec![CondArm {
                    condition: Expr::Binary(
                        BinaryOp::In,
                        Box::new(VarExpr(v_int(5))),
                        Box::new(Expr::List(vec![
                            Arg::Normal(VarExpr(v_int(1))),
                            Arg::Normal(VarExpr(v_int(2))),
                            Arg::Normal(VarExpr(v_int(3))),
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
            parse.stmts,
            vec![Stmt::TryExcept {
                body: vec![Stmt::Expr(Expr::VarExpr(v_int(5)))],
                excepts: vec![ExceptArm {
                    id: None,
                    codes: CatchCodes::Codes(vec![Arg::Normal(VarExpr(v_err(E_PROPNF)))]),
                    statements: vec![Stmt::Return { expr: None }],
                }],
            }]
        );
    }

    #[test]
    fn test_float() {
        let program = "10000.0;";
        let parse = parse_program(program).unwrap();
        assert_eq!(
            parse.stmts,
            vec![Stmt::Expr(Expr::VarExpr(v_float(10000.0)))]
        );
    }

    #[test]
    fn test_in_range() {
        let program = "a in {1,2,3};";
        let parse = parse_program(program).unwrap();
        let a = parse.names.find_name("a").unwrap();
        assert_eq!(
            parse.stmts,
            vec![Stmt::Expr(Expr::Binary(
                BinaryOp::In,
                Box::new(Id(a)),
                Box::new(Expr::List(vec![
                    Arg::Normal(VarExpr(v_int(1))),
                    Arg::Normal(VarExpr(v_int(2))),
                    Arg::Normal(VarExpr(v_int(3))),
                ])),
            ))]
        );
    }

    #[test]
    fn test_empty_list() {
        let program = "{};";
        let parse = parse_program(program).unwrap();
        assert_eq!(parse.stmts, vec![Stmt::Expr(Expr::List(vec![]))]);
    }

    #[test]
    fn test_verb_expr() {
        let program = "this:(\"verb\")(1,2,3);";
        let parse = parse_program(program).unwrap();
        assert_eq!(
            parse.stmts,
            vec![Stmt::Expr(Verb {
                location: Box::new(Id(parse.names.find_name("this").unwrap())),
                verb: Box::new(VarExpr(v_str("verb"))),
                args: vec![
                    Arg::Normal(VarExpr(v_int(1))),
                    Arg::Normal(VarExpr(v_int(2))),
                    Arg::Normal(VarExpr(v_int(3))),
                ],
            })]
        );
    }

    #[test]
    fn test_prop_expr() {
        let program = "this.(\"prop\");";
        let parse = parse_program(program).unwrap();
        assert_eq!(
            parse.stmts,
            vec![Stmt::Expr(Expr::Prop {
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
            parse.stmts,
            vec![Stmt::Expr(Expr::Unary(
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
            parse.stmts,
            vec![Stmt::Expr(Expr::Binary(
                BinaryOp::LtE,
                Box::new(VarExpr(v_int(2))),
                Box::new(Expr::Assign {
                    left: Box::new(Id(len)),
                    right: Box::new(Expr::Call {
                        function: "length".to_string(),
                        args: vec![Arg::Normal(Id(text))],
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
            parse.stmts,
            vec![Stmt::Expr(Expr::Assign {
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
            parse.stmts,
            vec![Stmt::Expr(Expr::Binary(
                BinaryOp::Eq,
                Box::new(Expr::List(vec![Arg::Normal(Id(parse
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
            parse.stmts,
            vec![Stmt::Expr(Expr::Call {
                function: "raise".to_string(),
                args: vec![Arg::Normal(Id(parse.names.find_name("E_PERMS").unwrap()))]
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
            parse.stmts,
            vec![
                Stmt::ForList {
                    id: parse.names.find_name("line").unwrap(),
                    expr: Expr::List(vec![
                        Arg::Normal(VarExpr(v_int(1))),
                        Arg::Normal(VarExpr(v_int(2))),
                        Arg::Normal(VarExpr(v_int(3))),
                    ]),
                    body: vec![],
                },
                Stmt::Expr(VarExpr(v_int(52))),
            ]
        );
    }

    #[test]
    fn try_catch_expr() {
        let program = "return {`x ! e_varnf => 666'};";
        let parse = parse_program(program).unwrap();

        let varnf = Arg::Normal(VarExpr(v_err(E_VARNF)));
        assert_eq!(
            parse.stmts,
            vec![Stmt::Return {
                expr: Some(Expr::List(vec![Arg::Normal(Expr::Catch {
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
        let invarg = Arg::Normal(VarExpr(v_err(E_INVARG)));

        assert_eq!(
            parse.stmts,
            vec![Stmt::Expr(Expr::Catch {
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
            parse.stmts,
            vec![Stmt::Expr(Expr::Catch {
                trye: Box::new(Verb {
                    location: Box::new(Expr::Prop {
                        location: Box::new(VarExpr(v_obj(0))),
                        property: Box::new(VarExpr(v_str("ftp_client"))),
                    }),
                    verb: Box::new(VarExpr(v_str("finish_get"))),
                    args: vec![Arg::Normal(Expr::Prop {
                        location: Box::new(Id(parse.names.find_name("this").unwrap())),
                        property: Box::new(VarExpr(v_str("connection"))),
                    })],
                }),
                codes: CatchCodes::Any,
                except: None,
            })]
        )
    }
}
