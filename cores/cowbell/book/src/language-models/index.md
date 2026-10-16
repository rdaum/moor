# Characters and tools that use language models

Cowbell includes optional characters and tools that accept requests in ordinary language. Mr.
Welcome answers questions about the world. The Architect's Compass supplies building tools. The Data
Visor supplies programming and inspection tools. Task rooms accept work through `do <task>`.

A model requests tools; it does not receive wizard authority. Tool operations use the captured
request actor. The shared `$llm_agent` runs conversations for rooms, wearables, and observers.
Experimental RLM execution is removed.

## Configure model access

Enable the curl worker in the server configuration. Set the client's endpoint and model from a
trusted wizard session. For example:

```moo
$llm_client.api_endpoint = "https://openrouter.ai/api/v1/chat/completions";
$llm_client.model = "deepseek/deepseek-v4.1-flash";
```

Set a credential with `$llm_client:set_api_key(key)`. The key is a private property on that client.
Protect world databases and checkpoints that contain configured credentials. Do not put a real key
in source files, test fixtures, prompts, or diagnostic output.

Clients are private by default. The owner can authorize specific actors:

```moo
$llm_client:set_access(false, {#123, #456});
```

Replace the example references with the players or NPCs that may use the client. To allow all
actors, the owner can call `$llm_client:set_access(true)`. This shares provider use, not tool
authority.

Agents reference `$llm_client` by default. Set `agent.client` to a separately configured client when
needed. Each client requires its own key; descendants cannot use an ancestor's credential. An
agent's `model_override` selects a model without changing the shared client.

Coding rooms have `@model` commands for endpoint, model, key, and model-list configuration. Creating
a dedicated room client does not copy the global key. Its owner must configure and share it with the
intended requesters.

## Accounts and request limits

Each request charges the captured actor. Automatic NPC work charges the NPC's account, including
triage. A player's suggestion request charges that player. Credential ownership does not select the
account charged. Owner-approved shared clients can serve many independent accounts.

Every attempt reserves capacity before network I/O. The reservation includes request data size and
the output-token limit. It is a conservative estimate, not a tokenizer measurement. Concurrent
requests cannot spend already reserved capacity.

When the provider returns valid usage, settlement replaces the reservation with reported tokens.
Missing or malformed usage keeps the reserved charge and marks it as unreported. Transport failures
also retain an unreported charge. A cancelled or abandoned request expires after its deadline; the
next account query or request reconciles it. Late replies cannot charge an already settled attempt
again.

Wizard commands accept players and NPCs:

- `@llm-budget <actor>` shows used, available, and reserved capacity, plus recent usage sources.
- `@llm-set-budget <actor> to <tokens>` changes the token budget after confirmation.
- `@llm-reset-usage <actor>` resets settled usage after confirmation. Active requests must finish
  before reset.

A budget is a local token control, not a provider currency limit. The provider can report usage
above the estimate; settlement records it and subsequent requests stop when capacity is exhausted.

The provider boundary applies these limits:

| Limit                            | Value                   |
| -------------------------------- | ----------------------- |
| Request data and response body   | 1 MiB each              |
| Messages in one request          | 256                     |
| Tools advertised in one request  | 128                     |
| Output-token limit               | 1–16,384; default 2,048 |
| Concurrent requests per actor    | 8                       |
| Attempts per request             | 3                       |
| Total request deadline           | 60 seconds              |
| HTTP attempt timeout             | At most 30 seconds      |
| Retained usage entries per actor | 100                     |

Only HTTP 408, 429, 500, 502, 503, and 504 can trigger a retry. Retries preserve the captured
request. Authentication failures, invalid JSON, and transport errors stop the request. A key,
endpoint, or access change prevents another attempt. Streaming is unsupported and is rejected before
dispatch. Provider error bodies are not included in user errors.

## Documentation, help, and command suggestions

The shared `$player.suggestions_llm_client` setting supplies the client for these suggestions.
Documentation misses can start suggestions through `@doc`. Help and command misses offer an explicit
`assist` action. Each player can run two suggestions across all connections and all three frontends.
A third request explains the limit without starting another model call.

