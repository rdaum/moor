# Server Commands and Database Assumptions

This chapter describes all of the commands that are built into the server and every property and verb in the database specifically accessed by the server. Aside from what is listed here, no assumptions are made by the server concerning the contents of the database.

## Command Lines That Receive Special Treatment

As was mentioned in the chapter on command parsing, there are a number of commands and special prefixes whose interpretation is fixed by the server. Examples include the flush command and the five intrinsic commands (PREFIX, OUTPUTPREFIX, SUFFIX, OUTPUTSUFFIX, and .program).

This section discusses all of these built-in pieces of the command-interpretation process in the order in which they occur.

### Flushing Unprocessed Input

It sometimes happens that a user changes their mind about having typed one or more lines of input and would like to `untype` them before the server actually gets around to processing them. If they react quickly enough, they can type their connection`s defined flush command; when the server first reads that command from the network, it immediately and completely flushes any as-yet unprocessed input from that user, printing a message to the user describing just which lines of input were discarded, if any.

> Fine point: The flush command is handled very early in the server`s processing of a line of input, before the line is entered into the task queue for the connection and well before it is parsed into words like other commands. For this reason, it must be typed exactly as it was defined, alone on the line, without quotation marks, and without any spaces before or after it.

When a connection is first accepted by the server, it is given an initial flush command setting taken from the current default. This initial setting can be changed later using the set_connection_option() command.

By default, each connection is initially given `.flush` as its flush command. If the property $server_options.default_flush_command exists, then its value overrides this default. If $server_options.default_flush_command is a non-empty string, then that string is the flush command for all new connections; otherwise, new connections are initially given no flush command at all.

### Out-of-Band Processing

It is possible to compile the server to recognize an out-of-band prefix and an out-of-band quoting prefix for input lines. These are strings that the server will check for at the beginning of every unflushed line of input from a non-binary connection, regardless of whether or not a player is logged in and regardless of whether or not reading tasks are waiting for input on that connection.

This check can be disabled entirely by setting connection option "disable-oob", in which case none of the rest of this section applies, i.e., all subsequent unflushed lines on that connection will be available unchanged for reading tasks or normal command parsing.

### Quoted Lines

We first describe how to ensure that a given input line will not be processed as an out-of-band command.

If a given line of input begins with the defined out-of-band quoting prefix (`#$"` by default), that prefix is stripped. The resulting line is then available to reading tasks or normal command parsing in the usual way, even if said resulting line now happens to begin with either the out-of-band prefix or the out-of-band quoting prefix.

For example, if a player types

```
#$"#$#mcp-client-set-type fancy
```

the server would behave exactly as if connection option "disable-oob" were set true and the player had instead typed

```
#$#mcp-client-set-type fancy
```

### Commands

If a given line of input begins with the defined out-of-band prefix (`#$#` by default), then it is not treated as a normal command or given as input to any reading task. Instead, the line is parsed into a list of words in the usual way and those words are given as the arguments in a call to $do_out_of_band_command().

If this verb does not exist or is not executable, the line in question will be completely ignored.

For example, with the default out-of-band prefix, the line of input

```
#$#mcp-client-set-type fancy
```

would result in the following call being made in a new server task:

```
$do_out_of_band_command("#$#mcp-client-set-type", "fancy")
```

During the call to $do_out_of_band_command(), the variable player is set to the object number representing the player associated with the connection from which the input line came. Of course, if that connection has not yet logged in, the object number will be negative. Also, the variable argstr will have as its value the unparsed input line as received on the network connection.

Out-of-band commands are intended for use by advanced client programs that may generate asynchronous events of which the server must be notified. Since the client cannot, in general, know the state of the player`s connection (logged-in or not, reading task or not), out-of-band commands provide the only reliable client-to-server communications channel.

[Telnet IAC](http://www.faqs.org/rfcs/rfc854.html) commands will also get captured and passed, as binary strings, to a `do_out_of_band_command` verb on the listener.

### Command-Output Delimiters

> Warning: This is a deprecated feature

Every MOO network connection has associated with it two strings, the `output prefix` and the `output suffix`. Just before executing a command typed on that connection, the server prints the output prefix, if any, to the player. Similarly, just after finishing the command, the output suffix, if any, is printed to the player. Initially, these strings are not defined, so no extra printing takes place.

The `PREFIX` and `SUFFIX` commands are used to set and clear these strings. They have the following simple syntax:

```
PREFIX  output-prefix
SUFFIX  output-suffix
```

That is, all text after the command name and any following spaces is used as the new value of the appropriate string. If there is no non-blank text after the command string, then the corresponding string is cleared. For compatibility with some general MUD client programs, the server also recognizes `OUTPUTPREFIX` as a synonym for `PREFIX` and `OUTPUTSUFFIX` as a synonym for `SUFFIX`.

These commands are intended for use by programs connected to the MOO, so that they can issue MOO commands and reliably determine the beginning and end of the resulting output. For example, one editor-based client program sends this sequence of commands on occasion:

```
PREFIX >>MOO-Prefix<<
SUFFIX >>MOO-Suffix<<
@list object:verb without numbers
PREFIX
SUFFIX
```

The effect of which, in a ToastCore-derived database, is to print out the code for the named verb preceded by a line containing only `>>MOO-Prefix<<` and followed by a line containing only `>>MOO-Suffix<<`. This enables the editor to reliably extract the program text from the MOO output and show it to the user in a separate editor window. There are many other possible uses.

> Warning: If the command thus bracketed calls suspend(), its output will be deemed “finished” then and there; the suffix thus appears at that point and not, as one might expect, later when the resulting background task has finally returned from its top-level verb call. Thus, use of this feature (which was designed before suspend() existed) is no longer recommended.

The built-in function `output_delimiters()` can be used by MOO code to find out the output prefix and suffix currently in effect on a particular network connection.

## The .program Command

The `.program` command is a common way for programmers to associate a particular MOO-code program with a particular verb. It has the following syntax:

```
.program object:verb
...several lines of MOO code...
.
```

That is, after typing the `.program` command, then all lines of input from the player are considered to be a part of the MOO program being defined. This ends as soon as the player types a line containing only a dot (`.`). When that line is received, the accumulated MOO program is checked for proper MOO syntax and, if correct, associated with the named verb.

If, at the time the line containing only a dot is processed, (a) the player is not a programmer, (b) the player does not have write permission on the named verb, or (c) the property `$server_options.protect_set_verb_code` exists and has a true value and the player is not a wizard, then an error message is printed and the named verb's program is not changed.

In the `.program` command, object may have one of three forms:

- The name of some object visible to the player. This is exactly like the kind of matching done by the server for the direct and indirect objects of ordinary commands. See the chapter on command parsing for details. Note that the special names `me` and `here` may be used.
- An object number, in the form `#number`.
- A _system property_ (that is, a property on `#0`), in the form `$name`. In this case, the current value of `#0.name` must be a valid object.

## Initial Punctuation in Commands

The server interprets command lines that begin with any of the following characters specially:

```
"        :        ;
```

Before processing the command, the initial punctuation character is replaced by the corresponding word below, followed by a space:

```
say      emote    eval
```

For example, the command line

```
"Hello, there.
```

is transformed into

```
say Hello, there.
```

before parsing.
