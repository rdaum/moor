# 'moor'; lambdaMOO all over again.

This is a rewrite / reimplementation of the LambdaMOO. It's done in Rust, and is intended to be a modernized version
of the original LambdaMOO server, with the goal of being able to run existing LambdaMOO worlds, but also to provide a
more modern foundation for future development.

(note: name is provisional and awful, suggestions accepted)

## Background on LambdaMOO

Chances are if you landed here you already know what this is, but I'll give a blurb:

LambdaMOO is/was a shared virtual world system similar to a (or kind of) MUD. It was originally written by Stephen White
(U Waterloo) and added to and maintained by Pavel Curtis (Xerox PARC) (and others) in C, and released in 1990. 

It was novel in that the bulk of the world's behaviour was implemented in a virtual machine, and the world itself was 
stored in a shared persistent programmable object database.

The world was structured -- like other MUDs -- with some of the aspects of interactive fiction / adventure games, where
users could move around, interact with objects, and interact with each other using a text-based interface. The focus
was primarily social interaction and creativity, and it was/is great.

There were many LambdaMOO worlds, and the most famous was LambdaMOO itself, which was a social experiment in shared
virtual space. It was a text-based virtual world, where users could create objects, and program them in a prototype-based
object-oriented language called "MOO". 

LambdaMOO is still in use today, and there are many active worlds running on it, but the community has on the whole lost
the bulk of its vitality. 

In its essence LambdaMOO is/was a live and interactive "social network" offering a somewhat richer social experience 
than what the web page-oriented systems of today offer. It's a shame that this line of evolution was mostly abandoned
in the 90s.

The original LambdaMOO server is still available, but is showing its age, and the codebase is not easy to work with to
add fundamentally new features like a modern database, or to support new network protocols, and most importantly, newer
user interface modalities / presentation modes.

## Project goals / status

The intent here is to start out at least fully compatible with LambdaMOO 1.8.x series and to be able to read and
execute existing cores, and the 1.0 feature release is targeting this rather ambitious but also rather restricted goal.
(primarily to maintain focus so I don't get distracted by the shiny things I've wanted to do for the last 30 years.)

### LambdaMOO is 30+ years old, why remain compatible?

* Because it's easy to go into the weeds creating new things, and never finishing. By having a concrete goal, and something
  to compare and test against, I may actually get somewhere.
* Because the *actual* useful and hard parts of those old MOO-type systems was the "user-space" type pieces (like
  LambdaCore/JHCore etc) and by making a new system run those old cores, there's more win.
* Because LambdaMOO itself is actually a very *complicated system with a lot of moving parts*; there's a compiler,  
  an object database, a virtual machine, a decompiler, and a network runtime all rolled into one. This, is, in some
  way... fun.

### Current status / features

* Pretty much feature complete / compatible with LambdaMOO 1.8.1 with a few caveats (see below)
* Can load and run LambdaMOO 1.8.x cores.
* Have tested against JaysHouseCore, and most of the functionality is there. Bugs are becoming increasingly rare.
* Hosts websocket, "telnet" (classic line oriented TCP connection), and console connections. MCP clients work, with
  remove editing, etc.
* Objects are stored in a RocksDB database, and safe and consistent and happy. Architecture allows for cleanly adding
  different storage backends.
* Monitoring/metrics support via Prometheus-compatible export.
* Separate network-host vs daemon process architecture means that upgrades/restarts can happen in-place without
  dropping live connections.

## How do I use it?

The easiest way to get started is to run the `docker compose` setup. This will bring up a complete server with `telnet`
and `websocket` interfaces. The server will be setup with an initial `JaysHouseCore` core import, and will be set up with
metrics monitoring via Grafana and VictoriaMetrics.

To do this, take a look at the local `docker-compose.yml` file, instructions are there, but it really just amounts to:

    `docker compose up`

Once you're familiar with how this setup works, you can get more creative. An actual production deployment can be fairly
easily derived from the `docker-compose.yml` file, and the provided `Dockerfile`.

