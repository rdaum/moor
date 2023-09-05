#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use moor_value::model::props::PropFlag;
    use moor_value::model::r#match::VerbArgsSpec;
    use moor_value::model::verbs::{BinaryType, VerbFlag};
    use moor_value::model::world_state::{WorldState, WorldStateSource};
    use moor_value::util::bitenum::BitEnum;
    use moor_value::var::error::Error::E_VERBNF;
    use moor_value::var::objid::Objid;
    use moor_value::var::{v_bool, v_empty_list, v_err, v_int, v_list, v_none, v_obj, v_str, Var};
    use moor_value::NOTHING;
    use moor_value::{AsByteBuffer, SYSTEM_OBJECT};

    use crate::compiler::codegen::compile;
    use crate::compiler::labels::Names;
    use crate::db::inmemtransient::InMemTransientDatabase;
    use crate::tasks::sessions::{MockClientSession, NoopClientSession, Session};
    use crate::tasks::VerbCall;
    use crate::vm::opcode::Op::*;
    use crate::vm::opcode::{Op, Program};
    use crate::vm::vm_execute::VmExecParams;
    use crate::vm::{ExecutionResult, VerbExecutionRequest, VM};

    fn mk_program(main_vector: Vec<Op>, literals: Vec<Var>, var_names: Names) -> Program {
        Program {
            literals,
            jump_labels: vec![],
            var_names,
            main_vector,
            fork_vectors: vec![],
            line_number_spans: vec![],
        }
    }

    async fn call_verb(state: &mut dyn WorldState, verb_name: &str, vm: &mut VM) {
        let o = Objid(0);

        let call = VerbCall {
            verb_name: verb_name.to_string(),
            location: o,
            this: o,
            player: o,
            args: vec![],
            caller: NOTHING,
        };
        let verb = state.find_method_verb_on(o, o, verb_name).await.unwrap();
        let program = Program::from_sliceref(verb.binary());
        let cr = VerbExecutionRequest {
            permissions: o,
            resolved_verb: verb,
            call,
            command: None,
            program,
        };
        assert!(vm.exec_call_request(0, cr).await.is_ok());
    }

    async fn exec_vm(state: &mut dyn WorldState, vm: &mut VM) -> Var {
        exec_vm_loop(vm, state, Arc::new(NoopClientSession::new())).await
    }

    // TODO: move this up into a testing utility. But also factor out common code with Task's loop
    //  so that we aren't duplicating and failing to keep in sync.
    async fn exec_vm_loop(
        vm: &mut VM,
        world_state: &mut dyn WorldState,
        client_connection: Arc<dyn Session>,
    ) -> Var {
        // Call repeatedly into exec until we ge either an error or Complete.
        loop {
            let (sched_send, _) = tokio::sync::mpsc::unbounded_channel();
            let vm_exec_params = VmExecParams {
                world_state,
                session: client_connection.clone(),
                scheduler_sender: sched_send.clone(),
                max_stack_depth: 50,
                ticks_left: 90_000,
                time_left: None,
            };
            match vm.exec(vm_exec_params, 1_000000).await {
                Ok(ExecutionResult::More) => continue,
                Ok(ExecutionResult::Complete(a)) => return a,
                Err(e) => panic!("error during execution: {:?}", e),
                Ok(ExecutionResult::Exception(e)) => {
                    panic!("MOO exception {:?}", e);
                }
                Ok(ExecutionResult::ContinueVerb {
                    permissions,
                    resolved_verb,
                    call,
                    command,
                    trampoline: _,
                    trampoline_arg: _,
                }) => {
                    let decoded_verb = Program::from_sliceref(resolved_verb.binary());
                    let cr = VerbExecutionRequest {
                        permissions,
                        resolved_verb,
                        call,
                        command,
                        program: decoded_verb,
                    };
                    vm.exec_call_request(0, cr).await.unwrap();
                }
                Ok(ExecutionResult::PerformEval {
                    permissions,
                    player,
                    program,
                }) => {
                    vm.exec_eval_request(0, permissions, player, program)
                        .await
                        .expect("Could not set up VM for verb execution");
                }
                Ok(ExecutionResult::DispatchFork(_)) => {
                    panic!("fork not implemented in test VM")
                }
                Ok(ExecutionResult::Suspend(_)) => {
                    panic!("suspend not implemented in test VM")
                }
                Ok(ExecutionResult::ContinueBuiltin {
                    bf_func_num: _,
                    arguments: _,
                }) => {}
            }
        }
    }

    // Create an in memory db with a single object (#0) containing a single provided verb.
    async fn test_db_with_verbs(verbs: &[(&str, &Program)]) -> InMemTransientDatabase {
        let state = InMemTransientDatabase::new();
        let mut tx = state.new_world_state().await.unwrap();
        let sysobj = tx
            .create_object(SYSTEM_OBJECT, NOTHING, SYSTEM_OBJECT, BitEnum::all())
            .await
            .unwrap();
        tx.update_property(SYSTEM_OBJECT, sysobj, "name", &v_str("system"))
            .await
            .unwrap();
        tx.update_property(SYSTEM_OBJECT, sysobj, "programmer", &v_int(1))
            .await
            .unwrap();
        tx.update_property(SYSTEM_OBJECT, sysobj, "wizard", &v_int(1))
            .await
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
        .await
        .unwrap();

        for (verb_name, program) in verbs {
            let binary = program.make_copy_as_vec();
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
            .await
            .unwrap();
        }
        tx.commit().await.unwrap();
        state
    }

    async fn test_db_with_verb(verb_name: &str, program: &Program) -> InMemTransientDatabase {
        test_db_with_verbs(&[(verb_name, program)]).await
    }

    #[tokio::test]
    async fn test_simple_vm_execute() {
        let program = mk_program(vec![Imm(0.into()), Pop, Done], vec![1.into()], Names::new());
        let state_source = test_db_with_verb("test", &program).await;
        let mut state = state_source.new_world_state().await.unwrap();
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_none());
    }

    #[tokio::test]
    async fn test_string_value_simple_indexing() {
        let state_source = test_db_with_verb(
            "test",
            &mk_program(
                vec![Imm(0.into()), Imm(1.into()), Ref, Return, Done],
                vec![v_str("hello"), 2.into()],
                Names::new(),
            ),
        )
        .await;
        let mut state = state_source.new_world_state().await.unwrap();
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_str("e"));
    }

    #[tokio::test]
    async fn test_string_value_range_indexing() {
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
        .await
        .new_world_state()
        .await
        .unwrap();
        let mut vm = VM::new();
        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_str("ell"));
    }

    #[tokio::test]
    async fn test_list_value_simple_indexing() {
        let mut state = test_db_with_verb(
            "test",
            &mk_program(
                vec![Imm(0.into()), Imm(1.into()), Ref, Return, Done],
                vec![v_list(vec![111.into(), 222.into(), 333.into()]), 2.into()],
                Names::new(),
            ),
        )
        .await
        .new_world_state()
        .await
        .unwrap();
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_int(222));
    }

    #[tokio::test]
    async fn test_list_value_range_indexing() {
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
                    v_list(vec![111.into(), 222.into(), 333.into()]),
                    2.into(),
                    3.into(),
                ],
                Names::new(),
            ),
        )
        .await
        .new_world_state()
        .await
        .unwrap();
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_list(vec![222.into(), 333.into()]));
    }

    #[tokio::test]
    async fn test_list_set_range() {
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
                    v_list(vec![111.into(), 222.into(), 333.into()]),
                    2.into(),
                    3.into(),
                    v_list(vec![321.into(), 123.into()]),
                ],
                var_names,
            ),
        )
        .await
        .new_world_state()
        .await
        .unwrap();
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_list(vec![111.into(), 321.into(), 123.into()]));
    }

    #[tokio::test]
    async fn test_list_splice() {
        let program = "a = {1,2,3,4,5}; return {@a[2..4]};";
        let binary = compile(program).unwrap();
        let mut state = test_db_with_verb("test", &binary)
            .await
            .new_world_state()
            .await
            .unwrap();
        let mut vm = VM::new();
        let _args = binary.find_var("args");
        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_list(vec![2.into(), 3.into(), 4.into()]));
    }

    #[tokio::test]
    async fn test_list_range_length() {
        let program = "return {{1,2,3}[2..$], {1}[$]};";
        let mut state = test_db_with_verb("test", &compile(program).unwrap())
            .await
            .new_world_state()
            .await
            .unwrap();
        let mut vm = VM::new();
        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(
            result,
            v_list(vec![v_list(vec![2.into(), 3.into()]), v_int(1)])
        );
    }

    #[tokio::test]
    async fn test_if_or_expr() {
        let program = "if (1 || 0) return 1; else return 2; endif";
        let mut state = test_db_with_verb("test", &compile(program).unwrap())
            .await
            .new_world_state()
            .await
            .unwrap();
        let mut vm = VM::new();
        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_int(1));
    }

    #[tokio::test]
    async fn test_string_set_range() {
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
        .await
        .new_world_state()
        .await
        .unwrap();
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_str("manbozorian"));
    }

    #[tokio::test]
    async fn test_property_retrieval() {
        let mut state = test_db_with_verb(
            "test",
            &mk_program(
                vec![Imm(0.into()), Imm(1.into()), GetProp, Return, Done],
                vec![v_obj(0), v_str("test_prop")],
                Names::new(),
            ),
        )
        .await
        .new_world_state()
        .await
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
                .await
                .unwrap();
        }
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_int(666));
    }

    #[tokio::test]
    async fn test_call_verb() {
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
        .await
        .new_world_state()
        .await
        .unwrap();
        let mut vm = VM::new();

        // Invoke the second verb
        call_verb(state.as_mut(), "test_call_verb", &mut vm).await;

        let result = exec_vm(state.as_mut(), &mut vm).await;

        assert_eq!(result, v_int(666));
    }

    async fn world_with_test_program(program: &str) -> Box<dyn WorldState> {
        let binary = compile(program).unwrap();
        test_db_with_verb("test", &binary)
            .await
            .new_world_state()
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn test_assignment_from_range() {
        let program = "x = 1; y = {1,2,3}; x = x + y[2]; return x;";
        let mut state = world_with_test_program(program).await;
        let mut vm = VM::new();
        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_int(3));
    }

    #[tokio::test]
    async fn test_while_loop() {
        let program =
            "x = 0; while (x<100) x = x + 1; if (x == 75) break; endif endwhile return x;";
        let mut state = world_with_test_program(program).await;
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_int(75));
    }

    #[tokio::test]
    async fn test_while_labelled_loop() {
        let program = "x = 0; while broken (1) x = x + 1; if (x == 50) break; else continue broken; endif endwhile return x;";
        let mut state = world_with_test_program(program).await;

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_int(50));
    }

    #[tokio::test]
    async fn test_while_breaks() {
        let program = "x = 0; while (1) x = x + 1; if (x == 50) break; endif endwhile return x;";
        let mut state = world_with_test_program(program).await;

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_int(50));
    }

    #[tokio::test]
    async fn test_while_continue() {
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
        let mut state = world_with_test_program(program).await;
        let mut vm = VM::new();
        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_int(50));
    }

    #[tokio::test]
    async fn test_for_list_loop() {
        let program = "x = {1,2,3,4}; z = 0; for i in (x) z = z + i; endfor return {i,z};";
        let mut state = world_with_test_program(program).await;

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_list(vec![v_int(4), v_int(10)]));
    }

    #[tokio::test]
    async fn test_for_range_loop() {
        let program = "z = 0; for i in [1..4] z = z + i; endfor return {i,z};";
        let mut state = world_with_test_program(program).await;

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_list(vec![v_int(4), v_int(10)]));
    }

    #[tokio::test]
    async fn test_basic_scatter_assign() {
        let program = "{a, b, c, ?d = 4} = {1, 2, 3}; return {d, c, b, a};";
        let mut state = world_with_test_program(program).await;

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_list(vec![v_int(4), v_int(3), v_int(2), v_int(1)]));
    }

    #[tokio::test]
    async fn test_more_scatter_assign() {
        let program = "{a, b, @c} = {1, 2, 3, 4}; {x, @y, ?z} = {5,6,7,8}; return {a,b,c,x,y,z};";
        let mut state = world_with_test_program(program).await;

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(
            result,
            v_list(vec![
                v_int(1),
                v_int(2),
                v_list(vec![v_int(3), v_int(4)]),
                v_int(5),
                v_list(vec![v_int(6), v_int(7)]),
                v_int(8),
            ])
        );
    }

    #[tokio::test]
    async fn test_scatter_multi_optional() {
        let program = "{?a, ?b, ?c, ?d = a, @remain} = {1, 2, 3}; return {d, c, b, a, remain};";
        let mut state = world_with_test_program(program).await;
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(
            result,
            v_list(vec![v_int(1), v_int(3), v_int(2), v_int(1), v_empty_list()])
        );
    }

    #[tokio::test]
    async fn test_scatter_regression() {
        // Wherein I discovered that precedence order for scatter assign was wrong wrong wrong.
        let program = r#"
        a = {{#2, #70, #70, #-1, #-1}, #70};
        thing = a[2];
        {?who = player, ?what = thing, ?where = this:_locations(who), ?dobj, ?iobj, @other} = a[1];
        return {who, what, where, dobj, iobj, @other};
        "#;
        let mut state = world_with_test_program(program).await;
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        // MOO has  {#2, #70, #70, #-1, #-1, {}} for this equiv in JHCore parse_parties, and does not
        // actually invoke `_locations` (where i've subbed 666) for these values.
        // So something is wonky about our scatter evaluation, looks like on the first arg.
        assert_eq!(
            result,
            v_list(vec![v_obj(2), v_obj(70), v_obj(70), v_obj(-1), v_obj(-1)])
        );
    }

    #[tokio::test]
    async fn test_new_scatter_regression() {
        let program = "{a,b,@c}= {1,2,3,4,5}; return c;";
        let mut state = world_with_test_program(program).await;
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_list(vec![v_int(3), v_int(4), v_int(5)]));
    }

    #[tokio::test]
    async fn test_scatter_precedence() {
        // Simplified case of operator precedence fix.
        let program = "{a,b,c} = {{1,2,3}}[1]; return {a,b,c};";
        let mut state = world_with_test_program(program).await;
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_list(vec![v_int(1), v_int(2), v_int(3)]));
    }

    #[tokio::test]
    async fn test_conditional_expr() {
        let program = "return 1 ? 2 | 3;";
        let mut state = world_with_test_program(program).await;

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_int(2));
    }

    #[tokio::test]
    async fn test_catch_expr() {
        let program = "return {`x ! e_varnf => 666', `321 ! e_verbnf => 123'};";
        let mut state = world_with_test_program(program).await;

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_list(vec![v_int(666), v_int(321)]));
    }

    #[tokio::test]
    async fn test_catch_expr_any() {
        let program = "return `raise(E_VERBNF) ! ANY';";
        let mut state = world_with_test_program(program).await;

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_err(E_VERBNF));
    }

    #[tokio::test]
    async fn test_try_except_stmt() {
        let program = "try a; except e (E_VARNF) return 666; endtry return 333;";
        let mut state = world_with_test_program(program).await;

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_int(666));
    }

    #[tokio::test]
    async fn test_try_finally_stmt() {
        let program = "try a; finally return 666; endtry return 333;";
        let mut state = world_with_test_program(program).await;

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_int(666));
    }

    #[tokio::test]
    async fn test_if_elseif_else_chain() {
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
        let mut state = world_with_test_program(program).await;
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_list(vec![v_int(3), v_int(2), v_int(1)]));
    }

    #[tokio::test]
    async fn test_if_elseif_elseif_chains() {
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
        let mut state = world_with_test_program(program).await;
        let mut vm = VM::new();
        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_int(6));
    }

    #[tokio::test]
    async fn test_range_set() {
        let program = "a={1,2,3,4}; a[1..2] = {3,4}; return a;";
        let mut state = world_with_test_program(program).await;
        let mut vm = VM::new();
        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_list(vec![v_int(3), v_int(4), v_int(3), v_int(4)]));
    }

    #[tokio::test]
    async fn test_str_index_assignment() {
        // There was a regression here where the value was being dropped instead of replaced.
        let program = r#"a = "you"; a[1] = "Y"; return a;"#;
        let mut state = world_with_test_program(program).await;
        let mut vm = VM::new();
        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_str("You"));
    }

    #[tokio::test]
    // Regression test for Op::Length inside try/except, which was causing a panic due to the stack
    // offset being off by 1.
    async fn test_regression_length_expr_inside_try_except() {
        let program = r#"
        try
          return "hello world"[2..$];
        except (E_RANGE)
        endtry
        "#;
        let mut state = world_with_test_program(program).await;
        let mut vm = VM::new();
        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_str("ello world"));
    }

    #[tokio::test]
    // Same bug as above existed for catch exprs
    async fn test_regression_length_expr_inside_catch() {
        let program = r#"
        return `"hello world"[2..$] ! ANY';
        "#;
        let mut state = world_with_test_program(program).await;
        let mut vm = VM::new();
        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_str("ello world"));
    }

    // And try/finally.
    #[tokio::test]
    async fn test_regression_length_expr_inside_finally() {
        let program = r#"
        try return "hello world"[2..$]; finally endtry return "oh nope!";
        "#;
        let mut state = world_with_test_program(program).await;
        let mut vm = VM::new();
        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_str("ello world"));
    }

    #[tokio::test]
    async fn test_labelled_while_regression() {
        let program = r#"
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
        "#;
        let mut state = world_with_test_program(program).await;
        let mut vm = VM::new();
        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_obj(2));
    }

    #[tokio::test]
    async fn test_eval() {
        let program = r#"return eval("return 5;");"#;
        let mut state = world_with_test_program(program).await;
        let mut vm = VM::new();
        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_list(vec![v_bool(true), v_int(5)]));
    }

    // $sysprop style references were returning E_INVARG but only inside eval.
    #[tokio::test]
    async fn test_regression_sysprops() {
        let program = r#"return {1, eval("return $test;")};"#;
        let mut state = world_with_test_program(program).await;
        let mut vm = VM::new();
        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(
            result,
            v_list(vec![v_bool(true), v_list(vec![v_int(1), v_int(1)])])
        );
    }

    #[tokio::test]
    async fn test_negation_precedence() {
        let program = r#"return (!1 || 1);"#;
        let mut state = world_with_test_program(program).await;
        let mut vm = VM::new();
        call_verb(state.as_mut(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_int(1));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_call_builtin() {
        let program = "return notify(#1, \"test\");";
        let mut state = world_with_test_program(program).await;

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm).await;

        let client_connection = Arc::new(MockClientSession::new());
        let result = exec_vm_loop(&mut vm, state.as_mut(), client_connection.clone()).await;
        assert_eq!(result, v_int(1));

        assert_eq!(client_connection.received(), vec!["test".to_string()]);
        client_connection.commit().await.expect("commit failed");
        assert_eq!(client_connection.committed(), vec!["test".to_string()]);
    }
}
