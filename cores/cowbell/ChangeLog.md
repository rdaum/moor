# Cowbell changelog

This changelog records features, significant bug fixes, compatibility changes, and upgrade notes for
the Cowbell core. See the [mooR changelog](../../ChangeLog.md) for server changes and earlier
Cowbell entries.

## Unreleased

### Added

- Players can enter custom pronouns with `@pronouns custom` or Meadow's account and profile editors.
  Sentence examples explain each form, and a preview precedes confirmation. Both interfaces use the
  same validated setter; cancellation keeps the previous pronouns.
- `@preview-message` shows draft messages from three example readers' perspectives before saving.
  Reactions support `preview-message` and include example sentences in their existing previews.
  Private reaction messages also supply the recipient as a named binding.
- Message editors and reaction message steps check the bindings supplied by their producer. Mistakes
  include a character position and a suggested name. `@message-bindings` explains the available
  participants. Custom message producers must declare bindings before authors use them.
- Message templates can select pronouns for any named participant with `{pronoun BINDING FORM}`. The
  participant sees “you” and “your”; other readers see that participant's chosen pronouns.
- Add a [builder's guide](manual/reactions.md) with command examples for reactions, membership
  rules, and a timed puzzle. These workflows require builder privileges, with no MOO programming.
- Add MOO parsing for an explicit Djot authoring profile, with source-location errors for
  unsupported syntax. Speech and help now preserve authored formatting in HTML, Djot, and plain
  text.
- Add `@describe --djot` and `--literal` to select description syntax. Description, note, and letter
  edits validate their selected syntax before saving.
- Add document values with validation, metadata encoding, and HTML, Djot, and plain text renderers
  for core code.
- Add `list:find_index(predicate)` to distinguish a matching value of zero from no match.

### Changed

- Relations define their own named fields. Rules and reaction steps reference a relation object or a
  reaction setting, so builders can reuse field names without sharing data. Queries can bind either
  side of a relation. Copies retain their chosen relation; change the setting to use a separate one.
  Incompatible fields and unavailable relations produce errors.
- Replace global relation registrations with `@facts OBJECT define FIELD:TYPE ...`. Use
  `fact in OBJECT ...`, `record in OBJECT ...`, and `forget in OBJECT ...`, or name a reaction
  setting in place of the object. Recreate rules and steps that used registered relation names.
  Stored tuples remain in their relation objects. Re-register extension predicates after upgrading;
  their registry now contains query providers only.

- Builders can author reactions with `@reaction`, declare state with `@state`, set access rules with
  `@rule`, and keep named facts with `@facts`. The commands require no programmer privileges or MOO
  literals. Help, inspection, preview, execution reports, and copying use the same services as
  tools.
- New reactions start disabled. Reactions for an event run by name, and their steps run in order.
  Match selection is explicit. Removing a reaction cancels its pending timers.
- The `@reaction` workflow replaces the raw-list `@add-reaction` and related editing commands. The
  `author_reaction` tool replaces the separate add and enable tools. Reinstall existing reactions
  after upgrading their stored definitions.

- Containers use the same checks and reactions for commands and actions. Taking an item directly
  also fires its container's take reaction. Viewing contents has its own `view_rule`.
- Actions take named arguments and report success, denial, or error. Wearing and removing items now
  emit their completed reactions. Passage conditions name `From` and `To`; key-independent rules no
  longer require a carried item.
- Objects can declare typed counters and state choices. Named timers support replacement and
  cancellation, recheck conditions when they wake, and stop after an installation is disabled or
  replaced. Reaction chains and pending timers have explicit limits.
- Reinstall existing reactions after upgrading. Action effects now use named value expressions.
  Replace anonymous `delay` effects with named `schedule` effects.

- Relations store each tuple once. Repeated assertion returns the existing identity, and one
  retraction removes the fact. The first write repairs duplicate records and indexes from older
  worlds. Owners can also run `relation:repair()` explicitly.
- Relation queries return distinct, case-sensitive answers. `query` raises an error if its result
  limit prevents a complete answer. `query_result` returns rows with an explicit completeness flag.

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

- Empty `say` and `emote` commands ask for text instead of failing in the Djot parser. Inline
  authoring also accepts empty content.
- Messages can select verb agreement for a participant's name or pronoun explicitly, including named
  object bindings. Use `{agree actor name:are|is}` for “Alex is” and `{agree actor pronoun:are|is}`
  for “They are.” Privilege announcements, furniture messages, and container refusals now agree with
  the participant they describe.

- Nested message alternatives now resolve their substitutions and preserve branch whitespace. Escape
  literal braces, pipes, and backslashes with a backslash. Stray closing braces are errors.
- Displayed message templates preserve capitalization, nested alternatives, and literal punctuation
  when edited again. Unsupported content raises an error instead of producing misleading text. Use
  `{cap CONTENT}`, `{bind NAME}`, and `{content NAME}` for explicit operations.

- Capitalized substitutions handle Unicode initials, including accented names, without changing the
  rest of the name.

- Article templates preserve capitalization and binding names ending in `c`. Mass nouns and plural
  nouns omit indefinite articles. Common pronunciation exceptions select `a` or `an`; set a thing's
  `indefinite_article` to override the guess, or leave it empty for automatic selection.

- Actors and things accept `pronoun_possessive('noun)` and `pronoun_possessive('adj)` consistently.
  Pronoun sets reject empty forms, missing fields, and invalid agreement flags.

- `@builder` and `@programmer` can ask for confirmation when a player has no description.

- Message templates reject unfinished braces and empty tokens immediately. Compilation has limits on
  source length, token count, and nesting depth.

- `@dig` works with delegated building permissions in another owner's area. Its duplicate passage
  check uses the room's passage list without requiring direct access to the area's private relation.
- Cowbell package updates include the rules schema, condition, and facts service objects and their
  constants.
- Programmer message commands accept calls through the programmer feature object, including
  `@set-message` and message bag edits. `@program` reads input from the player without requiring
  wizard privileges.
- Passage lookup preserves the caller's permissions. Owners and wizards can query a new area's empty
  passage relation without a permission error.
- Henri's interaction responses now use one set of editable reactions. Refused nearby pet attempts
  count toward his sleepy mood, and accepted feeding resets that counter. Petting and feeding check
  the actor, location, and access rules before reporting completion. Replace custom uses of
  `pets_received` with the declared `pet_attempts` counter.
- Removing an inherited reaction now removes its response on that object without changing its parent
  or leaving its timers active.
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
