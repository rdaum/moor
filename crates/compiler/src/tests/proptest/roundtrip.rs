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

//! Roundtrip tests: generate AST -> unparse -> parse -> unparse -> compare

use std::fs;
use std::path::PathBuf;

use crate::ast::{Arg, BinaryOp, CallTarget, CatchCodes, Expr, UnaryOp};
use crate::parse::parse_program;
use crate::unparse::unparse;
use crate::CompileOptions;
use moor_var::program::names::VarName;
use moor_var::Variant;
use moor_var::{ErrorCode, Var};
use proptest::prelude::*;

use super::generators::{arb_expr_layer1, arb_expr_layer2, arb_expr_layer2_complete, arb_expr_layer2b};

/// Format an expression to MOO source code.
/// This is a simplified unparser just for testing - we'll use it to generate
/// initial source, then verify parse->unparse->parse roundtrips.
fn format_expr_to_source(expr: &Expr) -> String {
    match expr {
        Expr::Value(v) => format_var(v),
        Expr::Error(code, None) => format_error_code(code),
        Expr::Error(code, Some(msg)) => {
            format!("{}({})", format_error_code(code), format_expr_to_source(msg))
        }
        Expr::Id(var) => match &var.nr {
            VarName::Named(sym) => sym.to_string(),
            VarName::Register(n) => format!("_r{}", n),
        },
        Expr::Binary(op, left, right) => {
            let op_str = match op {
                BinaryOp::Add => "+",
                BinaryOp::Sub => "-",
                BinaryOp::Mul => "*",
                BinaryOp::Div => "/",
                BinaryOp::Mod => "%",
                BinaryOp::Exp => "^",
                BinaryOp::Eq => "==",
                BinaryOp::NEq => "!=",
                BinaryOp::Lt => "<",
                BinaryOp::LtE => "<=",
                BinaryOp::Gt => ">",
                BinaryOp::GtE => ">=",
                BinaryOp::In => "in",
                BinaryOp::BitAnd => "&.",
                BinaryOp::BitOr => "|.",
                BinaryOp::BitXor => "^.",
                BinaryOp::BitShl => "<<",
                BinaryOp::BitShr => ">>",
                BinaryOp::BitLShr => ">>>",
            };
            format!(
                "({} {} {})",
                format_expr_to_source(left),
                op_str,
                format_expr_to_source(right)
            )
        }
        Expr::Unary(op, inner) => {
            let op_str = match op {
                UnaryOp::Neg => "-",
                UnaryOp::Not => "!",
                UnaryOp::BitNot => "~",
            };
            format!("({}{})", op_str, format_expr_to_source(inner))
        }
        Expr::And(left, right) => {
            format!(
                "({} && {})",
                format_expr_to_source(left),
                format_expr_to_source(right)
            )
        }
        Expr::Or(left, right) => {
            format!(
                "({} || {})",
                format_expr_to_source(left),
                format_expr_to_source(right)
            )
        }
        Expr::List(args) => {
            let elements: Vec<String> = args
                .iter()
                .map(|arg| match arg {
                    Arg::Normal(e) => format_expr_to_source(e),
                    Arg::Splice(e) => format!("@{}", format_expr_to_source(e)),
                })
                .collect();
            format!("{{{}}}", elements.join(", "))
        }
        Expr::Map(entries) => {
            let elements: Vec<String> = entries
                .iter()
                .map(|(k, v)| {
                    format!("{} -> {}", format_expr_to_source(k), format_expr_to_source(v))
                })
                .collect();
            format!("[{}]", elements.join(", "))
        }
        Expr::Index(base, index) => {
            format!("({}[{}])", format_expr_to_source(base), format_expr_to_source(index))
        }
        Expr::Range { base, from, to } => {
            format!(
                "({}[{}..{}])",
                format_expr_to_source(base),
                format_expr_to_source(from),
                format_expr_to_source(to)
            )
        }
        Expr::Cond {
            condition,
            consequence,
            alternative,
        } => {
            format!(
                "({} ? {} | {})",
                format_expr_to_source(condition),
                format_expr_to_source(consequence),
                format_expr_to_source(alternative)
            )
        }
        Expr::Prop { location, property } => {
            // Check if property is a string literal for static property access
            if let Expr::Value(v) = property.as_ref()
                && let Variant::Str(s) = v.variant()
            {
                return format!("({}).{}", format_expr_to_source(location), s.as_str());
            }
            // Dynamic property access
            format!(
                "({}).({})",
                format_expr_to_source(location),
                format_expr_to_source(property)
            )
        }
        Expr::Verb {
            location,
            verb,
            args,
        } => {
            let args_str: Vec<String> = args
                .iter()
                .map(|arg| match arg {
                    Arg::Normal(e) => format_expr_to_source(e),
                    Arg::Splice(e) => format!("@{}", format_expr_to_source(e)),
                })
                .collect();
            // Check if verb is a string literal for static verb call
            // Per LambdaMOO spec: verb names are identifiers or string expressions
            if let Expr::Value(v) = verb.as_ref()
                && let Variant::Str(s) = v.variant()
            {
                return format!(
                    "({}):{}({})",
                    format_expr_to_source(location),
                    s.as_str(),
                    args_str.join(", ")
                );
            }
            // Dynamic verb call - needs parentheses around expression
            format!(
                "({}):({})({})",
                format_expr_to_source(location),
                format_expr_to_source(verb),
                args_str.join(", ")
            )
        }
        Expr::Call { function, args } => {
            let args_str: Vec<String> = args
                .iter()
                .map(|arg| match arg {
                    Arg::Normal(e) => format_expr_to_source(e),
                    Arg::Splice(e) => format!("@{}", format_expr_to_source(e)),
                })
                .collect();
            match function {
                CallTarget::Builtin(sym) => {
                    format!("{}({})", sym, args_str.join(", "))
                }
                CallTarget::Expr(expr) => {
                    format!("call({}, {})", format_expr_to_source(expr), args_str.join(", "))
                }
            }
        }
        Expr::TryCatch {
            trye,
            codes,
            except,
        } => {
            let codes_str = match codes {
                CatchCodes::Any => "ANY".to_string(),
                CatchCodes::Codes(code_list) => {
                    let cs: Vec<String> = code_list
                        .iter()
                        .map(|arg| match arg {
                            Arg::Normal(e) => format_expr_to_source(e),
                            Arg::Splice(e) => format!("@{}", format_expr_to_source(e)),
                        })
                        .collect();
                    cs.join(", ")
                }
            };
            match except {
                Some(fallback) => {
                    format!(
                        "(`{} ! {} => {}')",
                        format_expr_to_source(trye),
                        codes_str,
                        format_expr_to_source(fallback)
                    )
                }
                None => {
                    format!("`{} ! {}'", format_expr_to_source(trye), codes_str)
                }
            }
        }
        Expr::Length => "$".to_string(),
        Expr::TypeConstant(var_type) => {
            use moor_var::VarType;
            match *var_type {
                VarType::TYPE_INT => "INT".to_string(),
                VarType::TYPE_FLOAT => "FLOAT".to_string(),
                VarType::TYPE_STR => "STR".to_string(),
                VarType::TYPE_OBJ => "OBJ".to_string(),
                VarType::TYPE_LIST => "LIST".to_string(),
                VarType::TYPE_MAP => "MAP".to_string(),
                VarType::TYPE_ERR => "ERR".to_string(),
                VarType::TYPE_BOOL => "BOOL".to_string(),
                VarType::TYPE_FLYWEIGHT => "FLYWEIGHT".to_string(),
                VarType::TYPE_LABEL => "LABEL".to_string(),
                _ => panic!("Unknown VarType: {:?}", var_type),
            }
        }
        _ => panic!("Unsupported expression type: {:?}", expr),
    }
}

