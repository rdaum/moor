# Cowbell runtime tests

For ordinary source changes, run the style and method checks:

```sh
make -C cores/cowbell check
```

For the complete integration suite used by CI, run:

```sh
make -C cores/cowbell check-full
```

`check-full` adds connected sessions, executable tutorials, harness checks, export roundtrip,
telnet/restart tests, Git worker tests, and source inventory checks. Session scenarios create fresh
worlds, so this suite takes several minutes. Run it when integration behavior changes.

The test tools use the main workspace and lockfile. Targets run in order because they share
generated directories. Style details stay in `gen.style.log` and `gen.style-strict.log`; failures
print the log.

`make -C cores/cowbell test-git-worker` checks Git access through the daemon, telnet host, and Git
worker. It uses a temporary local HTTP repository and requires the Git CLI for the fixture. It
checks binary contents, symlinks, tree metadata, worker errors, and reads pinned to a commit after a
branch moves. It also stages and applies Git `@changes` reviews, checks saved source after a branch
moves, and tests discard, changed settings, and wizard revocation during a fetch. The full Cowbell
subtree update is disabled by default because debug builds can exceed the runtime task budget. To
run that scenario manually, add `--full-core` to `tests/git_worker.py`. The method suite also checks
wizard-only dispatch, forged descriptors, local lookup, and malformed responses.

`make -C cores/cowbell` exports the shipped core. `make rebuild` first checks export stability, then
copies the exported sources into `src/`. Export removes source comments. Use this target only when
you intend to replace working sources.

## LLM tests

Run the focused regression methods without a model provider:

```sh
make -C cores/cowbell test-llm MOORC_TYPE=direct
```

The methods cover concrete failures in limits, cancellation, tool outcomes, permissions, compaction,
concurrent requests, and frontend configuration. Connected scenarios in `check-full` cover command
routing, connection ownership, confirmations, suggestions, project commands, and room commands.
Avoid repeating those contracts with tests that inspect collaborator layout or every internal limit.

Use the local HTTP fixture when changing provider or worker integration:

```sh
make -C cores/cowbell test-llm-provider MOORC_TYPE=direct
```

It checks real curl requests, retries, malformed replies, accounting, and persistent restart. The
restart case waits for reservation expiry and can take over a minute. Run it alone with
`test-llm-restart`. Neither target belongs to ordinary `check`.

`test-llm-live` is an optional smoke test using `OPENROUTER_API_KEY` and
`deepseek/deepseek-v4.1-flash`. It permits at most 12 requests, 768 output tokens per request, and
240 seconds. A loopback proxy holds the real key; the temporary world receives only a fixture token.
Live requests are excluded from `check`, `check-full`, and CI. The harness reports numeric usage and
results without logging prompts, replies, or credentials. There is no benchmark gate.

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

The document contract scenarios use fixture `#90121`. They cover canonical nodes, metadata
roundtrips, all three renderers, invalid trees, references, and resource limits:

```sh
make -C cores/cowbell test MOORC_TYPE=direct TEST_FILTER='#90121'
```

These scenarios use inert content and need no connection, worker, or live world. They are also part
of the default method suite. Their sample output can be checked with the installed Djot client
parser without adding a parser dependency to the core test runner.

Fixture `#90123` checks the MOO Djot parser, source diagnostics, input limits, literal attribution,
and validation before description and note edits. It also parses and resolves all 69 shipped help
articles. Each article has its own execution slice; the sweep does not combine their costs into one
command quota. The parser and renderer cases check all three output formats.

Fixture `#90125` checks delivery: one encoding per viewer, shared renderings for equivalent options,
matching live and historical documents, utility connection scope, complete rewrites, public welcome
output, literal banner substitutions, and contextual help authoring. Meadow checks document
transport through FlatBuffers and encrypted history, plus rewrite ownership, expiry, complete
content replacement, and rejection without consuming the target.

Fixture `#90124` checks room capture: shared description resolution, literal labels, retained
references after object recycling, caller permissions, and ambient and integrated content. Meadow
tests verify rendered panel ingestion and separation from historical narrative.

Fixture `#90122` checks event resolution: captured locations, actor and observer wording, literal
bindings, selected alternatives, caller permissions, producer hooks, and bounded expansion. It also
exercises private-message paragraphs, agent reports, malformed table rows, source-range numbering,
generated help, code documentation, and structured room/container descriptions. Headless event
scenarios also check MIME negotiation and reuse across connections.

## Commands and committed events

`runtime-session` uses the shared runner in `cores/tools/session-runner` with `--cowbell`. This
option enables Cowbell's flyweights and rich notifications. The runner compiles the core once per
invocation. Each `.moot` file gets a fresh database loaded from those compiled definitions, plus a
separate scheduler and output queues. Commands pass through the kernel command parser and scheduler.

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

