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

#[cfg(test)]
mod tests {

    use std::sync::Arc;

    use moor_values::model::PropFlag;
    use moor_values::model::VerbArgsSpec;
    use moor_values::model::{BinaryType, VerbFlag};
    use moor_values::model::{WorldState, WorldStateSource};
    use moor_values::util::BitEnum;
    use moor_values::var::Error::E_DIV;
    use moor_values::var::Objid;
    use moor_values::var::{
        v_bool, v_empty_list, v_err, v_int, v_list, v_none, v_obj, v_objid, v_str, Var,
    };

    use moor_values::NOTHING;
    use moor_values::{AsByteBuffer, SYSTEM_OBJECT};

    use crate::tasks::sessions::NoopClientSession;
    use crate::tasks::vm_test_utils::call_verb;
    use moor_compiler::compile;
    use moor_compiler::Names;
    use moor_compiler::Op;
    use moor_compiler::Op::*;
    use moor_compiler::Program;
    use moor_db::odb::RelBoxWorldState;
    use test_case::test_case;

    fn mk_program(main_vector: Vec<Op>, literals: Vec<Var>, var_names: Names) -> Program {
        Program {
            literals,
            jump_labels: vec![],
            var_names,
            main_vector: Arc::new(main_vector),
            fork_vectors: vec![],
            line_number_spans: vec![],
        }
    }

    // Create an in memory db with a single object (#0) containing a single provided verb.
    fn test_db_with_verbs(verbs: &[(&str, &Program)]) -> RelBoxWorldState {
        let (state, _) = RelBoxWorldState::open(None, 1 << 30);
        let mut tx = state.new_world_state().unwrap();
        let sysobj = tx
            .create_object(SYSTEM_OBJECT, NOTHING, SYSTEM_OBJECT, BitEnum::all())
            .unwrap();
        tx.update_property(SYSTEM_OBJECT, sysobj, "name", &v_str("system"))
            .unwrap();
        tx.update_property(SYSTEM_OBJECT, sysobj, "programmer", &v_int(1))
            .unwrap();
        tx.update_property(SYSTEM_OBJECT, sysobj, "wizard", &v_int(1))
            .unwrap();

        // Add $test
        tx.define_property(
            SYSTEM_OBJECT,
            sysobj,
            sysobj,
            "test",
            SYSTEM_OBJECT,
            BitEnum::all(),
            Some(v_int(1)),
        )
        .unwrap();

        for (verb_name, program) in verbs {
            let binary = program.make_copy_as_vec().unwrap();
            tx.add_verb(
                SYSTEM_OBJECT,
                sysobj,
                vec![verb_name.to_string()],
                sysobj,
                VerbFlag::rxd(),
                VerbArgsSpec::this_none_this(),
                binary,
                BinaryType::LambdaMoo18X,
            )
            .unwrap();
        }
        tx.commit().unwrap();
        state
    }

    fn test_db_with_verb(verb_name: &str, program: &Program) -> RelBoxWorldState {
        test_db_with_verbs(&[(verb_name, program)])
    }

    #[test]
    fn test_simple_vm_execute() {
        let program = mk_program(vec![Imm(0.into()), Pop, Done], vec![1.into()], Names::new());
        let state_source = test_db_with_verb("test", &program);
        let mut state = state_source.new_world_state().unwrap();
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(state.as_mut(), session, "test", vec![]);
        assert_eq!(result, Ok(v_none()));
    }

    #[test]
    fn test_string_value_simple_indexing() {
        let state_source = test_db_with_verb(
            "test",
            &mk_program(
                vec![Imm(0.into()), Imm(1.into()), Ref, Return, Done],
                vec![v_str("hello"), 2.into()],
                Names::new(),
            ),
        );
        let mut state = state_source.new_world_state().unwrap();
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(state.as_mut(), session, "test", vec![]);

        assert_eq!(result, Ok(v_str("e")));
    }

