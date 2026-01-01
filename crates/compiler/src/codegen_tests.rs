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

#[cfg(test)]
mod tests {
    use crate::{CompileOptions, codegen::compile};
    use moor_common::builtins::BUILTINS;
    use moor_var::{
        E_INVARG, E_INVIND, E_PERM, E_PROPNF, E_RANGE, Obj, SYSTEM_OBJECT, Symbol, UuObjid,
        program::{
            labels::{Label, Offset},
            opcode::{ForRangeOperand, ForSequenceOperand, Op::*, ScatterArgs, ScatterLabel},
        },
    };

    #[test]
    fn test_simple_add_expr() {
        let program = "1 + 2;";
        let binary = compile(program, CompileOptions::default()).unwrap();
        assert_eq!(
            binary.main_vector().to_vec(),
            vec![ImmInt(1), ImmInt(2), Add, Pop, Done]
        );
    }

    #[test]
    fn test_var_assign_expr() {
        let program = "a = 1 + 2;";
        let binary = compile(program, CompileOptions::default()).unwrap();

        let a = binary.find_var("a");
        /*
           "  0: 124 NUM 1",
           "  1: 125 NUM 2",
           "  2: 021 * ADD",
           "  3: 052 * PUT a",
           "  4: 111 POP",
           "  5: 123 NUM 0",
           "  6: 030 010 * AND 10",
        */
        assert_eq!(
            binary.main_vector().to_vec(),
            vec![ImmInt(1), ImmInt(2), Add, Put(a), Pop, Done],
        );
    }

