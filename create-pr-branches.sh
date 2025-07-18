#!/bin/bash
# Script to create branches for sequential PR strategy

set -e

echo "Creating branches for sequential PR strategy..."

# Save current state
CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
BASE_COMMIT="c42314da"

# Create PR 1: CST Foundation
echo "Creating PR 1: CST Foundation..."
git checkout -b pr/1-cst-foundation $BASE_COMMIT
git cherry-pick da50c6d4
echo "✓ Created pr/1-cst-foundation"

# Create PR 2: Tree-sitter Parser (based on PR 1)
echo "Creating PR 2: Tree-sitter Parser..."
git checkout -b pr/2-tree-sitter-parser pr/1-cst-foundation
git cherry-pick 81cbc2b0
echo "✓ Created pr/2-tree-sitter-parser"

# Create PR 3: Testing Infrastructure (based on PR 2)
echo "Creating PR 3: Testing Infrastructure..."
git checkout -b pr/3-testing-infrastructure pr/2-tree-sitter-parser
git cherry-pick ee2cb523
echo "✓ Created pr/3-testing-infrastructure"

# Create PR 4: CI/Documentation (based on PR 3)
echo "Creating PR 4: CI/Documentation..."
git checkout -b pr/4-ci-documentation pr/3-testing-infrastructure
git cherry-pick 81b4f33b
echo "✓ Created pr/4-ci-documentation"

# Return to original branch
git checkout $CURRENT_BRANCH

echo ""
echo "Branches created successfully!"
echo ""
echo "Recommended PR submission order:"
echo "1. Create PR from pr/1-cst-foundation to main"
echo "   Title: 'feat: Add CST (Concrete Syntax Tree) library foundation'"
echo "   ~6.5K lines, establishes foundation"
echo ""
echo "2. After PR 1 merges, create PR from pr/2-tree-sitter-parser to main"
echo "   Title: 'feat: Add tree-sitter parser implementation for MOO language'"
echo "   ~8K lines, core parser functionality"
echo ""
echo "3. After PR 2 merges, create PR from pr/3-testing-infrastructure to main"
echo "   Title: 'test: Add comprehensive testing infrastructure and examples'"
echo "   ~5K lines, validates implementation"
echo ""
echo "4. After PR 3 merges, create PR from pr/4-ci-documentation to main"
echo "   Title: 'ci: Add CI workflows, documentation and tooling support'"
echo "   ~900 lines, supporting infrastructure"
echo ""
echo "Alternative: Stack PRs on GitHub"
echo "- Create PR 1: pr/1-cst-foundation → main"
echo "- Create PR 2: pr/2-tree-sitter-parser → pr/1-cst-foundation"
echo "- Create PR 3: pr/3-testing-infrastructure → pr/2-tree-sitter-parser"
echo "- Create PR 4: pr/4-ci-documentation → pr/3-testing-infrastructure"
echo ""
echo "This creates a 'stack' where each PR shows only its changes"