    #[test]
    fn test_string_value_range_indexing() {
        let mut state = test_db_with_verb(
            "test",
            &mk_program(
                vec![
                    Imm(0.into()),
                    Imm(1.into()),
                    Imm(2.into()),
                    RangeRef,
                    Return,
                    Done,
                ],
                vec![v_str("hello"), 2.into(), 4.into()],
                Names::new(),
            ),
        )
        .new_world_state()
        .unwrap();
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(state.as_mut(), session, "test", vec![]);

        assert_eq!(result, Ok(v_str("ell")));
    }

    #[test]
    fn test_list_value_simple_indexing() {
        let mut state = test_db_with_verb(
            "test",
            &mk_program(
                vec![Imm(0.into()), Imm(1.into()), Ref, Return, Done],
                vec![v_list(&[111.into(), 222.into(), 333.into()]), 2.into()],
                Names::new(),
            ),
        )
        .new_world_state()
        .unwrap();
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(state.as_mut(), session, "test", vec![]);

        assert_eq!(result, Ok(v_int(222)));
    }

    #[test]
    fn test_list_value_range_indexing() {
        let mut state = test_db_with_verb(
            "test",
            &mk_program(
                vec![
                    Imm(0.into()),
                    Imm(1.into()),
                    Imm(2.into()),
                    RangeRef,
                    Return,
                    Done,
                ],
                vec![
                    v_list(&[111.into(), 222.into(), 333.into()]),
                    2.into(),
                    3.into(),
                ],
                Names::new(),
            ),
        )
        .new_world_state()
        .unwrap();
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(state.as_mut(), session, "test", vec![]);
        assert_eq!(result, Ok(v_list(&[222.into(), 333.into()])));
    }

    #[test]
    fn test_list_set_range() {
        let mut var_names = Names::new();
        let a = var_names.find_or_add_name("a");
        let mut state = test_db_with_verb(
            "test",
            &mk_program(
                vec![
                    Imm(0.into()),
                    Put(a),
                    Pop,
                    Push(a),
                    Imm(1.into()),
                    Imm(2.into()),
                    Imm(3.into()),
                    PutTemp,
                    RangeSet,
                    Put(a),
                    Pop,
                    PushTemp,
                    Pop,
                    Push(a),
                    Return,
                    Done,
                ],
                vec![
                    v_list(&[111.into(), 222.into(), 333.into()]),
                    2.into(),
                    3.into(),
                    v_list(&[321.into(), 123.into()]),
                ],
                var_names,
            ),
        )
        .new_world_state()
        .unwrap();
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(state.as_mut(), session, "test", vec![]);
        assert_eq!(result, Ok(v_list(&[111.into(), 321.into(), 123.into()])));
    }

    #[test]
    fn test_list_splice() {
        let program = "a = {1,2,3,4,5}; return {@a[2..4]};";
        let binary = compile(program).unwrap();
        let mut state = test_db_with_verb("test", &binary)
            .new_world_state()
            .unwrap();
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(state.as_mut(), session, "test", vec![]);
        assert_eq!(result, Ok(v_list(&[2.into(), 3.into(), 4.into()])));
    }

    #[test]
    fn test_if_or_expr() {
        let program = "if (1 || 0) return 1; else return 2; endif";
        let mut state = test_db_with_verb("test", &compile(program).unwrap())
            .new_world_state()
            .unwrap();
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(state.as_mut(), session, "test", vec![]);
        assert_eq!(result, Ok(v_int(1)));
    }

    #[test]
    fn test_string_set_range() {
        let mut var_names = Names::new();
        let a = var_names.find_or_add_name("a");

        let mut state = test_db_with_verb(
            "test",
            &mk_program(
                vec![
                    Imm(0.into()),
                    Put(a),
                    Pop,
                    Push(a),
                    Imm(1.into()),
                    Imm(2.into()),
                    Imm(3.into()),
                    PutTemp,
                    RangeSet,
                    Put(a),
                    Pop,
                    PushTemp,
                    Pop,
                    Push(a),
                    Return,
                    Done,
                ],
                vec![v_str("mandalorian"), 4.into(), 7.into(), v_str("bozo")],
                var_names,
            ),
        )
        .new_world_state()
        .unwrap();
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(state.as_mut(), session, "test", vec![]);
        assert_eq!(result, Ok(v_str("manbozorian")));
    }