Each suggestion captures its actor and connection and has a 30-second deadline. Results return only
to that connection. Detachment, changed authority or context, and client replacement invalidate
pending work. `stop` cancels the player's active suggestions. Suggestion jobs use anonymous objects
and release their task and context references when they close.

## Wearable interactions

Wearables share `$llm_tool_progress` flyweights for HUD and speech output. Each value captures at
most 8 KiB of literal labels, references, ranges, and argument summaries. It retains no live objects
or complete argument maps. Progress rendering performs no world lookup. Device authors can override
`_format_hud_message` and `_format_tts_message`; the inherited implementations use `tool_name` as
the device label.

Wear the device, then enter `use <device>`. The device accepts one interaction at a time, including
its initial input prompt. The Data Visor requires a programmer account and must remain worn. The
Architect's Compass requires builder privileges and also permits carried use.

Each interaction captures the actor, device owner, agent, and invoking connection. Questions and
private output stay on that connection. Losing required possession or changing the device owner ends
the interaction. A detached or reassigned connection also ends it.

`stop <device>` cancels pending input and model work. `reset <device>` stops the interaction and
clears the conversation. `reconfigure()` replaces idle configuration; stop active work first.
Sequential uses retain the device's conversation, but each use gets a new anonymous interaction.

The `interaction_timeout` property defaults to 300 seconds and accepts 1 through 3,600 seconds. This
deadline includes configuration, input, model work, continuations, and output. Reaching the
iteration limit offers at most three continuations within that same deadline.

Visor mutation tools check their previews again after confirmation. **Yes to All** applies only to
that operation type and target object during the current interaction. It does not authorize raw
evaluation. A changed target requires another decision. Cancelled confirmations and alternatives
produce explicit tool outcomes. An interrupted effect can require recovery or an explicit reset
before another request.

Both wearables use `evaluate_rule` to test a readable stored rule and `test_rule` to test an
expression. Their bindings use JSON objects, such as `{"Actor": "#65"}`. Explicit object references
are strings; ordinary text remains text. Stored-rule bindings are optional and default to an empty
object. Inputs are limited to 64 names and 16 KiB. A rule can still require specific inputs.
Evaluation uses the requester's permissions, including access to facts queried by the condition.

Wearable frontends supply `_setup_agent`, `_check_user_eligible`, and progress-formatting hooks.
Eligibility hooks run at admission. Native ownership and possession checks continue throughout the
interaction. `$llm_wearable_session` owns interaction input, scoped approval, common tools, and
cleanup. It specializes `$llm_job`, so rooms and wearables use the same model-child execution and
cancellation methods. Closing the interaction removes its tool descriptors and callback references.
Anonymous state is then collectible; the persistent wearable remains until explicitly recycled.

### Track project work

Both devices keep tasks, findings, and todos separately from the conversation. Wear an owned device
to use its project commands:

```text
plan_investigation visor with Review the lobby's command verbs
get_investigation_status visor
plan_building compass with Build a reading room
get_building_progress compass
```

During an interaction, the model uses one shared project catalog. `create_task` selects a task;
`record_finding` and `get_findings` store and query its discoveries. `task_status` reads its
progress, and `complete_task` records its result. `todo_write` and `get_todos` manage a shorter
checklist. Task records describe work; they do not execute parallel jobs.

`reset <device>` preserves project records. `clear_project <device>` stops the interaction and
clears tasks, findings, and todos. Reconfiguring the device creates a fresh project. Completed task
results remain until removal or clearing.

Each project holds at most 64 tasks and 128 todos. Todo data is limited to 64 KiB. Each task holds
at most 64 findings and 32 KiB of finding data. Task descriptions and todo text have a 4 KiB limit;
results have an 8 KiB limit. Exceeding a limit produces an error instead of growing retained state.

### Specialize a wearable