`@action PROPERTY` reads an action map from a property on the selected player, then submits it to
`#0:do_action` as a connected client invocation. The handler runs with no MOO caller frame, as it
does for a clicked web action. This works for non-programmers without evaluating MOO code. Use this
to test captured exit descriptors, including stale or forged ones.

`@event text/plain utility` finds a committed notification for the selected endpoint. It matches the
content type and audience metadata. `@event data:state room_snapshot` matches a data event's
namespace and kind. The match can select a later queued event with the requested contract.
Assertions report the endpoint and retained events when no event matches. Recipient isolation is
part of the assertion.

`@event present:text/html tools` matches a committed presentation for the selected endpoint. Its
MIME type and presentation target must match exactly.
`@event history:text_plain authority-own-history` consumes a committed mock `Session::log_event`
record. The selected endpoint's player, record type, and string value must match exactly. This
history assertion does not prove durable event-log storage.

`@quiet` checks that the selected endpoint has no pending output. `@noevents` checks its current
committed event queue. This assertion is a snapshot. It does not prove that a delayed task can never
publish an event. A delayed case must wait for its dispatch or use an explicit completion marker
before the final absence assertion.

`@text-within SECONDS TEXT` waits for the selected endpoint's next output line and checks it
exactly. The timeout must be from 1 through 30 seconds. Use this for a timer response, instead of a
fixed sleep. An unexpected line fails immediately. The runner does not search past it for a match.

The runner invokes the actual kernel parser and task scheduler without a daemon or network host.
Named endpoints exercise multiple in-process connections. The separate `test-wire` target exercises
real sockets and server restart.

## Executable tutorials

Six chapters of the [Cowbell Book](book/src/index.md) are also session tests: names and grammar,
object shortcuts, Pet Rock, Henri, rooms and passages, and custom exits. `SESSION_TUTORIALS` in the
Makefile selects their Markdown sources. The default session suite runs them alongside the `.moot`
scenarios. The runner compiles the core once, then gives each scenario or document a fresh database.
All runnable blocks in one document share its database and selected participant.

To run the executable tutorials:

```sh
make -C cores/cowbell test-tutorial
```

For an already prepared fixture overlay and built runner:

```sh
target/debug/core-session-runner --cowbell \
  --core-dir cores/cowbell/.runtime-headless-src \
  --moot cores/cowbell/book/src/programming/pet-rock.md \
  --wizard 90100 --programmer 90101 --nonprogrammer 90102
```

Repeat `--moot` to include several files or directories in one invocation. Directories discover
`.moot` files. Markdown documents must be named explicitly, so other documentation does not become a
test by accident.

Executable documents use top-level code fences that start in column one:

| Fence information      | Meaning                                                                            |
| ---------------------- | ---------------------------------------------------------------------------------- |
| `moot`                 | Run the existing session commands and assertions.                                  |
| `moo verb=OBJECT:VERB` | Submit this exact body through `@program OBJECT:VERB` as the selected participant. |
| `LANG example`         | Show an illustrative fragment without running it.                                  |

Backtick and tilde fences are supported. Prose and headings remain ordinary Markdown. Unknown fence
information, unclosed fences, and documents without any commands fail the test. Errors report the
document path, original line number, and current heading.

After a verb block, check the programming prompt and compilation confirmation in a `moot` block.
Standalone `=` and `~~` lines check queued output, including these editor responses. Keep assertions
beside the examples they describe. Do not maintain a second copy of the tutorial code in a fixture.

Test setup can supply participants, privileges, and a room. Keep it in a labelled section of the
document. Run the actual lesson with the reader's permissions. Setup must not install or repair the
objects that the lesson teaches readers to create.

## Harness and export checks

`test-harness` checks that an empty method filter fails. It also submits a fixture command that
raises an uncaught exception. The runner must reject that command for the expected reason. Negative
event scenarios also check that wrong recipient, content type, and audience assertions fail. Cowbell
marks caught command exceptions with boolean `command_exception` metadata. The runner rejects these
events. Ordinary denial events remain valid.

Expected failures print `PASS ... (expected failure)`. Unexpected success or the wrong error prints
the command output and fails the check. The Rust workspace test job runs the session runner's unit
tests. To run those tests locally, use `cargo test -p core-session-runner`.

`roundtrip` exports shipped sources, reimports the export, and compares both complete directory
trees. Both exports must contain objdef files. Fixture object IDs must be absent from the shipped
export.

`test-build-inventory` copies the core Makefile and shared rules into a temporary directory with a
small source fixture. It adds, edits, and deletes a nested object source, rebuilding after each
step. It checks the exported object and constants after deletion. The roundtrip check covers the
complete shipped core.

```sh
make -C cores/cowbell test-harness
make -C cores/cowbell roundtrip
make -C cores/cowbell test-build-inventory
```

Use `MOORC_TYPE=direct` to use an already built `target/debug/moorc`. `make clean` removes the
Cowbell-generated exports, overlays, and harness logs.

## Native scheduled work

