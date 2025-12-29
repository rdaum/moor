# Proptest AST Roundtrip Testing

## Overview

Property-based testing for the MOO parser/unparser using proptest to generate arbitrary valid ASTs, unparse them to source code, parse the result, and verify the roundtrip works correctly.

## Goals

- Catch parser/unparser bugs that hand-written tests miss
- Verify operator precedence handling across all combinations
- Test string escape sequences with full unicode
- Build confidence incrementally through layered coverage

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Coverage approach | Incremental layers | Catch issues in layers, merge as each stabilizes |
| Crate location | Dedicated `crates/compiler-proptest/` | Isolates test-only dependencies |
| Layer 1 scope | Literals + binary ops + unary ops | Where most precedence bugs live |
| String generation | Full unicode + all escapes | Catch edge cases early |
| Failure handling | Snapshot to files | Makes debugging easier |
| Test cases | 1000 normal, 10000 in CI | Balance of coverage and speed |

## Crate Structure

```
crates/compiler-proptest/
├── Cargo.toml
├── src/
│   ├── lib.rs           # Re-exports for use in other test crates if needed
│   ├── generators/
│   │   ├── mod.rs
│   │   ├── literals.rs  # Integer, float, string, bool, object, error generators
│   │   ├── expr.rs      # Expression generators (combines literals + ops)
│   │   └── stmt.rs      # Statement generators (future layers)
│   └── tests/
│       ├── mod.rs
│       └── roundtrip.rs # The actual proptest roundtrip tests
└── failures/            # Git-ignored directory for failed case snapshots
```

## Generator Strategy

### Layer 1: Literals and Operators

**Literal generators:**

| Generator | Output | Notes |
|-----------|--------|-------|
| `arb_integer()` | `Expr::Value(Var::Int(n))` | Full i64 range |
| `arb_float()` | `Expr::Value(Var::Float(f))` | Exclude NaN/Inf |
| `arb_string()` | `Expr::Value(Var::Str(s))` | Full unicode, escapes, no null bytes |
| `arb_bool()` | `Expr::Value(Var::Bool(b))` | true/false |
| `arb_object()` | `Expr::Value(Var::Obj(o))` | #-1, #0, #1..#N |
| `arb_error()` | `Expr::Error(code, None)` | All ErrorCode variants |

**Operator generators:**

| Generator | Variants |
|-----------|----------|
| `arb_binary_op()` | +, -, *, /, %, ^, ==, !=, <, >, <=, >=, in, &., \|., ^., <<, >> |
| `arb_unary_op()` | - (neg), ! (not), ~ (bitnot) |

**Combined expression generator:**
```rust
fn arb_expr_layer1(depth: usize) -> impl Strategy<Value = Expr>
    // Base case (depth=0): any literal
    // Recursive case:
    //   - 60% literal
    //   - 30% binary op with two sub-expressions (depth-1)
    //   - 10% unary op with one sub-expression (depth-1)
    // Max depth: 4-5 to avoid explosion
```

### Future Layers

**Layer 2:**
- Identifiers (`Expr::Id`)
- Property access (`Expr::Prop`)
- Index expressions (`Expr::Index`, `Expr::Range`)
- Conditional expressions (`Expr::Cond`)

**Layer 3:**
- Function/verb calls (`Expr::Call`, `Expr::Verb`)
- Lists and maps (`Expr::List`, `Expr::Map`)
- Scatter assignments

**Layer 4:**
- Statements (`StmtNode::Cond`, `ForList`, `While`, etc.)
- Lambdas (`Expr::Lambda`)
- Full programs with multiple statements

## Test Logic

```rust
proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn roundtrip_expr(expr in arb_expr_layer1(4)) {
        // 1. Wrap expr in a statement
        // 2. Unparse to source code
        // 3. Parse back to AST
        // 4. Unparse again
        // 5. Assert source == source2 (stable roundtrip)
        // 6. Assert ASTs structurally equivalent (ignoring spans)
    }
}
```

**Failure snapshots** written to `failures/YYYY-MM-DD-HHMMSS-{hash}.txt` containing:
- Generated expression (Debug format)
- Unparsed source
- Error message
- Proptest seed for reproduction

**CI configuration:** Use `PROPTEST_CASES=10000` environment variable.

## Dependencies

```toml
[dependencies]
proptest = "1.4"
moor-compiler = { path = "../compiler" }
moor-var = { path = "../var" }
```

## Implementation Tasks

1. Create crate structure and Cargo.toml
2. Implement literal generators
3. Implement operator generators
4. Implement combined expression generator with depth control
5. Implement roundtrip test with failure snapshots
6. Add to workspace and CI
7. Run and fix any discovered bugs
8. Iterate to Layer 2 when stable