`role_prompt` contains the device's persona. `_system_prompt` composes that persona with shared
`$llm_prompts` sections. Override the hook to choose different guidance for a descendant. Prompt
composition accepts at most 32 KiB of literal text and invokes no world-content callbacks.

`$llm_project_tools` binds an anonymous provider to each interaction's captured agent. Its read
operations declare inspection effects; writes declare mutation effects. Closing the interaction
releases the provider. A retained tool handle cannot continue using that closed interaction.

## Task rooms

`do`, `fix`, and `resume` share the same admission limits. `halt` cancels your queued and running
jobs. The room owner or a wizard can use `reset` to cancel all jobs and clear history.

Each request has an anonymous job with its own agent, requester, connection, and model task.
`$llm_room_runtime` admits jobs before dispatch and monitors their workers. A lost worker produces a
cancelled outcome. Restarting the monitor does not replay work. The deadline includes tool setup.

Room owners can configure these properties within the stated bounds:

| Property                    | Default | Allowed range |
| --------------------------- | ------- | ------------- |
| `max_workers`               | 1       | 1–8           |
| `max_jobs`                  | 32      | 1–32          |
| `max_pending_per_player`    | 3       | 1–8           |
| `max_query_length`          | 2,000   | 1–16,384      |
| `task_timeout_seconds`      | 180     | 1–3,600       |
| `max_history_context_items` | 4       | 0–4           |

Both running and queued jobs count toward admission limits. Each submitted context can contain at
most 256 messages and 1 MiB of data. History retains at most 20 outcomes, with context for the most
recent four by default. An oversized saved context is discarded. Stored result text is limited to
16,384 characters.

`compact` summarizes your most recent saved conversation. Active jobs manage their own compaction.
`fix` and `resume` require retained context and reject requests with unresolved tool effects.
Inspect the actual changes before submitting explicit recovery instructions.

## Write an agent or tool

All LLM source lives in `src/llm/`. Extend `$llm_agent` for conversations and `$llm_coding_room` for
a coding workspace. `$llm_room_observer` supplies automatic room observation. `$llm_prompt_observer`
supplies an explicit one-prompt response through `$llm_runner`. These frontends use the same
conversation runtime and tool contracts.

`$llm_presenter` supplies code, report, and table tools bound to one actor and connection. Each room
job creates an anonymous presenter and closes it when the job releases its state. Progress and
results go only to the captured connection. They do not broadcast to the room or switch to another
connection after a disconnect. Presentation values are limited to 64 KiB before rendering.

Source inspection and presentation share `$llm_verb_source` snapshots. These immutable flyweights
capture native metadata and source under requester permissions, including inherited verb aliases.
Full source and selected ranges have a 64 KiB snapshot limit. Request a smaller range for a larger
verb. Range listings keep their original line numbers; the number gutter is separate from source
text. Metadata-only inspection does not load source and still enforces native metadata permissions.

Object and player inspection use `$llm_tool_provider:description_text(target, actor)` for model
context. It preserves literal strings and renders structured descriptions through the document
pipeline. The actor is the viewer; the target supplies the `this` and direct-object bindings.
Callbacks run under actor authority. Input descriptions and rendered text each have a 16 KiB limit.

The shared `$llm_code_search` provider supplies the `grep` tool to LLM inspection catalogs. It
matches literal text without case sensitivity in locally defined verbs. An explicit object selects
one target. Otherwise, ordinary actors search themselves, their contents, and their room and its
contents. Wizards search global scope. Missing or explicitly unreadable targets fail.

Results contain matches, omission counts, oversized-line locations, and `next_cursor`. Continue with
the same pattern and scope until the cursor is empty. Report omissions from every page. An empty
cursor means traversal ended; unreadable or oversized source remains unsearched. Each match
identifies the object, numeric verb index, declared names, original line, and a short snippet.
Cursors refer to the current world. Restart after objects or verb definitions change.

Each page permits at most 50 matches, 64 objects, 128 verbs, 512 lines, and 256 KiB of scanned
source. The default match limit is 20. Patterns have a 128-character limit and result values have a
64 KiB limit. Oversized lines are reported and skipped. Native object, verb, and source enumeration
still materializes complete lists before these limits apply; transient allocation is not bounded by
the page budget.

