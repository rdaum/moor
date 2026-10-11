# Changelog

This changelog records relevant, user-facing changes to mooR: features, significant bug fixes,
compatibility changes, and changes that affect upgrades or operation.

Omit routine internal maintenance, development script fixes, formatting, refactoring, and test-only
changes unless they have a significant user-facing effect. Describe that effect when an entry is
warranted. Not every commit needs an entry.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased 2.0 series (`main`) branch

### Changed

- Meadow change reviews show source provenance, unsupported changes, and committed outcomes.
  Compiler markers return when an invalid saved draft is reopened. Review controls follow service
  capabilities, and reconnect preserves unsaved drafts while checking current review state.

- Meadow's Changes workspace discovers compatible core services and provides package creation,
  upstream configuration, Git/HTTP fetches, source uploads, and baseline adoption. Administrators
  can reopen saved reviews, discard pending reviews, and view retained completed reviews. Cowbell
  exposes completion history under the same authorization rules as individual receipts.

- The Cowbell Docker launcher starts HTTP and Git workers and accepts `--core`, `--git-upstream`,
  and `--baseline-objdef-dir`. Baseline preparation applies only to fresh imports. Existing
  databases retain their baselines and package configuration on restart.

- The single-process `moor` server supports read-only Git worker requests with `--enable-git-worker`
  or `services.git_worker.enabled`. Requests use isolated child processes with configurable resource
  limits. Separate worker enrollment is not required.

- Change reviews now let you browse local-only objects and verbs, including their source, and
  inspect property and object-attribute differences. Import preparation records ancestor hashes for
  those fields so Meadow can group their local edits, upstream edits, and conflicts with verb
  changes. These additional comparisons are read-only; applying changes still updates existing verb
  programs.

- `moor`, `moor-daemon`, `moorc`, and `scripts/dev.sh` accept `--git-upstream origin/main` to
  prepare program baselines from the common ancestor of the checkout and a locally available
  upstream branch while installing the working files as live code. `--baseline-objdef-dir` accepts
  an explicitly supplied base without Git. Prepared baselines survive export and import; Git
  preparation also records the source revisions. Server startup prepares baselines only when
  importing a new database; existing databases are left unchanged.
- Ordinary objdef directory imports preserve supplied baselines but no longer invent missing ones
  from the imported programs. Prepare an upstream baseline or explicitly adopt one before applying
  updates to an installation without tracking metadata.

- Cowbell and Snore now present upstream updates as `@changes fetch`, review, and apply. Commands
  explain review IDs and next steps, and background fetch/apply results are reported automatically
  to the requesting player. Diff output summarizes changes, hides unchanged programs, and uses
  object/verb names and short row numbers. Package objects and installed verbs with no matching
  upstream definition are reported separately and preserved during program updates. Cowbell uses a
  short overview and a dedicated code review in Meadow. Review links open program differences, with
  choices to keep local code, use upstream code, or edit a proposal before applying. Accepted source
  is compared when one of the programs matches its baseline hash. System objects use their `$name`
  references. Usage links prompt for arguments and show command syntax; overview and background
  status messages use inset styling. In Meadow, Cowbell replaces pulsing fetch/apply progress with
  the result on the initiating connection. Review actions show their command beneath the label.
  Direct authenticated API calls retain administrator permission checks. Meadow also lists package
  objects and verbs absent from the fetched source. Source information and upgrade checks are
  available through `@changes details`. Both cores keep the command reference in `help @changes`.
  Missing generation arguments report the required command instead of repeating a help page.

- `dump_object()` and `moorc` can omit accepted-baseline annotations with `include_baselines` set to
  false. Core source rebuilds use this option. Backup and checkpoint exports retain baselines by
  default.

- `load_object()` and `reload_object()` now take source plus one options map and return an object.
  Use `target`, `constants`, and load-only `allocation`. Removed conflict controls now fail
  explicitly; use `preview_objdef_changes()` for review. Merge preserves omitted attributes, and
  reload requires an existing target. Replacement clears absent members and ordinary metadata, and
  recreates verbs in source order. Direct input cannot supply accepted-baseline metadata.

### Added

`moor-git-worker`:

- Add a standalone Git worker for refs, directory listings, file reads, and complete directory
  snapshots over HTTP(S). Results identify the resolved commit and preserve binary file contents.
  Requests have configurable byte, entry, memory, concurrency, and time limits.

`Cowbell`:

- Add Git sources to `@changes`, with wizard-only fetching and saved commit provenance. Apply uses
  the reviewed source even if a branch moves. New imports configure Cowbell's upstream to the mooR
  repository, `refs/heads/main`, under `cores/cowbell/src`. Existing package settings are preserved.
- Add `$git` repository access with wizard-only network operations. Repository, snapshot, and entry
  flyweights support commit-pinned reads, local lookup, exact binary contents, and explicit UTF-8
  decoding.
- Show native schedules in `@ps`. Add `@schedules` for listing, `@schedule ID` for diagnostics, and
  `@stop-schedule ID` (`@kill-schedule`) for stopping future firings. Programmers manage their own
  schedules; wizards can manage all owners. Running firings remain separate tasks controlled by
  `@kill`.

`moor-emh`:

- Add an `export DIRECTORY` command that writes a complete database snapshot as objdef files and
  only moves the output from its `.in-progress` path after the export succeeds.

`kernel`:

- Add `@changes` to Cowbell and Snore for saved program reviews, conflict choices, validated drafts,
  guarded apply, and recovery. Both expose a versioned `$change_manager` API. HTTP staging uses a
  bounded UTF-8 text bundle. Each core declares its default package bindings in source.
