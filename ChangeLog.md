# Changelog

All notable changes to mooR will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

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
- Flyweight slots can now be assigned in-place like maps and lists (e.g., `myfw.slot = value;`)
  without using `flyslotset()`. This is a copy-on-write operation.
- New `emit_data()` builtin for structured client state synchronization over the websocket
- New `flycontentsset()` builtin

`list_sets`:

- New `complex_matches()` builtin to return all matches from the best match tier
- `complex_matches()` now supports `"all "` and `"*."` prefixes to return matches from all tiers
  (exact, prefix, substring, fuzzy) in priority order instead of just the best tier

`daemon`:

- Support for both IPC and TCP 0mq binding on same daemon
- New OAuth2 non-cookie flow support

`db`:

- Propagate more information on transaction conflict causes to logs

`testing`:

- Basic benchmarking "core" and simple bench tool for now just an example tick loop
- New "RPG"-like bench core utility

`db`:

- Write optimization - handle all writes in one thread and batch together; when queue backs up,
  coalesce writes for the same tuple.
- **Smart Merging / Optimistic Concurrency Improvements aka "CDRT-lite" **:
  - New "smart merging" for `List`, `Map`, `String`, and `Flyweight` types using operation hints to
    resolve conflicts automatically when changes are commutative (e.g., concurrent appends or unique
    key inserts)
  - Extensible `ConflictResolver` trait allowing for custom conflict resolution strategies
  - Support for idempotent writes (accepting identical values on conflict)
  - Improved trait bounds for database relations (`RelationDomain` and `RelationCodomain`)
  - Enhanced concurrency testing with expanded `shuttle` proptests

`tools/moorc`:

- `--test-files` (alias `--test-directory`) supports recursive directories and glob patterns for
  moot tests

`web-client`:

- Object browser can run `test_` verbs (single or all) with results dialog and test-verb toggle
- External links in MOO output can now be opened (previously not supported); links are highlighted
  and display a confirmation modal before opening
- Auto-emoji conversion (e.g., `:-)` to unicode emoji) - enabled by default, configurable; requires
  core support (event must explicitly mark that emojis are present)
- Improved verb editor pop-out behavior (better positioning, deselects verb in object browser)
- Interactive UUID-based object IDs with click-to-copy support and toast notification feedback
- Object ID highlighting now works for plain object IDs, not just UUID-based ones

`web-sdk`:

- New reusable mock web-host transport test harnesses for client testing
- Web-host version of MCP tool
- Centralized E2E decryption and DTO mapping

`docs`:

- New Flyweights chapter in the book
- OpenAPI 3.1.0 spec added and served at `/openapi.yaml`
- OpenAPI-derived WebSocket documentation added to the book

### Changed

`list_sets`:

- `complex_match()` now defaults to no fuzzy matching (threshold `0.0`)
- `complex_match` ordinals count across match tiers and `N.subject` tokens are supported

`kernel`:

- Fork dispatch now commits and requests a fresh transaction before scheduling new tasks
- `queued_tasks()` prefers the top non-builtin activation for prepared tasks
- `parse_command()` now accepts optional 4th argument to specify fuzzy match threshold
- Stored program format bumped to version 3 to enforce hard incompatibility with chained
  assignments through builtin properties

`regex`:

- `pcre_match()` now uses boolean args for `case_matters` and `repeat_until_no_matches`

`daemon`:

- Improved server/client heartbeat for better detection of lost connections
- Add SIGUSR1 handler for graceful shutdown which also exports an objdef dump
- Trigger said SIGUSR1 when low-level (fjall) DB write failures occur
- API routes versioned under `/v1` prefix (replacing `/api/*`)

`telnet-host`:

- Default content output changed to plain ASCII
- Support for screenreader no-ansi-grafx mode

`db`:

- Shutdown on DB write failures instead of continuing in an inconsistent state

`deployment`:

- Added easier start scripts for running mooR locally.

`core`:

- Default tick limit for `lambda-core` increased

`web-client`:

- Web-client split into its own repository [`meadow`](https://codeberg.org/timbran/meadow) to
  decouple release cadences and separate issue tracking
- Generic TypeScript webclient pieces split into separate web-sdk NPM package

`web-sdk`:

- Initial attach now skips reattach to preserve "connected" semantics

`infra`:

- FlatBuffer schema now published to npm registry

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

`list_sets`:

- `complex_match()` now returns `E_INVARG` when keys and targets lengths differ

`regex`:

- `pcre_match()` now handles unmatched optional capture groups without panicking

`compiler`:

- Decompilation for optional lambda arguments now handled correctly
- Fixed decompilation for various set operations including `PutPropAt`

`db`:

- Cleaned up transaction conflict UUID holder output in logs
- Fixed subtle race condition on verb/prop/ancest cache

`daemon`:

- Connection handling improvements for soft/hard detach behavior; ping timeout fixes
- IPC worker processes no longer assume CURVE encryption is required
- Reduced log spam for dangling connections, `bf_respond_to`, remote verb invocations, and
  connection handling
- `bg_seconds()` and `fg_seconds()` now accept float arguments

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

`web-client`:

- Duplicate completions in verb editor fixed
- Anonymous objects no longer shown in object browser
- Fixed missing image and source files in build
- Throttled inbound narrative DOM additions to improve performance
- Fixed TTS (text-to-speech) issues with skipped batch events and DOM restructuring
- Added warning for unsaved changes in verb editor and fixed interruptions during compilation
- Avoid missing events while tab is backgrounded/idle
- Screenreaders now skip reading thumbnails in descriptions
- Various accessibility improvements (verb editor discoverability, eval panel ARIA, room
  descriptions)
- Accessibility support enabled for all editors (property editor, eval panel, general text editor),
  not just verb editor (#620)
- Fixed prepopulated owner field for new objects/verbs/properties defaulting to selected item's
  owner instead of player
- Fixed build errors after Monaco editor upgrade

`infra`:

- Fixed missing license headers in several source files

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

- ?

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

---

## Future Versions

Development will focus on:

- Bug fixes and stability improvements
- Performance optimization
- Documentation expansion
- Community feedback integration

No new features are planned before the 1.0 stable release.

---

## ChangeLog Template

When preparing a new release, use this template for a new version section:

```markdown
## [X.X.X] - YYYY-MM-DD

### Added

- New feature descriptions

### Changed

- Modified behavior descriptions

### Fixed

- Bug fix descriptions

### Known Issues

- Known limitations or issues
```

Guidelines:

- Use semantic versioning for release versions
- Date format: YYYY-MM-DD
- Group changes by type (Added, Changed, Fixed, Known Issues)
- Write clear, user-facing descriptions
- Link to relevant issues or PRs where helpful
