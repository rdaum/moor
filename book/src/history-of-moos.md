# A Brief History of MUDs and MOOs

_This section is derived from Steven J. Owens' excellent [LambdaMOO Programming Tutorial](https://github.com/sevenecks/lambda-moo-programming/blob/master/tutorials/lambda-moo-steven-owens-guide.md), with updates for the modern era and mooR._

To understand MOO and mooR, it helps to know where they came from. MOO didn't emerge in isolation—it evolved from decades of experimentation with multi-user virtual worlds, each building on the innovations of its predecessors. The story begins with the earliest text-based multi-user games and leads through a fascinating progression of technical and social innovations.

> The following is all off the top of my head and based on my recollections. I don't care enough to try to google on stuff.
> 
> —Steven J. Owens

## MUD and MUD2

MUD and MUD2 were the granddaddies of it all, written by Richard Bartle and Roy Trubshaw, back in the UK in the late 70s/very early 80s.

MUD stood for "Multi User Dungeon" and it was a very hack & slash D&D sort of experience. Later, other people tried to make it sound more legitimate by redefining it as "Multi User Dimension."

## AberMUD

A while after MUD2, Alan Cox (who later became a major Linux kernel dev) and friends at the university of Aberystwyth tried to recreate MUD2, or at least a MUD2-like thing.

## LPMud

Written in the late 80s by Lars Pensjö (except I misspelled that because it's a Finnish name and there's an umlaut or something over the "o"). For some reason they downcased the "MUD" acronym.

In general LPMuds had a very hack & slash D&D flavor/culture. There was no player persistence on LPMuds; when you quit the game, your player character disappeared and their stuff was left on the ground.

LPMud was programmable in a sense, though I never programmed in an LP. There was no provision for regular players to program/customize LPMud.

## DikuMUD

Written in 1990 and first opened to the public in 1991, DikuMUD was, like LPMud, inspired by AberMUD, but was intended to be an alternative to LPMud. I don't know much about it, I don't know that I ever logged into one, though it's quite possible.

## TinyMUD

Written in the late 80s by Jim Aspnes at Carnegie Mellon University, in Pittsburgh. Eponymous with the TinyMUD that Aspnes ran at CMU for a few years, which was later, after it was no longer regularly running, renamed "TinyMUD Classic" to distinguish it from later TinyMUDs.

TinyMUD was the first MUD I ever played.

TinyMUD was specifically designed to be lightweight in terms of system resources, hence the "tiny".

Player character objects were persistent in TinyMUD.

TinyMUD was very much a "social" MUD, maybe a "role playing, not roll playing" MUD. It had zero game/RPG support.

TinyMUD was the first, or one of the first, MUDs to allow all players to build stuff and it supported complex boolean locks that could, combined with player-buildable rooms and exits, be used to construct interesting puzzles.

TinyMUD was also, my friends recollect, the source of using the convention of prefixing server commands with the "@" character, i.e. "@commandname", as opposed to the commands used for regular interaction within the user-created world of the TinyMUD, which had no prefix.

Around about then, there was an explosion of different MUD types. My impression at the time was that a lot of them were TinyMUD derivatives, TinyMUSH and TinyMUCK being two of the more popular flavors that I can think of, offhand. TinyMUSH had a macro system and TinyMUCK was programmable via a Forth-like language (which is a bit of a niche type of programming language).

## MOO

Around 1990, a guy named Stephen White, aka Ghondarl or Ghond, created MOO, which stands for MUD, Object-Oriented. "Object oriented" is term for a particular kind of programming language, I get a little bit into that further down.

MOO had a limited programming language, perhaps more of a macro language.

I was an acquaintance of Ghond's, I logged into his MOO (retroactively named "AlphaMOO" by the MOO community) for an hour or two, while he was working on it.

## LambdaMOO

Early 1991, Pavel Curtis opened LambdaMOO.

For LambdaMOO Pavel developed the moo coding language (unnamed but referred to generally as "moo code") into a significantly more powerful and sophisticated programming language than MOO. White has been quite clear that he credits Pavel with doing major work and considers LambdaMOO to be Pavel's baby.

MOO and LambdaMOO's code are entirely unrelated to TinyMUD and similar, but it was philosophically descended from TinyMUD, in that it emphasized the social aspect, and users being able to build "live" in the MOO, via in-MOO commands.

LambdaMOO enables regular users to program in the MOO. To do this, you need to have the MOO object that represents your player character be programmer enabled, this is done by setting a flag on your player object; this flag is referred to as the "programmer bit", just an object property that is set to the value 1 to enable it. LambdaMOO is profligate in handing out programmer bits. Some MOOs are not.

Most people refer to the underlying server of LambdaMOO as simply "MOO". Ghondarl's original MOO is pretty much nonexistent at this point.

MOO coding is what's called in the programming world a "live coding" system, meaning you can interact with the running system and modify the code "live", and the changes take effect immediately, without having to restart/reload the system. Smalltalk is one of the more famous "real programming languages" that provided live coding, and MOO's object model was influenced by both Smalltalk and the Self programming language's prototype-based inheritance.

Everything in MOO is an object. Objects can have data attached to them, stored in "properties", and code attached to them, stored in "verbs".

## ColdMUD / Genesis

Between 1993-1994, Greg Hudson created **ColdMUD** as a from-scratch reimplementation inspired by MOO, but with a more elegant and modern approach to the programming language. ColdMUD featured a purer object-oriented design and introduced **ColdC**, a more sophisticated embedded programming language that could even be used independently of the MUD server.

ColdMUD was distinctive for its fully disk-based object database (unlike MOO's periodic checkpoint system) and represented an attempt to create a more software-engineering-quality virtual world authoring system.

The project evolved when Brandon Gillespie forked ColdMUD and renamed it **Genesis**, which became used by several online communities and games, including a very large commercial MUD called "The Eternal City."

ColdMUD/Genesis never achieved LambdaMOO's widespread adoption, but it showed that the ideas about object-oriented, user-programmable virtual worlds were in the air—multiple people were working on similar problems around the same time. Notably, the authors of mooR were quite involved in using ColdMUD and learned from its more elegant language design.

## Enter mooR: Modern MOO for the Modern Era

**mooR** represents a complete reimplementation of MOO that preserves its essential character while bringing it into the contemporary computing landscape.

Built from the ground up in Rust, mooR maintains everything that made MOO special—the live coding, the democratic programming where regular users can modify the world, the persistent object-oriented database where everything is an object. But it addresses the limitations that accumulated over MOO's decades of existence.

Classic MOO servers process commands one at a time, which could create bottlenecks during busy periods. mooR uses **transactions** to allow multiple commands to run simultaneously while keeping the database consistent—solving the concurrency challenges that limited classic MOO's scalability. (For details, see [Transactions in the MOO Database](the-database/transactions.md).)

Beyond performance, mooR extends the MOO language with modern features like maps, symbols, enhanced error handling, and improved type systems, while maintaining backward compatibility with existing MOO code.

mooR also brings MOO into alignment with contemporary expectations for security, hosting, and maintainability, making it practical to run MOO communities on modern infrastructure.

The goal isn't to replace what made MOO revolutionary, but to ensure those innovations remain accessible and viable for new generations of users who want to build collaborative, programmable virtual worlds.