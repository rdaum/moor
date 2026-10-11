# Cowbell runtime tests

Run the same check as CI from the repository root:

```sh
make -C cores/cowbell check
```

The check runs the style audit, method tests, headless scenarios, session scenarios, harness checks,
export roundtrip, real telnet/restart checks, and source inventory checks. Targets run in order because they share generated
directories.

`make -C cores/cowbell test-git-worker` checks Git access through the daemon, telnet host, and Git worker.
It uses a temporary local HTTP repository and requires the Git CLI for the fixture.
It checks binary contents, symlinks, tree metadata, worker errors, and reads pinned to a commit after a branch moves.
It also stages and applies Git `@changes` reviews, checks saved source after a branch moves, and
tests discard, changed settings, and wizard revocation during a fetch. A full Cowbell subtree review
checks installed constants and preserves local package settings through apply.
The method suite also checks wizard-only dispatch, forged descriptors, local lookup, and malformed responses.

`make -C cores/cowbell` exports the shipped core. `make rebuild` first checks export stability, then
copies the exported sources into `src/`. Export removes source comments. Use this target only when
you intend to replace working sources.

## Fixtures and permissions

Method tests import `src/` into a disposable database. The generated `.runtime-test-src` overlay
adds fixtures from `tests/baseline/fixtures/`. Headless tests also add `tests/headless/` to
`.runtime-headless-src`.

The fixture principals are:

| Object   | Role                                           |
| -------- | ---------------------------------------------- |
| `#90100` | Wizard and programmer                          |
| `#90101` | Programmer without wizard privileges           |
| `#90102` | Player without programmer or wizard privileges |
| `#90103` | Isolated fixture room                          |
| `#90104` | Principal verification scenarios               |

The principal test checks role flags, player status, ownership, and location. It also proves each
principal can write its own property. Protected writes succeed for the wizard and fail for both
other principals. Fixtures stay outside `src/`. Each overlay is rebuilt from the full source tree,
so nested additions and deletions take effect.

```sh
make -C cores/cowbell test
make -C cores/cowbell test TEST_FILTER='#90104'
make -C cores/cowbell runtime-headless
make -C cores/cowbell runtime-headless HEADLESS_FILTERS=90010 HEADLESS_PLAYER_FILTERS= HEADLESS_WIZARD_FILTERS=
```

Method selection must discover at least one test. An empty filter fails. A method process has a
600-second limit; each method also has `HEADLESS_TIMEOUT`, which defaults to 10 seconds.

## Commands and committed events

`runtime-session` uses the existing Snore session runner with `--cowbell`. This option enables
Cowbell's flyweights and rich notifications. The runner imports a fresh temporary database for each
`.moot` file. Commands pass through the kernel command parser and scheduler.

```sh
make -C cores/cowbell runtime-session
```

Use `@wizard`, `@programmer`, and `@nonprogrammer` to select the fixture actor's default endpoint.
`%` submits a command. `;` evaluates MOO code. `=` checks one output line. `~~` checks that the line
contains the given text. A bare expectation is a MOO value expression evaluated as the literal
fixture wizard `#90100`. It does not run as the currently selected endpoint's player.

Named endpoints allow several connections for the same player:

```moot
@attach phone programmer
@phone
; return player;
#90101
@programmer
; return player;
#90101
@detach phone
```

`@attach NAME ACTOR` creates an endpoint. `@NAME` selects it. `@reassign NAME ACTOR` changes its
player. `@detach NAME` removes it. The existing endpoint names `wizard`, `programmer`, and
`nonprogrammer` remain available. `@text TEXT` consumes an expected output line for the selected
endpoint without evaluating a result expression.

`@event text/plain utility` finds a committed notification for the selected endpoint. It matches the
content type and audience metadata. `@event data:state room_snapshot` matches a data event's
namespace and kind. The match can select a later queued event with the requested contract.
Assertions report the endpoint and retained events when no event matches. Recipient isolation is
part of the assertion.

`@event present:text/html tools` matches a committed presentation for the selected endpoint.
Its MIME type and presentation target must match exactly.
`@event history:text_plain authority-own-history` consumes a committed mock `Session::log_event` record.
The selected endpoint's player, record type, and string value must match exactly.
This history assertion does not prove durable event-log storage.

`@quiet` checks that the selected endpoint has no pending output. `@noevents` checks its current
committed event queue. This assertion is a snapshot. It does not prove that a delayed task can never
publish an event. A delayed case must wait for its dispatch or use an explicit completion marker
before the final absence assertion.

The runner invokes the actual kernel parser and task scheduler without a daemon or network host.
Named endpoints exercise multiple in-process connections. The separate `test-wire` target exercises
real sockets and server restart.

