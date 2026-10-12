# Cowbell changelog

This changelog records features, significant bug fixes, compatibility changes, and upgrade notes for
the Cowbell core. See the [mooR changelog](../../ChangeLog.md) for server changes and earlier
Cowbell entries.

## Unreleased

### Added

- Add `list:find_index(predicate)` to distinguish a matching value of zero from no match.

### Fixed

- `list:unique()` now accepts nested lists and maps and preserves the first occurrence.
- Event queues now enforce the caller's property permissions for reads and writes. Calling a queue
  method no longer grants wizard access to another owner's queue.
