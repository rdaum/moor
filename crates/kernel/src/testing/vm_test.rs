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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use moor_common::{
        model::{ObjectKind, PropFlag, VerbArgsSpec, VerbFlag, WorldState, WorldStateSource},
        util::BitEnum,
    };
    use moor_var::{
        List, Obj, Var, v_empty_list, v_err, v_flyweight, v_int, v_list, v_map, v_obj, v_objid,
        v_str,
    };

    use moor_var::{NOTHING, SYSTEM_OBJECT, *};

    use crate::{testing::vm_test_utils::call_verb, vm::builtins::BuiltinRegistry};
    use moor_common::tasks::NoopClientSession;
    use moor_compiler::{CompileOptions, Names, Op, Op::*, Program, compile};
    use moor_db::{DatabaseConfig, TxDB};
    use moor_var::{
        Symbol,
        program::{ProgramType, program::PrgInner},
    };
    use test_case::test_case;

    fn mk_program(main_vector: Vec<Op>, literals: Vec<Var>, var_names: Names) -> Program {
        Program(Arc::new(PrgInner {
            literals,
            jump_labels: vec![],
            var_names,
            scatter_tables: vec![],
            for_sequence_operands: vec![],
            for_range_operands: vec![],
            range_comprehensions: vec![],
            list_comprehensions: vec![],
            error_operands: vec![],
            lambda_programs: vec![],
            main_vector,
            fork_vectors: vec![],
            line_number_spans: vec![],
            fork_line_number_spans: vec![],
        }))
    }

    // Create an in memory db with a single object (#0) containing a single provided verb.
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

    #[test]
    fn test_simple_vm_execute() {
        let program = mk_program(
            vec![Imm(0.into()), Pop, Done],
            vec![1.into()],
            Names::new(64),
        );
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
        assert_eq!(result, Ok(v_bool(false)));
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
        let state = state_source.new_world_state().unwrap();
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );

        assert_eq!(result, Ok(v_str("e")));
    }

    #[test]
    fn test_string_value_range_indexing() {
        let state = test_db_with_verb(
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
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );

        assert_eq!(result, Ok(v_str("ell")));
    }

    #[test]
    fn test_list_value_simple_indexing() {
        let state = test_db_with_verb(
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
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );

        assert_eq!(result, Ok(v_int(222)));
    }

    #[test]
    fn test_list_value_range_indexing() {
        let state = test_db_with_verb(
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
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result, Ok(v_list(&[222.into(), 333.into()])));
    }

    #[test]
    fn test_list_splice() {
        let program = "a = {1,2,3,4,5}; return {@a[2..4]};";
        let binary = compile(program, CompileOptions::default()).unwrap();
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
        assert_eq!(result, Ok(v_list(&[2.into(), 3.into(), 4.into()])));
    }

    #[test]
    fn test_if_or_expr() {
        let program = "if (1 || 0) return 1; else return 2; endif";
        let state = test_db_with_verb(
            "test",
            &compile(program, CompileOptions::default()).unwrap(),
        )
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
        assert_eq!(result, Ok(v_int(1)));
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
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
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
        let state = test_db_with_verbs(&[
            ("test_return_verb", &return_verb_binary),
            ("test_call_verb", &call_verb_binary),
        ])
        .new_world_state()
        .unwrap();
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test_call_verb",
            List::mk_list(&[]),
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
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result, Ok(v_int(3)));
    }

    #[test]
    fn test_while_loop() {
        let program =
            "x = 0; while (x<100) x = x + 1; if (x == 75) break; endif endwhile return x;";
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result, Ok(v_int(75)));
    }

    #[test]
    fn test_while_labelled_loop() {
        let program = "x = 0; while broken (1) x = x + 1; if (x == 50) break; else continue broken; endif endwhile return x;";
        let state = world_with_test_program(program);

        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );

        assert_eq!(result, Ok(v_int(50)));
    }

    #[test]
    fn test_while_breaks() {
        let program = "x = 0; while (1) x = x + 1; if (x == 50) break; endif endwhile return x;";
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
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
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
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
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
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
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
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
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let builtin_registry = BuiltinRegistry::new();
        let result = call_verb(
            state,
            session,
            builtin_registry,
            "test",
            List::mk_list(&[v_obj(SYSTEM_OBJECT), v_objid(32)]),
        );

        assert_eq!(result, Ok(v_int(0)));
    }

    #[test]
    fn test_regression_catch_issue_23() {
        // https://github.com//moor/issues/23
        let program =
            r#"try 5; except error (E_RANGE) return 1; endtry for x in [1..1] return 5; endfor"#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let builtin_registry = BuiltinRegistry::new();

        let result = call_verb(
            state,
            session.clone(),
            builtin_registry,
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result, Ok(v_int(5)));
    }

    #[test]
    // "Finally" should not get invoked on exit conditions like return/abort, etc.
    fn test_try_finally_regression_1() {
        let program =
            r#"a = 1; try return "hello world"[2..$]; a = 3; finally a = 2; endtry return a;"#;
        let compiled = compile(program, CompileOptions::default()).unwrap();
        let state = world_with_test_programs(&[("test", &compiled)]);
        let session = Arc::new(NoopClientSession::new());
        let builtin_registry = BuiltinRegistry::new();
        let result = call_verb(
            state,
            session.clone(),
            builtin_registry,
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result, Ok(v_str("ello world")));
    }

    #[test]
    // A 0 value was hanging around on the stack making the comparison fail.
    fn test_try_expr_regression() {
        let program = r#"if (E_INVARG == (vi = `verb_info(#-1, "blerg") ! ANY')) return 666; endif return 333;"#;
        let compiled = compile(program, CompileOptions::default()).unwrap();
        let state = world_with_test_programs(&[("test", &compiled)]);
        let session = Arc::new(NoopClientSession::new());
        let builtin_registry = BuiltinRegistry::new();
        let result = call_verb(
            state,
            session.clone(),
            builtin_registry,
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result, Ok(v_int(666)));
    }

    /// A VM body that is empty should return v_none() and not panic.
    #[test]
    fn test_regression_zero_body_function() {
        let binary = Program::new();
        let state = test_db_with_verb("test", &binary)
            .new_world_state()
            .unwrap();
        let session = Arc::new(NoopClientSession::new());
        let builtin_registry = BuiltinRegistry::new();

        let result = call_verb(
            state,
            session.clone(),
            builtin_registry,
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result, Ok(v_bool(false)));
    }

    #[test]
    fn test_catch_any_regression() {
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
        let builtin_registry = BuiltinRegistry::new();
        let result = call_verb(
            state,
            session.clone(),
            builtin_registry,
            "test",
            List::mk_list(&[]),
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
        let state = world_with_test_programs(&[("test", &compiled)]);
        let session = Arc::new(NoopClientSession::new());
        let builtin_registry = BuiltinRegistry::new();
        let result = call_verb(
            state,
            session.clone(),
            builtin_registry,
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result, Ok(v_str("ello world")));
    }

    #[test]
    fn test_try_finally_returns() {
        let program = r#"try return 666; finally return 333; endtry"#;
        let compiled = compile(program, CompileOptions::default()).unwrap();
        let state = world_with_test_programs(&[("test", &compiled)]);
        let session = Arc::new(NoopClientSession::new());
        let builtin_registry = BuiltinRegistry::new();
        let result = call_verb(
            state,
            session.clone(),
            builtin_registry,
            "test",
            List::mk_list(&[]),
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
        let state = world_with_test_programs(&[("test", &compiled)]);
        let session = Arc::new(NoopClientSession::new());
        let builtin_registry = BuiltinRegistry::new();
        let result = call_verb(
            state,
            session.clone(),
            builtin_registry,
            "test",
            List::mk_list(&[]),
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
        let state = world_with_test_programs(&[("test", &compiled)]);
        let session = Arc::new(NoopClientSession::new());
        let builtin_registry = BuiltinRegistry::new();
        let result = call_verb(
            state,
            session.clone(),
            builtin_registry,
            "test",
            List::mk_list(&[]),
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
        let state = world_with_test_programs(&[("test", &compiled)]);
        let session = Arc::new(NoopClientSession::new());
        let builtin_registry = BuiltinRegistry::new();
        let result = call_verb(
            state,
            session.clone(),
            builtin_registry,
            "test",
            List::mk_list(&[]),
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
        let state = world_with_test_programs(&[("test", &compiled)]);
        let session = Arc::new(NoopClientSession::new());
        let builtin_registry = BuiltinRegistry::new();
        let result = call_verb(
            state,
            session.clone(),
            builtin_registry,
            "test",
            List::mk_list(&[]),
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
        let state = world_with_test_programs(&[("test", &compiled)]);
        let session = Arc::new(NoopClientSession::new());
        let builtin_registry = BuiltinRegistry::new();
        let result = call_verb(
            state,
            session.clone(),
            builtin_registry,
            "test",
            List::mk_list(&[]),
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
        let state = world_with_test_programs(&[("test", &compiled)]);
        let session = Arc::new(NoopClientSession::new());
        let builtin_registry = BuiltinRegistry::new();
        let result = call_verb(
            state,
            session.clone(),
            builtin_registry,
            "test",
            List::mk_list(&[]),
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
        let state = world_with_test_programs(&[("test", &compiled)]);
        let session = Arc::new(NoopClientSession::new());
        let builtin_registry = BuiltinRegistry::new();
        let result = call_verb(
            state,
            session.clone(),
            builtin_registry,
            "test",
            List::mk_list(&[]),
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
        let state = world_with_test_programs(&[("test", &compiled)]);
        let session = Arc::new(NoopClientSession::new());
        let builtin_registry = BuiltinRegistry::new();
        let result = call_verb(
            state,
            session.clone(),
            builtin_registry,
            "test",
            List::mk_list(&[]),
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
        let state = world_with_test_programs(&[("test", &compiled)]);
        let session = Arc::new(NoopClientSession::new());
        let builtin_registry = BuiltinRegistry::new();
        let result = call_verb(
            state,
            session.clone(),
            builtin_registry,
            "test",
            List::mk_list(&[]),
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
        let state = world_with_test_programs(&[("test", &compiled)]);
        let session = Arc::new(NoopClientSession::new());
        let builtin_registry = BuiltinRegistry::new();
        let result = call_verb(
            state,
            session.clone(),
            builtin_registry,
            "test",
            List::mk_list(&[]),
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
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result, Ok(expected_result));
    }

    #[test]
    fn test_list_assignment_to_range() {
        let program = r#"l = {1,2,3}; l[2..3] = {6, 7, 8, 9}; return l;"#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(
            result,
            Ok(v_list(&[v_int(1), v_int(6), v_int(7), v_int(8), v_int(9)]))
        );
    }

    #[test]
    fn test_make_flyweight() {
        let program = r#"return <#1, .slot = "123", {1, 2, 3}>;"#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(
            result.unwrap(),
            v_flyweight(
                Obj::mk_id(1),
                &[(Symbol::mk("slot"), v_str("123"))],
                List::mk_list(&[v_int(1), v_int(2), v_int(3)]),
            )
        );
    }

    #[test]
    fn test_flyweight_slot() {
        let program = r#"return <#1, .slot = "123", {1, 2, 3}>.slot;"#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result.unwrap(), v_str("123"));
    }

    /// Test the test of builtins for slots
    #[test]
    fn test_flyweight_builtins() {
        let program = r#"let a = <#1, .slot = "123", {1, 2, 3}>;
        let b = flyslotremove(a, 'slot);
        let c = flyslotset(b, 'bananas, "456");
        return {c, flyslots(c)};"#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(
            result.unwrap(),
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
        let program = r#"
            let fw = <#1, .slot = "123", {1, 2, 3}>;
            return flycontents(fw)[2];
        "#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result.unwrap(), v_int(2));
    }

    /// Bug where stack offsets were wrong in maps, causing problems with the $ range operation
    #[test]
    fn test_range_in_map_oddities() {
        let program = r#"return[ "z"->5, "b"->"another_seq"[1..$]]["b"];"#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result.unwrap(), v_str("another_seq"));
    }

    #[test]
    fn test_range_flyweight_oddities() {
        let program = r#"
            let fw = <#1, .another_slot = 5, .slot = "123", {"another_seq"[1..$]}>;
            return flycontents(fw)[1];
        "#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result.unwrap(), v_str("another_seq"));
    }

    /// Regression test for issue #482 - ForRange with $ operator crashes
    #[test]
    fn test_for_range_length_dollar_regression() {
        let program = r#"
            for i in [1..3]
                return "hello"[1..$];
            endfor
        "#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result.unwrap(), v_str("hello"));
    }

    /// Regression test for issue #482 - ForSequence with $ operator crashes  
    #[test]
    fn test_for_sequence_length_dollar_regression() {
        let program = r#"
            for i in ({"a", "b", "c"})
                return "hello"[1..$];
            endfor
        "#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result.unwrap(), v_str("hello"));
    }

    #[test]
    fn test_for_range_comprehension() {
        let program = r#"return { x * 2 for x in [1..3] };"#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result.unwrap(), v_list(&[v_int(2), v_int(4), v_int(6)]));
    }

    #[test]
    fn test_for_list_comprehension() {
        let program = r#"return { x * 2 for x in ({1,2,3}) };"#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result.unwrap(), v_list(&[v_int(2), v_int(4), v_int(6)]));
    }

    #[test]
    fn test_for_list_comprehension_scope_regression() {
        let program = r#"
            let x = {1,2,3};
            if (false)
                y = {v * 2 for v in (x)};
            endif
            let z = 1;
            if (false)
            endif
            return z;
        "#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result.unwrap(), v_int(1));
    }

    #[test]
    fn test_for_v_k_in_map() {
        let program = r#"
        let result = {};
        for v, k in (["a" -> "b", "c" -> "d"])
            result = {@result, @{k, v}};
        endfor
        return result;
        "#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(
            result.unwrap(),
            v_list(&[v_str("a"), v_str("b"), v_str("c"), v_str("d")])
        );
    }

    #[test]
    fn test_for_v_k_in_list() {
        let program = r#"
        let result = {};
        for v, k in ({"a", "b"})
            result = {@result, @{k, v}};
        endfor
        return result;
        "#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(
            result.unwrap(),
            v_list(&[v_int(1), v_str("a"), v_int(2), v_str("b")])
        );
    }

    #[test]
    fn test_scope_width_regression() {
        let program = r#"
        let x = 1;
        for i in [0..1024]
            let y = 2 * i;
        endfor
        return 0;
        "#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result.unwrap(), v_int(0));
    }

    #[test]
    fn test_regress_except() {
        //    #[test_case("return {`x ! e_varnf => 666', `321 ! e_verbnf => 123'};",
        //         v_list(&[v_int(666), v_int(321)]); "catch expr 2")]
        let program = r#"
        return {`x ! e_varnf => 666', `321 ! e_verbnf => 123'};
        "#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result.unwrap(), v_list(&[v_int(666), v_int(321)]));
    }

    #[test]
    fn test_simple_fork() {
        let program = r#"
        fork (0)
            return 42;
        endfork
        return 24;
        "#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result.unwrap(), v_int(24));
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

        // First run the normal way to trigger the fork execution
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );

        // The main verb returns 99, but the fork should have executed and raised an error
        // Since our vm_test_utils now handles forks sequentially, the error from the fork
        // should propagate back to us
        match result {
            Err(exception) => {
                // We should get the exception from the fork
                assert_eq!(exception.error, Error::from(E_ARGS));
                // Check that we have backtrace information & stack information
                assert!(!exception.backtrace.is_empty());
                assert!(!exception.stack.is_empty());

                // Line no should be the 6th element of the last "list" in `stack`.
                let last_stack = exception.stack.last().expect("Expected a stack frame");
                let line_no = last_stack
                    .get(&v_int(6), IndexMode::OneBased)
                    .unwrap()
                    .as_integer()
                    .expect("Expected line number to be an integer");
                // The line number should be 5, which is where the error was raised in the fork block
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
        // Test that line numbers are correct when multiple forks are present
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

        // Test the first fork (should error on line 4)
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

        // Reset state for second fork test
        let mut _state = world_with_test_program(program);
        let _session = Arc::new(NoopClientSession::new());

        // We need to modify the program to test the second fork
        // Since the first fork will execute first, let's create a version where the first fork succeeds
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

        // The main thread should return 99, but we should also get a fork error eventually
        // For now, let's verify the main thread behavior
        match result {
            Err(exception) => {
                // Check if this is from the second fork
                if exception.error == E_PERM {
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
            Ok(_) => {
                // Main thread completed successfully, fork errors may be handled separately
            }
        }
    }

    #[test]
    fn test_nested_fork_line_numbers() {
        // Test line numbers with nested forks (fork within fork)
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
        // Test that line numbers are correct even when forks appear later in the program
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
        // Test that lambda compilation and MakeLambda opcode work
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

        // The result should be a lambda value
        assert!(
            result.as_lambda().is_some(),
            "Expected lambda value, got: {result:?}",
        );

        let lambda = result.as_lambda().unwrap();

        // Verify lambda has correct parameter structure
        assert_eq!(lambda.0.params.labels.len(), 1, "Expected 1 parameter");

        // Verify parameter is required type (not optional or rest)
        match &lambda.0.params.labels[0] {
            moor_var::program::opcode::ScatterLabel::Required(_) => {
                // This is what we expect
            }
            other => panic!("Expected Required parameter, got: {other:?}"),
        }

        // Test that the lambda can be converted back to literal form
        let literal_form = moor_compiler::to_literal(&result);
        assert!(
            literal_form.contains("{x} => x + 1"),
            "Lambda literal should contain correct syntax, got: {literal_form}",
        );
    }

    #[test]
    fn test_lambda_with_multiple_params() {
        // Test lambda with multiple parameter types
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

        // Verify parameter types
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

    // Lambda call tests for our new lambda execution implementation
    #[test]
    fn test_lambda_simple_call() {
        let program_text = r#"
            let add = {x, y} => x + y;
            return add(5, 3);
        "#;

        let state = world_with_test_program(program_text);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result, Ok(v_int(8)));
    }

    #[test]
    fn test_lambda_single_parameter() {
        let program_text = r#"
            let double = {x} => x * 2;
            return double(7);
        "#;

        let state = world_with_test_program(program_text);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result, Ok(v_int(14)));
    }

    #[test]
    fn test_lambda_no_parameters() {
        let program_text = r#"
            let hello = {} => "Hello, World!";
            return hello();
        "#;

        let state = world_with_test_program(program_text);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result, Ok(v_str("Hello, World!")));
    }

    #[test]
    fn test_lambda_optional_parameters() {
        let program_text = r#"
            let greet = {name, ?greeting} => (greeting || "Hello") + ", " + name + "!";
            let result1 = greet("Alice");
            let result2 = greet("Bob", "Hi");
            return {result1, result2};
        "#;

        let state = world_with_test_program(program_text);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        let expected = v_list(&[v_str("Hello, Alice!"), v_str("Hi, Bob!")]);
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn test_lambda_rest_parameters() {
        let program_text = r#"
            let sum = fn(@numbers)
                let total = 0;
                for n in (numbers)
                    total = total + n;
                endfor
                return total;
            endfn;
            return sum(1, 2, 3, 4, 5);
        "#;

        let state = world_with_test_program(program_text);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result, Ok(v_int(15)));
    }

    #[test]
    fn test_lambda_mixed_parameters() {
        let program_text = r#"
            let func = fn(required, ?optional, @rest)
                return {required, optional || 0, length(rest)};
            endfn;
            return func(42, 100, "a", "b", "c");
        "#;

        let state = world_with_test_program(program_text);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        let expected = v_list(&[v_int(42), v_int(100), v_int(3)]);
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn test_lambda_closure_capture() {
        let program_text = r#"
            let x = 10;
            let y = 20;
            let adder = {z} => x + y + z;
            return adder(5);
        "#;

        let state = world_with_test_program(program_text);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result, Ok(v_int(35))); // 10 + 20 + 5
    }

    #[test]
    fn test_lambda_nested_scope_capture() {
        let program_text = r#"
            let make_multiplier = fn(factor)
                return {x} => x * factor;
            endfn;
            let times3 = make_multiplier(3);
            return times3(7);
        "#;

        let state = world_with_test_program(program_text);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result, Ok(v_int(21))); // 7 * 3
    }

    #[test]
    fn test_lambda_double_recursive_call() {
        // Test lambda with two recursive calls (like fibonacci but simpler)
        let program_text = r#"
            fn test(x)
                return test(1) + test(2);
            endfn
            return 1;
        "#;

        let state = world_with_test_program(program_text);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result, Ok(v_int(1)));
    }

    #[test]
    fn test_lambda_recursive_fibonacci() {
        let program_text = r#"
            fn fib(n)
                if (n <= 1)
                    return n;
                else
                    return fib(n - 1) + fib(n - 2);
                endif
            endfn
            return fib(6);
        "#;

        let state = world_with_test_program(program_text);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result, Ok(v_int(8))); // fib(6) = 8
    }

    #[test]
    fn test_lambda_higher_order_map() {
        let program_text = r#"
            let map = fn(func, lst)
                let result = {};
                for item in (lst)
                    result = {@result, func(item)};
                endfor
                return result;
            endfn;
            let square = {x} => x * x;
            return map(square, {1, 2, 3, 4});
        "#;

        let state = world_with_test_program(program_text);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        let expected = v_list(&[v_int(1), v_int(4), v_int(9), v_int(16)]);
        assert_eq!(result, Ok(expected));
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

        // Should return an E_ARGS error
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

        // Should return an E_ARGS error
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

        // Should return an E_TYPE error
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

        // Should return an E_DIV error with proper stack trace format
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.error, E_DIV);

        // Verify the stack trace contains the lambda format
        // The lambda frame should show "test.<%fn>" in the traceback
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
        // Simple test to check lambda line number tracking without complex assertions
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

        // Should get a division by zero error
        assert!(result.is_err());
        let err = result.unwrap_err();

        // Verify we get the lambda format
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
        // Test that named recursive lambdas show the function name in stack traces
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

        // Should return an E_DIV error with proper stack trace format
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.error, E_DIV);

        // Verify the stack trace contains the function name "factorial"
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
        // Test that pure lambdas (no variable references) have empty captured environments
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

        // Pure lambda should have empty captured environment
        assert!(
            lambda.0.captured_env.is_empty(),
            "Pure lambda should have empty captured environment, got: {:?}",
            lambda.0.captured_env
        );
    }

    #[test]
    fn test_lambda_capture_with_outer_variable() {
        // Test that lambdas referencing outer variables capture them correctly
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

        // Lambda should have non-empty captured environment since it references 'multiplier'
        assert!(
            !lambda.0.captured_env.is_empty(),
            "Lambda should capture outer variable 'multiplier'"
        );
    }

    #[test]
    fn test_lambda_capture_multiple_variables() {
        // Test lambda capturing multiple outer variables
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

        // Lambda should capture variables since it references outer variables a and b
        assert!(
            !lambda.0.captured_env.is_empty(),
            "Lambda should capture outer variables 'a' and 'b'"
        );
    }

    #[test]
    fn test_lambda_capture_functionality() {
        // Test that capture works correctly for execution
        let program_text = r#"
            let base = 100;
            let f = {x} => x + base;
            return f(5);
        "#;

        let state = world_with_test_program(program_text);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );

        // Should correctly access captured variable
        assert_eq!(result, Ok(v_int(105))); // 5 + 100
    }

    #[test]
    fn test_lambda_capture_creation_only() {
        // Test pure lambda creation without calling it to isolate capture analysis
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

        // Lambda should have empty captured environment since it doesn't reference outer_var
        assert!(
            lambda.0.captured_env.is_empty(),
            "Lambda should have empty captured environment since it doesn't reference outer_var"
        );
    }

    #[test]
    fn test_for_loop_continue_regression() {
        // Test the failing continue case from looping.moot
        let program = r#"x = {}; for i in ({1, 2, 3, 4, 5}); if (i < 3); continue; endif; x = {@x, i}; endfor; return x;"#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        // Should return {3, 4, 5} - only values >= 3 should be added to x
        assert_eq!(result, Ok(v_list(&[v_int(3), v_int(4), v_int(5)])));
    }

    #[test]
    fn test_for_range_continue_regression() {
        // Test continue in for-range loops
        let program = r#"x = {}; for i in [1..5]; if (i < 3); continue; endif; x = {@x, i}; endfor; return x;"#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        // Should return {3, 4, 5} - only values >= 3 should be added to x
        assert_eq!(result, Ok(v_list(&[v_int(3), v_int(4), v_int(5)])));
    }

    #[test]
    fn test_for_range_with_objects() {
        // Test object range iteration
        let program = r#"x = {}; for o in [#1..#5]; x = {@x, o}; endfor; return x;"#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );

        // Should return {#1, #2, #3, #4, #5}
        assert_eq!(
            result,
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
        // Test for-range loops with floats
        let program = r#"x = {}; for f in [1.0..3.0]; x = {@x, f}; endfor; return x;"#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        // Should return {1.0, 2.0, 3.0}
        assert_eq!(
            result,
            Ok(v_list(&[v_float(1.0), v_float(2.0), v_float(3.0)]))
        );
    }

    #[test]
    fn test_for_range_type_mismatch() {
        // Test that mixing types in for-range is an error
        let program = r#"for x in [1..#5]; return x; endfor"#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        // Should throw E_TYPE exception
        assert!(matches!(result, Err(e) if e.error == E_TYPE));
    }

    #[test]
    fn test_scatter_multiple_optionals_regression() {
        // Regression test for scatter assignment with multiple optional parameters
        // Bug: When there are multiple optionals and not all values are provided,
        // the defaults are being incorrectly applied to parameters that DO have values

        // Test case 1: Two values provided for {a, ?b = {"1"}, ?c = 0}
        // The bug incorrectly gives b its default {"1"} instead of the provided "b"
        let program1 = r#"
            args = {"a", "b"};
            {a, ?b = {"1"}, ?c = 0} = args;
            return {a, b, c};
        "#;
        let state1 = world_with_test_program(program1);
        let session1 = Arc::new(NoopClientSession::new());
        let result1 = call_verb(
            state1,
            session1,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        // Should be {"a", "b", 0} but bug gives {"a", {"1"}, 0}
        assert_eq!(result1, Ok(v_list(&[v_str("a"), v_str("b"), v_int(0)])));

        // Test case 2: Three values provided, should fill both optionals correctly
        // This case works correctly
        let program2 = r#"
            args = {"a", "b", "c"};
            {a, ?b = {"1"}, ?c = 0} = args;
            return {a, b, c};
        "#;
        let state2 = world_with_test_program(program2);
        let session2 = Arc::new(NoopClientSession::new());
        let result2 = call_verb(
            state2,
            session2,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        // This works correctly: {"a", "b", "c"}
        assert_eq!(result2, Ok(v_list(&[v_str("a"), v_str("b"), v_str("c")])));
    }

    #[test]
    fn test_nested_loop_unlabeled_continue() {
        // Test that unlabeled continue works in nested loops (should continue inner loop)
        let program = r#"
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
        "#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        // Unlabeled continue should only skip {x,6} but still add 999 markers
        let expected = v_list(&[
            v_int(1),
            v_int(5),
            v_int(999),
            v_int(2),
            v_int(5),
            v_int(999),
        ]);
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn test_nested_loop_labeled_continue() {
        // Test nested loops with labeled continue to outer loop
        let program = r#"
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
        "#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session,
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        // When y == 6, we continue x, so we should skip adding {x,6} and the 999
        // Expected: for x=1: {1,5}; for x=2: {2,5}
        // The 999 markers should be skipped due to continue x
        let expected = v_list(&[v_int(1), v_int(5), v_int(2), v_int(5)]);
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn test_scatter_multiple_optionals_comprehensive() {
        // Test with 3 optionals: {a, ?b = "default_b", ?c = "default_c", ?d = "default_d"}

        // Case 1: All values provided
        let program = r#"
            {a, ?b = "default_b", ?c = "default_c", ?d = "default_d"} = {"A", "B", "C", "D"};
            return {a, b, c, d};
        "#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session.clone(),
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        let expected = v_list(&[v_str("A"), v_str("B"), v_str("C"), v_str("D")]);
        assert_eq!(result, Ok(expected));

        // Case 2: Missing last optional (d)
        let program = r#"
            {a, ?b = "default_b", ?c = "default_c", ?d = "default_d"} = {"A", "B", "C"};
            return {a, b, c, d};
        "#;
        let state = world_with_test_program(program);
        let result = call_verb(
            state,
            session.clone(),
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        let expected = v_list(&[v_str("A"), v_str("B"), v_str("C"), v_str("default_d")]);
        assert_eq!(result, Ok(expected));

        // Case 3: Missing last two optionals (c, d)
        let program = r#"
            {a, ?b = "default_b", ?c = "default_c", ?d = "default_d"} = {"A", "B"};
            return {a, b, c, d};
        "#;
        let state = world_with_test_program(program);
        let result = call_verb(
            state,
            session.clone(),
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        let expected = v_list(&[
            v_str("A"),
            v_str("B"),
            v_str("default_c"),
            v_str("default_d"),
        ]);
        assert_eq!(result, Ok(expected));

        // Case 4: Missing all optionals (only required 'a' provided)
        let program = r#"
            {a, ?b = "default_b", ?c = "default_c", ?d = "default_d"} = {"A"};
            return {a, b, c, d};
        "#;
        let state = world_with_test_program(program);
        let result = call_verb(
            state,
            session.clone(),
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        let expected = v_list(&[
            v_str("A"),
            v_str("default_b"),
            v_str("default_c"),
            v_str("default_d"),
        ]);
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn test_scatter_five_optionals_edge_cases() {
        // Test with 5 optionals to really stress test the system

        // Case 1: Provide first 3, missing last 2
        let program = r#"
            {a, ?b = 1, ?c = 2, ?d = 3, ?e = 4, ?f = 5} = {"A", "B", "C", "D"};
            return {a, b, c, d, e, f};
        "#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session.clone(),
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        let expected = v_list(&[
            v_str("A"),
            v_str("B"),
            v_str("C"),
            v_str("D"),
            v_int(4),
            v_int(5),
        ]);
        assert_eq!(result, Ok(expected));

        // Case 2: Provide only required + first optional, missing last 4
        let program = r#"
            {a, ?b = 1, ?c = 2, ?d = 3, ?e = 4, ?f = 5} = {"A", "B"};
            return {a, b, c, d, e, f};
        "#;
        let state = world_with_test_program(program);
        let result = call_verb(
            state,
            session.clone(),
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        let expected = v_list(&[
            v_str("A"),
            v_str("B"),
            v_int(2),
            v_int(3),
            v_int(4),
            v_int(5),
        ]);
        assert_eq!(result, Ok(expected));

        // Case 3: Provide only required, missing all 5 optionals
        let program = r#"
            {a, ?b = 1, ?c = 2, ?d = 3, ?e = 4, ?f = 5} = {"A"};
            return {a, b, c, d, e, f};
        "#;
        let state = world_with_test_program(program);
        let result = call_verb(
            state,
            session.clone(),
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        let expected = v_list(&[v_str("A"), v_int(1), v_int(2), v_int(3), v_int(4), v_int(5)]);
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn test_scatter_mixed_required_optionals() {
        // Test with multiple required parameters and optionals

        // Case: Two required, three optionals, provide all required + 1 optional
        let program = r#"
            {a, b, ?c = "C", ?d = "D", ?e = "E"} = {"A", "B", "provided_c"};
            return {a, b, c, d, e};
        "#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session.clone(),
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        let expected = v_list(&[
            v_str("A"),
            v_str("B"),
            v_str("provided_c"),
            v_str("D"),
            v_str("E"),
        ]);
        assert_eq!(result, Ok(expected));

        // Case: Two required, three optionals, provide all required + 2 optionals
        let program = r#"
            {a, b, ?c = "C", ?d = "D", ?e = "E"} = {"A", "B", "provided_c", "provided_d"};
            return {a, b, c, d, e};
        "#;
        let state = world_with_test_program(program);
        let result = call_verb(
            state,
            session.clone(),
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        let expected = v_list(&[
            v_str("A"),
            v_str("B"),
            v_str("provided_c"),
            v_str("provided_d"),
            v_str("E"),
        ]);
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn test_lambda_scatter_multiple_optionals_regression() {
        // Test lambda scatter with multiple optionals - this should expose bugs in lambda_scatter_assign
        // Test case 1: Missing third value (middle optional)
        let program = r#"
            f = {a, ?b = "default_b", ?c = "default_c"} => {a, b, c};
            return f("A", "B");
        "#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session.clone(),
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        // With two args provided, 'c' should get its default
        let expected = v_list(&[v_str("A"), v_str("B"), v_str("default_c")]);
        assert_eq!(result, Ok(expected));

        // Test case 2: All values provided
        let program = r#"
            f = {a, ?b = "default_b", ?c = "default_c"} => {a, b, c};
            return f("A", "B", "C");
        "#;
        let state = world_with_test_program(program);
        let result = call_verb(
            state,
            session.clone(),
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        let expected = v_list(&[v_str("A"), v_str("B"), v_str("C")]);
        assert_eq!(result, Ok(expected));

        // Test case 3: Missing second value - THIS IS THE BUG CASE
        // If lambda_scatter_assign is broken, this will fail
        let program = r#"
            f = {a, ?b = "default_b", ?c = "default_c"} => {a, b, c};
            return f("A");
        "#;
        let state = world_with_test_program(program);
        let result = call_verb(
            state,
            session.clone(),
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        // With only one arg, both optionals should get defaults
        let expected = v_list(&[v_str("A"), v_str("default_b"), v_str("default_c")]);
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn test_lambda_scatter_five_optionals() {
        // Stress test with 5 optionals in lambda
        let program = r#"
            func = {a, ?b = 1, ?c = 2, ?d = 3, ?e = 4, ?g = 5} => {a, b, c, d, e, g};
            return func("A", "B", "C");
        "#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session.clone(),
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        // First 3 get provided values, last 3 get defaults
        let expected = v_list(&[
            v_str("A"),
            v_str("B"),
            v_str("C"),
            v_int(3),
            v_int(4),
            v_int(5),
        ]);
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn test_lambda_scatter_with_complex_defaults() {
        // Test that lambda default expressions work correctly
        let program = r#"
            f = {a, ?b = {1, 2}, ?c = #5} => {a, b, c};
            return f("A");
        "#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session.clone(),
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        // Both optionals should get their complex defaults
        let expected = v_list(&[
            v_str("A"),
            v_list(&[v_int(1), v_int(2)]),
            v_obj(Obj::mk_id(5)),
        ]);
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn test_stack_tracking_after_try_finally() {
        // Regression test for stack underflow bug after try/finally
        // The compiler was not tracking the stack correctly after try/finally blocks
        let program = r#"
            result = #-3;
            try
                temp = 42;
            finally
                x = 1;
            endtry
            result != #-3 && raise(E_ASSERT, "should not raise");
            return result;
        "#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session.clone(),
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result, Ok(v_obj(Obj::mk_id(-3))));
    }

    #[test]
    fn test_regression_none_to_scatter_from_error() {
        // This is causing a panic because one of the elements in the scatter from `e` is "TYPE_NONE"
        // None is a kind of sigial for "variable not defined" not an actual "value" in mooR/MOO.
        // So I have explicit asserts to look for scenarios where it's getting assigned in.
        // We need to diagnose why this is happening (likely how the exception handler is populating `e`,
        //  and/or how scatter is treating it
        let program = r#"try
                                raise(E_INVARG, "test");
                              except e (ANY)
                                {a, b, c, d} = e;
                                return c;
                              endtry
                            "#;
        let state = world_with_test_program(program);
        let session = Arc::new(NoopClientSession::new());
        let result = call_verb(
            state,
            session.clone(),
            BuiltinRegistry::new(),
            "test",
            List::mk_list(&[]),
        );
        assert_eq!(result, Ok(v_int(0)));
    }
}
