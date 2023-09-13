use moor_value::util::quote_str;
use moor_value::var::variant::Variant;

use crate::compiler::ast;
use crate::compiler::ast::{Expr, Stmt, StmtNode};
use crate::compiler::parse::Parse;

use super::labels::Names;

// TODO:
//  - "" for empty string:
//    MOO: rest[1..match(rest, "^ *")[2]] = "" vs
//   MOOR: rest[1..match(rest, "^ *")[2]] = ;
//  - sysobj calls:
//    MOO: $bleh(foo) vs
//   MOOR: #0.bleh(foo)
//   with/without extra-parens

/// This could probably be combined with the structure for Parse.
#[derive(Debug)]
struct Unparse {
    names: Names,
}

impl Expr {
    /// Returns the precedence of the operator. The higher the return value the higher the precedent.
    fn precedence(&self) -> u8 {
        // The table here is in reverse order from the return argument because the numbers are based
        // directly on http://en.cppreference.com/w/cpp/language/operator_precedence
        // Should be kept in sync with the pratt parser in `parse.rs`
        // Starting from lowest to highest precedence...
        // TODO: drive Pratt and this from one common precedence table.
        let cpp_ref_prep = match self {
            Expr::Scatter(_, _) | Expr::Assign { .. } => 14,
            Expr::Cond { .. } => 13,
            Expr::Or(_, _) => 12,
            Expr::And(_, _) => 11,
            Expr::Binary(op, _, _) => match op {
                ast::BinaryOp::Eq => 7,
                ast::BinaryOp::NEq => 7,
                ast::BinaryOp::Gt => 6,
                ast::BinaryOp::GtE => 6,
                ast::BinaryOp::Lt => 6,
                ast::BinaryOp::LtE => 6,
                ast::BinaryOp::In => 6,

                ast::BinaryOp::Add => 4,
                ast::BinaryOp::Sub => 4,

                ast::BinaryOp::Mul => 3,
                ast::BinaryOp::Div => 3,
                ast::BinaryOp::Mod => 3,

                ast::BinaryOp::Exp => 2,
            },

            Expr::Unary(_, _) => 1,

            Expr::Prop { .. } => 1,
            Expr::Verb { .. } => 1,
            Expr::Range { .. } => 1,
            Expr::Index(_, _) => 2,

            Expr::VarExpr(_) => 1,
            Expr::Id(_) => 1,
            Expr::List(_) => 1,
            Expr::Pass { .. } => 1,
            Expr::Call { .. } => 1,
            Expr::Length => 1,
            Expr::Catch { .. } => 1,
        };
        15 - cpp_ref_prep
    }
}

