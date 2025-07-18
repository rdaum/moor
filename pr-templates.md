# PR Templates for Tree-sitter Implementation

## PR 1: CST Foundation

### Title
feat: Add CST (Concrete Syntax Tree) library foundation

### Description
This PR introduces the CST (Concrete Syntax Tree) as an intermediate representation between parsing and AST construction. This foundational change enables better error recovery, incremental parsing support, and parser-agnostic AST building.

#### What's Changed
- Introduced `CSTNode` structure with spans and semantic information
- Implemented CST to AST transformation pipeline
- Added CST comparison utilities for testing
- Updated parser infrastructure to support CST-based parsing
- Migrated PEST parser to use CST as intermediate representation

#### Why This Change?
- **Better Error Recovery**: CST preserves more parsing information for better error messages
- **Parser Agnostic**: Allows multiple parser backends (PEST, tree-sitter) to share AST building logic
- **Incremental Parsing**: Foundation for future incremental parsing support
- **Testing**: Easier to test and compare parser outputs at CST level

#### Testing
- Existing PEST parser tests continue to pass
- CST comparison utilities enable better parser testing

#### Impact
- No breaking changes - existing code continues to work
- Sets foundation for tree-sitter parser in next PR

---

## PR 2: Tree-sitter Parser Implementation

### Title
feat: Add tree-sitter parser implementation for MOO language

### Description
This PR adds a tree-sitter based parser alongside the existing PEST parser. The tree-sitter parser provides better error recovery, incremental parsing capabilities, and improved performance for large files.

#### What's Changed
- Implemented tree-sitter based parser using the CST infrastructure from PR #1
- Added generic tree traits for parser-agnostic AST building
- Implemented semantic walker for enhanced error recovery
- Added builder pattern for AST construction with Rust design patterns:
  - Option Iterator Pattern for cleaner field access
  - Strategy Pattern for node conversion
  - Newtype Pattern for type safety
  - State Pattern for error analysis
  - Builder Pattern for configuration
- Added parser feature flags for conditional compilation

#### Design Patterns Applied
Following Rust best practices from https://rust-unofficial.github.io/patterns/:
1. **Option Iterator Pattern**: Replaced verbose if-let chains with Option combinators
2. **Strategy Pattern**: Modular node converters for different node types
3. **Newtype Pattern**: Semantic types like `NodeKind`, `ByteOffset` for type safety
4. **State Pattern**: Error state analysis with fix suggestions
5. **Builder Pattern**: Fluent configuration for TSConverter

#### Why Tree-sitter?
- **Incremental Parsing**: Only reparse changed portions of code
- **Error Recovery**: Better handling of syntax errors with partial AST
- **Performance**: C-based parser with Rust bindings
- **Language Server Ready**: Foundation for future LSP support

#### Testing
- Parser parity tests ensure tree-sitter produces same AST as PEST
- Feature flag allows gradual migration
- No impact on existing PEST parser users

#### Performance
- Benchmarks show comparable performance for small files
- Significant improvement for large files and incremental updates

---

## PR 3: Testing Infrastructure

### Title
test: Add comprehensive testing infrastructure and examples

### Description
This PR adds comprehensive testing for the tree-sitter parser implementation, including comparison tests, benchmarks, and real-world examples.

#### What's Changed
- Added parser comparison tests between PEST and tree-sitter
- Created benchmarks for parser performance evaluation
- Added MOOT test improvements and documentation
- Created examples for parser validation and error comparison
- Added integration tests for generic tree traits
- Improved test coverage for various parsing scenarios

#### Test Categories
1. **Comparison Tests**: Ensure both parsers produce equivalent ASTs
2. **Performance Benchmarks**: Measure parsing speed and memory usage
3. **Error Recovery Tests**: Validate error handling and recovery
4. **Integration Tests**: Test generic tree trait system
5. **Real-world Examples**: Practical MOO code parsing scenarios

#### Key Improvements
- Test helper functions for better test organization
- Grouped test cases using vectors (following Ryan's style)
- Comprehensive error case testing
- Performance regression prevention

#### Documentation
- TESTING_TREE_SITTER.md: Guide for testing tree-sitter implementation
- TESTING_STRATEGY.md: Overall testing approach
- MOOT_IMPROVEMENTS.md: Improvements to MOOT test suite

---

## PR 4: CI/Documentation

### Title
ci: Add CI workflows, documentation and tooling support

### Description
This PR adds CI workflows and documentation to support the tree-sitter parser implementation.

#### What's Changed
- Added grammar validation workflow for tree-sitter
- Added MOOT test workflow for compatibility testing
- Added tree-sitter specific validation workflow
- Updated gitignore for tree-sitter artifacts
- Added comprehensive documentation

#### CI Workflows
1. **Grammar Validation**: Ensures tree-sitter grammar stays valid
2. **MOOT Tests**: Runs compatibility tests with existing MOO code
3. **Tree-sitter Validation**: Parser-specific tests and benchmarks
4. **General CI**: Updated to test with both parsers

#### Benefits
- Automated testing prevents regressions
- Grammar validation catches issues early
- Performance tracking across commits
- Documentation keeps implementation maintainable

---

## Review Tips for Ryan

### For Each PR:
1. **PR 1 (CST)**: Focus on the CST design and transformation logic
2. **PR 2 (Parser)**: Review tree traits abstraction and design patterns
3. **PR 3 (Tests)**: Check test coverage and parity between parsers
4. **PR 4 (CI)**: Verify workflows and documentation completeness

### Key Areas to Review:
- Error handling strategies and fallback patterns
- Performance implications of CST intermediate layer
- Feature flag implementation for gradual migration
- Test coverage for edge cases
- Design pattern applications and their benefits

### Questions to Consider:
- Is the CST structure sufficient for future needs?
- Are the tree traits generic enough for other parsers?
- Is the error recovery strategy comprehensive?
- Are the design patterns improving code clarity?
