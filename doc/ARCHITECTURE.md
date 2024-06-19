## Moor Architecture & High-Level Technical Overview

The following is an attempt to provide a high-level overview of the architecture of the Moor system. This is considered
a "living document" and will be updated as the system evolves, and in response to feedback from readers.

### The Atmospheric view

Moor is a multi-player authoring system for shared social spaces.

It can be used for MUD-style games, for social networks, for chat, or for general shared application development.

It is, to start, compatible with LambdaMOO, a classic object oriented persistent MUD system from the 1990s.

But it is designed to be more flexible and extensible, and to support richer front-ends and more modern tools.

### 10-thousand foot view

**_Moor_** is a network-accessible virtual machine for running shared user programs (verbs)
in a shared persistent object environment.

The object environment is based around permissioned, shared, persistent objects which use prototype-inheritance to share
behaviour (verbs) and data (properties).

For now verbs are authored in a simple dynamically typed scripting language called `MOO`. It has a simple familiar syntax. But the system is written with
the intention of supporting multiple languages and runtimes in the future. JavaScript is likely to be the first target.

This object environment and language as implemented are compatible with LambdaMOO 1.8.x, a philosophically similar system first built
in the 1990s. Existing LambdaMOO databases ("cores") can be imported, and _should_ run without modification (with some caveats)
in Moor.

At least that's the starting point.

The main differences with LambdaMOO are:

- Moor is designed to have richer front ends, including a graphical web-based interface. LambdaMOO is restricted to
  a text-line based interface.
- Moor is multi-threaded, LambdaMOO is single-threaded
- Moor uses multi-version control, with transactional isolation to control shared access to objects.\
  LambdaMOO time-slices program execution with a global interpreter lock.
- Moor is written in Rust, and is designed to be more modular and extensible.

### 1-thousand foot view

Moor is a multi-process system. There is a single `daemon` process, and multiple "host" processes. The host processes
are the actual user interfaces, and the daemon process is the shared object environment and the execution engine.

_Process_ wise, in broad strokes:

in the `daemon` process

- the embedded database (`daemon/src/db/`)
- a bytecode-executing virtual machine (`daemon/src/vm/`)
- its associated builtin-functions (`daemon/src/builtins/`)
- a MOO language compiler (`compiler/src/`) and decompiler/pretty-printer
- a task scheduler for executing verbs in virtual machines (`daemon/src/scheduler/`)
- a ZeroMQ based RPC server for handling requests from the host processes (`daemon/src/rpc_server.rs`)

in each "_host_" process (e.g. `web`, `telnet`, `console`):

- a listen loop of some kind (HTTP, TCP, etc.)
- a ZeroMQ RPC client which turns events to/from the daemon into RPC events

Users access the system via one of these host processes, but the actual work is done `daemon` side.

### 100-foot view

Getting more into the weeds inside the daemon / kernel itself:

The `daemon` process is the heart of the system. It's a multithreaded server which listens for RPC requests from
clients, and manages the shared object environment and the execution of verbs.

The daemon process can be stopped and restarted while still maintaining active host connections. And vice versa,
_host_ processes can be stopped and restarted.

Host processes can be located on different machines, and can be added or removed at any time. In this way a Moor system
is designed to be flexible in terms of cluster deployment, at least from the front-end perspective. (Right now, there
is no support for distributing the daemon process & its embedded database.)

#### Database & objects

Moor objects are stored in a custom transactional (multi-version controlled) database.

The database itself is (currently) an in-memory system and the total world size is limited to the size of the system's
memory. This may change in the near future.

The database provides durability guarantees through a write-ahead log. As much as possible,
the system is designed to be "crash resilient" and to recover from stoppages quickly and without data loss.

The intent is to provide ACID guarantees, similar to a classic SQL RDBMS.

Embedding the database in the daemon process is done to ensure faster access to the data than would be possible with a
separate database server.

Moor objects themselves are broken up into many small pieces (relations), based on their individual attributes.
These relations are in fact a form of binary relations; collections of tuples which are composed of a domain ("key")
and codomain ("value"). The domain is always indexed, and is generally considered to be a unique key.

The database additionally supports secondary-indexing on the codomain.

Objects have verbs, properties, parents, and children. Each of these is stored in a separate relation.

All operations on the database are transactional, and the database supports a form of "serializable isolation" to provide
consistent views of the data.

#### Permissions

Moor objects are all permissioned. This permission system is based on the classic LambdaMOO model, which follows a
somewhat Unix-like Access Control List (ACL) model based around ownership and permission bits.

Every object, verb, and property has owners and permission bits which determine who can read, write (or execute) them.

The system has a built-in "wizard" role, which is a superuser role. Wizards can read, write, and execute anything.

This permission model is initial, and designed to be compatible with existing LambdaMOO cores. The long term plan is to
supplement/subsume this model with a more robust capability-based model.

#### The MOO language

The MOO language is a simple dynamically typed scripting language. It has a simple familiar syntax, and is designed to be
easy to learn and use. It is designed to be a "safe" language, and is designed to be used in a multi-user environment.

