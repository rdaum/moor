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

#[test]
fn preserves_pass_expression_forms() {
    let shape = parse_shape(
        r#"
result = pass(@args);
result = pass();
result = pass(1,2,3,4);
pass = blop;
return pass;
"#,
    );
    let expected = r#"
(stmts
  (expr
    (assign
      (id result)
      (pass
        (args
          (splice
            (id args)
          )
        )
      )
    )
  )
  (expr
    (assign
      (id result)
      (pass
        (args
        )
      )
    )
  )
  (expr
    (assign
      (id result)
      (pass
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
          (arg
            (value 4)
          )
        )
      )
    )
  )
  (expr
    (assign
      (id pass)
      (id blop)
    )
  )
  (expr
    (return
      (id pass)
    )
  )
)"#;
    assert_eq!(shape, expected.trim());
}

#[test]
fn preserves_begin_scope_shape() {
    let shape = parse_shape(
        r#"
begin
  return 5;
end
"#,
    );
    let expected = r#"
(stmts
  (scope bindings=0
    (stmts
      (expr
        (return
          (value 5)
        )
      )
    )
  )
)"#;
    assert_eq!(shape, expected.trim());
}

#[test]
fn preserves_scoped_variable_binding_shape() {
    let shape = parse_shape(
        r#"
begin
  let x = 5;
  let y = 6;
  x = x + 6;
  let z = 7;
  let o;
  global a = 1;
end
return x;
"#,
    );
    let expected = r#"
(stmts
  (scope bindings=4
    (stmts
      (expr
        (decl kind=let id=x@1
          (value 5)
        )
      )
      (expr
        (decl kind=let id=y
          (value 6)
        )
      )
      (expr
        (assign
          (id x@1)
          (binary +
            (id x@1)
            (value 6)
          )
        )
      )
      (expr
        (decl kind=let id=z
          (value 7)
        )
      )
      (expr
        (decl kind=let id=o
        )
      )
      (expr
        (assign
          (id a)
          (value 1)
        )
      )
    )
  )
  (expr
    (return
      (id x@0)
    )
  )
)"#;
    assert_eq!(shape, expected.trim());
}

