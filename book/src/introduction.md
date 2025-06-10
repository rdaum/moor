# Introduction

mooR is a network-accessible, multi-user, programmable, interactive system well-suited to the construction of online
social environments, games, conferencing systems, and other collaborative software. The first application that mooR
supports is MOO. Think of a MOO as a text-based
virtual world, similar to an early predecessor of modern MMORPGs, but where both the environment and its rules can be
programmed by its participants. Its most common use, however, is as a multi-participant, shared narrative system in the
style of adventure games or "MUDs", and it is with this focus in
mind that I describe it here.

## Why MOOs are Special

MOOs offer a unique digital experience through:

- **Collaborative storytelling** where participants build a shared narrative
- **Creative freedom** to program and build your own spaces and objects
- **Community development** through persistent interactions and relationships
- **Text-based immersion** that engages imagination differently than graphics-heavy games

Participants (usually referred to as players) "connect" to mooR using a web browser -- or, historically through telnet,
SSH, or a specialized mud client. They interact with a virtual world that is represented by text, and their commands and
the results are conveyed via text.

The players can move around the virtual world and explore it, interact with other players or with simulated players or
other programmed entities, or with larger simulated devices or structures.

The player's actions can cause changes in that virtual world, and those changes are persistent; both the characters and
anything they create in the MOO world will persist until they are deliberately deleted. If the user disconnects and
reconnects days, months or even years later, they will still be there. If the server reboots, they will still be there.
Those changes can include moving things within the virtual world, altering things, and creating new things.

## A Typical MOO Interaction

Here's what a typical interaction looks like. A player sees a description of their surroundings, other characters
present, and objects they can interact with. They then type commands to interact with this virtual world:

> **The Living Room**
>
> It is very bright, open, and airy here, with large plate-glass windows looking southward over the pool to the gardens
> beyond. On the north wall, there is a rough stonework fireplace. The east and west walls are almost completely covered
> with large, well-stocked bookcases. An exit in the northwest corner leads to the kitchen and, in a more northerly
> direction, to the entrance hall. The door into the coat closet is at the north end of the east wall, and at the south
> end is a sliding glass door leading out onto a wooden deck. There are two sets of couches, one clustered around the
> fireplace and one with a view out the windows.
>
> You see Welcome Poster, a fireplace, the living room couch, Statue, a map of LambdaHouse, Fun Activities Board,
> Helpful
> Person Finder, The Birthday Machine, lag meter, and Cockatoo here.
>
> erin (out on his feet), elsa, lisdude (out on his feet), benny (out on his feet), and Fox (out on his feet) are here.
>
> **> poke cockatoo**
>
> The Cockatoo shifts about on its perch and bobs its head.
> Cockatoo squawks, "unless they are a brand new char with no objects."

## Getting Started

If you're new to MOOs, here's what to expect:

- **Creating a character**: Most MOOs have a registration process to create your persona
- **Basic navigation**: Commands like `look`, `go north`, or `@examine object` let you explore
- **Communication**: Use `say`, `emote`, or `page username` to interact with others
- **Help**: Type `help` or `@tutorial` for guidance specific to your MOO

The job of interpreting those commands is shared between the two major components in the mooR system: the _server_ and
the _database_. The server is a set of programs (written mostly in Rust), that manages the network connections,
maintains queues of commands and other tasks to be executed, controls all access to the database, and executes other
programs written in the MOO programming language.

The database contains representations of all the objects in the shared space, including the MOO programs that the server
executes to give those objects their specific behaviors.

Almost every command is parsed by the server into a call on a MOO procedure, or _verb_, that actually does the work.

So for example, when a player types `poke cockatoo`, the server parses that command and calls the `poke` verb on the
`cockatoo` object. The `poke` verb is a MOO procedure that defines what happens when a player pokes the cockatoo, and
here is an example of what that verb might look like in the database:

```moocode
> @list cockatoo:poke

#1479:"gag poke"   this none none
 1:  "gag/poke <this>";
 2:  "See the help for an extensive description of gag setting.";
 3:  "";
 4:  v = verb == "gag";
 5:  if (v)
 6:    if (this.gaggable == 0 && !(player == this.owner))
 7:      return player:tell("Only the owner can gag ", this.name, ".");
 8:    elseif ($object_utils:isa(player, $guest) && !this.guest_gaggable)
 9:      return player:tell("Guests can't gag ", this.name, ".  Feel free to join us by using @request to get a
character!");
10:    endif
11:  endif
12:  if (player.location != this.location)
13:    return player:tell("You need to be in the same room as ", this.name, " to do that.");
...
```

Programming in the MOO language is a central part of making non-trivial extensions to the database and thus, the shared
world and narrative.

## Moving Forward

Despite their text-based nature, MOOs continue to captivate users through their unique blend of social interaction,
collaborative creation, and programmable environments - offering an experience distinct from graphics-focused modern
games.

In the next chapter, I describe the structure and contents of a mooR database. The following chapter gives a complete
description of how the server performs its primary duty: parsing the commands typed by players. Next, I describe the
complete syntax and semantics of the MOO programming language. Finally, I describe aspects of server configuration and
administration, including how to run a mooR server and how to control the execution of tasks.



> Note: For the most part, this manual describes only those aspects of mooR that are entirely independent of the
> contents of the database. It does not describe, for example, the commands or programming interfaces present in the
> user's chosen database. There are exceptions to this, for situations where it seems prudent to delve deeper into these
> areas.

Finally, mooR itself is a rewrite of LambdaMOO -- a system from the 90s which pioneered the concepts described above.
mooR aims for full compatibility with existing LambdaMOO databases, but is in fact a from-scratch re-implementation with
its own extensions and modifications. The manual you are reading was written originally for LambdaMOO (and then modified
extensively for the ToastStunt fork of it), with sections modified and rewritten to reflect the changes or extensions
mooR has made.