- Bound curl-worker response bodies to 16 MiB by default and requests to 30 seconds unless a timeout
  is supplied. Optional HTTP settings select a smaller byte limit, strict UTF-8, and effective URL.
- Add `apply_objdef_changes()` for guarded program updates and metadata-only adoption. Apply checks
  original review evidence and validated drafts before writing programs and baselines together.
  Objdef import derives missing program baselines and preserves supplied baselines on restore.

- Add `preview_objdef_changes()` for read-only program comparison and edited-draft validation.
  Reports include field fingerprints, target identity guards, and per-row eligibility.

- Native scheduled tasks: `schedule_at(obj, verb, when [, args] [, options])` and
  `schedule_every(obj, verb, interval [, args] [, options])` arrange for a verb to run later, once
  or on a drift-free cadence, with each firing a fresh background task and no task suspended in
  between. Schedules are persisted in the tasks database (a new `schedules` keyspace; ids are stable
  across restart and never reused, the allocator's high-water mark being kept in a `tasks_meta`
  keyspace), are GC roots for anonymous objects, and are created/cancelled on the calling task's
  commit like `task_send`. The verb must be executable by the creator, checked at creation and again
  at each firing. Options: `adaptive` return-value protocol, `catchup` (`skip`/`once`/`all`),
  `overlap` (`skip`/`queue`/`concurrent`), `jitter`, `max_faults`, `pass_elapsed`, `state`,
  `persist`, `player`. Companions: `schedule_stop(id)` (returns `false`, never raises, for a stale
  id), `schedule_valid(id)`, `schedule_info(id)`, `schedules([owner])`, `schedules_for(target)`. The
  fired verb sees the schedule's creator as `caller` and `caller_perms()`; its `player` (the target,
  or the `player` option) must be an object the creator controls. A retired schedule stays visible
  to `schedule_info()` for 60 seconds, or until it is stopped, and is then dropped. Deadlines past
  Unix time `18446744073` (the 64-bit nanosecond limit of the store) are `E_INVARG` at creation, and
  a schedule whose next deadline would pass it retires with `deadline_out_of_range`. Book chapter:
  _Scheduled Tasks_.
- A task's transaction-retry counter is reset after every successful transaction boundary
  (`suspend`, `commit`, `read`, `task_recv`, fork dispatch). Previously it only ever incremented, so
  a long-lived loop that hit an occasional conflict would eventually abort with "Task retry limit
  exhausted" no matter how many boundaries it had committed in between.
- New algorithm builtins in `bf_algorithms.rs` for bounded searches over caller-supplied MOO values:
  - `grid_astar(width, height, start_x, start_y, goal_x, goal_y, tile_map, solid_tiles [, options])`
    performs server-side A* pathfinding over a flat tile map, with options for 4- or 8-directional
    movement, corner cutting, return shape, and node limits.
  - `grid_line(width, height, x0, y0, x1, y1)` returns an inclusive Bresenham line over grid
    coordinates.
  - `grid_los(width, height, x0, y0, x1, y1, tile_map, solid_tiles)` checks line of sight across
    walkable tiles.
  - `grid_reachable(width, height, start_x, start_y, tile_map, solid_tiles [, options])` and
    `grid_flood(width, height, start_x, start_y, tile_map, solid_tiles [, options])` perform bounded
    flood searches over tile maps.
  - `graph_shortest_path(edges, from, to [, options])`, `graph_reachable(edges, from [, options])`,
    and `graph_topsort(edges [, options])` operate on caller-supplied directed edge lists.
- New `fromliteral()` builtin for parsing one MOO literal value from a string without evaluating MOO
  code
- New `encode_cbor()` and `decode_cbor()` builtins for serializing MOO values to and from mooR's
  canonical CBOR binary representation. This representation is intended for compact value
  round-trips, external storage, and value fingerprints; lambdas are not currently supported.
- New `value_diff()` and `value_diff3()` builtins for bounded structural comparison of MOO values,
  including list/map/flyweight diffs, optional inline atom values, and conservative three-way
  conflict detection for package and commit-time tooling.
- New entity-state hash builtins for package and tooling code that needs stable fingerprints of
  object attributes, metadata, property values/info/metadata, and verb code/info/metadata:
  `object_attrs_hash()`, `object_metadata_hash()`, `property_value_hash()`, `property_info_hash()`,
  `property_metadata_hash()`, `verb_code_hash()`, `verb_info_hash()`, and `verb_metadata_hash()`.
- New term-manipulation and query builtins in `bf_algorithms.rs`. These provide small, bounded
  building blocks for pattern matching over MOO values and for implementing Datalog/Prolog-like rule
  evaluation in MOO code without writing custom search loops:
  - `term_unify(pattern, value [, bindings [, options]])` unifies supplied MOO values using
    `{'var, name}` variable markers and returns the resulting bindings map or false.
  - `term_match(pattern, values [, bindings [, options]])` applies one pattern to a list of
    candidate values and returns binding maps for successful matches.
  - `term_substitute(template, bindings [, options])` replaces variable markers from a bindings map,
    with options for raising on or leaving unbound variables.
  - `term_query(query, facts [, rules [, bindings [, options]]])` evaluates positive rule/fact
    queries over supplied MOO values, with bounded depth, step, binding, and solution limits.
  - `term_variables(term [, options])`, `term_ground(term [, options])`, and
    `term_normalize(term [, bindings [, options]])` inspect and canonicalize term structures.
- Granular task capability grants via the wizard-only two-argument form of `set_task_perms()`.
  Trusted code can now run as a selected principal while attaching additive, operation-specific
  grants for object reads/writes, object rename/move/recycle/chparent/listing, property
  read/write/define/delete, verb read/write/add/program/call, and selected builtin calls. This lets
  wizard-owned policy code delegate a narrow operation after doing its own authorization check
  instead of temporarily running broad follow-up work as a wizard or changing object ownership.
