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

use moor_common::model::CompileError;

use crate::{CompileOptions, ast::render_parse_shape, parse_program_frontend, unparse};

fn parse_shape(source: &str) -> String {
    let parse = parse_program_frontend(source, CompileOptions::default()).unwrap();
    render_parse_shape(&parse)
}

#[test]
fn preserves_assignment_and_binary_precedence() {
    let shape = parse_shape("a = 1 + 2;");
    let expected = r#"
(stmts
  (expr
    (assign
      (id a)
      (binary +
        (value 1)
        (value 2)
      )
    )
  )
)"#;
    assert_eq!(shape, expected.trim());
}

#[test]
fn preserves_scatter_rhs_index_precedence() {
    let shape = parse_shape("{connection} = args[1];");
    let expected = r#"
(stmts
  (expr
    (scatter
      (scatter-items
        (item kind=required id=connection
        )
      )
      (index
        (id args)
        (value 1)
      )
    )
  )
)"#;
    assert_eq!(shape, expected.trim());
}

#[test]
fn preserves_unary_vs_sysverb_call_precedence() {
    let shape = parse_shape(
        r#"
if (!$network:is_connected(this))
  return;
endif
"#,
    );
    let expected = r#"
(stmts
  (if
    (arm env=0
      (unary !
        (verb
          (prop
            (value #0)
            (value "network")
          )
          (value "is_connected")
          (args
            (arg
              (id this)
            )
          )
        )
      )
      (stmts
        (expr
          (return
          )
        )
      )
    )
  )
)"#;
    assert_eq!(shape, expected.trim());
}

#[test]
fn preserves_keyword_disambiguation_after_endfor() {
    let shape = parse_shape(
        r#"
for line in ({1,2,3})
endfor(52);
"#,
    );
    let expected = r#"
(stmts
  (for-list value=line key=_ env=0
    (list
      (args
        (arg
          (value 1)
        )
        (arg
          (value 2)
        )
        (arg
          (value 3)
        )
      )
    )
    (stmts
    )
  )
  (expr
    (value 52)
  )
)"#;
    assert_eq!(shape, expected.trim());
}

#[test]
fn allows_begin_and_end_as_identifiers() {
    let shape = parse_shape("begin(); end();");
    let expected = r#"
(stmts
  (expr
    (call
      (target
        (id begin)
      )
      (args
      )
    )
  )
  (expr
    (call
      (target
        (id end)
      )
      (args
      )
    )
  )
)"#;
    assert_eq!(shape, expected.trim());
}

#[test]
fn respects_legacy_type_constant_option() {
    let source = "return typeof(x) == INT;";
    let parse = parse_program_frontend(
        source,
        CompileOptions {
            legacy_type_constants: true,
            ..Default::default()
        },
    )
    .unwrap();
    let unparsed = unparse(&parse, false, true).unwrap().join("\n");
    assert!(unparsed.contains("TYPE_INT"));

    let parse = parse_program_frontend(source, CompileOptions::default()).unwrap();
    let unparsed = unparse(&parse, false, true).unwrap().join("\n");
    assert!(!unparsed.contains("TYPE_INT"));
}

#[test]
fn reports_unknown_loop_labels() {
    let err = parse_program_frontend("while (1) break nope; endwhile", CompileOptions::default())
        .unwrap_err();
    match err {
        CompileError::UnknownLoopLabel(_, label) => assert_eq!(label, "nope"),
        other => panic!("expected UnknownLoopLabel, got {other:?}"),
    }
}

#[test]
fn rejects_lexical_scopes_when_disabled() {
    let err = parse_program_frontend(
        "begin\n  let a = 1;\nend",
        CompileOptions {
            lexical_scopes: false,
            ..Default::default()
        },
    )
    .unwrap_err();
    match err {
        CompileError::DisabledFeature(_, feature) => assert_eq!(feature, "lexical_scopes"),
        other => panic!("expected ParseError, got {other:?}"),
    }
}

#[test]
fn rejects_assignment_to_const_scatter_bindings() {
    let err = parse_program_frontend(
        r#"
begin
    const {a, b} = {1, 2};
    a = 3;
end
"#,
        CompileOptions::default(),
    )
    .unwrap_err();
    match err {
        CompileError::AssignToConst(_, _) => {}
        other => panic!("expected AssignToConst, got {other:?}"),
    }
}

#[test]
fn rejects_duplicate_const_scatter_bindings() {
    let err = parse_program_frontend(
        r#"
begin
    const {a, b} = {1, 2};
    const {a, b} = {2, 3};
end
"#,
        CompileOptions::default(),
    )
    .unwrap_err();
    match err {
        CompileError::DuplicateVariable(_, _) => {}
        other => panic!("expected DuplicateVariable, got {other:?}"),
    }
}

#[test]
fn preserves_return_as_expression_shape() {
    let shape = parse_shape("true && return 5;");
    let expected = r#"
(stmts
  (expr
    (and
      (value true)
      (return
        (value 5)
      )
    )
  )
)"#;
    assert_eq!(shape, expected.trim());
}

#[test]
fn preserves_error_literals_with_arguments() {
    let shape = parse_shape(r#"return {e_invarg("test"), e_propnf(5), e_custom("booo")};"#);
    let expected = r#"
(stmts
  (expr
    (return
      (list
        (args
          (arg
            (error E_INVARG
              (value "test")
            )
          )
          (arg
            (error E_PROPNF
              (value 5)
            )
          )
          (arg
            (error E_CUSTOM
              (value "booo")
            )
          )
        )
      )
    )
  )
)"#;
    assert_eq!(shape, expected.trim());
}

#[test]
fn preserves_return_followed_by_keyword_prefixed_identifier() {
    let shape = parse_shape("return returnval;");
    let expected = r#"
(stmts
  (expr
    (return
      (id returnval)
    )
  )
)"#;
    assert_eq!(shape, expected.trim());
}

#[test]
fn preserves_binary_literal_shape() {
    let shape = parse_shape(r#"return b"SGVsbG8gV29ybGQ=";"#);
    let expected = r#"
(stmts
  (expr
    (return
      (value b"SGVsbG8gV29ybGQ=")
    )
  )
)"#;
    assert_eq!(shape, expected.trim());
}

#[test]
fn preserves_flyweight_slots_and_contents() {
    let shape = parse_shape(r#"<#1, .a = 1, {2}>;"#);
    let expected = r#"
(stmts
  (expr
    (flyweight
      (value #1)
      (slot a
        (value 1)
      )
      (contents
        (list
          (args
            (arg
              (value 2)
            )
          )
        )
      )
    )
  )
)"#;
    assert_eq!(shape, expected.trim());
}

#[test]
fn preserves_arrow_lambda_parameter_kinds() {
    let shape = parse_shape("let f = {a, ?b, @rest} => a + b;");
    let expected = r#"
(stmts
  (expr
    (decl kind=let id=f
      (lambda self=_
        (scatter-items
          (item kind=required id=a
          )
          (item kind=optional id=b
          )
          (item kind=rest id=rest
          )
        )
        (expr
          (return
            (binary +
              (id a)
              (id b)
            )
          )
        )
      )
    )
  )
)"#;
    assert_eq!(shape, expected.trim());
}

#[test]
fn preserves_statement_lambda_body_shape() {
    let shape = parse_shape("let f = fn(x) return x + 1; endfn;");
    let expected = r#"
(stmts
  (expr
    (decl kind=let id=f
      (lambda self=_
        (scatter-items
          (item kind=required id=x
          )
        )
        (scope bindings=0
          (stmts
            (expr
              (return
                (binary +
                  (id x)
                  (value 1)
                )
              )
            )
          )
        )
      )
    )
  )
)"#;
    assert_eq!(shape, expected.trim());
}

#[test]
fn allows_multiple_lambdas_with_same_parameter_name() {
    parse_program_frontend(
        r#"let funcs = {{s} => s + 1, {s} => s + 2};"#,
        CompileOptions::default(),
    )
    .unwrap();
}

#[test]
fn keeps_keyword_prefixed_identifiers_as_identifiers() {
    let shape = parse_shape("lets = 5; return global_salt;");
    let expected = r#"
(stmts
  (expr
    (assign
      (id lets)
      (value 5)
    )
  )
  (expr
    (return
      (id global_salt)
    )
  )
)"#;
    assert_eq!(shape, expected.trim());
}

#[test]
fn keeps_boolean_keyword_prefixed_identifiers_as_identifiers() {
    let parse = parse_program_frontend(
        r#"
{code, escape_char, ?truecolor_match = 0, ?xterm_256_match = 0} = args;
if (truecolor_match)
  ret = substitute(tostr(ret, ";2;%4;%5;%6m"), truecolor_match);
  return ret;
endif
"#,
        CompileOptions::default(),
    )
    .unwrap();
    assert!(parse.variables.find_name("truecolor_match").is_some());
    assert!(parse.variables.find_name("xterm_256_match").is_some());

    let parse = parse_program_frontend(
        "truecolor_match = 1; false_positive = 2;",
        CompileOptions::default(),
    )
    .unwrap();
    assert!(parse.variables.find_name("truecolor_match").is_some());
    assert!(parse.variables.find_name("false_positive").is_some());

    parse_program_frontend(
        r#"{path, ?require_extension = $false} = args;"#,
        CompileOptions::default(),
    )
    .unwrap();
    parse_program_frontend("x = $false;", CompileOptions::default()).unwrap();
}

#[test]
fn rejects_assignment_to_plain_const_binding() {
    let err = parse_program_frontend(
        r#"
const x = 5;
x = 6;
"#,
        CompileOptions::default(),
    )
    .unwrap_err();
    match err {
        CompileError::AssignToConst(_, _) => {}
        other => panic!("expected AssignToConst, got {other:?}"),
    }
}

#[test]
fn preserves_local_scatter_shadowing_shape() {
    let shape = parse_shape(
        r#"
begin
    a = 3;
    begin
        let {a, b} = {1, 2};
    end
end
"#,
    );
    let expected = r#"
(stmts
  (scope bindings=0
    (stmts
      (expr
        (assign
          (id a@0)
          (value 3)
        )
      )
      (scope bindings=2
        (stmts
          (expr
            (scatter
              (scatter-items
                (item kind=required id=a@2
                )
                (item kind=required id=b
                )
              )
              (list
                (args
                  (arg
                    (value 1)
                  )
                  (arg
                    (value 2)
                  )
                )
              )
            )
          )
        )
      )
    )
  )
)"#;
    assert_eq!(shape, expected.trim());
}

#[test]
fn preserves_bitwise_precedence_shape() {
    let shape = parse_shape("return 1 || 5 &. 3;");
    let expected = r#"
(stmts
  (expr
    (return
      (or
        (value 1)
        (binary &.
          (value 5)
          (value 3)
        )
      )
    )
  )
)"#;
    assert_eq!(shape, expected.trim());
}
