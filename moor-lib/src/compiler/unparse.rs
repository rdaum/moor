use moor_value::util::quote_str;
use moor_value::var::variant::Variant;

use crate::compiler::ast;
use crate::compiler::ast::{Stmt, StmtNode};
use crate::compiler::parse::Parse;

use super::labels::Names;

// TODO:
//  - "" for empty string:
//    MOO: rest[1..match(rest, "^ *")[2]] = "" vs
//   MOOR: rest[1..match(rest, "^ *")[2]] = ;
//  - sysobj calls:
//    MOO: $bleh(foo) vs
//   MOOR: #0.bleh(foo)

/// This could probably be combined with the structure for Parse.
#[derive(Debug)]
struct Unparse {
    names: Names,
}

impl ast::Expr {
    fn precedence(&self) -> u8 {
        // Returns the precedence of the operator. Higher values should be evaluated first.
        match self {
            ast::Expr::Assign { .. } => 1,
            ast::Expr::Cond { .. } => 2,
            ast::Expr::And(_, _) => 6,
            ast::Expr::Or(_, _) => 5,
            ast::Expr::Binary(op, _, _) => match op {
                ast::BinaryOp::Eq => 4,
                ast::BinaryOp::NEq => 4,
                ast::BinaryOp::Gt => 4,
                ast::BinaryOp::GtE => 4,
                ast::BinaryOp::Lt => 4,
                ast::BinaryOp::LtE => 4,
                ast::BinaryOp::In => 4,

                ast::BinaryOp::Add => 5,
                ast::BinaryOp::Sub => 5,

                ast::BinaryOp::Mul => 6,
                ast::BinaryOp::Div => 6,
                ast::BinaryOp::Mod => 6,

                ast::BinaryOp::Exp => 7,
            },

            ast::Expr::Unary(_, _) => 8,

            ast::Expr::Prop { .. } => 9,
            ast::Expr::Verb { .. } => 9,
            ast::Expr::Range { .. } => 9,
            ast::Expr::Index(_, _) => 9,

            ast::Expr::Scatter(_, _) => 8,

            ast::Expr::VarExpr(_) => 10,
            ast::Expr::Id(_) => 10,
            ast::Expr::List(_) => 10,
            ast::Expr::Pass { .. } => 10,
            ast::Expr::Call { .. } => 10,
            ast::Expr::Length => 10,
            ast::Expr::Catch { .. } => 10,
        }
    }
}

impl Unparse {
    fn new(names: Names) -> Self {
        Self { names }
    }

    fn unparse_arg(&self, arg: &ast::Arg) -> Result<String, anyhow::Error> {
        match arg {
            ast::Arg::Normal(expr) => Ok(self.unparse_expr(expr).unwrap()),
            ast::Arg::Splice(expr) => Ok(format!("@{}", self.unparse_expr(expr).unwrap())),
        }
    }

    fn unparse_args(&self, args: &[ast::Arg]) -> Result<String, anyhow::Error> {
        Ok(args
            .iter()
            .map(|arg| self.unparse_arg(arg).unwrap())
            .collect::<Vec<String>>()
            .join(", "))
    }

    fn unparse_catch_codes(&self, codes: &ast::CatchCodes) -> Result<String, anyhow::Error> {
        match codes {
            ast::CatchCodes::Codes(codes) => self.unparse_args(codes),
            ast::CatchCodes::Any => Ok(String::from("ANY")),
        }
    }

    fn unparse_var(&self, var: &moor_value::var::Var, aggressive: bool) -> String {
        if !aggressive {
            return format!("{var}");
        }
        if let Variant::Str(s) = var.variant() {
            let s = s.as_str();
            if !s.contains(' ') {
                s.into()
            } else {
                quote_str(s)
            }
        } else {
            format!("{var}")
        }
    }

