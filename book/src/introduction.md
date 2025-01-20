# Introduction

ToastStunt is a network-accessible, multi-user, programmable, interactive system well-suited to the construction of text-based adventure games, conferencing systems, and other collaborative software. Its most common use, however, is as a multi-participant, low-bandwidth virtual reality, and it is with this focus in mind that I describe it here.

Participants (usually referred to as _players_) connect to ToastStunt using a telnet, SSH, or specialized [mud client](https://en.wikipedia.org/wiki/MUD_client). Upon connection, they are usually presented with a _welcome message_ explaining how to either create a new _character_ or connect to an existing one. Characters are the embodiment of players in the virtual reality that is ToastStunt.

> Note: No one really connects to a MOO using just a telnet client these days. MUD Clients are incredibly common, and can connect on the telnet (or SSH) port. See the resources section for more information on these. There are even some web based clients ([dome-client](https://github.com/JavaChilly/dome-client.js)) out there that use websockets to connect to a MOO directly from the browser. And ToastStunt can be configured to offer secure connections using TLS.

Having connected to a character, players then give one-line commands that are parsed and interpreted by ToastStunt as appropriate. Such commands may cause changes in the virtual reality, such as the location of a character, or may simply report on the current state of that reality, such as the appearance of some object.

The job of interpreting those commands is shared between the two major components in the ToastStunt system: the _server_ and the _database_. The server is a program, written in a standard programming language, that manages the network connections, maintains queues of commands and other tasks to be executed, controls all access to the database, and executes other programs written in the MOO programming language. The database contains representations of all the objects in the virtual reality, including the MOO programs that the server executes to give those objects their specific behaviors.

Almost every command is parsed by the server into a call on a MOO procedure, or _verb_, that actually does the work. Thus, programming in the MOO language is a central part of making non-trivial extensions to the database and thus, the virtual reality.

In the next chapter, I describe the structure and contents of a ToastStunt database. The following chapter gives a complete description of how the server performs its primary duty: parsing the commands typed by players. Next, I describe the complete syntax and semantics of the MOO programming language. Finally, I describe all of the database conventions assumed by the server.

> Note: For the most part, this manual describes only those aspects of ToastStunt that are entirely independent of the contents of the database. It does not describe, for example, the commands or programming interfaces present in the ToastCore database. There are exceptions to this, for situations where it seems prudent to delve deeper into these areas.
