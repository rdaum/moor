# Justfile for Moor project test automation

# Default recipe lists all available commands
default:
    @just --list

# Install cargo-nextest if not already installed
install-nextest:
    @if ! command -v cargo-nextest >/dev/null 2>&1; then \
        echo "Installing cargo-nextest..."; \
        cargo install cargo-nextest; \
    else \
        echo "cargo-nextest already installed"; \
    fi

# Run tests with nextest (retries flaky tests automatically)
test: install-nextest
    cargo nextest run --workspace

# Run tests with nextest in CI mode (fewer retries, fail-fast)
test-ci: install-nextest
    cargo nextest run --workspace --profile ci

# Run tests without retries (for local development)
test-no-retries: install-nextest
    cargo nextest run --workspace --profile no-retries

# Run only the flaky scheduler integration tests
test-scheduler: install-nextest
    cargo nextest run --workspace 'test(scheduler_integration_test)'

# Run all tests with standard cargo test (fallback)
test-standard:
    cargo test --workspace

# Run compiler tests specifically
test-compiler:
    cargo nextest run --package moor-compiler

# Run tests with verbose output
test-verbose: install-nextest
    cargo nextest run --workspace --verbose

# Show nextest configuration
show-config: install-nextest
    cargo nextest show-config

# List all tests
list-tests: install-nextest
    cargo nextest list --workspace

# Run tests and generate JUnit XML report
test-junit: install-nextest
    cargo nextest run --workspace --message-format json > test-results.json

# Clean test artifacts
clean:
    cargo clean
    rm -f test-results.json

# Format code
fmt:
    cargo fmt --all

# Run clippy
clippy:
    cargo clippy --workspace --all-features --all-targets -- -D warnings

# Run all pre-commit checks
precommit: fmt clippy test

# Setup development environment
setup: install-nextest
    @echo "Development environment setup complete"
    @echo "Available commands:"
    @just --list
