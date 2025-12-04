# Changelog

All notable changes to mooR will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
