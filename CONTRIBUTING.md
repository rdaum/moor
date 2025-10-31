# Contributing to mooR

Thank you for your interest in contributing to mooR! This document provides guidelines and
instructions for contributing to the project.

## Project Philosophy

mooR is a collaborative project where contributors work together as colleagues. We value:

- **Pragmatic Solutions**: Simple, clean solutions over complex ones
- **Technical Honesty**: Speaking up when unsure or when approaches seem problematic
- **Code Quality**: Readability and maintainability as primary concerns
- **Performance Focus**: Especially in core runtime components (`kernel`, `db`)
- **Free Software Ethic**: All contributions are licensed under GPL-3.0, preserving user freedom

In general, we aim to learn and work together interactively - talking through concepts, features and
bugs; which is in keeping with the social form and history of MOO itself.

So don't hesitate to ask questions, push back on approaches, or seek clarification.

## Types of Contributions

mooR welcomes contributions in many forms -- here's some examples:

### Core Development (Rust)

- Server architecture improvements
- New builtin functions
- Performance optimizations
- Protocol extensions
- Bug fixes

### World Building (MOO Language)

- Creating new cores and experiences
- Porting existing MOO content
- Building modern web-enabled interfaces
- Documentation and tutorials

### Web Client Development

- UI/UX improvements
- New client features
- Performance optimizations
- Accessibility enhancements

### Documentation & Testing

- Expanding the mooR Book
- Creating tutorials and examples
- Stress testing and bug reports
- Improving test coverage

