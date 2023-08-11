#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use anyhow::Error;
    use async_trait::async_trait;
    use bincode::decode_from_slice;
    use tokio::sync::RwLock;

    use crate::BINCODE_CONFIG;
    use moor_value::util::bitenum::BitEnum;
    use moor_value::var::error::Error::E_VERBNF;
    use moor_value::var::objid::{Objid, NOTHING};
    use moor_value::var::{v_empty_list, v_err, v_int, v_list, v_none, v_obj, v_str, Var};

    use crate::compiler::codegen::compile;
    use crate::compiler::labels::Names;
    use crate::db::mock_world_state::MockWorldStateSource;
    use crate::tasks::{Sessions, VerbCall};
    use crate::vm::opcode::Op::*;
    use crate::vm::opcode::{Op, Program};
    use crate::vm::vm_execute::VmExecParams;
    use crate::vm::{ExecutionResult, VerbExecutionRequest, VM};
    use moor_value::model::permissions::PermissionsContext;
    use moor_value::model::props::PropFlag;
    use moor_value::model::world_state::{WorldState, WorldStateSource};

    struct NoopClientConnection {}
    impl NoopClientConnection {
        pub fn new() -> Self {
            Self {}
        }
    }

    #[async_trait]
    impl Sessions for NoopClientConnection {
        async fn send_text(&mut self, _player: Objid, _msg: &str) -> Result<(), anyhow::Error> {
            Ok(())
        }

        async fn shutdown(&mut self, _msg: Option<String>) -> Result<(), Error> {
            Ok(())
        }

        async fn connection_name(&self, player: Objid) -> Result<String, Error> {
            Ok(format!("player-{}", player.0))
        }

        async fn disconnect(&mut self, _player: Objid) -> Result<(), Error> {
            Ok(())
        }

        fn connected_players(&self) -> Result<Vec<Objid>, Error> {
            Ok(vec![])
        }

        fn connected_seconds(&self, _player: Objid) -> Result<f64, Error> {
            Ok(0.0)
        }

        fn idle_seconds(&self, _player: Objid) -> Result<f64, Error> {
            Ok(0.0)
        }
    }

    fn mk_program(main_vector: Vec<Op>, literals: Vec<Var>, var_names: Names) -> Program {
        Program {
            literals,
            jump_labels: vec![],
            var_names,
            main_vector,
            fork_vectors: vec![],
        }
    }

    async fn call_verb(
        state: &mut dyn WorldState,
        perms: PermissionsContext,
        verb_name: &str,
        vm: &mut VM,
    ) {
        let o = Objid(0);

        let call = VerbCall {
            verb_name: verb_name.to_string(),
            location: o,
            this: o,
            player: o,
            args: vec![],
            caller: NOTHING,
        };
        let verb = state.get_verb(perms.clone(), o, verb_name).await.unwrap();
        let (program, _) =
            decode_from_slice(verb.attrs.binary.as_ref().unwrap(), *BINCODE_CONFIG).unwrap();
        let cr = VerbExecutionRequest {
            permissions: perms,
            resolved_verb: verb,
            call,
            command: None,
            program,
        };
        assert!(vm.exec_call_request(0, cr).await.is_ok());
    }

    async fn exec_vm(state: &mut dyn WorldState, vm: &mut VM) -> Var {
        let client_connection = Arc::new(RwLock::new(NoopClientConnection::new()));
        // Call repeatedly into exec until we ge either an error or Complete.

        loop {
            let (sched_send, _) = tokio::sync::mpsc::unbounded_channel();
            let vm_exec_params = VmExecParams {
                world_state: state,
                sessions: client_connection.clone(),
                scheduler_sender: sched_send.clone(),
                max_stack_depth: 50,
                ticks_left: 90_000,
                time_left: None,
            };
            match vm.exec(vm_exec_params).await {
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
                    let (decoded_verb, _) = bincode::decode_from_slice(
                        resolved_verb.attrs.binary.as_ref().unwrap(),
                        *BINCODE_CONFIG,
                    )
                    .unwrap();
                    let cr = VerbExecutionRequest {
                        permissions,
                        resolved_verb,
                        call,
                        command,
                        program: decoded_verb,
                    };
                    vm.exec_call_request(0, cr).await.unwrap();
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

    fn mock_perms() -> PermissionsContext {
        PermissionsContext::root_for(Objid(0), BitEnum::all())
    }

    #[tokio::test]
    async fn test_simple_vm_execute() {
        let program = mk_program(vec![Imm(0.into()), Pop, Done], vec![1.into()], Names::new());
        let mut state_src = MockWorldStateSource::new_with_verb("test", &program);
        let mut state = state_src.new_world_state().await.unwrap();
        let mut vm = VM::new();

        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_none());
    }

    #[tokio::test]
    async fn test_string_value_simple_indexing() {
        let mut state = MockWorldStateSource::new_with_verb(
            "test",
            &mk_program(
                vec![Imm(0.into()), Imm(1.into()), Ref, Return, Done],
                vec![v_str("hello"), 2.into()],
                Names::new(),
            ),
        )
        .new_world_state()
        .await
        .unwrap();
        let mut vm = VM::new();

        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_str("e"));
    }

    #[tokio::test]
    async fn test_string_value_range_indexing() {
        let mut state = MockWorldStateSource::new_with_verb(
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
        .await
        .unwrap();
        let mut vm = VM::new();
        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_str("ell"));
    }

    #[tokio::test]
    async fn test_list_value_simple_indexing() {
        let mut state = MockWorldStateSource::new_with_verb(
            "test",
            &mk_program(
                vec![Imm(0.into()), Imm(1.into()), Ref, Return, Done],
                vec![v_list(vec![111.into(), 222.into(), 333.into()]), 2.into()],
                Names::new(),
            ),
        )
        .new_world_state()
        .await
        .unwrap();
        let mut vm = VM::new();

        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_int(222));
    }

    #[tokio::test]
    async fn test_list_value_range_indexing() {
        let mut state = MockWorldStateSource::new_with_verb(
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
        .new_world_state()
        .await
        .unwrap();
        let mut vm = VM::new();

        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_list(vec![222.into(), 333.into()]));
    }

    #[tokio::test]
    async fn test_list_set_range() {
        let mut var_names = Names::new();
        let a = var_names.find_or_add_name("a");
        let mut state = MockWorldStateSource::new_with_verb(
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
        .new_world_state()
        .await
        .unwrap();
        let mut vm = VM::new();

        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_list(vec![111.into(), 321.into(), 123.into()]));
    }

    #[tokio::test]
    async fn test_list_splice() {
        let program = "a = {1,2,3,4,5}; return {@a[2..4]};";
        let binary = compile(program).unwrap();
        let mut state = MockWorldStateSource::new_with_verb("test", &binary)
            .new_world_state()
            .await
            .unwrap();
        let mut vm = VM::new();
        let _args = binary.find_var("args");
        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_list(vec![2.into(), 3.into(), 4.into()]));
    }

    #[tokio::test]
    async fn test_list_range_length() {
        let program = "return {{1,2,3}[2..$], {1}[$]};";
        let mut state = MockWorldStateSource::new_with_verb("test", &compile(program).unwrap())
            .new_world_state()
            .await
            .unwrap();
        let mut vm = VM::new();
        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(
            result,
            v_list(vec![v_list(vec![2.into(), 3.into()]), v_int(1)])
        );
    }

    #[tokio::test]
    async fn test_if_or_expr() {
        let program = "if (1 || 0) return 1; else return 2; endif";
        let mut state = MockWorldStateSource::new_with_verb("test", &compile(program).unwrap())
            .new_world_state()
            .await
            .unwrap();
        let mut vm = VM::new();
        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_int(1));
    }

    #[tokio::test]
    async fn test_string_set_range() {
        let mut var_names = Names::new();
        let a = var_names.find_or_add_name("a");

        let mut state = MockWorldStateSource::new_with_verb(
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
        .await
        .unwrap();
        let mut vm = VM::new();

        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_str("manbozorian"));
    }

    #[tokio::test]
    async fn test_property_retrieval() {
        let mut state = MockWorldStateSource::new_with_verb(
            "test",
            &mk_program(
                vec![Imm(0.into()), Imm(1.into()), GetProp, Return, Done],
                vec![v_obj(0), v_str("test_prop")],
                Names::new(),
            ),
        )
        .new_world_state()
        .await
        .unwrap();
        {
            state
                .define_property(
                    mock_perms(),
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

        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
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
        let mut state = MockWorldStateSource::new_with_verbs(vec![
            ("test_return_verb", &return_verb_binary),
            ("test_call_verb", &call_verb_binary),
        ])
        .new_world_state()
        .await
        .unwrap();
        let mut vm = VM::new();

        // Invoke the second verb
        call_verb(state.as_mut(), mock_perms(), "test_call_verb", &mut vm).await;

        let result = exec_vm(state.as_mut(), &mut vm).await;

        assert_eq!(result, v_int(666));
    }

    async fn world_with_test_program(program: &str) -> Box<dyn WorldState> {
        let binary = compile(program).unwrap();
        MockWorldStateSource::new_with_verb("test", &binary)
            .new_world_state()
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn test_assignment_from_range() {
        let program = "x = 1; y = {1,2,3}; x = x + y[2]; return x;";
        let mut state = world_with_test_program(program).await;
        let mut vm = VM::new();
        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_int(3));
    }

    #[tokio::test]
    async fn test_while_loop() {
        let program =
            "x = 0; while (x<100) x = x + 1; if (x == 75) break; endif endwhile return x;";
        let mut state = world_with_test_program(program).await;
        let mut vm = VM::new();

        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_int(75));
    }

    #[tokio::test]
    async fn test_while_labelled_loop() {
        let program = "x = 0; while broken (1) x = x + 1; if (x == 50) break; else continue broken; endif endwhile return x;";
        let mut state = world_with_test_program(program).await;

        let mut vm = VM::new();

        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_int(50));
    }

    #[tokio::test]
    async fn test_while_breaks() {
        let program = "x = 0; while (1) x = x + 1; if (x == 50) break; endif endwhile return x;";
        let mut state = world_with_test_program(program).await;

        let mut vm = VM::new();

        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
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
        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_int(50));
    }

    #[tokio::test]
    async fn test_for_list_loop() {
        let program = "x = {1,2,3,4}; z = 0; for i in (x) z = z + i; endfor return {i,z};";
        let mut state = world_with_test_program(program).await;

        let mut vm = VM::new();

        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_list(vec![v_int(4), v_int(10)]));
    }

    #[tokio::test]
    async fn test_for_range_loop() {
        let program = "z = 0; for i in [1..4] z = z + i; endfor return {i,z};";
        let mut state = world_with_test_program(program).await;

        let mut vm = VM::new();

        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_list(vec![v_int(4), v_int(10)]));
    }

    #[tokio::test]
    async fn test_basic_scatter_assign() {
        let program = "{a, b, c, ?d = 4} = {1, 2, 3}; return {d, c, b, a};";
        let mut state = world_with_test_program(program).await;

        let mut vm = VM::new();

        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_list(vec![v_int(4), v_int(3), v_int(2), v_int(1)]));
    }

    #[tokio::test]
    async fn test_more_scatter_assign() {
        let program = "{a, b, @c} = {1, 2, 3, 4}; {x, @y, ?z} = {5,6,7,8}; return {a,b,c,x,y,z};";
        let mut state = world_with_test_program(program).await;

        let mut vm = VM::new();

        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
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

        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
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

        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
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

        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_list(vec![v_int(3), v_int(4), v_int(5)]));
    }

    #[tokio::test]
    async fn test_scatter_precedence() {
        // Simplified case of operator precedence fix.
        let program = "{a,b,c} = {{1,2,3}}[1]; return {a,b,c};";
        let mut state = world_with_test_program(program).await;
        let mut vm = VM::new();

        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_list(vec![v_int(1), v_int(2), v_int(3)]));
    }

    #[tokio::test]
    async fn test_conditional_expr() {
        let program = "return 1 ? 2 | 3;";
        let mut state = world_with_test_program(program).await;

        let mut vm = VM::new();

        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_int(2));
    }

    #[tokio::test]
    async fn test_catch_expr() {
        let program = "return {`x ! e_varnf => 666', `321 ! e_verbnf => 123'};";
        let mut state = world_with_test_program(program).await;

        let mut vm = VM::new();

        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_list(vec![v_int(666), v_int(321)]));
    }

    #[tokio::test]
    async fn test_catch_expr_any() {
        let program = "return `raise(E_VERBNF) ! ANY';";
        let mut state = world_with_test_program(program).await;

        let mut vm = VM::new();

        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_err(E_VERBNF));
    }

    #[tokio::test]
    async fn test_try_except_stmt() {
        let program = "try a; except e (E_VARNF) return 666; endtry return 333;";
        let mut state = world_with_test_program(program).await;

        let mut vm = VM::new();

        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_int(666));
    }

    #[tokio::test]
    async fn test_try_finally_stmt() {
        let program = "try a; finally return 666; endtry return 333;";
        let mut state = world_with_test_program(program).await;

        let mut vm = VM::new();

        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
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

        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
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
        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_int(6));
    }

    #[tokio::test]
    async fn test_range_set() {
        let program = "a={1,2,3,4}; a[1..2] = {3,4}; return a;";
        let mut state = world_with_test_program(program).await;
        let mut vm = VM::new();
        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_list(vec![v_int(3), v_int(4), v_int(3), v_int(4)]));
    }

    #[tokio::test]
    async fn test_str_index_assignment() {
        // There was a regression here where the value was being dropped instead of replaced.
        let program = r#"a = "you"; a[1] = "Y"; return a;"#;
        let mut state = world_with_test_program(program).await;
        let mut vm = VM::new();
        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;
        let result = exec_vm(state.as_mut(), &mut vm).await;
        assert_eq!(result, v_str("You"));
    }

    struct MockClientConnection {
        received: Vec<String>,
    }
    impl MockClientConnection {
        pub fn new() -> Self {
            Self { received: vec![] }
        }
    }
    #[async_trait]
    impl Sessions for MockClientConnection {
        async fn send_text(&mut self, _player: Objid, msg: &str) -> Result<(), Error> {
            self.received.push(String::from(msg));
            Ok(())
        }

        async fn shutdown(&mut self, _msg: Option<String>) -> Result<(), Error> {
            Ok(())
        }

        async fn connection_name(&self, player: Objid) -> Result<String, Error> {
            Ok(format!("player-{}", player))
        }

        async fn disconnect(&mut self, _player: Objid) -> Result<(), Error> {
            Ok(())
        }

        fn connected_players(&self) -> Result<Vec<Objid>, Error> {
            Ok(vec![])
        }

        fn connected_seconds(&self, _player: Objid) -> Result<f64, Error> {
            Ok(0.0)
        }

        fn idle_seconds(&self, _player: Objid) -> Result<f64, Error> {
            Ok(0.0)
        }
    }

    async fn exec_vm_with_mock_client_connection(
        vm: &mut VM,
        state: &mut dyn WorldState,
        client_connection: Arc<RwLock<MockClientConnection>>,
    ) -> Var {
        // Call repeatedly into exec until we ge either an error or Complete.
        loop {
            let (sched_send, _) = tokio::sync::mpsc::unbounded_channel();
            let vm_exec_params = VmExecParams {
                world_state: state,
                sessions: client_connection.clone(),
                scheduler_sender: sched_send.clone(),
                max_stack_depth: 50,
                ticks_left: 90_000,
                time_left: None,
            };
            match vm.exec(vm_exec_params).await {
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
                    let (decoded_verb, _) = bincode::decode_from_slice(
                        resolved_verb.attrs.binary.as_ref().unwrap(),
                        *BINCODE_CONFIG,
                    )
                    .unwrap();
                    let cr = VerbExecutionRequest {
                        permissions,
                        resolved_verb,
                        call,
                        command,
                        program: decoded_verb,
                    };
                    vm.exec_call_request(0, cr).await.unwrap();
                }
                Ok(ExecutionResult::DispatchFork(_)) => {
                    panic!("dispatch fork not supported in this test");
                }
                Ok(ExecutionResult::Suspend(_)) => {
                    panic!("suspend not supported in this test");
                }
                Ok(ExecutionResult::ContinueBuiltin {
                    bf_func_num: _,
                    arguments: _,
                }) => {}
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_call_builtin() {
        let program = "return notify(#1, \"test\");";
        let mut state = world_with_test_program(program).await;

        let mut vm = VM::new();

        call_verb(state.as_mut(), mock_perms(), "test", &mut vm).await;

        let client_connection = Arc::new(RwLock::new(MockClientConnection::new()));
        let result =
            exec_vm_with_mock_client_connection(&mut vm, state.as_mut(), client_connection.clone())
                .await;
        assert_eq!(result, v_int(1));

        assert_eq!(
            client_connection.read().await.received,
            vec!["test".to_string()]
        );
    }
}
