# mooR

### ... What is this?

"**_mooR_**" is:

- A system for building shared, programmable, Internet-accessible virtual _social spaces_
- A programming and authoring _tool_ for shared environments.
- Compatible with [LambdaMOO](https://en.wikipedia.org/wiki/MOO) 1.8.x

_mooR_ provides (from the bottom layer up...)

- A fast, durable, transactional networked database with object and relational characteristics
- A runtime for securely live-authoring and hosting programs and content that run persistently on that database
- An authentication and authorization system for controlling access to said programs ("verbs") and content.
- A programming language for building objects and verbs, along with the ability to plug in other languages / runtimes.
- Tools and user environments for interacting with the system and writing verbs and editing content.
- Compatibility with the classic LambdaMOO programming language and runtime.

And it is designed to be used for:

- Collaborative virtual environments
- Socializing
- Multiuser games
- Persistent agents
- Interactive fiction
- Your entertainment and delight

mooR is written from the ground up, with reference to the LambdaMOO sources, but is not a fork of LambdaMOO.

A "core" foundation designed expecially for mooR lives at http://github.com/rdaum/cowbell

### And enhanced...

Enhancements over base the LambdaMOO 1.8.x system include (but are not limited to):

- Runtime features:
  - A fully multithreaded architecture, taking advantage of the wizardly powers of modern multicore computing
    machines.
  - A native web front end, with rich content presentation.
  - A text import / export format for objects that can be read by a human and edited by a standard text editor.
  - An architecture that is easier to extend and add to.
- Language features:
  - Lexically scoped variables / `begin` / `end` blocks
  - Maps: an associative container type (`[ "key" -> "value", ... ]`)
  - List / Range comprehensions, similar to Python, Julia, etc. (`{ x * 2 for x in [1..5]}`)
  - UTF-8 strings
  - 64-bit integers and floats
  - Symbol (interned string) type (`'symbol`)
  - Booleans (`true` / `false`)
  - "flyweights" - a lightweight anonymous reference counted immutable object / container type.

### Why?

Socializing, authoring, and creating on the Internet is in many ways broken. We want to make it better, by giving people
tools to create their _own_ spaces, and to create their own _things_ and _tools_ within those spaces.

It should be fun, it should be easy, it should be accessible, it should be open, it should be collaborative, it should
be programmable, it should be extensible, it should be secure, it should be private, it should be free.

This kind of environment is our take on how we can make that happen:

- Shared, self-authored, spaces
- Where you make things together
- Easy to learn tools
- Easy to share what you make
- Secure, and as private as you want it to be
- Driven around a shared narrative

In short: Build your own village.

### Background

Launching in the early 1990s [LambdaMOO](https://en.wikipedia.org/wiki/LambdaMOO) is an online social environment, as
well as an open source software package for building such environments for yourself.

LambdaMOO -- the place -- still exists today, with an unbroken 30+ year history, and a small but dedicated community of
users and developers continue to use it -- both [LambdaMOO the place](https://lambda.moo.mud.org/), and
[MOO the server software](https://github.com/wrog/lambdamoo) for other communities.

MOO predates "social media", predates Facebook, Twitter, MySpace, Friendster, Tumblr, GeoCities, and... everything else.

In fact, it predates the world-wide web itself, and offers a very different kind of interaction with the Internet, one
that is synchronous and live, text-based, not graphical, and is based around an evolving narrative that the users
themselves create.

It is a multiuser virtual world, a MUD, a narrative "game", a chat room, a virtual environment, a social network, a
programming environment, and a platform for collaborative fiction -- all in one.

It is a place where people can meet, talk, and build things together. And it's kind of awesome.

(for a longer description, see [doc/lambda-background.md](./doc/lambda-background.md))

### Back to the Future

But it some senses, the actual technology did not age well at all. It lacks multimedia of any kind, its interface is
dated, it is not very accessible to new users, and the once active community of developers and participants has
dwindled to a small but dedicated group of enthusiasts.

And the server itself is aged; it is written in C -- is single threaded, with some known architectural limitations, and
is not very easy to extend or modify. While there are newer versions and forks (such as Stunt, ToastStunt, etc.) that
address many of these issues, they are still based on the same original codebase and architecture -- remaining bound by
the single-threaded, single-core model of the original.

_mooR_ is an attempt to reimagine LambdaMOO for the modern world, while retaining the core concepts and ideas that made
it so compelling in the first place. It is a ground-up rewrite (in Rust). And while it maintains full compatibility with
existing LambdaMOO "cores" (databases, worlds), it also offers a new, more flexible and extensible architecture, and
extensions to the runtime to make it more adaptable to modern use cases:

- A web-native architecture which allows for richer clients than a standard text-based terminal, including graphical
  clients, web clients, and mobile clients. Images, videos, emojis, rich text are all feasible, while keeping the
  narrative metaphor and creative aspects of the system intact.
- A multi-core, multi-threaded, runtime, with a transactional, multiversion concurrency model instead of a global
  lock on the database, as in MOO. This allows for theoretically greater scalability.
- A flexible, pluggable virtual machine environment which allows "verbs" to be written in alternative languages,
  such as JavaScript or WebAssembly modules (WIP).

### How do I use it?

The easiest way to get started is to run the `docker compose` setup. This will bring up a complete server with `telnet`
and `web` interfaces. The server will be setup with an initial `JaysHouseCore` core import.

To run, take a look at the local `docker-compose.yml` file, instructions are there, but it really just amounts to:

```
docker compose up
```

This will bring up 3 containers:

- `moor-daemon` - the backend service that runs the actual MOO, but is not exposed to users
- `moor-telnet-host` - exposes a traditional MUD-style "telnet" (line-oriented-TCP) connection. On port 8888.
- `moor-web-host` - exposes a web client front end listen on port 8080.

So to connect, point your browser to `http://localhost:8080` or if you're feeling old-school: `telnet localhost 8888`

Studying the `docker-compose.yml` file should give some insight to how things are glued together.

### How is this thing put together? What even is it?

The server is written in Rust, and is composed of a number of crates and running services. These services communicate
with the main server process over an RPC system and message bus implemented on top of ZeroMQ.

For a high level architecture description plus a more detailed breakdown on how the server is put together, see the
[ARCHITECTURE.md](./doc/ARCHITECTURE.md) document.

### License?

_mooR_ is licensed under the GNU General Public License, version 3.0. See the [LICENSE](./LICENSE) file for details.

You can make modifications as you like, but if you distribute those modifications, you must also distribute the source
code for those modifications under the same license.

The choice to use the GPL was made to ensure that the software remains open and free, and that any modifications to it
are also open and free. This is in keeping with the spirit of the original LambdaMOO server, which was also under the
GPL license.

Further, since portions of the code inside `mooR` are based on readings of the LambdaMOO server code, staying with
the GPL is the right thing to do.

### Who made this?

The bulk of development has been by [myself](https://github.com/rdaum).

Extensive work on the decompiler/unparser, along with general testing, code sanitization, and cleanup has been done by
[Norman Nunley](https://github.com/nnunley).

Implementation of a robust integration testing framework, along with porting a pile of tests from ToastStunt, and
generally finding bugs and helping with the fixing of them has been done by [Zoltán Nagy](https://github.com/abesto).

Extensive testing has been done by many others.

There's been plenty of inspiration and help from a community of fellow old-school MOO
(and [ColdMUD](https://muds.fandom.com/wiki/ColdMUD)!) folks that I've known since the 90s.

Finally, LambdaMOO _itself_ was primarily authored by Pavel Curtis, with the original LambdaMOO server being written by
Stephen White. Successive versions and forks have been maintained by a number of people.

### What's done?

At this point `mooR` is capable of importing and running a full LambdaCore, JaysHouseCore, etc. database.

Everything should work. If it doesn't, that's a bug. Which you should report.

.... With _some_ caveats:

- Outbound network connections (`open_network_connection`) are not supported and likely won't be.
- Many/Most extensions present in ToastStunt, Stunt, etc. WAIFs, etc. are not supported. Some of these are
  possible to add in the future, others do not fit the design philosophy of the system going forward.

For a list of the status of the implementation of standard LambdaMOO builtin functions, see
[builtin_functions_status.md](./doc/builtin_functions_status.md).

### What's next?

There's a lot of work to do. We're looking for contributors, testers, and users. We're also looking for feedback, ideas,
and use cases.

We're also looking for funding, and for partners who want to build things on top of mooR.

The immediate horizon is to get the initial release out, which will be a drop-in replacement for LambdaMOO, with
some additional features. This will include a web-based client. To get there the following is still required

- Robustness and stability work.
- Documentation, including a user manual, a developer manual, and a system administrator manual.
- Performance testing to ensure that the system can handle a large number of users and objects.
- Continued development on a new core that can take advantage of mooR's rich-content/web presentation abilities.

### Join us!

If you're interested in helping out, or just want to chat, please join us on
our [Discord server](https://discord.gg/f5NnYJBfcG).

Note: When the time is right the Discord will be replaced by a running instance of `mooR` itself.