Agents compose an anonymous `$llm_conversation` object. Use `agent:get_context()` to read a CoW
snapshot and `agent:set_context(messages)` to install a validated transcript while idle. Direct
transcript properties are removed. Each new agent, including descendants of configured agents,
starts with independent conversation state.

A transcript holds at most 256 messages and 1 MiB of JSON values. It cannot retain live objects or
callbacks. Tool calls and their placeholder results enter the transcript together, before effects.
Tool results replace their matching journal positions. Invalid replacements leave stored messages
unchanged. A failed reset also retains unresolved tool outcomes.

Compaction starts when retained messages reach 192 entries or 512 KiB, or when reported usage
reaches the configured token threshold. It preserves complete tool exchanges. A revision check
rejects stale summaries, including after messages change and then return to their original text.
Ownership changes also invalidate pending summaries.

Use anonymous objects for temporary mutable agents and jobs. Ordinary MOO objects require explicit
recycling. Use immutable flyweights for tool descriptors and results.

Tool results contain JSON data, with a 64 KiB limit on both the complete outcome and its encoded
content. Normalization validates a result before the agent retains it. Invalid inspection results
become failures. Invalid mutation results remain unknown until explicit recovery, because a handler
can commit effects before returning. Error details retain at most 2048 characters and codes retain
at most 256 characters. Tool arguments are not written through a separate error-log API.

The prompt observer creates anonymous agents and runners under its owner's authority.
Reconfiguration releases idle collaborators and refuses to replace an active response. A descendant
gets separate collaborators. A runner checks access to its attached agent before every request,
including after direct property changes.

Observers compose an anonymous `$llm_memory` object backed by an anonymous relation. Its tools
remember, recall, and forget exact facts. Duplicate facts keep their original age. Each memory holds
at most 256 facts and 64 KiB of tuple values. Subjects have at most 128 characters; fact text has at
most 1,024. A full memory rejects additions.

Compaction requires source indices for every retained claim and coverage of every original fact. It
preserves the oldest supporting timestamp. Invalid proposals and stale snapshots leave all facts
unchanged. Every access checks the bound object's current owner. A configured observer's descendants
start with independent memory.

A tool descriptor contains a name, description, input schema, target, handler, and effect class.
Compose catalogs with `$llm_tool_catalog:compose(...)`. Conflicting names fail registration. The
shared schema validator checks required fields, nested objects and arrays, exact field names and
enums, and size and numeric bounds. Unsupported constraints fail registration.

The visor uses shared inspection, documentation, and behavior descriptors. Property and object
inspection use `get_property`, `list_properties`, and `inspect_object`. Compass building tools use
the same descriptors in either frontend. Select descriptors from a provider's `get_tools()` catalog
instead of copying its schema into a frontend.

Tool handlers must still authorize the actual operation. Schemas validate data; they do not grant
access. Use the captured actor rather than ambient `player` or billing metadata. An exception after
a mutation starts may mean that effects committed. Such an outcome requires inspection before
resumption.

`$llm_client:chat(...)` returns provider data. `$llm_response:mk(data)` validates and exposes the
assistant response. `simple_query()` returns exact visible text. Consumers that expect JSON must
parse and validate it; the client does not remove fences or extract JSON fragments.

## Test without a provider bill

From the repository root, run:

```sh
make -C cores/cowbell test-llm-provider MOORC_TYPE=direct
```

This uses a local HTTP fixture and the real curl worker. It checks tool continuation, compaction,
accounting, retries, error redaction, and timeout. For a paid smoke test, set `OPENROUTER_API_KEY`
in the shell and run `test-llm-live` instead. The key stays in the test proxy process. The live
target permits at most 12 attempts by default and uses `deepseek/deepseek-v4.1-flash`.

For a character that needs no model configuration, follow
[Henri's tutorial](../actors/henri-and-the-cupboard.md).
