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

mod expr;
mod stmt;

use crate::{
    ast,
    ast::{Stmt, StmtNode},
    decompile::DecompileError,
    parse::Parse,
};
use base64::{Engine, engine::general_purpose};
use moor_common::util::quote_str;
use moor_var::{Obj, Var, Variant, program::opcode::ScatterLabel};
use std::collections::HashMap;

/// This could probably be combined with the structure for Parse.
#[derive(Debug)]
pub(crate) struct Unparse<'a> {
    tree: &'a Parse,
    fully_paren: bool,
    indent_width: usize,
}

const INDENT_LEVEL: usize = 2;

impl<'a> Unparse<'a> {
    fn new(tree: &'a Parse, fully_paren: bool, should_indent: bool) -> Self {
        let indent_width = if should_indent { INDENT_LEVEL } else { 0 };
        Self {
            tree,
            fully_paren,
            indent_width,
        }
    }

    fn write_arg<W: std::fmt::Write>(
        &self,
        arg: &ast::Arg,
        writer: &mut W,
    ) -> Result<(), DecompileError> {
        match arg {
            ast::Arg::Normal(expr) => self.write_expr(expr, writer),
            ast::Arg::Splice(expr) => {
                write!(writer, "@")?;
                self.write_expr(expr, writer)
            }
        }
    }

    fn write_args<W: std::fmt::Write>(
        &self,
        args: &[ast::Arg],
        writer: &mut W,
    ) -> Result<(), DecompileError> {
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                write!(writer, ", ")?;
            }
            self.write_arg(arg, writer)?;
        }
        Ok(())
    }

    fn write_catch_codes<W: std::fmt::Write>(
        &self,
        codes: &ast::CatchCodes,
        writer: &mut W,
    ) -> Result<(), DecompileError> {
        match codes {
            ast::CatchCodes::Codes(codes) => self.write_args(codes, writer),
            ast::CatchCodes::Any => write!(writer, "ANY").map_err(Into::into),
        }
    }

    /// Format lambda parameters as a comma-separated string with proper prefixes.
    /// Used by both simple (`{params} => expr`) and complex (`fn (params) ... endfn`) lambda syntax.
    fn write_lambda_params<W: std::fmt::Write>(
        &self,
        params: &[ast::ScatterItem],
        writer: &mut W,
    ) -> Result<(), DecompileError> {
        for (i, param) in params.iter().enumerate() {
            if i > 0 {
                write!(writer, ", ")?;
            }

            let prefix = match param.kind {
                ast::ScatterKind::Required => "",
                ast::ScatterKind::Optional => "?",
                ast::ScatterKind::Rest => "@",
            };
            let name = self.unparse_variable(&param.id);
            if let Some(default) = &param.expr {
                write!(writer, "{}{} = ", prefix, name.as_arc_str())?;
                self.write_expr(default, writer)?;
            } else {
                write!(writer, "{}{}", prefix, name.as_arc_str())?;
            }
        }
        Ok(())
    }

    fn write_scatter_items<W: std::fmt::Write>(
        &self,
        items: &[ast::ScatterItem],
        writer: &mut W,
    ) -> Result<(), DecompileError> {
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                write!(writer, ", ")?;
            }

            let prefix = match item.kind {
                ast::ScatterKind::Required => "",
                ast::ScatterKind::Optional => "?",
                ast::ScatterKind::Rest => "@",
            };
            let name = self.unparse_variable(&item.id);
            if let Some(expr) = &item.expr {
                write!(writer, "{prefix}{} = ", name.as_arc_str())?;
                self.write_expr(expr, writer)?;
            } else {
                write!(writer, "{prefix}{}", name.as_arc_str())?;
            }
        }
        Ok(())
    }

    fn write_indent<W: std::fmt::Write>(
        &self,
        indent: usize,
        writer: &mut W,
    ) -> Result<(), DecompileError> {
        for _ in 0..(indent * self.indent_width) {
            writer.write_char(' ')?;
        }
        Ok(())
    }
}

fn append_spaces(buffer: &mut String, count: usize) {
    for _ in 0..count {
        buffer.push(' ');
    }
}

pub fn unparse(
    tree: &Parse,
    fully_paren: bool,
    indent: bool,
) -> Result<Vec<String>, DecompileError> {
    let unparse = Unparse::new(tree, fully_paren, indent);
    let mut buffer = String::new();

    unparse.unparse_stmts(&tree.stmts, &mut buffer, 0)?;
    Ok(buffer.lines().map(|s| s.to_string()).collect())
}

