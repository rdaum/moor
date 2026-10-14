<!-- Copyright (C) 2026 The mooR Authors -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Contextual suggestions

Suggestions are read-only queries over Cowbell's command environment. They do not execute commands,
run transfer policies, or predict whether an action will succeed. Meadow's inspector is the first
consumer; the request and response are independent of that UI.

## Request and response

Any object inherits this endpoint:

```moo
provider:suggestions(source, query, ?context = [], ?limit = 12)
```

`source` names a candidate source and `query` is the text being entered. An optional context
identifies a template, its active slot, and already supplied arguments:

```moo
$match:input_context("put {dobj} in {iobj}", "dobj", ["iobj" -> "#48"])
// ["template" -> "put {dobj} in {iobj}", "active" -> "dobj", "bindings" -> ["iobj" -> "#48"], ...]
```

Templates have at most two named slots (`dobj`, `iobj`, or the inspector's single `input` slot). The
active slot must occupy a complete parser argument. Binding values are single-line command text,
usually exact `#` references. Missing bindings are provisional arguments, not empty strings. A
request returns at most 50 rows; query and template lengths are limited to 256 and 1024 characters.
Omit the context for an unconstrained source query.

The authenticated task player supplies the viewing identity. Callers cannot request another player's
private inventory by changing the provider.

```text
{
  items: [{ id, label, value, detail }],
  more: boolean
}
```

`id` distinguishes choices; `label` is shown in the field; `value` is inserted into the command. For
object choices, `value` and `detail` contain the object reference. Duplicate names therefore remain
distinguishable and selecting one cannot resolve to a different object because of its name. `more`
asks the client to narrow the query; the endpoint does not return an unbounded catalog.

## Shared environmental search

`player:match_environment(command, context)` accepts these context keys:

| Key       | Meaning                                                             |
| --------- | ------------------------------------------------------------------- |
| `'scope`  | `"nearby"` (default), `"inventory"`, or `"contents"`                |
| `'target` | The container for `contents`; the object to exclude for `inventory` |

The default retains the command scope: player, inventory, worn items, mailbox, and room-contributed
objects/aliases. Inventory narrows that scope to carried items. Contents expands into an explicitly
specified, reachable target through `target:match_scope_for(player, context)`.

Rooms and containers contribute through the same `match_scope_for` hook. Containers expose contents
only while reachable and open and when their existing general viewing rule permits it. `contents()`
and `look_self()` share that visibility check, including a reach/open recheck after authored rule
callbacks. Object names, aliases, and additional environmental aliases enter the shared ranking
step; repeated scope entries collapse to one row while retaining their aliases.

For command-name suggestions, query the player with source `"commands"`. This uses the existing
`verb_suggestions()` catalog, whose discovery walks `command_environment()`. The command strip
continues using that catalog's richer signature/hint metadata.

## Matching a known command

When a context is supplied, `$match:matching_suggestions`:

1. Parses the template once using `parse_command`, with a placeholder in its unfilled object slot.
2. Substitutes each scoped object into that slot in the parsed command map.
3. Calls `find_command_verb` against the player's existing command environment, including direct and
   indirect targets and inherited verb signatures.
4. Keeps candidates with a matching command verb. An ambiguous fixed argument is checked against
   each of its parser-provided candidates, as in normal command dispatch.

With both slots unfilled, the matcher probes possible receivers for an `any` active argspec and
checks candidate receivers for a `this` constraint. It does not enumerate every pair of arguments.
Binding either slot narrows subsequent checks for the other.

`provider:suggestion_eligibility(source, references, ?context = [])` checks up to 64 canonical
references against that same complete candidate source before ranking. Each result has `eligible`
and, when eligible, `label` and `value`. A refused reference has a generic `reason` without a label.
An object need not appear on the current suggestion page to be eligible. This is the transcript
selection path; its result remains advisory.

This checks verb names, prepositions, and direct/indirect argspecs without calling
`dispatch_command_verb`. It does not reveal the correct key by evaluating an unlock policy. Custom
room handlers outside normal verb matching need an authored candidate source if they want to offer
completion.

The shared ranker orders exact, prefix, word-prefix, and substring matches, case-insensitively. It
retains provider order within each rank and strips internal search keys from the response.

## Authoring sources

Objects can override `suggestion_candidates(source, query, context)` for domain-specific choices,
returning candidate maps with `id`, `label`, `value`, optional `detail`, and optional `keys`
(aliases). Use `pass(@args)` for the environmental sources. Keep providers side-effect-free and
enforce viewing rules before returning any candidate. The shared endpoint handles query validation,
ranking, deduplication, and response limits.

For object sources, extend environmental scope hooks instead of reconstructing visibility and
matching inside the provider. A source for non-object text, such as colors, can return labelled
string values directly.

An inspector action opts into suggestions through its input descriptor:

```moo
["id" -> "take_from", "label" -> "Take from", "command" -> "get {input} from " + tostr(this),
  "input" -> ["label" -> "What are you taking out?", "placeholder" -> "Find an item inside…",
    "suggestions" -> ["provider" -> $url_utils:to_curie_str(this), "source" -> "contents"]]]
```

Meadow supplies the action's command template with each request. Its reusable `SuggestionInput` and
`useSuggestions` components support debounced queries, immediate choices on focus, pointer
selection, arrow keys, Enter/Tab selection, and stale-response rejection. Selection fills the field;
submission remains a separate action through the existing command connection. Editing a selected
label clears its bound reference. Escape dismisses choices before dismissing the inspector.

The inspector refreshes suggestions on the same connection state revisions as inspection data. There
is no polling. Typed input remains usable if suggestions fail, and command execution always rechecks
current state and authority.

## Main command input

`player:command_input_context(before_cursor, after_cursor)` parses a single-line draft with a cursor
marker and returns its exact argument range as `before` and `after` strings, the argument's `query`,
and its suggestion `source`/`context`. It does not dispatch the parsed command. If the parser's
normalization prevents an exact raw-text round trip, it returns an empty map and ordinary text entry
remains available. Meadow applies this to a serialized draft, mapping retained reference ranges back
to their display labels.