**Important**: We aim to keep the documentation in sync with the code. If you add a feature, update
the book in the same PR. If you notice something wrong or missing in the documentation, submit a PR
to fix it.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Environment](#development-environment)
- [Project Structure](#project-structure)
- [Coding Guidelines](#coding-guidelines)
- [Testing](#testing)
- [Pull Request Process](#pull-request-process)
- [Types of Contributions](#types-of-contributions)
- [Community](#community)

## Code of Conduct

By participating in this project, you are expected to uphold our community standards:

- Be respectful and inclusive
- Focus on constructive, technical discussions
- Help maintain a welcoming environment for all contributors

## Getting Started

### Prerequisites

- **Rust**: Version 1.90.0 or later
  - We generally avoid nightly/unstable features in production code
  - We aim to keep up with stable Rust releases as they become available
  - The nightly toolchain is only used for formatting (rustfmt features)
- **Node.js**: For web client development
- **Docker**: For containerized development and testing
- **Git**: For version control

### First Steps

1. **Fork the repository** on [Codeberg](https://codeberg.org/timbran/moor) (this creates your own
   copy of the repository)
2. **Clone your fork** locally
3. **Set up the development environment** (see below)
4. **Explore the codebase** and identify an area of interest

## Development Environment

### Quick Start with Docker

The easiest way to get started is with Docker Compose:

```bash
docker compose up
```

This starts all services:

- `moor-daemon`: Backend MOO service
- `moor-telnet-host`: Telnet interface (port 8888)
- `moor-web-host`: Web API server
- `moor-frontend`: Web client (port 8080)

### Local Development Setup

For active development, use the development scripts:

```bash
# Full development stack (daemon + web client)
npm run full:dev

# Individual components
npm run daemon:dev      # Backend daemon
npm run web-host:dev    # Web API server
npm run dev             # Web client only
```

**Note**: The npm scripts start the daemon and web host, but not the telnet host. For telnet
development, use the bacon configuration below.

### Bacon Development Tool

For file-watching development with automatic restarts, use [bacon](https://dystroy.org/bacon/):

```bash
# Install bacon
cargo install bacon

# Run development services with file watching
bacon daemon           # Daemon with file watching
bacon telnet           # Telnet host with file watching
bacon web              # Web host with file watching
bacon test             # Run tests with file watching
```

See [`bacon.toml`](bacon.toml) for the complete configuration.

### Building and Testing

```bash
# Build the entire workspace
cargo build --workspace

# Run all tests
cargo test --workspace

# Run tests for a specific crate
cargo test -p moor-kernel

# Linting and code quality - ALL RUST CODE MUST BE CLIPPY CLEAN
cargo clippy --workspace --all-targets --all-features

# Format all files according to project standards
dprint fmt
```

## Project Structure

mooR is organized as a Rust workspace with multiple crates:

### Core Runtime

- `crates/var`: Core MOO language datatypes
- `crates/common`: Common data/object model pieces, interfaces, and shared utilities
- `crates/kernel`: The MOO virtual machine and runtime and task scheduler
- `crates/compiler`: MOO language compiler
- `crates/db`: Transactional object database layer
- `crates/daemon`: Main server daemon (this is where the "moo" lives)

### Networking & Protocols

- `crates/telnet-host`: Traditional telnet interface
- `crates/web-host`: Web/WebSocket API server
- `crates/rpc/`: Components used for communicating between the hosts and the daemon.

### Utilities & Testing

- `crates/testing/`: Test harnesses and load tools
- `tools/moorc`: Command-line compiler
- `tools/moor-emh`: Emergency Medical Hologram -- Offline database management / toolkit

### Web Client

- `web-client/`: React/Vite web interface which acts as a rich browser based client

## Coding Guidelines

### Rust Code Style

- **Edition**: Rust 2024
- **Formatting**: Use the project's specific formatting rules:
  ```bash
  # Use the provided script (recommended)
  ./format-rust.sh

  # Or manually with nightly toolchain
  cargo +nightly fmt -- --config reorder_imports=true,imports_indent=Block,imports_layout=Mixed
  ```

  **Why nightly?** We use nightly rustfmt features for consistent import organization:
  - `reorder_imports=true`: Groups and sorts imports consistently
  - `imports_indent=Block`: Uses block-style indentation for imports
  - `imports_layout=Mixed`: Allows mixed import styles (single vs multi-line)

  **Note**: This is the only place we use nightly features - all production code runs on stable
  Rust.
- **Naming**:
  - Modules: `snake_case`
  - Types: `PascalCase`
  - Traits: Verb-noun combinations
  - **Important**: Names must describe what code does, not implementation details or history
  - **Avoid**: Implementation details ("ZodValidator", "MCPWrapper"), temporal context ("NewAPI",
    "LegacyHandler"), pattern names unless they add clarity
- **Imports**: All `use` statements at top of file/module (avoid per-function imports -- P.S. some
  LLMs like to do this and it's annoying)
- **Avoid deep nesting**: Rust code -- with its extensive use of matching over ADT -- can trend
  towards deeply nested code that becomes increasingly difficult to read. To avoid this there are a
  number of techniques:
  - **Early Returns**: Short-circuit out of your function on _negative_ conditions, leaving the
    _positive_ case for last. This helps the reader understand the codeflow and emphasizes the
    function's overall purpose.
    - Handle error cases and invalid conditions first
    - Return early with `?` operator for `Result`/`Option` types
    - Makes the "happy path" clear and uncluttered
  - **Let-else statements**: Use `let else` for conditional binding with early returns on failure
  - **Avoid `else` branches**: Generally prefer early returns over `else` branches on `if`
    statements
  - **Factor out into separate functions**: Break complicated deeply nested blocks into smaller,
    focused functions

#### Example: Transforming Nested Code to Early Returns

**Before (Deeply Nested):**

```rust
fn process_user_input(input: &str) -> Result<User, Error> {
    if !input.is_empty() {
        let trimmed = input.trim();
        if trimmed.len() >= 3 {
            if let Ok(user) = User::parse(trimmed) {
                if user.is_valid() {
                    Ok(user)
                } else {
                    Err(Error::InvalidUser)
                }
            } else {
                Err(Error::ParseFailed)
            }
        } else {
            Err(Error::TooShort)
        }
    } else {
        Err(Error::EmptyInput)
    }
}
```

**After (Early Returns):**

```rust
fn process_user_input(input: &str) -> Result<User, Error> {
    if input.is_empty() {
        return Err(Error::EmptyInput);
    }

    let trimmed = input.trim();
    if trimmed.len() < 3 {
        return Err(Error::TooShort);
    }

    let user = User::parse(trimmed)?;

    if !user.is_valid() {
        return Err(Error::InvalidUser);
    }

    Ok(user)
}
```

**Using let-else:**

```rust
fn process_user_input(input: &str) -> Result<User, Error> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(Error::EmptyInput);
    }

    let Ok(user) = User::parse(trimmed) else {
        return Err(Error::ParseFailed);
    };

    user.is_valid().then_some(user).ok_or(Error::InvalidUser)
}
```

- **Comments**: Describe what code does NOW, not historical context or implementation details

### Performance Considerations

Performance is paramount, especially in `kernel` and `db` crates:

- Prefer low or zero-copy solutions where possible
- Follow cache-friendly patterns
- Consider vectorization for amenable problems
- Avoid unnecessary allocations
- Measure your changes before and after
- If you're writing code which is an optimization... prove it!

### Documentation

- **License Headers**: Every source file must have GPLv3 license header (enforced by
  `.licensure.yml`)
  - Install: `cargo install licensure`
  - Run: `licensure -i --project`
- **Rustdoc**: Major functions and modules require proper documentation
- **Comments**: Should explain _why_ and _what_, not _how_
- **No Marketing Language**: Avoid laudatory or promotional language
- **Evergreen Comments**: No references to historical context or implementation details unless it's
  important.
- **Book Documentation**: The mooR Book ([`book/`](book/README.md)) is our primary user
  documentation
  - **Keep it current**: If you add a feature, update the book in the same PR
  - **Fix issues**: If you notice documentation errors, submit a PR to fix them
  - **Fill gaps**: If you find missing documentation, help fill it in

### TypeScript/JavaScript

- Use 4-space indentation
- Follow ESLint configuration in `eslint.config.mjs`
- Run `npm run lint` and `npm run typecheck` before committing
- **Formatting**: All TypeScript, JSON, and Markdown files must be formatted with `dprint fmt`

## Testing

### Testing Philosophy

We follow strict testing practices:

- **Comprehensive Coverage**: Unit, integration, and end-to-end tests
- **Avoid Mocked Behavior**: Tests should exercise real logic where possible
- **Clean Output**: Test output must be pristine to pass
- **Test-Driven Development**: TDD can be a helpful approach - write tests first when practical

### Debugging Process

When debugging issues, follow this systematic approach:

1. **Root Cause Investigation**:
   - Read error messages carefully
   - Reproduce consistently
   - Check recent changes via git diff

2. **Pattern Analysis**:
   - Find working examples in the codebase
   - Compare against reference implementations
   - Identify differences between working and broken code

3. **Hypothesis and Testing**:
   - Form a single clear hypothesis
   - Test with minimal changes
   - Verify before continuing

4. **Implementation Rules**:
   - Always have a simple failing test case
   - Never add multiple fixes at once
   - Test after each change
   - If a fix doesn't work, re-analyze rather than adding more fixes

### Running Tests

```bash
# All Rust tests
cargo test --workspace

# With debug output
cargo test -- --nocapture

# Web client tests
npm run test

# Specific test module
cargo test -p moor-kernel js_execute::tests::test_simple_js_execution
```

### Adding Tests

- Unit tests go in `#[cfg(test)]` modules beside implementations
- Integration tests in each crate's `tests/` directory
- Use the Moot harness (`cargo test -p moot`) for runtime behavior
- Load testing tools in `crates/testing/`

## Performance Tracing

mooR includes optional Chrome Trace Event Format tracing for performance analysis and debugging.
This allows you to capture detailed performance data that can be visualized in Chrome DevTools.

**For detailed tracing instructions, see [doc/TRACING.md](doc/TRACING.md).**

### Available Tracing Methods

#### 1. Using Bacon Development Tool

For file-watching development with tracing:

```bash
# Install bacon if not already installed
cargo install bacon

# Run daemon with tracing enabled
bacon daemon-traced
```

#### 2. Standalone Tracing

For direct tracing without file watching:

```bash
# Build and run daemon with tracing feature
cargo build --features trace_events -p moor-daemon
./target/debug/moor-daemon ./moor-data --db development.db --trace-output moor-trace.json

# Or use the npm script
npm run daemon:traced
```

#### 3. Docker with Tracing

For containerized tracing:

```bash
# Build tracing-enabled Docker image
docker build --target backend --build-arg TRACE_EVENTS=true -t moor-daemon-traced .

# Run with Docker Compose tracing override
docker compose -f docker-compose.yml -f docker-compose.tracing.yml up -d moor-daemon

# Trace files will be available in ./traces/moor-trace.json
```

#### 4. Full Development Stack with Tracing

For the complete development environment with tracing:

```bash
# Start all services with tracing enabled
npm run full:dev-traced
```

### Using Trace Files

1. **Generate trace file**: Use any of the methods above to run the daemon with tracing
2. **Access trace file**:
   - Local builds: `moor-trace.json` in current directory
   - Docker: `./traces/moor-trace.json`
3. **Analyze in Chrome DevTools**:
   - Open Chrome DevTools (F12)
   - Go to "Performance" tab
   - Click "Load profile" button
   - Select your trace file
   - The trace will display detailed performance data including:
     - Function execution times
     - Task scheduling
     - Database operations
     - Network activity

### Trace File Format

Trace files use the Chrome Trace Event Format and include:

- Process and thread metadata
- Function execution spans
- Database operation timing
- Task scheduling events
- Network I/O events

### Performance Overhead

Tracing adds minimal overhead when enabled. The system uses efficient event collection and writes
events asynchronously to avoid impacting runtime performance.

## Pull Request Process

### Before Submitting

1. **Ensure tests pass**: `cargo test --workspace`
2. **Run linters**: `cargo clippy --workspace --all-targets --all-features` (ALL RUST CODE MUST BE
   CLIPPY CLEAN)
3. **Format code**:
   - Rust: Run `./format-rust.sh` or ensure formatting is correct with `./format-rust.sh --check`
   - TypeScript/JSON/Markdown: Run `dprint fmt`
4. **Check license headers**: Ensure all source files have GPLv3 headers (use `.licensure.yml` tool)
5. **Update documentation**: If your changes affect user-facing behavior

### Creating a Pull Request

1. **Create a branch** with descriptive name
2. **Make focused changes** - one unit of work per PR (single feature or bug fix)
3. **Write clean commits** following standard git conventions:
   - Short, imperative subject line (e.g., "Fix command FIFO blocking on suspended tasks")
   - Optional details in body for cross-cutting changes
   - Squash incidental formatting changes into main commit
4. **Submit your PR**: Push your branch to your fork on Codeberg and create a pull request from
   there to the main repository

### PR Description Guidelines

When creating a pull request, include the following information:

- **Summary**: Brief description of what changes were made
- **Problem**: What issue or improvement this addresses
- **Solution**: How your changes solve the problem
- **Testing**: Describe what testing was performed, including:
  - Results of `cargo test --workspace`
  - Confirmation that all Rust code passes clippy checks
  - Verification that Rust files are formatted with `./format-rust.sh --check`
  - Verification that TypeScript/JSON/Markdown files are formatted with `dprint fmt`
  - Manual testing scenarios if applicable
- **Related Issues**: Links to any relevant issues or discussions

## Reporting Issues

When reporting issues, please help us understand and reproduce the problem by providing detailed
information.

### Bug Reports vs Feature Requests

- **Bug Reports**: For when something isn't working as expected or documented
- **Feature Requests**: For new functionality or improvements

### Essential Information for Bug Reports

1. **Reproduction Steps**:
   - Clear, step-by-step instructions to reproduce the issue
   - Include specific commands, inputs, or actions
   - Note any specific conditions required

2. **Version Information**:
   - Look for the startup log line that includes version and commit hash:
   ```
   moor-daemon | 2025-10-29T02:25:51.723186Z  INFO main crates/daemon/src/main.rs:434: moor 0.9.0-alpha (commit: 7c56ea9) daemon starting...
   ```
   - Include both version number and commit hash

3. **Expected vs Actual Behavior**:
   - What you expected to happen
   - What actually happened
   - Error messages, stack traces, or unexpected output

4. **Environment Details**:
   - Operating system and version
   - Docker version (if using containers)
   - Any relevant configuration details

5. **Performance Traces (Optional but Helpful)**:
   - For performance issues or panics, consider attaching a trace file
   - See [Performance Tracing](#performance-tracing) section for how to generate traces
   - Traces can provide detailed insight into what was happening before a crash or performance
     problem

### Compatibility Baseline

When reporting behavioral differences, note that our baseline for comparison is **LambdaMOO 1.8.x**
(and now 1.9.x).

- We aim for compatibility with LambdaMOO behavior
- While we include some ToastStunt compatibilities, we do not aim for full compliance with the
  ToastStunt fork
- Please specify which LambdaMOO version you're comparing against

## LLM and Agentic Contributions

We welcome contributions that leverage modern development tools, including AI assistants and LLMs,
but with important caveats. In fact, many parts of the mooR codebase have had attention and
production from an LLM, and it's been an essential tool in our development process.

### Code Generation Guidelines

- **Critical Review Required**: All AI-generated code must be thoroughly reviewed and understood by
  the human contributor before submission
- **No Blind Acceptance**: Do not submit code you don't fully understand or can't explain
- **Own Your Work**: You are responsible for the quality, correctness, and maintainability of all
  code you submit
- **Call Out AI Slop**: If you see something that looks like "AI slop" - confused, overly complex,
  or poorly reasoned code or documentation - do not hesitate to call it out, whether in existing
  code or new proposed contributions

### Communication Guidelines

- **No AI-Generated Commit Messages**: Write your own commit messages that accurately describe the
  changes
- **No AI-Generated PR Descriptions**: Describe your work in your own words
- **Avoid Marketing Language**: AI-generated text often includes excessive praise or promotional
  language - strip this out
- **Be Specific and Technical**: Focus on what the code actually does, not generic descriptions
- **No Hallucinations**: Ensure all technical claims and descriptions are accurate

### Best Practices

- Use AI as a tool, not a replacement for your own understanding
- Always test and verify AI-generated code thoroughly
- Be prepared to explain and defend your implementation choices
- Remember that you are the author and are responsible for the work

## Community

### Communication Channels

- **Issues & Pull Requests**: [Codeberg](https://codeberg.org/timbran/moor/issues) (primary)
- **Discussion**: [Discord Community](https://discord.gg/Ec94y5983z)
- **Documentation**: [mooR Book](https://timbran.org/book/html/)

**Note**: We only accept pull requests through Codeberg. The GitHub repository is a mirror only.

### Getting Help

If you're stuck or need guidance:

1. Check the [mooR Book](https://timbran.org/book/html/) for potential answers
2. Search existing issues for similar problems
3. Ask in the Discord community
4. Create a new issue with detailed context

### Recognition

All contributors are recognized in the project's:

- `Cargo.toml` authors list
- Release notes
- Project documentation

## License

By contributing to mooR, you agree that your contributions will be licensed under the same
[GPL-3.0 license](LICENSE) that covers the project.

### Important Licensing Notes

- **Core Database Files**: The files in the `cores/` directory (including JHCore and other MOO
  databases) have complex licensing situations and are NOT covered by the project's GPL-3.0 license.
  See the `cores/LICENSING.md` file for detailed information about the historical licensing
  complexities.

- **Documentation**: The contents of the `book/` directory are licensed separately under the terms
  specified in `book/src/legal.md`.

---

Thank you for contributing to mooR! Your efforts help build a vibrant, modern platform for online
communities and collaborative spaces.
