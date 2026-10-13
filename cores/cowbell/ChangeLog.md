# Cowbell changelog

This changelog records features, significant bug fixes, compatibility changes, and upgrade notes for
the Cowbell core. See the [mooR changelog](../../ChangeLog.md) for server changes and earlier
Cowbell entries.

## Unreleased

### Added

- Add MOO parsing for an explicit Djot authoring profile, with source-location errors for
  unsupported syntax. Speech and help now preserve authored formatting in HTML, Djot, and plain
  text.
- Add `@describe --djot` and `--literal` to select description syntax. Description, note, and letter
  edits validate their selected syntax before saving.
- Add document values with validation, metadata encoding, and HTML, Djot, and plain text renderers
  for core code.
- Add `list:find_index(predicate)` to distinguish a matching value of zero from no match.

### Changed

- Rule conditions now use named predicate arguments and explicit input references. For example,
  `This owner_is(Accessor)?` becomes `owner_is(object: input.This, owner: input.Accessor)`. Recreate
  saved rules and reaction conditions in the new format. The previous nested-list rule
  representation and implicit object-name matching are no longer supported.
- Custom rule predicates require named input and output declarations through
  `$rule_engine:register_predicate`. Rules no longer discover arbitrary `fact_*` verbs on objects.
- Install custom reactions with `$reaction:install(host, property, reaction)` or the builder
  commands. Raw property assignment no longer authorizes execution. Reinstall approved reactions
  after an ownership transfer or definition change. Shipped objects use the same installer during
  world initialization. Direct `fire_trigger` calls require the host owner or a wizard.

- Notifications and narrative history carry the resolved document in `cowbell_document` metadata.
  Connections share the captured content and reuse renderings for the same format.
- Login output uses the document pipeline before authentication. Welcome banners select their
  authored syntax with `welcome_message_kind`; inserted server names remain literal text.
- Remove unused `$ansi`, `$html`, and event content-type preference helpers.

- Room narrative and panels share one captured view. Meadow preserves formatted descriptions and
  captured references in its room panel. Room snapshots now carry a rendered body and annotations.

- Descriptions no longer infer substitution templates from braces. Use explicit substitution
  constructors. Notes and letters use `content_kind` (`literal` or `djot_blocks`) in place of
  output-format preferences.
- Events now resolve viewer-specific document content before rendering. Delivery reuses that
  document across connection formats. Movement messages retain the action location and resolve actor
  and observer wording separately.
- Replace the content `compose`, `render_as`, and event `transform_for` APIs with constructors,
  `event:resolve(viewer)`, and `$document:render(document, format)`. Strings now represent literal
  text. Authored markup requires an explicit Djot input kind.
- Source listings store line numbers separately from source text. Renderers add the number gutter.
- Agent reports, private messages, and command guidance use document nodes. Output keeps its
  structure across HTML, Djot, and plain text without embedded ANSI or generated markup strings.

### Fixed

- Nested rule conditions preserve their grouping during evaluation and when displayed for editing.
  Negation accepts inputs supplied by the operation. Unknown inputs and predicates are rejected
  before a condition is saved.
- Reaction dispatch no longer borrows a new owner's authority after an ownership transfer. Copied or
  edited installation grants cannot authorize effects. Delayed effects recheck the installation
  before execution. State effects cannot change built-in object properties.
- Reject forged trigger requests and threshold reports. Movement reactions require native movement
  callbacks, and `$reaction:change` checks thresholds against the values it actually changes.
- Missing or broken rule predicates remain errors under negation. Reaction construction rejects
  unsupported conditions, and rule parsing rejects trailing input.
- Rule and reaction builder commands now edit properties with the authenticated builder's authority.

- Meadow rewrites replace the body, annotations, and attached document together. Rejected rewrites
  leave the original message available for an authorized replacement.
- Contextual help for doors, social actions, mail, and navigation retains its authored formatting.

- Wearing LLM devices no longer fails with a numeric type error when displaying token usage. The
  diagnostic status method uses the same corrected calculation.
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
