# mooR minimal core

This core is the smallest useful objdef database for booting mooR and testing the runtime. It is
roughly analogous to LambdaMOO's `Minimal.db`: a root object, a system object, one room, and one
wizard/programmer/player object with enough code to log in and evaluate MOO expressions.

It is intentionally not a real usable social core. It does not provide builder commands, rooms,
player affordances, social commands, etc. It also has no authorization / authentication layer,
password management, etc. Read $login and $sysobj in Cowbell and/or Lambda-Moor for examples on how
to implement that.

## Object Layout

The source lives in `src/` as objdef files:

- `#0`, `SYSOBJ`, `sysobj.moo`: system object. Its `do_login_command` returns `#3`, the test
  wizard/player.
- `#1`, `ROOT`, `root.moo`: root object with the `import_export_id` property.
- `#2`, `FIRST_ROOM`, `first_room.moo`: first room. It provides a readable `eval` verb for basic
  interactive testing.
- `#3`, `WIZARD`, `wizard.moo`: the single wizard, programmer, and player.
- `constants.moo`: symbolic object constants used by the objdef source.

The stable `import_export_id` values make the generated objdef output predictable and keep object
references readable when the core is rebuilt.

## Building

The Makefile wraps `moorc` and builds a normalized objdef dump into `gen.objdir`:

```sh
make -C cores/minimal-core
```

By default this runs `cargo run -p moorc`. To use an existing binary instead:

```sh
make -C cores/minimal-core MOORC_TYPE=direct
```

To run through the Docker image:

```sh
make -C cores/minimal-core MOORC_TYPE=docker
```

`gen.objdir` is generated output and should not be committed.

## Testing

Validate the source import and export:

```sh
make -C cores/minimal-core test
```

This core has no method test suite. The target verifies that its objdef source imports and exports
successfully. Use `make check` to check the shared rules and export/reimport stability too.

## Rebuilding Source

Use `rebuild` after changing source files when you want to round-trip through `moorc` and replace
`src/` with the normalized objdef output:

```sh
make -C cores/minimal-core rebuild
```

The target checks export/reimport stability before copying normalized objdef into `src/`. It
preserves other source files. Export removes source comments, so review the diff before committing.
`make reformat` is an alias for the same operation.

## Importing

When no binary database exists, the daemon can import this core from:

```text
cores/minimal-core/src
```

After import, the login path should authenticate as `#3`. The `eval` verb is on `#2`, so basic
expression evaluation works once the connected player is in the first room.

The Makefile includes the shared rules in `cores/make/`. `make help` lists the common targets and
compiler settings. `make build` and `make output` also build `gen.objdir/`. `MOORC_PROFILE=release`
selects the release compiler in Cargo or direct mode. `MOORC=...` overrides the compiler command;
the core's feature options still apply.