    fn unparse_expr(&self, current_expr: &ast::Expr) -> Result<String, anyhow::Error> {
        let brace_if_lower = |expr: &ast::Expr| -> String {
            if expr.precedence() < current_expr.precedence() {
                format!("({})", self.unparse_expr(expr).unwrap())
            } else {
                self.unparse_expr(expr).unwrap()
            }
        };
        match current_expr {
            ast::Expr::Assign { left, right } => {
                let left_frag = self.unparse_expr(left)?;
                let right_frag = self.unparse_expr(right)?;
                Ok(format!("{left_frag} = {right_frag}"))
            }
            ast::Expr::Pass { args } => {
                let mut buffer = String::new();
                buffer.push_str("pass");
                if !args.is_empty() {
                    buffer.push('(');
                    buffer.push_str(self.unparse_args(args).unwrap().as_str());
                    buffer.push(')');
                }
                Ok(buffer)
            }
            ast::Expr::VarExpr(var) => Ok(self.unparse_var(var, true)),
            ast::Expr::Id(id) => Ok(self.names.name_of(id).unwrap().to_string()),
            ast::Expr::Binary(op, left_expr, right_expr) => Ok(format!(
                "{} {} {}",
                brace_if_lower(left_expr),
                op,
                brace_if_lower(right_expr)
            )),
            ast::Expr::And(left, right) => Ok(format!(
                "{} && {}",
                brace_if_lower(left),
                brace_if_lower(right)
            )),
            ast::Expr::Or(left, right) => Ok(format!(
                "{} || {}",
                brace_if_lower(left),
                brace_if_lower(right)
            )),
            ast::Expr::Unary(op, expr) => Ok(format!("{}{}", op, brace_if_lower(expr))),
            ast::Expr::Prop { location, property } => {
                let location = match (&**location, &**property) {
                    (ast::Expr::VarExpr(var), ast::Expr::Id(_)) if var.is_root() => {
                        String::from("$")
                    }
                    _ => format!("{}.", self.unparse_expr(location).unwrap()),
                };
                let prop = match &**property {
                    ast::Expr::Id(id) => self.names.name_of(id).unwrap().to_string(),
                    ast::Expr::VarExpr(var) => self.unparse_var(var, true),
                    _ => self.unparse_expr(property).unwrap(),
                };
                Ok(format!("{location}{prop}"))
            }
            ast::Expr::Call { function, args } => {
                let mut buffer = String::new();
                buffer.push_str(function);
                buffer.push('(');
                buffer.push_str(self.unparse_args(args)?.as_str());
                buffer.push(')');
                Ok(buffer)
            }
            ast::Expr::Verb {
                location,
                verb,
                args,
            } => {
                let mut buffer = String::new();
                buffer.push_str(brace_if_lower(location).as_str());
                buffer.push(':');
                buffer.push_str(brace_if_lower(verb).as_str());
                buffer.push('(');
                buffer.push_str(self.unparse_args(args)?.as_str());
                buffer.push(')');
                Ok(buffer)
            }
            ast::Expr::Range { base, from, to } => Ok(format!(
                "{}[{}..{}]",
                brace_if_lower(base),
                brace_if_lower(from),
                brace_if_lower(to)
            )),
            ast::Expr::Cond {
                condition,
                consequence,
                alternative,
            } => Ok(format!(
                "{} ? {} | {}",
                self.unparse_expr(condition).unwrap(),
                self.unparse_expr(consequence).unwrap(),
                self.unparse_expr(alternative).unwrap()
            )),
            ast::Expr::Catch {
                trye,
                codes,
                except,
            } => {
                let mut buffer = String::new();
                buffer.push('`');
                buffer.push_str(self.unparse_expr(trye)?.as_str());
                buffer.push_str(" ! ");
                buffer.push_str(self.unparse_catch_codes(codes)?.as_str());
                if let Some(except) = except {
                    buffer.push_str(" => ");
                    buffer.push_str(self.unparse_expr(except)?.as_str());
                }
                buffer.push('\'');
                Ok(buffer)
            }
            ast::Expr::Index(lvalue, index) => Ok(format!(
                "{}[{}]",
                self.unparse_expr(lvalue).unwrap(),
                self.unparse_expr(index).unwrap()
            )),
            ast::Expr::List(list) => {
                let mut buffer = String::new();
                buffer.push('{');
                buffer.push_str(self.unparse_args(list)?.as_str());
                buffer.push('}');
                Ok(buffer)
            }
            ast::Expr::Scatter(vars, expr) => {
                let mut buffer = String::new();
                buffer.push('(');
                for var in vars {
                    match var.kind {
                        ast::ScatterKind::Required => {}
                        ast::ScatterKind::Optional => {
                            buffer.push('?');
                        }
                        ast::ScatterKind::Rest => {
                            buffer.push('@');
                        }
                    }
                    buffer.push_str(self.names.name_of(&var.id).unwrap());
                    if let Some(expr) = &var.expr {
                        buffer.push_str(self.unparse_expr(expr)?.as_str());
                    }
                    buffer.push_str(", ");
                }
                buffer.pop();
                buffer.pop();
                buffer.push_str(") = ");
                buffer.push_str(self.unparse_expr(expr)?.as_str());
                Ok(buffer)
            }
            ast::Expr::Length => Ok(String::from("$")),
        }
    }

