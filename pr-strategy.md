# PR Strategy for Tree-sitter Implementation

## Current Situation
- Single PR with ~21K lines of changes
- 4 logical commits already organized
- Too large for effective review

## Recommended Approach: Sequential PRs

```
main
  │
  ├─→ PR 1: CST Foundation (~6.5K lines)
  │     │   - Core CST library
  │     │   - PEST parser migration to CST
  │     │   - Foundation for multi-parser support
  │     │
  │     └─→ PR 2: Tree-sitter Parser (~8K lines)
  │           │   - Tree-sitter implementation
  │           │   - Design patterns applied
  │           │   - Feature-flagged
  │           │
  │           └─→ PR 3: Testing (~5K lines)
  │                 │   - Comprehensive tests
  │                 │   - Benchmarks
  │                 │   - Examples
  │                 │
  │                 └─→ PR 4: CI/Docs (~900 lines)
  │                       - Workflows
  │                       - Documentation
```

## Benefits of This Approach

### For Ryan (Reviewer):
1. **Manageable Chunks**: Each PR is focused and reviewable in one sitting
2. **Logical Progression**: Each PR builds on the previous
3. **Early Feedback**: Can provide feedback on foundation before reviewing implementation
4. **Easier Testing**: Can test each layer independently

### For You (Author):
1. **Faster Merges**: Smaller PRs merge faster
2. **Reduced Conflicts**: Less chance of conflicts with main
3. **Iterative Improvement**: Can incorporate feedback progressively
4. **Clear History**: Clean git history with logical progression

## GitHub Stacked PRs Feature

GitHub now supports "stacked" PRs where you can:
1. Create PR 1: `pr/1-cst-foundation` → `main`
2. Create PR 2: `pr/2-tree-sitter-parser` → `pr/1-cst-foundation`
3. Create PR 3: `pr/3-testing-infrastructure` → `pr/2-tree-sitter-parser`
4. Create PR 4: `pr/4-ci-documentation` → `pr/3-testing-infrastructure`

This way:
- Each PR shows only its own changes
- PRs can be reviewed in parallel
- When PR 1 merges, PR 2 automatically retargets to main
- GitHub shows the relationship between PRs

## Implementation Steps

1. **Run the branch creation script**:
   ```bash
   chmod +x create-pr-branches.sh
   ./create-pr-branches.sh
   ```

2. **Create the PRs in order** (or as a stack)

3. **Use the templates** from `pr-templates.md` for descriptions

4. **Add review hints** in each PR:
   - What to focus on
   - What's out of scope (coming in next PR)
   - Any known issues or future improvements

## Alternative: Two-PR Approach

If 4 PRs seems like too much overhead:

1. **PR 1: CST + Tree-sitter Implementation** (~14.5K lines)
   - Still large but contains complete feature
   - Can be reviewed as a unit

2. **PR 2: Tests + CI + Docs** (~6K lines)
   - All supporting infrastructure
   - Can be reviewed more quickly

## Recommendation

I strongly recommend the 4-PR approach because:
- Ryan's style guide emphasizes clarity and reviewability
- Smaller PRs align with the "single responsibility" principle
- The logical separation already exists in your commits
- It enables faster iteration and feedback

Would you like me to run the script to create the PR branches?
