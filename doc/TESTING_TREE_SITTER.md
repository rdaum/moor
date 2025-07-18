# Testing Tree-Sitter Parser with MOOT Tests

This document explains how to run MOOT tests using the tree-sitter parser implementation.

## Prerequisites

1. Build moorc with tree-sitter support:
```bash
cargo build --release --bin moorc --features="tree-sitter-parser"
```

## Running Tests

### Run all MOOT tests with tree-sitter-moot parser:
```bash
cargo run --release --bin moorc --features="tree-sitter-parser" -- \
    --src-textdump crates/testing/moot/Test.db \
    --out-textdump /tmp/test_out.db \
    --test-directory crates/kernel/testsuite/moot \
    --test-wizard 2 \
    --test-programmer 3 \
    --test-player 4 \
    --parser tree-sitter-moot
```

### Run only eval tests:
```bash
cargo run --release --bin moorc --features="tree-sitter-parser" -- \
    --src-textdump crates/testing/moot/Test.db \
    --out-textdump /tmp/test_out.db \
    --test-directory crates/kernel/testsuite/moot/eval \
    --test-wizard 2 \
    --test-programmer 3 \
    --test-player 4 \
    --parser tree-sitter-moot
```

## How It Works

1. The `--parser` flag accepts the following values:
   - `cst` (default): Use the CST parser
   - `tree-sitter`: Use the tree-sitter parser with enhanced errors
   - `tree-sitter-moot`: Use the tree-sitter parser with MOOT-compatible errors

2. The moorc command sets the `MOO_PARSER` environment variable based on the `--parser` flag.

3. The following functions check the `MOO_PARSER` environment variable:
   - `bf_eval()`: Used by the `eval()` builtin
   - `set_verb_code()`: Used when setting verb code
   - World state executor: Used for verb programming

4. If the environment variable is set and the parser exists, it uses that parser. Otherwise, it falls back to the default compiler.

## MOOT-Compatible Error Messages

The `tree-sitter-moot` parser generates simplified error messages that match MOOT test expectations:
- `"expected ident"` for missing identifiers
- `"unexpected token"` for unexpected tokens
- Simple line/column error positions without visual indicators

## Troubleshooting

If tests fail:
1. Check that the tree-sitter-parser feature is enabled
2. Verify the Test.db file exists at the specified path
3. Check the test output for specific error messages
4. Compare error messages between `cst` and `tree-sitter-moot` parsers

## Implementation Notes

Due to architectural constraints, we use environment variables rather than passing the parser selection through the full execution pipeline. This is a pragmatic solution that allows testing without major architectural changes.