    fn unparse_stmt(&self, stmt: &ast::Stmt, indent: usize) -> Result<String, anyhow::Error> {
        let mut base_str = String::new();
        // Statements should not end in a newline, but should be terminated with a semicolon.
        match &stmt.0 {
            StmtNode::Cond { arms, otherwise } => {
                let cond_frag = self.unparse_expr(&arms[0].condition).unwrap();
                let stmt_frag = self.unparse_stmts(&arms[0].statements, indent + 4).unwrap();
                base_str.push_str(format!("if ({})\n{}", cond_frag, stmt_frag).as_str());
                for arm in arms.iter().skip(1) {
                    let cond_frag = self.unparse_expr(&arm.condition)?;
                    let stmt_frag = self.unparse_stmts(&arm.statements, indent + 4)?;
                    base_str.push_str(" ".repeat(indent).as_str());
                    base_str.push_str(format!("elseif ({})\n{}", cond_frag, stmt_frag).as_str());
                }
                if !otherwise.is_empty() {
                    let stmt_frag = self.unparse_stmts(otherwise, indent + 4).unwrap();
                    base_str.push_str(" ".repeat(indent).as_str());
                    base_str.push_str(format!("else\n{}", stmt_frag).as_str());
                }
                base_str.push_str(" ".repeat(indent).as_str());
                base_str.push_str("endif");
            }
            StmtNode::ForList { id, expr, body } => {
                let expr_frag = self.unparse_expr(expr)?;
                let stmt_frag = self.unparse_stmts(body, indent + 4)?;
                base_str.push_str(
                    format!(
                        "for {} in ({})\n{}{}endfor",
                        self.names.name_of(id).unwrap(),
                        expr_frag,
                        stmt_frag,
                        " ".repeat(indent)
                    )
                    .as_str(),
                );
            }
            StmtNode::ForRange { id, from, to, body } => {
                let from_frag = self.unparse_expr(from)?;
                let to_frag = self.unparse_expr(to)?;
                let stmt_frag = self.unparse_stmts(body, indent + 4)?;

                base_str.push_str(
                    format!(
                        "for {} in [{}..{}]\n{}{}endfor",
                        self.names.name_of(id).unwrap(),
                        from_frag,
                        to_frag,
                        stmt_frag,
                        " ".repeat(indent)
                    )
                    .as_str(),
                );
            }
            StmtNode::While {
                id,
                condition,
                body,
            } => {
                let cond_frag = self.unparse_expr(condition)?;
                let stmt_frag = self.unparse_stmts(body, indent + 4)?;
                base_str.push_str("while ");
                if let Some(id) = id {
                    base_str.push_str(self.names.name_of(id).unwrap());
                }
                base_str.push_str(format!("({})\n{}endwhile", cond_frag, stmt_frag).as_str());
            }
            StmtNode::Fork { id, time, body } => {
                let delay_frag = self.unparse_expr(time)?;
                let stmt_frag = self.unparse_stmts(body, indent + 4)?;
                base_str.push_str("fork ");
                if let Some(id) = id {
                    base_str.push_str(self.names.name_of(id).unwrap());
                }
                base_str.push_str(format!("({})\n{}\nendfork", delay_frag, stmt_frag).as_str());
            }
            StmtNode::TryExcept { body, excepts } => {
                let stmt_frag = self.unparse_stmts(body, indent + 4)?;
                base_str.push_str(format!("try\n{}", stmt_frag).as_str());
                for except in excepts {
                    let stmt_frag = self.unparse_stmts(&except.statements, indent + 4)?;
                    base_str.push_str("except ");
                    if let Some(id) = &except.id {
                        base_str.push_str(self.names.name_of(id).unwrap());
                        base_str.push(' ');
                    }
                    let catch_codes = self.unparse_catch_codes(&except.codes)?;
                    base_str.push_str(format!("({catch_codes})\n{stmt_frag}").as_str());
                }
                base_str.push_str("endtry");
            }
            StmtNode::TryFinally { body, handler } => {
                let stmt_frag = self.unparse_stmts(body, indent + 4)?;
                let handler_frag = self.unparse_stmts(handler, indent + 4)?;
                base_str
                    .push_str(format!("try\n{stmt_frag}finally\n{handler_frag}endtry").as_str());
            }
            StmtNode::Break { exit } => {
                base_str.push_str("break");
                if let Some(exit) = &exit {
                    base_str.push(' ');
                    base_str.push_str(self.names.name_of(exit).unwrap());
                }
                base_str.push(';');
            }
            StmtNode::Continue { exit } => {
                base_str.push_str("continue");
                if let Some(exit) = &exit {
                    base_str.push(' ');
                    base_str.push_str(self.names.name_of(exit).unwrap());
                }
                base_str.push(';');
            }
            StmtNode::Return { expr } => {
                base_str.push_str("return");
                if let Some(ret_expr) = &expr {
                    base_str.push(' ');
                    base_str.push_str(self.unparse_expr(ret_expr).unwrap().as_str());
                }
                base_str.push(';');
            }
            StmtNode::Expr(expr) => {
                base_str.push_str(self.unparse_expr(expr).unwrap().as_str());
                base_str.push(';');
            }
        };
        Ok(base_str)
    }