- New `task_perms()` builtin returning the current permissions principal and active capability
  grants for MOO-level inspection.

`compiler`:

- `include!("path")` and `include_bin!("path")` macros in the objdef compiler for embedding external
  file contents (String and Binary respectively) at compile time.
- List and range comprehensions now support an optional trailing `if` filter clause, for example
  `{ item for item in (items) if valid(item) }`.

`web-host`:

- WebRTC data channel support for realtime event delivery; domain-based routing sends matching
  namespaces over unreliable/unordered data channels, falling back to WebSocket
- Batch world state API: `POST /v1/batch` endpoint for executing multiple world state actions in a
  single transaction (FlatBuffer and JSON payloads)
- Object query API: `GET /v1/objects/query` endpoint for filtered object lookups using parent,
  location, owner, and flag filters with secondary index acceleration
- OpenAPI specification updated for new endpoints

`daemon`:

- New `moor` single-process binary runs daemon, telnet host, and web host in one process.
- `moor` can optionally run an embedded curl worker; is disabled by default.
- Debian packaging now emits a separate `moor` package for the single-process server instead of
  bundling `/usr/bin/moor` into `moor-daemon`.

`rpc`:

- FlatBuffer schema for batch world state RPC and `QueryObjects`
- `QueryObjects` struct with parent/location/owner/flags_all/flags_any filters, wired through
  `WorldState` trait, batch actions, FlatBuffer schema, and daemon message handler
- Remaining batch world state action builder helpers (`ws_request_property`, `ws_request_verb_code`,
  `ws_program_verb`)

`testing`:

- Multi-object benchmark for distributed write workloads
- Integration tests for batch world state RPC

### Changed

`objdef`:

- Add a contextual `clear` clause to property declarations and overrides. It removes the local value
  while allowing permission and metadata updates in the same declaration. Exports now write this
  clause for clear values. Existing files remain accepted, but older importers cannot read the new
  clause. `clear` remains valid as an identifier or constant, including after `=`.

`kernel`:

- New `moor-vm` crate extracted from `moor-kernel`, separating pure VM types (activations, frames,
  opcode loop, unwind, stack ops, exec state) from kernel host services via a `WorldStateCallback`
  trait; kernel provides `KernelHost` impl forwarding to TLS with zero vtable overhead
  (monomorphized generics)
- `value_hash()` now hashes mooR's canonical CBOR representation instead of a literal string form,
  defaults to SHA256, supports additional algorithms (`md5`, `sha1`, `sha224`, `sha256`, `sha384`,
  `sha512`, and `ripemd160`), and can return the raw digest as a binary value.

`daemon`:

- Scheduler rewritten from single-threaded channel-based event loop to direct mutex calls on shared
  `TaskLifecycle` state behind `Arc<Mutex>`, eliminating per-request oneshot allocation and
  unnecessary thread serialization.

`db`:

- Relation-level rebase on CAS failure: if the winning commit didn't modify any relation the loser
  also modified (detected via `Arc::ptr_eq`), re-slot prepared indexes onto the winner's snapshot
  and CAS again without re-checking
- Key-level CAS rebase via bloom filter of modified keys per committed snapshot; enables successful
  rebase when two transactions modify different keys in the same relation
- Cumulative bloom filter on snapshot to skip `check_all` when a committing transaction's keys don't
  overlap any intervening commit's keys; epoch-bounded (resets every 32 versions) to prevent
  saturation
- Tombstone `HashSet`+`RwLock` replaced with lock-free `AtomicBloom` (64KB, <0.02% false positive
  rate at 10K keys), eliminating RwLock contention on the provider read path
- Faster `Instant` on ARM64

`compiler`:

- Compiler rewritten from scratch as a hand-rolled Pratt style parser, much faster than the old Pest
  based grammar.
- Lexical scopes and list/range comprehensions are now always enabled. The corresponding compiler
  options and feature compatibility checks were removed; deprecated daemon and `moorc` feature flags
  are accepted for old configs, but false values warn and are treated as enabled.

`objdef`:

- Report progress while collecting objects for a full objdef export and identify individual objects
  that take longer than five seconds.

`rpc`:

- `InvokeVerb`, `Command`, and `Eval` now carry an `InvocationMode` union instead of a client token.
  `ConnectedInvocation` runs with an interactive connection and sends output to its event stream.
  `InvokeVerb` and `Command` acknowledge submission. `Eval` retains its synchronous `EvalResult`
  reply. `CaptureOutputInvocation` runs with no connection. After the root task finishes, it returns
  one `InvocationResponse` with the outcome and committed narrative output. Successful and failed
  calls preserve this output. Both modes require an `AuthToken`. This principal is the player and
  the authority.
- A captured call waits at most `timeout_ms`, bounded by the new `runtime.max_capture_deadline`
  daemon setting (default 60 seconds, and never more than 300). A `timeout_ms` of zero asks for that
  configured maximum, so a caller need not guess. A request asking for longer is refused rather than
  clamped, and a deadline that passes cancels the task and answers with a `TaskAbortedLimit` time
  error.
- Captured output is bounded. A call that produces more than 10,000 events or 8 MiB of them fails
  rather than accumulating a response nobody can receive. Output the call addresses to another
  player is delivered to that player as usual rather than captured, and a forked task's output is
  published rather than returned.
- `CallSystemVerb` is replaced by `InvokeWelcomeMessage`, which takes no arguments and always
  invokes `#0:do_login_command` as `#0` with its output captured. There is no longer any way to call
  an arbitrary verb with system authority and no credentials.
