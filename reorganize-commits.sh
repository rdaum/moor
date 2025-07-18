#!/bin/bash
# Script to reorganize commits into 4 commits structure

set -e

echo "Current branch has 3 commits that need to be reorganized into 4..."
echo "Backing up current branch state..."
git branch -f backup-reorganize-2 HEAD

echo "Going back to base commit..."
git reset c42314da

echo "Staging all changes to reorganize them..."
# Stage everything so we can selectively commit
git add -A

echo "Creating commit 1: CST library foundation..."
# Reset to unstage everything
git reset

# Add CST library files
git add crates/compiler/src/cst.rs
git add crates/compiler/src/parsers/parse_cst.rs
git add crates/compiler/src/errors/cst_compare.rs

# Also need to add the parts of other files that enable CST
git add crates/compiler/src/parsers/mod.rs
git add crates/compiler/src/lib.rs
git add crates/compiler/src/errors/mod.rs

git commit -m "feat: Add CST (Concrete Syntax Tree) library foundation

- Introduce CST as intermediate representation between parsing and AST
- Add CSTNode structure with spans and semantic information
- Implement CST to AST transformation pipeline
- Add CST comparison utilities for testing
- Update parser infrastructure to support CST-based parsing"

echo "Creating commit 2: Tree-sitter parser implementation..."
# Add tree-sitter specific files
git add crates/compiler/src/parsers/tree_sitter/
git add crates/compiler/src/parsers/parser_trait.rs
git add crates/compiler/src/builders/
git add crates/compiler/src/errors/enhanced_errors.rs
git add crates/compiler/src/errors/tree_error_recovery.rs
git add crates/compiler/src/unparse.rs
git add crates/compiler/src/moo.pest
git add crates/compiler/src/objdef.rs

# Add modifications to existing files
git add crates/compiler/src/codegen.rs
git add crates/compiler/src/decompile.rs
git add crates/compiler/Cargo.toml
git add crates/kernel/src/config.rs
git add crates/kernel/src/vm/compile_selector.rs
git add crates/kernel/src/vm/mod.rs
git add Cargo.toml
git add Cargo.lock

git commit -m "feat: Add tree-sitter parser implementation for MOO language

- Implement tree-sitter based parser alongside existing PEST parser
- Add generic tree traits for parser-agnostic AST building
- Implement semantic walker for enhanced error recovery
- Add builder pattern for AST construction
- Support incremental parsing and better error messages
- Add parser feature flags for conditional compilation"

echo "Creating commit 3: Testing infrastructure..."
# Add test files
git add crates/compiler/src/testing/
git add crates/compiler/benches/
git add examples/
git add crates/kernel/src/testing/
git add doc/TESTING_*.md
git add doc/MOOT_IMPROVEMENTS.md
git add tools/moorc/
git add crates/testing/load-tools/

# Add test-related modifications
git add crates/db/src/tx_management/relation_tx.rs
git add crates/kernel/Cargo.toml
git add crates/kernel/src/tasks/
git add crates/kernel/src/vm/builtins/
git add crates/kernel/src/vm/moo_execute.rs
git add crates/kernel/src/vm/vm_call.rs
git add crates/var/benches/
git add crates/var/src/program/opcode.rs

git commit -m "test: Add comprehensive testing infrastructure and examples

- Add parser comparison tests between PEST and tree-sitter
- Create benchmarks for parser performance evaluation
- Add MOOT test improvements and documentation
- Create examples for parser validation and error comparison
- Add load testing tools and scheduler test utilities
- Improve test coverage for various parsing scenarios"

echo "Creating commit 4: CI and documentation..."
# Add CI and documentation files
git add .github/workflows/
git add .gitignore
git add doc/TESTING_TREE_SITTER.md

git commit -m "ci: Add CI workflows, documentation and tooling support

- Add grammar validation workflow for tree-sitter
- Add MOOT test workflow for compatibility testing
- Add tree-sitter specific validation workflow
- Update gitignore for tree-sitter artifacts
- Add comprehensive tree-sitter testing documentation"

echo "Done! The commits have been reorganized."
echo "Done! The commits have been reorganized."
echo "Original state backed up in 'backup-reorganize-2' branch."
echo ""
echo "To view the new commit structure:"
echo "  git log --oneline -4"
echo ""
echo "To restore original state if needed:"
echo "  git reset --hard backup-reorganize-2"
echo ""
echo "To push the reorganized branch:"
echo "  git push --force-with-lease origin ndn/tree-sitter-on-new-parse"
