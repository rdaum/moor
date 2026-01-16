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

//! Low-level VM tests using raw opcodes.
//! These tests exercise the bytecode interpreter directly without going through the compiler.

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use triomphe::Arc as TArc;

    use moor_common::{
        model::{ObjectKind, PropFlag, VerbArgsSpec, VerbFlag, WorldStateSource},
        util::BitEnum,
    };
    use moor_var::{List, Symbol, Var, v_bool, v_int, v_list, v_objid, v_str};

    use moor_var::{NOTHING, SYSTEM_OBJECT, *};

    use crate::{testing::vm_test_utils::call_verb, vm::builtins::BuiltinRegistry};
    use moor_common::tasks::NoopClientSession;
    use moor_compiler::{Names, Op, Op::*, Program};
    use moor_db::{DatabaseConfig, TxDB};
    use moor_var::program::{ProgramType, program::PrgInner};

    fn mk_program(main_vector: Vec<Op>, literals: Vec<Var>, var_names: Names) -> Program {
        Program(TArc::new(PrgInner {
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
}