A manual for the MOO language can be found at [LambdaMOO Programmer's Manual](https://www.hayseed.net/MOO/manuals/ProgrammersManual.html).

The MOO language is compiled to opcodes, which are executed by a virtual machine. The opcode set is designed such that
the program can be "decompiled" back into a human-readable MOO code. In this way all verbs in the system can be read
and modified by any user (who has permissions), without the source code itself being stored in the database.

For now, Moor sticks to the classic LambdaMOO 1.8.x language without some of the extensions that were added in later
by the MOO community (such as those offered by Stunt or ToastStunt). So MOO does not have e.g. "WAIFs" (light weight objects),
or dictionary/map types. These may be added in the future.

#### The virtual machine

The virtual machine is a stack-based machine which executes opcodes. It is designed to be fast and efficient, and is
designed to be able to execute many verbs at once in multiple threads.

The system has been architected with the intent of supporting multiple languages and runtimes in the future. As such
each verb in the database is annotated with a "language" field, which will be used to determine which runtime to use.

At this time only the MOO language is supported.

#### The scheduler

The scheduler is a multi-threaded task scheduler which is responsible for executing verbs in the virtual machine. Every
top-level verb execution -- usually initiated by a user 'command' -- is scheduled as a task in the scheduler.

Each task is executed in a separate operating system thread. Additionally each task is given a separate database
transaction. For the duration of the execution, the task has a consistent view of the database, and is isolated from
other tasks.

When the task completes, an attempt is made to commit the transaction. If the commit fails because of a conflict with
data modified by another task, the task is retried. This is a form of "optimistic concurrency control". If the task
fails too many times, it is aborted and the user is informed.

#### Commands & verb executions.

The system has a built-in command parser which is responsible for parsing user input and converting it into a task
execution, which is scheduled in the scheduler. The command parser is exactly the same as the one used in LambdaMOO 1.8.x. which
is a fairly rudimentary English-like parser in the style of classic adventure games. See the LambdaMOO Programmer's Manual
for more details.

Top-level verb execution tasks can be additionally scheduled by RPC calls from the host processes. This is how e.g.
the web host process is able to execute verbs in response to HTTP requests to do things like read and write properties
on objects, independent of user commands.

#### RPC

The system uses ZeroMQ for inter-process communication. The daemon process listens on a ZeroMQ socket for RPC requests
from the host processes. The host processes use ZeroMQ to send requests to the daemon process. Each request is a
simple `bincode`-serialized message which is dispatched to the RPC handler in the daemon process.

Moor uses a simple request-response model for RPC. The host process sends a request, and the daemon process sends a
response.

Additionally, there are two broadcast `pubsub` channels which are used to send events from the daemon to the host processes:

- The `narrative` channel is used to send "narrative" events, which are used to inform the host processes of things
  that have happened in the world. This ultimately ties back to the MOO `notify` built-in function. For now this
  merely dispatches text strings, but in the future it will dispatch more structured events which clients can use
  to update their user interface or local model of the world. Other events that occur on this channel include:
  - `SystemMessage` for notifications of system-level events
  - `RequestInput` for prompting the user for input
  - `Disconnect` for notifying the user that they have been disconnected and requesting that the host close or
    invalidate the client connection
- The `broadcast` channel will be used to send system events, such as shutdown, restart, and other system-level events.
  (For now only "ping-pong" client live-ness check events are sent on this channel.)

#### Authentication / Authorization

MOO itself has a simple built-in authentication system. Users are identified by a "player" object, which is a special
kind of object in the database. The player object has a password. A login verb (`$do_login_command`) is used to
authenticate a user. Initial connections are given a "connection" object, which is used to represent their connection to the
system. Once authenticated, the connection object is replaced with the player object.

In Moor the authentication system is extended with the use of [PASETO](https://github.com/paseto-standard/paseto-spec) tokens. Every RPC call from a host process to the
daemon process is required to have a valid token. The token is used to identify the user and their permissions. The
tokens are signed by the daemon process, and granted at login time.

The same PASETO token system is used by the web host process to manage user sessions.

#### Front-end host processes

The system is designed to be flexible in terms of front-end host processes.

To maintain a classic LambdaMOO MUD-style interface, a `telnet` host process is provided. This is a simple TCP server
which listens for telnet connections and dispatches them to the daemon process.

To provide a more modern web-based interface, a `web` host process is provided. This is a simple HTTP server which
provides a RESTful API for interacting with the system for login, verb execution, and property retrieval. The web host
process additionally maintains a WebSockets connection to the daemon process for sending commands and receiving
narrative events in the same style as the telnet interface. In the future, additional WebSockets modalities will be
provided for receiving structured JSON events to provide a richer user interface.

In addition to these, a `console` host process is provided. This is a simple command-line interface which is used for
attaching to the daemon process in a manner similar to the telnet interface, but with history, tab-completion, and
other modern conveniences. In the future this tool will be extended to provide administrative and debugging tools.