fn format_var(v: &Var) -> String {
    match v.variant() {
        Variant::Int(n) => format!("{}", n),
        Variant::Float(f) => {
            // Ensure we always have a decimal point for floats
            if f.fract() == 0.0 {
                format!("{:.1}", f)
            } else {
                format!("{}", f)
            }
        }
        Variant::Str(s) => {
            // Escape the string properly
            let escaped = s
                .as_str()
                .chars()
                .map(|c| match c {
                    '"' => "\\\"".to_string(),
                    '\\' => "\\\\".to_string(),
                    '\n' => "\\n".to_string(),
                    '\t' => "\\t".to_string(),
                    '\r' => "\\r".to_string(),
                    c if c.is_control() => format!("\\x{:02x}", c as u32),
                    c => c.to_string(),
                })
                .collect::<String>();
            format!("\"{}\"", escaped)
        }
        Variant::Obj(obj) => {
            if obj.is_sysobj() {
                format!("#{}", obj.id().0)
            } else {
                // UUID-based object - use a simple representation
                "#0".to_string()
            }
        }
        Variant::Bool(b) => {
            if b {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        _ => panic!("Unsupported var type: {:?}", v.variant()),
    }
}

fn format_error_code(code: &ErrorCode) -> String {
    use ErrorCode::*;
    match code {
        E_NONE => "E_NONE",
        E_TYPE => "E_TYPE",
        E_DIV => "E_DIV",
        E_PERM => "E_PERM",
        E_PROPNF => "E_PROPNF",
        E_VERBNF => "E_VERBNF",
        E_VARNF => "E_VARNF",
        E_INVIND => "E_INVIND",
        E_RECMOVE => "E_RECMOVE",
        E_MAXREC => "E_MAXREC",
        E_RANGE => "E_RANGE",
        E_ARGS => "E_ARGS",
        E_NACC => "E_NACC",
        E_INVARG => "E_INVARG",
        E_QUOTA => "E_QUOTA",
        E_FLOAT => "E_FLOAT",
        E_FILE => "E_FILE",
        E_EXEC => "E_EXEC",
        E_INTRPT => "E_INTRPT",
        ErrCustom(sym) => return format!("E_CUSTOM({})", sym),
    }
    .to_string()
}

/// Save a failure snapshot to disk for debugging.
fn save_failure(source: &str, error: &str, seed: &str) {
    let timestamp = chrono::Utc::now().format("%Y-%m-%d-%H%M%S");
    let hash = &format!("{:x}", md5::compute(source))[..8];
    let filename = format!("{}-{}.txt", timestamp, hash);

    let failures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("proptest-failures");
    fs::create_dir_all(&failures_dir).ok();

    let path = failures_dir.join(&filename);
    let content = format!(
        "=== PROPTEST FAILURE ===\n\
         Seed: {}\n\
         \n\
         === SOURCE ===\n\
         {}\n\
         \n\
         === ERROR ===\n\
         {}\n",
        seed, source, error
    );

    if let Err(e) = fs::write(&path, &content) {
        eprintln!("Failed to write failure snapshot to {:?}: {}", path, e);
    } else {
        eprintln!("Failure snapshot saved to {:?}", path);
    }
}

/// Run a roundtrip test on the given expression.
fn run_roundtrip(expr: &Expr) -> Result<(), TestCaseError> {
    // 1. Format the expression to source code (as a statement)
    let source = format!("return {};", format_expr_to_source(expr));

    // 2. Parse the source
    let parse_result = parse_program(&source, CompileOptions::default());
    let parsed = match parse_result {
        Ok(p) => p,
        Err(e) => {
            let error = format!("Parse error: {:?}", e);
            save_failure(&source, &error, "unknown");
            return Err(TestCaseError::fail(format!(
                "Failed to parse generated source:\n{}\nError: {:?}",
                source, e
            )));
        }
    };

    // 3. Unparse back to source
    let unparse_result = unparse(&parsed, false, true);
    let unparsed = match unparse_result {
        Ok(lines) => lines.join("\n"),
        Err(e) => {
            let error = format!("Unparse error: {:?}", e);
            save_failure(&source, &error, "unknown");
            return Err(TestCaseError::fail(format!(
                "Failed to unparse:\n{}\nError: {:?}",
                source, e
            )));
        }
    };

    // 4. Parse the unparsed source again
    let reparse_result = parse_program(&unparsed, CompileOptions::default());
    let reparsed = match reparse_result {
        Ok(p) => p,
        Err(e) => {
            let error = format!(
                "Reparse error: {:?}\nOriginal: {}\nUnparsed: {}",
                e, source, unparsed
            );
            save_failure(&source, &error, "unknown");
            return Err(TestCaseError::fail(format!(
                "Failed to reparse unparsed source:\nOriginal: {}\nUnparsed: {}\nError: {:?}",
                source, unparsed, e
            )));
        }
    };

    // 5. Unparse the reparsed version
    let reunparse_result = unparse(&reparsed, false, true);
    let reunparsed = match reunparse_result {
        Ok(lines) => lines.join("\n"),
        Err(e) => {
            let error = format!("Reunparse error: {:?}", e);
            save_failure(&source, &error, "unknown");
            return Err(TestCaseError::fail(format!(
                "Failed to reunparse:\n{}\nError: {:?}",
                source, e
            )));
        }
    };

    // 6. Assert the two unparsed versions are identical (stable roundtrip)
    if unparsed.trim() != reunparsed.trim() {
        let error = format!(
            "Roundtrip not stable!\nFirst unparse: {}\nSecond unparse: {}",
            unparsed, reunparsed
        );
        save_failure(&source, &error, "unknown");
        return Err(TestCaseError::fail(error));
    }

    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn roundtrip_layer1_expr(expr in arb_expr_layer1(4)) {
        run_roundtrip(&expr)?;
    }

    #[test]
    fn roundtrip_layer2_expr(expr in arb_expr_layer2(3)) {
        run_roundtrip(&expr)?;
    }

    #[test]
    fn roundtrip_layer2b_expr(expr in arb_expr_layer2b(3)) {
        run_roundtrip(&expr)?;
    }

    #[test]
    fn roundtrip_layer2_complete_expr(expr in arb_expr_layer2_complete(2)) {
        run_roundtrip(&expr)?;
    }
}

#[cfg(test)]
mod manual_tests {
    use super::*;

    #[test]
    fn test_keyword_prefix_with_letter() {
        // "ina" starts with "in" keyword but followed by letter - should work
        let source = "return ina;";
        let result = parse_program(source, CompileOptions::default());
        assert!(result.is_ok(), "Failed to parse 'ina': {:?}", result);
    }

    #[test]
    fn test_keyword_prefix_with_digit() {
        // "in0" starts with "in" keyword but followed by digit - should work
        let source = "return in0;";
        let result = parse_program(source, CompileOptions::default());
        assert!(result.is_ok(), "Failed to parse 'in0': {:?}", result);
    }

    #[test]
    fn test_keyword_prefix_for_digit() {
        // "for1" starts with "for" keyword but followed by digit - should work
        let source = "return for1;";
        let result = parse_program(source, CompileOptions::default());
        assert!(result.is_ok(), "Failed to parse 'for1': {:?}", result);
    }

    #[test]
    fn test_simple_integer() {
        let source = "return 42;";
        let parsed = parse_program(source, CompileOptions::default()).unwrap();
        let unparsed = unparse(&parsed, false, true).unwrap().join("\n");
        assert_eq!(unparsed.trim(), "return 42;");
    }

    #[test]
    fn test_binary_op() {
        let source = "return (1 + 2);";
        let parsed = parse_program(source, CompileOptions::default()).unwrap();
        let unparsed = unparse(&parsed, false, true).unwrap().join("\n");
        // The unparser may simplify parentheses
        let reparsed = parse_program(&unparsed, CompileOptions::default()).unwrap();
        let reunparsed = unparse(&reparsed, false, true).unwrap().join("\n");
        assert_eq!(unparsed.trim(), reunparsed.trim());
    }

    #[test]
    fn test_format_string_with_escapes() {
        let v = Var::mk_str("hello\nworld");
        let formatted = format_var(&v);
        assert_eq!(formatted, "\"hello\\nworld\"");
    }

    #[test]
    fn test_list_roundtrip() {
        let source = "return {1, 2, 3};";
        let parsed = parse_program(source, CompileOptions::default()).unwrap();
        let unparsed = unparse(&parsed, false, true).unwrap().join("\n");
        let reparsed = parse_program(&unparsed, CompileOptions::default()).unwrap();
        let reunparsed = unparse(&reparsed, false, true).unwrap().join("\n");
        assert_eq!(unparsed.trim(), reunparsed.trim());
    }

    #[test]
    fn test_map_roundtrip() {
        let source = "return [1 -> \"a\", 2 -> \"b\"];";
        let parsed = parse_program(source, CompileOptions::default()).unwrap();
        let unparsed = unparse(&parsed, false, true).unwrap().join("\n");
        let reparsed = parse_program(&unparsed, CompileOptions::default()).unwrap();
        let reunparsed = unparse(&reparsed, false, true).unwrap().join("\n");
        assert_eq!(unparsed.trim(), reunparsed.trim());
    }

    #[test]
    fn test_identifier_roundtrip() {
        let source = "return foo;";
        let parsed = parse_program(source, CompileOptions::default()).unwrap();
        let unparsed = unparse(&parsed, false, true).unwrap().join("\n");
        let reparsed = parse_program(&unparsed, CompileOptions::default()).unwrap();
        let reunparsed = unparse(&reparsed, false, true).unwrap().join("\n");
        assert_eq!(unparsed.trim(), reunparsed.trim());
    }
}