    #[test]
    fn test_var_assign_retr_expr() {
        let program = "a = 1 + 2; return a;";
        let binary = compile(program, CompileOptions::default()).unwrap();

        let a = binary.find_var("a");

        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                ImmInt(1),
                ImmInt(2),
                Add,
                Put(a),
                Pop,
                Push(a),
                Return,
                Pop,
                Done
            ]
        );
    }

    #[test]
    fn test_while_stmt() {
        let program = "while (1) x = x + 1; endwhile";
        let binary = compile(program, CompileOptions::default()).unwrap();

        let x = binary.find_var("x");

        /*
        " 0: 124                   NUM 1",
        " 1: 001 010             * WHILE 10",
        "  3: 085                   PUSH x",
        "  4: 124                    NUM 1",
        "  5: 021                 * ADD",
        "  6: 052                 * PUT x",
        "  7: 111                   POP",
        "  8: 107 000               JUMP 0",
                 */
        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                ImmInt(1),
                While {
                    jump_label: 1.into(),
                    environment_width: 0,
                },
                Push(x),
                ImmInt(1),
                Add,
                Put(x),
                Pop,
                EndScope { num_bindings: 0 },
                Jump { label: 0.into() },
                Done
            ]
        );
    }

    #[test]
    fn test_for_in_list_stmt() {
        let program = "for x in ({1,2,3}) b = x + 5; endfor";
        let binary = compile(program, CompileOptions::default()).unwrap();

        let b = binary.find_var("b");
        let x = binary.find_var("x");

        /*
        "  0: 124                   NUM 1",
        "  1: 016                 * MAKE_SINGLETON_LIST",
        "  2: 125                   NUM 2",
        "  3: 102                   LIST_ADD_TAIL",
        "  4: 126                   NUM 3",
        "  5: 102                  LIST_ADD_TAIL",
        "  6: 124                   NUM 1",
        "  7: 005 019017         * FOR_LIST x 17",
        " 10: 086                   PUSH x",
        " 11: 128                    NUM 5",
        " 12: 021                 * ADD",
        " 13: 052                 * PUT b",
        " 14: 111                   POP",
        " 15: 107 007               JUMP 7",
                 */
        // The label for the ForList is not quite right here
        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                ImmInt(1),
                MakeSingletonList,
                ImmInt(2),
                ListAddTail,
                ImmInt(3),
                ListAddTail,
                BeginForSequence { operand: Offset(0) },
                IterateForSequence,
                Push(x),
                ImmInt(5),
                Add,
                Put(b),
                Pop,
                Jump { label: 1.into() },
                EndScope { num_bindings: 0 },
                Done
            ]
        );
        assert_eq!(
            binary.for_sequence_operand(Offset(0)),
            &ForSequenceOperand {
                value_bind: x,
                key_bind: None,
                end_label: 0.into(),
                environment_width: 0,
            }
        );
        assert_eq!(binary.jump_label(Label(0)).position.0, 15);
    }

    #[test]
    fn test_for_range() {
        let program = "for n in [1..5] player:tell(a); endfor";
        let binary = compile(program, CompileOptions::default()).unwrap();

        let player = binary.find_var("player");
        let a = binary.find_var("a");
        let n = binary.find_var("n");

        /*
         0: 124                   NUM 1
         1: 128                   NUM 5
         2: 006 019 014         * FOR_RANGE n 14
         5: 072                   PUSH player
         6: 100 000               PUSH_LITERAL "tell"
         8: 085                   PUSH a
         9: 016                 * MAKE_SINGLETON_LIST
        10: 010                 * CALL_VERB
        11: 111                   POP
        12: 107 002               JUMP 2
        */
        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                ImmInt(1),
                ImmInt(5),
                BeginForRange { operand: Offset(0) },
                IterateForRange,
                Push(player),
                ImmSymbol(Symbol::mk("tell")),
                Push(a),
                MakeSingletonList,
                CallVerb,
                Pop,
                Jump { label: 1.into() },
                EndScope { num_bindings: 0 },
                Done
            ]
        );
        assert_eq!(
            binary.for_range_operand(Offset(0)),
            &ForRangeOperand {
                loop_variable: n,
                end_label: 0.into(),
                environment_width: 0,
            }
        );
    }

    #[test]
    fn test_fork() {
        let program = "fork (5) player:tell(\"a\"); endfork";
        let binary = compile(program, CompileOptions::default()).unwrap();

        let player = binary.find_var("player");
        let a = binary.find_label_for_literal("a".into());

        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                ImmInt(5),
                Fork {
                    fv_offset: 0.into(),
                    id: None
                },
                Done
            ]
        );
        assert_eq!(
            binary.fork_vector(Offset(0)).to_vec(),
            vec![
                Push(player),                  // player
                ImmSymbol(Symbol::mk("tell")), // tell
                Imm(a),                        // 'a'
                MakeSingletonList,
                CallVerb,
                Pop,
                Done
            ]
        );
    }

    #[test]
    fn test_fork_id() {
        let program = "fork fid (5) player:tell(fid); endfork";
        let binary = compile(program, CompileOptions::default()).unwrap();

        let player = binary.find_var("player");
        let fid = binary.find_var("fid");

        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                ImmInt(5),
                Fork {
                    fv_offset: 0.into(),
                    id: Some(fid)
                },
                Done
            ]
        );
        assert_eq!(
            binary.fork_vector(Offset(0)).to_vec(),
            vec![
                Push(player),                  // player
                ImmSymbol(Symbol::mk("tell")), // tell
                Push(fid),                     // fid
                MakeSingletonList,
                CallVerb,
                Pop,
                Done
            ]
        );
    }

    #[test]
    fn test_and_or() {
        let program = "a = (1 && 2 || 3);";
        let binary = compile(program, CompileOptions::default()).unwrap();
        let a = binary.find_var("a");

        /*
         0: 124                   NUM 1
         1: 030 004             * AND 4
         3: 125                   NUM 2
         4: 031 007             * OR 7
         6: 126                   NUM 3
         7: 052                 * PUT a
         8: 111                   POP
        */
        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                ImmInt(1),
                And(0.into()),
                ImmInt(2),
                Or(1.into()),
                ImmInt(3),
                Put(a),
                Pop,
                Done
            ]
        );
        assert_eq!(binary.jump_label(Label(0)).position.0, 3);
        assert_eq!(binary.jump_label(Label(1)).position.0, 5);
    }

    #[test]
    fn test_unknown_builtin_call() {
        let program = "bad_builtin(1, 2, 3);";
        let binary = compile(program, CompileOptions::default()).unwrap();
        // Should compile successfully as a lambda call, not a builtin call
        // bad_builtin will be treated as a variable expression
        assert!(!binary.main_vector().is_empty());
    }

    #[test]
    fn test_known_builtin() {
        let program = "disassemble(player, \"test\");";
        let binary = compile(program, CompileOptions::default()).unwrap();

        let player = binary.find_var("player");
        let test = binary.find_label_for_literal("test".into());
        /*
         0: 072                   PUSH player
         1: 016                 * MAKE_SINGLETON_LIST
         2: 100 000               PUSH_LITERAL "test"
         4: 102                   LIST_ADD_TAIL
         5: 012 000             * CALL_FUNC disassemble
         7: 111                   POP
        */
        let disassemble = BUILTINS.find_builtin(Symbol::mk("disassemble")).unwrap();
        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                Push(player), // Player
                MakeSingletonList,
                Imm(test),
                ListAddTail,
                FuncCall { id: disassemble },
                Pop,
                Done
            ]
        );
    }

    #[test]
    fn test_cond_expr() {
        let program = "a = (1 == 2 ? 3 | 4);";
        let binary = compile(program, CompileOptions::default()).unwrap();

        let a = binary.find_var("a");

        /*
         0: 124                   NUM 1
         1: 125                   NUM 2
         2: 023                 * EQ
         3: 013 008             * IF_EXPR 8
         5: 126                   NUM 3
         6: 107 009               JUMP 9
         8: 127                   NUM 4
         9: 052                 * PUT a
        10: 111                   POP
        */
        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                ImmInt(1),
                ImmInt(2),
                Eq,
                IfQues(0.into()),
                ImmInt(3),
                Jump { label: 1.into() },
                ImmInt(4),
                Put(a),
                Pop,
                Done
            ]
        )
    }

    #[test]
    fn test_verb_call() {
        let program = "player:tell(\"test\");";
        let binary = compile(program, CompileOptions::default()).unwrap();

        let player = binary.find_var("player");
        let test = binary.find_label_for_literal("test".into());

        /*
              0: 072                   PUSH player
              1: 100 000               PUSH_LITERAL "tell"
              3: 100 001               PUSH_LITERAL "test"
              5: 016                 * MAKE_SINGLETON_LIST
              6: 010                 * CALL_VERB
              7: 111                   POP
        */
        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                Push(player), // Player
                ImmSymbol(Symbol::mk("tell")),
                Imm(test),
                MakeSingletonList,
                CallVerb,
                Pop,
                Done,
            ]
        );
    }

    #[test]
    fn test_string_get() {
        let program = "return \"test\"[1];";
        let binary = compile(program, CompileOptions::default()).unwrap();
        assert_eq!(
            binary.main_vector().to_vec(),
            vec![Imm(0.into()), ImmInt(1), Ref, Return, Pop, Done]
        );
    }

    #[test]
    fn test_string_get_range() {
        let program = "return \"test\"[1..2];";
        let binary = compile(program, CompileOptions::default()).unwrap();
        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                Imm(0.into()),
                ImmInt(1),
                ImmInt(2),
                RangeRef,
                Return,
                Pop,
                Done
            ]
        );
    }

    #[test]
    fn test_index_set() {
        let program = "a[2] = \"3\";";
        let binary = compile(program, CompileOptions::default()).unwrap();
        let a = binary.find_var("a");

        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                Push(a),
                ImmInt(2),
                Imm(0.into()),
                PutTemp,
                IndexSet,
                Put(a),
                Pop,
                PushTemp,
                Pop,
                Done
            ]
        );
    }

    #[test]
    fn test_range_set() {
        let program = "a[2..4] = \"345\";";
        let binary = compile(program, CompileOptions::default()).unwrap();
        let a = binary.find_var("a");

        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                Push(a),
                ImmInt(2),
                ImmInt(4),
                Imm(0.into()),
                PutTemp,
                RangeSet,
                Put(a),
                Pop,
                PushTemp,
                Pop,
                Done
            ]
        );
    }

    #[test]
    fn test_list_get() {
        let program = "return {1,2,3}[1];";
        let binary = compile(program, CompileOptions::default()).unwrap();
        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                ImmInt(1),
                MakeSingletonList,
                ImmInt(2),
                ListAddTail,
                ImmInt(3),
                ListAddTail,
                ImmInt(1),
                Ref,
                Return,
                Pop,
                Done
            ]
        );
    }

    #[test]
    fn test_list_get_range() {
        let program = "return {1,2,3}[1..2];";
        let binary = compile(program, CompileOptions::default()).unwrap();
        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                ImmInt(1),
                MakeSingletonList,
                ImmInt(2),
                ListAddTail,
                ImmInt(3),
                ListAddTail,
                ImmInt(1),
                ImmInt(2),
                RangeRef,
                Return,
                Pop,
                Done
            ]
        );
    }

    #[test]
    fn test_list_get_last() {
        let program = "return {1,2,3}[$];";
        let binary = compile(program, CompileOptions::default()).unwrap();
        assert!(
            binary
                .main_vector()
                .to_vec()
                .iter()
                .any(|op| matches!(op, Length(_))),
            "Expected $ index to emit Length opcode"
        );
    }

    #[test]
    fn test_range_length() {
        let program = "a = {1, 2, 3}; b = a[2..$];";
        let binary = compile(program, CompileOptions::default()).unwrap();

        let a = binary.find_var("a");
        let b = binary.find_var("b");

        /*
         0: 124                   NUM 1
         1: 016                 * MAKE_SINGLETON_LIST
         2: 125                   NUM 2
         3: 102                   LIST_ADD_TAIL
         4: 126                   NUM 3
         5: 102                   LIST_ADD_TAIL
         6: 052                 * PUT a
         7: 111                   POP
         8: 085                   PUSH a
         9: 125                   NUM 2
        10: 112 001 000           LENGTH 0
        13: 015                 * RANGE
        14: 053                 * PUT b
        15: 111                   POP
        16: 123                   NUM 0
        17: 030 021             * AND 21
                */
        assert_eq!(
            binary.main_vector().to_vec(),
            [
                ImmInt(1),
                MakeSingletonList,
                ImmInt(2),
                ListAddTail,
                ImmInt(3),
                ListAddTail,
                Put(a),
                Pop,
                Push(a),
                ImmInt(2),
                Length(0.into()),
                RangeRef,
                Put(b),
                Pop,
                Done
            ]
        );
    }

    #[test]
    fn test_range_length_end_expression() {
        let program = "a = {1, 2, 3}; b = a[1..$ - 2];";
        let binary = compile(program, CompileOptions::default()).unwrap();

        assert!(
            binary
                .main_vector()
                .to_vec()
                .iter()
                .any(|op| matches!(op, Length(_))),
            "Expected range end expression to emit Length opcode"
        );
    }

    #[test]
    fn test_list_splice() {
        let program = "return {@args[1..2]};";
        let binary = compile(program, CompileOptions::default()).unwrap();

        /*
          0: 076                   PUSH args
          1: 124                   NUM 1
          2: 125                   NUM 2
          3: 015                 * RANGE
          4: 017                 * CHECK_LIST_FOR_SPLICE
          5: 108                   RETURN
         12: 110                   DONE
        */
        let args = binary.find_var("args");
        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                Push(args),
                ImmInt(1),
                ImmInt(2),
                RangeRef,
                CheckListForSplice,
                Return,
                Pop,
                Done
            ]
        );
    }

    #[test]
    fn test_try_finally() {
        let program = "try a=1; finally a=2; endtry";
        let binary = compile(program, CompileOptions::default()).unwrap();

        let a = binary.find_var("a");
        /*
         0: 112 009 008         * TRY_FINALLY 8
         3: 124                   NUM 1
         4: 052                 * PUT a
         5: 111                   POP
         6: 112 005               END_FINALLY
         8: 125                   NUM 2
         9: 052                 * PUT a
        10: 111                   POP
        11: 112 006               CONTINUE
        */
        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                TryFinally {
                    end_label: 0.into(),
                    environment_width: 0,
                },
                ImmInt(1),
                Put(a),
                Pop,
                EndFinally,
                ImmInt(2),
                Put(a),
                Pop,
                FinallyContinue,
                Done
            ]
        );
    }

    #[test]
    fn test_try_excepts() {
        let program = "try a=1; except a (E_INVARG) a=2; except b (E_PROPNF) a=3; endtry";
        let binary = compile(program, CompileOptions::default()).unwrap();

        let a = binary.find_var("a");
        let b = binary.find_var("b");

        /*
          0: 100 000               PUSH_LITERAL E_INVARG
          2: 016                 * MAKE_SINGLETON_LIST
          3: 112 002 021           PUSH_LABEL 21
          6: 100 001               PUSH_LITERAL E_PROPNF
          8: 016                 * MAKE_SINGLETON_LIST
          9: 112 002 028           PUSH_LABEL 28
         12: 112 008 002         * TRY_EXCEPT 2
         15: 124                   NUM 1
         16: 052                 * PUT a
         17: 111                   POP
         18: 112 004 033           END_EXCEPT 33
         21: 052                 * PUT a
         22: 111                   POP
         23: 125                   NUM 2
         24: 052                 * PUT a
         25: 111                   POP
         26: 107 033               JUMP 33
         28: 053                 * PUT b
         29: 111                   POP
         30: 126                   NUM 3
         31: 052                 * PUT a
         32: 111                   POP

        */
        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                ImmErr(E_INVARG),
                MakeSingletonList,
                PushCatchLabel(0.into()),
                ImmErr(E_PROPNF),
                MakeSingletonList,
                PushCatchLabel(1.into()),
                TryExcept {
                    num_excepts: 2,
                    environment_width: 0,
                    end_label: 2.into(),
                },
                ImmInt(1),
                Put(a),
                Pop,
                EndExcept(2.into()),
                Put(a),
                Pop,
                ImmInt(2),
                Put(a),
                Pop,
                Jump { label: 2.into() },
                Put(b),
                Pop,
                ImmInt(3),
                Put(a),
                Pop,
                Done
            ]
        );
    }

    #[test]
    fn test_catch_expr() {
        let program = "x = `x + 1 ! e_propnf, E_PERM => 17';";
        let binary = compile(program, CompileOptions::default()).unwrap();
        /*
         0: 100 000               PUSH_LITERAL E_PROPNF
         2: 016                 * MAKE_SINGLETON_LIST
         3: 100 001               PUSH_LITERAL E_PERM
         5: 102                   LIST_ADD_TAIL
         6: 112 002 017           PUSH_LABEL 17
         9: 112 007             * CATCH
        11: 085                   PUSH x
        12: 124                   NUM 1
        13: 021                 * ADD
        14: 112 003 019           END_CATCH 19
        17: 111                   POP
        18: 140                   NUM 17
        19: 052                 * PUT x
        20: 111                   POP

         */
        let x = binary.find_var("x");

        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                ImmErr(E_PROPNF),
                MakeSingletonList,
                ImmErr(E_PERM),
                ListAddTail,
                PushCatchLabel(0.into()),
                TryCatch {
                    handler_label: 0.into(),
                    end_label: 1.into(),
                },
                Push(x),
                ImmInt(1),
                Add,
                EndCatch(1.into()),
                Pop,
                ImmInt(17),
                Put(x),
                Pop,
                Done
            ]
        )
    }

    #[test]
    fn test_catch_any_expr() {
        let program = "return `raise(E_INVARG) ! ANY';";
        let binary = compile(program, CompileOptions::default()).unwrap();

        /*
          0: 123                   NUM 0
          1: 112 002 014           PUSH_LABEL 14
          4: 112 007             * CATCH
          6: 100 000               PUSH_LITERAL E_INVARG
          8: 016                 * MAKE_SINGLETON_LIST
          9: 012 004             * CALL_FUNC raise
         11: 112 003 016           END_CATCH 16
         14: 124                   NUM 1
         15: 014                 * INDEX
         16: 108                   RETURN
        */
        let raise = BUILTINS.find_builtin(Symbol::mk("raise")).unwrap();
        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                ImmInt(0),
                PushCatchLabel(0.into()),
                TryCatch {
                    handler_label: 0.into(),
                    end_label: 1.into(),
                },
                ImmErr(E_INVARG),
                MakeSingletonList,
                FuncCall { id: raise },
                EndCatch(1.into()),
                ImmInt(1),
                Ref,
                Return,
                Pop,
                Done
            ]
        )
    }

    #[test]
    fn test_sysobjref() {
        let program = "$string_utils:from_list(test_string);";
        let binary = compile(program, CompileOptions::default()).unwrap();

        let test_string = binary.find_var("test_string");
        /*
         0: 100 000               PUSH_LITERAL #0
         2: 100 001               PUSH_LITERAL "string_utils"
         4: 009                 * GET_PROP
         5: 100 002               PUSH_LITERAL "from_list"
         7: 085                   PUSH test_string
         8: 016                 * MAKE_SINGLETON_LIST
         9: 010                 * CALL_VERB
        */
        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                ImmObjid(SYSTEM_OBJECT),
                ImmSymbol(Symbol::mk("string_utils")),
                GetProp,
                ImmSymbol(Symbol::mk("from_list")),
                Push(test_string),
                MakeSingletonList,
                CallVerb,
                Pop,
                Done
            ]
        )
    }

    #[test]
    fn test_sysverbcall() {
        let program = "$verb_metadata(#1, 1);";
        let binary = compile(program, CompileOptions::default()).unwrap();
        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                ImmObjid(SYSTEM_OBJECT),
                ImmSymbol(Symbol::mk("verb_metadata")),
                ImmObjid(Obj::mk_id(1)),
                MakeSingletonList,
                ImmInt(1),
                ListAddTail,
                CallVerb,
                Pop,
                Done
            ]
        )
    }
    #[test]
    fn test_basic_scatter_assign() {
        let program = "{a, b, c} = args;";
        let binary = compile(program, CompileOptions::default()).unwrap();
        let (a, b, c) = (
            binary.find_var("a"),
            binary.find_var("b"),
            binary.find_var("c"),
        );
        /*
         0: 076                   PUSH args
         1: 112 013 001 001 002
            018 000 009         * SCATTER 3/3/4: args/0 9
         9: 111                   POP
        */

        assert_eq!(
            binary.main_vector().to_vec(),
            vec![Push(binary.find_var("args")), Scatter(Offset(0)), Pop, Done]
        );
        let sa = binary.scatter_table(Offset(0)).clone();
        assert_eq!(
            sa,
            ScatterArgs {
                labels: vec![
                    ScatterLabel::Required(a),
                    ScatterLabel::Required(b),
                    ScatterLabel::Required(c),
                ],
                done: 0.into()
            }
        );
    }

    #[test]
    fn test_more_scatter_assign() {
        let program = "{first, second, ?third = 0} = args;";
        let binary = compile(program, CompileOptions::default()).unwrap();
        let (first, second, third) = (
            binary.find_var("first"),
            binary.find_var("second"),
            binary.find_var("third"),
        );
        /*
          0: 076                   PUSH args
          1: 112 013 003 002 004
             018 000 019 000 020
             013 016             * SCATTER 3/2/4: first/0 second/0 third/13 16
         13: 123                   NUM 0
         14: 054                 * PUT third
         15: 111                   POP
        */
        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                Push(binary.find_var("args")),
                Scatter(Offset(0)),
                ImmInt(0),
                Put(binary.find_var("third")),
                Pop,
                Pop,
                Done
            ]
        );
        assert_eq!(
            binary.scatter_table(Offset(0)).clone(),
            ScatterArgs {
                labels: vec![
                    ScatterLabel::Required(first),
                    ScatterLabel::Required(second),
                    ScatterLabel::Optional(third, Some(0.into())),
                ],
                done: 1.into(),
            }
        )
    }

    #[test]
    fn test_some_more_scatter_assign() {
        let program = "{a, b, ?c = 8, @d} = args;";
        let binary = compile(program, CompileOptions::default()).unwrap();
        /*
         0: 076                   PUSH args
         1: 112 013 004 002 004
            018 000 019 000 020
            015 021 000 018     * SCATTER 4/2/4: a/0 b/0 c/15 d/0 18
        15: 131                   NUM 8
        16: 054                 * PUT c
        17: 111                   POP
        18: 111                   POP

                */
        let (a, b, c, d) = (
            binary.find_var("a"),
            binary.find_var("b"),
            binary.find_var("c"),
            binary.find_var("d"),
        );
        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                Push(binary.find_var("args")),
                Scatter(Offset(0)),
                ImmInt(8),
                Put(binary.find_var("c")),
                Pop,
                Pop,
                Done
            ]
        );
        assert_eq!(
            binary.scatter_table(Offset(0)).clone(),
            ScatterArgs {
                labels: vec![
                    ScatterLabel::Required(a),
                    ScatterLabel::Required(b),
                    ScatterLabel::Optional(c, Some(0.into())),
                    ScatterLabel::Rest(d),
                ],
                done: 1.into(),
            }
        )
    }

    #[test]
    fn test_even_more_scatter_assign() {
        let program = "{a, ?b, ?c = 8, @d, ?e = 9, f} = args;";
        let binary = compile(program, CompileOptions::default()).unwrap();
        let (a, b, c, d, e, f) = (
            binary.find_var("a"),
            binary.find_var("b"),
            binary.find_var("c"),
            binary.find_var("d"),
            binary.find_var("e"),
            binary.find_var("f"),
        );
        /*
          0: 076                   PUSH args
          1: 112 013 006 002 004
             018 000 019 001 020
             019 021 000 022 022
             023 000 025         * SCATTER 6/2/4: a/0 b/1 c/19 d/0 e/22 f/0 25
         19: 131                   NUM 8
         20: 054                 * PUT c
         21: 111                   POP
         22: 132                   NUM 9
         23: 056                 * PUT e
         24: 111                   POP
         25: 111                   POP
        */
        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                Push(binary.find_var("args")),
                Scatter(Offset(0)),
                ImmInt(8),
                Put(binary.find_var("c")),
                Pop,
                ImmInt(9),
                Put(binary.find_var("e")),
                Pop,
                Pop,
                Done
            ]
        );
        assert_eq!(
            binary.scatter_table(Offset(0)).clone(),
            ScatterArgs {
                labels: vec![
                    ScatterLabel::Required(a),
                    ScatterLabel::Optional(b, None),
                    ScatterLabel::Optional(c, Some(0.into())),
                    ScatterLabel::Rest(d),
                    ScatterLabel::Optional(e, Some(1.into())),
                    ScatterLabel::Required(f),
                ],
                done: 2.into(),
            }
        );
    }

    #[test]
    fn test_scatter_precedence() {
        let program = "{a,b,?c, @d} = {{1,2,player:kill(b)}}[1]; return {a,b,c};";
        let binary = compile(program, CompileOptions::default()).unwrap();
        let (a, b, c, d) = (
            binary.find_var("a"),
            binary.find_var("b"),
            binary.find_var("c"),
            binary.find_var("d"),
        );
        /*
         0: 124                   NUM 1
         1: 016                 * MAKE_SINGLETON_LIST
         2: 125                   NUM 2
         3: 102                   LIST_ADD_TAIL
         4: 072                   PUSH player
         5: 100 000               PUSH_LITERAL "kill"
         7: 086                   PUSH b
         8: 016                 * MAKE_SINGLETON_LIST
         9: 010                 * CALL_VERB
        10: 102                   LIST_ADD_TAIL
        11: 016                 * MAKE_SINGLETON_LIST
        12: 124                   NUM 1
        13: 014                 * INDEX
        14: 112 013 004 002 004
            018 000 019 000 020
            001 021 000 028     * SCATTER 4/2/4: a/0 b/0 c/1 d/0 28
        28: 111                   POP
        29: 085                   PUSH a
        30: 016                 * MAKE_SINGLETON_LIST
        31: 086                   PUSH b
        32: 102                   LIST_ADD_TAIL
        33: 087                   PUSH c
        34: 102                   LIST_ADD_TAIL
        35: 108                   RETURN
               */

        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                ImmInt(1),
                MakeSingletonList,
                ImmInt(2),
                ListAddTail,
                Push(binary.find_var("player")),
                ImmSymbol(Symbol::mk("kill")),
                Push(b),
                MakeSingletonList,
                CallVerb,
                ListAddTail,
                MakeSingletonList,
                ImmInt(1),
                Ref,
                Scatter(Offset(0)),
                Pop,
                Push(a),
                MakeSingletonList,
                Push(b),
                ListAddTail,
                Push(c),
                ListAddTail,
                Return,
                Pop,
                Done
            ]
        );
        assert_eq!(
            binary.scatter_table(Offset(0)).clone(),
            ScatterArgs {
                labels: vec![
                    ScatterLabel::Required(a),
                    ScatterLabel::Required(b),
                    ScatterLabel::Optional(c, None),
                    ScatterLabel::Rest(d),
                ],
                done: 0.into(),
            }
        );
    }

    #[test]
    fn test_indexed_assignment() {
        let program = r#"this.stack[5] = 5;"#;
        let binary = compile(program, CompileOptions::default()).unwrap();

        /*
                  0: 073                   PUSH this
                  1: 100 000               PUSH_LITERAL "stack"
                  3: 008                 * PUSH_GET_PROP
                  4: 128                   NUM 5
                  5: 128                   NUM 5
                  6: 105                   PUT_TEMP
                  7: 007                 * INDEXSET
                  8: 011                 * PUT_PROP
                  9: 111                   POP
                 10: 106                   PUSH_TEMP
        */
        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                Push(binary.find_var("this")),
                ImmSymbol(Symbol::mk("stack")),
                PushGetProp,
                ImmInt(5),
                ImmInt(5),
                PutTemp,
                IndexSet,
                PutProp,
                Pop,
                PushTemp,
                Pop,
                Done
            ]
        )
    }

    #[test]
    fn test_assignment_from_range() {
        let program = r#"x = 1; y = {1,2,3}; x = x + y[2];"#;
        let binary = compile(program, CompileOptions::default()).unwrap();

        let x = binary.find_var("x");
        let y = binary.find_var("y");

        /*
         0: 124                   NUM 1
         1: 052                 * PUT x
         2: 111                   POP
         3: 124                   NUM 1
         4: 016                 * MAKE_SINGLETON_LIST
         5: 125                   NUM 2
         6: 102                   LIST_ADD_TAIL
         7: 126                   NUM 3
         8: 102                   LIST_ADD_TAIL
         9: 053                 * PUT y
        10: 111                   POP
        11: 085                   PUSH x
        12: 086                   PUSH y
        13: 125                   NUM 2
        14: 014                 * INDEX
        15: 021                 * ADD
        16: 052                 * PUT x
        17: 111                   POP
        30: 110                   DONE
        */

        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                ImmInt(1),
                Put(x),
                Pop,
                ImmInt(1),
                MakeSingletonList,
                ImmInt(2),
                ListAddTail,
                ImmInt(3),
                ListAddTail,
                Put(y),
                Pop,
                Push(x),
                Push(y),
                ImmInt(2),
                Ref,
                Add,
                Put(x),
                Pop,
                Done
            ]
        )
    }

    #[test]
    fn test_get_property() {
        let program = r#"return this.stack;"#;
        let binary = compile(program, CompileOptions::default()).unwrap();

        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                Push(binary.find_var("this")),
                ImmSymbol(Symbol::mk("stack")),
                GetProp,
                Return,
                Pop,
                Done
            ]
        )
    }

    #[test]
    fn test_call_verb() {
        let program = r#"#0:test_verb();"#;
        let binary = compile(program, CompileOptions::default()).unwrap();
        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                ImmObjid(Obj::mk_id(0)),
                ImmSymbol(Symbol::mk("test_verb")),
                ImmEmptyList,
                CallVerb,
                Pop,
                Done
            ]
        )
    }

    #[test]
    fn test_0_arg_return() {
        let program = r#"return;"#;
        let binary = compile(program, CompileOptions::default()).unwrap();
        assert_eq!(binary.main_vector().to_vec(), vec![Return0, Pop, Done])
    }

    #[test]
    fn test_pass() {
        let program = r#"
            result = pass(@args);
            result = pass();
            result = pass(1,2,3,4);
            pass = blop;
            return pass;
        "#;
        let binary = compile(program, CompileOptions::default()).unwrap();
        let result = binary.find_var("result");
        let pass = binary.find_var("pass");
        let args = binary.find_var("args");
        let blop = binary.find_var("blop");
        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                Push(args),
                CheckListForSplice,
                Pass,
                Put(result),
                Pop,
                ImmEmptyList,
                Pass,
                Put(result),
                Pop,
                ImmInt(1),
                MakeSingletonList,
                ImmInt(2),
                ListAddTail,
                ImmInt(3),
                ListAddTail,
                ImmInt(4),
                ListAddTail,
                Pass,
                Put(result),
                Pop,
                Push(blop),
                Put(pass),
                Pop,
                Push(pass),
                Return,
                Pop,
                Done
            ]
        )
    }

    #[test]
    fn test_regression_length_expr_inside_try_except() {
        let program = compile(
            r#"
        try
          return "hello world"[2..$];
        except (E_RANGE)
        endtry
        "#,
            CompileOptions::default(),
        )
        .unwrap();

        /*
         0: 100 000               PUSH_LITERAL E_RANGE
         2: 016                 * MAKE_SINGLETON_LIST
         3: 112 002 020           PUSH_LABEL 20
         6: 112 008 001         * TRY_EXCEPT 1
         9: 100 001               PUSH_LITERAL "hello world"
        11: 125                   NUM 2
        12: 112 001 003           LENGTH 3
        15: 015                 * RANGE
        16: 108                   RETURN
        17: 112 004 021           END_EXCEPT 21
        20: 111                   POP
        33: 110                   DONE
                */
        assert_eq!(
            program.main_vector().to_vec(),
            vec![
                ImmErr(E_RANGE),
                MakeSingletonList,
                PushCatchLabel(Label(0)),
                TryExcept {
                    num_excepts: 1,
                    environment_width: 0,
                    end_label: 1.into(),
                },
                Imm(Label(0)),
                ImmInt(2),
                // Our offset is different because we don't count PushLabel in the stack.
                Length(Offset(0)),
                RangeRef,
                Return,
                Pop,
                EndExcept(Label(1)),
                Pop,
                Done
            ]
        );
    }

    #[test]
    fn test_catch_handler_regression() {
        let prg = "`this ! E_INVIND';";
        let binary = compile(prg, CompileOptions::default()).unwrap();
        let this = binary.find_var("this");

        /*
         0: 100 000               PUSH_LITERAL E_INVIND
         2: 016                 * MAKE_SINGLETON_LIST
         6: 112 002 015           PUSH_LABEL 15
         9: 112 007             * CATCH
        11: 073                   PUSH this
        12: 112 003 017           END_CATCH 17
        15: 124                   NUM 1
        16: 014                 * INDEX
        17: 111                   POP
        18: 123                   NUM 0
        19: 030 023             * AND 23
        */
        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                ImmErr(E_INVIND),
                MakeSingletonList,
                PushCatchLabel(0.into()),
                TryCatch {
                    handler_label: 0.into(),
                    end_label: 1.into(),
                },
                Push(this),
                EndCatch(1.into()),
                ImmInt(1),
                Ref,
                Pop,
                Done
            ]
        )
    }

    #[test]
    fn test_range_in_map_oddities() {
        let program = r#"return[ "a"->5, "b"->"another_seq"[1..$]];"#;
        let binary = compile(program, CompileOptions::default()).unwrap();
        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                MakeMap,
                Imm(Label(0)),
                ImmInt(5),
                MapInsert,
                Imm(Label(1)),
                Imm(Label(2)),
                ImmInt(1),
                Length(Offset(2)),
                RangeRef,
                MapInsert,
                Return,
                Pop,
                Done
            ]
        );
    }

    #[test]
    fn test_uuobjid_resolution() {
        // First test with a regular object ID to make sure the basic parsing works
        let program = "return #123;";
        let binary = compile(program, CompileOptions::default()).unwrap();
        assert_eq!(
            binary.main_vector().to_vec(),
            vec![ImmObjid(Obj::mk_id(123)), Return, Pop, Done]
        );

        // Now test with the UUID
        let program = "return #001A01-9901A71113;";
        let binary = compile(program, CompileOptions::default()).unwrap();

        // Parse the UUID string to create the expected Obj
        let uuid = UuObjid::from_uuid_string("001A01-9901A71113").unwrap();
        let expected_obj = Obj::mk_uuobjid(uuid);

        assert_eq!(
            binary.main_vector().to_vec(),
            vec![ImmObjid(expected_obj), Return, Pop, Done]
        );

        // Test verb call with UUID
        let program = "#001A01-9901A71113:tell(\"test\");";
        let binary = compile(program, CompileOptions::default()).unwrap();

        let test = binary.find_label_for_literal("test".into());

        assert_eq!(
            binary.main_vector().to_vec(),
            vec![
                ImmObjid(expected_obj),
                ImmSymbol(Symbol::mk("tell")),
                Imm(test),
                MakeSingletonList,
                CallVerb,
                Pop,
                Done
            ]
        );
    }

    /// If the same variable is used in two different loops, the compiler was not handling the
    /// break label correctly.
    #[test]
    fn regression_test_for_in_list_break() {
        let program =
            r#"for x in ({1,2,3,4}) break x; endfor for x in ({1,2,3,4}) break x; endfor"#;
        compile(program, CompileOptions::default()).unwrap();
    }

    #[test]
    fn regression_test_for_in_range_break() {
        let program = r#"for x in [1..5] break x; endfor for x in [1..5] break x; endfor"#;
        compile(program, CompileOptions::default()).unwrap();
    }

    #[test]
    fn regression_scatter_prec() {
        let program = r#"{x} = y = z;"#;
        compile(program, CompileOptions::default()).unwrap();
    }

    #[test]
    fn regression_invalid_range_end_expression() {
        use moor_common::model::CompileError;

        let result = compile("$;", CompileOptions::default());
        assert!(result.is_err(), "Expected parse error for bare '$'");
        assert!(
            matches!(result, Err(CompileError::ParseError { .. })),
            "Expected ParseError for bare '$', got {result:?}"
        );
    }

    /// Test that assigning to a captured variable in a lambda produces a compile error.
    /// Lambdas capture by value, so mutation would not propagate to the outer scope.
    #[test]
    fn test_lambda_assign_to_captured_variable_error() {
        use moor_common::model::CompileError;

        // Arrow lambda assigning to captured variable
        let program = r#"x = 5; let f = {} => x = 10;"#;
        let result = compile(program, CompileOptions::default());
        assert!(
            result.is_err(),
            "Should error when assigning to captured variable"
        );
        if let Err(CompileError::AssignmentToCapturedVariable(_, sym)) = result {
            assert_eq!(sym.as_string(), "x");
        } else {
            panic!(
                "Expected AssignmentToCapturedVariable error, got {:?}",
                result
            );
        }

        // fn-style lambda assigning to captured variable
        let program = r#"x = 5; let f = fn() x = 10; endfn;"#;
        let result = compile(program, CompileOptions::default());
        assert!(
            result.is_err(),
            "Should error when assigning to captured variable in fn lambda"
        );
        if let Err(CompileError::AssignmentToCapturedVariable(_, sym)) = result {
            assert_eq!(sym.as_string(), "x");
        } else {
            panic!(
                "Expected AssignmentToCapturedVariable error, got {:?}",
                result
            );
        }

        // Scatter assignment to captured variable
        let program = r#"x = 5; let f = fn() {x} = {10}; endfn;"#;
        let result = compile(program, CompileOptions::default());
        assert!(
            result.is_err(),
            "Should error when scatter-assigning to captured variable"
        );
        if let Err(CompileError::AssignmentToCapturedVariable(_, sym)) = result {
            assert_eq!(sym.as_string(), "x");
        } else {
            panic!(
                "Expected AssignmentToCapturedVariable error, got {:?}",
                result
            );
        }
    }

    /// Test that reading a captured variable (without assignment) still works.
    #[test]
    fn test_lambda_read_captured_variable_ok() {
        // Reading captured variable should be fine
        let program = r#"x = 5; let f = {} => x + 1;"#;
        compile(program, CompileOptions::default()).unwrap();

        // Multiple reads should be fine
        let program = r#"x = 5; y = 10; let f = {} => x + y;"#;
        compile(program, CompileOptions::default()).unwrap();
    }

    /// Test that assigning to local variables inside a lambda is fine.
    #[test]
    fn test_lambda_assign_to_local_variable_ok() {
        // Assigning to a lambda parameter is fine
        let program = r#"let f = {x} => x = x + 1;"#;
        compile(program, CompileOptions::default()).unwrap();

        // Assigning to a local variable declared inside the lambda is fine (using let)
        let program = r#"let f = fn() let local = 5; local = 10; endfn;"#;
        compile(program, CompileOptions::default()).unwrap();
    }

    /// Test that shadowing a captured variable with `let` allows assignment to the shadow.
    #[test]
    fn test_lambda_shadow_captured_variable_ok() {
        // Outer x exists, but lambda declares its own x with let, then assigns to it
        let program = r#"x = 5; let f = fn() let x = 1; x = 2; return x; endfn;"#;
        compile(program, CompileOptions::default()).unwrap();

        // Multiple levels of shadowing
        let program =
            r#"x = 5; let f = fn() let x = 1; let g = fn() let x = 2; x = 3; endfn; endfn;"#;
        compile(program, CompileOptions::default()).unwrap();
    }

    /// Test that implicitly-declared variables (via assignment without `let`) inside
    /// a lambda are NOT treated as captured variables.
    /// Regression test for issue where `val = expr` inside a lambda was incorrectly
    /// flagged as "assignment to captured variable" when `val` didn't exist in outer scope.
    #[test]
    fn test_lambda_implicit_local_variable_ok() {
        // Variable implicitly declared via assignment inside lambda body
        let program = r#"
            fn get_sysref(o)
                for prop in (properties(#0))
                    val = `#0.(prop) ! ANY => 0';
                    if (val == o)
                        return prop;
                    endif
                endfor
                return "";
            endfn
        "#;
        compile(program, CompileOptions::default()).unwrap();

        // Simpler case: just assign to a new variable inside lambda
        let program = r#"let f = fn() val = 5; return val; endfn;"#;
        compile(program, CompileOptions::default()).unwrap();

        // Lambda with parameter and implicit variable
        // Note: Arrow lambdas are expression-only, use fn()...endfn for multi-statement bodies
        let program = r#"let f = fn(x) val = x * 2; return val; endfn;"#;
        compile(program, CompileOptions::default()).unwrap();
    }
}
