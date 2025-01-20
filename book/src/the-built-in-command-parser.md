# The Built-in Command Parser

The MOO server is able to do a small amount of parsing on the commands that a player enters. In particular, it can break apart commands that follow one of the following forms:

- verb
- verb direct-object
- verb direct-object preposition indirect-object

Real examples of these forms, meaningful in the ToastCore database, are as follows:

```
look
take yellow bird
put yellow bird in cuckoo clock
```

Note that English articles (i.e., `the`, `a`, and `an`) are not generally used in MOO commands; the parser does not know that they are not important parts of objects' names.

To have any of this make real sense, it is important to understand precisely how the server decides what to do when a player types a command.

But first, we mention the three situations in which a line typed by a player is not treated as an ordinary command:

1. The line may exactly match the connection's defined flush command, if any (`.flush` by default), in which case all pending lines of input are cleared and nothing further is done with the flush command itself. Likewise, any line may be flushed by a subsequent flush command before the server otherwise gets a chance to process it. For more on this, see Flushing Unprocessed Input.
2. The line may begin with a prefix that qualifies it for out-of-band processing and thence, perhaps, as an out-of-band command. For more on this, see Out-of-band Processing.
3. The connection may be subject to a read() call (see section Operations on Network Connections) or there may be a .program command in progress (see section The .program Command), either of which will consume the line accordingly. Also note that if connection option "hold-input" has been set, all in-band lines typed by the player are held at this point for future reading, even if no reading task is currently active.

Otherwise, we (finally) have an actual command line that can undergo normal command parsing as follows:

The server checks whether or not the first non-blank character in the command is one of the following:

- `"`
- `:`
- `;`

If so, that character is replaced by the corresponding command below, followed by a space:

- `say`
- `emote`
- `eval`

For example this command:

```
"Hi, there.
```

will be treated exactly as if it were as follows:

```
say Hi, there.
```

The server next breaks up the command into words. In the simplest case, the command is broken into words at every run of space characters; for example, the command `foo bar baz` would be broken into the words `foo`, `bar`, and `baz`. To force the server to include spaces in a "word", all or part of a word can be enclosed in double-quotes. For example, the command:

```
foo "bar mumble" baz" "fr"otz" bl"o"rt
```

is broken into the words `foo`, `bar mumble`, `baz frotz`, and `blort`.

Finally, to include a double-quote or a backslash in a word, they can be preceded by a backslash, just like in MOO strings.

Having thus broken the string into words, the server next checks to see if the first word names any of the six "built-in" commands:

- `.program`
- `PREFIX`
- `OUTPUTPREFIX`
- `SUFFIX`
- `OUTPUTSUFFIX`
- or the connection's defined _flush_ command, if any (`.flush` by default).

The first one of these is only available to programmers, the next four are intended for use by client programs, and the last can vary from database to database or even connection to connection; all six are described in the final chapter of this document, "Server Commands and Database Assumptions". If the first word isn't one of the above, then we get to the usual case: a normal MOO command.

The server next gives code in the database a chance to handle the command. If the verb `$do_command()` exists, it is called with the words of the command passed as its arguments and `argstr` set to the raw command typed by the user. If `$do_command()` does not exist, or if that verb-call completes normally (i.e., without suspending or aborting) and returns a false value, then the built-in command parser is invoked to handle the command as described below. Otherwise, it is assumed that the database code handled the command completely and no further action is taken by the server for that command.

> Note: `$do_command` is a corified reference. It refers to the verb `do_command` on #0. More details on corifying properties and verbs are presented later.

If the built-in command parser is invoked, the server tries to parse the command into a verb, direct object, preposition and indirect object. The first word is taken to be the verb. The server then tries to find one of the prepositional phrases listed at the end of the previous section, using the match that occurs earliest in the command. For example, in the very odd command `foo as bar to baz`, the server would take `as` as the preposition, not `to`.

If the server succeeds in finding a preposition, it considers the words between the verb and the preposition to be the direct object and those after the preposition to be the indirect object. In both cases, the sequence of words is turned into a string by putting one space between each pair of words. Thus, in the odd command from the previous paragraph, there are no words in the direct object (i.e., it is considered to be the empty string, `""`) and the indirect object is `"bar to baz"`.

If there was no preposition, then the direct object is taken to be all of the words after the verb and the indirect object is the empty string.

The next step is to try to find MOO objects that are named by the direct and indirect object strings.

First, if an object string is empty, then the corresponding object is the special object `#-1` (aka `$nothing` in ToastCore). If an object string has the form of an object number (i.e., a hash mark (`#`) followed by digits), and the object with that number exists, then that is the named object. If the object string is either `"me"` or `"here"`, then the player object itself or its location is used, respectively.

