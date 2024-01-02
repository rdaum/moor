```
    Multiuser,
  Online,
 Objects &
Relations
```

## What is?

"Moor" is:

  * A system for building shared, programmable, Internet-accessible virtual _social spaces_
  * A programming and authoring _tool_ for shared environments.
  * Fully compatible with LambdaMOO 1.8.x

Moor provides, in layers:

  * A fast secure transactional database with object and relational characteristics
  * A runtime for securely live-authoring and hosting programs and content that run persistently on that database
  * An authentication and authorization system for controlling access to said programs ("verbs") and content.
  * A programming language for building objects and verbs, along with the ability to plug in other languages / runtimes.
  * Tools and user environments for interacting with the system and writing verbs and editing content.

For:

  * Collaborative virtual environments
  * Socializing
  * Multiuser games
  * Persistent agents
  * Interactive fiction

### Background

In the early 1990s LambdaMOO was a popular online social environment, as well as a software package for building such 
environments yourself.

LambdaMOO still exists today, with an unbroken 30+ year history, and a small but dedicated community of users and
developers continue to use it -- both LambdaMOO the place, and MOO the server software for other communities.

MOO predates "social media", predates Facebook, Twitter, MySpace, Friendster, Tumblr, GeoCities, and... everything else.

In fact, it predates the world-wide web itself, and offers a very different kind of interaction with the Internet, one
that is text-based, not graphical, and is based around an evolving narrative that the users themselves create.

It is a multiuser virtual world, a MUD, a text-based "game", a chat room, a social network, a programming environment, 
and a platform for collaborative fiction. 

It is a place where people can meet, talk, and build things together.

### Back to the Future

But it some senses, the actual technology did not age well at all. It lacks multimedia of any kind, its interface is
arcane, it is not very accessible to new users, and the once active community of developers and participants has 
dwindled to a small but dedicated group of enthusiasts.

And the server itself is aged; it is written in C -- is single threaded, with some known architectural limitations, and
is not very easy to extend or modify. While there are newer versions and forks (such as Stunt, ToastStunt, etc.) that 
address many of these issues, they are still based on the same original codebase and architecture -- remaining bound by
the single-threaded, single-core model of the original.

Moor is an attempt to reimagine LambdaMOO for the modern world, while retaining the core concepts and ideas that made
it so compelling in the first place. It is a ground-up rewrite (in Rust). And while it maintains full compatibility with
existing LambdaMOO "cores" (databases, worlds), it also offers a new, more flexible and extensible architecture, and
extensions to the runtime to make it more adaptable to modern use cases:

  * A web-native architecture which allows for richer clients than a standard text-based terminal, including graphical
    clients, web clients, and mobile clients. Images, videos, emojis, rich text are all feasible, while keeping the
    narrative metaphor and creative aspects of the system intact.
  * A multi-core, multi-threaded, runtime, with a transactional, multiversion concurrency model instead of a global 
    lock on the database, as in MOO. This allows for theoretically greater scalability.
  * A flexible, pluggable virtual machine environment which allows "verbs" to be written in alternative languages, 
    such as JavaScript or WebAssembly modules (WIP).

### Why? 

Socializing, authoring, and creating on the Internet is in many ways broken. We want to make it better, by giving people
tools to create their _own_ spaces, and to create their own _tools_ within those spaces.

It should be fun, it should be easy, it should be accessible, it should be open, it should be collaborative, it should
be programmable, it should be extensible, it should be secure, it should be private, it should be free.

### How?

If you're an existing MOO administrator, you can run your existing MOO database on Moor, and it should work just fine, 
with the following caveats:

  * No external network connection support or builtins for that. (Web front ends and alternative protocols are done 
    in the Rust server layer, not in the MOO core.)
  * No support for the extensions present in ToastStunt, Stunt, etc. (e.g. `map` type, WAIFs etc.). (Some of these may
    come in the future.)

If you're a new user, you should use our provided core, which is custom-built for Moor, and provides a more modern
experience, with a web-based client, and a more modern programming environment.

notes on how to run & deploy here

### What's next?

new features roadmap here