## Harness and export checks

`test-harness` checks that an empty method filter fails. It also submits a fixture command that
raises an uncaught exception. The runner must reject that command for the expected reason. Negative
event scenarios also check that wrong recipient, content type, and audience assertions fail. Cowbell
marks caught command exceptions with boolean `command_exception` metadata. The runner rejects these
events. Ordinary denial events remain valid.

`roundtrip` exports shipped sources, reimports the export, and compares both complete directory
trees. Both exports must contain objdef files. Fixture object IDs must be absent from the shipped
export.

`test-build-inventory` copies the core into a temporary directory. It adds, edits, and deletes a
nested object source, rebuilding after each step. It checks the exported object and constants after
deletion.

```sh
make -C cores/cowbell test-harness
make -C cores/cowbell roundtrip
make -C cores/cowbell test-build-inventory
```

Use `MOORC_TYPE=direct` to use an already built `target/debug/moorc`. `make clean` removes the
Cowbell-generated exports, overlays, and harness logs.

## Native scheduled work

Cowbell uses the runtime `schedule_*` builtins directly. It has no core scheduler object or polling loop.
Housekeeping creates one recurring schedule. Henri creates six adaptive recurring schedules and chooses a fresh delay after each firing.

Schedule creation and cancellation commit with the calling transaction. Stored IDs and their schedule requests therefore commit together.
`pass_elapsed` is false for these callbacks. Housekeeping disables adaptive returns so its swept-player count cannot change the interval.
Henri enables adaptive returns. Each positive return adjusts the next native deadline; the native cadence uses scheduled deadlines, rather than callback completion times.
Callback faults retain the base retry interval. The runtime default retires a schedule after 50 consecutive faults.

Native schedules persist in the runtime task database. An objdef export contains object properties, including stored IDs, but does not export the schedule store.
Fresh objdef imports therefore contain no schedules. Housekeeping and Henri replace invalid stored IDs when started.
No startup resume hook or legacy schedule-property conversion is required.

Headless scenario `#90001` tests actual housekeeping and Henri startup, duplicate starts, stale IDs, callback execution, adaptive delay ranges, cancellation, and caller controls.
Its recording player captures command events without changing the production output interface.
It also tests a two-player housekeeping sweep and one grouped room announcement.

## Real connections and restart

```sh
make -C cores/cowbell test-wire
```

This target builds the server and runs `tests/wire.py` against an isolated temporary database.
The script opens two real telnet connections for the same player. It checks committed world state
and actual housekeeping schedule callbacks across server restart. It also checks committed schedule
cancellation and checkpoint export. Schedule persistence uses the runtime task database. An objdef
export remains a separate representation of world objects.

These checks do not exercise browser rendering or the Meadow websocket transport. Rich payload
contracts and endpoint routing have in-process session coverage. Host-specific browser behavior
requires a separate client check.

## Change tools and agent workflows

The MCP acceptance test starts an isolated daemon, web host, telnet host, and HTTP/Git workers. It
serves a local Git repository with a nested source directory and an HTTP bundle. No external model
or Git hosting account is required.

```sh
cargo build -p moor-daemon -p moor-web-host -p moor-telnet-host \
  -p moor-mcp-host -p moor-git-worker -p moor-curl-worker
node --import tsx cores/cowbell/tests/mcp/changes.ts
```

Run these commands from the repository root. The test drives review changes through real MCP stdio
requests. It checks catalog discovery, the guide, argument errors, compiler errors, edited source,
application, and retained results through both MCP hosts and both source transports.

The `change_tools.moot` scenario checks the shared provider and requester permissions.
`change_agents.moot` uses a deterministic model to exercise actual RLM and agentic room execution.
Its inspection request leaves live code unchanged. Its update request validates and applies a
multiline edited program through the shared tools. The session runner creates an ephemeral signing
key so room authorization uses the normal capability checks.

## Plain-telnet change reviews

```sh
node --import tsx cores/cowbell/tests/changes/telnet.ts
node --import tsx cores/cowbell/tests/changes/telnet.ts --git
```

The test uses the same isolated world and local sources as the MCP test. It sends ordinary telnet
commands without GMCP or HTML. It covers multiple review pages, comparisons, multiline editing,
cancellation, reconnect, a concurrent review change, compiler repair, apply, result pagination,
finish, history, and another review. The HTTP run also exercises failed-fetch retry.
Independent assertions check the installed programs. They do not drive the administrator workflow.

The `change_terminal.moot` scenario checks displayed-row guards, prepared selection invalidation,
group removal, and recovery of saved decisions after live code changes.
