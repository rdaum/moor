# PR 1: CST Foundation

## Title
feat: Add CST (Concrete Syntax Tree) library foundation

## Description
This PR introduces the CST (Concrete Syntax Tree) as an intermediate representation between parsing and AST construction. This foundational change enables better error recovery, incremental parsing support, and parser-agnostic AST building.

### What's Changed
- Introduced `CSTNode` structure with spans and semantic information
- Implemented CST to AST transformation pipeline
- Added CST comparison utilities for testing
- Updated parser infrastructure to support CST-based parsing
- Migrated PEST parser to use CST as intermediate representation
- Added pre-commit framework for code quality checks

### Why This Change?
- **Better Error Recovery**: CST preserves more parsing information for better error messages
- **Parser Agnostic**: Allows multiple parser backends (PEST, tree-sitter) to share AST building logic
- **Incremental Parsing**: Foundation for future incremental parsing support
- **Testing**: Easier to test and compare parser outputs at CST level

### Testing
- Existing PEST parser tests continue to pass
- CST comparison utilities enable better parser testing
- Pre-commit hooks ensure code quality

### Impact
- No breaking changes - existing code continues to work
- Sets foundation for tree-sitter parser in next PR

### Files Changed Summary
- **Core CST Implementation**: `cst.rs` - The main CST data structures and transformation logic
- **Parser Integration**: `parse_cst.rs` - PEST parser adapted to produce CST
- **Testing Utilities**: `cst_compare.rs` - Tools for comparing CST structures in tests
- **Module Updates**: Updated `lib.rs`, `mod.rs` files to expose CST functionality
- **Code Quality**: Added pre-commit configuration for maintaining standards

### Note on PR Series
This is PR 1 of 4 in a series to introduce tree-sitter parsing support:
1. **CST Foundation** (this PR) - ~6.5K lines
2. Tree-sitter Parser Implementation - ~8K lines
3. Testing Infrastructure - ~5K lines
4. CI/Documentation - ~900 lines

The large change has been split into reviewable chunks that build on each other.