    #[test]
    fn test_property_retrieval() {
        let mut state = test_db_with_verb(
            "test",
            &mk_program(
                vec![Imm(0.into()), Imm(1.into()), GetProp, Return, Done],
                vec![v_obj(0), v_str("test_prop")],
                Names::new(),
            ),
        )
        .new_world_state()
        .unwrap();
        {
            state
                .define_property(
                    Objid(0),
                    Objid(0),
                    Objid(0),
                    "test_prop",
                    Objid(0),
                    BitEnum::new_with(PropFlag::Read) | PropFlag::Write,
                    Some(v_int(666)),
                )
                .unwrap();
        }
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(state.as_mut(), session, "test", vec![]);
        assert_eq!(result, Ok(v_int(666)));
    }

    #[test]
    fn test_call_verb() {
        // Prepare two, chained, test verbs in our environment, with simple operations.

        // The first merely returns the value "666" immediately.
        let return_verb_binary = mk_program(
            vec![Imm(0.into()), Return, Done],
            vec![v_int(666)],
            Names::new(),
        );

        // The second actually calls the first verb, and returns the result.
        let call_verb_binary = mk_program(
            vec![
                Imm(0.into()), /* obj */
                Imm(1.into()), /* verb */
                Imm(2.into()), /* args */
                CallVerb,
                Return,
                Done,
            ],
            vec![v_obj(0), v_str("test_return_verb"), v_empty_list()],
            Names::new(),
        );
        let mut state = test_db_with_verbs(&[
            ("test_return_verb", &return_verb_binary),
            ("test_call_verb", &call_verb_binary),
        ])
        .new_world_state()
        .unwrap();
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(state.as_mut(), session, "test_call_verb", vec![]);

        assert_eq!(result, Ok(v_int(666)));
    }

    fn world_with_test_program(program: &str) -> Box<dyn WorldState> {
        let binary = compile(program).unwrap();
        test_db_with_verb("test", &binary)
            .new_world_state()
            .unwrap()
    }

    #[test]
    fn test_assignment_from_range() {
        let program = "x = 1; y = {1,2,3}; x = x + y[2]; return x;";
        let mut state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(state.as_mut(), session, "test", vec![]);
        assert_eq!(result, Ok(v_int(3)));
    }

    #[test]
    fn test_while_loop() {
        let program =
            "x = 0; while (x<100) x = x + 1; if (x == 75) break; endif endwhile return x;";
        let mut state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(state.as_mut(), session, "test", vec![]);
        assert_eq!(result, Ok(v_int(75)));
    }

    #[test]
    fn test_while_labelled_loop() {
        let program = "x = 0; while broken (1) x = x + 1; if (x == 50) break; else continue broken; endif endwhile return x;";
        let mut state = world_with_test_program(program);

        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(state.as_mut(), session, "test", vec![]);

        assert_eq!(result, Ok(v_int(50)));
    }

    #[test]
    fn test_while_breaks() {
        let program = "x = 0; while (1) x = x + 1; if (x == 50) break; endif endwhile return x;";
        let mut state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(state.as_mut(), session, "test", vec![]);

        assert_eq!(result, Ok(v_int(50)));
    }

    #[test]
    fn test_while_continue() {
        // Verify that continue works as expected vs break.
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
        let mut state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(state.as_mut(), session, "test", vec![]);

        assert_eq!(result, Ok(v_int(50)));
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
        let mut state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(state.as_mut(), session, "test", vec![]);

        assert_eq!(result, Ok(v_list(&[v_int(3), v_int(2), v_int(1)])));
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
        let mut state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(state.as_mut(), session, "test", vec![]);

        assert_eq!(result, Ok(v_int(6)));
    }

