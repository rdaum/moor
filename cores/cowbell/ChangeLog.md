# Cowbell changelog

This changelog records features, significant bug fixes, compatibility changes, and upgrade notes for
the Cowbell core. See the [mooR changelog](../../ChangeLog.md) for server changes and earlier
Cowbell entries.

## Unreleased

### Added

- Wizards can use `@audit-names` to inspect all stored object names and aliases for invisible names,
  control characters, directional formatting, and malformed values. Reports include object IDs and
  escaped values. The audit leaves names unchanged; `--players` limits it to accounts.

- Shared string checks reject blank or hidden-control names and aliases before object or player
  edits and account creation. Description setters reject hidden controls while retaining multiline
  Unicode text. Builder edits and LLM description tools use these checks. Existing stored values
  remain unchanged; raw programmer assignments can bypass core setters.

- ANSI colour and style codes in text and descriptions now reach telnet and web clients. Other
  terminal controls, such as cursor movement, screen clearing, and window titles, still appear as
  visible `\x1b` text. A line that turns a style on is reset before it ends. Names and aliases still
  reject all control characters.

- Every player can use `@describe me as TEXT` to describe themselves. Builders keep the same command
  for objects and exits.

- Room listings with more than eight people name the first five and count the rest, for example
  "Ada, Bert, Cy, Di, Ed and 41 others are deeply asleep." `$look.actor_listing_limit` and
  `$look.actor_listing_shown` set the limits.

- `@llm-config setup` prompts for a provider URL, model, and API key, then configures shared access
  for Mr. Welcome, wearables, and assistance. `scripts/dev.sh` performs this setup with OpenRouter
  and `deepseek/deepseek-v4.1-flash` when `OPENROUTER_KEY` or `OPENROUTER_API_KEY` is set, and
  starts the curl worker.

- Objects control noun phrases, grammar, and matching names through inherited verbs. `@grammar` lets
  builders inspect and edit articles, plural names, and room-listing preferences. Rooms, inventory,
  clothing, and stock messages share those choices. Articles work in commands and completion without
  fuzzy matching. See the [naming lesson](book/src/building/names-and-grammar.md).

- Builders can send events between objects, use named messages and message bags in reaction steps,
  and direct owned NPCs with `@act`. `@schedule` can send recurring events with optional timing
  variation. `@exit-provider` lists, approves, refreshes, and removes custom room exits.
- `@set-message` creates named messages on owned objects. Ordinary object messages have default
  actor and object roles; custom `message_bindings` code is only needed for additional participants.
  The Pet Rock, Henri, and portal tutorials use these commands for their setup and connections.

- Builders can use `@set-pronouns ACTOR to PRESET|custom` to set pronouns for actors they own.
  Custom entry previews the words and checks editing permission again before saving. Players and
  NPCs share the validated setter; `@pronouns` remains the personal-settings command.

- `#` accepts expressions in property assignments, such as
  `#Henri.pronouns = $pronouns:lookup("he/him")`. The
  [object shortcuts tutorial](book/src/programming/object-shortcuts.md) and `help #` explain lookup,
  properties, scopes, and verb calls.

- Add an executable [Henri tutorial](book/src/actors/henri-and-the-cupboard.md) after Pet Rock. An
  ordinary programmer builds a cat, food cupboard, key, and couch, then connects their reactions.
- Builders can use custom event names in `@reaction OBJECT NAME after EVENT`. The names belong to
  each receiving object and need no global registration.

- Room authors can register custom exits such as portals and lifts. Room descriptions, `exits`,
  links, and route hints use the same options. Unavailable travel explains why; destinations can be
  unknown until use. Saved links recheck the actor, source, registration, and destination.
- New worlds include a travel example area connected to the First Room. Visitors can try doors,
  keys, parallel passages, one-way travel, and passages and portals between areas.
- Builders can use `@set-passage DIRECTION access CONDITION` for readable door conditions and edit
  `open_msg`, `close_msg`, `lock_msg`, and `unlock_msg`. The room and portal guides contain runnable
  tutorials.
- Reactions can use `open_exit`, `close_exit`, `lock_exit`, and `unlock_exit`. These actions use the
  same presence, key, and access checks as typed door commands.

