# Pre-commit Hook Setup

This project uses [pre-commit](https://pre-commit.com/) to ensure code quality and catch issues before they reach GitHub.

## Quick Start

```bash
# Run the setup script
./setup-pre-commit.sh

# Or manually:
pip install pre-commit
pre-commit install
pre-commit install --hook-type pre-push
```

## What Gets Checked

### On Every Commit (fast checks ~30s)
- **Formatting**: `cargo fmt` ensures consistent code style
- **Linting**: `cargo clippy` catches common mistakes and anti-patterns
- **Compilation**: Ensures code compiles with all feature combinations
- **License Headers**: Verifies new Rust files have proper license headers
- **File Hygiene**: Trailing whitespace, file endings, merge conflicts
- **Quick Tests**: Runs unit tests with limited threads

### On Push (comprehensive checks ~2-5min)
- **Full Test Suite**: All tests with all features
- **Security Audit**: `cargo audit` checks for known vulnerabilities
- **Integration Tests**: Full integration test suite

## Hook Details

The configuration in `.pre-commit-config.yaml` includes:

1. **Standard file checks** (pre-commit-hooks)
   - Remove trailing whitespace
   - Fix end of file
   - Check YAML/TOML syntax
   - Detect merge conflicts
   - Prevent large files (>500KB)

2. **Rust-specific checks**
   - `cargo fmt --check`: Formatting verification
   - `cargo clippy`: Linting with warnings as errors
   - `cargo check`: Compilation with all features
   - `cargo test`: Quick unit tests on commit, full suite on push

3. **Project-specific checks**
   - License header verification for new Rust files
   - Tree-sitter feature compilation
   - Example and benchmark compilation

## Manual Usage

```bash
# Run on staged files
pre-commit run

# Run on all files
pre-commit run --all-files

# Run specific hook
pre-commit run cargo-fmt

# Update hooks to latest versions
pre-commit autoupdate
```

## Skipping Hooks (Emergency Only)

```bash
# Skip pre-commit hooks
git commit --no-verify

# Skip pre-push hooks
git push --no-verify
```

**Note**: Skipping hooks is discouraged as it may break CI builds.

## Troubleshooting

### Hook Fails But CI Would Pass
- Ensure you're using the same Rust version as CI
- Run `cargo clean` and try again
- Check that all dependencies are up to date

### Performance Issues
- The quick tests on commit should take <30s
- If slower, consider using `--no-verify` for WIP commits
- Push hooks are intentionally comprehensive

### Adding New Hooks
1. Edit `.pre-commit-config.yaml`
2. Run `pre-commit try-repo . --all-files` to test
3. Commit the configuration change

## Why Pre-commit?

This setup ensures:
- ✅ No broken commits reach GitHub
- ✅ Consistent code style across the team
- ✅ Early detection of common issues
- ✅ Faster CI runs (many issues caught locally)
- ✅ Better code review experience

The hooks mirror our GitHub Actions CI checks, preventing the frustration of pushing code only to have CI fail on basic issues.