Cowbell uses the runtime `schedule_*` builtins directly. It has no core scheduler object or polling
loop. Housekeeping creates one recurring schedule. The Henri tutorial creates an optional adaptive
schedule and chooses a fresh delay after each firing.

Schedule creation and cancellation commit with the calling transaction. Stored IDs and their
schedule requests therefore commit together. `pass_elapsed` is false for these callbacks.
Housekeeping disables adaptive returns so its swept-player count cannot change the interval. The
tutorial enables adaptive returns. Each positive return adjusts the next native deadline; the native
cadence uses scheduled deadlines, rather than callback completion times. Callback faults retain the
base retry interval. The runtime default retires a schedule after 50 consecutive faults.

Native schedules persist in the runtime task database. An objdef export contains object properties,
including stored IDs, but does not export the schedule store. Fresh objdef imports therefore contain
no schedules. Housekeeping and the tutorial start command replace invalid stored IDs.

Headless scenario `#90001` tests housekeeping startup, duplicate starts, stale IDs, callback
execution, cancellation, and caller controls. Its recording player captures command events. It also
tests a two-player housekeeping sweep and one grouped room announcement. The Henri tutorial tests
its actual adaptive callback under the author's permissions, duplicate starts, stale IDs, and
cancellation. Its test uses a short first wait rather than waiting several minutes.

## Real connections and restart

```sh
make -C cores/cowbell test-wire
```

This target builds the server and runs `tests/wire.py` against an isolated temporary database. The
script opens two real telnet connections for the same player. It checks committed world state and
actual housekeeping schedule callbacks across server restart. It also checks committed schedule
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
node tools/tsc.mjs --noEmit -p cores/cowbell/tests/tsconfig.json
node --import tsx cores/cowbell/tests/mcp/changes.ts
```

Run these commands from the repository root. The test drives review changes through real MCP stdio
requests. It checks catalog discovery, the guide, argument errors, compiler errors, edited source,
historical source, declared additions, and prepared application through both hosts and both source
transports. The fixture contains 73 existing verbs, a new helper, and a new property. Review pages,
effect pages, and result pages must include the complete set.

The test restarts the persistent world before apply and after finish. It drops an apply reply at the
client boundary, starts another MCP process, and recovers the committed result. It also revokes the
requester's wizard authority during worker suspension. A native-host timeout probe verifies that a
committed mutation executes only once. Independent assertions check installed programs and retained
property data and permissions. A separate uploaded review binds a new object to an explicit address
and installs its contents without calling an initialization verb.

Use `--web-only` for a focused run of the TypeScript host. The full acceptance run must cover both
hosts.

The `change_tools.moot` scenario checks the shared provider and requester permissions.
`change_agents.moot` uses a deterministic model to exercise base and coding room execution through
the shared LLM agent. Its inspection request leaves live code unchanged. Its update request
validates and applies a multiline edited program through the shared tools. The session runner
creates an ephemeral signing key so room authorization uses the normal capability checks. Both room
paths also test a denied programmer, authority loss between preview and apply, and draft recovery
after a live edit. Model responses are deterministic; no external model account is required.

## Plain-telnet change reviews

```sh
node --import tsx cores/cowbell/tests/changes/telnet.ts
node --import tsx cores/cowbell/tests/changes/telnet.ts --git
```

The test uses the same isolated world and local sources as the MCP test. It sends ordinary telnet
commands without GMCP or HTML. It covers multiple review pages, comparisons, multiline editing,
cancellation, reconnect, a concurrent review change, compiler repair, apply, result pagination,
finish, history, and another review. The HTTP run also exercises failed-fetch retry. Independent
assertions check the installed programs. They do not drive the administrator workflow.

The `change_terminal.moot` scenario checks displayed-row guards, prepared selection invalidation,
group removal, and recovery of saved decisions after live code changes.

## Complete change-management checks

Run the shared native tests and browser acceptance suite from the repository root:

```sh
cargo test -p moor-objdef
cargo test -p moor-mcp-host
cargo build -p moor
npm run test:changes --prefix clients/meadow
```

The browser suite includes history repair, dependency groups, explicit object bindings, live edits
after preview, rollback, restart, and response loss. Its large-review case exercises search,
keyboard navigation, narrow layout, complete effect and result pages, and preserved editor text.

Prepare the headless core, then run each focused service and agent scenario:

```sh
make -C cores/cowbell build-test-tools runtime-headless-src MOORC_TYPE=direct
for scenario in change_manager_preservation change_manager_results change_tools change_agents change_terminal; do
  target/debug/core-session-runner \
    --cowbell --core-dir cores/cowbell/.runtime-headless-src \
    --moot "cores/cowbell/tests/baseline/sessions/$scenario.moot" \
    --wizard 90100 --programmer 90101 --nonprogrammer 90102
done
```

These fixtures use temporary worlds. They do not use the development database or require a live
GitHub connection.
