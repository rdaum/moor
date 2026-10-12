# Snore Core

_Just boring enough._

Snore Core is a LambdaCore fork for mooR. It keeps familiar MOO commands, rooms, objects, mail,
news, guests, and live programming, with internals updated for mooR. The aim is a core that feels
familiar to longtime players and works with the approach taught by most MOO tutorials.

The sources use explicit local bindings, booleans, maps, closures, and UUID user objects. Core
objects retain numbered identities. There are no core FTP, HTTP, or Gopher services, recycler pools,
output pagers, or automatic word wrapping.

## Try it

From the repository root, with the repository's stable Rust toolchain installed:

```sh
./cores/snore/run-monolith.sh
```

Then connect from another terminal or your MOO client:

```sh
telnet 127.0.0.1 8888
```

A fresh world accepts `connect Wizard` without a password, or `connect Guest`. Set the wizard
password with `@password` before making the server accessible to other people. Start with `help`,
`help introduction`, and `@who`.

The launcher imports `src/` on its first run. Later runs resume the saved world in
`cores/snore/gen.monolith/`. Source edits do not update that saved world. Ctrl-C stops the server
and preserves its data.

To try a fresh import while keeping an existing world:

```sh
MOOR_RUN_DIR=/tmp/snore-trial ./cores/snore/run-monolith.sh
```

The default listener is `127.0.0.1:8888`. Set `MOOR_TELNET_PORT`, `MOOR_TELNET_ADDRESS`, or
`MOOR_PROFILE=release` as needed. `run-monolith.sh --help` lists the settings. The launcher works
from any directory.

## Multiple connections

A player can connect from more than one client. Speech and received private messages reach all of
that player's connections. Help, who listings, read prompts, and editor listings go to the client
that requested them. `@quit` closes only the connection that issued it.

Core authors can use `tell` for player-wide output, `tell_current` for the current connection, or
`tell_connection` for an explicit destination. Each has a line-list form. See the
[output conventions](STYLE_GUIDE.md#output-and-retained-features) for permissions and
missing-connection behavior.

## Build and check

```sh
make -C cores/snore                 # Import sources and export normalized objdef
make -C cores/snore reformat        # Rewrite src/ with normalized objdef (alias: rebuild)
make -C cores/snore check           # Style, methods, commands, sessions, and roundtrips
make -C cores/snore test-wire       # Real TCP login, reconnect, and authoring
make -C cores/snore test-extraction # Destructive extraction in disposable worlds
```

The checks use isolated databases. They do not open the launcher's saved world. Generated exports,
test overlays, logs, and running worlds use the ignored `gen.*` names. `make clean` removes build
and test output while preserving launcher state.

`make reformat` checks export/reimport stability before copying the export over `src/`. It
regenerates formatting and does not preserve source comments. Verb docstrings remain part of the
code and are preserved. Explicit `let` and `const` declarations are also preserved.

The [test guide](tests/README.md) describes individual targets, fixture conventions, and validation
limits. Use `make help` for the command list. A split daemon/telnet launcher is also available
through `make run` and `make run-telnet` in separate terminals.

## Shape your world

`$player` supplies in-world behavior and shared communication. `$mail_recipient_class` adds mail and
news. The default account class, `$default_player`, adds non-VR conveniences such as teleporting,
editing, and structural inspection. A game can derive from the shared player/mail layer without
inheriting those conveniences.

Builder, programmer, and wizard classes retain the familiar LambdaCore hierarchy. Their commands
live in separate feature objects, installed by default on the corresponding classes. Installing a
feature does not grant programmer or wizard permission. See
[player classes](docs/player-classes.md).

The client controls visual wrapping and long-output paging. Private `page` messaging remains. Editor
`fill` remains an explicit operation that changes text; it does not wrap command output.

## Reference

- [Design and compatibility](docs/design.md): retained services, intentional changes, and limits.
- [Programming interfaces](docs/reference.md): matching hooks, collection callbacks, and utilities.
- [Transactions and maintenance](docs/transactions.md): commit boundaries, quotas, and extraction.
- [Style guide](STYLE_GUIDE.md): conventions for modifying the core.
- [mooR book](../../book/src/SUMMARY.md): language, database, permissions, and server reference.
- [Licensing and attribution](LICENSE.md): LambdaCore provenance and inherited credits.

Byte quotas are the supported default. The object-count alternative depends on native quota support
that is not implemented. Some runtime compatibility issues remain; the separate
[runtime findings](../../docs/modern-lambda-runtime-findings.md) record their evidence and status.

The core Makefiles share compiler, export, roundtrip, and method-test rules in `cores/make/`. Use
`make help` to list targets. `MOORC_TYPE=direct` uses an existing `target/debug/moorc`;
`MOORC_TYPE=docker` uses `MOORC_IMAGE`. `MOORC_PROFILE=release` selects a release compiler for Cargo
or direct mode. `MOORC=...` still overrides the compiler command, and `FEATURES` still sets Snore's
language options. `runtime-headless` and `runtime-session` are aliases for `test-headless` and
`test-session`. `make check` also tests the shared Makefile rules.