    pub fn unparse_stmts(&self, stms: &[Stmt], indent: usize) -> Result<String, anyhow::Error> {
        let prefix = " ".repeat(indent);
        let results = stms
            .iter()
            .map(|s| {
                self.unparse_stmt(s, indent)
                    .map(|line| format!("{prefix}{line}"))
            })
            .collect::<Result<Vec<String>, anyhow::Error>>()?;

        Ok(results.join("\n") + "\n")
    }
}

pub fn unparse(tree: &Parse) -> Result<String, anyhow::Error> {
    let unparse = Unparse::new(tree.names.clone());
    Ok(unparse.unparse_stmts(&tree.stmts, 0).unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use test_case::test_case;
    use unindent::unindent;

    #[test_case("a = 1;\n"; "assignment")]
    #[test_case("a = 1 + 2;\n"; "assignment with expr")]
    #[test_case("1 + 2 * 3 + 4;\n"; "binops with same precident")]
    #[test_case("1 * 2 + 3;\n"; "binops with different precident")]
    #[test_case("1 * (2 + 3);\n"; "binops with mixed precident and parens")]
    #[test_case("return;\n"; "Empty return")]
    #[test_case("return 20;\n";"return with args")]
    #[test_case(r#"
    if (expression)
        statements;
    endif
    "#; "simple if")]
    #[test_case(r#"
    if (expr)
        1;
    elseif (expr)
        2;
    elseif (expr)
        3;
    else
        4;
    endif
    "#; "if elseif chain")]
    #[test_case("`x.y ! E_PROPNF, E_PERM => 17';\n"; "catch expression")]
    #[test_case("method(a, b, c);\n"; "call function")]
    #[test_case(r#"
    try
        statements;
        statements;
    except e (E_DIV)
        statements;
    except e (E_PERM)
        statements;
    endtry
    "#; "exception handling")]
    #[test_case(r#"
    try
        basic;
    finally
        finalize();
    endtry
    "#; "try finally")]
    pub fn compare_parse_roundtrip(original: &str) {
        let stripped = unindent(original);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped, result);
    }

    #[test]
    pub fn unparse_complex_function() {
        let body = r#"
        brief = args && args[1];
        player:tell(this:namec_for_look_self(brief));
        things = this:visible_of(setremove(this:contents(), player));
        integrate = {};
        try
            if (this.integration_enabled)
                for i in (things)
                    if (this:ok_to_integrate(i) && (!brief || !is_player(i)))
                        integrate = {@integrate, i};
                        things = setremove(things, i);
                    endif
                endfor
                "for i in (this:obvious_exits(player))";
                for i in (this:exits())
                    if (this:ok_to_integrate(i))
                        integrate = setadd(integrate, i);
                        "changed so prevent exits from being integrated twice in the case of doors and the like";
                    endif
                endfor
            endif
        except (E_INVARG)
            player:tell("Error in integration: ");
        endtry
        if (!brief)
            desc = this:description(integrate);
            if (desc)
                player:tell_lines(desc);
            else
                player:tell("You see nothing special.");
            endif
        endif
        "there's got to be a better way to do this, but.";
        if (topic = this:topic_msg())
            if (0)
                this.topic_sign:show_topic();
            else
                player:tell(this.topic_sign:integrate_room_msg());
            endif
        endif
        "this:tell_contents(things, this.ctype);";
        this:tell_contents(things);
        "#;
        let stripped = unindent(body);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped, result);
    }

    pub fn parse_and_unparse(original: &str) -> Result<String, anyhow::Error> {
        let tree = crate::compiler::parse::parse_program(original).unwrap();
        unparse(&tree)
    }
}