/// Walk a syntax tree and annotate each statement with line number that corresponds to what would
/// have been generated by `unparse`
/// Used for generating line number spans in the bytecode.
pub fn annotate_line_numbers(start_line_no: usize, tree: &mut [Stmt]) -> usize {
    let mut line_no = start_line_no;
    for stmt in tree.iter_mut() {
        stmt.tree_line_no = line_no;
        match &mut stmt.node {
            StmtNode::Cond { arms, otherwise } => {
                // IF & ELSEIFS
                for arm in arms.iter_mut() {
                    // IF / ELSEIF line
                    line_no += 1;
                    // Walk arm.statements ...
                    line_no = annotate_line_numbers(line_no, &mut arm.statements);
                }
                if let Some(otherwise) = otherwise {
                    // ELSE line ...
                    line_no += 1;
                    // Walk otherwise ...
                    line_no = annotate_line_numbers(line_no, &mut otherwise.statements);
                }
                // ENDIF
                line_no += 1;
            }
            StmtNode::ForList { body, .. }
            | StmtNode::ForRange { body, .. }
            | StmtNode::While { body, .. }
            | StmtNode::Fork { body, .. } => {
                // FOR/WHILE/FORK
                line_no += 1;
                // Walk body ...
                line_no = annotate_line_numbers(line_no, body);
                // ENDFOR/ENDWHILE/ENDFORK
                line_no += 1;
            }
            StmtNode::Expr(_) | StmtNode::Break { .. } | StmtNode::Continue { .. } => {
                // All single-line statements.
                line_no += 1;
            }
            StmtNode::TryExcept {
                body,
                excepts,
                environment_width: _,
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
                body,
                handler,
                environment_width: _,
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
            StmtNode::Scope {
                body,
                num_bindings: _,
            } => {
                // BEGIN
                line_no += 1;
                // Walk body ...
                line_no = annotate_line_numbers(line_no, body);
                // ENDLET
                line_no += 1;
            }
        }
    }
    line_no
}

/// Utility function to produce a MOO literal from a Var/Variant.
/// This is kept in `compiler` and not in `common` because it's specific to the MOO language, and
/// other languages could have different representations.
pub fn to_literal(v: &Var) -> String {
    match v.variant() {
        Variant::None => "None".to_string(),
        Variant::Obj(oid) => {
            format!("{oid}")
        }
        Variant::Bool(b) => {
            format!("{b}")
        }
        Variant::Int(i) => i.to_string(),
        Variant::Float(f) => {
            format!("{f:?}")
        }
        Variant::List(l) => {
            let mut result = String::new();
            result.push('{');
            for (i, v) in l.iter().enumerate() {
                if i > 0 {
                    result.push_str(", ");
                }
                result.push_str(to_literal(&v).as_str());
            }
            result.push('}');
            result
        }
        Variant::Str(s) => quote_str(s.as_str()),
        Variant::Map(m) => {
            let mut result = String::new();
            result.push('[');
            for (i, (k, v)) in m.iter().enumerate() {
                if i > 0 {
                    result.push_str(", ");
                }
                result.push_str(to_literal(&k).as_str());
                result.push_str(" -> ");
                result.push_str(to_literal(&v).as_str());
            }
            result.push(']');
            result
        }
        Variant::Err(e) => {
            let err_name = e.name().to_string().to_uppercase();
            // If there's a message, format as E_CODE("message")
            if let Some(msg) = &e.msg {
                format!("{}({})", err_name, quote_str(msg.as_str()))
            } else {
                err_name
            }
        }
        Variant::Flyweight(fl) => {
            // Syntax:
            // < delegate, .slot = value, ..., { ... } >
            let mut result = String::new();
            result.push('<');
            result.push_str(fl.delegate().to_literal().as_str());
            let slots = fl.slots_storage();
            if !slots.is_empty() {
                for (k, v) in slots.iter() {
                    result.push_str(", .");
                    result.push_str(&k.as_arc_str());
                    result.push_str(" = ");
                    result.push_str(to_literal(v).as_str());
                }
            }
            let v = fl.contents();
            if !v.is_empty() {
                result.push_str(", {");
                for (i, v) in v.iter().enumerate() {
                    if i > 0 {
                        result.push_str(", ");
                    }
                    result.push_str(to_literal(&v).as_str());
                }
                result.push('}');
            }

            result.push('>');
            result
        }
        Variant::Sym(s) => {
            format!("'{}", s.as_arc_str())
        }
        Variant::Binary(b) => {
            let encoded = general_purpose::URL_SAFE.encode(b.as_bytes());
            format!("b\"{encoded}\"")
        }
        Variant::Lambda(l) => {
            use crate::decompile;
            use moor_var::program::opcode::ScatterLabel;

            // Build parameter list with proper names and syntax
            let param_strings: Vec<String> =
                l.0.params
                    .labels
                    .iter()
                    .map(|label| match label {
                        ScatterLabel::Required(name) => {
                            // Find the variable in lambda body's var_names and get its symbol
                            if let Some(var) = l.0.body.var_names().find_variable(name) {
                                var.to_symbol().as_arc_str().to_string()
                            } else {
                                format!("param_{}", name.0) // Fallback if name not found
                            }
                        }
                        ScatterLabel::Optional(name, _) => {
                            if let Some(var) = l.0.body.var_names().find_variable(name) {
                                format!("?{}", var.to_symbol().as_arc_str())
                            } else {
                                format!("?param_{}", name.0)
                            }
                        }
                        ScatterLabel::Rest(name) => {
                            if let Some(var) = l.0.body.var_names().find_variable(name) {
                                format!("@{}", var.to_symbol().as_arc_str())
                            } else {
                                format!("@param_{}", name.0)
                            }
                        }
                    })
                    .collect();
            let param_str = param_strings.join(", ");

            // Just manually construct the lambda syntax - simpler than reconstructing AST
            let decompiled_tree = decompile::program_to_tree(&l.0.body).unwrap();
            let temp_unparse = Unparse::new(&decompiled_tree, false, true);

            // Check if this is a simple expression lambda or multi-statement
            let is_simple_expr = decompiled_tree.stmts.len() == 1
                && matches!(
                    &decompiled_tree.stmts[0].node,
                    crate::ast::StmtNode::Expr(crate::ast::Expr::Return(Some(_)))
                );

            let body_str = if is_simple_expr {
                // Expression lambda: return expr; → just show the expr
                if let crate::ast::StmtNode::Expr(crate::ast::Expr::Return(Some(expr))) =
                    &decompiled_tree.stmts[0].node
                {
                    temp_unparse.unparse_expr(expr).unwrap()
                } else {
                    unreachable!()
                }
            } else {
                // Multi-statement lambda - use fn () ... endfn syntax
                let mut buffer = String::new();
                let _ = temp_unparse.write_lambda_body_inline(&decompiled_tree.stmts, &mut buffer);
                buffer.trim_end().to_string()
            };

            let use_fn_syntax = !is_simple_expr;

            // Build metadata string for captured environment and self-reference
            let mut metadata_parts = vec![];

            if !l.0.captured_env.is_empty() {
                let mut captured_vars: Vec<String> = vec![];

                for (scope_depth, frame) in l.0.captured_env.iter().enumerate() {
                    for (var_offset, var_value) in frame.iter().enumerate() {
                        // Only include variables that are not None/v_none
                        if var_value.is_none() {
                            continue;
                        }

                        // Search for variable names in the lambda body's name table that match this scope and offset
                        let var_names = l.0.body.var_names();
                        let maybe_name = var_names
                            .names()
                            .iter()
                            .filter_map(|name| {
                                // Check if this name corresponds to our scope depth and offset
                                if name.1 as usize == scope_depth && name.0 as usize == var_offset {
                                    var_names.ident_for_name(name)
                                } else {
                                    None
                                }
                            })
                            .next();

                        match maybe_name {
                            Some(symbol) => {
                                // Include both variable name and value for clarity
                                captured_vars.push(format!(
                                    "{}: {}",
                                    symbol.as_arc_str(),
                                    to_literal(var_value)
                                ));
                            }
                            None => {
                                // Fall back to just the value if no name is found
                                captured_vars.push(to_literal(var_value));
                            }
                        }
                    }
                }

                if !captured_vars.is_empty() {
                    metadata_parts.push(format!("captured [{}]", captured_vars.join(", ")));
                }
            }

            if let Some(_self_var) = l.0.self_var {
                // For now, represent self-reference as a simple marker
                metadata_parts.push("self 1".to_string());
            }

            if use_fn_syntax {
                // Multi-statement lambda: fn (params) statements endfn
                if metadata_parts.is_empty() {
                    format!("fn ({param_str}) {body_str} endfn")
                } else {
                    format!(
                        "fn ({param_str}) {body_str} endfn with {}",
                        metadata_parts.join(" ")
                    )
                }
            } else {
                // Simple expression lambda: {params} => expr
                if metadata_parts.is_empty() {
                    format!("{{{param_str}}} => {body_str}")
                } else {
                    format!(
                        "{{{param_str}}} => {body_str} with {}",
                        metadata_parts.join(" ")
                    )
                }
            }
        }
    }
}

/// Like `to_literal_objsub` but formats lists whose literal form would exceed 80 characters
/// into multiple lines with indentation.
pub fn to_literal_objsub(v: &Var, name_subs: &HashMap<Obj, String>, indent_depth: usize) -> String {
    let f = |o: &Obj| {
        if let Some(name_sub) = name_subs.get(o) {
            name_sub.clone()
        } else if o.is_anonymous() {
            // For anonymous objects, use the objdef format with internal ID
            if let Some(anon_id) = o.anonymous_objid() {
                let (autoincrement, rng, epoch_ms) = anon_id.components();
                let first_group = ((autoincrement as u64) << 6) | (rng as u64);
                format!("#anon_{first_group:06X}-{epoch_ms:010X}")
            } else {
                format!("{o}")
            }
        } else {
            format!("{o}")
        }
    };
    let mut result = String::new();

    match v.variant() {
        Variant::List(l) => {
            // First, try to format on one line
            let mut single_line = String::new();
            single_line.push('{');
            for (i, v) in l.iter().enumerate() {
                if i > 0 {
                    single_line.push_str(", ");
                }
                single_line.push_str(
                    to_literal_objsub(&v, name_subs, indent_depth + INDENT_LEVEL).as_str(),
                );
            }
            single_line.push('}');

            // If single line exceeds 80 characters, format multiline
            if single_line.len() > 80 {
                result.push('{');
                for (i, v) in l.iter().enumerate() {
                    if i > 0 {
                        result.push(',');
                    }
                    result.push('\n');
                    append_spaces(&mut result, indent_depth + INDENT_LEVEL);
                    result.push_str(
                        to_literal_objsub(&v, name_subs, indent_depth + INDENT_LEVEL).as_str(),
                    );
                }
                result.push('\n');
                append_spaces(&mut result, indent_depth);
                result.push('}');
            } else {
                result = single_line;
            }
        }
        Variant::Map(m) => {
            // First, try to format on one line
            let mut single_line = String::new();
            single_line.push('[');
            for (i, (k, v)) in m.iter().enumerate() {
                if i > 0 {
                    single_line.push_str(", ");
                }
                single_line.push_str(
                    to_literal_objsub(&k, name_subs, indent_depth + INDENT_LEVEL).as_str(),
                );
                single_line.push_str(" -> ");
                single_line.push_str(
                    to_literal_objsub(&v, name_subs, indent_depth + INDENT_LEVEL).as_str(),
                );
            }
            single_line.push(']');

            // If single line exceeds 80 characters, format multiline
            if single_line.len() > 80 {
                result.push('[');
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 {
                        result.push(',');
                    }
                    result.push('\n');
                    append_spaces(&mut result, indent_depth + INDENT_LEVEL);
                    result.push_str(
                        to_literal_objsub(&k, name_subs, indent_depth + INDENT_LEVEL).as_str(),
                    );
                    result.push_str(" -> ");
                    result.push_str(
                        to_literal_objsub(&v, name_subs, indent_depth + INDENT_LEVEL).as_str(),
                    );
                }
                result.push('\n');
                append_spaces(&mut result, indent_depth);
                result.push(']');
            } else {
                result = single_line;
            }
        }
        Variant::Flyweight(fl) => {
            // Syntax:
            // < delegate, .slot = value, ..., { ... } >
            result.push('<');
            result.push_str(&f(fl.delegate()));
            let slots = fl.slots_storage();
            if !slots.is_empty() {
                for (k, v) in slots.iter() {
                    result.push_str(", .");
                    result.push_str(&k.as_arc_str());
                    result.push_str(" = ");
                    result.push_str(
                        to_literal_objsub(v, name_subs, indent_depth + INDENT_LEVEL).as_str(),
                    );
                }
            }
            let v = fl.contents();
            if !v.is_empty() {
                result.push_str(", {");
                for (i, v) in v.iter().enumerate() {
                    if i > 0 {
                        result.push_str(", ");
                    }
                    result.push_str(
                        to_literal_objsub(&v, name_subs, indent_depth + INDENT_LEVEL).as_str(),
                    );
                }
                result.push('}');
            }

            result.push('>');
        }
        Variant::Obj(oid) => {
            result.push_str(&f(&oid));
        }
        Variant::Lambda(l) => {
            // Special objdef formatting for lambdas - needs to match the grammar:
            // lambda_captured = { "captured" ~ "[" ~ (captured_var_map ~ ("," ~ captured_var_map)*)? ~ "]" }
            // captured_var_map = { "{" ~ (captured_var_entry ~ ("," ~ captured_var_entry)*)? ~ "}" }
            // captured_var_entry = { ident ~ ":" ~ literal }

            // Build parameter string
            let param_str =
                l.0.params
                    .labels
                    .iter()
                    .map(|label| match label {
                        ScatterLabel::Required(name) => {
                            l.0.body
                                .var_names()
                                .ident_for_name(name)
                                .map(|s| s.as_arc_str().to_string())
                                .unwrap_or_else(|| "x".to_string())
                        }
                        ScatterLabel::Optional(name, _) => {
                            let var_name =
                                l.0.body
                                    .var_names()
                                    .ident_for_name(name)
                                    .map(|s| s.as_arc_str().to_string())
                                    .unwrap_or_else(|| "x".to_string());
                            format!("?{var_name}")
                        }
                        ScatterLabel::Rest(name) => {
                            let var_name =
                                l.0.body
                                    .var_names()
                                    .ident_for_name(name)
                                    .map(|s| s.as_arc_str().to_string())
                                    .unwrap_or_else(|| "x".to_string());
                            format!("@{var_name}")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");

            // Build body string - for objdef we need to reconstruct the lambda body
            // For now, use a simple placeholder that's valid MOO syntax
            let body_str = "1"; // Simple placeholder

            // Build metadata for objdef format
            let mut metadata_parts = vec![];

            if !l.0.captured_env.is_empty() {
                let mut scope_maps = vec![];

                for (scope_depth, frame) in l.0.captured_env.iter().enumerate() {
                    let mut captured_vars: Vec<String> = vec![];

                    for (var_offset, var_value) in frame.iter().enumerate() {
                        if var_value.is_none() {
                            continue;
                        }

                        let var_names = l.0.body.var_names();
                        let maybe_name = var_names
                            .names()
                            .iter()
                            .filter_map(|name| {
                                if name.1 as usize == scope_depth && name.0 as usize == var_offset {
                                    var_names.ident_for_name(name)
                                } else {
                                    None
                                }
                            })
                            .next();

                        if let Some(symbol) = maybe_name {
                            captured_vars.push(format!(
                                "{}: {}",
                                symbol.as_arc_str(),
                                to_literal_objsub(
                                    var_value,
                                    name_subs,
                                    indent_depth + INDENT_LEVEL
                                )
                            ));
                        }
                    }

                    if !captured_vars.is_empty() {
                        scope_maps.push(format!("{{{}}}", captured_vars.join(", ")));
                    }
                }

                if !scope_maps.is_empty() {
                    metadata_parts.push(format!("captured [{}]", scope_maps.join(", ")));
                }
            }

            if let Some(_self_var) = l.0.self_var {
                metadata_parts.push("self 1".to_string());
            }

            if metadata_parts.is_empty() {
                result.push_str(&format!("{{{param_str}}} => {body_str}"));
            } else {
                result.push_str(&format!(
                    "{{{param_str}}} => {body_str} with {}",
                    metadata_parts.join(" ")
                ));
            }
        }
        _ => {
            result.push_str(to_literal(v).as_str());
        }
    };
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CompileOptions, ast::assert_trees_match_recursive};

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
    basic = 5;
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
    #[test_case(r#"{a, b, ?c, @d} = args;"#; "scatter assign")]
    #[test_case(r#"{?a = 5} = args;"#; "scatter assign optional expression argument")]
    #[test_case(r#"5;
           fork (5)
             1;
           endfork
           2;"#; "unlabelled fork decompile")]
    #[test_case(r#"5;
           fork tst (5)
             1;
           endfork
           2;"#; "labelled fork decompile")]
    #[test_case(r#"while (1)
             continue;
             break;
           endwhile"#; "continue decompile")]
    #[test_case(r#"this:("@listgag")();"#; "verb expr escaping @")]
    #[test_case(r#"this:("listgag()")();"#; "verb expr escaping brackets ")]
    #[test_case(r#"1 ^ 2;"#; "exponents")]
    #[test_case(r#"(a + b)[1];"#; "index precedence")]
    #[test_case(r#"a ? b | (c ? d | e);"#; "conditional precedence")]
    #[test_case(r#"1 ^ (2 + 3);"#; "exponent precedence")]
    #[test_case(r#"(1 + 2) ^ (2 + 3);"#; "exponent precedence 2")]
    #[test_case(r#"verb[1..5 - 1];"#; "range precedence")]
    #[test_case(r#"1 && ((a = 5) && 3);"#; "and/or precedence")]
    #[test_case(r#"n + 10 in a;"#; "in precedence")]
    #[test_case(r#"begin
  let a = 5;
  a = a + 3;
end"#; "variable declaration and reassignment")]
    #[test_case(r#"begin
  let {a, b} = {1, 2};
  a = 5;
  b = 2;
end"#; "scatter declaration and reassignment")]
    #[test_case(r#"begin
  let {a, ?b = 5, @c} = {1, 2, 3};
  b = 2;
  c = 1;
end"#; "complex scatter declaration with optional and rest")]
    #[test_case("3 &. 1;\n"; "bitwise and")]
    #[test_case("5 |. 2;\n"; "bitwise or")]
    #[test_case("6 ^. 3;\n"; "bitwise xor")]
    #[test_case("8 << 1;\n"; "left shift")]
    #[test_case("16 >> 2;\n"; "right shift")]
    #[test_case("~5;\n"; "bitwise not")]
    #[test_case("3 &. 1 |. 2;\n"; "bitwise and or precedence")]
    #[test_case("(3 |. 1) &. 2;\n"; "bitwise parentheses")]
    pub fn compare_parse_roundtrip(original: &str) {
        let stripped = unindent(original);
        let result = parse_and_unparse(&stripped).unwrap();

        // Compare the stripped version of the original to the stripped version of the result, they
        // should end up identical.
        assert_eq!(stripped.trim(), result.trim());

        // Now parse both again, and verify that the complete ASTs match, ignoring the parser line
        // numbers, but validating everything else.
        let parsed_original =
            crate::parse::parse_program(&stripped, CompileOptions::default()).unwrap();
        let parsed_decompiled =
            crate::parse::parse_program(&result, CompileOptions::default()).unwrap();
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

    #[test]
    fn test_unparse_lexical_scope_block() {
        let program = r#"b = 3;
        begin
          let a = 5;
        end"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn regress_test() {
        let program = r#"n + 10 in a;"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn test_local_scatter() {
        let program = r#"begin
          let {things, ?nothingstr = "nothing", ?andstr = " and ", ?commastr = ", ", ?finalcommastr = ","} = args;
        end"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn test_local_const() {
        let program = r#"begin
          const {things, ?nothingstr = "nothing", ?andstr = " and ", ?commastr = ", ", ?finalcommastr = ","} = args;
        end"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn test_flyweight() {
        let program = r#"return <#1, .slot = "123", {1, 2, 3}>;"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn for_range_comprehension() {
        let program = r#"return { x * 2 for x in [1..3] };"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn for_list_comprehension() {
        let program = r#"return { x * 2 for x in ({1, 2, 3}) };"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn for_v_x_in_map() {
        let program = r#" for v, k in (["a" -> "b", "c" -> "d"])
        endfor"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn for_v_x_in_list() {
        let program = r#" for v, k in ({1, 2, 3})
        endfor"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    pub fn parse_and_unparse(original: &str) -> Result<String, DecompileError> {
        let tree = crate::parse::parse_program(original, CompileOptions::default()).unwrap();
        Ok(unparse(&tree, false, true)?.join("\n"))
    }

    #[test]
    fn test_unparse_empty_map_regression() {
        let program = r#"return [];"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn test_type_literals() {
        let program = r#"return {TYPE_INT, TYPE_STR, TYPE_OBJ, TYPE_LIST, TYPE_MAP, TYPE_SYM, TYPE_FLYWEIGHT};"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn test_legacy_type_literals_migration() {
        // Test that legacy type constants (INT, OBJ, STR, etc.) are parsed correctly
        // when legacy_type_constants is enabled, and that they output as TYPE_* forms.
        use crate::parse::{CompileOptions, parse_program};

        let legacy_options = CompileOptions {
            legacy_type_constants: true,
            ..Default::default()
        };

        // Parse with legacy mode - should parse INT as type constant
        let legacy_program = r#"return typeof(x) == INT;"#;
        let tree = parse_program(legacy_program, legacy_options.clone());
        assert!(
            tree.is_ok(),
            "Legacy type constant should parse: {:?}",
            tree
        );

        // Parse legacy and unparse - should output TYPE_INT
        let tree = tree.unwrap();
        let unparsed = unparse(&tree, false, true)
            .expect("Failed to unparse")
            .join("\n");
        assert!(
            unparsed.contains("TYPE_INT"),
            "Legacy INT should unparse as TYPE_INT, got: {}",
            unparsed
        );

        // Without legacy mode, INT should be treated as a variable
        let normal_options = CompileOptions::default();
        let normal_tree = parse_program(legacy_program, normal_options);
        assert!(normal_tree.is_ok(), "INT as variable should parse");
        let unparsed = unparse(&normal_tree.unwrap(), false, true)
            .expect("Unparse")
            .join("\n");
        // INT should appear as a variable name (lowercase), not TYPE_INT
        assert!(
            !unparsed.contains("TYPE_INT"),
            "Without legacy mode, INT should be a variable, got: {}",
            unparsed
        );
    }

    #[test]
    fn test_lambda_unparse_simple() {
        let program = r#"return {x} => x + 1;"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn test_lambda_unparse_optional() {
        let program = r#"return {x, ?y} => x + y;"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn test_lambda_unparse_optional_with_default() {
        let program = r#"return {x, ?y = 5} => x + y;"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn test_lambda_unparse_rest() {
        let program = r#"return {x, @rest} => x + length(rest);"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn test_lambda_unparse_complex() {
        let program = r#"return {x, ?y = 5, @rest} => x + y + length(rest);"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn test_named_function_unparse() {
        let program = r#"fn x(y)
          return y * x(2);
        endfn
        return x(2);"#;
        let stripped = unindent(program);

        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn test_lambda_unparse_no_params() {
        let program = r#"return {} => 42;"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn test_chained_lambda_call_unparse() {
        // Test that chained lambda calls like make_getter()() round-trip correctly
        let program = r#"return make_getter()();"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn test_chained_lambda_call_with_args_unparse() {
        // Test chained calls with arguments
        let program = r#"return make_adder(5)(10);"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn test_operator_precedence_and_or_regression_minimal() {
        // Minimal test case for operator precedence bug
        // The issue is that parentheses are being removed when they shouldn't be
        let program = r#"if (a || (!b && c))
          return 1;
        endif"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    #[ignore]
    fn test_operator_precedence_and_or_regression_full() {
        // Full complex expression from LambdaMOO's match_verb for reference
        let program = r#"if (vargs[2] == "any" || (!prepstr && vargs[2] == "none") || index("/" + vargs[2] + "/", "/" + prepstr + "/") && (vargs[1] == "any" || (!dobjstr && vargs[1] == "none") || (dobj == what && vargs[1] == "this")) && (vargs[3] == "any" || (!iobjstr && vargs[3] == "none") || (iobj == what && vargs[3] == "this")) && index(verb_info(where[1], vrb)[2], "x") && verb_code(where[1], vrb))
          return 1;
        endif"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn test_fully_paren_formatting() {
        let program = r#"return 1 + 2 * 3;"#;
        let tree = crate::parse::parse_program(program, CompileOptions::default()).unwrap();

        // Test normal precedence (should be: 1 + 2 * 3)
        let normal = unparse(&tree, false, true).unwrap().join("\n");
        assert_eq!(normal.trim(), "return 1 + 2 * 3;");

        // Test fully parenthesized (should be: (1) + ((2) * (3)))
        let fully_paren = unparse(&tree, true, true).unwrap().join("\n");
        assert_eq!(fully_paren.trim(), "return 1 + (2 * 3);");
    }

    #[test]
    fn test_unindented_formatting() {
        let program = r#"if (1)
  return 2;
endif"#;
        let tree = crate::parse::parse_program(program, CompileOptions::default()).unwrap();

        // Test normal indented (should have indentation)
        let indented = unparse(&tree, false, true).unwrap().join("\n");
        assert!(indented.contains("  return 2;"));

        // Test unindented (should have no indentation)
        let unindented = unparse(&tree, false, false).unwrap().join("\n");
        let lines: Vec<&str> = unindented.lines().collect();
        assert_eq!(lines[0], "if (1)");
        assert_eq!(lines[1], "return 2;"); // No leading spaces
        assert_eq!(lines[2], "endif");
    }

    #[test]
    fn test_indented_vs_unindented() {
        let program = "if (1)\n  a = 2;\nendif";
        let tree = crate::parse::parse_program(program, CompileOptions::default()).unwrap();

        let indented = unparse(&tree, false, true).unwrap().join("\n");
        let unindented = unparse(&tree, false, false).unwrap().join("\n");

        // With indentation should have 2 spaces before "a = 2;"
        assert_eq!(indented, "if (1)\n  a = 2;\nendif");

        // Without indentation should have no leading spaces
        assert_eq!(unindented, "if (1)\na = 2;\nendif");
    }

    #[test]
    fn test_simple_fully_paren() {
        let program = "a + b;";
        let tree = crate::parse::parse_program(program, CompileOptions::default()).unwrap();

        let normal = unparse(&tree, false, true).unwrap().join("\n");
        let fully_paren = unparse(&tree, true, true).unwrap().join("\n");

        assert_eq!(normal, "a + b;");
        assert_eq!(fully_paren, "a + b;");
    }

    #[test]
    fn test_moo_left_to_right_precedence() {
        // Test the specific case from the Discord conversation
        // In MOO, || and && have the same precedence and are left-associative
        // So: a || b && c should parse as: (a || b) && c
        let program = "ticks_left() < 5000 || seconds_left() < 2 && suspend(1);";
        let tree = crate::parse::parse_program(program, CompileOptions::default()).unwrap();
        let result = unparse(&tree, false, true).unwrap().join("\n");

        // Test that roundtrip is stable
        let reparsed = crate::parse::parse_program(&result, CompileOptions::default()).unwrap();
        let result2 = unparse(&reparsed, false, true).unwrap().join("\n");

        // The roundtrip should be stable
        assert_eq!(result.trim(), result2.trim());
    }

    #[test]
    fn test_moo_left_to_right_precedence_expected() {
        // Test that MOO's left-to-right precedence is correctly handled
        // Simple case that exposes the difference
        let program = "a || b && c;";

        let tree = crate::parse::parse_program(program, CompileOptions::default()).unwrap();
        let result = unparse(&tree, false, true).unwrap().join("\n");

        // Should roundtrip correctly
        assert_eq!(program.trim(), result.trim());

        // Let's also check with parentheses to verify the parsing
        let program_with_parens = "(a || b) && c;";
        let tree_with_parens =
            crate::parse::parse_program(program_with_parens, CompileOptions::default()).unwrap();
        let result_with_parens = unparse(&tree_with_parens, false, true).unwrap().join("\n");

        // With MOO left-to-right precedence, "a || b && c" should parse the same as "(a || b) && c"
        // So both ASTs should be equivalent when unparsed
        assert_eq!(result.trim(), result_with_parens.trim());

        // Test fully parenthesized output shows the grouping
        let fully_paren = unparse(&tree, true, true).unwrap().join("\n");
        assert_eq!(fully_paren.trim(), "(a || b) && c;");
    }

    #[test]
    fn test_match_utils_complex_expression_roundtrip() {
        // The complex expression from match_utils.moo:97 that was causing roundtrip issues
        let program = r#"if ((vargs[2] == "any" || !prepstr && vargs[2] == "none" || index("/" + vargs[2] + "/", "/" + prepstr + "/")) && (vargs[1] == "any" || !dobjstr && vargs[1] == "none" || dobj == what && vargs[1] == "this") && (vargs[3] == "any" || !iobjstr && vargs[3] == "none" || iobj == what && vargs[3] == "this") && index(verb_info(where[1], vrb)[2], "x") && verb_code(where[1], vrb))
  return 1;
endif"#;

        let tree = crate::parse::parse_program(program, CompileOptions::default()).unwrap();
        let result = unparse(&tree, false, true).unwrap().join("\n");

        // Test that roundtrip is stable
        let reparsed = crate::parse::parse_program(&result, CompileOptions::default()).unwrap();
        let result2 = unparse(&reparsed, false, true).unwrap().join("\n");

        assert_eq!(result.trim(), result2.trim(), "Roundtrip should be stable");
    }

    #[test]
    fn test_in_operator_precedence() {
        // Test that IN operator has same precedence as comparison operators
        let program = "a == b in c;";

        let tree = crate::parse::parse_program(program, CompileOptions::default()).unwrap();
        let result = unparse(&tree, false, true).unwrap().join("\n");

        // Should roundtrip correctly
        assert_eq!(program.trim(), result.trim());

        // Test fully parenthesized output shows correct grouping
        let fully_paren = unparse(&tree, true, true).unwrap().join("\n");
        assert_eq!(fully_paren.trim(), "(a == b) in c;");
    }

    #[test]
    fn test_exponentiation_right_associativity() {
        // Test that ^ operator is right associative
        let program = "a ^ b ^ c;";

        let tree = crate::parse::parse_program(program, CompileOptions::default()).unwrap();
        let result = unparse(&tree, false, true).unwrap().join("\n");

        // Should roundtrip correctly
        assert_eq!(program.trim(), result.trim());

        // Test fully parenthesized output shows right associativity
        let fully_paren = unparse(&tree, true, true).unwrap().join("\n");
        assert_eq!(fully_paren.trim(), "a ^ (b ^ c);");
    }

    #[test]
    fn test_exponentiation_precedence_with_multiplication() {
        // Test that ^ has higher precedence than *
        let program = "a ^ b * c;";

        let tree = crate::parse::parse_program(program, CompileOptions::default()).unwrap();
        let result = unparse(&tree, false, true).unwrap().join("\n");

        // Should roundtrip correctly
        assert_eq!(program.trim(), result.trim());

        // Test fully parenthesized output shows precedence grouping
        let fully_paren = unparse(&tree, true, true).unwrap().join("\n");
        assert_eq!(fully_paren.trim(), "(a ^ b) * c;");
    }

    #[test]
    fn test_empty_map_equality_roundtrip() {
        compare_parse_roundtrip("return [] == [];");
    }

    #[test]
    fn test_empty_list_equality_roundtrip() {
        compare_parse_roundtrip("return {} == {};");
    }

    #[test]
    fn test_empty_map_complex_expression_roundtrip() {
        compare_parse_roundtrip(r#"return [] == [] && "yes" || "no";"#);
    }

    #[test]
    fn test_computed_verb_name_no_extra_parens() {
        // Regression test for #626: computed verb names should not get double-wrapped in parens.
        // $ansi:(this.some_function)() should NOT become $ansi:((this.some_function))()
        compare_parse_roundtrip("return $ansi:(this.some_function)();");
    }

    // Tests for multi-statement lambda unparsing
    // These test that fn () ... endfn syntax with multiple statements can be unparsed
    // Note: The unparser produces canonical format which may differ slightly from input
    // (e.g., adding 'let' for new variable declarations). We verify the output can be re-parsed.

    #[test]
    fn test_multi_statement_lambda_unparse() {
        // Multi-statement lambda using fn/endfn syntax
        let program = r#"f = fn ()
            x = 1;
            return x + 1;
        endfn;
        return f();"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        // Verify the output can be re-parsed (semantic equivalence)
        parse_and_unparse(&result).expect("Unparsed output should be re-parseable");
        // Check key elements are present
        assert!(result.contains("fn ()"), "Should contain fn ()");
        assert!(result.contains("endfn"), "Should contain endfn");
        assert!(result.contains("x = 1"), "Should contain x = 1");
        assert!(result.contains("return x + 1"), "Should contain return");
    }

    #[test]
    fn test_multi_statement_lambda_with_params_unparse() {
        // Multi-statement lambda with parameters
        let program = r#"f = fn (a, b)
            sum = a + b;
            return sum * 2;
        endfn;
        return f(1, 2);"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        parse_and_unparse(&result).expect("Unparsed output should be re-parseable");
        assert!(result.contains("fn (a, b)"), "Should contain fn (a, b)");
        assert!(result.contains("endfn"), "Should contain endfn");
    }

    #[test]
    fn test_multi_statement_lambda_with_conditionals_unparse() {
        // Multi-statement lambda with control flow
        let program = r#"f = fn (x)
            if (x > 0)
                return x;
            endif
            return -x;
        endfn;
        return f(-5);"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        parse_and_unparse(&result).expect("Unparsed output should be re-parseable");
        assert!(result.contains("fn (x)"), "Should contain fn (x)");
        assert!(result.contains("endfn"), "Should contain endfn");
        assert!(result.contains("if (x > 0)"), "Should contain conditional");
    }

    #[test]
    fn test_nested_multi_statement_lambdas_unparse() {
        // Nested multi-statement lambdas
        let program = r#"outer = fn (x)
            inner = fn (y)
                return y * 2;
            endfn;
            return inner(x) + 1;
        endfn;
        return outer(5);"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        parse_and_unparse(&result).expect("Unparsed output should be re-parseable");
        assert!(result.contains("fn (x)"), "Should contain outer fn");
        assert!(result.contains("fn (y)"), "Should contain inner fn");
        // Should have two endfn markers
        assert_eq!(
            result.matches("endfn").count(),
            2,
            "Should have two endfn markers"
        );
    }

    #[test]
    fn test_lambda_in_list_with_multi_statements_unparse() {
        // Lambda stored in a list (common pattern in OMeta parsers)
        let program = r#"handlers = {
            fn ()
                x = 1;
                return x + 1;
            endfn,
            fn ()
                y = 2;
                return y + 2;
            endfn
        };
        f = handlers[1];
        return f();"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        parse_and_unparse(&result).expect("Unparsed output should be re-parseable");
        // Should have two lambda functions
        assert_eq!(
            result.matches("fn ()").count(),
            2,
            "Should have two lambdas"
        );
        assert_eq!(
            result.matches("endfn").count(),
            2,
            "Should have two endfn markers"
        );
    }
}
