# Proptest Opcode Fuzz Testing Design

## Overview

Property-based testing for MOO VM opcodes to verify:
1. Opcode sequences don't crash the VM
2. Opcode execution produces consistent results
3. Stack operations maintain invariants
4. Error handling is robust

## Key Files and Structures

### Opcode Definition
**File:** `crates/var/src/program/opcode.rs`

```rust
// Lines 24-176: Op enum with all opcodes
#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub enum Op {
    Add,                    // Binary arithmetic
    And(Label),             // Short-circuit with jump label
    BitAnd, BitOr, BitXor,  // Bitwise operations
    CallVerb,               // Method dispatch
    Div, Mul, Sub, Mod, Exp,// Arithmetic
    Done,                   // End execution
    Eq, Ne, Lt, Le, Gt, Ge, // Comparisons
    Fork { fv_offset, id }, // Task forking
    FuncCall { id },        // Builtin function call
    GetProp, PutProp,       // Property access
    If(Label, u16),         // Conditional
    Imm(Label),             // Load literal from table
    ImmInt(i32),            // Immediate integer
    ImmFloat(f64),          // Immediate float
    ImmEmptyList,           // Empty list literal
    Jump { label },         // Unconditional jump
    Length(Offset),         // Get sequence length
    ListAppend, ListAddTail,// List operations
    MakeSingletonList,      // Create [value]
    MakeMap, MapInsert,     // Map operations
    Not, UnaryMinus,        // Unary operators
    Pop,                    // Discard top of stack
    Push(Name), Put(Name),  // Variable access
    Ref, RangeRef,          // Indexing
    Return, Return0,        // Return from verb
    Scatter(Offset),        // Destructuring
    // ... control flow, scopes, lambdas
}
```

### Supporting Types
**File:** `crates/var/src/program/labels.rs`
- `Label`: u16 jump target
- `Offset`: u16 table offset

**File:** `crates/var/src/program/names.rs`
- `Name`: (u16, u16, u16) = (id, scope_depth, scope_id)

### Program Structure
**File:** `crates/var/src/program/program.rs`

```rust
pub struct PrgInner {
    pub literals: Vec<Var>,           // Literal values table
    pub jump_labels: Vec<u16>,        // Jump target addresses
    pub var_names: Names,             // Variable name mappings
    pub scatter_tables: Vec<...>,     // Scatter assignment specs
    pub for_sequence_operands: Vec<ForSequenceOperand>,
    pub for_range_operands: Vec<ForRangeOperand>,
    pub main_vector: Vec<Op>,         // Main opcode stream
    pub fork_vectors: Vec<Vec<Op>>,   // Forked task opcodes
    // ...
}
```

### VM Execution
**File:** `crates/kernel/src/vm/moo_execute.rs`

```rust
// Line 130: Main execution function
pub fn moo_frame_execute(
    tick_slice: usize,
    tick_count: &mut usize,
    permissions: Obj,
    f: &mut MooStackFrame,
    features_config: &FeaturesConfig,
) -> ExecutionResult
```

### Test Infrastructure
**File:** `crates/kernel/src/testing/vm_test.rs`

```rust
// Lines 35-52: Create Program from opcode sequence
fn mk_program(main_vector: Vec<Op>, literals: Vec<Var>, var_names: Names) -> Program

// Lines 54-107: Create test database with verbs
fn test_db_with_verb(verb_name: &str, program: &Program) -> TxDB

// Usage example (line 110-127):
let program = mk_program(
    vec![Imm(0.into()), Pop, Done],
    vec![1.into()],
    Names::new(64),
);
let state_source = test_db_with_verb("test", &program);
let result = call_verb(state, session, registry, "test", args);
```

**File:** `crates/kernel/src/testing/vm_test_utils.rs`
- `call_verb()`: Execute verb and get result

## Fuzz Testing Strategy

### Layer 1: Simple Opcodes (No Jumps/Labels)
Generate sequences of stack-based opcodes that don't require labels:
- Immediate values: `ImmInt`, `ImmFloat`, `ImmEmptyList`
- Arithmetic: `Add`, `Sub`, `Mul`, `Div`, `Mod`
- Comparison: `Eq`, `Ne`, `Lt`, `Le`, `Gt`, `Ge`
- Unary: `Not`, `UnaryMinus`, `BitNot`
- Stack: `Pop`
- Termination: `Return`, `Return0`, `Done`

Properties to verify:
- VM doesn't crash
- Stack underflow is handled gracefully
- Type errors produce E_TYPE, not panics

### Layer 2: Literal Table Access
Generate opcodes that reference literal tables:
- `Imm(label)` with valid literal indices
- List/map operations with literals

Properties:
- Invalid indices produce errors, not panics
- Literal access is consistent

### Layer 3: Variable Access
Generate `Push(Name)` and `Put(Name)` with valid variable names.

Properties:
- Undefined variable access produces E_VARNF
- Variable assignment works correctly

### Layer 4: Control Flow
Generate valid control flow with matching labels:
- `If(label)` / `Jump`
- `While` / `Exit`
- `BeginScope` / `EndScope`

Properties:
- All jumps land on valid opcodes
- Scope nesting is balanced
- Loop termination is reached

### Layer 5: Full Program Generation
Use compiler-generated programs as oracle:
- Compile source → opcodes
- Execute opcodes
- Verify result matches expected

## Implementation Plan

1. Create `crates/kernel/src/tests/proptest/` directory
2. Add proptest dependency to kernel crate
3. Implement opcode generators layer by layer
4. Create execution harness that catches panics
5. Define properties for each layer

## Example Generator Structure

```rust
// Generate immediate value opcodes
fn arb_imm_op() -> impl Strategy<Value = Op> {
    prop_oneof![
        any::<i32>().prop_map(Op::ImmInt),
        any::<f64>().prop_filter_map(|f| {
            if f.is_finite() { Some(Op::ImmFloat(f)) } else { None }
        }),
        Just(Op::ImmEmptyList),
        Just(Op::ImmNone),
    ]
}

// Generate binary arithmetic with proper stack setup
fn arb_binary_arithmetic() -> impl Strategy<Value = Vec<Op>> {
    (arb_imm_op(), arb_imm_op(), arb_binary_op())
        .prop_map(|(a, b, op)| vec![a, b, op])
}

fn arb_binary_op() -> impl Strategy<Value = Op> {
    prop_oneof![
        Just(Op::Add),
        Just(Op::Sub),
        Just(Op::Mul),
        Just(Op::Div),
        Just(Op::Mod),
    ]
}
```

## Safety Considerations

1. **Stack Underflow**: VM should handle gracefully
2. **Invalid Labels**: Should produce errors, not panics
3. **Type Mismatches**: Should produce E_TYPE
4. **Infinite Loops**: Use tick limits
5. **Resource Exhaustion**: Limit program size
