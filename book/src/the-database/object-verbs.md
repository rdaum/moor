# Object Verbs

The final kind of piece making up an object is _verbs_. A verb is a named MOO program that is associated with a
particular object. Most verbs implement commands that a player might type; for example, in the LambdaCore database,
there is a verb on all objects representing containers that implements commands of the form `put object in container`.

It is also possible for MOO programs to invoke the verbs defined on objects. Some verbs, in fact, are designed to be
used only from inside MOO code; they do not correspond to any particular player command at all. Thus, verbs in MOO are
like the _procedures_ or _methods_ found in some other programming languages.

> Note: There are even more ways to refer to _verbs_ and their counterparts in other programming language: _procedure_,
_function_, _subroutine_, _subprogram_, and _method_ are the primary ones. However, in _Object Oriented Programming_
> abbreviated _OOP_ you may primarily know them as methods.

## Verb ownership and permissions

As with properties, every verb has an owner and a set of permission bits. The owner of a verb can change its program,
its permission bits, and its argument specifiers (discussed below). Only a wizard can change the owner of a verb.

The owner of a verb also determines the permissions with which that verb runs; that is, the program in a verb can do
whatever operations the owner of that verb is allowed to do and no others. Thus, for example, a verb owned by a wizard
must be written very carefully, since wizards are allowed to do just about anything.

> Warning: This is serious business. The MOO has a variety of checks in place for permissions (at the object, verb and
> property levels) that are all but ignored when a verb is executing with a wizard's permissions. You may want to create
> a
> non-wizard character and give them the programmer bit, and write much of your code there, leaving the wizard bit for
> things that actually require access to everything, despite permissions.

| Permission Bit | Description                                  |
|----------------|----------------------------------------------|
| r (read)       | Let non-owners see the verb code             |
| w (write)      | Let non-owners write the verb code           |
| x (execute)    | Let verb be invoked from within another verb |
| d (debug)      | Let the verb raise errors to be caught       |

The permission bits on verbs are drawn from this set: `r` (read), `w` (write), `x` (execute), and `d` (debug). Read
permission lets non-owners see the program for a verb and, symmetrically, write permission lets them change that
program. The other two bits are not, properly speaking, permission bits at all; they have a universal effect, covering
both the owner and non-owners.

The execute bit determines whether or not the verb can be invoked from within a MOO program (as opposed to from the
command line, like the `put` verb on containers). If the `x` bit is not set, the verb cannot be called from inside a
program. This is most obviously useful for `this none this` verbs which are intended to be executed from within other
verb programs, however, it may be useful to set the `x` bit on verbs that are intended to be executed from the command
line, as then those can also
be executed from within another verb.

The setting of the debug bit determines what happens when the verb's program does something erroneous, like subtracting
a number from a character string. If the `d` bit is set, then the server _raises_ an error value; such raised errors can
be _caught_ by certain other pieces of MOO code. If the error is not caught, however, the server aborts execution of the
command and, by default, prints an error message on the terminal of the player whose command is being executed. (See the
chapter on server assumptions about the database for details on how uncaught errors are handled.) If the `d` bit is not
set, then no error is raised, no message is printed, and the command is not aborted; instead the error value is returned
as the result of the erroneous operation.

> Note: The `d` bit exists for historical reasons. Originally, MOO had no exception handling - errors were 
> always returned as values. The `d` bit was introduced to enable exception-style error handling that could be 
> caught with `try`-`except` statements. All new verbs should have the `d` bit set. Over time, old verbs written 
> assuming the `d` bit would not be set should be changed to use exception handling instead.

## Verb argument specifiers

In addition to an owner and some permission bits, every verb has three _argument specifiers_, one each for the
`direct object`, the `preposition`, and the `indirect object`. The direct and indirect specifiers are each drawn from
this set: `this`, `any`, or `none`. The preposition specifier is `none`, `any`, or one of the items in this list:

| Preposition              |
|--------------------------|
| with/using               |
| at/to                    |
| in front of              |
| in/inside/into           |
| on top of/on/onto/upon   |
| out of/from inside/from  |
| over                     |
| through                  |
| under/underneath/beneath |
| behind                   |
| beside                   |
| for/about                |
| is                       |
| as                       |
| off/off of               |

The argument specifiers are used in the process of parsing commands, described in the next chapter.
