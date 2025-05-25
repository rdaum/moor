# Introduction

mooR is a network-accessible, multi-user, programmable, interactive system well-suited to the construction of online
social environments, games, conferencing systems, and other collaborative software. Its most common use, however, is as
a multi-participant, shared narrative system in the style of adventure games or "MUDs", and it is with this focus in
mind that I describe it here.

Participants (usually referred to as _players_) "connect" to mooR using a web browser -- or, historically through
`telnet`, `SSH`, or a specialized [mud client](https://en.wikipedia.org/wiki/MUD_client).

Upon connection, they are usually presented with a _welcome message_ explaining how to either create a new _character_
or connect to an existing one. Characters are the embodiment of players in the virtual reality that is mooR.

Having connected to a character, players then give one-line commands that are parsed and interpreted by mooR as
appropriate. Such commands may cause changes in the shared reality, such as the location of a character, or may simply
report on the current state of that reality, such as the appearance of some object.

The job of interpreting those commands is shared between the two major components in the mooR system: the _server_ and
the _database_. The server is a set of programs (written mostly in Rust), that manages the network connections,
maintains queues of commands and other tasks to be executed, controls all access to the database, and executes other
programs written in the MOO programming language.

The database contains representations of all the objects in the shared space, including the MOO programs that the server
executes to give those objects their specific behaviors.

Almost every command is parsed by the server into a call on a MOO procedure, or _verb_, that actually does the work.
Thus, programming in the MOO language is a central part of making non-trivial extensions to the database and thus, the
shared world and narrative.

In the next chapter, I describe the structure and contents of a mooR database. The following chapter gives a complete
description of how the server performs its primary duty: parsing the commands typed by players. Next, I describe the
complete syntax and semantics of the MOO programming language. Finally, I describe all of the database conventions
assumed by the server.

> Note: For the most part, this manual describes only those aspects of mooR that are entirely independent of the
> contents of the database. It does not describe, for example, the commands or programming interfaces present in the
> user's chosen database. There are exceptions to this, for situations where it seems prudent to delve deeper into these
> areas.

Finally, mooR itself is a rewrite of LambdaMOO -- a system from the 90s which pioneered the concepts described above.
mooR aims for full compatibility with existing LambdaMOO databases, but is in fact a from-scratch re-implementation with
its own extensions and modifications. The manual you are reading was written originally for LambdaMOO (and then modified
extensively for the ToastStunt fork of it), with sections modified and rewritten to reflect the changes or extensions
mooR has made.