    #[test]
    fn regression_infinite_loop_bf_error() {
        // This ended up in an infinite loop because of faulty error handling coming out of the
        // builtin call when 'what' is not a valid object.
        // The verb is 'd' so E_INVARG should throw exception, exit the loop, but 'twas proceeding.
        let program = r#"{what, targ} = args;
                         try
                           while (what != targ)
                             what = parent(what);
                           endwhile
                           return targ != #-1;
                         except (E_INVARG)
                           return 0;
                         endtry"#;
        let mut state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state.as_mut(),
            session,
            "test",
            vec![v_objid(SYSTEM_OBJECT), v_obj(32)],
        );

        assert_eq!(result, Ok(v_int(0)));
    }

    #[test]
    fn test_regression_catch_issue_23() {
        // https://github.com//moor/issues/23
        let program =
            r#"try 5; except error (E_RANGE) return 1; endtry for x in [1..1] return 5; endfor"#;
        let mut state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(state.as_mut(), session.clone(), "test", vec![]);
        assert_eq!(result, Ok(v_int(5)));
    }

    /// A VM body that is empty should return v_none() and not panic.
    #[test]
    fn test_regression_zero_body_function() {
        let binary = Program {
            literals: vec![],
            jump_labels: vec![],
            var_names: Names::default(),
            main_vector: Arc::new(vec![]),
            fork_vectors: vec![],
            line_number_spans: vec![],
        };
        let mut state = test_db_with_verb("test", &binary)
            .new_world_state()
            .unwrap();
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(state.as_mut(), session.clone(), "test", vec![]);
        assert_eq!(result, Ok(v_none()));
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
            v_list(&[v_int(1), v_list(&[v_bool(true), v_int(1)])]); "eval builtin")]
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
        v_obj(2);
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
    #[test_case("{a,b,c} = {{1,2,3}}[1]; return {a,b,c};" , 
        v_list(&[v_int(1), v_int(2), v_int(3)]); "tuple/splice assignment")]
    #[test_case("return {{1,2,3}[2..$], {1}[$]};", 
        v_list(&[
            v_list(&[v_int(2), v_int(3)]), v_int(1)]);
        "range to end retrieval")]
    #[test_case( "{a,b,@c}= {1,2,3,4,5}; return c;",
        v_list(&[v_int(3), v_int(4), v_int(5)]); "new scatter regression")]
    #[test_case("{?a, ?b, ?c, ?d = a, @remain} = {1, 2, 3}; return {d, c, b, a, remain};" , 
        v_list(&[v_int(1), v_int(3), v_int(2), v_int(1), v_empty_list()]); "complicated scatter")]
    #[test_case("{a, b, @c} = {1, 2, 3, 4}; {x, @y, ?z} = {5,6,7,8}; return {a,b,c,x,y,z};" , 
        v_list(&[
            v_int(1),
            v_int(2),
            v_list(&[v_int(3), v_int(4)]),
            v_int(5),
            v_list(&[v_int(6), v_int(7)]),
            v_int(8),
        ]); "scatter complex 2")]
    #[test_case("{a, b, c, ?d = 4} = {1, 2, 3}; return {d, c, b, a};" , 
        v_list(&[v_int(4), v_int(3), v_int(2), v_int(1)]); "scatter optional")]
    #[test_case("z = 0; for i in [1..4] z = z + i; endfor return {i,z};" , 
        v_list(&[v_int(4), v_int(10)]); "for range loop")]
    #[test_case("x = {1,2,3,4}; z = 0; for i in (x) z = z + i; endfor return {i,z};" , 
        v_list(&[v_int(4), v_int(10)]); "for list loop")]
    #[test_case(r#"if (E_INVARG == (vi = `verb_info(#-1, "blerg") ! ANY')) return 666; endif return 333;"#, 
        v_int(666); "verb_info invalid object error")]
    fn test_run(program: &str, expected_result: Var) {
        let mut state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(state.as_mut(), session, "test", vec![]);
        assert_eq!(result, Ok(expected_result));
    }
}
