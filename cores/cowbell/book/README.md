# Maintaining the Cowbell Book

The reader's book starts at [Welcome to Cowbell](src/index.md). Its
[table of contents](src/SUMMARY.md) collects the existing tutorials and marks unwritten chapters as
planned. This file records coverage, gaps, and the work needed to fill them.

The book serves visitors, builders, new programmers, and world operators. Running and maintaining a
world is a full subject, including work outside the MOO. It is not a list of wizard commands.

## Build and read

From the repository root, with mdBook installed:

```sh
make -C cores/cowbell book
make -C cores/cowbell book-serve
```

The build writes `cores/cowbell/book/html/index.html`. The preview server prints its local address.
The book uses mdBook's HTML renderer and the MOO highlighting script from the main mooR book. It
does not require additional preprocessors. The initial structure was checked with mdBook 0.4.52.

Run executable tutorials with:

```sh
make -C cores/cowbell test-tutorial
```

Building HTML does not execute examples. `mdbook test` is not the MOO tutorial runner. The
`SESSION_TUTORIALS` list in [the Makefile](../Makefile) selects the six tested documents. Keep each
tutorial in one file: its runnable blocks share a database. See
[runtime testing](../RUNTIME_TESTING.md#executable-tutorials) for fence syntax and focused commands.

For a planned chapter, use an empty destination in `SUMMARY.md`. Do not create an empty page that
looks like a finished guide. The build rejects missing files in ordinary chapter links.

## What the first structure includes

Ten chapters moved from `docs/` and `manual/` into the book. Their executable examples remain in the
same documents. New introductions connect those chapters and identify missing material. They do not
replace the missing procedures with unverified instructions.

| Existing chapter                                                              | Coverage                                                                  | Example validation                                                                |
| ----------------------------------------------------------------------------- | ------------------------------------------------------------------------- | --------------------------------------------------------------------------------- |
| [Names and grammar](src/building/names-and-grammar.md)                        | Names, aliases, articles, pronouns, and room listings.                    | Executable tutorial.                                                              |
| [Rooms and passages](src/building/rooms-and-passages.md)                      | Rooms, passages, keyed doors, parallel and one-way routes.                | Main walkthrough is executable; later sharing and area examples are illustrative. |
| [Rules, reactions, and relations](src/behaviour/rules-reactions-relations.md) | Concepts and several builder examples.                                    | Not registered as an executable document.                                         |
| [Author reactions](src/behaviour/reactions.md)                                | Commands for messages, state, conditions, facts, timers, and diagnostics. | Not registered as an executable document.                                         |
| [Pronouns and messages](src/behaviour/pronouns-and-messages.md)               | Perspective, participants, pronouns, and message authoring.               | Not registered as an executable document.                                         |
| [Object shortcuts](src/programming/object-shortcuts.md)                       | Finding objects, properties, expressions, and verb calls with `#`.        | Executable tutorial.                                                              |
| [Pet rock](src/programming/pet-rock.md)                                       | First verb, messages, state, triggers, relations, and inheritance.        | Executable tutorial.                                                              |
| [Custom exits](src/programming/custom-exits.md)                               | A portal integrated with room output and travel.                          | Executable tutorial.                                                              |
| [Henri](src/actors/henri-and-the-cupboard.md)                                 | An NPC, moods, food, sitting, and recurring responses.                    | Executable tutorial.                                                              |
| [Message templates](src/reference/message-template-reference.md)              | Substitutions and naming interfaces.                                      | Reference; not an executable document.                                            |

These validation labels describe how the documents are registered. They do not claim that every
illustrative example runs, or that the surrounding features lack other tests.

## The narrative that still needs writing

The contents supply places for the chapters below. Source pointers identify material to inspect; the
presence of an implementation does not prove that its user workflow is complete. Unlinked source and
test paths are relative to `cores/cowbell/`.

### Running and maintaining a world

Seven operator chapters now follow the [operator introduction](src/running/index.md). They explain
current procedures rather than promise a complete deployment guide for every backend.

| Chapter                                                 | Coverage now                                                                                                   | Remaining work                                                                                                 |
| ------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| [Start a world](src/running/starting.md)                | Local Compose setup, initial import and baselines, login, authored object, stop/restart, storage paths.        | Run the complete Docker build and browser walkthrough in an automated disposable deployment.                   |
| [Connections and workers](src/running/configuration.md) | Component roles, configuration precedence, local ports, worker dependencies, and limits of changing `RUN_DIR`. | Public TLS deployment walkthrough and a separate multi-instance recipe.                                        |
| [Accounts and authority](src/running/accounts.md)       | Registration, roles, builder/programmer grants, login settings, and community policy.                          | Account recovery, full privilege revocation, and a tested two-account delegation tutorial.                     |
| [Backup and restore](src/running/backups.md)            | Stopped-directory archive, configuration and keys, separate restore directory, object exports.                 | Full Compose/Meadow history restore, external storage inventory, and backend-specific backup procedures.       |
| [Review updates](src/running/updates.md)                | Baseline/live/incoming versions, fetch, choices, drafts, groups, preview/apply, partial results, recovery.     | A self-contained executable tutorial that fetches controlled source, introduces a local edit, and resolves it. |
| [Upgrade](src/running/upgrades.md)                      | Rehearsal, compatibility, separate server/core updates, local work, post-upgrade checks, recovery limits.      | A recorded version-to-version upgrade and rollback rehearsal with a populated world.                           |
| [Operate and diagnose](src/running/operations.md)       | Logs, client/backend distinction, permissions, tasks versus schedules, and review recovery.                    | Worked incidents for failed workers and model requests, plus deployment-specific monitoring.                   |

Validation for this expansion:

- All four launcher contract tests pass. These inspect actual Compose configuration without building
  or starting containers.
- The local port override resolves to exactly three loopback bindings. It uses Compose's `!override`
  support to replace the original published ports.
- Existing command scenarios for change-review shortcuts, decision preservation, background
  responses, and schedule commands pass: `change_terminal.moot`, `change_manager_preservation.moot`,
  `change_manager_commands.moot`, and `schedule_commands.moot`.
- A temporary server experiment stops Cowbell, archives its runtime directory, restores it under a
  different directory, and starts the copy. Login, an authored object, a stored property, and a
  signed token survive. This is a checked experiment, not a new committed regression test.
- The full Compose deployment, Meadow history recovery, and cross-version upgrade are not claimed as
  verified by those checks.

The main implementation references remain `scripts/start-moor-cowbell.sh` at the repository root,
`src/change_manager.moo`, `src/features/`, `src/auth/`, `tests/wire.py`, and the relevant sections
of the mooR Book. A session test cannot establish that a deployment backup restores after an
upgrade.

### Visiting and authoring

| Planned chapter or revision         | Missing reader journey                                                                                                                                                                                      | Source material                                                                                                                                                                                |
| ----------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Conversation, belongings, and mail  | Meet someone, speak privately or publicly, carry and give things, use mail, leave, and return. Explain the visibility of each interaction.                                                                  | `src/features/social_features.moo`, `src/features/mail_features.moo`, `src/messaging/`, `src/help/help_topics.moo`.                                                                            |
| Meadow and terminal clients         | Follow the same visit in both clients. Explain narrative history, room/object actions, suggestions, editors, and command equivalents. Identify actual differences rather than promise identical interfaces. | [Meadow README](../../../clients/meadow/README.md), [client authoring tools](../../../book/src/web-client/authoring-tools.md), `src/player.moo`, Meadow's narrative and suggestion components. |
| Supplied objects                    | Furnish a room using containers, clothing, food, seats, and notes. Explain how to choose a prototype and discover its settings.                                                                             | `src/items/`, prototype box, existing item and command scenarios.                                                                                                                              |
| Descriptions and formatting         | Write and preview a description in both clients. Explain literal text, Djot, templates, paragraphs, and links before introducing the document API.                                                          | `src/features/builder_features.moo`, `src/document/`, `src/format/`, rooms and naming tutorials.                                                                                               |
| Building together                   | Two builders share an area, grant limited access, connect rooms across areas, inspect authority, and revoke access. Show a denied operation and how to resolve it.                                          | `src/utils/grant_utils.moo`, `src/world/`, [capabilities](../CAPABILITIES.md), illustrative sections of the rooms tutorial.                                                                    |
| Rules and reactions: reduce overlap | Keep the introduction focused on why the three systems exist. Put the command walkthrough in one home. Add a path from conditions to state, scoped relations, reuse, preview, and trace.                    | The two existing behaviour chapters and `src/rules/`, `src/relations/`. Preserve the current object-scoped relation model.                                                                     |
| Messages: connect the lessons       | Introduce a plain message before participants and substitutions. Link from the first reaction to the message guide. Separate the practical lesson from the full reference.                                  | Existing pronoun/message chapters, `src/events/`, `src/pronouns.moo`, naming tutorial.                                                                                                         |

### Programming, characters, and models

| Planned chapter                       | Missing reader journey                                                                                                                                                                                                        | Source material                                                                                                                                                         |
| ------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Editing, testing, and debugging verbs | Find a verb, edit it in Meadow and through commands, understand compilation and runtime errors, inspect a property, and test with another account.                                                                            | `src/features/prog_features.moo`, `src/utils/prog_utils.moo`, Meadow editors, [runtime testing](../RUNTIME_TESTING.md).                                                 |
| Designing a reusable prototype        | Extend a parent, decide what a builder can customize, expose messages/rules/triggers, and test a child owned by someone else. Explain verb permissions with a concrete example.                                               | Pet rock's closing sections, `src/root.moo`, `src/items/thing.moo`, [style guide](../STYLE_GUIDE.md), [capabilities](../CAPABILITIES.md).                               |
| Events, documents, and client actions | Follow one action through participants, recipient-specific messages, document creation, rendering, and delivery. Add an object or command link and check Meadow and terminal output. Explain which object controls each step. | `src/events/`, `src/document/`, `src/format/`, `src/look.moo`, `src/examination.moo`, [client output](../../../book/src/web-client/client-output-and-presentations.md). |
| Activities, timers, and schedules     | Give a character an activity and a recurring action. Inspect, cancel, and restart it. Distinguish task-local delays from persistent schedules.                                                                                | Henri, `src/actor.moo`, `src/player_activity.moo`, builder schedule commands, `tests/headless/headless_scheduler_scenarios.moo`, `tests/wire.py`.                       |
| Supplied model tools                  | Try Mr. Welcome, the Compass, and the Visor. Explain required access, confirmations, results, stopping, and failures. Check what actually works through telnet.                                                               | `src/llm/mr_welcome.moo`, `architects_compass.moo`, `data_visor.moo`, `llm_wearable.moo`, `tests/headless/headless_llm_scenarios.moo`.                                  |
| Model configuration and use           | Connect provider/worker setup to object settings. Identify credentials, billing identity, request authority, data sent to models, limits, and usage reporting.                                                                | `src/llm/llm_client.moo`, `llm_chat_opts.moo`, `llm_wearable.moo`, server worker configuration, [capabilities](../CAPABILITIES.md).                                     |
| Writing an agent and tools            | Start with one observation and one permitted operation. Explain context, tool results, cancellation, and errors. Establish which agent implementation authors should extend.                                                  | `src/llm/llm_agent.moo`, `llm_agent_tool.moo`, `src/agentic/`, `src/llm/change_tools.moo`, `agent_building_tools.moo`, agent scenarios.                                 |

### Reference and discovery

The book needs a command index grouped by task, with required roles and links to lessons. It also
needs a prototype and API reference covering objects, collections, relations, events, and utilities.
Those references should follow the concepts taught in the book. Avoid presenting every internal
method as a supported authoring interface.

Meadow suggestions and in-world help need a documented relationship to those references. The old
README linked to `docs/suggestions.md`, but that file is absent from this checkout. The book records
the missing topic instead of retaining the broken link.

## What stays outside the reader's book

- [Rule and reaction data](../manual/behavior-model.md) mixes current internals with proposed
  definition/instance records. Keep its status explicit. Do not teach those records as the current
  authoring interface.
- [Capability security](../CAPABILITIES.md), [runtime testing](../RUNTIME_TESTING.md), and the
  [style guide](../STYLE_GUIDE.md) remain contributor references. New reader chapters can introduce
  the relevant concepts and link to them.
- [The audit matrix](../AUDIT.md) remains maintenance material.
- [The earlier substitution notes](../sub_documentation.md) need comparison with the current message
  reference before reuse. They are not a second source for the book's message syntax.
- The mooR Book remains the language and server reference. Cowbell needs complete operating
  procedures that link to it, rather than a copy of its configuration tables.

## Suggested writing order

1. Complete the operator verification gaps above. Turn the `@changes` walkthrough into an executable
   lesson, add the full deployment restore check, and rehearse a versioned upgrade.
2. Complete the visitor and client tours, then the basic object and shared-building lessons. These
   supply prerequisites currently scattered through longer tutorials.
3. Reduce overlap in the behaviour chapters. Make the builder path continuous and add executable
   coverage for the selected walkthroughs.
4. Document the programming tools, reusable prototypes, and event/document path. Reuse the pet rock
   and Henri to introduce concepts before adding unrelated examples.
5. Document schedules and model setup, then the supplied model tools and agent extension path.
   Verify the latter against both agent implementations before prescribing an API.
6. Assemble the command and prototype references from those checked workflows, and check that
   in-world help points readers to the same concepts and commands.

For each new procedure, show the expected result and at least one useful diagnostic. Test it with
the stated role. Use MOO code when the task needs programming, and explain that code to a new
programmer. Do not replace available authoring commands with unexplained property assignments.
