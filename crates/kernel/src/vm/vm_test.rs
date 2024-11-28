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
    use moor_values::Error::E_DIV;
    use moor_values::{
        v_bool, v_empty_list, v_err, v_int, v_list, v_map, v_none, v_obj, v_objid, v_str, Var,
    };

    use moor_values::NOTHING;
    use moor_values::{AsByteBuffer, SYSTEM_OBJECT};

    use crate::builtins::BuiltinRegistry;
    use crate::tasks::sessions::NoopClientSession;
    use crate::tasks::vm_test_utils::call_verb;
    use moor_compiler::Op;
    use moor_compiler::Op::*;
    use moor_compiler::Program;
    use moor_compiler::{compile, UnboundNames};
    use moor_compiler::{CompileOptions, Names};
    use moor_db_wiredtiger::WiredTigerDB;
    use moor_values::Symbol;
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
    fn test_db_with_verbs(verbs: &[(&str, &Program)]) -> WiredTigerDB {
        let (state, _) = WiredTigerDB::open(None);
        let mut tx = state.new_world_state().unwrap();
        let sysobj = tx
            .create_object(&SYSTEM_OBJECT, &NOTHING, &SYSTEM_OBJECT, BitEnum::all())
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

        // Add $test
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
            let binary = program.make_copy_as_vec().unwrap();
            tx.add_verb(
                &SYSTEM_OBJECT,
                &sysobj.clone(),
                vec![Symbol::mk(verb_name)],
                &sysobj.clone(),
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

    fn test_db_with_verb(verb_name: &str, program: &Program) -> WiredTigerDB {
        test_db_with_verbs(&[(verb_name, program)])
    }

    #[test]
    fn test_simple_vm_execute() {
        let program = mk_program(
            vec![Imm(0.into()), Pop, Done],
            vec![1.into()],
            Names::new(64),
        );
        let state_source = test_db_with_verb("test", &program);
        let mut state = state_source.new_world_state().unwrap();
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state.as_mut(),
            session,
            Arc::new(BuiltinRegistry::new()),
            "test",
            vec![],
        );
        assert_eq!(result, Ok(v_none()));
    }

    #[test]
    fn test_string_value_simple_indexing() {
        let state_source = test_db_with_verb(
            "test",
            &mk_program(
                vec![Imm(0.into()), Imm(1.into()), Ref, Return, Done],
                vec![v_str("hello"), 2.into()],
                Names::new(64),
            ),
        );
        let mut state = state_source.new_world_state().unwrap();
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state.as_mut(),
            session,
            Arc::new(BuiltinRegistry::new()),
            "test",
            vec![],
        );

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
                Names::new(64),
            ),
        )
        .new_world_state()
        .unwrap();
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state.as_mut(),
            session,
            Arc::new(BuiltinRegistry::new()),
            "test",
            vec![],
        );

        assert_eq!(result, Ok(v_str("ell")));
    }

    #[test]
    fn test_list_value_simple_indexing() {
        let mut state = test_db_with_verb(
            "test",
            &mk_program(
                vec![Imm(0.into()), Imm(1.into()), Ref, Return, Done],
                vec![v_list(&[111.into(), 222.into(), 333.into()]), 2.into()],
                Names::new(64),
            ),
        )
        .new_world_state()
        .unwrap();
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state.as_mut(),
            session,
            Arc::new(BuiltinRegistry::new()),
            "test",
            vec![],
        );

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
                Names::new(64),
            ),
        )
        .new_world_state()
        .unwrap();
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state.as_mut(),
            session,
            Arc::new(BuiltinRegistry::new()),
            "test",
            vec![],
        );
        assert_eq!(result, Ok(v_list(&[222.into(), 333.into()])));
    }

    #[test]
    fn test_list_set_range() {
        let mut var_names = UnboundNames::new();
        let a = var_names.find_or_add_name_global("a").unwrap();
        let (var_names, mapping) = var_names.bind();
        let a = mapping[&a];
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
        let result = call_verb(
            state.as_mut(),
            session,
            Arc::new(BuiltinRegistry::new()),
            "test",
            vec![],
        );
        assert_eq!(result, Ok(v_list(&[111.into(), 321.into(), 123.into()])));
    }

    #[test]
    fn test_list_splice() {
        let program = "a = {1,2,3,4,5}; return {@a[2..4]};";
        let binary = compile(program, CompileOptions::default()).unwrap();
        let mut state = test_db_with_verb("test", &binary)
            .new_world_state()
            .unwrap();
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state.as_mut(),
            session,
            Arc::new(BuiltinRegistry::new()),
            "test",
            vec![],
        );
        assert_eq!(result, Ok(v_list(&[2.into(), 3.into(), 4.into()])));
    }

    #[test]
    fn test_if_or_expr() {
        let program = "if (1 || 0) return 1; else return 2; endif";
        let mut state = test_db_with_verb(
            "test",
            &compile(program, CompileOptions::default()).unwrap(),
        )
        .new_world_state()
        .unwrap();
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state.as_mut(),
            session,
            Arc::new(BuiltinRegistry::new()),
            "test",
            vec![],
        );
        assert_eq!(result, Ok(v_int(1)));
    }

    #[test]
    fn test_string_set_range() {
        let mut var_names = UnboundNames::new();
        let a = var_names.find_or_add_name_global("a").unwrap();
        let (var_names, mapping) = var_names.bind();
        let a = mapping[&a];
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

        let result = call_verb(
            state.as_mut(),
            session,
            Arc::new(BuiltinRegistry::new()),
            "test",
            vec![],
        );
        assert_eq!(result, Ok(v_str("manbozorian")));
    }

    #[test]
    fn test_property_retrieval() {
        let mut state = test_db_with_verb(
            "test",
            &mk_program(
                vec![Imm(0.into()), Imm(1.into()), GetProp, Return, Done],
                vec![v_objid(0), v_str("test_prop")],
                Names::new(64),
            ),
        )
        .new_world_state()
        .unwrap();
        {
            state
                .define_property(
                    &SYSTEM_OBJECT,
                    &SYSTEM_OBJECT,
                    &SYSTEM_OBJECT,
                    Symbol::mk("test_prop"),
                    &SYSTEM_OBJECT,
                    BitEnum::new_with(PropFlag::Read) | PropFlag::Write,
                    Some(v_int(666)),
                )
                .unwrap();
        }
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state.as_mut(),
            session,
            Arc::new(BuiltinRegistry::new()),
            "test",
            vec![],
        );
        assert_eq!(result, Ok(v_int(666)));
    }

    #[test]
    fn test_call_verb() {
        // Prepare two, chained, test verbs in our environment, with simple operations.

        // The first merely returns the value "666" immediately.
        let return_verb_binary = mk_program(
            vec![Imm(0.into()), Return, Done],
            vec![v_int(666)],
            Names::new(64),
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
            vec![v_objid(0), v_str("test_return_verb"), v_empty_list()],
            Names::new(64),
        );
        let mut state = test_db_with_verbs(&[
            ("test_return_verb", &return_verb_binary),
            ("test_call_verb", &call_verb_binary),
        ])
        .new_world_state()
        .unwrap();
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state.as_mut(),
            session,
            Arc::new(BuiltinRegistry::new()),
            "test_call_verb",
            vec![],
        );

        assert_eq!(result, Ok(v_int(666)));
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

    #[test]
    fn test_assignment_from_range() {
        let program = "x = 1; y = {1,2,3}; x = x + y[2]; return x;";
        let mut state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state.as_mut(),
            session,
            Arc::new(BuiltinRegistry::new()),
            "test",
            vec![],
        );
        assert_eq!(result, Ok(v_int(3)));
    }

    #[test]
    fn test_while_loop() {
        let program =
            "x = 0; while (x<100) x = x + 1; if (x == 75) break; endif endwhile return x;";
        let mut state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state.as_mut(),
            session,
            Arc::new(BuiltinRegistry::new()),
            "test",
            vec![],
        );
        assert_eq!(result, Ok(v_int(75)));
    }

    #[test]
    fn test_while_labelled_loop() {
        let program = "x = 0; while broken (1) x = x + 1; if (x == 50) break; else continue broken; endif endwhile return x;";
        let mut state = world_with_test_program(program);

        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state.as_mut(),
            session,
            Arc::new(BuiltinRegistry::new()),
            "test",
            vec![],
        );

        assert_eq!(result, Ok(v_int(50)));
    }

    #[test]
    fn test_while_breaks() {
        let program = "x = 0; while (1) x = x + 1; if (x == 50) break; endif endwhile return x;";
        let mut state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state.as_mut(),
            session,
            Arc::new(BuiltinRegistry::new()),
            "test",
            vec![],
        );

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
        let result = call_verb(
            state.as_mut(),
            session,
            Arc::new(BuiltinRegistry::new()),
            "test",
            vec![],
        );

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
        let result = call_verb(
            state.as_mut(),
            session,
            Arc::new(BuiltinRegistry::new()),
            "test",
            vec![],
        );

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
        let result = call_verb(
            state.as_mut(),
            session,
            Arc::new(BuiltinRegistry::new()),
            "test",
            vec![],
        );

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
        let builtin_registry = Arc::new(BuiltinRegistry::new());
        let result = call_verb(
            state.as_mut(),
            session,
            builtin_registry,
            "test",
            vec![v_obj(SYSTEM_OBJECT), v_objid(32)],
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
        let builtin_registry = Arc::new(BuiltinRegistry::new());

        let result = call_verb(
            state.as_mut(),
            session.clone(),
            builtin_registry,
            "test",
            vec![],
        );
        assert_eq!(result, Ok(v_int(5)));
    }

    #[test]
    // "Finally" should not get invoked on exit conditions like return/abort, etc.
    fn test_try_finally_regression_1() {
        let program =
            r#"a = 1; try return "hello world"[2..$]; a = 3; finally a = 2; endtry return a;"#;
        let compiled = compile(program, CompileOptions::default()).unwrap();
        let mut state = world_with_test_programs(&[("test", &compiled)]);
        let session = Arc::new(NoopClientSession::new());
        let builtin_registry = Arc::new(BuiltinRegistry::new());
        let result = call_verb(
            state.as_mut(),
            session.clone(),
            builtin_registry,
            "test",
            vec![],
        );
        assert_eq!(result, Ok(v_str("ello world")));
    }

    #[test]
    // A 0 value was hanging around on the stack making the comparison fail.
    fn test_try_expr_regression() {
        let program = r#"if (E_INVARG == (vi = `verb_info(#-1, "blerg") ! ANY')) return 666; endif return 333;"#;
        let compiled = compile(program, CompileOptions::default()).unwrap();
        let mut state = world_with_test_programs(&[("test", &compiled)]);
        let session = Arc::new(NoopClientSession::new());
        let builtin_registry = Arc::new(BuiltinRegistry::new());
        let result = call_verb(
            state.as_mut(),
            session.clone(),
            builtin_registry,
            "test",
            vec![],
        );
        assert_eq!(result, Ok(v_int(666)));
    }

    /// A VM body that is empty should return v_none() and not panic.
    #[test]
    fn test_regression_zero_body_function() {
        let binary = Program::new();
        let mut state = test_db_with_verb("test", &binary)
            .new_world_state()
            .unwrap();
        let session = Arc::new(NoopClientSession::new());
        let builtin_registry = Arc::new(BuiltinRegistry::new());

        let result = call_verb(
            state.as_mut(),
            session.clone(),
            builtin_registry,
            "test",
            vec![],
        );
        assert_eq!(result, Ok(v_none()));
    }

    #[test]
    fn test_catch_any_regression() {
        let top_of_stack = r#"
            try
                try
                   #1.location:cause_error();
                   return "should not reach here";
                except error (ANY)
                   this:raise_error();
                   return "should not reach here";
                endtry
            except id (ANY)
                return "should reach here";
            endtry
            return "should not reach here";
            "#;
        let bottom_of_stack = r#"raise(E_ARGS);"#;

        let mut state = world_with_test_programs(&[
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
        let builtin_registry = Arc::new(BuiltinRegistry::new());
        let result = call_verb(
            state.as_mut(),
            session.clone(),
            builtin_registry,
            "test",
            vec![],
        );
        assert_eq!(result, Ok(v_str("should reach here")));
    }

    #[test]
    fn test_try_except_str() {
        let program = r#"        try
          return "hello world"[2..$];
        except (E_RANGE)
        endtry"#;
        let compiled = compile(program, CompileOptions::default()).unwrap();
        eprintln!("{}", compiled);
        let mut state = world_with_test_programs(&[("test", &compiled)]);
        let session = Arc::new(NoopClientSession::new());
        let builtin_registry = Arc::new(BuiltinRegistry::new());
        let result = call_verb(
            state.as_mut(),
            session.clone(),
            builtin_registry,
            "test",
            vec![],
        );
        assert_eq!(result, Ok(v_str("ello world")));
    }

    #[test]
    fn test_try_finally_returns() {
        let program = r#"try return 666; finally return 333; endtry"#;
        let compiled = compile(program, CompileOptions::default()).unwrap();
        let mut state = world_with_test_programs(&[("test", &compiled)]);
        let session = Arc::new(NoopClientSession::new());
        let builtin_registry = Arc::new(BuiltinRegistry::new());
        let result = call_verb(
            state.as_mut(),
            session.clone(),
            builtin_registry,
            "test",
            vec![],
        );
        assert_eq!(result, Ok(v_int(333)));
    }

    #[test]
    fn test_lexical_scoping() {
        // Assign a value to a global from a lexically scoped value.
        let program = r#"
        x = 52;
        begin
            let y = 42;
            x = y;
        end
        return x;
        "#;
        let compiled = compile(program, CompileOptions::default()).unwrap();
        let mut state = world_with_test_programs(&[("test", &compiled)]);
        let session = Arc::new(NoopClientSession::new());
        let builtin_registry = Arc::new(BuiltinRegistry::new());
        let result = call_verb(
            state.as_mut(),
            session.clone(),
            builtin_registry,
            "test",
            vec![],
        );
        assert_eq!(result, Ok(v_int(42)));
    }

    #[test]
    fn test_lexical_scoping_shadowing1() {
        // Global with inner scope shadowing it, return value should be the value assigned in the
        // outer (global) scope, since the new lexical scoped value should not be visible.
        let program = r#"
        x = 52;
        begin
            let x = 42;
            x = 1;
        end
        return x;
        "#;
        let compiled = compile(program, CompileOptions::default()).unwrap();
        let mut state = world_with_test_programs(&[("test", &compiled)]);
        let session = Arc::new(NoopClientSession::new());
        let builtin_registry = Arc::new(BuiltinRegistry::new());
        let result = call_verb(
            state.as_mut(),
            session.clone(),
            builtin_registry,
            "test",
            vec![],
        );
        assert_eq!(result, Ok(v_int(52)));
    }

    #[test]
    fn test_lexical_scoping_shadowing2() {
        // Global is set, then shadowed in lexical scope, and returned inside the inner scope,
        // should return the inner scope value.
        let program = r#"
        x = 52;
        begin
            let x = 42;
            let y = 66;
            return {x, y};
        end
        "#;
        let compiled = compile(program, CompileOptions::default()).unwrap();
        let mut state = world_with_test_programs(&[("test", &compiled)]);
        let session = Arc::new(NoopClientSession::new());
        let builtin_registry = Arc::new(BuiltinRegistry::new());
        let result = call_verb(
            state.as_mut(),
            session.clone(),
            builtin_registry,
            "test",
            vec![],
        );
        assert_eq!(result, Ok(v_list(&[v_int(42), v_int(66)])));
    }

    #[test]
    fn test_lexical_scoping_we_must_go_deeper() {
        // Global is set, then shadowed in lexical scope, and returned inside the inner scope,
        // should return the inner scope value.
        let program = r#"
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
        "#;
        let compiled = compile(program, CompileOptions::default()).unwrap();
        let mut state = world_with_test_programs(&[("test", &compiled)]);
        let session = Arc::new(NoopClientSession::new());
        let builtin_registry = Arc::new(BuiltinRegistry::new());
        let result = call_verb(
            state.as_mut(),
            session.clone(),
            builtin_registry,
            "test",
            vec![],
        );
        assert_eq!(result, Ok(v_list(&[v_int(42), v_int(13), v_int(99)])));
    }

    /// Verify that if statements get their own lexical scope, in this case "y" shadowing the
    /// global "y" value.
    #[test]
    fn test_lexical_scoping_in_if_blocks() {
        let program = r#"
        global y = 2;
        let z = 3;
        if (1)
            let y = 5;
            return {y, z};
        else
            return 0;
        endif"#;
        let compiled = compile(program, CompileOptions::default()).unwrap();
        let mut state = world_with_test_programs(&[("test", &compiled)]);
        let session = Arc::new(NoopClientSession::new());
        let builtin_registry = Arc::new(BuiltinRegistry::new());
        let result = call_verb(
            state.as_mut(),
            session.clone(),
            builtin_registry,
            "test",
            vec![],
        );
        assert_eq!(result, Ok(v_list(&[v_int(5), v_int(3)])));
    }

    /// Same as above but for `while`
    #[test]
    fn test_lexical_scoping_in_while_blocks() {
        let program = r#"
        global y = 2;
        let z = 3;
        while (1)
            let y = 5;
            return {y, z};
        endwhile"#;
        let compiled = compile(program, CompileOptions::default()).unwrap();
        let mut state = world_with_test_programs(&[("test", &compiled)]);
        let session = Arc::new(NoopClientSession::new());
        let builtin_registry = Arc::new(BuiltinRegistry::new());
        let result = call_verb(
            state.as_mut(),
            session.clone(),
            builtin_registry,
            "test",
            vec![],
        );
        assert_eq!(result, Ok(v_list(&[v_int(5), v_int(3)])));
    }

    /// And same as above for "for in"
    #[test]
    fn test_lexical_scoping_in_for_blocks() {
        let program = r#"
        global y = 2;
        let z = 3;
        for x in ({1,2,3})
            let y = 5;
            return {y, z};
        endfor"#;
        let compiled = compile(program, CompileOptions::default()).unwrap();
        let mut state = world_with_test_programs(&[("test", &compiled)]);
        let session = Arc::new(NoopClientSession::new());
        let builtin_registry = Arc::new(BuiltinRegistry::new());
        let result = call_verb(
            state.as_mut(),
            session.clone(),
            builtin_registry,
            "test",
            vec![],
        );
        assert_eq!(result, Ok(v_list(&[v_int(5), v_int(3)])));
    }

    /// And for try/except
    #[test]
    fn test_lexical_scoping_in_try_blocks() {
        let program = r#"
        global y = 2;
        let z = 3;
        try
            let y = 5;
            return {y, z};
        except (E_INVARG)
            return 0;
        endtry"#;
        let compiled = compile(program, CompileOptions::default()).unwrap();
        let mut state = world_with_test_programs(&[("test", &compiled)]);
        let session = Arc::new(NoopClientSession::new());
        let builtin_registry = Arc::new(BuiltinRegistry::new());
        let result = call_verb(
            state.as_mut(),
            session.clone(),
            builtin_registry,
            "test",
            vec![],
        );
        assert_eq!(result, Ok(v_list(&[v_int(5), v_int(3)])));
    }

    #[test]
    fn test_const_assign() {
        let program = r#"
        const x = 42;
        return x;
        "#;
        let compiled = compile(program, CompileOptions::default()).unwrap();
        let mut state = world_with_test_programs(&[("test", &compiled)]);
        let session = Arc::new(NoopClientSession::new());
        let builtin_registry = Arc::new(BuiltinRegistry::new());
        let result = call_verb(
            state.as_mut(),
            session.clone(),
            builtin_registry,
            "test",
            vec![],
        );
        assert_eq!(result, Ok(v_int(42)));
    }

    #[test]
    fn test_local_scatter_assign() {
        let program = r#"a = 1;
        begin
            let {a, b} = {2, 3};
            return {a, b};
        end
        "#;
        let compiled = compile(program, CompileOptions::default()).unwrap();
        let mut state = world_with_test_programs(&[("test", &compiled)]);
        let session = Arc::new(NoopClientSession::new());
        let builtin_registry = Arc::new(BuiltinRegistry::new());
        let result = call_verb(
            state.as_mut(),
            session.clone(),
            builtin_registry,
            "test",
            vec![],
        );
        assert_eq!(result, Ok(v_list(&[v_int(2), v_int(3)])));
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
    #[test_case("return -9223372036854775808;", v_int(i64::MIN); "minint")]
    #[test_case("return [ 1 -> 2][1];", v_int(2); "map index")]
    #[test_case("return [ 0 -> 1, 1 -> 2, 2 -> 3, 3 -> 4][1..3];",
        v_map(&[(v_int(1),v_int(2)), (v_int(2),v_int(3))]); "map range")]
    #[test_case(r#"m = [ 1 -> "one", 2 -> "two", 3 -> "three" ]; m[1] = "abc"; return m;"#,
        v_map(&[(v_int(1),v_str("abc")), (v_int(2),v_str("two")), (v_int(3),v_str("three"))]); "map assignment")]
    #[test_case("return [ 0 -> 1, 1 -> 2, 2 -> 3, 3 -> 4][1..$];",
        v_map(&[(v_int(1),v_int(2)), (v_int(2),v_int(3),), (v_int(3),v_int(4))]); "map range to end")]
    #[test_case("l = {1,2,3}; l[2..3] = {6, 7, 8, 9}; return l;",
         v_list(&[v_int(1), v_int(6), v_int(7), v_int(8), v_int(9)]); "list assignment to range")]
    fn test_run(program: &str, expected_result: Var) {
        let mut state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state.as_mut(),
            session,
            Arc::new(BuiltinRegistry::new()),
            "test",
            vec![],
        );
        assert_eq!(result, Ok(expected_result));
    }

    #[test]
    fn test_list_assignment_to_range() {
        let program = r#"l = {1,2,3}; l[2..3] = {6, 7, 8, 9}; return l;"#;
        let mut state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state.as_mut(),
            session,
            Arc::new(BuiltinRegistry::new()),
            "test",
            vec![],
        );
        assert_eq!(
            result,
            Ok(v_list(&[v_int(1), v_int(6), v_int(7), v_int(8), v_int(9)]))
        );
    }
}