const INDENT_LEVEL: usize = 4;

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

    fn unparse_expr(&self, current_expr: &Expr) -> Result<String, anyhow::Error> {
        let brace_if_lower = |expr: &Expr| -> String {
            if expr.precedence() < current_expr.precedence() {
                format!("({})", self.unparse_expr(expr).unwrap())
            } else {
                self.unparse_expr(expr).unwrap()
            }
        };
        match current_expr {
            Expr::Assign { left, right } => {
                let left_frag = self.unparse_expr(left)?;
                let right_frag = self.unparse_expr(right)?;
                Ok(format!("{left_frag} = {right_frag}"))
            }
            Expr::Pass { args } => {
                let mut buffer = String::new();
                buffer.push_str("pass");
                if !args.is_empty() {
                    buffer.push('(');
                    buffer.push_str(self.unparse_args(args).unwrap().as_str());
                    buffer.push(')');
                }
                Ok(buffer)
            }
            Expr::VarExpr(var) => Ok(self.unparse_var(var, false)),
            Expr::Id(id) => Ok(self.names.name_of(id).unwrap().to_string()),
            Expr::Binary(op, left_expr, right_expr) => Ok(format!(
                "{} {} {}",
                brace_if_lower(left_expr),
                op,
                brace_if_lower(right_expr)
            )),
            Expr::And(left, right) => Ok(format!(
                "{} && {}",
                brace_if_lower(left),
                brace_if_lower(right)
            )),
            Expr::Or(left, right) => Ok(format!(
                "{} || {}",
                brace_if_lower(left),
                brace_if_lower(right)
            )),
            Expr::Unary(op, expr) => Ok(format!("{}{}", op, brace_if_lower(expr))),
            Expr::Prop { location, property } => {
                let location = match (&**location, &**property) {
                    (Expr::VarExpr(var), Expr::VarExpr(_)) if var.is_root() => String::from("$"),
                    _ => format!("{}.", self.unparse_expr(location).unwrap()),
                };
                let prop = match &**property {
                    Expr::VarExpr(var) => format!("{}", self.unparse_var(var, true)),
                    _ => format!("({})", self.unparse_expr(property)?),
                };
                Ok(format!("{location}{prop}"))
            }
            Expr::Verb {
                location,
                verb,
                args,
            } => {
                let location = match (&**location, &**verb) {
                    (Expr::VarExpr(var), Expr::VarExpr(_)) if var.is_root() => String::from("$"),
                    _ => format!("{}:", self.unparse_expr(location).unwrap()),
                };
                let verb = match &**verb {
                    Expr::VarExpr(var) => self.unparse_var(var, true),
                    _ => format!("({})", self.unparse_expr(verb).unwrap()),
                };
                let mut buffer = String::new();
                buffer.push_str(format!("{location}{verb}").as_str());
                buffer.push('(');
                buffer.push_str(self.unparse_args(args)?.as_str());
                buffer.push(')');
                Ok(buffer)
            }
            Expr::Call { function, args } => {
                let mut buffer = String::new();
                buffer.push_str(function);
                buffer.push('(');
                buffer.push_str(self.unparse_args(args)?.as_str());
                buffer.push(')');
                Ok(buffer)
            }
            Expr::Range { base, from, to } => Ok(format!(
                "{}[{}..{}]",
                brace_if_lower(base),
                brace_if_lower(from),
                brace_if_lower(to)
            )),
            Expr::Cond {
                condition,
                consequence,
                alternative,
            } => Ok(format!(
                "{} ? {} | {}",
                self.unparse_expr(condition).unwrap(),
                self.unparse_expr(consequence).unwrap(),
                self.unparse_expr(alternative).unwrap()
            )),
            Expr::Catch {
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
            Expr::Index(lvalue, index) => Ok(format!(
                "{}[{}]",
                self.unparse_expr(lvalue).unwrap(),
                self.unparse_expr(index).unwrap()
            )),
            Expr::List(list) => {
                let mut buffer = String::new();
                buffer.push('{');
                buffer.push_str(self.unparse_args(list)?.as_str());
                buffer.push('}');
                Ok(buffer)
            }
            Expr::Scatter(vars, expr) => {
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
                    buffer.push_str(self.names.name_of(&var.id)?);
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
            Expr::Length => Ok(String::from("$")),
        }
    }

    fn unparse_stmt(&self, stmt: &Stmt, indent: usize) -> Result<Vec<String>, anyhow::Error> {
        let indent_frag = " ".repeat(indent);
        // Statements should not end in a newline, but should be terminated with a semicolon.
        match &stmt.node {
            StmtNode::Cond { arms, otherwise } => {
                let mut stmt_lines = Vec::with_capacity(arms.len() + 2);
                let cond_frag = self.unparse_expr(&arms[0].condition)?;
                let mut stmt_frag =
                    self.unparse_stmts(&arms[0].statements, indent + INDENT_LEVEL)?;
                stmt_lines.push(format!("{}if ({})", indent_frag, cond_frag));
                stmt_lines.append(&mut stmt_frag);
                for arm in arms.iter().skip(1) {
                    let cond_frag = self.unparse_expr(&arm.condition)?;
                    let mut stmt_frag =
                        self.unparse_stmts(&arm.statements, indent + INDENT_LEVEL)?;
                    stmt_lines.push(format!("{}elseif ({})", indent_frag, cond_frag));
                    stmt_lines.append(&mut stmt_frag);
                }
                if !otherwise.is_empty() {
                    let mut stmt_frag = self.unparse_stmts(otherwise, indent + INDENT_LEVEL)?;
                    stmt_lines.push(format!("{}else", indent_frag));
                    stmt_lines.append(&mut stmt_frag);
                }
                stmt_lines.push(format!("{}endif", indent_frag));
                Ok(stmt_lines)
            }
            StmtNode::ForList { id, expr, body } => {
                let mut stmt_lines = Vec::with_capacity(body.len() + 3);

                let expr_frag = self.unparse_expr(expr)?;
                let mut stmt_frag = self.unparse_stmts(body, indent + INDENT_LEVEL)?;
                stmt_lines.push(format!(
                    "{}for {} in ({})",
                    indent_frag,
                    self.names.name_of(id)?,
                    expr_frag
                ));
                stmt_lines.append(&mut stmt_frag);
                stmt_lines.push(format!("{}endfor", indent_frag));
                Ok(stmt_lines)
            }
            StmtNode::ForRange { id, from, to, body } => {
                let mut stmt_lines = Vec::with_capacity(body.len() + 3);

                let from_frag = self.unparse_expr(from)?;
                let to_frag = self.unparse_expr(to)?;
                let mut stmt_frag = self.unparse_stmts(body, indent + INDENT_LEVEL)?;

                stmt_lines.push(format!(
                    "{}for {} in [{}..{}]",
                    indent_frag,
                    self.names.name_of(id)?,
                    from_frag,
                    to_frag
                ));
                stmt_lines.append(&mut stmt_frag);
                stmt_lines.push(format!("{}endfor", indent_frag));
                Ok(stmt_lines)
            }
            StmtNode::While {
                id,
                condition,
                body,
            } => {
                let mut stmt_lines = Vec::with_capacity(body.len() + 3);

                let cond_frag = self.unparse_expr(condition)?;
                let mut stmt_frag = self.unparse_stmts(body, indent + INDENT_LEVEL)?;

                let mut base_str = "while ".to_string();
                if let Some(id) = id {
                    base_str.push_str(self.names.name_of(id)?);
                }
                stmt_lines.push(format!("{}({})", base_str, cond_frag));
                stmt_lines.append(&mut stmt_frag);
                stmt_lines.push(format!("{}endwhile", indent_frag));
                Ok(stmt_lines)
            }
            StmtNode::Fork { id, time, body } => {
                let mut stmt_lines = Vec::with_capacity(body.len() + 3);

                let delay_frag = self.unparse_expr(time)?;
                let mut stmt_frag = self.unparse_stmts(body, indent + INDENT_LEVEL)?;
                let mut base_str = format!("{}fork", indent_frag);
                if let Some(id) = id {
                    base_str.push_str(self.names.name_of(id)?);
                }
                stmt_lines.push(format!("{}({})", base_str, delay_frag));
                stmt_lines.append(&mut stmt_frag);
                stmt_lines.push(format!("{}endfork", indent_frag));
                Ok(stmt_lines)
            }
            StmtNode::TryExcept { body, excepts } => {
                let mut stmt_lines = Vec::with_capacity(body.len() + 3);

                let mut stmt_frag = self.unparse_stmts(body, indent + INDENT_LEVEL)?;
                stmt_lines.push("try".to_string());
                stmt_lines.append(&mut stmt_frag);
                for except in excepts {
                    let mut stmt_frag =
                        self.unparse_stmts(&except.statements, indent + INDENT_LEVEL)?;
                    let mut base_str = "except ".to_string();
                    if let Some(id) = &except.id {
                        base_str.push_str(self.names.name_of(id)?);
                        base_str.push(' ');
                    }
                    let catch_codes = self.unparse_catch_codes(&except.codes)?;
                    base_str.push_str(format!("({catch_codes})").as_str());
                    stmt_lines.push(base_str);
                    stmt_lines.append(&mut stmt_frag);
                }
                stmt_lines.push(format!("{}endtry", indent_frag));
                Ok(stmt_lines)
            }
            StmtNode::TryFinally { body, handler } => {
                let mut stmt_lines = Vec::with_capacity(body.len() + 3);

                let mut stmt_frag = self.unparse_stmts(body, indent + INDENT_LEVEL)?;
                let mut handler_frag = self.unparse_stmts(handler, indent + INDENT_LEVEL)?;
                stmt_lines.push("try".to_string());
                stmt_lines.append(&mut stmt_frag);
                stmt_lines.push("finally".to_string());
                stmt_lines.append(&mut handler_frag);
                stmt_lines.push(format!("{}endtry", indent_frag));
                Ok(stmt_lines)
            }
            StmtNode::Break { exit } => {
                let mut base_str = "break".to_string();
                if let Some(exit) = &exit {
                    base_str.push(' ');
                    base_str.push_str(self.names.name_of(exit)?);
                }
                base_str.push(';');
                Ok(vec![base_str])
            }
            StmtNode::Continue { exit } => {
                let mut base_str = "continue".to_string();
                if let Some(exit) = &exit {
                    base_str.push(' ');
                    base_str.push_str(self.names.name_of(exit)?);
                }
                base_str.push(';');
                Ok(vec![base_str])
            }
            StmtNode::Return { expr } => Ok(match expr {
                None => {
                    vec![format!("{}return;", indent_frag)]
                }
                Some(e) => {
                    vec![format!("{}return {};", indent_frag, self.unparse_expr(e)?)]
                }
            }),
            StmtNode::Expr(expr) => Ok(vec![format!(
                "{}{};",
                indent_frag,
                self.unparse_expr(expr)?
            )]),
        }
    }

    pub fn unparse_stmts(
        &self,
        stms: &[Stmt],
        indent: usize,
    ) -> Result<Vec<String>, anyhow::Error> {
        let mut results = vec![];
        for stmt in stms {
            results.append(&mut self.unparse_stmt(stmt, indent)?);
        }
        Ok(results)
    }
}

pub fn unparse(tree: &Parse) -> Result<Vec<String>, anyhow::Error> {
    let unparse = Unparse::new(tree.names.clone());
    unparse.unparse_stmts(&tree.stmts, 0)
}

/// Walk a syntax tree and annotate each statement with line number that corresponds to what would
/// have been generated by `unparse`
/// Used for generating line number spans in the bytecode.
pub fn annotate_line_numbers(start_line_no: usize, tree: &mut [Stmt]) -> usize {
    let mut line_no = start_line_no;
    for stmt in tree.iter_mut() {
        stmt.tree_line_no = line_no;
        match &mut stmt.node {
            StmtNode::Cond {
                ref mut arms,
                ref mut otherwise,
            } => {
                // IF & ELSEIFS
                for arm in arms.iter_mut() {
                    // IF / ELSEIF line
                    line_no += 1;
                    // Walk arm.statements ...
                    line_no = annotate_line_numbers(line_no, &mut arm.statements);
                }
                if !otherwise.is_empty() {
                    // ELSE line ...
                    line_no += 1;
                    // Walk otherwise ...
                    line_no = annotate_line_numbers(line_no, otherwise);
                }
                // ENDIF
                line_no += 1;
            }
            StmtNode::ForList { ref mut body, .. }
            | StmtNode::ForRange { ref mut body, .. }
            | StmtNode::While { ref mut body, .. }
            | StmtNode::Fork { ref mut body, .. } => {
                // FOR/WHILE/FORK
                line_no += 1;
                // Walk body ...
                line_no = annotate_line_numbers(line_no, body);
                // ENDFOR/ENDWHILE/ENDFORK
                line_no += 1;
            }
            StmtNode::Expr(_)
            | StmtNode::Break { .. }
            | StmtNode::Continue { .. }
            | StmtNode::Return { .. } => {
                // All single-line statements.
                line_no += 1;
            }
            StmtNode::TryExcept {
                ref mut body,
                ref mut excepts,
            } => {
                // TRY
                line_no += 1;
                // Walk body ...
                line_no = annotate_line_numbers(line_no, body);
                // Excepts
                for except in excepts {
                    // EXCEPT <...>
                    line_no += 1;
                    line_no = annotate_line_numbers(line_no, &mut except.statements);
                }
                // ENDTRY
                line_no += 1;
            }
            StmtNode::TryFinally {
                ref mut body,
                ref mut handler,
            } => {
                // TRY
                line_no += 1;
                // Walk body ...
                line_no = annotate_line_numbers(line_no, body);
                // FINALLY
                line_no += 1;
                // Walk handler ...
                line_no = annotate_line_numbers(line_no, handler);
                // ENDTRY
                line_no += 1;
            }
        }
    }
    line_no
}

#[cfg(test)]
mod tests {
    use crate::compiler::ast::assert_trees_match_recursive;
    use pretty_assertions::assert_eq;
    use test_case::test_case;
    use unindent::unindent;

    use super::*;

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
    #[test_case(r#"return "test";"#; "string literal")]
    #[test_case(r#"return "test \"test\"";"#; "string literal with escaped quote")]
    #[test_case(r#"return #1.test;"#; "property access")]
    #[test_case(r#"return #1:test(1, 2, 3);"#; "verb call")]
    #[test_case(r#"return #1:test();"#; "verb call no args")]
    #[test_case(r#"return $test(1);"#; "sysverb")]
    #[test_case(r#"return $options;"#; "sysprop")]
    #[test_case(r#"options = "test";
    return #0.(options);"#; "sysobj prop expr")]
    pub fn compare_parse_roundtrip(original: &str) {
        let stripped = unindent(original);
        let result = parse_and_unparse(&stripped).unwrap();

        // Compare the stripped version of the original to the stripped version of the result, they
        // should end up identical.
        assert_eq!(stripped.trim(), result.trim());

        // Now parse both again, and verify that the complete ASTs match, ignoring the parser line
        // numbers, but validating everything else.
        let parsed_original = crate::compiler::parse::parse_program(&stripped).unwrap();
        let parsed_decompiled = crate::compiler::parse::parse_program(&result).unwrap();
        assert_trees_match_recursive(&parsed_original.stmts, &parsed_decompiled.stmts)
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
        assert_eq!(stripped.trim(), result.trim());
    }

    pub fn parse_and_unparse(original: &str) -> Result<String, anyhow::Error> {
        let tree = crate::compiler::parse::parse_program(original).unwrap();
        Ok(unparse(&tree)?.join("\n"))
    }
}