#[test]
fn preserves_error_literals_without_arguments() {
    let shape =
        parse_shape(r#"return {e_invarg, e_propnf, e_custom, e__ultra_long_custom, e_unknown};"#);
    let expected = r#"
(stmts
  (expr
    (return
      (list
        (args
          (arg
            (error E_INVARG
            )
          )
          (arg
            (error E_PROPNF
            )
          )
          (arg
            (error E_CUSTOM
            )
          )
          (arg
            (error E__ULTRA_LONG_CUSTOM
            )
          )
          (arg
            (error E_UNKNOWN
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
fn preserves_exponent_float_literal_shape() {
    let shape = parse_shape("return 1e-09;");
    let expected = r#"
(stmts
  (expr
    (return
      (value 1e-9)
    )
  )
)"#;
    assert_eq!(shape, expected.trim());
}

#[test]
fn preserves_direct_sysverb_call_shape() {
    let shape = parse_shape(r#"#0:test_verb(1,2,3,"test");"#);
    let expected = r#"
(stmts
  (expr
    (verb
      (value #0)
      (value "test_verb")
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
        (arg
          (value "test")
        )
      )
    )
  )
)"#;
    assert_eq!(shape, expected.trim());
}

#[test]
fn preserves_indexed_range_end_shape() {
    let shape = parse_shape("a = {1, 2, 3}; b = a[2..$];");
    let expected = r#"
(stmts
  (expr
    (assign
      (id a)
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
    )
  )
  (expr
    (assign
      (id b)
      (range
        (id a)
        (value 2)
        (length)
      )
    )
  )
)"#;
    assert_eq!(shape, expected.trim());
}

#[test]
fn preserves_underscore_prefixed_identifier_shape() {
    let shape = parse_shape("_house == home;");
    let expected = r#"
(stmts
  (expr
    (binary ==
      (id _house)
      (id home)
    )
  )
)"#;
    assert_eq!(shape, expected.trim());
}

#[test]
fn preserves_comparison_assignment_chain_shape() {
    let shape = parse_shape("(2 <= (len = length(player)));");
    let expected = r#"
(stmts
  (expr
    (binary <=
      (value 2)
      (assign
        (id len)
        (call
          (builtin length)
          (args
            (arg
              (id player)
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
fn preserves_try_catch_expression_shapes() {
    let shape = parse_shape("return {`x ! e_varnf => 666'};");
    let expected = r#"
(stmts
  (expr
    (return
      (list
        (args
          (arg
            (try-expr
              (id x)
              (codes
                (args
                  (arg
                    (error E_VARNF
                    )
                  )
                )
              )
              (except
                (value 666)
              )
            )
          )
        )
      )
    )
  )
)"#;
    assert_eq!(shape, expected.trim());

    let shape = parse_shape(r#"`raise(E_INVARG) ! ANY';"#);
    let expected = r#"
(stmts
  (expr
    (try-expr
      (call
        (builtin raise)
        (args
          (arg
            (error E_INVARG
            )
          )
        )
      )
      (codes any)
    )
  )
)"#;
    assert_eq!(shape, expected.trim());

    let shape = parse_shape(r#"`$ftp_client:finish_get(this.connection) ! ANY';"#);
    let expected = r#"
(stmts
  (expr
    (try-expr
      (verb
        (prop
          (value #0)
          (value "ftp_client")
        )
        (value "finish_get")
        (args
          (arg
            (prop
              (id this)
              (value "connection")
            )
          )
        )
      )
      (codes any)
    )
  )
)"#;
    assert_eq!(shape, expected.trim());
}

#[test]
fn preserves_parenthesized_grouping_shape() {
    let shape = parse_shape("1 && (2 || 3);");
    let expected = r#"
(stmts
  (expr
    (and
      (value 1)
      (or
        (value 2)
        (value 3)
      )
    )
  )
)"#;
    assert_eq!(shape, expected.trim());
}

#[test]
fn preserves_simple_map_shape() {
    let shape = parse_shape("[ 1 -> 2, 3 -> 4 ];");
    let expected = r#"
(stmts
  (expr
    (map
      (entry
        (value 1)
        (value 2)
      )
      (entry
        (value 3)
        (value 4)
      )
    )
  )
)"#;
    assert_eq!(shape, expected.trim());
}

#[test]
fn preserves_additional_flyweight_forms() {
    let shape = parse_shape("<#1>;");
    let expected = r#"
(stmts
  (expr
    (flyweight
      (value #1)
    )
  )
)"#;
    assert_eq!(shape, expected.trim());

    let shape = parse_shape("<#1, a_list>;");
    let expected = r#"
(stmts
  (expr
    (flyweight
      (value #1)
      (contents
        (id a_list)
      )
    )
  )
)"#;
    assert_eq!(shape, expected.trim());
}

#[test]
fn preserves_additional_lambda_forms() {
    let shape = parse_shape("let f = {x} => 5;");
    let expected = r#"
(stmts
  (expr
    (decl kind=let id=f
      (lambda self=_
        (scatter-items
          (item kind=required id=x
          )
        )
        (expr
          (return
            (value 5)
          )
        )
      )
    )
  )
)"#;
    assert_eq!(shape, expected.trim());
}

#[test]
fn preserves_bitwise_operator_shapes() {
    let shape = parse_shape("return 3 &. 1;");
    let expected = r#"
(stmts
  (expr
    (return
      (binary &.
        (value 3)
        (value 1)
      )
    )
  )
)"#;
    assert_eq!(shape, expected.trim());

    let shape = parse_shape("return 16 >> 2;");
    let expected = r#"
(stmts
  (expr
    (return
      (binary >>
        (value 16)
        (value 2)
      )
    )
  )
)"#;
    assert_eq!(shape, expected.trim());

    let shape = parse_shape("return ~5;");
    let expected = r#"
(stmts
  (expr
    (return
      (unary ~
        (value 5)
      )
    )
  )
)"#;
    assert_eq!(shape, expected.trim());
}

#[test]
fn preserves_not_precedence_on_verb_call_shape() {
    let shape = parse_shape("return !(#2:move(5));");
    let expected = r#"
(stmts
  (expr
    (return
      (unary !
        (verb
          (value #2)
          (value "move")
          (args
            (arg
              (value 5)
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
fn preserves_keyword_ambiguous_assignment_after_endfor() {
    let shape = parse_shape(
        r#"
for a in ({1,2,3})
endfor
info = 5;
forgotten = 3;
"#,
    );
    let expected = r#"
(stmts
  (for-list value=a key=_ env=0
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
    (assign
      (id info)
      (value 5)
    )
  )
  (expr
    (assign
      (id forgotten)
      (value 3)
    )
  )
)"#;
    assert_eq!(shape, expected.trim());
}

#[test]
fn parses_scope_regression_program() {
    parse_program_frontend(
        r#"
{dude, chamber, chamber_index} = args;
if (chamber.mode == "package")
  reagent = $player_drug_reagent:create(chamber.product_name, chamber.reagents, chamber.quality);
  this.reagents[reagent] = `this.reagents[reagent] ! ANY => 0' + 5;
endif
for quantity, reagent in (received_reagents)
  this.reagents[reagent] = `this.reagents[reagent] ! ANY => 0' + quantity;
endfor
"#,
        CompileOptions::default(),
    )
    .unwrap();
}

#[test]
fn parses_begin_prefix_regression_program() {
    parse_program_frontend(
        r#"if (iobjstr == "")
  player:tell("Nothing to do.");
  return;
elseif (iobjstr == "next")
  if (this.last_read == length(this.notice_desc))
    player:tell("End of notices: Start with the first one?");
    if ($command_utils:yes_or_no())
      beginning = 1;
    else
      player:tell("No more jumps to make.");
      return;
    endif
  else
    beginning = this.last_read + 1;
    what = this.last_scan;
  endif
else
  beginning = 1;
  what = iobjstr;
  this.last_scan = what;
endif"#,
        CompileOptions::default(),
    )
    .unwrap();
}

#[test]
fn parses_auditdb_regression_program() {
    parse_program_frontend(
        r#""Usage:  @auditDB [player] [from <start>] [to <end>] [for <matching string>]";
set_task_perms(player);
dobj = player:my_match_player(dobjstr);
if (!dobjstr)
dobj = player;
elseif ($command_utils:player_match_failed(dobj, dobjstr) && (!(valid(dobj = $string_utils:literal_object(dobjstr)) && $command_utils:yes_or_no("Continue?"))))
return;
endif
dobjwords = $string_utils:words(dobjstr);
if (args[1..length(dobjwords)] == dobjwords)
args = args[length(dobjwords) + 1..length(args)];
endif
if (!(parse_result = $code_utils:_parse_audit_args(@args)))
player:notify(tostr("Usage:  ", verb, " [player] [from <start>] [to <end>] [for <match>]"));
return;
endif
start = parse_result[1];
end = parse_result[2];
match = parse_result[3];
player:notify(tostr("Objects owned by ", valid(dobj) ? dobj:name() | dobj, ((" (from #" + tostr(start)) + " to #") + tostr(end), match ? " matching " + match | "", ")", ":"));
player:notify("");
count = 0;
"Only print every third suspension";
do_print = 0;
for i in [start..end]
o = toobj(i);
if ($command_utils:running_out_of_time())
(do_print = (do_print + 1) % 3) || player:notify(tostr("... ", o));
suspend(5);
endif
if (valid(o) && (o.owner == dobj))
found = 0;
names = {o:name(), @o.aliases};
while (names && (!found))
if (index(names[1], match) == 1)
found = 1;
endif
names = listdelete(names, 1);
endwhile
if (found)
player:notify(tostr(o:name(), " (", o, ")"));
count = count + 1;
do_print = 0;
endif
endif
endfor
if (count)
player:notify("");
endif
player:notify(tostr("Total: ", count, " object", (count == 1) ? "." | "s."));
return 0 && "Automatically Added Return";"#,
        CompileOptions::default(),
    )
    .unwrap();
}