- HTTP eval uses captured execution. Verb programming now uses only the auth token and creates no
  daemon connection. Webhook system handlers also run without connection state. They retain their
  response type and use the same deadline and cancellation logic.
- The HTTP command, eval, and verb-invocation endpoints accept an optional `timeout_ms` query
  parameter. An absent value or zero uses the daemon maximum. Invalid deadlines return HTTP 400.

### Fixed

- Change reviews account for inherited permissions when comparing value-only property overrides,
  avoiding false local edits after import.
- Meadow can refresh a stale change comparison against the running MOO. Refreshing asks for
  confirmation before clearing saved choices and unsaved edits.

- Tasks now report an error if they cannot load `$handle_uncaught_error`. Previously, these tasks
  could leave callers waiting indefinitely for a result.

`telnet-host`:

- Deliver inbound client-data message names as strings to prevent client-supplied GMCP and MSDP
  names from accumulating permanently in the symbol table.
- Honor client-initiated CHARSET requests and disable both negotiated directions when a connection
  option is turned off.
- Limit retained GMCP support declarations to 1,024 packages per connection and reject MXP command
  links containing decoded control characters or renderer delimiters.
- Use the daemon's JSON boolean mode from the first GMCP message, including connections accepted
  during host startup. MSSP now reports the actual connected-player count.

`kernel`:

- Match exception handlers by error code even when an error carries a value. Typed `except` clauses
  and error-catching expressions now catch these errors and retain their payloads.
- objdef dumps now include the calling task's pending changes. Reload and constants parsing use the
  configured language features. In-memory source labels no longer permit filesystem includes.
- A load or reload failure after writes begin now aborts the task and rolls back its transaction,
  including earlier writes. Catching an error cannot commit a partial import. Administrative imports
  retry commit conflicts and report success only after commit.

- Defer input, worker responses, explicit task resumes, and maintenance wakeups during GC sweep.
  Resumed tasks keep their return values or errors. Shutdown cancels these accepted responses
  instead of starting more execution or saving incomplete continuations.

- Keep expired task continuations visible to garbage collection and shutdown until dispatch. Timer
  and immediate wakeups now wait during GC sweep and stop dispatching during shutdown.
- Ignore queued wake signals from an earlier suspension of the same task. These signals could
  otherwise resume a task before its current wake condition was satisfied.

- Resume tasks queued during garbage collection when the collection finishes. Previously, new GC
  waiters could remain suspended until a server restart.
- Release GC admission and waiting tasks after a GC worker panic, so later collection cycles can
  run.
- Report task resumption and retry setup failures to callers, release dependent tasks, and settle
  scheduled firings. Retry setup errors no longer panic in the scheduler service.
- Ignore completion, retry, and panic callbacks from replaced task attempts. Late callbacks no
  longer remove a replacement task or publish its pending effects.
- Reject message and transaction-renewal requests from replaced task dispatches. Late requests no
  longer drain the replacement's mailbox or change its pending effects.
- Keep pending schedule changes with their originating task dispatch. Late create, stop, and query
  requests no longer change or expose a replacement task's unpublished schedules.
- Ignore stale session, fork, and task-control requests from replaced task dispatches. A late
  notification error or player-switch reply no longer changes the replacement task's identity or
  cancellation state.
- Retain terminal task results during session finalization, including cancellation and transaction
  renewal failure. Concurrent abort requests wait for the reserved result.
