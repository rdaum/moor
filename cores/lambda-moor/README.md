# Lambda-moor: Modernized LambdaCore 2018

This is a modified copy of "LambdaCore 2018" as downloaded from https://lisdude.com/moo/, adapted to
work with mooR's modern features and options.

## What's been changed

The modifications consist only of the following:

- Dumped into mooR's ["objdef" format](../../book/src/the-system/objdef-file-format.md) so it can be
  more easily read and edited
- Changed passwords to use `argon2` cryptography instead of `crypt` for basic security improvement
- Fixed many verbs so that mooR's boolean-returns don't cause issues when turned on
- Fixed many verbs so that mooR's UUID-style objects don't cause issues

## Purpose

The goal here is to provide a basically stock LambdaCore that works with mooR's modern features,
suitable for testing or use by people who want a familiar MOO environment. This is not necessarily
meant to be a basis for mooR's development focus generally.

Note that mooR itself is entirely capable of loading a stock LambdaCore (or other) textdump and
running it, as long as the set of options (boolean returns, uuid objects, etc) are configured
correctly and compatibly.

## Building and Testing

The `Makefile` provides several useful targets for working with this codebase:

### Basic compilation

- `make` or `make gen.moo-textdump` - Compiles the objdef sources into a traditional MOO textdump
  for validation
- `make gen.objdir` - Compiles into a new [objdef](../../book/src/the-system/objdef-file-format.md)
  directory structure

### Development workflow

- `make rebuild` - **WARNING: DESTRUCTIVE** - Builds a new
  [objdef](../../book/src/the-system/objdef-file-format.md) dump and overwrites your local `src/`
  files with the compiled result. Use this as the last step before committing changes.
- `make test` - Runs the test suite using the compiled code -- though this core doesn't really have
  a test suite. Note that by default it will scan the entire core for verbs prefixed `test_` and run
  them, which may not make sense for LambdaCore
- `make clean` - Removes generated files

### Configuration

The Makefile automatically enables mooR's modern features:

- Boolean returns from comparison operators
- Symbol support in builtins
- Custom error handling
- UUID-style object IDs

Set `DEBUG=1` to run the compiler under gdb for debugging compilation issues.
