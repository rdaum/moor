# Contributing to mooR

Thank you for your interest in contributing to mooR! This document provides guidelines and instructions for contributing
to the project.

## Project Philosophy

mooR is a collaborative project where contributors work together as colleagues. We value:

- **Pragmatic Solutions**: Simple, clean solutions over complex ones
- **Technical Honesty**: Speaking up when unsure or when approaches seem problematic
- **Code Quality**: Readability and maintainability as primary concerns
- **Performance Focus**: Especially in core runtime components (`kernel`, `db`)
- **Free Software Ethic**: All contributions are licensed under GPL-3.0, preserving user freedom

In general, we aim to learn and work together interactively - talking through concepts, features and bugs;
which is in keeping with the social form and history of MOO itself.

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

**Important**: We aim to keep the documentation in sync with the code. If you add a feature, update the book in the same
PR. If you notice something wrong or missing in the documentation, submit a PR to fix it.

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

- **Rust**: Version 1.88.0 or later
- **Node.js**: For web client development
- **Docker**: For containerized development and testing
- **Git**: For version control

### First Steps

1. **Fork the repository** on [Codeberg](https://codeberg.org/timbran/moor) (this creates your own copy of the
   repository)
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

**Note**: The npm scripts start the daemon and web host, but not the telnet host. For telnet development, use the bacon
configuration below.

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
  cargo +nightly fmt -- --config reorder_imports=true,imports_indent=Block,imports_layout=Mixed
  ```
- **Naming**:
    - Modules: `snake_case`
    - Types: `PascalCase`
    - Traits: Verb-noun combinations
    - **Important**: Names must describe what code does, not implementation details or history
    - **Avoid**: Implementation details ("ZodValidator", "MCPWrapper"), temporal context ("NewAPI", "LegacyHandler"),
      pattern names unless they add clarity
- **Imports**: All `use` statements at top of file/module (avoid per-function imports -- P.S. some LLMs like to do this
  and it's annoying)
- **Early Returns**: _Strongly_ preferred over deep nesting. Let-else is your friend. Avoid `else` branches on if
  statements generally.
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

- **License Headers**: Every source file must have GPLv3 license header (enforced by `.licensure.yml`)
    - Install: `cargo install licensure`
    - Run: `licensure -i --project`
- **Rustdoc**: Major functions and modules require proper documentation
- **Comments**: Should explain *why* and *what*, not *how*
- **No Marketing Language**: Avoid laudatory or promotional language
- **Evergreen Comments**: No references to historical context or implementation details unless it's important.
- **Book Documentation**: The mooR Book ([`book/`](book/README.md)) is our primary user documentation
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

## Pull Request Process

### Before Submitting

1. **Ensure tests pass**: `cargo test --workspace`
2. **Run linters**: `cargo clippy --workspace --all-targets --all-features` (ALL RUST CODE MUST BE CLIPPY CLEAN)
3. **Format code**: Run `dprint fmt` for all TypeScript, JSON, and Markdown files
4. **Check license headers**: Ensure all source files have GPLv3 headers (use `.licensure.yml` tool)
5. **Update documentation**: If your changes affect user-facing behavior

### Creating a Pull Request

1. **Create a branch** with descriptive name
2. **Make focused changes** - one unit of work per PR (single feature or bug fix)
3. **Write clean commits** following standard git conventions:
    - Short, imperative subject line (e.g., "Fix command FIFO blocking on suspended tasks")
    - Optional details in body for cross-cutting changes
    - Squash incidental formatting changes into main commit
4. **Submit your PR**: Push your branch to your fork on Codeberg and create a pull request from there to the main
   repository

### PR Description Guidelines

When creating a pull request, include the following information:

- **Summary**: Brief description of what changes were made
- **Problem**: What issue or improvement this addresses
- **Solution**: How your changes solve the problem
- **Testing**: Describe what testing was performed, including:
    - Results of `cargo test --workspace`
    - Confirmation that all Rust code passes clippy checks
    - Verification that files are formatted with `dprint fmt`
    - Manual testing scenarios if applicable
- **Related Issues**: Links to any relevant issues or discussions

## Reporting Issues

When reporting issues, please help us understand and reproduce the problem by providing detailed information.

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

### Compatibility Baseline

When reporting behavioral differences, note that our baseline for comparison is **LambdaMOO 1.8.x** (and now 1.9.x).

- We aim for compatibility with LambdaMOO behavior
- While we include some ToastStunt compatibilities, we do not aim for full compliance with the ToastStunt fork
- Please specify which LambdaMOO version you're comparing against

## LLM and Agentic Contributions

We welcome contributions that leverage modern development tools, including AI assistants and LLMs, but with important
caveats. In fact, many parts of the mooR codebase have had attention and production from an LLM, and it's been an
essential tool in our development process.

### Code Generation Guidelines

- **Critical Review Required**: All AI-generated code must be thoroughly reviewed and understood by the human
  contributor before submission
- **No Blind Acceptance**: Do not submit code you don't fully understand or can't explain
- **Own Your Work**: You are responsible for the quality, correctness, and maintainability of all code you submit
- **Call Out AI Slop**: If you see something that looks like "AI slop" - confused, overly complex, or poorly
  reasoned code or documentation - do not hesitate to call it out, whether in existing code or new proposed
  contributions

### Communication Guidelines

- **No AI-Generated Commit Messages**: Write your own commit messages that accurately describe the changes
- **No AI-Generated PR Descriptions**: Describe your work in your own words
- **Avoid Marketing Language**: AI-generated text often includes excessive praise or promotional language - strip this
  out
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

By contributing to mooR, you agree that your contributions will be licensed under the same [GPL-3.0 license](LICENSE)
that covers the project.

---

Thank you for contributing to mooR! Your efforts help build a vibrant, modern platform for online communities and
collaborative spaces.