- Preserve empty enclosing scopes when calling nested parameterless lambdas, preventing a panic when
  their bodies access local bindings ([#554](https://github.com/timbran-project/moor/issues/554)).
  Stored program format 6 retains the entry layout; affected older programs need recompilation.
- Preserve JSON `null` values as `$nothing` (`#-1`) across `parse_json()` and `generate_json()`
- Handle failures while renewing a task transaction after commit without retrying or
  double-committing tasks in `suspend(0)`, immediate `task_recv()`, `commit()`, and fork dispatch
- Advance task ID allocation past restored suspended tasks so newly submitted tasks cannot reuse
  persisted IDs after restart
- Permission compatibility fixes for regular programmer/user operations:
  - Built-in object attribute assignment for `.name`, `.r`, `.w`, and `.f`, `move()`, `chparent()`,
    and `recycle()` now require owner-or-wizard authority instead of accepting public object write
    alone.
  - `add_verb()` no longer requires the caller to have the programmer bit when the caller otherwise
    has object write permission and owner authority.
  - `verb_code()` no longer requires the caller to have the programmer bit when the caller has read
    permission on the verb.
  - `delete_verb()` is authorized by write permission on the object defining the verb, not ownership
    of or write permission on the verb itself.
  - `object_bytes()` and `connection_name()` now check current task permissions rather than
    `caller_perms()`.
- `parse_command()` result map: `argstr` key/value types were swapped (key used `v_str`, value used
  `use_sym_or_str`) and `prep` key used `v_str` instead of `use_sym_or_str`.
- `kill_task(task_id())` now aborts the current task instead of completing successfully.
- `add_verb()` and `set_verb_info()` now reject empty and all-space verb names, matching LambdaMOO.
- Stale timer wheel entries no longer cause spurious task wakes; added monotonic generation counter
  to `TimerEntry` and `SuspendedTask` so that re-suspended tasks discard leftover timer entries from
  previous suspensions

`db`:

- `delete_property()` is authorized by write permission on the object defining the property, not
  ownership of or write permission on the property itself.
- `queue_info()` is now available to regular callers instead of requiring wizard permissions.

`daemon`:

- Daemon and host YAML configuration loading now uses direct serde/clap overlay logic instead of
  Figment, rejects unknown keys instead of silently ignoring stale or misspelled keys, and preserves
  the current config-file shapes. Book examples now use the current `features`, `import_export`, and
  `database` section names.

`docs`:

- Corrected book documentation for `verb_info()`, `verb_code()`, `set_verb_code()`, `add_verb()`,
  `delete_verb()`, and `queue_info()` permission and error behavior.

`compiler`:

- Duplicate constants (both duplicate names and duplicate values) now detected and rejected in
  objdef compiler
- Large objdef imports no longer stall while calculating verb source locations by repeatedly
  scanning each object file from the beginning
- Adjacent closing delimiters on nested flyweights no longer parse as right-shift operators
- Objdef imports and checkpoint exports reject empty verb names with actionable diagnostics
- Trailing newlines in objdef output corrected
- Fixed duplicate declarations in compiler roundtrip proptests

`infra`:

- SDK publish job fixed (missing build step, incorrect relative dirs)

`telnet-host`:

- Rich formatted telnet output (`text/djot` and `text/markdown`) now normalizes line endings to
  `CRLF` before sending to clients, avoiding layout issues in less-tolerant telnet clients

## [1.0.1] - 2026-06-14 (`v1.0-release` branch)

### Fixed

`kernel`:

- Permission compatibility fixes for regular programmer/user operations:
  - Built-in object attribute assignment for `.name`, `.r`, `.w`, and `.f`, `move()`, `chparent()`,
    and `recycle()` now require owner-or-wizard authority instead of accepting public object write
    alone.
  - `add_verb()` no longer requires the caller to have the programmer bit when the caller otherwise
    has object write permission and owner authority.
  - `verb_code()` no longer requires the caller to have the programmer bit when the caller has read
    permission on the verb.
  - `delete_verb()` is authorized by write permission on the object defining the verb, not ownership
    of or write permission on the verb itself.
  - `object_bytes()` and `connection_name()` now check current task permissions rather than
    `caller_perms()`.

`db`:

- `delete_property()` is authorized by write permission on the object defining the property, not
  ownership of or write permission on the property itself.
- `queue_info()` is now available to regular callers instead of requiring wizard permissions.

`docs`:

- Corrected book documentation for `verb_info()`, `verb_code()`, `set_verb_code()`, `add_verb()`,
  `delete_verb()`, and `queue_info()` permission and error behavior.

## [1.0.0-rc2] - Forthcoming (`v1.0-release` branch)

### Fixed

`kernel`:

- `parse_command()` result map: `argstr` key/value types were swapped and `prep` key used wrong
  string type
- `parse_command()` behaviour for matching "here" was broken for dobj/iobj matching. Now defaults to
  player location with an optional new argument to override.

## [1.0.0-rc1] - 2026-03-05 (`v1.0-release` branch)

### BREAKING: Database Migration Required

**This release breaks binary database compatibility** for the world DB, tasks DB, and connections
DB. This is due to the upgrade to fjall 3.0, and we took this opportunity to also improve opcode
encoding and the task/connection DB formats. This is intended to be the final breaking change before
the 1.0 stable release.

**Migration steps:**

1. Export your world to objdef format using `dump_database()` or the objdef dump tools
2. Shut down the server
3. Delete your entire binary database directory
4. Restart the server and re-import from your objdef dump

Databases from beta7 and earlier cannot be loaded directly.

### Added

`kernel`:

- New `task_send()` and `task_recv()` builtins for inter-task messaging; allows wizard or task owner
  to send transient messages to a task, with `task_recv()` supporting both blocking and non-blocking
  modes. Delivery is deferred until the sending task commits; reception is a transaction boundary
  similar to `read()`
- New `server_options.max_task_mailbox` to configure a cap on task mailbox size
- New `hotp()`, `totp()`, `random_bytes()`, `encode_base32()`, and `decode_base32()` builtins for
  two-factor authentication support (TOTP defaults to SHA256; use `'sha1` for Google Authenticator
  compatibility)
- `round()` now accepts optional second argument to specify decimal places
- `renumber()` now supports bidirectional conversion between numeric and UUID object IDs
- Flyweight slots can now be assigned in-place like maps and lists (e.g., `myfw.slot = value;`)
  without using `flyslotset()`. This is a copy-on-write operation.
- New `emit_data()` builtin for structured client state synchronization over the websocket
- New `flycontentsset()` builtin
- New `complex_matches()` builtin to return all matches from the best match tier
- `complex_matches()` now supports `"all "` and `"*."` prefixes to return matches from all tiers
  (exact, prefix, substring, fuzzy) in priority order instead of just the best tier

`daemon`:

- Support for both IPC and TCP ZeroMQ binding on same daemon
- New OAuth2 non-cookie flow support

`db`:

- Propagate more information on transaction conflict causes to logs
- Write optimization - handle all writes in one thread and batch together; when queue backs up,
  coalesce writes for the same tuple.
- Smart merging / optimistic concurrency improvements ("CRDT-lite"):
  - New "smart merging" for `List`, `Map`, `String`, and `Flyweight` types using operation hints to
    resolve conflicts automatically when changes are commutative (e.g., concurrent appends or unique
    key inserts)
  - Extensible `ConflictResolver` trait allowing for custom conflict resolution strategies
  - Support for idempotent writes (accepting identical values on conflict)
  - Improved trait bounds for database relations (`RelationDomain` and `RelationCodomain`)
  - Enhanced concurrency testing with expanded `shuttle` proptests

`testing`:

- Basic benchmarking "core" and simple bench tool for now just some example tick loops in an RPG
  style

`tools/moorc`:

- `--test-files` (alias `--test-directory`) supports recursive directories and glob patterns for
  moot tests

`web-sdk`:

- New published `web-sdk` package, extracted from the former in-repo web client to support Meadow
  and other web consumers
- New reusable mock web-host transport test harnesses for client testing
- Web-host version of MCP tool
- Centralized E2E decryption and DTO mapping

`docs`:

- New Flyweights chapter in the book
- OpenAPI 3.1.0 spec added and served at `/openapi.yaml`
- OpenAPI-derived WebSocket documentation added to the book

### Changed

`kernel`:

- Fork dispatch now commits and requests a fresh transaction before scheduling new tasks
- `queued_tasks()` prefers the top non-builtin activation for prepared tasks
- `parse_command()` now accepts optional 4th argument to specify fuzzy match threshold
- Stored program format bumped to version 3 to enforce hard incompatibility with chained assignments
  through builtin properties
- Scheduler immediate-wake handling has been optimized to reduce dispatch overhead
- Verb flag and permission metadata is now cached along execution flow to reduce repeated lookups
- `complex_match()` now defaults to no fuzzy matching (threshold `0.0`)
- `complex_match` ordinals count across match tiers and `N.subject` tokens are supported
- `pcre_match()` now uses boolean args for `case_matters` and `repeat_until_no_matches`

`daemon`:

- Improved server/client heartbeat for better detection of lost connections
- Add SIGUSR1 handler for graceful shutdown which also exports an objdef dump
- Trigger said SIGUSR1 when low-level (fjall) DB write failures occur
- API routes versioned under `/v1` prefix (replacing `/api/*`)

`web-host`:

- API surface was reorganized with stricter request/content negotiation handling
- OAuth2 flow was hardened and expanded for deployment support
- Explicit OpenAPI specification published

`telnet-host`:

- Default content output changed to plain ASCII
- Support for screenreader no-ansi-grafx mode

`db`:

- Shutdown on DB write failures instead of continuing in an inconsistent state
- Commit processing internals moved from channel-centric flow to lock-based coordination
- Write locking now uses `parking_lot::Mutex` for lower synchronization overhead
- Reduced transaction snapshot pointer churn and decoupled read-only cache publication
- Removed cache mutexes and centralized transaction-local mutability for cache paths
- `moor-db` internals were split/flattened to simplify module boundaries and public API usage

`deployment`:

- Added easier start scripts for running mooR locally.

`cores`:

- Default tick limit for `lambda-core` increased

`web-client`:

- The web client has been removed from this repository and renamed to
  [`meadow`](https://codeberg.org/timbran/meadow), now maintained as its own Git repository
- Shared TypeScript transport/client pieces were split into the published `web-sdk` package used by
  Meadow

`web-sdk`:

- Initial attach now skips reattach to preserve "connected" semantics
- WebSocket session/control-frame handling was centralized in `web-sdk` and shared by clients

`mcp-host`:

- JSON-RPC handling has been hardened for stricter 2.0 semantics, including batch request handling
  and proper notification behavior

`infra`:

- FlatBuffer schema now published to npm registry
- Switched to a maintained `rust-zeromq` fork

### Fixed

`kernel`:

- Allow task owners (not just wizards) to kill their suspended tasks
- `suspend()` no longer hangs indefinitely for very short (sub-millisecond) delays
- Set default return value for `suspend()` calls that might retry
- Property definition now correctly fails if a child already has a property with the same name
- `queued_tasks()` and `active_tasks()` now filter by `task_perms` correctly
- Fixed potential data race in `valid_task()`
- Reset task clock on commit failures
- Added missing `E_PERM` mapping for `!r` verbs
- Return `false`/`0` for unprogrammed verbs for LambdaMOO compatibility (#621)
- Use proper exponential backoff for transaction abort-retry
- Chained assignments through builtin properties (e.g., `this.location.inventory = xxx`) no longer
  cause E_PERM errors; fixed excessive redundant writes (#625)
- Variables beginning with `global` (e.g., `global_salt`) now compile correctly
- `range_set()` on strings now returns `E_RANGE` when the start index is out of bounds (#627)
- `complex_match()` now returns `E_INVARG` when keys and targets lengths differ
- `pcre_match()` now handles unmatched optional capture groups without panicking
- Fixed replacement behavior in `pcre_replace()` and avoided cascading failures after regex errors

`compiler`:

- Decompilation for optional lambda arguments now handled correctly
- Fixed decompilation for various set operations including `PutPropAt`
- Fixed decompiler output for computed verb names adding extra parentheses (#626)

`db`:

- Cleaned up transaction conflict UUID holder output in logs
- Fixed subtle race condition on verb/prop/ancest cache
- Ownership transfer semantics now match LambdaMOO/ToastStunt: only wizards may reassign object,
  property, or verb ownership

`daemon`:

- Connection handling improvements for soft/hard detach behavior; ping timeout fixes
- IPC worker processes no longer assume CURVE encryption is required
- Reduced log spam for dangling connections, `bf_respond_to`, remote verb invocations, and
  connection handling
- `bg_seconds()` and `fg_seconds()` now accept float arguments
- Event-log history no longer replays or re-records presentation events

`var`:

- `range_set()` on strings now handles UTF-8 offsetting correctly (#627)

`mcp-host`:

- Added ping/pong support for proper connection liveness checking
- Fixed resolve_object bugs
- Fixed connection management

`telnet-host`:

- UTF-8 multibyte character input sequences now handled correctly
- Handle multibyte IAC telnet sequences, passing them to `$do_out_of_band_command` as Binary
  payloads

`web-host`:

- Unify auth extraction and RPC client usage with typed Axum extractors
- Fixed initial websocket attach to skip reattach and preserve "connected" semantics
- Improved reconnect diagnostics

`web-sdk`:

- Fixed event ID attribution for historical event logging to prevent event duplication on refresh
- Fixed web MCP runners
- Patched places where `None` values were leaking through (e.g., `clear_property` now raises
  `E_INVARG` for properties with no parent)
- Allow multiple MOOs for web-mcp client
- Restored missing `ws-session` module after refactor

`infra`:

- Fixed missing license headers in several source files

`rpc`:

- Added deserialization coverage for `WorldStateError` and `CompileError` RPC error payloads

## [1.0.0-beta7] - 2026-01-01

### Added

`mcp-host`:

- MCP diff/patch tools for object and verb definitions
- Additional MCP command/eval tools, including agent compile-test support

`db`:

- Track commit and conflict counts in `db_counters`

`compiler`:

- Proptest-based AST roundtrip tests with expanded statement generators

`docs`:

- Web-client section added to the book with expanded guidance
- Document new boolean, task, and list builtins

`kernel`:

- New `all`, `none`, `tobool`, and `valid_task` builtins for truthiness checks and task validation

### Changed

`compiler`:

- Rename type constants to `TYPE_*` format Requires code migration.
- Allow `begin`/`end` as identifiers for compatibility

`core`:

- Disable `$recycler` by default in the LambdaMOO core fork

`kernel`:

- `slice()` now mirrors ToastStunt-style indexing with default index 1 and list/string handling
- `parse_json()` boolean conversions now honor boolean-return configuration
- `toint()` now converts booleans to 1 or 0, and boolean-returning builtins are consistent when
  enabled

`tooling`:

- Textdump import now more fully supports ToastStunt format
- Remove legacy textdump export support

### Fixed

`compiler`:

- Treat NBSP as whitespace in program input streams
- Prevent panic on freefloating `$` expressions
- Order parser keywords by length to avoid prefix matching errors
- Fix capture analysis for same-named lambda variables
- Fix transitive capture and chained lambda call handling

`kernel`:

- Correct line number offsets reported for `for`-range errors
- Align `bf_isa` behavior with the ToastStunt equivalent

## [1.0.0-beta6] - 2025-12-28

### Added

`daemon`:

- New `event_log` builtin for explicitly logging to player event history without immediate output
- New `urlencode` and `urldecode` builtins for URL encoding/decoding
- ASCII art banner at launch

`web-client`:

- **Rewritable prompt events** - send placeholder messages that can be replaced later (useful for
  background processing or multi-step workflows)
- **Inline command links** - MOO output can now include clickable links and inspection operations
- **Collapsible look headings** - look output sections can be collapsed
- **Player creation wizard** - improved new player flow with an (optional) privacy policy stage and
  pronouns selection and thumbnail/avatar pic upload
- Add ability to force reload/import from objdef files in object browser
- Application-level WebSocket keep-alive messages to prevent proxy timeouts
- Object browser verb editor can now edit verb names and argument specs

### Changed

`daemon`:

- Connections and tasks databases now compact every 5 minutes to prevent journal bloat
- Stale connections pruned from connections DB at startup and periodically
- Connection timestamp updates batched and compaction runs asynchronously

`db`:

- Exponential backoff added to transaction start spin-loop to reduce CPU waste under contention
  (#586)

`web-client`:

- Improved colour system for both dark and light themes
- Code editors now use comic-mono font
- General object browser look & feel improvements
- Command history preserves edits when navigating with arrow keys
- Auto-say mode disabled by default

### Fixed

`compiler`:

- Arrow lambda parameter scopes now properly isolated (fixes DuplicateVariable errors when multiple
  arrow lambdas use the same parameter name)
- Assignment into captured variables in lambdas now forbidden to reduce confusion
- Multi-statement lambdas now handled correctly in decompile and unparse (#583)
- Transitive capture for nested lambdas (outer lambdas now correctly capture variables needed by
  inner lambdas)

`daemon`:

- Textdump writer crash on UUID objects (#584)

`tooling`:

- Objdef dump bug where flyweight delegates were not being substituted with symbolic constants
- Import/export ID names now validated and de-duplicated on dump

`web-client`:

- Light theme popup styling
- Object browser verb editor: fixed 'd' flag not rendering/editing properly
- Object browser verb editor: fixed multi-aliased verb names getting lost when editing

## [1.0.0-beta5] - 2025-12-20

### Added

`daemon`:

- New floating-point math builtins
- New `html_query` builtin for fuzzy HTML parsing
- `listen()` now accepts ToastStunt-style options maps
- `bf_eval` can accept predefined variable bindings

`telnet-host`:

- TLS support for inbound telnet connections
- TCP keep-alive configuration options

`web-client`:

- Verb suggestions palette with optional auto-say mode
- Link previews and an iframe-based welcome screen

`mcp-host`:

- Support for requesting external core MCP tools from a running MOO

### Changed

`web-host`:

- WebSocket auth now uses a subprotocol to pass tokens and client metadata

`telnet-host`:

- Djot formatting replaces markdown rendering

`curl-worker`:

- TLS stack now uses rustls, removing the implicit OpenSSL dependency

### Fixed

`daemon`:

- UTF-8 correctness in `match`/`rmatch` and string search/replace
- Password prompt flow and login command handling
- `parse_command` environment setup errors
- New task scheduling now runs immediately again

`telnet-host`:

- Listener port mismatch in argument handling

`web-client`:

- Mobile sizing regressions

## [1.0.0-beta4] - 2025-12-10

### Added

`web-client`:

- **Generic text editor component** - presentable editor for editing descriptions, mail, notes, etc.
- **Speech bubbles** for `say` events (optional, disabled by default) for cores that tag their
  events right
- **Upload prompt** for image uploads (thumbnails, etc.) allows uploading of images to cores that
  support it.

testing:

- LambdaMOO compatibility test harness for comparative testing against the reference implementation

### Changed

`daemon`:

- **Major performance improvements** to the core `Var` type and VM execution:
  - Restructured core value type from Rust enum to tagged union for faster cloning
  - Optimized type dispatch logic throughout the VM
  - Switched string storage to use `arcstr` for more efficient string handling
  - Refactored VM environment and activation construction for reduced overhead
  - Reduced copies in FlatBuffers serialization for RPC layer
- Sequence updates moved off the main thread for improved concurrency

### Fixed

`daemon`:

- **CRITICAL**: Fixed garbage collection bug where anonymous objects located inside other objects
  were incorrectly collected (objects whose only reference was their container were being lost)
- Fixed lambda capture analysis to correctly handle nested scopes (was incorrectly capturing
  variables from nested lambdas and inner scopes)

packaging / release:

- Fixed deb package signing in workflow and instructions

---

## [1.0.0-beta3] - 2025-12-03

### Added

`mcp-host`:

- **New Model Context Protocol (MCP) server** - enables AI assistants like Claude to interact with
  MOO worlds
  - Connect as wizard or player with configurable permissions
  - Automatic reconnection support for long-running sessions
  - Rich prompts and function help for AI interaction
  - Object definition dump/diff capabilities
  - Included in Docker images
  - Debian package support for distribution through Debian repository

`daemon`:

- New "yes to all" form option for prompts
- Script for verifying that builtin table changes are stable

packaging / release:

- GPG signing support for all Debian packages built by cargo-deb and dpkg-deb
  - Workflow now imports GPG key from secrets and signs packages automatically
  - Users can verify package authenticity with `dpkg-sig --verify`

### Changed

`daemon`:

- `ftime()` builtin now accepts boolean argument (not just int) for better type safety

`web-client`:

- Continued TTS (text-to-speech) improvements
- Better display and reading of 'inset' content panels

`db`:

- Performance optimization: skip conflict checking when no concurrent commits occur
- Relations now marked as fully loaded at initial import, which then skips later loads, meaning perf
  improvements in transaction/database layer that should help avoid gratuitous disk seeks.

### Fixed

`daemon`:

- **CRITICAL**: Fixed garbage collection bug with anonymous objects that could cause accidental
  premature collection of objects that still had references
- Fixed `parse_json` to handle null JSON values like ToastStunt for compatibility
- Fixed `none` values incorrectly appearing in error stack frames in exception handling
- Fixed regression in `ftime()` caused by off-by-1 error in argument handling

`telnet-host`:

- Better handling of linefeeds from markdown/djot formatted content

`web-client`:

- Improved display of inset content panels
- General accessibility / TTS improvements

packaging / release:

- Fixed Docker build failures caused by missing `moor-mcp-host` binary in release builds
- Fixed deployment directory permissions for Docker setups
- Removed unused .env files from deploy directories

testing:

- Added script to verify builtin table stability

---

## [1.0.0-beta2] - 2025-11-26

### Added

- Book documentation for `parse_command` and related builtins
- Object browser can be opened focused on a specific object from MOO code
- Web client eval panel has proper return value MOO literal output

### Changed

- Upgraded ariadne library for compiler error reporting
- Anonymous objects can now be transmitted over RPC to hosts (but not for use as a stored reference,
  e.g. not traced for GC)
- Dependency updates (Cargo, npm)

### Fixed

`daemon`:

- Critical bug fix: property and verb cache keys now work correctly with UUID objids and anonymous
  objects (#575)
- `handle_task_timeout` and `handle_uncaught_error` verbs now called correctly
- Fixes to line numbers & file names in objdef compilation errors. They were showing inconsistently.

`web-client`:

- Web client reconnection (hopefully) no longer spams connection attempts (#567)
- Web client disconnection events now fire correctly
- Web-client object browser pre-selection fixes
- Web client remembers user's encryption prompt choice

packaging / release:

- IPC socket directories for Debian package installs

### Known Issues

`web-client`:

- TTS dictation inside "inset" panels constantly repeats itself as new events are added to them.
- Odd formatting of spacing inside the fixed-width "ANSI graphics" eval error output

---

## [1.0.0-beta1] - 2025-11-18

### Status

**This marks the beginning of the 1.0-beta phase.** The core runtime and database formats are now
considered stable. mooR is in feature freeze, with focus on bug fixes, documentation, and
performance optimization leading up to the 1.0 stable release.

### Added

- Official pre-built Docker images available on Codeberg Container Registry
  - `codeberg.org/timbran/moor:latest-x86_64` and `latest-aarch64` for backend services
  - `codeberg.org/timbran/moor-frontend:latest-x86_64` and `latest-aarch64` for frontend
- Debian packages published to Codeberg Debian package repository
- Production deployment configurations in `deploy/` directory:
  - `telnet-only/` - Minimal telnet-only setup
  - `web-basic/` - Web-enabled HTTP deployment
  - `web-ssl/` - Production HTTPS with Let's Encrypt
- Comprehensive deployment documentation in README files
- Formal ChangeLog documenting release history and stability commitments

### Changed

- **Database format is now stable**: Database format version `release-1.0.0` is locked for the beta
  period
  - Pre-beta version 3.0.0 databases are automatically migrated to `release-1.0.0` on first startup
  - Migration is a simple version marker update (no data format changes)
  - No breaking format changes expected before stable 1.0 release
  - LambdaMOO 1.8.x textdump databases continue to be fully supported
- Simplified database migration: Older pre-beta formats (1.0.0, 2.0.0) no longer supported for
  direct migration
- Docker Compose examples now default to pre-built images from Codeberg
- Development docker-compose.yml includes improved documentation for importing traditional textdump
  databases
- README updated to reflect beta status and feature freeze

### Fixed

- GPL-3.0 license metadata in Debian packages
- Docker build resource management for multi-architecture builds

### Known Issues

- None documented yet - this is the baseline for the 1.0-beta phase
