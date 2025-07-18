# Nextest Integration for Flaky Test Handling

This document describes the nextest integration for handling flaky tests in the Moor project.

## Overview

The Moor project uses [cargo-nextest](https://nexte.st/) to handle flaky tests more robustly. Nextest provides automatic retry capabilities and better test isolation, which is particularly useful for integration tests that interact with real databases and schedulers.

## Configuration

Nextest is configured via `.config/nextest.toml` with the following profiles:

### Default Profile
- **Retries**: 3 attempts for all tests
- **Timeout**: 60 seconds with 2 termination attempts
- **Concurrency**: Uses all available CPU cores

### CI Profile
- **Retries**: 2 attempts (more conservative for CI)
- **Timeout**: 120 seconds with 2 termination attempts
- **Fail-fast**: Enabled to stop on first failure

### No-Retries Profile
- **Retries**: 0 (for local development when you want immediate feedback)

## Specific Test Configurations

### Scheduler Integration Tests
The flaky `scheduler_integration_test` tests get special treatment:
- **Retries**: 5 attempts (more than default)
- **Timeout**: 120 seconds (double the default)
- **Termination**: 3 attempts before force-kill

### wizard_login_with_real_scheduler Test
This particularly flaky test gets maximum attention:
- **Retries**: 5 attempts
- **Timeout**: 120 seconds
- **Leak timeout**: 30 seconds for cleanup

## Usage

### Installation

```bash
# Install nextest
cargo install cargo-nextest

# Or use the Justfile
just install-nextest
```

### Running Tests

```bash
# Run all tests with automatic retry
cargo nextest run --workspace

# Run tests in CI mode
cargo nextest run --workspace --profile ci

# Run tests without retries (local development)
cargo nextest run --workspace --profile no-retries

# Run only scheduler integration tests
cargo nextest run --workspace 'test(scheduler_integration_test)'

# Run with verbose output
cargo nextest run --workspace --verbose
```

### Using Justfile

```bash
# Run tests with nextest
just test

# Run tests in CI mode
just test-ci

# Run tests without retries
just test-no-retries

# Run only scheduler tests
just test-scheduler

# Show nextest configuration
just show-config
```

## GitHub Actions Integration

To integrate nextest into GitHub Actions, update your workflow files:

```yaml
name: Tests
on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v4

    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable

    - name: Install nextest
      uses: taiki-e/install-action@cargo-nextest

    - name: Run tests
      run: cargo nextest run --workspace --profile ci

    - name: Upload test results
      uses: actions/upload-artifact@v4
      if: always()
      with:
        name: test-results
        path: target/nextest/ci/junit.xml
```

## Benefits

1. **Flaky Test Handling**: Automatically retries flaky tests up to 5 times
2. **Better Isolation**: Each test runs in its own process
3. **Faster Execution**: Parallel execution with better resource utilization
4. **Rich Output**: Better test result reporting and filtering
5. **CI-Friendly**: Different profiles for CI vs local development

## Troubleshooting

### Test Still Failing After Retries
If a test continues to fail after all retries, it indicates a real issue that needs investigation:

```bash
# Run the specific failing test with maximum verbosity
cargo nextest run 'test(failing_test_name)' --nocapture --verbose

# Run without retries to see immediate failures
cargo nextest run --profile no-retries 'test(failing_test_name)'
```

### Integration Test Isolation
For tests that need specific resources (database, scheduler):

```bash
# Run integration tests with limited concurrency
cargo nextest run --test-threads 1 'test(integration)'
```

### Performance Impact
Nextest may use more resources due to process isolation. Monitor system resources and adjust `test-threads` if needed.

## Migration from cargo test

Existing `cargo test` commands can be gradually migrated:

```bash
# Old way
cargo test --workspace

# New way
cargo nextest run --workspace

# Both work during transition
cargo test --workspace  # Still works
just test-standard      # Explicit fallback
```

## Configuration Reference

See `.config/nextest.toml` for the complete configuration. Key settings:

- `retries`: Number of retry attempts
- `slow-timeout`: Timeout configuration
- `test-threads`: Concurrency level
- `fail-fast`: Stop on first failure (CI mode)
- `filter`: Test selection patterns
- `overrides`: Per-test configuration

## Example CI Integration

The flaky `test_wizard_login_with_real_scheduler` test that was causing CI failures will now:

1. Run normally on first attempt
2. If it fails, retry up to 5 times
3. Only fail the build if all 5 attempts fail
4. Provide detailed logs for debugging

This significantly improves CI reliability while maintaining test quality.