### Missing / Next steps before 1.0

* Bugs, bugs, bugs. Collect em' all.
* Generally, open issues / missing features can be seen here: https://github.com/rdaum/moor/issues
* Major missing features:
    * Quota support.
    * Background tasks resumption after restart (from DB and from textdump load.)
    * Dump to a backup `textdump` format.
    * `$do_command`; LambdaMOO has the ability to attempt execution of a command through
      user code on `#0:do_command`; if that fails, it then dispatches through the regular
      built-in command handler. I need to get around to this.
    * `read`; This is used for prompts, password changes, editor, etc. It's slightly tricky
      because of the 'transactional' nature of I/O in Moor where all verb and I/O operations
      can be retried on transaction commit failure. Haven't decided what to do about this.
    * Actual transaction retry on commit-conflict. (Mainly because without actual users and stress testing I haven't
      been able to provoke this scenario to test against yet. The hooks are there, just not done)
* Improvements needed:
    * Performance improvements. Especially caching at the DB layer is missing and this thing will run dog slow
      without it
    * Better auth (SSO, OAuth2, etc?). Better crypt/password support.

### Unsupported features that might not get supported

* `encode_binary` & `decode_binary`:  These two functions allow for escaped binary
  sequences along with a network option for sending them, etc.
  But:
    * `moor`'s strings are utf8 so arbitrary byte sequences aren't going to cut it and
    * we're on a websocket, and have better ways of doing binary than encoding it into the
      output.
    * The alternative will be to provide a `binary` type that can be used for this purpose
      and to have special `notify` calls for emitting them to the client.
* Network connections, outbound and inbound (e.g. `open_network_connection`, `listen`,  
  `unlisten` etc). My intent is for the network service layer to be implemented at the Rust level, in the
  server daemon, not in MOO code.

### But then...

The following are targeted as eventual goals / additions once 1.0 (fall 2023) is out the door:

   * A richer front-end experience. Support for websockets as a connection method means that the server can provide  
     a richer narrative stream to browsers (with in-core support assistance.) A client which provides a full proper 
     UI experience with interactive UI components, graphical elements, and so on are the end-goal here.
   * Support for multiple programming language for programming MOO verbs/objects. The backend has been written such that
     this is feasible. Authoring verbs in JavaScript/TypeScript will be the first target, and WebAssembly modules are
     also a possibility. These verbs would still run within the same shared environment and use the same shared object
     environment, but would allow for a more modern programming experience.
   * A more scalable server architecture; the system right now is divided into separate "host" frontends for network  
     connections, and a common backend `daemon` which manages the database, virtual machine, and task scheduler. This
     can be further split up to permit a distributed database backend or distributing other components, to meet higher
     scalability goals if that is needed.
   * Enhancements to the MOO data model and language, to support a richer / smoother authoring experience. Some ideas 
     are:
     * Datalog-style relations / predicates; for managing logical relationships between entities. This could allow
       bidirectional (or more) relationships like already exist with e.g. `location`/`contents`, but more generalized,
       and to allow for making complex worlds easier to maintain.
     * Adding a map/dictionary type. MOO predates the existence of dictionary types as a standard type in most languages.  
       MOO's type system only has lists and uses "associative lists" for maps, which are a bit awkward. Immutable/CoW
       maps with an explicit syntax would be a nice addition. Other MOO offshoots (Stunt, etc.) do already provided this.
     * Adding a `binary` type. MOO's type system is very string-oriented, and there's not an elegant way to represent
       arbitrary binary data. (There's `encode_binary` and `decode_binary` builtins, but these are not the way I'd do it
       today.)
     * and so on

## Contribute and help!

Contributions are welcome and encouraged.

Right now the best way to contribute is to run the system and report bugs, or to try to run your own LambdaMOO core
and report bugs. (or to fix bugs and submit PRs!)

Ryan (ryan.daum@gmail.com)
