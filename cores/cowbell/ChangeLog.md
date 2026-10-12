# Cowbell changelog

This changelog records features, significant bug fixes, compatibility changes, and upgrade notes for
the Cowbell core. See the [mooR changelog](../../ChangeLog.md) for server changes and earlier
Cowbell entries.

## Unreleased

### Added

- Add document values with validation, metadata encoding, and HTML, Djot, and plain text renderers
  for core code.
- Add `list:find_index(predicate)` to distinguish a matching value of zero from no match.

### Fixed

- `string:to_list("")` returns the whole input as one field instead of looping indefinitely.
- `string:split()` and `string:replace_all()` now match case consistently for every delimiter
  length. Matching is case-sensitive by default. Pass `false` as the final argument to ignore case.
  `string:to_list()` retains its case-insensitive default and accepts `true` for exact matching.
- Negative widths in `$str_proto:space()` anchor repeated fill on the right. Empty fill raises
  `E_INVARG` when padding is required, instead of dividing by zero.
- `string:words_list()` splits tabs, carriage returns, and newlines as well as spaces.
- `string:is_numeric()` returns a boolean instead of a regex match list.
- `list:unique()` now accepts nested lists and maps and preserves the first occurrence.
- Event queues now enforce the caller's property permissions for reads and writes. Calling a queue
  method no longer grants wizard access to another owner's queue.
