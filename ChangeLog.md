# Changelog

All notable changes to mooR will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0-beta1] - 2025-11-18

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

## [Unreleased]

### Added

-

### Changed

-

### Fixed

-

### Known Issues

-

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