> Note: $nothing is considered a `corified` object. This means that a _property_ has been created on `#0` named `nothing` with the value of `#-1`. For example (after creating the property): `;#0.nothing = #-1` This allows you to reference the `#-1` object via it's corified reference of `$nothing`. In practice this can be very useful as you can use corified references in your code (and should!) instead of object numbers. Among other benefits this allows you to write your code (which references other objects) once and then swap out the corified reference, pointing to a different object. For instance if you have a new error logging system and you want to replace the old $error_logger reference with your new one, you wont have to find all the references to the old error logger object number in your code. You can just change the property on `#0` to reference the new object.

Otherwise, the server considers all of the objects whose location is either the player (i.e., the objects the player is "holding", so to speak) or the room the player is in (i.e., the objects in the same room as the player); it will try to match the object string against the various names for these objects.

The matching done by the server uses the `aliases` property of each of the objects it considers. The value of this property should be a list of strings, the various alternatives for naming the object. If it is not a list, or the object does not have an `aliases` property, then the empty list is used. In any case, the value of the `name` property is added to the list for the purposes of matching.

The server checks to see if the object string in the command is either exactly equal to or a prefix of any alias; if there are any exact matches, the prefix matches are ignored. If exactly one of the objects being considered has a matching alias, that object is used. If more than one has a match, then the special object `#-2` (aka `$ambiguous_match` in ToastCore) is used. If there are no matches, then the special object `#-3` (aka `$failed_match` in ToastCore) is used.

So, now the server has identified a verb string, a preposition string, and direct- and indirect-object strings and objects. It then looks at each of the verbs defined on each of the following four objects, in order:

1. the player who typed the command
2. the room the player is in
3. the direct object, if any
4. the indirect object, if any.

For each of these verbs in turn, it tests if all of the the following are true:

- the verb string in the command matches one of the names for the verb
- the direct- and indirect-object values found by matching are allowed by the corresponding _argument specifiers_ for the verb
- the preposition string in the command is matched by the _preposition specifier_ for the verb.

I'll explain each of these criteria in turn.

Every verb has one or more names; all of the names are kept in a single string, separated by spaces. In the simplest case, a verb-name is just a word made up of any characters other than spaces and stars (i.e., ' ' and `*`). In this case, the verb-name matches only itself; that is, the name must be matched exactly.

If the name contains a single star, however, then the name matches any prefix of itself that is at least as long as the part before the star. For example, the verb-name `foo*bar` matches any of the strings `foo`, `foob`, `fooba`, or `foobar`; note that the star itself is not considered part of the name.

If the verb name _ends_ in a star, then it matches any string that begins with the part before the star. For example, the verb-name `foo*` matches any of the strings `foo`, `foobar`, `food`, or `foogleman`, among many others. As a special case, if the verb-name is `*` (i.e., a single star all by itself), then it matches anything at all.

Recall that the argument specifiers for the direct and indirect objects are drawn from the set `none`, `any`, and `this`. If the specifier is `none`, then the corresponding object value must be `#-1` (aka `$nothing` in ToastCore); that is, it must not have been specified. If the specifier is `any`, then the corresponding object value may be anything at all. Finally, if the specifier is `this`, then the corresponding object value must be the same as the object on which we found this verb; for example, if we are considering verbs on the player, then the object value must be the player object.

Finally, recall that the argument specifier for the preposition is either `none`, `any`, or one of several sets of prepositional phrases, given above. A specifier of `none` matches only if there was no preposition found in the command. A specifier of `any` always matches, regardless of what preposition was found, if any. If the specifier is a set of prepositional phrases, then the one found must be in that set for the specifier to match.

So, the server considers several objects in turn, checking each of their verbs in turn, looking for the first one that meets all of the criteria just explained. If it finds one, then that is the verb whose program will be executed for this command. If not, then it looks for a verb named `huh` on the room that the player is in; if one is found, then that verb will be called. This feature is useful for implementing room-specific command parsing or error recovery. If the server can't even find a `huh` verb to run, it prints an error message like `I couldn't understand that.` and the command is considered complete.

At long last, we have a program to run in response to the command typed by the player. When the code for the program begins execution, the following built-in variables will have the indicated values:

| Variable | Value                                                    |
| -------- | -------------------------------------------------------- |
| player   | an object, the player who typed the command              |
| this     | an object, the object on which this verb was found       |
| caller   | an object, the same as <code>player</code>               |
| verb     | a string, the first word of the command                  |
| argstr   | a string, everything after the first word of the command |
| args     | a list of strings, the words in <code>argstr</code>      |
| dobjstr  | a string, the direct object string found during parsing  |
| dobj     | an object, the direct object value found during matching |
| prepstr  | a string, the prepositional phrase found during parsing  |
| iobjstr  | a string, the indirect object string                     |
| iobj     | an object, the indirect object value                     |

The value returned by the program, if any, is ignored by the server.