- Rooms can react to completed passage door operations with `passage_open`, `passage_close`,
  `passage_lock`, and `passage_unlock`. Both sides receive the event, with their own exit name.
- Passages can connect rooms in different areas. Both ends discover the connection, and walking
  routes can continue through the destination area's rooms. `@dig --allow-parallel` creates a
  separate connection between already connected rooms.
- `help pronouns` and `help messages` explain pronoun settings and message authoring, with command
  examples and sentences for each reader.
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
- Add a [builder's guide](book/src/behaviour/reactions.md) with command examples for reactions,
  membership rules, and a timed puzzle. These workflows require builder privileges, with no MOO
  programming.
- Add MOO parsing for an explicit Djot authoring profile, with source-location errors for
  unsupported syntax. Speech and help now preserve authored formatting in HTML, Djot, and plain
  text.
- Add `@describe --djot` and `--literal` to select description syntax. Description, note, and letter
  edits validate their selected syntax before saving.
- Add document values with validation, metadata encoding, and HTML, Djot, and plain text renderers
  for core code.
- Add `list:find_index(predicate)` to distinguish a matching value of zero from no match.

### Changed

- LLM agents retain failure counters only for registered tools and cap them at the retry limit.
  Removing a tool releases its counter. Invented tool names no longer grow retained failure state
  across interactions; each unknown call still returns an explicit tool error.

- The naming API replaces `is_plural()`, `is_countable()`, `is_proper_noun()` and their item flags
  with `noun_grammar()`, `noun_phrase()`, `listing_form()`, and `match_names()`. Custom prototypes
  must update old overrides. Indefinite mass and plural nouns now use “some.”
- Letter subjects use `.subject`; their physical name is `letter`. Mailbox lists and readers retain
  the subject. Default door messages quote their side labels, such as `the door "north"`.

- Henri and his cupboard, kibble, key, and couch are now tutorial content. New core imports no
  longer contain those objects or their system aliases. The travel example rooms remain included.

- Building grant lookup checks the incoming principal before reading another actor's grants.
  Ordinary callers can no longer obtain someone else's stored capability through the area helper.

- Custom transport providers must register with their room and implement `travel_options` and
  `use_exit`. `use_exit` returns a destination or false; the room performs the checked move. The old
  `transport_connections` scan no longer contributes routes. See the portal tutorial.
- Room-defined actions now appear beside actions supplied by room contents.
- Passage traversal evaluates access again after departure callbacks, so a revoked rule fact can
  stop the move even when the passage and inventory are unchanged.
- A passage no longer reports arrival when a movement callback sends the traveler somewhere else.
- Building tools use the same room-name and direction checks as builder commands. Passage previews
  check both endpoints and exit-name collisions before reporting success.

- Ownership transfers include inherited properties with the `c` flag. A new room owner can save
  descriptions without a wizard repairing those property permissions.
- Passage descriptions use explicit literal, inline Djot, or template syntax. Use
  `@describe --literal`, `--djot`, or `--template` before the exit name. Braces in literal text stay
  literal. Edits without a format flag preserve the selected syntax. New descriptions use sentence
  mode and retain their spelling and punctuation; fragment mode remains an explicit option.
- Ambient descriptions link the complete phrase without requiring a direction word. The narrative
  also lists exit names. Structured descriptions retain formatting and substitutions in room prose.
- `@edit-description` accepts passage directions and preserves their source syntax. Saving rechecks
  editing authority and rejects conflicting descriptions or replacement connections. `@passage`
  previews room prose and movement messages. Arrival and departure wording use their dedicated
  fields, without reusing the ambient description.
- Door commands share passage access rules with movement. Key-independent rules work without an
  inventory item. An explicitly named key must be carried. Ambiguous `open door` commands ask for an
  exit name. Successful operations refresh room panels on both sides.
- Passage creation returns the installed value with its connection identity. Keep that value for
  later edits. Updates preserve identity; replacement and reinstallation assign a new identity. A
  room pair alone cannot select among parallel passages.
- Existing worlds need their passage stores rebuilt after this update. As a wizard, call
  `area:rebuild_passages()` for each area that stores passages. It checks the records before
  conversion and returns the old relation as a backup. Old saved routes and exit links then expire.
- Moving a room between areas preserves its connections. Area admission requires `add_room`
  authority, including when the room moves through `moveto` directly. Passage creation checks the
  rights at both endpoints; area ownership alone does not grant rights over a foreign room.
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

### Removed

- Remove the unused LLM event-queue prototype. Observer runtimes own their bounded queues.
- Remove experimental RLM execution, workspace eval, and system registration from the LLM subsystem.
  Both task-room frontends now use the shared LLM agent and guarded change tools.
- Remove the duplicate LLM agentic conversation, loop, and tool prototypes. Shipped callers use
  `$llm_agent` and `$llm_agent_tool` directly.

### Fixed

- Room entry and exit no longer render live room panels or arrival descriptions for disconnected
  players. This avoids repeated room captures and task timeouts in rooms with sleeping occupants.

- The shared LLM/MCP `grant_capability` tool converts JSON permission names into symbols correctly.
  LLM documentation lookup recognizes the bare `sub` and `sub_utils` aliases.

- Compass and MCP route tools resolve room names across connected areas and distinguish missing
  rooms from rooms with no open route. Explicit starting objects are respected.
- LLM property inspection handles symbol names and includes inherited properties with their current
  values and defining objects. Unreadable values remain hidden.

- LLM suggestions no longer lose replies or fallback messages when worker cleanup races with output
  delivery.

- MCP calls to shared LLM tools validate their published arguments and report failed operations as
  errors. Compiler failures retain their diagnostics instead of exposing an internal flyweight as a
  successful result. MCP also preserves MOO error codes and messages for rejected calls.

- LLM observers use their configured client's model when no observer preference is set. Ordinary
  room conversations no longer fail with an invalid model name under the default configuration.

- LLM wearable progress shares bounded HUD and speech rendering. Inspection displays the supplied
  object reference, and source-range progress accepts either optional boundary independently. The
  visor and compass use one immutable progress value with separate rendering methods and frontend
  hooks. Progress displays supplied references and argument summaries without querying object code.

- LLM documentation, help, and command suggestions explain the two-job capacity limit on the
  invoking connection. A third request no longer produces a command traceback or starts model work.
  Existing suggestions remain active until completion or `stop`.

- LLM tool outcomes accept only JSON data and have 64 KiB limits for retained and encoded values.
  Invalid results cannot retain live objects or callbacks in the agent. Failed inspection results
  let later tools continue. Invalid mutation results retain an explicit recovery barrier, because
  the operation can have committed before returning its result. Tool error details are bounded.

- LLM object and player inspection render structured descriptions as readable text. Literal strings
  retain their punctuation. Description callbacks and rendering use the requesting actor's
  authority, and descriptions have a 16 KiB limit before and after rendering.

- LLM inspection tools share paginated code search across the visor, compass, and task rooms. The
  `grep` tool returns structured matches and continuation cursors, with per-page work and output
  limits. Missing or explicitly unreadable objects return errors. Results report unreadable source
  and oversized lines instead of silently presenting an incomplete search. Search remains literal
  and case-insensitive, with local default scope for ordinary actors and global scope for wizards.

- LLM source inspection, visor listings, and room previews share immutable source snapshots.
  Inherited aliases use native lookup without target metadata callbacks. Source ranges retain their
  original line numbers and optional number gutters. Each snapshot has a 64 KiB limit; smaller
  ranges from larger verbs remain available. Metadata inspection does not load source. Remove the
  visor's duplicate source-payload and definer helpers.

- LLM wearables share stored-rule evaluation through the behavior-tool provider. Both rule tools
  accept JSON bindings; `evaluate_rule` no longer accepts a MOO literal string. Stored-rule bindings
  are optional and default to empty. Named rule properties use native reads under requester
  permissions. Failed reads return inspection errors and do not stop later tools as uncertain
  mutations.

- LLM wearables share one project-tool catalog. The compass now uses `create_task`,
  `record_finding`, and `task_status`, like the visor. Both devices can query findings and complete
  tasks through tools. Project tools belong to the active interaction and reject calls after it
  closes. Failed project reads no longer require recovery for an uncertain mutation.
- LLM wearable prompts use shared guidance and separate device personas. Remove duplicated MOO
  examples, incorrect language claims, and instructions for nonexistent parallel task execution.
  Device authors can specialize `role_prompt` and `_system_prompt`.

- LLM observers capture room input before running client or content callbacks. Those callbacks run
  under the observer job deadline, so failures cannot interrupt room delivery or item movement. Mr.
  Welcome reacts to native item transfers; direct callback calls create no observations. Owners can
  inspect bounded intake error counts and codes through the observer runtime.

- The managed Cowbell package includes all shipped LLM collaborators. Source reviews can resolve
  their constant bindings and include their definitions.
- LLM agent project state now uses private anonymous collaborators. Todo lists, tasks, and findings
  have count and byte limits. Access follows the agent's current owner. Removing a task releases its
  descendants and findings, and retained handles cannot modify removed work. The visor and compass
  share task selection and status. Their planning commands accept a device and description:
  `plan_investigation visor with DESCRIPTION` and `plan_building compass with DESCRIPTION`.
  `clear_project DEVICE` stops the interaction and clears its tasks, findings, and todos.
- LLM command suggestions use the shared bounded job lifecycle and requester authority. Command,
  help, and documentation suggestions share admission limits, deadlines, cancellation, and captured
  output connections. Suggestions preserve object aliases and readable state, use current custom
  exits and room hints, and discard replies when context changes. Candidate links require matching
  command syntax or an exact room command. Native metadata collection excludes private definitions.
- LLM assist opt-in requests are private and bound to their originating connection. Bare `assist`
  selects that connection's newest request. Request links use `assist --request TOKEN` and work
  once. Requests expire after two minutes and become invalid after connection, room, role, owner, or
  client changes. Each player retains at most eight requests and 32 KiB of input and request
  metadata.
- LLM help suggestions use the shared anonymous job lifecycle, including cancellation, deadlines,
  and the invoking connection. Help providers and feature probes receive the requesting player's
  authority. Replies link only to topics in the captured catalog; changed catalogs discard pending
  suggestions. Help tools and suggestions share bounded topic collection.
- LLM documentation suggestions keep their requester and connection in bounded anonymous jobs. Each
  player can run two suggestions, with a 30-second deadline for context collection, model work, and
  output. `stop` cancels pending suggestions. Detached or reassigned connections, revoked programmer
  status, and client changes cancel pending work. Context collection uses native read permissions,
  excludes private ancestor and system-reference metadata, and rechecks context before publishing a
  reply.
- LLM guide tools share inspection, help, and social providers. Mr. Welcome uses the common `object`
  and `to_room` argument names; `inspect_object` replaces the duplicate `find_object` tool. Social
  tools use the NPC's authority and captured room. Persona callbacks require an active LLM job and
  reject output after movement, shutdown, or permission changes. Failed lookups and denied reads
  remain tool failures.
- LLM observer room events and direct questions now share one bounded queue and conversation. Direct
  questions use the NPC's tool authority and billing identity. Usage stays on the invoking
  connection. Deadlines include setup and output; reset and shutdown cancel owned tasks. Interrupted
  mutations require inspection and an explicit reset before further execution.
- LLM observers use their dedicated client for readiness checks and triage. Structured events retain
  their speaker and action, and conversations directed at another NPC do not trigger a reply. Triage
  accepts an explicit visible decision and discards results when the conversation changes.
- LLM observer speech preserves ordinary sentences previously suppressed by phrase filters. Leaked
  tool markup cannot produce speech or actions. Replies stay in the captured room and are suppressed
  after the observer moves or is disabled. Mr. Welcome starts without a dumped agent.

- LLM behavior tools on wearables confirm rule, message, state, reaction, and fact changes on the
  initiating connection. Changed previews require a new decision. Reaction authoring keeps the
  selected host and copy destination through confirmation; renaming objects cannot redirect a write.
- LLM tools and builder commands use the caller's native read permissions for message, rule, and
  reaction lists. Private properties stay private. Property-name lookup uses native metadata without
  calling object code.

- LLM tools and builder commands check property permissions when editing messages. Owning an object
  that refers to a foreign message bag does not grant access to that bag. Neither entry point
  manufactures write grants. Changes during template compilation cannot retain revoked write
  permissions.

- LLM room tool output uses each job's captured requester and connection. Source code, tables, and
  reports no longer broadcast to the room. Closed jobs cannot publish through retained presentation
  tools, and detached or reassigned connections receive no delayed output.
- LLM task-room output reaches each requester's session when the ambient player differs. The shared
  utility output method accepts the receiver's authenticated principal without selecting another
  session.
- LLM frontends now share `src/llm/` and direct system references: `$llm_coding_room`, `$llm_runner`
  and `$llm_prompt_observer`. The separate `$agentic` namespace is removed. Room configuration uses
  `max_iterations`, `system_prompt`, and `task_timeout_seconds`.
- LLM requests use a shared provider boundary with bounded payloads, deadlines, and transient HTTP
  retries. Options are inert data; streaming and invalid options fail before dispatch. Provider
  error bodies do not enter user errors. Retries retain the original request and stop after access
  or credential changes.
- LLM clients require a local key and explicit sharing for other actors. Descendants cannot use
  inherited credentials. Room configuration preserves the owner's authority and no longer exposes
  key fragments.
- Every LLM request purpose reserves and settles the captured actor's account, including NPC work,
  compaction, triage, and suggestions. Missing usage and abandoned requests retain explicit
  unreported charges. Usage logs retain 100 entries. Budget commands support NPCs and show reserved
  capacity.
- Model preferences belong to individual LLM agents. Configuring a wearable or observer no longer
  changes the shared client's model. Text-only queries preserve provider text without extracting
  JSON fragments.
- LLM room catalog hooks receive requester authority. Internal agent methods reject forged actors,
  including calls from methods added to the same object.
- LLM prompt observer reconfiguration releases idle anonymous collaborators and rejects replacement
  during an active response. Runners preserve caller authority and reject access to foreign agents.
- LLM programming tools can update verb metadata and argument specifications under the requester's
  permissions. The visor uses shared inspection and authoring catalogs, including `get_property`,
  `list_properties`, and `inspect_object`. Wearable configuration checks the incoming principal and
  creates each anonymous agent under its intended owner.
- The LLM visor uses the shared programming catalog: `program_verb`, `set_verb_info`, and
  `set_property_info`. Property values are JSON values. Programming tools bound to a wearable
  confirm writes on its initiating connection. Closing the interaction disables retained tool
  providers and restores the device's catalog. Compiler errors preserve the previous verb code.
- LLM building tools on the compass and visor confirm changes on the initiating connection. Changed
  object or passage previews require another decision. Revoking builder, programmer, or wizard
  status ends the interaction. Building permission denials return failures, and failed object
  placement attempts cleanup. Incomplete cleanup identifies the surviving object.
- The LLM visor eval tool remains opt-in and requires a separate confirmation for each call. Code
  runs with the captured player's identity and permissions. Runtime errors report an unknown outcome
  because earlier effects may have committed. The default programming catalog does not include eval.
- LLM agent tools validate required fields, nested arguments, and limits before callbacks or
  mutations. Invalid arguments produce a definite failure. Change tools use the same validator, with
  exact field names and enum strings.
- LLM agents keep transcripts in independent anonymous conversation objects. Guarded context methods
  replace direct property writes. Transcripts have count and size limits, reject incomplete tool
  exchanges, and compact before capacity is exhausted. Stale summaries cannot replace newer
  messages.
- LLM agents obey iteration and token limits, stop later tools after cancellation, and report
  invalid provider replies. Context compaction retains complete tool exchanges and preserves
  concurrent messages. Tool failures have explicit outcomes; unresolved effects prevent blind
  retries.
- LLM agent rooms enforce queue limits before dispatch and support independent concurrent jobs.
  `halt` cancels the requester's work; `reset` requires room owner or wizard authority. Worker loss
  cancels orphaned model calls without replay. History and saved contexts have bounded retention.
- LLM room revisions select the requester's newest history entry. Resumed tasks carry fresh
  authorization. Requests use anonymous jobs that capture the requester and connection, track the
  model child, and release callback references.
- LLM room history and queued requests are private. Status and context views filter by requester or
  room administrator. Public room descriptions do not include transcript excerpts.
- LLM observer memory uses a shared anonymous component with bounded fact storage and exact recall
  and removal. Duplicate facts retain their age. Descendants have independent memory. Compaction
  rejects proposals that omit source facts, and ownership changes revoke the previous owner's
  access.
- LLM observer fact compaction validates proposed facts before replacement, preserves source ages,
  and retains concurrent changes. Fact administration requires owner or wizard authority.
- LLM tool inspection reports verb argument specifications correctly. LLM wearables use one empty
  task ID, handle missing tasks, and parse rule bindings as data. Task initialization and completion
  require owner or wizard authority.

- `@dump-database` waits for the checkpoint export to finish and reports its result, instead of
  announcing success when the request is only queued.

- Room narratives group generated exit, contents, and occupant lines without blank lines between
  each sentence. Authored paragraphs retain their spacing, including welcome messages and room
  prose.

- `go` accepts local exit names and adjacent destination names, including
  `go to the travel gallery`. Missing and ambiguous routes give useful messages. Door locks and
  travel permissions still apply. `look` uses the same exit lookup for passages and custom exits.
- The travel gallery has an inspectable lift button that responds to `push` and `press`. Its brass
  door also accepts `brass door` as a name.

- Clicked exit actions use the authenticated player when invoked directly by the web host.
  Previously, they failed while checking the absent MOO caller's permissions.
- Room narratives describe travel with “You can go …”, using directions, authored phrases, or
  destination names. Exits already described in the room prose are not repeated. Closed doors and
  unavailable routes retain their state and explanations. The example lift links its “ride lift”
  instruction to the current travel action.

- Mailbox retrieval cannot take an object outside the mailbox by its identifier. It accepts only
  letters currently in that mailbox.
- Object lookup uses the command's visible scope and handles aliases and ambiguity consistently.
  Scoped lookups require membership even for identifiers. Repeated scope entries do not create
  duplicate candidates.
- Authoring commands preserve quoted punctuation in object names and retain quotes and spacing in
  descriptions and property values. `#"Dr. Moss".name` and quoted names such as `"to"` work. Command
  parsing and core word helpers now share whitespace, quote, and escape handling.

- Quoted object names work with `#` and `@verb`. Message commands and `@property` preserve the text
  after a quoted object reference, including quotes within message text.

- `#` preserves numbered object references and rejects ambiguous names instead of choosing an
  arbitrary match. Invalid expressions, denied operations, and failed lookups report command errors.
  Property assignments accept `=` with or without surrounding spaces.

- `@move` works for ordinary objects and NPCs instead of failing on a missing player property.
- Item entry and exit reactions on characters use the surrounding room, so their messages are
  visible there.
- Builders can create and move furniture they own. Sitting and standing on that furniture run its
  reactions; rejected moves leave sitters in place.

- Clicking an exit in Meadow checks the room and connection shown by that link. Old links cannot use
  a different room's exit or a replacement connection. Door actions check the current state.
- Closed and locked passages remain visible in room descriptions, `exits`, and room panels. Door
  links offer an opening command. Both endpoints show connections stored in another area. Connection
  changes refresh occupied room panels and prevent older exit captures from replacing the new state.
- Passage configuration cannot modify the shared prototype through an ordinary caller. Configuring a
  persistent passage requires its owner or a wizard; flyweight configuration remains a value
  operation.
- Description editors work for ordinary builders and save with their current permissions. A save
  from another connection cannot use an existing editor session.
- Passage edits preserve custom delegates, extension slots, contents, and door messages. Editing one
  side cannot redirect the connection or change the other side without its required authority.
- One-way passages reject reverse travel and reverse routes. Removed passages cannot move actors,
  and saved open passage values cannot bypass current door state. Walking reports arrival only after
  movement succeeds.
- Room transfers and recycling keep exit discovery consistent at both endpoints. A rejected move
  preserves engagement, and a completed departure sends one notification to room contents.
- `@describe` and movement actions recognize passage flyweights. Named one-way exits do not require
  an inferred return direction. Remote calls cannot operate another room's doors.
- Reaction commands keep the line breaks in previews, status, and help output.
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
