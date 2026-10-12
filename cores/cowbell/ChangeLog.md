# Cowbell changelog

This changelog records features, significant bug fixes, compatibility changes, and upgrade notes for
the Cowbell core. See the [mooR changelog](../../ChangeLog.md) for server changes and earlier
Cowbell entries.

## Unreleased

### Added

- Add MOO parsing for an explicit Djot authoring profile, with source-location errors for unsupported syntax.
  Speech and help now preserve authored formatting in HTML, Djot, and plain text.
- Add `@describe --djot` and `--literal` to select description syntax. Description, note, and letter
  edits validate their selected syntax before saving.
- Add document values with validation, metadata encoding, and HTML, Djot, and plain text renderers
  for core code.
- Add `list:find_index(predicate)` to distinguish a matching value of zero from no match.

### Changed

- Room narrative and panels share one captured view. Meadow preserves formatted descriptions and
  captured references in its room panel. Room snapshots now carry a rendered body and annotations.

- Descriptions no longer infer substitution templates from braces. Use explicit substitution constructors.
  Notes and letters use `content_kind` (`literal` or `djot_blocks`) in place of output-format preferences.
- Events now resolve viewer-specific document content before rendering. Delivery reuses that
  document across connection formats. Movement messages retain the action location and resolve
  actor and observer wording separately.
- Replace the content `compose`, `render_as`, and event `transform_for` APIs with constructors,
  `event:resolve(viewer)`, and `$document:render(document, format)`. Strings now represent literal
  text. Authored markup requires an explicit Djot input kind.
- Source listings store line numbers separately from source text. Renderers add the number gutter.
- Agent reports, private messages, and command guidance use document nodes. Output keeps its
  structure across HTML, Djot, and plain text without embedded ANSI or generated markup strings.

### Fixed

- Wearing LLM devices no longer fails with a numeric type error when displaying token usage.
  The diagnostic status method uses the same corrected calculation.
- Help topics and command lists use columns again in rich clients, including commands with separate
  programmer source links. Ordinary prose and numbered lists keep their existing layout.
- Closed-container looks resolve denial messages with the container binding and preserve structured
  descriptions. Code documentation no longer appears empty when supplied as a code-block value.
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
