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

- `make`, `make build`, or `make gen.objdir` exports normalized
  [objdef](../../book/src/the-system/objdef-file-format.md) into `gen.objdir/`.
- `make gen.moo-textdump` or `make output` exports a traditional MOO textdump.
- `make roundtrip` exports, reimports, and compares the two objdef trees.
- `make check` checks the shared Makefile rules, imports the core, and checks the roundtrip.

### Development workflow

- `make reformat` or `make rebuild` checks roundtrip stability, then overwrites `src/` files with
  normalized objdef. This removes source comments. Review the diff before committing.
- `make test` validates import and export. This core has no method test suite.
- `make clean` - Removes generated files

### Configuration

The Makefile automatically enables mooR's modern features:

- Boolean returns from comparison operators
- Symbol support in builtins
- Custom error handling
- UUID-style object IDs

Set `DEBUG=1` to run the compiler under gdb for debugging compilation issues.

All cores include the rules in `cores/make/`. Use `make help` to list targets. The default compiler
launcher is Cargo. `MOORC_TYPE=direct` uses `target/debug/moorc`; `MOORC_TYPE=docker` uses
`MOORC_IMAGE` (default: `moor-moor-daemon`). `MOORC_PROFILE=release` selects a release build for
Cargo or direct mode. `MOORC=...` overrides the command; the core's feature options still apply.
