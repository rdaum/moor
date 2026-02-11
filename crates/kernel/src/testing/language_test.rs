// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

//! Language behavior tests for MOO code execution.
//! These tests compile MOO source code and verify runtime semantics.

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use moor_common::model::CompileError;
    use moor_common::model::{ObjectKind, VerbArgsSpec, VerbFlag, WorldState, WorldStateSource};
    use moor_common::tasks::NoopClientSession;
    use moor_common::util::BitEnum;
    use moor_compiler::{CompileOptions, Program, compile};
    use moor_db::{DatabaseConfig, TxDB};
    use moor_var::program::ProgramType;
    use moor_var::{
        E_ARGS, E_DIV, E_PERM, E_QUOTA, E_RANGE, E_TYPE, Error, IndexMode, List, NOTHING, Obj,
        SYSTEM_OBJECT, Symbol, Var, v_bool_int, v_empty_list, v_err, v_float, v_flyweight, v_int,
        v_list, v_map, v_obj, v_objid, v_str, v_sym,
    };
    use test_case::test_case;

    use crate::testing::vm_test_utils::{call_eval_builtin_with_env, call_verb};
    use crate::vm::builtins::BuiltinRegistry;

    /// Create an in-memory db with a single object (#0) containing verbs.
    fn test_db_with_verbs(verbs: &[(&str, &Program)]) -> TxDB {
        let (state, _) = TxDB::open(None, DatabaseConfig::default());
        let mut tx = state.new_world_state().unwrap();
        let sysobj = tx
            .create_object(
                &SYSTEM_OBJECT,
                &NOTHING,
                &SYSTEM_OBJECT,
                BitEnum::all(),
                ObjectKind::NextObjid,
            )
            .unwrap();
        tx.update_property(
            &SYSTEM_OBJECT,
            &sysobj,
            Symbol::mk("name"),
            &v_str("system"),
        )
        .unwrap();
        tx.update_property(&SYSTEM_OBJECT, &sysobj, Symbol::mk("programmer"), &v_int(1))
            .unwrap();
        tx.update_property(&SYSTEM_OBJECT, &sysobj, Symbol::mk("wizard"), &v_int(1))
            .unwrap();

        tx.define_property(
            &SYSTEM_OBJECT,
            &sysobj,
            &sysobj,
            Symbol::mk("test"),
            &SYSTEM_OBJECT,
            BitEnum::all(),
            Some(v_int(1)),
        )
        .unwrap();

        for (verb_name, program) in verbs {
            tx.add_verb(
                &SYSTEM_OBJECT,
                &sysobj.clone(),
                vec![Symbol::mk(verb_name)],
                &sysobj.clone(),
                VerbFlag::rxd(),
                VerbArgsSpec::this_none_this(),
                ProgramType::MooR((*program).clone()),
            )
            .unwrap();
        }
        tx.commit().unwrap();
        state
    }

    fn test_db_with_verb(verb_name: &str, program: &Program) -> TxDB {
        test_db_with_verbs(&[(verb_name, program)])
    }

    fn world_with_test_program(program: &str) -> Box<dyn WorldState> {
        let binary = compile(program, CompileOptions::default()).unwrap();
        let db = test_db_with_verb("test", &binary);
        db.new_world_state().unwrap()
    }

    fn world_with_test_programs(programs: &[(&str, &Program)]) -> Box<dyn WorldState> {
        let db = test_db_with_verbs(programs);
        db.new_world_state().unwrap()
    }

    type ExecResult = Result<Var, moor_common::tasks::Exception>;

    /// Execute MOO code and return the result.
    fn run_moo(program: &str) -> ExecResult {
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        )
    }

    /// Execute MOO code with arguments and return the result.
    fn run_moo_with_args(program: &str, args: List) -> ExecResult {
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        call_verb(state, session, BuiltinRegistry::new(), "test", args)
    }

    #[test]
    fn test_list_splice() {
        assert_eq!(
            run_moo("a = {1,2,3,4,5}; return {@a[2..4]};"),
            Ok(v_list(&[2.into(), 3.into(), 4.into()]))
        );
    }

    #[test]
    fn test_if_or_expr() {
        assert_eq!(
            run_moo("if (1 || 0) return 1; else return 2; endif"),
            Ok(v_int(1))
        );
    }

    #[test]
    fn test_assignment_from_range() {
        assert_eq!(
            run_moo("x = 1; y = {1,2,3}; x = x + y[2]; return x;"),
            Ok(v_int(3))
        );
    }

    #[test]
    fn test_while_loop() {
        assert_eq!(
            run_moo("x = 0; while (x<100) x = x + 1; if (x == 75) break; endif endwhile return x;"),
            Ok(v_int(75))
        );
    }

    #[test]
    fn test_while_labelled_loop() {
        assert_eq!(
            run_moo(
                "x = 0; while broken (1) x = x + 1; if (x == 50) break; else continue broken; endif endwhile return x;"
            ),
            Ok(v_int(50))
        );
    }

    #[test]
    fn test_while_breaks() {
        assert_eq!(
            run_moo("x = 0; while (1) x = x + 1; if (x == 50) break; endif endwhile return x;"),
            Ok(v_int(50))
        );
    }

    #[test]
    fn test_while_continue() {
        let program = r#"
        x = 0;
        while (1)
            x = x + 1;
            if (x == 50)
                break;
            else
                continue;
            endif
            continue;
        endwhile
        return x;
        "#;
        assert_eq!(run_moo(program), Ok(v_int(50)));
    }

    #[test]
    fn test_if_elseif_else_chain() {
        let program = r#"
            ret = {};
            for a in ({1,2,3})
                if (a == 1)
                    ret = {1, @ret};
                elseif (a == 2)
                    ret = {2, @ret};
                else
                    ret = {3, @ret};
                endif
            endfor
            return ret;
        "#;
        assert_eq!(
            run_moo(program),
            Ok(v_list(&[v_int(3), v_int(2), v_int(1)]))
        );
    }

    #[test]
    fn test_if_elseif_elseif_chains() {
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
        assert_eq!(run_moo(program), Ok(v_int(6)));
    }

    #[test]
    fn regression_infinite_loop_bf_error() {
        // This ended up in an infinite loop because of faulty error handling coming out of the
        // builtin call when 'what' is not a valid object.
        let program = r#"{what, targ} = args;
                         try
                           while (what != targ)
                             what = parent(what);
                           endwhile
                           return targ != #-1;
                         except (E_INVARG)
                           return 0;
                         endtry"#;
        assert_eq!(
            run_moo_with_args(program, List::mk_list(&[v_obj(SYSTEM_OBJECT), v_objid(32)])),
            Ok(v_int(0))
        );
    }

    #[test]
    fn test_regression_catch_issue_23() {
        // https://github.com//moor/issues/23
        assert_eq!(
            run_moo(
                r#"try 5; except error (E_RANGE) return 1; endtry for x in [1..1] return 5; endfor"#
            ),
            Ok(v_int(5))
        );
    }

    #[test]
    fn test_try_finally_regression_1() {
        // "Finally" should not get invoked on exit conditions like return/abort, etc.
        assert_eq!(
            run_moo(
                r#"a = 1; try return "hello world"[2..$]; a = 3; finally a = 2; endtry return a;"#
            ),
            Ok(v_str("ello world"))
        );
    }

    #[test]
    fn test_try_expr_regression() {
        // A 0 value was hanging around on the stack making the comparison fail.
        assert_eq!(
            run_moo(
                r#"if (E_INVARG == (vi = `verb_info(#-1, "blerg") ! ANY')) return 666; endif return 333;"#
            ),
            Ok(v_int(666))
        );
    }

    #[test]
    fn test_regression_zero_body_function() {
        // A VM body that is empty should return v_bool(false) or v_int(0) and not panic.
        let binary = Program::new();
        let state = test_db_with_verb("test", &binary)
            .new_world_state()
            .unwrap();
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result, Ok(v_int(0)));
    }

    #[test]
    fn test_catch_any_regression() {
        // Test that nested try/except correctly catches errors from verb calls
        let top_of_stack = r#"
            try
                try
                   #1.location:cause_error();
                   return "should not reach here";
                except z (ANY)
                   this:raise_error();
                   return "should not reach here";
                endtry
            except id (ANY)
                return "should reach here";
            endtry
            return "should not reach here";
            "#;
        let bottom_of_stack = r#"raise(E_ARGS);"#;

        let state = world_with_test_programs(&[
            (
                "raise_error",
                &compile(bottom_of_stack, CompileOptions::default()).unwrap(),
            ),
            (
                "test",
                &compile(top_of_stack, CompileOptions::default()).unwrap(),
            ),
        ]);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result, Ok(v_str("should reach here")));
    }

    #[test]
    fn test_try_except_str() {
        let program = r#"try
          return "hello world"[2..$];
        except (E_RANGE)
        endtry"#;
        assert_eq!(run_moo(program), Ok(v_str("ello world")));
    }

    #[test]
    fn test_try_finally_returns() {
        assert_eq!(
            run_moo(r#"try return 666; finally return 333; endtry"#),
            Ok(v_int(333))
        );
    }

    #[test]
    fn test_lexical_scoping() {
        // Assign a value to a global from a lexically scoped value.
        assert_eq!(
            run_moo(
                r#"
                x = 52;
                begin
                    let y = 42;
                    x = y;
                end
                return x;
                "#
            ),
            Ok(v_int(42))
        );
    }

    #[test]
    fn test_lexical_scoping_shadowing1() {
        // Global with inner scope shadowing it, return value should be the value assigned in the
        // outer (global) scope, since the new lexical scoped value should not be visible.
        assert_eq!(
            run_moo(
                r#"
                x = 52;
                begin
                    let x = 42;
                    x = 1;
                end
                return x;
                "#
            ),
            Ok(v_int(52))
        );
    }

    #[test]
    fn test_lexical_scoping_shadowing2() {
        // Global is set, then shadowed in lexical scope, and returned inside the inner scope,
        // should return the inner scope value.
        assert_eq!(
            run_moo(
                r#"
                x = 52;
                begin
                    let x = 42;
                    let y = 66;
                    return {x, y};
                end
                "#
            ),
            Ok(v_list(&[v_int(42), v_int(66)]))
        );
    }

    #[test]
    fn test_lexical_scoping_we_must_go_deeper() {
        // Global is set, then shadowed in lexical scope, and returned inside the inner scope,
        // should return the inner scope value.
        assert_eq!(
            run_moo(
                r#"
                x = 52;
                begin
                    let x = 42;
                    let y = 66;
                    begin
                        let z = 99;
                        y = 13;
                        return {x, y, z};
                    end
                end
                "#
            ),
            Ok(v_list(&[v_int(42), v_int(13), v_int(99)]))
        );
    }

    #[test]
    fn test_lexical_scoping_in_if_blocks() {
        // Verify that if statements get their own lexical scope, in this case "y" shadowing the
        // global "y" value.
        assert_eq!(
            run_moo(
                r#"
                global y = 2;
                let z = 3;
                if (1)
                    let y = 5;
                    return {y, z};
                else
                    return 0;
                endif"#
            ),
            Ok(v_list(&[v_int(5), v_int(3)]))
        );
    }

    #[test]
    fn test_lexical_scoping_in_while_blocks() {
        assert_eq!(
            run_moo(
                r#"
                global y = 2;
                let z = 3;
                while (1)
                    let y = 5;
                    return {y, z};
                endwhile"#
            ),
            Ok(v_list(&[v_int(5), v_int(3)]))
        );
    }

    #[test]
    fn test_lexical_scoping_in_for_blocks() {
        assert_eq!(
            run_moo(
                r#"
                global y = 2;
                let z = 3;
                for x in ({1,2,3})
                    let y = 5;
                    return {y, z};
                endfor"#
            ),
            Ok(v_list(&[v_int(5), v_int(3)]))
        );
    }

    #[test]
    fn test_lexical_scoping_in_try_blocks() {
        assert_eq!(
            run_moo(
                r#"
                global y = 2;
                let z = 3;
                try
                    let y = 5;
                    return {y, z};
                except (E_INVARG)
                    return 0;
                endtry"#
            ),
            Ok(v_list(&[v_int(5), v_int(3)]))
        );
    }

    #[test]
    fn test_const_assign() {
        assert_eq!(run_moo("const x = 42; return x;"), Ok(v_int(42)));
    }

    #[test]
    fn test_local_scatter_assign() {
        assert_eq!(
            run_moo(
                r#"a = 1;
                begin
                    let {a, b} = {2, 3};
                    return {a, b};
                end
                "#
            ),
            Ok(v_list(&[v_int(2), v_int(3)]))
        );
    }

    #[test_case("return 1;", v_int(1); "simple return")]
    #[test_case(
        r#"rest = "me:words"; rest[1..0] = ""; return rest;"#,
        v_str("me:words"); "range assignment"
    )]
    #[test_case(r#"rest = "me:words"; rest[0..2] = ""; return rest;"#, v_str(":words");
        "range assignment 2")]
    #[test_case(r#"return (!1 || 1);"#, v_int(1); "not/or precedence")]
    #[test_case(r#"return {1, eval("return $test;")};"#,
            v_list(&[v_int(1), v_list(&[v_bool_int(true), v_int(1)])]); "eval builtin")]
    #[test_case(
        r#"string="you";
                         i = index("abcdefghijklmnopqrstuvwxyz", string[1]);
                         string[1] = "ABCDEFGHIJKLMNOPQRSTUVWXYZ"[i];
                         return string;
                        "#,
        v_str("You"); "string index / index assignment / case compare"
    )]
    #[test_case(
        r#"
          c = 0;
          object = #1;
          while properties (1)
            c = c + 1;
            if (c > 10)
                return #4;
            endif
            object = #2;
            break properties;
          endwhile
          return object;
        "#,
        v_objid(2);
        "labelled break"
    )]
    #[test_case(
        r#"
        try return "hello world"[2..$]; finally endtry return "oh nope!";
        "#,
        v_str("ello world");
        "try finally string indexing"
    )]
    #[test_case(r#"return `"hello world"[2..$] ! ANY';"#, v_str("ello world"))]
    #[test_case(
        r#"
        try
          return "hello world"[2..$];
        except (E_RANGE)
        endtry
        "#,
        v_str("ello world"); "try except string indexing"
    )]
    #[test_case(r#"a = "you"; a[1] = "Y"; return a;"#, v_str("You") ; "string index assignment")]
    #[test_case("a={1,2,3,4}; a[1..2] = {3,4}; return a;",
        v_list(&[v_int(3), v_int(4), v_int(3), v_int(4)]) ; "range assignment 3")]
    #[test_case("try a; finally return 666; endtry return 333;",
        v_int(666); "try finally")]
    #[test_case("try a; except e (E_VARNF) return 666; endtry return 333;",
        v_int(666); "try except")]
    #[test_case("return `1/0 ! ANY';", v_err(E_DIV); "catch expr 1")]
    #[test_case("return {`x ! e_varnf => 666', `321 ! e_verbnf => 123'};",
        v_list(&[v_int(666), v_int(321)]); "catch expr 2")]
    #[test_case("return 1 ? 2 | 3;", v_int(2);"ternary expr")]
    #[test_case("{a,b,c} = {{1,2,3}}[1]; return {a,b,c};",
        v_list(&[v_int(1), v_int(2), v_int(3)]); "tuple/splice assignment")]
    #[test_case("return {{1,2,3}[2..$], {1}[$]};",
        v_list(&[
            v_list(&[v_int(2), v_int(3)]), v_int(1)]);
        "range to end retrieval")]
    #[test_case( "{a,b,@c}= {1,2,3,4,5}; return c;",
        v_list(&[v_int(3), v_int(4), v_int(5)]); "new scatter regression")]
    #[test_case("{?a, ?b, ?c, ?d = a, @remain} = {1, 2, 3}; return {d, c, b, a, remain};",
        v_list(&[v_int(1), v_int(3), v_int(2), v_int(1), v_empty_list()]); "complicated scatter")]
    #[test_case("{a, b, @c} = {1, 2, 3, 4}; {x, @y, ?z} = {5,6,7,8}; return {a,b,c,x,y,z};",
        v_list(&[
            v_int(1),
            v_int(2),
            v_list(&[v_int(3), v_int(4)]),
            v_int(5),
            v_list(&[v_int(6), v_int(7)]),
            v_int(8),
        ]); "scatter complex 2")]
    #[test_case("{a, b, c, ?d = 4} = {1, 2, 3}; return {d, c, b, a};",
        v_list(&[v_int(4), v_int(3), v_int(2), v_int(1)]); "scatter optional")]
    #[test_case("z = 0; for i in [1..4] z = z + i; endfor return {i,z};",
        v_list(&[v_int(4), v_int(10)]); "for range loop")]
    #[test_case("x = {1,2,3,4}; z = 0; for i in (x) z = z + i; endfor return {i,z};",
        v_list(&[v_int(4), v_int(10)]); "for list loop")]
    #[test_case(r#"if (E_INVARG == (vi = `verb_info(#-1, "blerg") ! ANY')) return 666; endif return 333;"#,
        v_int(666); "verb_info invalid object error")]
    #[test_case("return -9223372036854775808;", v_int(i64::MIN); "minint")]
    #[test_case("return [ 1 -> 2][1];", v_int(2); "map index")]
    #[test_case("return [ 0 -> 1, 1 -> 2, 2 -> 3, 3 -> 4][1..3];",
        v_map(&[(v_int(1),v_int(2)), (v_int(2),v_int(3))]); "map range")]
    #[test_case(r#"m = [ 1 -> "one", 2 -> "two", 3 -> "three" ]; m[1] = "abc"; return m;"#,
        v_map(&[(v_int(1),v_str("abc")), (v_int(2),v_str("two")), (v_int(3),v_str("three"))]); "map assignment"
    )]
    #[test_case("return [ 0 -> 1, 1 -> 2, 2 -> 3, 3 -> 4][1..$];",
        v_map(&[(v_int(1),v_int(2)), (v_int(2),v_int(3),), (v_int(3),v_int(4))]); "map range to end"
    )]
    #[test_case("l = {1,2,3}; l[2..3] = {6, 7, 8, 9}; return l;",
         v_list(&[v_int(1), v_int(6), v_int(7), v_int(8), v_int(9)]); "list assignment to range")]
    #[test_case("1 == 2 && return 0; 1 == 1 && return 1; return 2;", v_int(1); "short circuit return expr"
    )]
    #[test_case("true && return;", v_int(0); "short circuit empty return expr")]
    #[test_case("return 5 &. 3;", v_int(1); "bitwise and")]
    #[test_case("return 5 |. 3;", v_int(7); "bitwise or")]
    #[test_case("return 5 ^. 3;", v_int(6); "bitwise xor")]
    #[test_case("return 5 << 1;", v_int(10); "left shift")]
    #[test_case("return 10 >> 1;", v_int(5); "right shift")]
    #[test_case("return ~5;", v_int(-6); "bitwise complement")]
    fn test_run(program: &str, expected_result: Var) {
        assert_eq!(run_moo(program), Ok(expected_result));
    }

    #[test]
    fn test_list_assignment_to_range() {
        assert_eq!(
            run_moo(r#"l = {1,2,3}; l[2..3] = {6, 7, 8, 9}; return l;"#),
            Ok(v_list(&[v_int(1), v_int(6), v_int(7), v_int(8), v_int(9)]))
        );
    }

    #[test]
    fn test_make_flyweight() {
        assert_eq!(
            run_moo(r#"return <#1, .slot = "123", {1, 2, 3}>;"#).unwrap(),
            v_flyweight(
                Obj::mk_id(1),
                &[(Symbol::mk("slot"), v_str("123"))],
                List::mk_list(&[v_int(1), v_int(2), v_int(3)]),
            )
        );
    }

    #[test]
    fn test_flyweight_slot() {
        assert_eq!(
            run_moo(r#"return <#1, .slot = "123", {1, 2, 3}>.slot;"#).unwrap(),
            v_str("123")
        );
    }

    #[test]
    fn test_flyweight_slot_assignment() {
        assert_eq!(
            run_moo(
                r#"
                let fw = <#1, .slot = "123", {1, 2, 3}>;
                fw.slot = "456";
                return fw.slot;
                "#
            )
            .unwrap(),
            v_str("456")
        );
    }

    #[test]
    fn test_flyweight_slot_assignment_in_list() {
        assert_eq!(
            run_moo(
                r#"
                let fw = <#1, .slot = "123", {1, 2, 3}>;
                let l = {fw};
                l[1].slot = "456";
                return l[1].slot;
                "#
            )
            .unwrap(),
            v_str("456")
        );
    }

    #[test]
    fn test_flyweight_builtins() {
        assert_eq!(
            run_moo(
                r#"let a = <#1, .slot = "123", {1, 2, 3}>;
                let b = flyslotremove(a, 'slot);
                let c = flyslotset(b, 'bananas, "456");
                return {c, flyslots(c)};"#
            )
            .unwrap(),
            v_list(&[
                v_flyweight(
                    Obj::mk_id(1),
                    &[(Symbol::mk("bananas"), v_str("456"))],
                    List::mk_list(&[v_int(1), v_int(2), v_int(3)]),
                ),
                v_map(&[(v_sym("bananas"), v_str("456"))])
            ])
        );
    }

    #[test]
    fn test_flyweight_sequence() {
        assert_eq!(
            run_moo(
                r#"
                let fw = <#1, .slot = "123", {1, 2, 3}>;
                return flycontents(fw)[2];
                "#
            )
            .unwrap(),
            v_int(2)
        );
    }

    #[test]
    fn test_flyweight_contents_set() {
        assert_eq!(
            run_moo(
                r#"
                let a = <#1, .slot = "123", {1, 2, 3}>;
                let b = flycontentsset(a, {"x", "y"});
                return {flycontents(a), flycontents(b)};
                "#
            )
            .unwrap(),
            v_list(&[
                v_list(&[v_int(1), v_int(2), v_int(3)]),
                v_list(&[v_str("x"), v_str("y")]),
            ])
        );
    }

    #[test]
    fn test_range_in_map_oddities() {
        // Bug where stack offsets were wrong in maps, causing problems with the $ range operation
        assert_eq!(
            run_moo(r#"return[ "z"->5, "b"->"another_seq"[1..$]]["b"];"#).unwrap(),
            v_str("another_seq")
        );
    }

    #[test]
    fn test_range_flyweight_oddities() {
        assert_eq!(
            run_moo(
                r#"
                let fw = <#1, .another_slot = 5, .slot = "123", {"another_seq"[1..$]}>;
                return flycontents(fw)[1];
                "#
            )
            .unwrap(),
            v_str("another_seq")
        );
    }

    #[test]
    fn test_for_range_length_dollar_regression() {
        // Regression test for issue #482 - ForRange with $ operator crashes
        assert_eq!(
            run_moo(
                r#"
                for i in [1..3]
                    return "hello"[1..$];
                endfor
                "#
            )
            .unwrap(),
            v_str("hello")
        );
    }

    #[test]
    fn test_for_sequence_length_dollar_regression() {
        // Regression test for issue #482 - ForSequence with $ operator crashes
        assert_eq!(
            run_moo(
                r#"
                for i in ({"a", "b", "c"})
                    return "hello"[1..$];
                endfor
                "#
            )
            .unwrap(),
            v_str("hello")
        );
    }

    #[test]
    fn test_for_range_comprehension() {
        assert_eq!(
            run_moo(r#"return { x * 2 for x in [1..3] };"#).unwrap(),
            v_list(&[v_int(2), v_int(4), v_int(6)])
        );
    }

    #[test]
    fn test_for_list_comprehension() {
        assert_eq!(
            run_moo(r#"return { x * 2 for x in ({1,2,3}) };"#).unwrap(),
            v_list(&[v_int(2), v_int(4), v_int(6)])
        );
    }

    #[test]
    fn test_for_list_comprehension_scope_regression() {
        assert_eq!(
            run_moo(
                r#"
                let x = {1,2,3};
                if (false)
                    y = {v * 2 for v in (x)};
                endif
                let z = 1;
                if (false)
                endif
                return z;
                "#
            )
            .unwrap(),
            v_int(1)
        );
    }

    #[test]
    fn test_for_v_k_in_map() {
        assert_eq!(
            run_moo(
                r#"
                let result = {};
                for v, k in (["a" -> "b", "c" -> "d"])
                    result = {@result, @{k, v}};
                endfor
                return result;
                "#
            )
            .unwrap(),
            v_list(&[v_str("a"), v_str("b"), v_str("c"), v_str("d")])
        );
    }

    #[test]
    fn test_for_v_k_in_list() {
        assert_eq!(
            run_moo(
                r#"
                let result = {};
                for v, k in ({"a", "b"})
                    result = {@result, @{k, v}};
                endfor
                return result;
                "#
            )
            .unwrap(),
            v_list(&[v_int(1), v_str("a"), v_int(2), v_str("b")])
        );
    }

    #[test]
    fn test_scope_width_regression() {
        assert_eq!(
            run_moo(
                r#"
                let x = 1;
                for i in [0..1024]
                    let y = 2 * i;
                endfor
                return 0;
                "#
            )
            .unwrap(),
            v_int(0)
        );
    }

    #[test]
    fn test_regress_except() {
        assert_eq!(
            run_moo(r#"return {`x ! e_varnf => 666', `321 ! e_verbnf => 123'};"#).unwrap(),
            v_list(&[v_int(666), v_int(321)])
        );
    }

    #[test]
    fn test_simple_fork() {
        assert_eq!(
            run_moo(
                r#"
                fork (0)
                    return 42;
                endfork
                return 24;
                "#
            )
            .unwrap(),
            v_int(24)
        );
    }

    #[test]
    fn test_fork_error_line_numbers() {
        // This test verifies that line numbers are correctly reported in exceptions
        // that occur within fork blocks during testing
        let program = r#"x = 1;
        fork (0)
            y = 2;
            z = 3;
            raise(E_ARGS);
        endfork
        return 99;"#;

        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());

        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );

        match result {
            Err(exception) => {
                assert_eq!(exception.error, Error::from(E_ARGS));
                assert!(!exception.backtrace.is_empty());
                assert!(!exception.stack.is_empty());

                let last_stack = exception.stack.last().expect("Expected a stack frame");
                let line_no = last_stack
                    .get(&v_int(6), IndexMode::OneBased)
                    .unwrap()
                    .as_integer()
                    .expect("Expected line number to be an integer");
                assert_eq!(
                    line_no, 5,
                    "Expected line number in the backtrace to be 5, but got {line_no}"
                );
            }
            Ok(_) => {
                panic!("Expected an exception to be raised from the fork");
            }
        }
    }

    #[test]
    fn test_multiple_forks_line_numbers() {
        let program = r#"x = 1;
        fork (0)
            y = 2;
            raise(E_ARGS);  // line 4
        endfork
        z = 3;
        fork (0)
            a = 4;
            b = 5;
            raise(E_PERM);  // line 10
        endfork
        return 99;"#;

        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());

        let result = call_verb(
            state,
            session.clone(),
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );

        match result {
            Err(exception) => {
                assert_eq!(exception.error, Error::from(E_ARGS));
                let last_stack = exception.stack.last().expect("Expected a stack frame");
                let line_no = last_stack
                    .get(&v_int(6), IndexMode::OneBased)
                    .unwrap()
                    .as_integer()
                    .expect("Expected line number to be an integer");
                assert_eq!(
                    line_no, 4,
                    "Expected line number in the first fork to be 4, but got {line_no}"
                );
            }
            Ok(_) => {
                panic!("Expected an exception to be raised from the first fork");
            }
        }

        let mut _state = world_with_test_program(program);
        let _session = Arc::new(NoopClientSession::new());

        let program2 = r#"x = 1;
        fork (0)
            y = 2;
            return 42;  // first fork succeeds
        endfork
        z = 3;
        fork (0)
            a = 4;
            b = 5;
            raise(E_PERM);  // line 10
        endfork
        return 99;"#;

        let state = world_with_test_program(program2);
        let session = Arc::new(NoopClientSession::new());

        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );

        if let Err(exception) = result
            && exception.error == E_PERM
        {
            let last_stack = exception.stack.last().expect("Expected a stack frame");
            let line_no = last_stack
                .get(&v_int(6), IndexMode::OneBased)
                .unwrap()
                .as_integer()
                .expect("Expected line number to be an integer");
            assert_eq!(
                line_no, 10,
                "Expected line number in the second fork to be 10, but got {line_no}"
            );
        }
    }

    #[test]
    fn test_nested_fork_line_numbers() {
        let program = r#"x = 1;
        fork (0)
            y = 2;
            fork (0)
                z = 3;
                raise(E_QUOTA);  // line 6
            endfork
            a = 4;
        endfork
        return 99;"#;

        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());

        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );

        match result {
            Err(exception) => {
                assert_eq!(exception.error, Error::from(E_QUOTA));
                let last_stack = exception.stack.last().expect("Expected a stack frame");
                let line_no = last_stack
                    .get(&v_int(6), IndexMode::OneBased)
                    .unwrap()
                    .as_integer()
                    .expect("Expected line number to be an integer");
                assert_eq!(
                    line_no, 6,
                    "Expected line number in the nested fork to be 6, but got {line_no}"
                );
            }
            Ok(_) => {
                panic!("Expected an exception to be raised from the nested fork");
            }
        }
    }

    #[test]
    fn test_fork_line_numbers_with_offset() {
        let program = r#"// Line 1
        x = 1;          // Line 2
        y = 2;          // Line 3
        z = 3;          // Line 4
        if (true)       // Line 5
            a = 4;      // Line 6
            b = 5;      // Line 7
        endif           // Line 8
        fork (0)        // Line 9
            c = 6;      // Line 10
            d = 7;      // Line 11
            raise(E_RANGE);  // Line 12
        endfork         // Line 13
        return 99;"#;

        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());

        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );

        match result {
            Err(exception) => {
                assert_eq!(exception.error, Error::from(E_RANGE));
                let last_stack = exception.stack.last().expect("Expected a stack frame");
                let line_no = last_stack
                    .get(&v_int(6), IndexMode::OneBased)
                    .unwrap()
                    .as_integer()
                    .expect("Expected line number to be an integer");
                assert_eq!(
                    line_no, 11,
                    "Expected line number in the offset fork to be 11, but got {line_no}"
                );
            }
            Ok(_) => {
                panic!("Expected an exception to be raised from the fork");
            }
        }
    }

    #[test]
    fn test_lambda_creation() {
        let program_text = r#"
            let f = {x} => x + 1;
            return f;
        "#;

        let program = compile(program_text, CompileOptions::default()).unwrap();
        let state_source = test_db_with_verb("test", &program);
        let state = state_source.new_world_state().unwrap();
        let session = Arc::new(NoopClientSession::new());

        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        )
        .unwrap();

        assert!(
            result.as_lambda().is_some(),
            "Expected lambda value, got: {result:?}",
        );

        let lambda = result.as_lambda().unwrap();
        assert_eq!(lambda.0.params.labels.len(), 1, "Expected 1 parameter");

        match &lambda.0.params.labels[0] {
            moor_var::program::opcode::ScatterLabel::Required(_) => {}
            other => panic!("Expected Required parameter, got: {other:?}"),
        }

        let literal_form = moor_compiler::to_literal(&result);
        assert!(
            literal_form.contains("{x} => x + 1"),
            "Lambda literal should contain correct syntax, got: {literal_form}",
        );
    }

    #[test]
    fn test_lambda_with_multiple_params() {
        let program_text = r#"
            let f = {x, ?y, @rest} => x + y;
            return f;
        "#;

        let program = compile(program_text, CompileOptions::default()).unwrap();
        let state_source = test_db_with_verb("test", &program);
        let state = state_source.new_world_state().unwrap();
        let session = Arc::new(NoopClientSession::new());

        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        )
        .unwrap();

        let lambda = result.as_lambda().unwrap();
        assert_eq!(lambda.0.params.labels.len(), 3, "Expected 3 parameters");

        match &lambda.0.params.labels[0] {
            moor_var::program::opcode::ScatterLabel::Required(_) => {}
            other => panic!("Expected Required parameter, got: {other:?}"),
        }

        match &lambda.0.params.labels[1] {
            moor_var::program::opcode::ScatterLabel::Optional(_, _) => {}
            other => panic!("Expected Optional parameter, got: {other:?}"),
        }

        match &lambda.0.params.labels[2] {
            moor_var::program::opcode::ScatterLabel::Rest(_) => {}
            other => panic!("Expected Rest parameter, got: {other:?}"),
        }
    }

    #[test]
    fn test_lambda_simple_call() {
        assert_eq!(
            run_moo(
                r#"
                let add = {x, y} => x + y;
                return add(5, 3);
                "#
            ),
            Ok(v_int(8))
        );
    }

    #[test]
    fn test_lambda_single_parameter() {
        assert_eq!(
            run_moo(
                r#"
                let double = {x} => x * 2;
                return double(7);
                "#
            ),
            Ok(v_int(14))
        );
    }

    #[test]
    fn test_lambda_no_parameters() {
        assert_eq!(
            run_moo(
                r#"
                let hello = {} => "Hello, World!";
                return hello();
                "#
            ),
            Ok(v_str("Hello, World!"))
        );
    }

    #[test]
    fn test_lambda_optional_parameters() {
        assert_eq!(
            run_moo(
                r#"
                let greet = {name, ?greeting} => (greeting || "Hello") + ", " + name + "!";
                let result1 = greet("Alice");
                let result2 = greet("Bob", "Hi");
                return {result1, result2};
                "#
            ),
            Ok(v_list(&[v_str("Hello, Alice!"), v_str("Hi, Bob!")]))
        );
    }

    #[test]
    fn test_lambda_rest_parameters() {
        assert_eq!(
            run_moo(
                r#"
                let sum = fn(@numbers)
                    let total = 0;
                    for n in (numbers)
                        total = total + n;
                    endfor
                    return total;
                endfn;
                return sum(1, 2, 3, 4, 5);
                "#
            ),
            Ok(v_int(15))
        );
    }

    #[test]
    fn test_lambda_mixed_parameters() {
        assert_eq!(
            run_moo(
                r#"
                let func = fn(required, ?optional, @rest)
                    return {required, optional || 0, length(rest)};
                endfn;
                return func(42, 100, "a", "b", "c");
                "#
            ),
            Ok(v_list(&[v_int(42), v_int(100), v_int(3)]))
        );
    }

    #[test]
    fn test_lambda_closure_capture() {
        assert_eq!(
            run_moo(
                r#"
                let x = 10;
                let y = 20;
                let adder = {z} => x + y + z;
                return adder(5);
                "#
            ),
            Ok(v_int(35))
        );
    }

    #[test]
    fn test_lambda_nested_scope_capture() {
        assert_eq!(
            run_moo(
                r#"
                let make_multiplier = fn(factor)
                    return {x} => x * factor;
                endfn;
                let times3 = make_multiplier(3);
                return times3(7);
                "#
            ),
            Ok(v_int(21))
        );
    }

    #[test]
    fn test_lambda_double_recursive_call() {
        assert_eq!(
            run_moo(
                r#"
                fn test(x)
                    return test(1) + test(2);
                endfn
                return 1;
                "#
            ),
            Ok(v_int(1))
        );
    }

    #[test]
    fn test_lambda_recursive_fibonacci() {
        assert_eq!(
            run_moo(
                r#"
                fn fib(n)
                    if (n <= 1)
                        return n;
                    else
                        return fib(n - 1) + fib(n - 2);
                    endif
                endfn
                return fib(6);
                "#
            ),
            Ok(v_int(8))
        );
    }

    #[test]
    fn test_parameterless_lambda_captures_outer_variable() {
        // This is the core bug fix test: parameterless lambdas must capture outer variables.
        // Previously, CaptureAnalyzer used .unwrap_or(0) for parameterless lambdas,
        // so variables at scope depth > 0 weren't captured.
        assert_eq!(
            run_moo(
                r#"
                let value = 42;
                let get_value = fn ()
                    return value;
                endfn;
                return get_value();
                "#
            ),
            Ok(v_int(42))
        );
    }

    #[test]
    fn test_nested_parameterless_lambda_capture() {
        // Verify lambda_scope_depth tracking works for nested parameterless lambdas.
        // The outer lambda creates `inner`, and the nested parameterless lambda
        // must capture both `outer` (from grandparent scope) and `inner` (from parent scope).
        assert_eq!(
            run_moo(
                r#"
                let outer = 10;
                let make_getter = fn ()
                    let inner = 5;
                    return fn ()
                        return outer + inner;
                    endfn;
                endfn;
                let getter = make_getter();
                return getter();
                "#
            ),
            Ok(v_int(15))
        );
    }

    #[test]
    fn test_triple_nested_lambda_capture() {
        // Verify transitive capture works at deeper nesting levels.
        assert_eq!(
            run_moo(
                r#"
                let a = 1;
                let f1 = fn ()
                    let b = 2;
                    let f2 = fn ()
                        let c = 3;
                        return fn ()
                            return a + b + c;
                        endfn;
                    endfn;
                    return f2();
                endfn;
                let r1 = f1();
                return r1();
                "#
            ),
            Ok(v_int(6))
        );
    }

    #[test]
    fn test_lambda_double_call_syntax() {
        // Verify that make_getter()() syntax works (calling a returned lambda immediately)
        assert_eq!(
            run_moo(
                r#"
                let make_getter = fn ()
                    return fn ()
                        return 42;
                    endfn;
                endfn;
                return make_getter()();
                "#
            ),
            Ok(v_int(42))
        );
    }

    #[test]
    fn test_lambda_with_param_containing_nested_parameterless() {
        // Lambda with params containing a nested parameterless lambda.
        // The nested lambda must capture both `outer` and `x` (from wrapper's param).
        assert_eq!(
            run_moo(
                r#"
                let outer = 10;
                let wrapper = fn (x)
                    return fn ()
                        return outer + x;
                    endfn;
                endfn;
                let inner = wrapper(5);
                return inner();
                "#
            ),
            Ok(v_int(15))
        );
    }

    /// Generate MOO code for N levels of nested parameterless lambdas.
    /// Each level creates a local variable with value (level), and the innermost
    /// lambda returns the sum of all captured variables.
    fn generate_nested_lambda_code(depth: usize) -> String {
        assert!(depth >= 1, "depth must be at least 1");

        let mut code = String::new();

        // Create variables at each level: v0 = 1, then nested lambdas with v1 = 2, v2 = 3, etc.
        code.push_str("let v0 = 1;\n");

        // Generate nested lambda structure
        for level in 1..depth {
            code.push_str(&format!("let f{} = fn ()\n", level - 1));
            code.push_str(&format!("    let v{} = {};\n", level, level + 1));
        }

        // Innermost lambda returns sum of all variables
        code.push_str(&format!("let f{} = fn ()\n", depth - 1));
        code.push_str("    return ");
        for i in 0..depth {
            if i > 0 {
                code.push_str(" + ");
            }
            code.push_str(&format!("v{}", i));
        }
        code.push_str(";\nendfn;\n");

        // Close all the outer lambdas and return their results
        for level in (1..depth).rev() {
            code.push_str(&format!("    return f{}();\n", level));
            code.push_str("endfn;\n");
        }

        // Call the outermost lambda
        code.push_str("return f0();\n");

        code
    }

    // Parametric test for nested lambda capture at various depths.
    // Expected result is triangular number: depth * (depth + 1) / 2
    #[test_case(1, 1; "depth 1 - single lambda")]
    #[test_case(2, 3; "depth 2 - double nested")]
    #[test_case(3, 6; "depth 3 - triple nested")]
    #[test_case(4, 10; "depth 4")]
    #[test_case(5, 15; "depth 5")]
    #[test_case(10, 55; "depth 10 - stress test")]
    fn test_nested_lambda_capture_depth(depth: usize, expected_sum: i64) {
        let code = generate_nested_lambda_code(depth);
        assert_eq!(run_moo(&code), Ok(v_int(expected_sum)));
    }

    #[test]
    fn test_lambda_counter_generator_blocked() {
        // Counter generator pattern requires mutable capture, which is intentionally blocked.
        // Assigning to a captured variable is a compile error because:
        // 1. Captures are by-value (copied at lambda creation time)
        // 2. Mutations wouldn't persist across calls anyway (env is copied each activation)
        // 3. The error prevents confusing semantics
        //
        // For a working counter pattern, use flyweights (see test_flyweight_counter_pattern)
        // or store state in an object property.
        let program = r#"
            fn make_counter(initial)
                let count = initial;
                return fn ()
                    count = count + 1;
                    return count;
                endfn;
            endfn
        "#;
        let result = compile(program, CompileOptions::default());
        assert!(
            matches!(
                result,
                Err(CompileError::AssignmentToCapturedVariable(_, _))
            ),
            "Expected AssignmentToCapturedVariable error, got: {:?}",
            result
        );
    }

    #[test]
    fn test_lambda_higher_order_map() {
        assert_eq!(
            run_moo(
                r#"
                let map = fn(func, lst)
                    let result = {};
                    for item in (lst)
                        result = {@result, func(item)};
                    endfor
                    return result;
                endfn;
                let square = {x} => x * x;
                return map(square, {1, 2, 3, 4});
                "#
            ),
            Ok(v_list(&[v_int(1), v_int(4), v_int(9), v_int(16)]))
        );
    }

    #[test]
    fn test_lambda_parameter_error_too_few_args() {
        let program_text = r#"
            let add = {x, y} => x + y;
            return add(5); // Missing second argument
        "#;

        let program = compile(program_text, CompileOptions::default()).unwrap();
        let state_source = test_db_with_verb("test", &program);
        let state = state_source.new_world_state().unwrap();
        let session = Arc::new(NoopClientSession::new());

        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.error, E_ARGS);
    }

    #[test]
    fn test_lambda_parameter_error_too_many_args() {
        let program_text = r#"
            let add = {x, y} => x + y;
            return add(5, 3, 7); // Too many arguments
        "#;

        let program = compile(program_text, CompileOptions::default()).unwrap();
        let state_source = test_db_with_verb("test", &program);
        let state = state_source.new_world_state().unwrap();
        let session = Arc::new(NoopClientSession::new());

        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.error, E_ARGS);
    }

    #[test]
    fn test_lambda_call_type_error() {
        let program_text = r#"
            let not_a_lambda = 42;
            return not_a_lambda(1, 2, 3); // Should fail with E_TYPE
        "#;

        let program = compile(program_text, CompileOptions::default()).unwrap();
        let state_source = test_db_with_verb("test", &program);
        let state = state_source.new_world_state().unwrap();
        let session = Arc::new(NoopClientSession::new());

        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.error, E_TYPE);
    }

    #[test]
    fn test_lambda_stack_trace_format() {
        let program_text = r#"
            let lambda_func = fn(a, b)
                return a / b; // Division by zero will cause error
            endfn;
            return lambda_func(1, 0); // Call lambda with division by zero
        "#;

        let program = compile(program_text, CompileOptions::default()).unwrap();
        let state_source = test_db_with_verb("test", &program);
        let state = state_source.new_world_state().unwrap();
        let session = Arc::new(NoopClientSession::new());

        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.error, E_DIV);

        let stack_contains_lambda_format = err.backtrace.iter().any(|frame| {
            if let Some(frame_str) = frame.as_string() {
                frame_str.contains("test.<fn>")
            } else {
                false
            }
        });

        assert!(
            stack_contains_lambda_format,
            "Stack trace should contain 'test.<lambda>' format. Actual backtrace: {:?}",
            err.backtrace
        );
    }

    #[test]
    fn test_lambda_line_numbers_simple() {
        let program_text = "let f = fn(x) return x / 0; endfn; return f(1);";

        let program = compile(program_text, CompileOptions::default()).unwrap();
        let state_source = test_db_with_verb("test", &program);
        let state = state_source.new_world_state().unwrap();
        let session = Arc::new(NoopClientSession::new());

        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );

        assert!(result.is_err());
        let err = result.unwrap_err();

        let has_lambda_frame = err.backtrace.iter().any(|frame| {
            if let Some(frame_str) = frame.as_string() {
                frame_str.contains("test.<fn>")
            } else {
                false
            }
        });

        assert!(has_lambda_frame, "Should have lambda frame in stack trace");
    }

    #[test]
    fn test_lambda_named_function_stack_trace() {
        let program_text = r#"
            fn factorial(n)
                if (n <= 1)
                    return 1;
                else
                    return n * factorial(n - 1) / 0; // Division by zero in recursion
                endif
            endfn
            return factorial(3);
        "#;

        let program = compile(program_text, CompileOptions::default()).unwrap();
        let state_source = test_db_with_verb("test", &program);
        let state = state_source.new_world_state().unwrap();
        let session = Arc::new(NoopClientSession::new());

        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.error, E_DIV);

        let has_named_lambda_frame = err.backtrace.iter().any(|frame| {
            if let Some(frame_str) = frame.as_string() {
                frame_str.contains("test.factorial")
            } else {
                false
            }
        });

        assert!(
            has_named_lambda_frame,
            "Should have named lambda frame 'factorial' in stack trace"
        );
    }

    #[test]
    fn test_lambda_capture_pure_lambda() {
        let program_text = r#"
            let f = {x} => x * 5;
            return f;
        "#;

        let program = compile(program_text, CompileOptions::default()).unwrap();
        let state_source = test_db_with_verb("test", &program);
        let state = state_source.new_world_state().unwrap();
        let session = Arc::new(NoopClientSession::new());

        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        )
        .unwrap();

        let lambda = result.as_lambda().unwrap();

        assert!(
            lambda.0.captured_env.is_empty(),
            "Pure lambda should have empty captured environment, got: {:?}",
            lambda.0.captured_env
        );
    }

    #[test]
    fn test_lambda_capture_with_outer_variable() {
        let program_text = r#"
            let multiplier = 10;
            let f = {x} => x * multiplier;
            return f;
        "#;

        let program = compile(program_text, CompileOptions::default()).unwrap();
        let state_source = test_db_with_verb("test", &program);
        let state = state_source.new_world_state().unwrap();
        let session = Arc::new(NoopClientSession::new());

        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        )
        .unwrap();

        let lambda = result.as_lambda().unwrap();

        assert!(
            !lambda.0.captured_env.is_empty(),
            "Lambda should capture outer variable 'multiplier'"
        );
    }

    #[test]
    fn test_lambda_capture_multiple_variables() {
        let program_text = r#"
            let a = 5;
            let b = 10;
            let c = 15;
            let f = {x} => x + a + b; // Only references a and b, not c
            return f;
        "#;

        let program = compile(program_text, CompileOptions::default()).unwrap();
        let state_source = test_db_with_verb("test", &program);
        let state = state_source.new_world_state().unwrap();
        let session = Arc::new(NoopClientSession::new());

        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        )
        .unwrap();

        let lambda = result.as_lambda().unwrap();

        assert!(
            !lambda.0.captured_env.is_empty(),
            "Lambda should capture outer variables 'a' and 'b'"
        );
    }

    #[test]
    fn test_lambda_capture_functionality() {
        assert_eq!(
            run_moo(
                r#"
                let base = 100;
                let f = {x} => x + base;
                return f(5);
                "#
            ),
            Ok(v_int(105))
        );
    }

    #[test]
    fn test_lambda_capture_creation_only() {
        let program_text = r#"
            let outer_var = 999; // This should NOT be captured since it's not referenced
            let f = {x} => x + 1; // Only references parameter 'x', not 'outer_var'
            return f; // Return the lambda itself, don't call it
        "#;

        let program = compile(program_text, CompileOptions::default()).unwrap();
        let state_source = test_db_with_verb("test", &program);
        let state = state_source.new_world_state().unwrap();
        let session = Arc::new(NoopClientSession::new());

        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        )
        .unwrap();

        let lambda = result.as_lambda().unwrap();

        assert!(
            lambda.0.captured_env.is_empty(),
            "Lambda should have empty captured environment since it doesn't reference outer_var"
        );
    }

    #[test]
    fn test_for_loop_continue_regression() {
        assert_eq!(
            run_moo(
                r#"x = {}; for i in ({1, 2, 3, 4, 5}); if (i < 3); continue; endif; x = {@x, i}; endfor; return x;"#
            ),
            Ok(v_list(&[v_int(3), v_int(4), v_int(5)]))
        );
    }

    #[test]
    fn test_for_range_continue_regression() {
        assert_eq!(
            run_moo(
                r#"x = {}; for i in [1..5]; if (i < 3); continue; endif; x = {@x, i}; endfor; return x;"#
            ),
            Ok(v_list(&[v_int(3), v_int(4), v_int(5)]))
        );
    }

    #[test]
    fn test_for_range_with_objects() {
        assert_eq!(
            run_moo(r#"x = {}; for o in [#1..#5]; x = {@x, o}; endfor; return x;"#),
            Ok(v_list(&[
                v_obj(Obj::mk_id(1)),
                v_obj(Obj::mk_id(2)),
                v_obj(Obj::mk_id(3)),
                v_obj(Obj::mk_id(4)),
                v_obj(Obj::mk_id(5))
            ]))
        );
    }

    #[test]
    fn test_for_range_with_floats() {
        assert_eq!(
            run_moo(r#"x = {}; for f in [1.0..3.0]; x = {@x, f}; endfor; return x;"#),
            Ok(v_list(&[v_float(1.0), v_float(2.0), v_float(3.0)]))
        );
    }

    #[test]
    fn test_for_range_type_mismatch() {
        let result = run_moo(r#"for x in [1..#5]; return x; endfor"#);
        assert!(matches!(result, Err(e) if e.error == E_TYPE));
    }

    #[test]
    fn test_scatter_multiple_optionals_regression() {
        assert_eq!(
            run_moo(
                r#"
                args = {"a", "b"};
                {a, ?b = {"1"}, ?c = 0} = args;
                return {a, b, c};
                "#
            ),
            Ok(v_list(&[v_str("a"), v_str("b"), v_int(0)]))
        );

        assert_eq!(
            run_moo(
                r#"
                args = {"a", "b", "c"};
                {a, ?b = {"1"}, ?c = 0} = args;
                return {a, b, c};
                "#
            ),
            Ok(v_list(&[v_str("a"), v_str("b"), v_str("c")]))
        );
    }

    #[test]
    fn test_nested_loop_unlabeled_continue() {
        assert_eq!(
            run_moo(
                r#"
                result = {};
                for x in [1..2]
                    for y in [5..6]
                        if (y == 6)
                            continue; // Should continue the inner y loop
                        endif
                        result = {@result, @{x, y}};
                    endfor
                    result = {@result, 999}; // This should NOT be skipped for unlabeled continue
                endfor
                return result;
                "#
            ),
            Ok(v_list(&[
                v_int(1),
                v_int(5),
                v_int(999),
                v_int(2),
                v_int(5),
                v_int(999),
            ]))
        );
    }

    #[test]
    fn test_nested_loop_labeled_continue() {
        assert_eq!(
            run_moo(
                r#"
                result = {};
                for x in [1..2]
                    for y in [5..6]
                        if (y == 6)
                            continue x;
                        endif
                        result = {@result, @{x, y}};
                    endfor
                    result = {@result, 999}; // This should be skipped when we continue x
                endfor
                return result;
                "#
            ),
            Ok(v_list(&[v_int(1), v_int(5), v_int(2), v_int(5)]))
        );
    }

    #[test]
    fn test_scatter_multiple_optionals_comprehensive() {
        assert_eq!(
            run_moo(
                r#"
                {a, ?b = "default_b", ?c = "default_c", ?d = "default_d"} = {"A", "B", "C", "D"};
                return {a, b, c, d};
                "#
            ),
            Ok(v_list(&[v_str("A"), v_str("B"), v_str("C"), v_str("D")]))
        );

        assert_eq!(
            run_moo(
                r#"
                {a, ?b = "default_b", ?c = "default_c", ?d = "default_d"} = {"A", "B", "C"};
                return {a, b, c, d};
                "#
            ),
            Ok(v_list(&[
                v_str("A"),
                v_str("B"),
                v_str("C"),
                v_str("default_d")
            ]))
        );

        assert_eq!(
            run_moo(
                r#"
                {a, ?b = "default_b", ?c = "default_c", ?d = "default_d"} = {"A", "B"};
                return {a, b, c, d};
                "#
            ),
            Ok(v_list(&[
                v_str("A"),
                v_str("B"),
                v_str("default_c"),
                v_str("default_d"),
            ]))
        );

        assert_eq!(
            run_moo(
                r#"
                {a, ?b = "default_b", ?c = "default_c", ?d = "default_d"} = {"A"};
                return {a, b, c, d};
                "#
            ),
            Ok(v_list(&[
                v_str("A"),
                v_str("default_b"),
                v_str("default_c"),
                v_str("default_d"),
            ]))
        );
    }

    #[test]
    fn test_scatter_five_optionals_edge_cases() {
        assert_eq!(
            run_moo(
                r#"
                {a, ?b = 1, ?c = 2, ?d = 3, ?e = 4, ?f = 5} = {"A", "B", "C", "D"};
                return {a, b, c, d, e, f};
                "#
            ),
            Ok(v_list(&[
                v_str("A"),
                v_str("B"),
                v_str("C"),
                v_str("D"),
                v_int(4),
                v_int(5),
            ]))
        );

        assert_eq!(
            run_moo(
                r#"
                {a, ?b = 1, ?c = 2, ?d = 3, ?e = 4, ?f = 5} = {"A", "B"};
                return {a, b, c, d, e, f};
                "#
            ),
            Ok(v_list(&[
                v_str("A"),
                v_str("B"),
                v_int(2),
                v_int(3),
                v_int(4),
                v_int(5),
            ]))
        );

        assert_eq!(
            run_moo(
                r#"
                {a, ?b = 1, ?c = 2, ?d = 3, ?e = 4, ?f = 5} = {"A"};
                return {a, b, c, d, e, f};
                "#
            ),
            Ok(v_list(&[
                v_str("A"),
                v_int(1),
                v_int(2),
                v_int(3),
                v_int(4),
                v_int(5)
            ]))
        );
    }

    #[test]
    fn test_scatter_mixed_required_optionals() {
        assert_eq!(
            run_moo(
                r#"
                {a, b, ?c = "C", ?d = "D", ?e = "E"} = {"A", "B", "provided_c"};
                return {a, b, c, d, e};
                "#
            ),
            Ok(v_list(&[
                v_str("A"),
                v_str("B"),
                v_str("provided_c"),
                v_str("D"),
                v_str("E"),
            ]))
        );

        assert_eq!(
            run_moo(
                r#"
                {a, b, ?c = "C", ?d = "D", ?e = "E"} = {"A", "B", "provided_c", "provided_d"};
                return {a, b, c, d, e};
                "#
            ),
            Ok(v_list(&[
                v_str("A"),
                v_str("B"),
                v_str("provided_c"),
                v_str("provided_d"),
                v_str("E"),
            ]))
        );
    }

    #[test]
    fn test_lambda_scatter_multiple_optionals_regression() {
        assert_eq!(
            run_moo(
                r#"
                f = {a, ?b = "default_b", ?c = "default_c"} => {a, b, c};
                return f("A", "B");
                "#
            ),
            Ok(v_list(&[v_str("A"), v_str("B"), v_str("default_c")]))
        );

        assert_eq!(
            run_moo(
                r#"
                f = {a, ?b = "default_b", ?c = "default_c"} => {a, b, c};
                return f("A", "B", "C");
                "#
            ),
            Ok(v_list(&[v_str("A"), v_str("B"), v_str("C")]))
        );

        assert_eq!(
            run_moo(
                r#"
                f = {a, ?b = "default_b", ?c = "default_c"} => {a, b, c};
                return f("A");
                "#
            ),
            Ok(v_list(&[
                v_str("A"),
                v_str("default_b"),
                v_str("default_c")
            ]))
        );
    }

    #[test]
    fn test_lambda_scatter_five_optionals() {
        assert_eq!(
            run_moo(
                r#"
                func = {a, ?b = 1, ?c = 2, ?d = 3, ?e = 4, ?g = 5} => {a, b, c, d, e, g};
                return func("A", "B", "C");
                "#
            ),
            Ok(v_list(&[
                v_str("A"),
                v_str("B"),
                v_str("C"),
                v_int(3),
                v_int(4),
                v_int(5),
            ]))
        );
    }

    #[test]
    fn test_lambda_scatter_with_complex_defaults() {
        assert_eq!(
            run_moo(
                r#"
                f = {a, ?b = {1, 2}, ?c = #5} => {a, b, c};
                return f("A");
                "#
            ),
            Ok(v_list(&[
                v_str("A"),
                v_list(&[v_int(1), v_int(2)]),
                v_obj(Obj::mk_id(5)),
            ]))
        );
    }

    #[test]
    fn test_stack_tracking_after_try_finally() {
        assert_eq!(
            run_moo(
                r#"
                result = #-3;
                try
                    temp = 42;
                finally
                    x = 1;
                endtry
                result != #-3 && raise(E_ASSERT, "should not raise");
                return result;
                "#
            ),
            Ok(v_obj(Obj::mk_id(-3)))
        );
    }

    #[test]
    fn test_regression_none_to_scatter_from_error() {
        assert_eq!(
            run_moo(
                r#"try
                    raise(E_INVARG, "test");
                  except e (ANY)
                    {a, b, c, d} = e;
                    return c;
                  endtry
                "#
            ),
            Ok(v_int(0))
        );
    }

    /// Test that initial_env bindings are applied to eval frames.
    #[test]
    fn test_eval_initial_env() {
        // Compile a program that references variable 'x'
        let program = compile("return x;", CompileOptions::default()).unwrap();
        let (db, _) = TxDB::open(None, DatabaseConfig::default());
        {
            let mut tx = db.new_world_state().unwrap();
            tx.create_object(
                &SYSTEM_OBJECT,
                &NOTHING,
                &SYSTEM_OBJECT,
                BitEnum::all(),
                ObjectKind::NextObjid,
            )
            .unwrap();
            tx.update_property(
                &SYSTEM_OBJECT,
                &SYSTEM_OBJECT,
                Symbol::mk("programmer"),
                &v_int(1),
            )
            .unwrap();
            tx.commit().unwrap();
        }

        let state = db.new_world_state().unwrap();
        let session = Arc::new(NoopClientSession::new());
        let initial_env = [(Symbol::mk("x"), v_int(42))];

        let result = call_eval_builtin_with_env(
            state,
            session,
            BuiltinRegistry::new(),
            SYSTEM_OBJECT,
            program,
            &initial_env,
        );

        assert_eq!(result, Ok(v_int(42)));
    }

    /// Test that multiple initial_env bindings work.
    #[test]
    fn test_eval_initial_env_multiple_vars() {
        let program = compile("return x + y;", CompileOptions::default()).unwrap();
        let (db, _) = TxDB::open(None, DatabaseConfig::default());
        {
            let mut tx = db.new_world_state().unwrap();
            tx.create_object(
                &SYSTEM_OBJECT,
                &NOTHING,
                &SYSTEM_OBJECT,
                BitEnum::all(),
                ObjectKind::NextObjid,
            )
            .unwrap();
            tx.update_property(
                &SYSTEM_OBJECT,
                &SYSTEM_OBJECT,
                Symbol::mk("programmer"),
                &v_int(1),
            )
            .unwrap();
            tx.commit().unwrap();
        }

        let state = db.new_world_state().unwrap();
        let session = Arc::new(NoopClientSession::new());
        let initial_env = [(Symbol::mk("x"), v_int(10)), (Symbol::mk("y"), v_int(32))];

        let result = call_eval_builtin_with_env(
            state,
            session,
            BuiltinRegistry::new(),
            SYSTEM_OBJECT,
            program,
            &initial_env,
        );

        assert_eq!(result, Ok(v_int(42)));
    }

    /// Test that object variables work in initial_env.
    #[test]
    fn test_eval_initial_env_object_var() {
        // Note: can't use "obj" as it's a type constant (OBJ)
        let program = compile("return target;", CompileOptions::default()).unwrap();
        let (db, _) = TxDB::open(None, DatabaseConfig::default());
        {
            let mut tx = db.new_world_state().unwrap();
            tx.create_object(
                &SYSTEM_OBJECT,
                &NOTHING,
                &SYSTEM_OBJECT,
                BitEnum::all(),
                ObjectKind::NextObjid,
            )
            .unwrap();
            tx.update_property(
                &SYSTEM_OBJECT,
                &SYSTEM_OBJECT,
                Symbol::mk("programmer"),
                &v_int(1),
            )
            .unwrap();
            tx.commit().unwrap();
        }

        let state = db.new_world_state().unwrap();
        let session = Arc::new(NoopClientSession::new());
        let initial_env = [(Symbol::mk("target"), v_obj(Obj::mk_id(2)))];

        let result = call_eval_builtin_with_env(
            state,
            session,
            BuiltinRegistry::new(),
            SYSTEM_OBJECT,
            program,
            &initial_env,
        );

        assert_eq!(result, Ok(v_obj(Obj::mk_id(2))));
    }

    /// Test that unused initial_env variables don't cause errors.
    #[test]
    fn test_eval_initial_env_unused_var() {
        let program = compile("return 42;", CompileOptions::default()).unwrap();
        let (db, _) = TxDB::open(None, DatabaseConfig::default());
        {
            let mut tx = db.new_world_state().unwrap();
            tx.create_object(
                &SYSTEM_OBJECT,
                &NOTHING,
                &SYSTEM_OBJECT,
                BitEnum::all(),
                ObjectKind::NextObjid,
            )
            .unwrap();
            tx.update_property(
                &SYSTEM_OBJECT,
                &SYSTEM_OBJECT,
                Symbol::mk("programmer"),
                &v_int(1),
            )
            .unwrap();
            tx.commit().unwrap();
        }

        let state = db.new_world_state().unwrap();
        let session = Arc::new(NoopClientSession::new());
        // Pass a variable that's not referenced in the program
        let initial_env = [(Symbol::mk("unused"), v_int(999))];

        let result = call_eval_builtin_with_env(
            state,
            session,
            BuiltinRegistry::new(),
            SYSTEM_OBJECT,
            program,
            &initial_env,
        );

        assert_eq!(result, Ok(v_int(42)));
    }

    #[test]
    fn test_for_loop_type_error_line_number() {
        // Test that E_TYPE from for loop with invalid sequence reports correct line
        let program = r#"x = 1;
y = 2;
z = "not a list";
for item in (z)
    result = item;
endfor
return 99;"#;

        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());

        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );

        match result {
            Err(exception) => {
                assert_eq!(exception.error, Error::from(E_TYPE));

                // Verify helpful error message for string iteration
                assert!(
                    exception
                        .error
                        .message()
                        .contains("strings are not iterable"),
                    "Expected helpful message about strings, got: {:?}",
                    exception.error
                );

                let last_stack = exception.stack.last().expect("Expected a stack frame");
                let line_no = last_stack
                    .get(&v_int(6), IndexMode::OneBased)
                    .unwrap()
                    .as_integer()
                    .expect("Expected line number to be an integer");
                // Line 4 is "for item in (z)" - the for loop header
                assert_eq!(
                    line_no, 4,
                    "Expected line number to be 4 (for loop), but got {line_no}"
                );
            }
            Ok(_) => {
                panic!("Expected E_TYPE from for loop with string");
            }
        }
    }

    #[test]
    fn test_lambda_capture_same_name_different_scope() {
        // Regression test: lambda declared first, then fork block with same variable names.
        // The lambda's inner variables (val, prop) should NOT be confused with
        // the outer scope's variables in the fork block.
        // Bug: CaptureAnalyzer looked up variables by Symbol name in outer_names,
        // getting the wrong Name tuple when the same name existed at multiple scopes.
        assert_eq!(
            run_moo(
                r#"
                fn get_val(lst)
                    for prop in (lst)
                        val = prop * 2;
                        if (val > 5)
                            return val;
                        endif
                    endfor
                    return 0;
                endfn
                fork (0)
                    val = 99;
                    prop = 88;
                endfork
                return get_val({1, 2, 3, 4});
                "#
            ),
            Ok(v_int(6))
        );
    }

    #[test]
    fn test_flyweight_counter_pattern() {
        // Test flyweight-based counter using self parameter
        // Flyweights are immutable, so each call returns a new flyweight
        // Need to extract the lambda and call it with the flyweight as argument
        let result = run_moo(
            r#"
            let counter = <#1, .count = 0, .inc = fn(self)
                return <self.delegate, .count = self.count + 1, .inc = self.inc>;
            endfn>;
            let inc_fn = counter.inc;
            let c1 = inc_fn(counter);
            let c2 = inc_fn(c1);
            let c3 = inc_fn(c2);
            return {counter.count, c1.count, c2.count, c3.count};
            "#,
        );
        // Expected: {0, 1, 2, 3} - each flyweight has its own count
        assert_eq!(
            result,
            Ok(v_list(&[v_int(0), v_int(1), v_int(2), v_int(3)]))
        );
    }
}
