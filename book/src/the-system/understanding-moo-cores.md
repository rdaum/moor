# Understanding MOO Cores

To understand how MOO servers work, it's helpful to think about the relationship between the server software and the database that makes a MOO actually functional. This is where the concept of a "core" comes in.

## What is a MOO Core?

A MOO core is a starting database that contains all the fundamental objects, verbs, and systems needed to make a MOO actually work. Think of it like this:

**Hardware vs. Operating System Analogy:**
- **mooR server** = Computer hardware
- **MOO core** = Operating system

Just as a computer without an operating system is just expensive metal and silicon, a MOO server without a core database is just a program that can store objects but doesn't know how to do anything useful with them.

**House vs. Foundation Analogy:**
- **mooR server** = The foundation and basic structure
- **MOO core** = The electrical wiring, plumbing, and basic rooms that make it livable

## What's in a Core?

A typical MOO core includes:

- **Basic objects**: Players, rooms, containers, and other fundamental object types
- **Command verbs**: The code that handles commands like `look`, `say`, `get`, `drop`, etc.
- **System verbs**: Login/logout handling, communication systems, building commands
- **Utility objects**: Libraries for common programming tasks
- **Administrative tools**: Commands for managing the MOO (`@create`, `@recycle`, `@chmod`, etc.)
- **Social systems**: Who lists, mail systems, communication channels

Without these, your mooR server would just be an empty database with no way for players to interact meaningfully.

## Historical Context: LambdaCore

[**LambdaCore**](https://lambda.moo.mud.org/pub/MOO/) is the most famous and historically important MOO core. Created in the early 1990s at Xerox PARC, it established many of the conventions that MOO programmers still use today:

- The `@` prefix for administrative commands
- Standard object hierarchies (like `$thing`, `$room`, `$player`)
- Common verb patterns and programming idioms
- Basic building and social systems

LambdaCore became the foundation for probably hundreds of MOOs and influenced the design of many other virtual world systems. Most MOO programmers, even today, learned their craft on LambdaCore-derived systems.

## JaysHouseCore: A Technical Alternative

[**JaysHouseCore**](https://jhcore.sourceforge.net/) emerged from development at JaysHouse, a popular MOO in the 1990s that attracted a more technically-minded community. Unlike LambdaCore, which was designed for general use, JaysHouseCore was built with programmers and technical users in mind.

Key characteristics of JaysHouseCore include:
- **Enhanced programming tools**: More sophisticated development utilities
- **Technical focus**: Features designed for power users and MOO programmers
- **Proven stability**: [Waterpoint MOO](https://www.waterpoint.org/) is built overtop of JHCore

A copy of JHCore is included -- for testing purposes -- in the `mooR` github repository, and is the default core used by the included `docker compose` configuration.

## Toast Cores and Compatibility

**ToastStunt** (and its predecessor, Stunt) created enhanced cores that added many powerful features beyond what LambdaCore offered. However, Toast cores present challenges for mooR:

- **Format incompatibility**: Toast databases use a different storage format
- **Feature dependencies**: Toast cores rely on specific Toast-only features
- **Extension conflicts**: Some Toast extensions work differently than mooR's approach

While mooR implements many Toast-compatible features, it's not a drop-in replacement. Toast cores would need significant modification to run on mooR, but most importantly, mooR will mostly reject a textdump from ToastStunt, so it's best to avoid them.

## mooR's Future: The Cowbell Core

The mooR project is developing its own core called ["cowbell"](https://github.com/rdaum/cowbell/) (named with a nod to the famous "more cowbell" meme). Cowbell aims to:

- **Showcase mooR features**: Take advantage of mooR's unique capabilities and extensions
- **Web native UI**: Cowbell will be built with the web client in mind, and offer a rich media interface.
- **Modern approach**: Incorporate lessons learned from decades of MOO development
- **Clean foundation**: Start fresh rather than carrying forward historical baggage
- **Documentation**: Be well-documented and easy to understand for new programmers

**Current Status**: Cowbell is still in early development. The basics exist, but it needs significant work to be a fully-featured core suitable for production MOOs. See

**Contributions Welcome**: If you're interested in MOO development, contributing to cowbell is a great way to get involved. The project needs:
- Core object implementations
- Command verb programming
- Documentation and examples
- Testing and feedback
- User interface improvements

See: https://github.com/rdaum/cowbell/

## Choosing Your Path

When setting up a mooR server, you'll need to decide:

1. **Start with minimal**: Begin with basic objects and build your own systems
2. **Adapt existing code**: Port code from LambdaCore or other sources
3. **Wait for cowbell**: Follow cowbell development and contribute to its progress
4. **Hybrid approach**: Combine elements from multiple sources

Each approach has trade-offs in terms of effort, features, and long-term maintainability.

The next section will cover the practical mechanics of actually running a mooR server once you understand these foundational concepts.
