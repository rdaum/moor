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

//! Property-based testing for MOO VM opcodes.
//!
//! This module provides proptest strategies for generating opcode sequences
//! and verifying VM behavior properties like:
//! - No crashes on any valid opcode sequence
//! - Proper error handling for invalid operations
//! - Stack invariants maintained during execution

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use moor_common::{
        model::{ObjectKind, VerbArgsSpec, VerbFlag, WorldStateSource},
        tasks::NoopClientSession,
        util::BitEnum,
    };
    use moor_compiler::{Names, Op, Program};
    use moor_db::{DatabaseConfig, TxDB};
    use moor_var::{
        List, Symbol, Var, NOTHING, SYSTEM_OBJECT,
        program::{ProgramType, labels::{JumpLabel, Label, Offset}, program::PrgInner},
        v_int, v_str,
    };
    use proptest::prelude::*;

    use crate::{testing::vm_test_utils::call_verb, vm::builtins::BuiltinRegistry};

    /// Create a Program from an opcode sequence
    fn mk_program(main_vector: Vec<Op>, literals: Vec<Var>, var_names: Names) -> Program {
        mk_program_with_labels(main_vector, literals, var_names, vec![])
    }

    /// Create a Program with jump labels for control flow
    /// jump_offsets: list of PC offsets for each label (index = label id, value = PC offset)
    fn mk_program_with_labels(
        main_vector: Vec<Op>,
        literals: Vec<Var>,
        var_names: Names,
        jump_offsets: Vec<u16>,
    ) -> Program {
        let jump_labels = jump_offsets
            .into_iter()
            .enumerate()
            .map(|(i, offset)| JumpLabel {
                id: Label(i as u16),
                name: None,
                position: Offset(offset),
            })
            .collect();
        Program(Arc::new(PrgInner {
            literals,
            jump_labels,
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

    /// Create a test database with a verb containing the given program
    fn test_db_with_verb(verb_name: &str, program: &Program) -> TxDB {
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

        tx.add_verb(
            &SYSTEM_OBJECT,
            &sysobj.clone(),
            vec![Symbol::mk(verb_name)],
            &sysobj.clone(),
            VerbFlag::rxd(),
            VerbArgsSpec::this_none_this(),
            ProgramType::MooR(program.clone()),
        )
        .unwrap();
        tx.commit().unwrap();
        state
    }

    /// Execute a program and return the result, catching panics
    fn execute_program_safe(program: &Program) -> Result<Var, String> {
        let state_source = test_db_with_verb("test", program);
        let state = state_source.new_world_state().unwrap();
        let session = Arc::new(NoopClientSession::new());

        // Use std::panic::catch_unwind to catch any panics
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            call_verb(
                state,
                session,
                BuiltinRegistry::new(),
                "test",
                List::mk_list(&[]),
            )
        }));

        match result {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err(format!("Exception: {:?}", e)),
            Err(panic) => {
                let msg = if let Some(s) = panic.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "Unknown panic".to_string()
                };
                Err(format!("Panic: {}", msg))
            }
        }
    }

    // =========================================================================
    // LAYER 1: Immediate Value Opcodes
    // =========================================================================

    /// Generate immediate integer opcodes
    fn arb_imm_int() -> impl Strategy<Value = Op> {
        any::<i32>().prop_map(Op::ImmInt)
    }

    /// Generate immediate float opcodes (finite values only)
    fn arb_imm_float() -> impl Strategy<Value = Op> {
        any::<f64>()
            .prop_filter("must be finite", |f| f.is_finite())
            .prop_map(Op::ImmFloat)
    }

    /// Generate simple immediate value opcodes
    fn arb_imm_simple() -> impl Strategy<Value = Op> {
        prop_oneof![
            arb_imm_int(),
            arb_imm_float(),
            Just(Op::ImmEmptyList),
            Just(Op::ImmNone),
        ]
    }

    /// Generate a binary arithmetic opcode
    fn arb_binary_arith_op() -> impl Strategy<Value = Op> {
        prop_oneof![
            Just(Op::Add),
            Just(Op::Sub),
            Just(Op::Mul),
            Just(Op::Div),
            Just(Op::Mod),
        ]
    }

    /// Generate a comparison opcode
    fn arb_comparison_op() -> impl Strategy<Value = Op> {
        prop_oneof![
            Just(Op::Eq),
            Just(Op::Ne),
            Just(Op::Lt),
            Just(Op::Le),
            Just(Op::Gt),
            Just(Op::Ge),
        ]
    }

    /// Generate a unary opcode
    fn arb_unary_op() -> impl Strategy<Value = Op> {
        prop_oneof![Just(Op::Not), Just(Op::UnaryMinus), Just(Op::BitNot),]
    }

    // =========================================================================
    // LAYER 1 TESTS: Stack-based operations with immediate values
    // =========================================================================

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Test that pushing immediate values and returning doesn't crash
        #[test]
        fn proptest_imm_return(value in arb_imm_simple()) {
            let program = mk_program(
                vec![value, Op::Return, Op::Done],
                vec![],
                Names::new(64),
            );
            let result = execute_program_safe(&program);
            // Should not panic - errors are acceptable
            prop_assert!(result.is_ok() || result.unwrap_err().starts_with("Exception"));
        }

        /// Test binary arithmetic with two immediate integers
        #[test]
        fn proptest_binary_arith_int(
            a in any::<i32>(),
            b in any::<i32>(),
            op in arb_binary_arith_op()
        ) {
            let program = mk_program(
                vec![Op::ImmInt(a), Op::ImmInt(b), op, Op::Return, Op::Done],
                vec![],
                Names::new(64),
            );
            let result = execute_program_safe(&program);
            // Should not panic - division by zero etc. should produce errors
            prop_assert!(result.is_ok() || result.unwrap_err().starts_with("Exception"));
        }

        /// Test comparison operations with integers
        #[test]
        fn proptest_comparison_int(
            a in any::<i32>(),
            b in any::<i32>(),
            op in arb_comparison_op()
        ) {
            let program = mk_program(
                vec![Op::ImmInt(a), Op::ImmInt(b), op, Op::Return, Op::Done],
                vec![],
                Names::new(64),
            );
            let result = execute_program_safe(&program);
            // Comparisons should always succeed
            prop_assert!(result.is_ok());
        }

        /// Test unary operations
        #[test]
        fn proptest_unary_int(value in any::<i32>(), op in arb_unary_op()) {
            let program = mk_program(
                vec![Op::ImmInt(value), op, Op::Return, Op::Done],
                vec![],
                Names::new(64),
            );
            let result = execute_program_safe(&program);
            // Should not panic
            prop_assert!(result.is_ok() || result.unwrap_err().starts_with("Exception"));
        }

        /// Test that Pop followed by Return0 works
        #[test]
        fn proptest_pop_return0(value in arb_imm_simple()) {
            let program = mk_program(
                vec![value, Op::Pop, Op::Return0, Op::Done],
                vec![],
                Names::new(64),
            );
            let result = execute_program_safe(&program);
            prop_assert!(result.is_ok());
        }
    }

    // =========================================================================
    // LAYER 2: Literal Table Access
    // =========================================================================

    /// Generate a sequence of opcodes that push literals and perform operations
    fn arb_literal_program() -> impl Strategy<Value = (Vec<Op>, Vec<Var>)> {
        // Generate 1-5 literal values
        prop::collection::vec(
            prop_oneof![
                any::<i32>().prop_map(|i| v_int(i as i64)),
                any::<i32>().prop_map(|i| v_str(&format!("str{}", i))),
            ],
            1..=5,
        )
        .prop_flat_map(|literals| {
            let num_literals = literals.len();
            // Generate opcodes that reference these literals
            prop::collection::vec(0..num_literals, 1..=3).prop_map(move |indices| {
                let mut ops = Vec::new();
                for idx in indices {
                    ops.push(Op::Imm(idx.into()));
                }
                // Pop all but one, then return
                for _ in 1..ops.len() {
                    ops.push(Op::Pop);
                }
                ops.push(Op::Return);
                ops.push(Op::Done);
                (ops, literals.clone())
            })
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Test literal table access
        #[test]
        fn proptest_literal_access((ops, literals) in arb_literal_program()) {
            let program = mk_program(ops, literals, Names::new(64));
            let result = execute_program_safe(&program);
            prop_assert!(result.is_ok());
        }
    }

    // =========================================================================
    // LAYER 3: List and Map Operations
    // =========================================================================

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Test MakeSingletonList
        #[test]
        fn proptest_make_singleton_list(value in any::<i32>()) {
            let program = mk_program(
                vec![
                    Op::ImmInt(value),
                    Op::MakeSingletonList,
                    Op::Return,
                    Op::Done,
                ],
                vec![],
                Names::new(64),
            );
            let result = execute_program_safe(&program);
            prop_assert!(result.is_ok());
        }

        /// Test ListAppend
        #[test]
        fn proptest_list_append(a in any::<i32>(), b in any::<i32>()) {
            let program = mk_program(
                vec![
                    Op::ImmInt(a),
                    Op::MakeSingletonList,
                    Op::ImmInt(b),
                    Op::ListAddTail,
                    Op::Return,
                    Op::Done,
                ],
                vec![],
                Names::new(64),
            );
            let result = execute_program_safe(&program);
            prop_assert!(result.is_ok());
        }

        /// Test MakeMap and MapInsert
        #[test]
        fn proptest_map_ops(key in any::<i32>(), value in any::<i32>()) {
            let program = mk_program(
                vec![
                    Op::MakeMap,
                    Op::ImmInt(key),
                    Op::ImmInt(value),
                    Op::MapInsert,
                    Op::Return,
                    Op::Done,
                ],
                vec![],
                Names::new(64),
            );
            let result = execute_program_safe(&program);
            prop_assert!(result.is_ok());
        }
    }

    // =========================================================================
    // LAYER 4: Indexing Operations
    // =========================================================================

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Test list indexing (Ref) - may produce E_RANGE for out-of-bounds
        #[test]
        fn proptest_list_index(values in prop::collection::vec(any::<i32>(), 1..=5), idx in 1i32..=10) {
            let mut ops = vec![Op::ImmEmptyList];
            for v in &values {
                ops.push(Op::ImmInt(*v));
                ops.push(Op::ListAddTail);
            }
            ops.push(Op::ImmInt(idx));
            ops.push(Op::Ref);
            ops.push(Op::Return);
            ops.push(Op::Done);

            let program = mk_program(ops, vec![], Names::new(64));
            let result = execute_program_safe(&program);
            // Should not panic - E_RANGE is acceptable for out-of-bounds
            prop_assert!(result.is_ok() || result.unwrap_err().starts_with("Exception"));
        }

        /// Test string indexing
        #[test]
        fn proptest_string_index(idx in 1i32..=20) {
            let program = mk_program(
                vec![
                    Op::Imm(0.into()),  // "hello"
                    Op::ImmInt(idx),
                    Op::Ref,
                    Op::Return,
                    Op::Done,
                ],
                vec![v_str("hello")],
                Names::new(64),
            );
            let result = execute_program_safe(&program);
            // E_RANGE for out-of-bounds is acceptable
            prop_assert!(result.is_ok() || result.unwrap_err().starts_with("Exception"));
        }
    }

    // =========================================================================
    // Manual regression tests for specific edge cases
    // =========================================================================

    #[test]
    fn test_empty_program() {
        let program = mk_program(vec![Op::Done], vec![], Names::new(64));
        let result = execute_program_safe(&program);
        assert!(result.is_ok());
    }

    #[test]
    fn test_return_without_value() {
        let program = mk_program(vec![Op::Return0, Op::Done], vec![], Names::new(64));
        let result = execute_program_safe(&program);
        assert!(result.is_ok());
    }

    #[test]
    fn test_division_by_zero() {
        let program = mk_program(
            vec![Op::ImmInt(1), Op::ImmInt(0), Op::Div, Op::Return, Op::Done],
            vec![],
            Names::new(64),
        );
        let result = execute_program_safe(&program);
        // Should produce E_DIV, not panic
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Exception"));
    }

    #[test]
    fn test_type_operations_no_panic() {
        // Test that type-mixed operations don't crash the VM
        // They may succeed with coercion or produce exceptions - either is acceptable
        let program = mk_program(
            vec![
                Op::Imm(0.into()), // String "hello"
                Op::ImmInt(1),
                Op::Sub,
                Op::Return,
                Op::Done,
            ],
            vec![v_str("hello")],
            Names::new(64),
        );
        let result = execute_program_safe(&program);
        // The key invariant: no panics - either success or exception
        assert!(result.is_ok() || result.unwrap_err().starts_with("Exception"));
    }

    // =========================================================================
    // LAYER 5: Control Flow
    // =========================================================================

    #[test]
    fn test_simple_if_true() {
        // if (1) return 42; else return 0; endif
        // Jump labels: label 0 -> PC 4 (else branch)
        let program = mk_program_with_labels(
            vec![
                Op::ImmInt(1),        // 0: condition (true)
                Op::If(0.into(), 0),  // 1: if false, jump to label 0
                Op::ImmInt(42),       // 2: true branch value
                Op::Return,           // 3: return from true branch
                Op::ImmInt(0),        // 4: false branch value (label 0 target)
                Op::Return,           // 5: return from false branch
                Op::Done,             // 6: end
            ],
            vec![],
            Names::new(64),
            vec![4], // label 0 -> PC 4
        );
        let result = execute_program_safe(&program);
        assert!(result.is_ok());
    }

    #[test]
    fn test_simple_if_false() {
        // if (0) return 42; else return 0; endif
        let program = mk_program_with_labels(
            vec![
                Op::ImmInt(0),        // 0: condition (false)
                Op::If(0.into(), 0),  // 1: if false, jump to label 0
                Op::ImmInt(42),       // 2: true branch (skipped)
                Op::Return,           // 3: return (skipped)
                Op::ImmInt(0),        // 4: false branch (label 0 target)
                Op::Return,           // 5: return from false branch
                Op::Done,             // 6: end
            ],
            vec![],
            Names::new(64),
            vec![4], // label 0 -> PC 4
        );
        let result = execute_program_safe(&program);
        assert!(result.is_ok());
    }

    #[test]
    fn test_unconditional_jump() {
        // Jump over some code
        let program = mk_program_with_labels(
            vec![
                Op::Jump { label: 0.into() }, // 0: jump to label 0
                Op::ImmInt(999),              // 1: skipped
                Op::Return,                   // 2: skipped
                Op::ImmInt(42),               // 3: label 0 target
                Op::Return,                   // 4: return 42
                Op::Done,                     // 5: end
            ],
            vec![],
            Names::new(64),
            vec![3], // label 0 -> PC 3
        );
        let result = execute_program_safe(&program);
        assert!(result.is_ok());
    }

    // =========================================================================
    // LAYER 6: Variable Access
    // =========================================================================

    #[test]
    fn test_variable_put_push() {
        use moor_var::program::names::Name;

        // x = 42; return x;
        // Name(offset, scope_depth, scope_id)
        // Offset 11 is after the global variables (player, this, caller, etc.)
        let x_name = Name(11, 0, 0);

        let program = mk_program(
            vec![
                Op::ImmInt(42),       // 0: push 42
                Op::Put(x_name),      // 1: x = 42
                Op::Pop,              // 2: discard dup from Put
                Op::Push(x_name),     // 3: push x
                Op::Return,           // 4: return x
                Op::Done,             // 5: end
            ],
            vec![],
            Names::new(64),
        );
        let result = execute_program_safe(&program);
        assert!(result.is_ok());
    }

    #[test]
    fn test_multiple_variables() {
        use moor_var::program::names::Name;

        // a = 10; b = 20; return a + b;
        let a_name = Name(11, 0, 0);
        let b_name = Name(12, 0, 0);

        let program = mk_program(
            vec![
                Op::ImmInt(10),       // 0: push 10
                Op::Put(a_name),      // 1: a = 10
                Op::Pop,              // 2: discard
                Op::ImmInt(20),       // 3: push 20
                Op::Put(b_name),      // 4: b = 20
                Op::Pop,              // 5: discard
                Op::Push(a_name),     // 6: push a
                Op::Push(b_name),     // 7: push b
                Op::Add,              // 8: a + b
                Op::Return,           // 9: return result
                Op::Done,             // 10: end
            ],
            vec![],
            Names::new(64),
        );
        let result = execute_program_safe(&program);
        assert!(result.is_ok());
    }

    // =========================================================================
    // LAYER 7: Bitwise Operations
    // =========================================================================

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]

        /// Test bitwise AND
        #[test]
        fn proptest_bit_and(a in any::<i32>(), b in any::<i32>()) {
            let program = mk_program(
                vec![Op::ImmInt(a), Op::ImmInt(b), Op::BitAnd, Op::Return, Op::Done],
                vec![],
                Names::new(64),
            );
            let result = execute_program_safe(&program);
            prop_assert!(result.is_ok());
        }

        /// Test bitwise OR
        #[test]
        fn proptest_bit_or(a in any::<i32>(), b in any::<i32>()) {
            let program = mk_program(
                vec![Op::ImmInt(a), Op::ImmInt(b), Op::BitOr, Op::Return, Op::Done],
                vec![],
                Names::new(64),
            );
            let result = execute_program_safe(&program);
            prop_assert!(result.is_ok());
        }

        /// Test bitwise XOR
        #[test]
        fn proptest_bit_xor(a in any::<i32>(), b in any::<i32>()) {
            let program = mk_program(
                vec![Op::ImmInt(a), Op::ImmInt(b), Op::BitXor, Op::Return, Op::Done],
                vec![],
                Names::new(64),
            );
            let result = execute_program_safe(&program);
            prop_assert!(result.is_ok());
        }

        /// Test bit shifts
        #[test]
        fn proptest_bit_shift(a in any::<i32>(), b in 0i32..64) {
            let program = mk_program(
                vec![Op::ImmInt(a), Op::ImmInt(b), Op::BitShl, Op::Return, Op::Done],
                vec![],
                Names::new(64),
            );
            let result = execute_program_safe(&program);
            prop_assert!(result.is_ok() || result.unwrap_err().starts_with("Exception"));
        }
    }

    // =========================================================================
    // LAYER 8: Logical Operations
    // =========================================================================

    #[test]
    fn test_and_short_circuit_true() {
        // 1 && 2 -> should evaluate both, return 2
        let program = mk_program_with_labels(
            vec![
                Op::ImmInt(1),        // 0: first operand (truthy)
                Op::And(0.into()),    // 1: if false, jump to label 0
                Op::ImmInt(2),        // 2: second operand
                Op::Return,           // 3: return second operand
                Op::Return,           // 4: label 0 - return first (false case)
                Op::Done,             // 5: end
            ],
            vec![],
            Names::new(64),
            vec![4], // label 0 -> PC 4
        );
        let result = execute_program_safe(&program);
        assert!(result.is_ok());
    }

    #[test]
    fn test_and_short_circuit_false() {
        // 0 && 2 -> should short-circuit, return 0
        let program = mk_program_with_labels(
            vec![
                Op::ImmInt(0),        // 0: first operand (falsy)
                Op::And(0.into()),    // 1: if false, jump to label 0
                Op::ImmInt(2),        // 2: second operand (skipped)
                Op::Return,           // 3: return (skipped)
                Op::Return,           // 4: label 0 - return first (0)
                Op::Done,             // 5: end
            ],
            vec![],
            Names::new(64),
            vec![4], // label 0 -> PC 4
        );
        let result = execute_program_safe(&program);
        assert!(result.is_ok());
    }

    #[test]
    fn test_or_short_circuit_true() {
        // 1 || 2 -> should short-circuit, return 1
        let program = mk_program_with_labels(
            vec![
                Op::ImmInt(1),        // 0: first operand (truthy)
                Op::Or(0.into()),     // 1: if true, jump to label 0
                Op::ImmInt(2),        // 2: second operand (skipped)
                Op::Return,           // 3: return (skipped)
                Op::Return,           // 4: label 0 - return first (1)
                Op::Done,             // 5: end
            ],
            vec![],
            Names::new(64),
            vec![4], // label 0 -> PC 4
        );
        let result = execute_program_safe(&program);
        assert!(result.is_ok());
    }

    #[test]
    fn test_or_short_circuit_false() {
        // 0 || 2 -> should evaluate second, return 2
        let program = mk_program_with_labels(
            vec![
                Op::ImmInt(0),        // 0: first operand (falsy)
                Op::Or(0.into()),     // 1: if true, jump to label 0 (not taken)
                Op::ImmInt(2),        // 2: second operand
                Op::Return,           // 3: return second
                Op::Return,           // 4: label 0 (not reached)
                Op::Done,             // 5: end
            ],
            vec![],
            Names::new(64),
            vec![4], // label 0 -> PC 4
        );
        let result = execute_program_safe(&program);
        assert!(result.is_ok());
    }

    // =========================================================================
    // LAYER 9: Compiler Oracle Tests
    // Compile MOO source, execute, verify results match expectations
    // =========================================================================

    /// Compile a MOO program from source and execute it safely
    fn compile_and_execute(source: &str) -> Result<Var, String> {
        use moor_compiler::{CompileOptions, compile};

        let program = compile(source, CompileOptions::default())
            .map_err(|e| format!("Compile error: {:?}", e))?;

        let state_source = test_db_with_verb("test", &program);
        let state = state_source.new_world_state().unwrap();
        let session = Arc::new(NoopClientSession::new());

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            call_verb(
                state,
                session,
                BuiltinRegistry::new(),
                "test",
                List::mk_list(&[]),
            )
        }));

        match result {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err(format!("Exception: {:?}", e)),
            Err(panic) => {
                let msg = if let Some(s) = panic.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "Unknown panic".to_string()
                };
                Err(format!("Panic: {}", msg))
            }
        }
    }

    #[test]
    fn test_oracle_simple_return() {
        let result = compile_and_execute("return 42;");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), v_int(42));
    }

    #[test]
    fn test_oracle_arithmetic() {
        let result = compile_and_execute("return 10 + 20 * 2;");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), v_int(50));
    }

    #[test]
    fn test_oracle_string_concat() {
        let result = compile_and_execute(r#"return "hello" + " " + "world";"#);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), v_str("hello world"));
    }

    #[test]
    fn test_oracle_list_operations() {
        let result = compile_and_execute("return {1, 2, 3}[2];");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), v_int(2));
    }

    #[test]
    fn test_oracle_if_true() {
        let result = compile_and_execute("if (1) return 42; else return 0; endif");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), v_int(42));
    }

    #[test]
    fn test_oracle_if_false() {
        let result = compile_and_execute("if (0) return 42; else return 0; endif");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), v_int(0));
    }

    #[test]
    fn test_oracle_for_loop() {
        let result = compile_and_execute(r#"
            sum = 0;
            for i in ({1, 2, 3, 4, 5})
                sum = sum + i;
            endfor
            return sum;
        "#);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), v_int(15));
    }

    #[test]
    fn test_oracle_while_loop() {
        let result = compile_and_execute(r#"
            i = 0;
            while (i < 5)
                i = i + 1;
            endwhile
            return i;
        "#);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), v_int(5));
    }

    #[test]
    fn test_oracle_nested_lists() {
        let result = compile_and_execute("return {{1, 2}, {3, 4}}[2][1];");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), v_int(3));
    }

    #[test]
    fn test_oracle_comparison() {
        let result = compile_and_execute("return 5 > 3 && 2 < 4;");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), v_int(1));
    }

    #[test]
    fn test_oracle_ternary() {
        let result = compile_and_execute("return 1 ? 42 | 0;");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), v_int(42));
    }

    #[test]
    fn test_oracle_string_index() {
        let result = compile_and_execute(r#"return "hello"[2];"#);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), v_str("e"));
    }

    #[test]
    fn test_oracle_string_range() {
        let result = compile_and_execute(r#"return "hello"[2..4];"#);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), v_str("ell"));
    }

    #[test]
    fn test_oracle_map_literal() {
        let result = compile_and_execute(r#"return ["a" -> 1, "b" -> 2]["a"];"#);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), v_int(1));
    }

    // Proptest: Generate random arithmetic expressions and verify no crashes
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]

        #[test]
        fn proptest_oracle_arithmetic(a in -1000i32..1000, b in -1000i32..1000) {
            let source = format!("return {} + {};", a, b);
            let result = compile_and_execute(&source);
            prop_assert!(result.is_ok());
            prop_assert_eq!(result.unwrap(), v_int((a as i64) + (b as i64)));
        }

        #[test]
        fn proptest_oracle_multiply(a in -100i32..100, b in -100i32..100) {
            let source = format!("return {} * {};", a, b);
            let result = compile_and_execute(&source);
            prop_assert!(result.is_ok());
            prop_assert_eq!(result.unwrap(), v_int((a as i64) * (b as i64)));
        }

        #[test]
        fn proptest_oracle_comparison(a in -1000i32..1000, b in -1000i32..1000) {
            let source = format!("return {} < {};", a, b);
            let result = compile_and_execute(&source);
            prop_assert!(result.is_ok());
            prop_assert_eq!(result.unwrap(), v_int(if a < b { 1 } else { 0 }));
        }

        #[test]
        fn proptest_oracle_list_length(len in 0usize..10) {
            let elements: Vec<String> = (0..len).map(|i| i.to_string()).collect();
            let source = format!("return length({{{}}});", elements.join(", "));
            let result = compile_and_execute(&source);
            prop_assert!(result.is_ok());
            prop_assert_eq!(result.unwrap(), v_int(len as i64));
        }
    }
}
