#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use anyhow::Error;
    use async_trait::async_trait;
    use tokio::sync::RwLock;
    use tracing_test::traced_test;

    use crate::compiler::codegen::compile;
    use crate::compiler::labels::Names;
    use crate::db::mock_world_state::MockWorldStateSource;
    use crate::db::state::{WorldState, WorldStateSource};
    use crate::model::objects::ObjFlag;
    use crate::model::props::PropFlag;
    use crate::model::ObjectError;
    use crate::model::ObjectError::VerbNotFound;
    use crate::tasks::Sessions;
    use crate::util::bitenum::BitEnum;
    use crate::var::error::Error::E_VERBNF;
    use crate::var::{v_err, v_int, v_list, v_obj, v_str, Objid, Var, VAR_NONE};
    use crate::vm::opcode::Op::*;
    use crate::vm::opcode::{Binary, Op};
    use crate::vm::vm::{ExecutionResult, VM};

    struct NoopClientConnection {}
    impl NoopClientConnection {
        pub fn new() -> Self {
            Self {}
        }
    }

    #[async_trait]
    impl Sessions for NoopClientConnection {
        async fn send_text(&mut self, _player: Objid, _msg: String) -> Result<(), anyhow::Error> {
            Ok(())
        }

        async fn connected_players(&self) -> Result<Vec<Objid>, Error> {
            Ok(vec![])
        }
    }

    fn mk_binary(main_vector: Vec<Op>, literals: Vec<Var>, var_names: Names) -> Binary {
        Binary {
            literals,
            jump_labels: vec![],
            var_names,
            main_vector,
            fork_vectors: vec![],
        }
    }

    fn call_verb(state: &mut dyn WorldState, verb_name: &str, vm: &mut VM) {
        let o = Objid(0);

        assert!(vm
            .setup_verb_method_call(
                0,
                state,
                o,
                verb_name,
                o,
                o,
                BitEnum::new_with(ObjFlag::Wizard) | ObjFlag::Programmer,
                &[],
            )
            .is_ok());
    }

    fn exec_vm(state: &mut dyn WorldState, vm: &mut VM) -> Var {
        tokio_test::block_on(async {
            let client_connection = Arc::new(RwLock::new(NoopClientConnection::new()));
            // Call repeatedly into exec until we ge either an error or Complete.
            loop {
                match vm.exec(state, client_connection.clone()).await {
                    Ok(ExecutionResult::More) => continue,
                    Ok(ExecutionResult::Complete(a)) => return a,
                    Err(e) => panic!("error during execution: {:?}", e),
                    Ok(ExecutionResult::Exception(e)) => {
                        panic!("MOO exception {:?}", e);
                    }
                }
            }
        })
    }

    #[test]
    fn test_verbnf() {
        let mut state_src = MockWorldStateSource::new();
        let mut state = state_src.new_world_state().unwrap();
        let mut vm = VM::new();
        let o = Objid(0);

        match vm.setup_verb_method_call(
            0,
            state.as_mut(),
            o,
            "test",
            o,
            o,
            BitEnum::new_with(ObjFlag::Wizard) | ObjFlag::Programmer,
            &[],
        ) {
            Err(e) => match e.downcast::<ObjectError>() {
                Ok(VerbNotFound(vo, vs)) => {
                    assert_eq!(vo, o);
                    assert_eq!(vs, "test");
                }
                _ => {
                    panic!("expected verbnf error");
                }
            },
            _ => panic!("expected verbnf error"),
        }
    }

    #[test]
    fn test_simple_vm_execute() {
        let binary = mk_binary(vec![Imm(0.into()), Pop, Done], vec![1.into()], Names::new());
        let mut state_src = MockWorldStateSource::new_with_verb("test", &binary);
        let mut state = state_src.new_world_state().unwrap();
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, VAR_NONE);
    }

    #[test]
    fn test_string_value_simple_indexing() {
        let mut state = MockWorldStateSource::new_with_verb(
            "test",
            &mk_binary(
                vec![Imm(0.into()), Imm(1.into()), Ref, Return, Done],
                vec![v_str("hello"), 2.into()],
                Names::new(),
            ),
        )
        .new_world_state()
        .unwrap();
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_str("e"));
    }

    #[test]
    fn test_string_value_range_indexing() {
        let mut state = MockWorldStateSource::new_with_verb(
            "test",
            &mk_binary(
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
        let mut vm = VM::new();
        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_str("ell"));
    }

    #[test]
    fn test_list_value_simple_indexing() {
        let mut state = MockWorldStateSource::new_with_verb(
            "test",
            &mk_binary(
                vec![Imm(0.into()), Imm(1.into()), Ref, Return, Done],
                vec![v_list(vec![111.into(), 222.into(), 333.into()]), 2.into()],
                Names::new(),
            ),
        )
        .new_world_state()
        .unwrap();
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_int(222));
    }

    #[test]
    fn test_list_value_range_indexing() {
        let mut state = MockWorldStateSource::new_with_verb(
            "test",
            &mk_binary(
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
        .unwrap();
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_list(vec![222.into(), 333.into()]));
    }

    #[test]
    fn test_list_set_range() {
        let mut var_names = Names::new();
        let a = var_names.find_or_add_name("a");
        let mut state = MockWorldStateSource::new_with_verb(
            "test",
            &mk_binary(
                vec![
                    Imm(0.into()),
                    Put(a.0),
                    Pop,
                    Push(a.0),
                    Imm(1.into()),
                    Imm(2.into()),
                    Imm(3.into()),
                    PutTemp,
                    RangeSet,
                    Put(a.0),
                    Pop,
                    PushTemp,
                    Pop,
                    Push(a.0),
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
        .unwrap();
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_list(vec![111.into(), 321.into(), 123.into()]));
    }

    #[test]
    fn test_list_splice() {
        let program = "a = {1,2,3,4,5}; return {@a[2..4]};";
        let binary = compile(program).unwrap();
        let mut state = MockWorldStateSource::new_with_verb("test", &binary)
            .new_world_state()
            .unwrap();
        let mut vm = VM::new();
        let _args = binary.find_var("args");
        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_list(vec![2.into(), 3.into(), 4.into()]));
    }

    #[test]
    fn test_list_range_length() {
        let program = "return {{1,2,3}[2..$], {1}[$]};";
        let mut state = MockWorldStateSource::new_with_verb("test", &compile(program).unwrap())
            .new_world_state()
            .unwrap();
        let mut vm = VM::new();
        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(
            result,
            v_list(vec![v_list(vec![2.into(), 3.into()]), v_int(1)])
        );
    }

    #[test]
    fn test_if_or_expr() {
        let program = "if (1 || 0) return 1; else return 2; endif";
        let mut state = MockWorldStateSource::new_with_verb("test", &compile(program).unwrap())
            .new_world_state()
            .unwrap();
        let mut vm = VM::new();
        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_int(1));
    }

    #[test]
    fn test_string_set_range() {
        let mut var_names = Names::new();
        let a = var_names.find_or_add_name("a");

        let mut state = MockWorldStateSource::new_with_verb(
            "test",
            &mk_binary(
                vec![
                    Imm(0.into()),
                    Put(a.0),
                    Pop,
                    Push(a.0),
                    Imm(1.into()),
                    Imm(2.into()),
                    Imm(3.into()),
                    PutTemp,
                    RangeSet,
                    Put(a.0),
                    Pop,
                    PushTemp,
                    Pop,
                    Push(a.0),
                    Return,
                    Done,
                ],
                vec![v_str("mandalorian"), 4.into(), 7.into(), v_str("bozo")],
                var_names,
            ),
        )
        .new_world_state()
        .unwrap();
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_str("manbozorian"));
    }

    #[test]
    fn test_property_retrieval() {
        let mut state = MockWorldStateSource::new_with_verb(
            "test",
            &mk_binary(
                vec![Imm(0.into()), Imm(1.into()), GetProp, Return, Done],
                vec![v_obj(0), v_str("test_prop")],
                Names::new(),
            ),
        )
        .new_world_state()
        .unwrap();
        {
            state
                .add_property(
                    Objid(0),
                    "test_prop",
                    Objid(0),
                    BitEnum::new_with(PropFlag::Read) | PropFlag::Write,
                    Some(v_int(666)),
                )
                .unwrap();
        }
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_int(666));
    }

    #[test]
    fn test_call_verb() {
        // Prepare two, chained, test verbs in our environment, with simple operations.

        // The first merely returns the value "666" immediately.
        let return_verb_binary = mk_binary(
            vec![Imm(0.into()), Return, Done],
            vec![v_int(666)],
            Names::new(),
        );

        // The second actually calls the first verb, and returns the result.
        let call_verb_binary = mk_binary(
            vec![
                Imm(0.into()), /* obj */
                Imm(1.into()), /* verb */
                Imm(2.into()), /* args */
                CallVerb,
                Return,
                Done,
            ],
            vec![v_obj(0), v_str("test_return_verb"), v_list(vec![])],
            Names::new(),
        );
        let mut state = MockWorldStateSource::new_with_verbs(vec![
            ("test_return_verb", &return_verb_binary),
            ("test_call_verb", &call_verb_binary),
        ])
        .new_world_state()
        .unwrap();
        let mut vm = VM::new();

        // Invoke the second verb
        call_verb(state.as_mut(), "test_call_verb", &mut vm);

        let result = exec_vm(state.as_mut(), &mut vm);

        assert_eq!(result, v_int(666));
    }

    fn world_with_test_program(program: &str) -> Box<dyn WorldState> {
        let binary = compile(program).unwrap();
        let state = MockWorldStateSource::new_with_verb("test", &binary)
            .new_world_state()
            .unwrap();
        state
    }

    #[test]
    fn test_assignment_from_range() {
        let program = "x = 1; y = {1,2,3}; x = x + y[2]; return x;";
        let mut state = world_with_test_program(program);
        let mut vm = VM::new();
        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_int(3));
    }

    #[test]
    fn test_while_loop() {
        let program =
            "x = 0; while (x<100) x = x + 1; if (x == 75) break; endif endwhile return x;";
        let mut state = world_with_test_program(program);
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_int(75));
    }

    #[test]
    fn test_while_labelled_loop() {
        let program = "x = 0; while broken (1) x = x + 1; if (x == 50) break; else continue broken; endif endwhile return x;";
        let mut state = world_with_test_program(program);

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_int(50));
    }

    #[test]
    fn test_while_breaks() {
        let program = "x = 0; while (1) x = x + 1; if (x == 50) break; endif endwhile return x;";
        let mut state = world_with_test_program(program);

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_int(50));
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
        let mut vm = VM::new();
        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_int(50));
    }

    #[test]
    fn test_for_list_loop() {
        let program = "x = {1,2,3,4}; z = 0; for i in (x) z = z + i; endfor return {i,z};";
        let mut state = world_with_test_program(program);

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_list(vec![v_int(4), v_int(10)]));
    }

    #[test]
    fn test_for_range_loop() {
        let program = "z = 0; for i in [1..4] z = z + i; endfor return {i,z};";
        let mut state = world_with_test_program(program);

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_list(vec![v_int(4), v_int(10)]));
    }

    #[test]
    fn test_basic_scatter_assign() {
        let program = "{a, b, c, ?d = 4} = {1, 2, 3}; return {d, c, b, a};";
        let mut state = world_with_test_program(program);

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_list(vec![v_int(4), v_int(3), v_int(2), v_int(1)]));
    }

    #[test]
    fn test_more_scatter_assign() {
        let program = "{a, b, @c} = {1, 2, 3, 4}; {x, @y, ?z} = {5,6,7,8}; return {a,b,c,x,y,z};";
        let mut state = world_with_test_program(program);

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
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

    #[test]
    fn test_scatter_multi_optional() {
        let program = "{?a, ?b, ?c, ?d = a, @remain} = {1, 2, 3}; return {d, c, b, a, remain};";
        let mut state = world_with_test_program(program);
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(
            result,
            v_list(vec![v_int(1), v_int(3), v_int(2), v_int(1), v_list(vec![])])
        );
    }

    #[test]
    #[traced_test]
    fn test_scatter_regression() {
        // Wherein I discovered that precedence order for scatter assign was wrong wrong wrong.
        let program = r#"
        a = {{#2, #70, #70, #-1, #-1}, #70};
        thing = a[2];
        {?who = player, ?what = thing, ?where = this:_locations(who), ?dobj, ?iobj, @other} = a[1];
        return {who, what, where, dobj, iobj, @other};
        "#;
        let mut state = world_with_test_program(program);
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        // MOO has  {#2, #70, #70, #-1, #-1, {}} for this equiv in JHCore parse_parties, and does not
        // actually invoke `_locations` (where i've subbed 666) for these values.
        // So something is wonky about our scatter evaluation, looks like on the first arg.
        assert_eq!(
            result,
            v_list(vec![v_obj(2), v_obj(70), v_obj(70), v_obj(-1), v_obj(-1)])
        );
    }

    #[test]
    #[traced_test]
    fn test_scatter_precedence() {
        // Simplified case of operator precedence fix.
        let program = "{a,b,c} = {{1,2,3}}[1]; return {a,b,c};";
        let mut state = world_with_test_program(program);
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_list(vec![v_int(1), v_int(2), v_int(3)]));
    }

    #[test]
    fn test_conditional_expr() {
        let program = "return 1 ? 2 | 3;";
        let mut state = world_with_test_program(program);

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_int(2));
    }

    #[test]
    fn test_catch_expr() {
        let program = "return {`x ! e_varnf => 666', `321 ! e_verbnf => 123'};";
        let mut state = world_with_test_program(program);

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_list(vec![v_int(666), v_int(321)]));
    }

    #[test]
    fn test_catch_expr_any() {
        let program = "return `raise(E_VERBNF) ! ANY';";
        let mut state = world_with_test_program(program);

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_err(E_VERBNF));
    }

    #[test]
    fn test_try_except_stmt() {
        let program = "try a; except e (E_VARNF) return 666; endtry return 333;";
        let mut state = world_with_test_program(program);

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_int(666));
    }

    #[test]
    fn test_try_finally_stmt() {
        let program = "try a; finally return 666; endtry return 333;";
        let mut state = world_with_test_program(program);

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_int(666));
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
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_list(vec![v_int(3), v_int(2), v_int(1)]));
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
        let mut vm = VM::new();
        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_int(6));
    }

    #[test]
    fn test_range_set() {
        let program = "a={1,2,3,4}; a[1..2] = {3,4}; return a;";
        let mut state = world_with_test_program(program);
        let mut vm = VM::new();
        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_list(vec![v_int(3), v_int(4), v_int(3), v_int(4)]));
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
        async fn send_text(&mut self, _player: Objid, msg: String) -> Result<(), Error> {
            self.received.push(msg);
            Ok(())
        }

        async fn connected_players(&self) -> Result<Vec<Objid>, Error> {
            Ok(vec![])
        }
    }

    async fn exec_vm_with_mock_client_connection(
        vm: &mut VM,
        state: &mut dyn WorldState,
        client_connection: Arc<RwLock<MockClientConnection>>,
    ) -> Var {
        // Call repeatedly into exec until we ge either an error or Complete.
        loop {
            match vm.exec(state, client_connection.clone()).await {
                Ok(ExecutionResult::More) => continue,
                Ok(ExecutionResult::Complete(a)) => return a,
                Err(e) => panic!("error during execution: {:?}", e),
                Ok(ExecutionResult::Exception(e)) => {
                    panic!("MOO exception {:?}", e);
                }
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_call_builtin() {
        let program = "return notify(#1, \"test\");";
        let mut state = world_with_test_program(program);

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);

        let client_connection = Arc::new(RwLock::new(MockClientConnection::new()));
        let result =
            exec_vm_with_mock_client_connection(&mut vm, state.as_mut(), client_connection.clone())
                .await;
        assert_eq!(result, VAR_NONE);

        assert_eq!(
            client_connection.read().await.received,
            vec!["test".to_string()]
        );
    }
}
