# Cowbell

<p align="center">
  <img src="padishah.png" alt="I need... more cowbell" width="400">
</p>

Cowbell is a starting world and a library of objects for communities that want to build a shared,
programmable social space.

A **MOO** is a shared world where people meet through text, explore rooms, and create places and
objects together. Participants can also program how those objects behave while the world runs. The
world keeps its objects, code, and changes between visits.

[mooR](../../README.md) is the server that runs the world. Cowbell supplies the initial objects and
code, including rooms, player characters, commands, and tools to extend them. In MOO terminology,
this starting database is a **core**. [Meadow](../../clients/meadow/README.md) is the browser client
through which people interact with the world. Cowbell supports plain text terminal clients too.

## What you can do

- **Meet and explore.** Talk, whisper, gesture, and move between rooms. The starting world includes
  connected rooms, furniture, and Henri, an interactive cat.
- **Build places and things.** Add rooms and passages, write descriptions, and create objects from
  supplied types such as containers, clothing, seats, and notes.
- **Give objects behavior.** Configure rules and reactions, or write MOO code for new commands and
  interactions.
- **Inspect and edit the world.** Meadow provides object information, action buttons, and editors
  alongside the conversation. In-world commands provide access to building and programming tools.
- **Experiment with language models.** Optional characters and tools include a concierge, a code
  explorer, and a building assistant.

## How the world works

### Objects and behavior

Rooms, people, and possessions are objects with stored information and code. A door, for example,
has a description and a lock state. Its code determines what happens when someone tries to open it.
New objects can inherit behavior from existing types, so each door does not need its own opening
code.

Rules describe conditions, such as whether someone carries a particular key. Reactions connect an
action to a response. A reaction can make a cat purr after someone pets it. Builders can combine
rules and reactions without a program for each interaction. Programmers can add new behaviors in MOO
code.

The [introduction to rules, reactions, and relations](docs/rules-reactions-relations.md) explains
how these parts fit together. The [builder's guide](manual/reactions.md) gives commands for a
responsive box, club membership, and a timed puzzle. These examples require builder privileges, with
no MOO programming.

The [rooms and passages guide](manual/rooms-and-passages.md) walks through building a workshop,
connecting its rooms, and adding a keyed door. The [portal tutorial](manual/custom-exits.md) shows
programmers how custom travel can appear in room descriptions and exit lists. Both tutorials run in
the test suite. In a fresh world, type `examples` in the First Room to visit the travel examples.

Objects also have owners and permissions. Cowbell provides ways to grant specific operations on an
object without handing over ownership. This lets people share building work and give tools limited
authority.

### Events and presentation

An **event** describes something that happens in the world, such as a person speaking or an object
moving. It identifies the participants and carries content to describe the action. Cowbell adapts
that description for each recipient. Alice sees “You say, ‘Hello!’” while someone else sees “Alice
says, ‘Hello!’”

The resulting description is a **document**: text organized into paragraphs, lists, tables, code,
and links to objects or commands. A **renderer** turns that document into a display format, such as
HTML for a browser or plain text for a terminal. World code can describe the content once and use it
in both clients.

Cowbell sends the formatted text together with the document. Meadow currently displays the text
formatted by the core. Other clients can use the document to choose their own presentation. Room
descriptions use the same content for the conversation view and Meadow's room panel.

Authors can use **Djot**, a text markup language similar to Markdown, for formatted descriptions and
help. Cowbell converts that markup into document structure. Authors can also choose literal text,
which keeps punctuation such as asterisks as written.

## Try Cowbell

Cowbell requires the **`main` branch of mooR**. It uses features that are not available in the
stable 1.0 release.

The [mooR quick start](../../README.md#quick-start) describes the bundled server and client setup.
The Cowbell launcher uses Docker Compose and Node.js/npm.

From the mooR repository root, run:

```sh
./scripts/start-moor-cowbell.sh
```

Then open [Meadow at localhost:8080](http://localhost:8080). A terminal connection is also available
at `localhost:8888`. The launcher keeps the world's saved data in `run-cowbell/`.

Once connected, type `help` for the topic list. Topics include conversation, movement, building, and
programming. Building and programming commands require the corresponding permissions in the world.

## Direction

We want communities to own and shape their social spaces, with room for creativity, story, and play.
Cowbell combines the text-centered culture of MOOs with browser interfaces for conversation,
exploration, and editing.

Cowbell is still in development. Current goals include more tools for builders, more documentation,
and asynchronous communication through mailboxes and bulletin boards. Mobile interaction and
accessibility are continuing design goals. The supplied objects focus on social worlds, with room
for communities to add game mechanics.

## Development and contribution

Cowbell lives in `cores/cowbell/` within the mooR repository. Its source files describe the objects
and code that make up the starting database. mooR can import and export these files, so world code
can be kept in version control.

For a development server and web client without Docker, use the
[frontend development setup](../../README.md#alternative-frontend-development-setup).

Run the Cowbell checks from the repository root:

```sh
make -C cores/cowbell check
```

Developer references cover specific parts of the project:

- [Runtime testing](RUNTIME_TESTING.md): test suites and focused checks.
- [Style guide](STYLE_GUIDE.md): conventions for Cowbell code.
- [Capabilities](CAPABILITIES.md): permissions and delegated authority.
- [Rule and reaction data](manual/behavior-model.md): internal condition representation and proposed
  reaction records.
- [Contextual suggestions](docs/suggestions.md): choices offered in Meadow's command inputs.
- [Audit matrix](AUDIT.md): review coverage and remaining work.

Contributions and questions are welcome on [Discord](https://discord.gg/Ec94y5983z).
